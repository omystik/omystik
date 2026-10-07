use anyhow::{anyhow, Result};
use compute_abi::{ProgramManifest, ProgramRuntimeKind};
use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use mesh_gateway_wire::{BlobId, FaceARequest, FaceAResponse};

use crate::upload::put_blob;

#[derive(Debug, Clone)]
pub struct DeployedProgram {
    pub program_id: compute_abi::ProgramId,
    pub program_version: compute_abi::ProgramVersion,
    pub package_blob_id: BlobId,
    pub manifest_blob_id: BlobId,
    pub runtime_kind: ProgramRuntimeKind,
}

/// Generic smart-program deployment helper.
///
/// The caller owns the manifest and package bytes.
/// This crate stays domain-agnostic.
pub async fn deploy_program(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    manifest: ProgramManifest,
    package_bytes: Vec<u8>,
    runtime_kind: ProgramRuntimeKind,
    metadata: Vec<(String, String)>,
) -> Result<DeployedProgram> {
    let manifest_bytes = mesh_gateway_wire::encode(&manifest)?;
    let manifest_blob_id = put_blob(net_client, ypolo_peer, manifest_bytes).await?;
    let package_blob_id = put_blob(net_client, ypolo_peer, package_bytes).await?;

    let resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::DeployProgram {
                program_id: manifest.program_id.clone(),
                program_version: manifest.program_version.clone(),
                package_blob_id,
                manifest_blob_id,
                runtime_kind: runtime_kind.clone(),
                metadata,
            },
        )
        .await?;

    match resp {
        FaceAResponse::DeployProgramAck { accepted: true, .. } => Ok(DeployedProgram {
            program_id: manifest.program_id,
            program_version: manifest.program_version,
            package_blob_id,
            manifest_blob_id,
            runtime_kind,
        }),
        FaceAResponse::DeployProgramAck {
            accepted: false,
            reason,
        } => Err(anyhow!(
            "DeployProgram rejected: {}",
            reason.unwrap_or_else(|| "unknown".into())
        )),
        FaceAResponse::Error { message } => Err(anyhow!("DeployProgram error: {message}")),
        other => Err(anyhow!("unexpected DeployProgram response: {other:?}")),
    }
}