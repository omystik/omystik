use anyhow::{anyhow, Result};
use mesh_gateway_wire::{ArtifactKind, JobStore};

pub async fn load_artifact_bytes(
    store: &JobStore,
    artifact_id: &[u8; 32],
    kind: ArtifactKind,
) -> Result<Vec<u8>> {
    let blob_id = store
        .get_artifact(artifact_id, kind.clone())
        .await?
        .ok_or_else(|| {
            anyhow!(
                "artifact not found: kind={:?}, id={}",
                kind,
                hex::encode(artifact_id)
            )
        })?;

    store.read_blob(&blob_id).await.map_err(Into::into)
}