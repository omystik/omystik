use std::fs::{self, File};
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::Path;
use blake3::Hasher;
use std::result::Result::Ok;

pub mod blakecheck;

use anyhow::{anyhow, Error, Result};

use chacha20poly1305::aead::{OsRng, stream::{EncryptorBE32, DecryptorBE32}};

use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use rand::RngCore;

// pub const STORAGE_DIR: &str = "./data/content/";
pub fn get_storage_dir() -> String {
    std::env::var("STORAGE_DIR").unwrap_or_else(|_| "./data/content/".to_string())
}

// pub const KEYS_DIR: &str = "./data/core/keys/";
pub fn get_keys_dir() -> String {
    std::env::var("KEYS_DIR").unwrap_or_else(|_| "./data/core/keys/".to_string())
}

const BUFFER_SIZE: usize = 1024 * 2048;

///// CRYPTOGRAPHY SECTION
pub async fn newkey() -> Result<([u8; 32], [u8; 19]), Error> {

    let mut key = [0u8; 32];
    let mut nonce = [0u8; 19];

    OsRng.fill_bytes(&mut key);
    OsRng.fill_bytes(&mut nonce);
    
    Ok((key, nonce))
}

async fn check_cid(encryption_case: bool, cid: Option<&str>, input_doc_path: Option<&str>) -> Result<bool, Error>{
    /*
    2 different behaviours:
    - encryption case: chech_cid(true, cid needed), encryption is aborded if:
        > data folder or CID not provided,
        > CID founded in data folder. 
    
    - Decryption case: chech_cid(false, input_doc_path needed), decryption is aborded if:
        > input_doc_path not provided,
        > CID not founded in data folder. 
    */

    // while in encryption case
    if encryption_case {
    if input_doc_path.is_none() {
        println!("Encryption case: No CID provided");
        return Ok(false);
    }
    let value = input_doc_path.unwrap();

    if !fs::exists(value)? {
        //println!("Mission does not exist yet");
        return Ok(false);
    }

    // Iterate over the encrypted data directory
    let entries = match fs::read_dir(value) {
        Ok(entries) => entries,
        Err(e) => {
            println!("Encryption case: Error reading directory: {}", e);
            return Ok(false);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue, // Skip invalid entries
        };

        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(value_cid) = cid {
                let file_cid = format!("{}.bin", value_cid);
                if name == file_cid {
                    //println!("Encryption case: CID: {} been found", value);
                    return Ok(true);
                }
            }
        }
    }

    // CID not found
    return Ok(false);
    
    } else {
        // while in decryption case
        
        if let Some(value) = input_doc_path {
            if !fs::exists(value)? {
                println!("Document does not exist at this path.");
                return Ok(false);
            }

            let mut file_encrypted = File::open(value)
                .map_err(|e| anyhow!("Failed to open file_encrypted {}", e))?;

            let mut buffer_check = vec![0u8; BUFFER_SIZE + 16];
            let mut hasher = Hasher::new();

            while let Ok(n) = file_encrypted.read(&mut buffer_check) {
                if n == 0 {
                    break;
                }
                hasher.update(&buffer_check[..n]);
            }

            if let Some(expected_cid) = cid {
                let tmp_cid = hasher.finalize().to_hex().to_string();
                if tmp_cid != expected_cid {
                    println!("Decryption case: File content hash does not match CID");
                    return Ok(false);
                } else {
                    return Ok(true);
                }
            }
        }
        println!("Decryption case: No input doc path provided");
        Ok(false)
    }

}

/// Encrypt file
pub async fn encrypt_file(
    key: &[u8; 32],
    nonce: &[u8; 19],
    input_path: &str
) -> Result<String, Error> {
    
    let mut hasher = Hasher::new();

    let tmp_output_path = format!("{}encdoc.tmp", get_storage_dir());
    
    let mut tmp_output = BufWriter::new(File::create(&tmp_output_path)?);

    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    
    let mut stream_encryptor = EncryptorBE32::from_aead(cipher, nonce.as_ref().into());

    let mut input = BufReader::new(File::open(input_path)?);

    let mut buffer = vec![0u8; BUFFER_SIZE];

    loop {
        let read_bytes = input.read(&mut buffer)?;
        
        if read_bytes == BUFFER_SIZE {

            let ciphertext = stream_encryptor
                .encrypt_next(buffer.as_slice())
                .map_err(|e|anyhow!("Failed to encrypt ciphertext chunk {}", e))?;
            tmp_output.write(&ciphertext)?;
            hasher.update(&ciphertext);

        } else {

            let ciphertext = stream_encryptor
                .encrypt_last(&buffer[..read_bytes])
                .map_err(|e| anyhow!("Failed to encrypt last chunk {}", e))?;
            tmp_output.write(&ciphertext)?;
            hasher.update(&ciphertext);
            break;
        }
    
        tmp_output.flush()?;
    }

    // finalize the hash and get the CID
    let cid = hasher.finalize().to_hex().to_string();

    // CHECK existence of CID in data storage
    let is_cid_exist = check_cid(true, Some(&cid), Some(&get_storage_dir())).await?;

    if !is_cid_exist {
        
        let final_path = format!("{}{}.bin", get_storage_dir(), cid);

        // Ensure the parent directory is created.
        if let Some(parent) = Path::new(&final_path).parent(){
            fs::create_dir_all(parent)?;
        } else {
            return Err(anyhow!("Failed to created path because of non existing parent"))
        }
        // Move/rename the .tmp file to the final path
        fs::rename(&tmp_output_path, &final_path)?;

        println!("Encryption done!");
        Ok(cid)

    } else {
        fs::remove_file(&tmp_output_path)?;
        return Err(anyhow!("Document already exist, upload and encryption aborded."))
    }
}


pub async fn decrypt_file(
    key: &[u8; 32],
    nonce: &[u8; 19],
    cid: &str,
    output_path: &str
) -> Result<(), Error> {
   
    let input_doc_path = format!("{}{}.bin", get_storage_dir(), cid);

    // CHECK existence of CID in data storage
    let is_cid_exist = check_cid(false, Some(&cid), Some(&input_doc_path)).await?;

    if is_cid_exist {

        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    
        let mut stream_decryptor = DecryptorBE32::from_aead(cipher, nonce.as_ref().into());

        let mut input = BufReader::new(File::open(&input_doc_path)?);
        
        let mut output = BufWriter::new(File::create(output_path)?);

        let mut buffer = vec![0u8; BUFFER_SIZE + 16]; // +16 for tag overhead

        loop {
            let read_bytes = input.read(&mut buffer)?;

            if read_bytes == BUFFER_SIZE + 16 {

                let cleartext = stream_decryptor
                    .decrypt_next(buffer.as_slice())
                    .map_err(|e|anyhow!("Failed to decrypt cleartext chunk {}", e))?;
                output.write(&cleartext)?;

            } else if read_bytes == 0 {
                break

            } else {

                let cleartext = stream_decryptor
                    .decrypt_last(&buffer[..read_bytes])
                    .map_err(|e| anyhow!("Failed to decrypt last chunk {}", e))?;
                output.write(&cleartext)?;
                break;
            }
            output.flush()?;
        }

    println!("Decryption done!");
    }

    Ok(())
}

pub async fn remove_stored_file(cid: &str) -> Result<(), Error> {
   
    let input_doc_path = format!("{}{}.bin", get_storage_dir(), cid);
     if !Path::new(&input_doc_path).exists() {
        return Err(anyhow!("No CID found in persistent storage, cannot remove CID"))
    }

    // Remove the file
    fs::remove_file(&input_doc_path)?;

    println!("Successfully removed CID file: {}", input_doc_path);
    Ok(())


}
// #[tokio::main]
// async fn main() -> Result<(), Error>{

//     let (new_key, new_nonce) = newkey().await?;
//     let mut rng = rand::thread_rng();
    
//     //let (new_key2, _) = newkey().await?;
//     let mission = format!("{}",rng.next_u32());

//     let cid = encrypt_file(&new_key, &new_nonce, "../ghost-ground/data/mystikp2p.png", &mission).await?;
//     // encrypt_file(&new_key, &new_nonce, "../ghost-ground/data/mystikp2p.png", &mission).await?;
    
//     decrypt_file(&new_key, &new_nonce, &cid, &mission, "../content-hashing/data_dec/mystikp2p.png").await?;
//     // decrypt_file(&new_key, &new_nonce, "d83145dc624fe9f277b8bdafbe0fcf5f1ebaa1f216040112ed84e14b38e8dfa2", &mission, "../content-hashing/data_dec/mystikp2p.png").await?;    

//     Ok(())
// }
