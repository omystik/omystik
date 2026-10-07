use anyhow::{anyhow, Result};
use crate::{SignedManifest, ArtifactKind, BlobId};
use blake3;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Verify a SignedManifest:
/// - at least `min_signatures` valid signatures
/// - signatures are over canonical encoded manifest
pub fn verify_signed_manifest(
    signed: &SignedManifest,
    min_signatures: usize,
) -> Result<()> {
    if signed.signatures.len() < min_signatures {
        return Err(anyhow!(
            "not enough signatures: got {}, need {}",
            signed.signatures.len(),
            min_signatures
        ));
    }

    let manifest_bytes = crate::encode(&signed.manifest)?;

    let mut valid = 0;

    for sig in &signed.signatures {
        let pk = VerifyingKey::from_bytes(&sig.signer_key)
            .map_err(|e| anyhow!("invalid public key: {e}"))?;

        let signature = Signature::from_slice(&sig.sig)
            .map_err(|e| anyhow!("invalid signature bytes: {e}"))?;

        if pk.verify(&manifest_bytes, &signature).is_ok() {
            valid += 1;
        }
    }

    if valid < min_signatures {
        return Err(anyhow!(
            "insufficient valid signatures: got {}, need {}",
            valid,
            min_signatures
        ));
    }

    Ok(())
}

pub fn verify_manifest_blobs(
    entries: &Vec<crate::ArtifactEntry>,
    get_blob: impl Fn(&BlobId) -> Result<Vec<u8>>,
) -> Result<()> {
    for entry in entries {
        let bytes = get_blob(&entry.cid)?;

        let computed = blake3::hash(&bytes).into();

        if computed != entry.cid {
            return Err(anyhow!(
                "blob integrity mismatch for {:?}",
                entry.kind
            ));
        }

        if bytes.len() as u64 != entry.byte_len {
            return Err(anyhow!(
                "byte_len mismatch for {:?}",
                entry.kind
            ));
        }
    }

    Ok(())
}

pub fn require_artifacts(
    entries: &Vec<crate::ArtifactEntry>,
    required: &[ArtifactKind],
) -> Result<()> {
    for r in required {
        if !entries.iter().any(|e| &e.kind == r) {
            return Err(anyhow!("missing required artifact: {:?}", r));
        }
    }
    Ok(())
}