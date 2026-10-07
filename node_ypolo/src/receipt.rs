use anyhow::Result;
use compute_abi::{BlobId, ComputeJobSpec, ComputeReceipt, Hash32};
use mesh_gateway_wire::JobId;

fn hash_bytes(bytes: &[u8]) -> Hash32 {
    *blake3::hash(bytes).as_bytes()
}

pub fn build_receipt(
    job_id: JobId,
    spec: &ComputeJobSpec,
    program_manifest_blob_id: BlobId,
    input_bytes: &[Vec<u8>],
    output_blob_ids: Vec<BlobId>,
    artifact_blob_ids: Vec<BlobId>,
    started_unix_ms: u64,
    finished_unix_ms: u64,
) -> ComputeReceipt {
    ComputeReceipt {
        job_id,
        program_id: spec.program_id.clone(),
        program_version: spec.program_version.clone(),
        program_manifest_blob_id,
        input_hashes: input_bytes.iter().map(|b| hash_bytes(b)).collect(),
        output_blob_ids,
        artifact_blob_ids,
        worker_version: env!("CARGO_PKG_VERSION").to_string(),
        started_unix_ms,
        finished_unix_ms,
        metadata: spec.metadata.clone(),
    }
}

pub fn encode_receipt(receipt: &ComputeReceipt) -> Result<Vec<u8>> {
    Ok(mesh_gateway_wire::encode(receipt)?)
}