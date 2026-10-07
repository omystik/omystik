use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use kms_core_client::{encrypt, CipherParameters, EncryptionResult};

use mesh_gateway_wire::ArtifactKind;

use crate::pylon_client::MeshGatewayClient;
use crate::types::{FheType, KeyId};

/// The minimum artifact set required for local encryption on a Mesh A node.
///
/// Today this mirrors what `kms_core_client::encrypt(...)` expects to find in local
/// storage for a given `key_id`.
///
/// In practice:
/// - `PublicKey` is required for encryption
/// - `PublicKeyMetadata` is usually needed by the storage/material loading path
/// - `ServerKey` is also fetched so the local cache is complete for subsequent
///   operations and aligned with the current KMS storage conventions
pub fn required_keygen_artifacts() -> [ArtifactKind; 3] {
    [
        ArtifactKind::PublicKey,
        ArtifactKind::PublicKeyMetadata,
        ArtifactKind::ServerKey,
    ]
}

/// Ensure the key material needed for local encryption is present in the local cache.
///
/// `key_id` is the KMS key generation request ID (same semantics as existing KMS).
pub async fn materialize_key_for_encryption(
    gateway: &mut MeshGatewayClient,
    key_id: [u8; 32],
) -> Result<()> {
    for kind in required_keygen_artifacts() {
        let _ = gateway.fetch_artifact(key_id, kind).await?;
    }
    Ok(())
}

/// Ensure global verification material is present locally.
///
/// This is useful for Mesh A nodes that also want to verify KMS-originated results
/// or prepare for later decrypt/result-processing flows.
///
/// `signing_key_id` is typically the well-known signing key ID used by the KMS
/// (`SIGNING_KEY_ID` in the KMS stack), represented as raw `[u8; 32]`.
pub async fn materialize_verification_material(
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

/// Ensure CRS is present locally.
///
/// `crs_id` is the request ID of the CRS generation operation.
pub async fn materialize_crs(
    gateway: &mut MeshGatewayClient,
    crs_id: [u8; 32],
) -> Result<()> {
    let _ = gateway.fetch_artifact(crs_id, ArtifactKind::Crs).await?;
    Ok(())
}

/// Encrypt locally using cached/public material fetched through Pylon.
///
/// This function:
/// 1. fetches/materializes the required public key artifacts for `key_id`,
/// 2. calls the existing `kms_core_client::encrypt(...)`,
/// 3. returns the standard `EncryptionResult`.
///
/// `party_id`:
/// - keeps the same meaning as in `kms_core_client::encrypt(...)`
/// - in practice, once artifacts are materialized from Pylon, the existing storage
///   layout still expects a party namespace; use the party that published/fetched
///   the artifacts into the local cache layout that your Mesh node follows.
///
/// If your Mesh-side cache later becomes party-agnostic, this can be simplified.
pub async fn encrypt_with_gateway(
    gateway: &mut MeshGatewayClient,
    cache_root: &Path,
    party_id: usize,
    params: CipherParameters,
) -> Result<EncryptionResult, Box<dyn std::error::Error + 'static>> {
    materialize_key_for_encryption(gateway, params.key_id.into_bytes()).await?;
    encrypt(cache_root, party_id, params).await
}

/// Convenience helper to build `CipherParameters` for Mesh-side producers.
///
/// This is useful for data-producing Mesh A nodes that do not want to construct
/// the CLI-flavored config object manually every time.
pub fn make_cipher_parameters(
    key_id: KeyId,
    fhe_type: FheType,
    to_encrypt_hex: impl Into<String>,
) -> CipherParameters {
    CipherParameters {
        to_encrypt: to_encrypt_hex.into(),
        data_type: fhe_type,
        no_compression: false,
        no_precompute_sns: false,
        key_id,
        batch_size: 1,
        num_requests: 1,
        ciphertext_output_path: None,
    }
}

/// Helper for Mesh nodes that store cache under a conventional local path like `keys/`.
pub fn default_cache_root() -> PathBuf {
    PathBuf::from("keys")
}

/// Validate that a given cache root looks usable for local KMS-style materialization.
///
/// This is intentionally light-touch. It does not enforce the full directory tree;
/// it only checks the root exists or can be created by the caller later.
pub fn validate_cache_root(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("cache root path must not be empty"));
    }
    Ok(())
}