use anyhow::{anyhow, Result};
use compute_abi::{
    BlobId, ComputeJobSpec, ProgramId, ProgramManifest, ProgramRuntimeKind, ProgramVersion,
};
use mesh_gateway_wire::JobStore;

pub async fn load_program_manifest_by_blob_id(
    store: &JobStore,
    manifest_blob_id: &BlobId,
) -> Result<ProgramManifest> {
    let bytes = store.read_blob(manifest_blob_id).await?;
    let manifest: ProgramManifest = mesh_gateway_wire::decode(&bytes)
        .map_err(|e| anyhow!("decode ProgramManifest failed: {e}"))?;
    Ok(manifest)
}

pub fn validate_program_manifest_for_install(
    manifest: &ProgramManifest,
    requested_program_id: &ProgramId,
    requested_program_version: &ProgramVersion,
    runtime_kind: &ProgramRuntimeKind,
) -> Result<()> {
    if &manifest.program_id != requested_program_id {
        return Err(anyhow!(
            "program_id mismatch: request={}, manifest={}",
            requested_program_id.0,
            manifest.program_id.0
        ));
    }

    if &manifest.program_version != requested_program_version {
        return Err(anyhow!(
            "program_version mismatch: request={}, manifest={}",
            requested_program_version.0,
            manifest.program_version.0
        ));
    }

    if manifest.abi_version != "compute-abi/v1" {
    return Err(anyhow!("unsupported abi_version: {}", manifest.abi_version));
    }

    match runtime_kind {
        ProgramRuntimeKind::NativeBuiltin => {
            if manifest.executor_backend != "tfhe" {
                return Err(anyhow!(
                    "unsupported executor_backend for NativeBuiltin on this node: {}",
                    manifest.executor_backend
                ));
            }
        }

        ProgramRuntimeKind::ExternalProcessV1 => {
            if manifest.executor_backend != "external-process-v1" {
                return Err(anyhow!(
                    "unsupported executor_backend for ExternalProcessV1 on this node: {}",
                    manifest.executor_backend
                ));
            }
        }

        ProgramRuntimeKind::WasmModule => {
            return Err(anyhow!("WasmModule is not enabled on this node"));
        }

        ProgramRuntimeKind::ComputeGraphV1 => {
            return Err(anyhow!(
                "ComputeGraphV1 is not enabled on this node; deploy as ExternalProcessV1 or enable a graph runtime"
            ));
        }
    }

    Ok(())
}

pub fn validate_program_invocation(
    spec: &ComputeJobSpec,
    manifest: &ProgramManifest,
) -> Result<()> {
    if manifest.program_id != spec.program_id {
        return Err(anyhow!(
            "program_id mismatch: spec={}, manifest={}",
            spec.program_id.0,
            manifest.program_id.0
        ));
    }

    if manifest.program_version != spec.program_version {
        return Err(anyhow!(
            "program_version mismatch: spec={}, manifest={}",
            spec.program_version.0,
            manifest.program_version.0
        ));
    }

    if manifest.abi_version != "compute-abi/v1" {
        return Err(anyhow!("unsupported abi_version: {}", manifest.abi_version));
    }

    if spec.expected_outputs != manifest.output_slots.len() as u32 {
        return Err(anyhow!(
            "expected_outputs mismatch: spec={}, manifest={}",
            spec.expected_outputs,
            manifest.output_slots.len()
        ));
    }

        let is_streaming_session = spec
        .metadata
        .iter()
        .any(|(k, v)| k == "session.mode" && v == "streaming");

    if !is_streaming_session && spec.inputs.len() != manifest.input_slots.len() {
        return Err(anyhow!(
            "input count mismatch: spec={}, manifest={}",
            spec.inputs.len(),
            manifest.input_slots.len()
        ));
    }

    if is_streaming_session && spec.inputs.len() > manifest.input_slots.len() {
        return Err(anyhow!(
            "streaming session has too many direct inputs: spec={}, manifest={}",
            spec.inputs.len(),
            manifest.input_slots.len()
        ));
    }

    for req in &manifest.required_artifacts {
        if !req.required {
            continue;
        }

        let found = spec.artifacts.iter().any(|a| a.artifact_kind == req.kind);

        if !found {
            return Err(anyhow!(
                "required artifact missing from ComputeJobSpec: {}",
                req.kind
            ));
        }
    }

    Ok(())
}