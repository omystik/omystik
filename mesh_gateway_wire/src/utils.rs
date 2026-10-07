
use anyhow::Result;
use anyhow::{anyhow, bail};
use crate::SignedArtifact;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};


pub fn blake3_blob_id(bytes: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}


pub fn verify_signed_artifact(a: &SignedArtifact) -> Result<()> {
    
    if a.signer.len() != 32 {
        bail!("SignedArtifact.signer must be 32 bytes (ed25519 pubkey)");
    }
    if a.sig.len() != 64 {
        bail!("SignedArtifact.sig must be 64 bytes (ed25519 signature)");
    }

    let pk_bytes: [u8; 32] = a.signer.as_slice().try_into().map_err(|_| anyhow!("bad signer len"))?;
    let sig_bytes: [u8; 64] = a.sig.as_slice().try_into().map_err(|_| anyhow!("bad sig len"))?;

    let pk = VerifyingKey::from_bytes(&pk_bytes)
        .map_err(|e| anyhow!("invalid signer pubkey: {e}"))?;

    // message = key_id || blob_id
    let mut msg = Vec::with_capacity(64);
    msg.extend_from_slice(&a.key_id);
    msg.extend_from_slice(&a.blob_id);

    let sig = Signature::from_bytes(&sig_bytes);
    pk.verify(&msg, &sig)
        .map_err(|e| anyhow!("signature verify failed: {e}"))?;

    Ok(())
}