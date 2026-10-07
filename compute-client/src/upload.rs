use anyhow::{anyhow, Result};
use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use mesh_gateway_wire::{blake3_blob_id, ArtifactKind, BlobId, FaceARequest, FaceAResponse};

pub async fn put_blob(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    bytes: Vec<u8>,
) -> Result<BlobId> {
    let blob_id = blake3_blob_id(&bytes);
    net_client.put_blob(ypolo_peer, blob_id, bytes).await?;
    Ok(blob_id)
}

pub async fn register_artifact(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    id: [u8; 32],
    kind: ArtifactKind,
    blob_id: BlobId,
) -> Result<()> {
    let resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::RegisterArtifact { id, kind, blob_id },
        )
        .await?;

    match resp {
        FaceAResponse::RegisterArtifactAck { ok: true, .. } => Ok(()),
        FaceAResponse::RegisterArtifactAck { ok: false, message } => Err(anyhow!(
            "RegisterArtifact rejected: {}",
            message.unwrap_or_else(|| "unknown".into())
        )),
        FaceAResponse::Error { message } => Err(anyhow!("RegisterArtifact error: {message}")),
        other => Err(anyhow!("unexpected RegisterArtifact response: {other:?}")),
    }
}