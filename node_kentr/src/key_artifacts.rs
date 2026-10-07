use anyhow::{Context, Result};
pub use mesh_gateway_wire::ArtifactKind;
use std::{fs, path::Path};

pub fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s)
        .with_context(|| format!("invalid hex32: {s}"))?;

    if bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "expected 32 bytes, got {} for {s}",
            bytes.len()
        ));
    }

    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn hex32(id: [u8; 32]) -> String {
    hex::encode(id)
}

pub fn artifact_kind_dir(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::PublicKey => "PublicKey",
        ArtifactKind::PublicKeyMetadata => "PublicKeyMetadata",
        ArtifactKind::ServerKey => "ServerKey",
        ArtifactKind::Crs => "CRS",
        ArtifactKind::VerfKey => "VerfKey",
        ArtifactKind::VerfAddress => "VerfAddress",
        ArtifactKind::ProgramManifest => "ProgramManifest",
        ArtifactKind::ComputeReceipt => "ComputeReceipt",
        ArtifactKind::EncryptedInput => "EncryptedInput",
        ArtifactKind::EncryptedOutput => "EncryptedOutput",
        ArtifactKind::PublicParams => "PublicParams",
    }
}

pub fn write_keyset_artifact_to_all_pub_parties(
    keys_folder: &Path,
    party_count: usize,
    key_id: [u8; 32],
    kind: ArtifactKind,
    bytes: &[u8],
) -> Result<()> {
    let key_hex = hex32(key_id);
    let kind_dir = artifact_kind_dir(&kind);

    for party in 1..=party_count {
        let dir = keys_folder
            .join(format!("PUB-p{party}"))
            .join(kind_dir);

        fs::create_dir_all(&dir)
            .with_context(|| format!("create {}", dir.display()))?;

        let path = dir.join(&key_hex);

        fs::write(&path, bytes)
            .with_context(|| format!("write {}", path.display()))?;
    }

    Ok(())
}