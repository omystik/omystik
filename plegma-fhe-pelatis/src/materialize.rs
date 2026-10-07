use anyhow::Result;
use mesh_gateway_wire::ArtifactKind;

use crate::pylon_client::MeshGatewayClient;

/// Materialize the minimal verification bootstrap material.
///
/// This is typically the first thing a Mesh A node should fetch before it starts
/// trusting KMS-originated public artifacts or decrypt results.
pub async fn materialize_verification_bundle(
    gateway: &mut MeshGatewayClient,
    signing_key_id: [u8; 32],
) -> Result<()> {
    let _ = gateway
        .fetch_artifact(signing_key_id, ArtifactKind::VerfKey)
        .await?;
    let _ = gateway
        .fetch_artifact(signing_key_id, ArtifactKind::VerfAddress)
        .await?;
    Ok(())
}

/// Materialize the full key bundle needed by Mesh A nodes for local FHE usage.
///
/// This corresponds to what the old client expected to retrieve from public storage
/// for a generated key:
/// - PublicKey
/// - PublicKeyMetadata
/// - ServerKey
pub async fn materialize_key_bundle(
    gateway: &mut MeshGatewayClient,
    key_id: [u8; 32],
) -> Result<()> {
    let _ = gateway
        .fetch_artifact(key_id, ArtifactKind::PublicKey)
        .await?;
    let _ = gateway
        .fetch_artifact(key_id, ArtifactKind::PublicKeyMetadata)
        .await?;
    let _ = gateway
        .fetch_artifact(key_id, ArtifactKind::ServerKey)
        .await?;
    Ok(())
}

/// Materialize a CRS bundle.
///
/// Right now CRS is a single artifact.
/// Keeping this helper makes the Mesh-side API symmetric with key bundles.
pub async fn materialize_crs_bundle(
    gateway: &mut MeshGatewayClient,
    crs_id: [u8; 32],
) -> Result<()> {
    let _ = gateway.fetch_artifact(crs_id, ArtifactKind::Crs).await?;
    Ok(())
}

/// Materialize everything referenced by a manifest.
///
/// This is useful once Pylon publishes a manifest for a key or CRS request and
/// Mesh A nodes want to cache the entire bundle without knowing the exact set of
/// artifact kinds ahead of time.
pub async fn materialize_from_manifest(
    gateway: &mut MeshGatewayClient,
    id: [u8; 32],
) -> Result<()> {
    let manifest = gateway.fetch_manifest(id).await?;
    for entry in manifest.entries {
        let _ = gateway.fetch_artifact(id, entry.kind).await?;
    }
    Ok(())
}

/// Materialize a standard bundle for a key-producing flow:
/// - global verification material
/// - key bundle
///
/// This is the common path for nodes that need to start encrypting under a new key.
pub async fn materialize_standard_key_context(
    gateway: &mut MeshGatewayClient,
    signing_key_id: [u8; 32],
    key_id: [u8; 32],
) -> Result<()> {
    materialize_verification_bundle(gateway, signing_key_id).await?;
    materialize_key_bundle(gateway, key_id).await?;
    Ok(())
}