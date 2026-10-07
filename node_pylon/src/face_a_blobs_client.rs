use anyhow::{anyhow, Result};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use libp2p::{PeerId, StreamProtocol};
use mesh_gateway_wire::{encode, decode, FaceABlobHeader, BlobId};

pub async fn put_blob(
    mut control: libp2p_stream::Control,
    peer: PeerId,
    proto: StreamProtocol,
    blob_id: BlobId,
    bytes: &[u8],
) -> Result<()> {
    let stream = control
        .open_stream(peer, proto)
        .await
        .map_err(|e| anyhow!("open_stream failed: {e}"))?;

    let (mut r, mut w) = stream.split();

    // send header
    let hdr = FaceABlobHeader::Put { blob_id, size: bytes.len() as u64 };
    write_frame(&mut w, &encode(&hdr)?).await?;
    w.write_all(bytes).await?;
    w.flush().await?;
    w.close().await?;

    // read ack
    let ack_bytes = read_frame(&mut r).await?;
    let ack: FaceABlobHeader = decode(&ack_bytes)?;
    match ack {
        FaceABlobHeader::Ack { ok: true, .. } => Ok(()),
        FaceABlobHeader::Ack { ok: false, message } => Err(anyhow!("Put rejected: {:?}", message)),
        _ => Err(anyhow!("unexpected blob response")),
    }
}

async fn write_frame<W: AsyncWriteExt + Unpin>(w: &mut W, bytes: &[u8]) -> Result<()> {
    let len = bytes.len() as u32;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(bytes).await?;
    Ok(())
}
async fn read_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<Vec<u8>> {
    let mut lenb = [0u8; 4];
    r.read_exact(&mut lenb).await?;
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}
