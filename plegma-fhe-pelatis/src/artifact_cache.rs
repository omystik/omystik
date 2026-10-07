use anyhow::{anyhow, Result};
use mesh_gateway_wire::ArtifactKind;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Local cache for public FHE artifacts on Mesh A nodes.
///
/// Threshold layout:
///   <root>/PUB-p<party_id>/<ArtifactKind as dir>/<hex(id)>
///
/// Examples:
///   keys/PUB-p1/PublicKey/<key_id_hex>
///   keys/PUB-p2/ServerKey/<key_id_hex>
///   keys/PUB-p3/CRS/<crs_id_hex>
#[derive(Clone, Debug)]
pub struct ArtifactCache {
    root: PathBuf,
}

impl ArtifactCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn party_root(&self, party_id: usize) -> PathBuf {
        self.root.join(format!("PUB-p{}", party_id))
    }

    pub fn path_for_party(
        &self,
        party_id: usize,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> PathBuf {
        self.party_root(party_id)
            .join(kind.as_storage_dir())
            .join(hex::encode(id))
    }

    pub async fn has_for_party(
        &self,
        party_id: usize,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> bool {
        fs::metadata(self.path_for_party(party_id, id, kind)).await.is_ok()
    }

    pub async fn put_for_party(
        &self,
        party_id: usize,
        id: &[u8; 32],
        kind: ArtifactKind,
        bytes: &[u8],
    ) -> Result<()> {
        let path = self.path_for_party(party_id, id, kind);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&path, bytes).await?;
        Ok(())
    }

    pub async fn get_for_party(
        &self,
        party_id: usize,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> Result<Vec<u8>> {
        let path = self.path_for_party(party_id, id, kind);
        fs::read(&path)
            .await
            .map_err(|e| anyhow!("failed to read artifact at {:?}: {e}", path))
    }

    pub async fn remove_for_party(
        &self,
        party_id: usize,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> Result<()> {
        let path = self.path_for_party(party_id, id, kind);
        match fs::remove_file(&path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!("failed to remove artifact at {:?}: {e}", path)),
        }
    }

    pub async fn has_in_any_party(
        &self,
        party_ids: &[usize],
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> bool {
        for &party_id in party_ids {
            if self.has_for_party(party_id, id, kind.clone()).await {
                return true;
            }
        }
        false
    }

    pub async fn put_for_parties(
        &self,
        party_ids: &[usize],
        id: &[u8; 32],
        kind: ArtifactKind,
        bytes: &[u8],
    ) -> Result<()> {
        for &party_id in party_ids {
            self.put_for_party(party_id, id, kind.clone(), bytes).await?;
        }
        Ok(())
    }

    pub async fn get_from_any_party(
        &self,
        party_ids: &[usize],
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> Result<Vec<u8>> {
        for &party_id in party_ids {
            let path = self.path_for_party(party_id, id, kind.clone());
            if let Ok(bytes) = fs::read(&path).await {
                return Ok(bytes);
            }
        }

        Err(anyhow!(
            "artifact not found in any configured party directory: kind={:?}, id={}",
            kind,
            hex::encode(id)
        ))
    }
}