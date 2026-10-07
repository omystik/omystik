use anyhow::{anyhow, Result};
use compute_abi::{ArtifactBinding, ComputeJobSpec, InputPayload, ProgramId, ProgramVersion};
use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use mesh_gateway_wire::{
    ArtifactKind, BlobId, FaceARequest, FaceAResponse, JobId, JobMetadata, JobType, Payload,
};

use crate::upload::{put_blob, register_artifact};

#[derive(Debug, Clone)]
pub struct ArtifactUpload {
    pub artifact_id: [u8; 32],
    pub artifact_kind: ArtifactKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SubmittedComputeJob {
    pub job_id: JobId,
    pub artifact_blob_ids: Vec<BlobId>,
}

pub fn build_compute_job_spec(
    program_id: ProgramId,
    program_version: ProgramVersion,
    inputs: Vec<InputPayload>,
    artifacts: Vec<(ArtifactUpload, Option<BlobId>)>,
    expected_outputs: u32,
    metadata: Vec<(String, String)>,
) -> ComputeJobSpec {
    let artifact_bindings = artifacts
        .into_iter()
        .map(|(artifact, blob_id_hint)| ArtifactBinding {
            artifact_id: artifact.artifact_id,
            artifact_kind: artifact_kind_string(&artifact.artifact_kind),
            blob_id_hint,
        })
        .collect();

    ComputeJobSpec {
        program_id,
        program_version,
        inputs,
        artifacts: artifact_bindings,
        expected_outputs,
        metadata,
    }
}

pub async fn submit_compute_job(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    job_id: JobId,
    spec: ComputeJobSpec,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(mesh_gateway_wire::encode(&spec)?);

    let resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::SubmitJob {
                job_id,
                job_type: JobType::FheComputing,
                payload,
                metadata,
            },
        )
        .await?;

    match resp {
        FaceAResponse::AckJob { accepted: true, .. } => Ok(()),
        FaceAResponse::AckJob {
            accepted: false,
            reason,
            ..
        } => Err(anyhow!(
            "SubmitJob rejected: {}",
            reason.unwrap_or_else(|| "unknown".into())
        )),
        FaceAResponse::Error { message } => Err(anyhow!("SubmitJob error: {message}")),
        other => Err(anyhow!("unexpected SubmitJob response: {other:?}")),
    }
}

/// Generic helper: upload runtime inputs as blobs.
pub async fn upload_input_blobs(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    input_bytes: Vec<Vec<u8>>,
) -> Result<(Vec<BlobId>, Vec<InputPayload>)> {
    let mut blob_ids = Vec::with_capacity(input_bytes.len());
    let mut payloads = Vec::with_capacity(input_bytes.len());

    for bytes in input_bytes {
        let blob_id = put_blob(net_client, ypolo_peer, bytes).await?;
        blob_ids.push(blob_id);
        payloads.push(InputPayload::BlobRef(blob_id));
    }

    Ok((blob_ids, payloads))
}

/// Generic helper: upload and register artifacts.
pub async fn upload_and_register_artifacts(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    artifacts: Vec<ArtifactUpload>,
) -> Result<(Vec<(ArtifactUpload, Option<BlobId>)>, Vec<BlobId>)> {
    let mut bindings = Vec::with_capacity(artifacts.len());
    let mut blob_ids = Vec::with_capacity(artifacts.len());

    for artifact in artifacts {
        let blob_id = put_blob(net_client, ypolo_peer, artifact.bytes.clone()).await?;

        register_artifact(
            net_client,
            ypolo_peer,
            artifact.artifact_id,
            artifact.artifact_kind.clone(),
            blob_id,
        )
        .await?;

        blob_ids.push(blob_id);
        bindings.push((artifact, Some(blob_id)));
    }

    Ok((bindings, blob_ids))
}

fn artifact_kind_string(kind: &ArtifactKind) -> String {
    match kind {
        ArtifactKind::VerfKey => "VerfKey",
        ArtifactKind::VerfAddress => "VerfAddress",
        ArtifactKind::PublicKey => "PublicKey",
        ArtifactKind::PublicKeyMetadata => "PublicKeyMetadata",
        ArtifactKind::ServerKey => "ServerKey",
        ArtifactKind::Crs => "Crs",
        ArtifactKind::ProgramManifest => "ProgramManifest",
        ArtifactKind::ComputeReceipt => "ComputeReceipt",
        ArtifactKind::EncryptedInput => "EncryptedInput",
        ArtifactKind::EncryptedOutput => "EncryptedOutput",
        ArtifactKind::PublicParams => "PublicParams",
    }
    .to_string()
}

pub fn inline_inputs(input_bytes: Vec<Vec<u8>>) -> Vec<InputPayload> {
    input_bytes.into_iter().map(InputPayload::Inline).collect()
}

pub async fn update_compute_job_input(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    job_id: JobId,
    spec: ComputeJobSpec,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(mesh_gateway_wire::encode(&spec)?);

    let resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::UpdateJobInput {
                job_id,
                payload,
                metadata,
            },
        )
        .await?;

    match resp {
        FaceAResponse::UpdateJobInputAck {
            accepted: true, ..
        } => Ok(()),

        FaceAResponse::UpdateJobInputAck {
            accepted: false,
            reason,
            ..
        } => Err(anyhow!(
            "UpdateJobInput rejected: {}",
            reason.unwrap_or_else(|| "unknown".into())
        )),

        FaceAResponse::Error { message } => {
            Err(anyhow!("UpdateJobInput error: {message}"))
        }

        other => Err(anyhow!("unexpected UpdateJobInput response: {other:?}")),
    }
}

pub async fn push_program_session_input(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
    input_kind: impl Into<String>,
    payload_bytes: Vec<u8>,
    observed_unix_ms: u64,
    sequence: u64,
) -> Result<()> {
    let input_kind = input_kind.into();

    let resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::PushProgramSessionInput {
                session_id,
                input_kind: input_kind.clone(),
                payload: Payload::Inline(payload_bytes),
                observed_unix_ms,
                sequence,
            },
        )
        .await?;

    match resp {
        FaceAResponse::PushProgramSessionAck {
            session_id: ack_session_id,
            input_kind: ack_input_kind,
            sequence: ack_sequence,
            ..
        } if ack_session_id == session_id
            && ack_input_kind == input_kind
            && ack_sequence == sequence =>
        {
            Ok(())
        }

        FaceAResponse::PushProgramSessionAck {
            session_id: ack_session_id,
            input_kind: ack_input_kind,
            sequence: ack_sequence,
            ..
        } => Err(anyhow!(
            "PushProgramSessionInput ack mismatch: expected session={} kind={} seq={}, got session={} kind={} seq={}",
            hex::encode(session_id),
            input_kind,
            sequence,
            hex::encode(ack_session_id),
            ack_input_kind,
            ack_sequence
        )),

        FaceAResponse::Error { message } => {
            Err(anyhow!("PushProgramSessionInput error: {message}"))
        }

        other => Err(anyhow!(
            "unexpected PushProgramSessionInput response: {other:?}"
        )),
    }
}