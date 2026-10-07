use anyhow::{anyhow, Result};
use futures::{
    io::{AsyncReadExt, AsyncWriteExt},
    channel::mpsc,
    SinkExt,
};
use libp2p::Stream;
use mesh_gateway_wire::{decode, encode, JobStore, FaceABlobHeader, blake3_blob_id}; // SignedArtifact, verify_signed_artifact

pub type PinAck = ([u8; 32], [u8; 32]); // (key_id, blob_id)

pub async fn handle_inbound_blob_stream(
    store: JobStore,
    stream: Stream,
    mut pin_ack_tx: mpsc::Sender<PinAck>,
) -> Result<()> {
    let (mut r, mut w) = stream.split();

    let hdr_bytes = read_frame(&mut r).await?;
    let hdr: FaceABlobHeader = decode(&hdr_bytes)?;

    match hdr {
        // - 6 March -
        FaceABlobHeader::Put { blob_id, size } => {
            if store.blob_exists(&blob_id).await {
                let ack = FaceABlobHeader::Ack { ok: true, message: Some("dedup".into()) };
                write_frame(&mut w, &encode(&ack)?).await?;
                w.close().await?;
                return Ok(());
            }

            let size_usize = usize::try_from(size)
                .map_err(|_| anyhow!("size too large"))?;
            let mut buf = vec![0u8; size_usize];
            r.read_exact(&mut buf).await?;

            let expected = blake3_blob_id(&buf);
            if expected != blob_id {
                let ack = FaceABlobHeader::Ack {
                    ok: false,
                    message: Some("blob_id != blake3(bytes)".into()),
                };
                write_frame(&mut w, &encode(&ack)?).await?;
                w.close().await?;
                return Ok(());
            }

            store.write_blob(&blob_id, &buf).await?;

            let ack = FaceABlobHeader::Ack { ok: true, message: None };
            write_frame(&mut w, &encode(&ack)?).await?;
            w.close().await?;

            Ok(())
        }
        // - -


        FaceABlobHeader::Get { blob_id } => {
            let bytes = match store.read_blob(&blob_id).await {
                Ok(b) => b,
                Err(e) => {
                    let ack = FaceABlobHeader::Ack { ok: false, message: Some(format!("missing blob: {e}")) };
                    write_frame(&mut w, &encode(&ack)?).await?;
                    w.close().await?;
                    return Ok(());
                }
            };

            let ack = FaceABlobHeader::Ack { ok: true, message: None };
            write_frame(&mut w, &encode(&ack)?).await?;
            w.write_all(&bytes).await?;
            w.close().await?;
            Ok(())
        }

        FaceABlobHeader::Ack { .. } => Err(anyhow!("client sent Ack as request")),
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

