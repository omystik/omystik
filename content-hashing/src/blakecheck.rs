use anyhow::{anyhow, Result, Error};
use std::path::Path;
use blake3::{Hash, Hasher};

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::fs;
//use libp2p::futures::{AsyncWriteExt, AsyncReadExt};

const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB

/// Sends the file in chunks, then sends the 32-byte BLAKE3 hash.
/// Layout:
///  1) 8 bytes:  little-endian file length
///  2) file content (that many bytes)
///  3) 32 bytes: BLAKE3 hash
pub async fn send_file_with_hash<S>(
    mut stream: S,
    file_path: &Path,
    cid: &str,
) -> Result<(), Error> where S: AsyncWriteExt + Unpin, {

    //tracing::debug!("Launch@send_file_with_hash");

    // Open the file (blocking std I/O for demo).
    let mut file = File::open(file_path).await?;
    //tracing::debug!("ChunkBlake file \nSEND\nFile path {:?}", file_path);
    
    // Compute total size (so we can write it up front).
    let metadata = file.metadata().await?;
    let file_len = metadata.len();

    // Prepare a hasher to feed bytes as we send them.
    let mut hasher = Hasher::new();

    // 1) Write file length as 8 bytes (little-endian).
    stream.write_all(&file_len.to_le_bytes()).await?;

    // 2) Read file in chunks, write to `stream`, and update the hasher.
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut remaining = file_len;

    while remaining > 0 {
        //tracing::debug!("ChunkBlake file \nSEND\nFile length remaining {}\n hasher {:?}", remaining, hasher);
        let to_read = CHUNK_SIZE.min(remaining as usize);
        let n = file.read(&mut buffer[..to_read]).await?;
        if n == 0 {
            break; // EOF unexpectedly
        }
        hasher.update(&buffer[..n]);
        stream.write_all(&buffer[..n]).await?;
        remaining -= n as u64;

    }

    // 3) Finalize hash and send it (32 bytes).
    let hash: Hash = hasher.finalize();
    stream.write_all(hash.as_bytes()).await?;

    // 4) Append the CID
    // Send CID length as 8-byte little-endian
    let cid_bytes = cid.as_bytes();
    let cid_length = cid_bytes.len() as u64;
    stream.write_all(&cid_length.to_le_bytes()).await?;
    // then send the actual CID
    stream.write_all(cid_bytes).await?;

    // 5) Close the stream (important!).
    stream.flush().await?;
    stream.shutdown().await?;

    //tracing::debug!("Send file with hash is shuted down");

    Ok(())
}

/// Receives the file in chunks, writes it to `output_path`, and verifies the BLAKE3 hash.
/// Layout expected:
///  1) 8 bytes:  little-endian file length
///  2) file content (that many bytes)
///  3) 32 bytes: BLAKE3 hash
pub async fn receive_file_with_hash<R>(
    mut reader: R,
    output_path: &Path,
) -> Result<String, Error> where R: AsyncReadExt + Unpin, {

    // Prepare a file for writing.
    let temp_name_file = output_path.join("temp.bin");
    let mut out_file = File::create(&temp_name_file).await?;
    let mut hasher = Hasher::new();

    // Step 1: Read the first 8 bytes to get the file length.
    let mut len_buf = [0u8; 8];
    reader.read_exact(&mut len_buf).await?;
    let file_len = u64::from_le_bytes(len_buf);
    //tracing::debug!("Expected file length: {}", file_len);

    // Step 2: Read the file content based on the file length.
    let mut remaining = file_len;
    let mut buffer = vec![0u8; CHUNK_SIZE];
    while remaining > 0 {
        let to_read = CHUNK_SIZE.min(remaining as usize);
        let n = reader.read(&mut buffer[..to_read]).await?;
        if n == 0 {
            return Err(anyhow!("Unexpected EOF while reading file content."));
        }
        out_file.write_all(&buffer[..n]).await?;
        hasher.update(&buffer[..n]);
        remaining -= n as u64;
        //tracing::debug!("Received {} bytes, {} bytes remaining.", n, remaining);
    }

    // Step 3: Read the final 32 bytes as the BLAKE3 hash.
    let mut hash_buf = [0u8; 32];
    reader.read_exact(&mut hash_buf).await?;
    let received_hash = Hash::from_bytes(hash_buf);

    // Compute the hash of the received data.
    let computed_hash = hasher.finalize();

    //tracing::debug!("Received hash: {:?}", received_hash);
    //tracing::debug!("Computed hash: {:?}", computed_hash);

    // Verify the integrity of the received file.
    if computed_hash != received_hash {
        return Err(anyhow!("Hash mismatch! File integrity compromised."));
    }

    //tracing::debug!("File received and verified successfully.");

    // Step 4: Read the CID
    let mut cid_len_buf = [0u8; 8];
    reader.read_exact(&mut cid_len_buf).await?;
    let cid_length = u64::from_le_bytes(cid_len_buf);
    let mut cid_bytes = vec![0u8; cid_length as usize];
    reader.read_exact(&mut cid_bytes).await?;
    let cid = String::from_utf8(cid_bytes)
        .map_err(|e| anyhow!("Invalid CID encoding: {:?}", e))?;

    //tracing::debug!("Received CID: {}", cid);

    // Step 5: rename temp file to output_path/cid.bin
    let final_path = output_path.join(format!("{}.bin", cid));

    fs::rename(&temp_name_file, &final_path).await?;
    
    Ok(cid)
}

// Simple transfer of key and its name.
pub async fn send_key<S>(
    mut stream: S,
    key_path: &Path,
) -> Result<(), Error>
where
    S: AsyncWriteExt + Unpin,
{
    let key_name = key_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid key file name"))?
        .to_string();

    let key_name_bytes = key_name.as_bytes();
    let key_name_len = key_name_bytes.len() as u64;

    let mut file = File::open(key_path).await?;
    let metadata = file.metadata().await?;
    let key_len = metadata.len();

    // 1) send key name length + key name
    stream.write_all(&key_name_len.to_le_bytes()).await?;
    stream.write_all(key_name_bytes).await?;

    // 2) send key payload length
    stream.write_all(&key_len.to_le_bytes()).await?;

    // 3) send key payload
    let mut buffer = vec![0u8; 8 * 1024];
    let mut remaining = key_len;

    while remaining > 0 {
        let to_read = buffer.len().min(remaining as usize);
        let n = file.read(&mut buffer[..to_read]).await?;
        if n == 0 {
            break;
        }

        stream.write_all(&buffer[..n]).await?;
        remaining -= n as u64;
    }

    stream.flush().await?;
    stream.shutdown().await?;

    Ok(())
}

// receiving key and its name.
pub async fn receive_key<R>(reader: &mut R) -> Result<(Vec<u8>, String), Error>
where
    R: AsyncReadExt + Unpin,
{
    // 1) read key name length
    let mut name_len_buf = [0u8; 8];
    reader.read_exact(&mut name_len_buf).await?;
    let key_name_len = u64::from_le_bytes(name_len_buf) as usize;

    if key_name_len == 0 {
        return Err(anyhow!("Received empty key name"));
    }

    // 2) read key name
    let mut key_name_buf = vec![0u8; key_name_len];
    reader.read_exact(&mut key_name_buf).await?;
    let key_name = String::from_utf8(key_name_buf)
        .map_err(|e| anyhow!("Invalid key name encoding: {}", e))?;

    // 3) read key payload length
    let mut len_buf = [0u8; 8];
    reader.read_exact(&mut len_buf).await?;
    let key_len = u64::from_le_bytes(len_buf) as usize;

    if key_len == 0 {
        return Err(anyhow!("Received empty key payload"));
    }

    // 4) read key payload
    let mut key_bytes = vec![0u8; key_len];
    reader.read_exact(&mut key_bytes).await?;

    Ok((key_bytes, key_name))
}