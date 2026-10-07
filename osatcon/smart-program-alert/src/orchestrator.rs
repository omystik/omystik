use anyhow::{self, Result};
use compute_abi::{InputPayload, ProgramId, ProgramVersion};
use compute_client::submit::{
    build_compute_job_spec, push_program_session_input, 
    submit_compute_job, SubmittedComputeJob, update_compute_job_input,
};

use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use mesh_gateway_wire::{
    FaceARequest, FaceAResponse, JobId, JobMetadata,
    JobResultPayload, JobState, Payload
};

use crate::computation::{
    AlertMathEncryptedConfigV1, AlertMathEncryptedFieldsV1, AlertMathInputV1,
    AlertMathOutputV1,
    build_input_blob_v1, build_encrypted_convoy_payload_v1, 
    build_encrypted_satellite_payload_v1, build_encrypted_config_payload_v1,
    decode_output_blob_v1,
    EncryptedConvoyPointV1, EncryptedSatelliteBatchV1,
};

use crate::graph::{
    SESSION_INPUT_KIND_CONFIG_V1 as SESSION_INPUT_CONFIG_V1,
    SESSION_INPUT_KIND_CONVOY_V1 as SESSION_INPUT_CONVOY_V1,
    SESSION_INPUT_KIND_SATELLITE_V1 as SESSION_INPUT_SATELLITE_V1,
};

pub fn build_alert_input_v1(
    encrypted_config: AlertMathEncryptedConfigV1,
    encrypted_convoy: EncryptedConvoyPointV1,
    encrypted_satellites: EncryptedSatelliteBatchV1,
    metadata: Vec<(String, String)>,
) -> Result<AlertMathInputV1> {
    Ok(AlertMathInputV1 {
        encrypted_config,
        encrypted_fields: AlertMathEncryptedFieldsV1 {
            convoy_payload_ct: build_encrypted_convoy_payload_v1(encrypted_convoy)?,
            satellite_payload_ct: build_encrypted_satellite_payload_v1(encrypted_satellites)?,
        },
        metadata,
    })
}

fn build_alert_compute_spec_v1(input: AlertMathInputV1) -> Result<compute_abi::ComputeJobSpec> {
    let manifest = crate::computation::manifest_v1();
    let input_blob = build_input_blob_v1(input)?;

    Ok(build_compute_job_spec(
        manifest.program_id,
        manifest.program_version,
        vec![InputPayload::Inline(input_blob)],
        vec![],
        manifest.output_slots.len() as u32,
        vec![],
    ))
}

/// Submit a SatCon alert job against an already-installed program.
///
/// This crate owns:
/// - how the program input blob is shaped
/// - the expected output count
/// - the inline invocation contract for this program version
pub async fn submit_alert_job_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    job_id: JobId,
    input: AlertMathInputV1,
    metadata: JobMetadata,
) -> Result<SubmittedComputeJob> {
    let spec = build_alert_compute_spec_v1(input)?;

    submit_compute_job(net_client, ypolo_peer, job_id, spec, metadata).await?;

    Ok(SubmittedComputeJob {
        job_id,
        artifact_blob_ids: vec![],
    })
}

pub async fn push_alert_update_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_job_id: JobId,
    input: AlertMathInputV1,
    metadata: JobMetadata,
) -> Result<()> {
    let spec = build_alert_compute_spec_v1(input)?;

    update_compute_job_input(
        net_client,
        ypolo_peer,
        session_job_id,
        spec,
        metadata,
    )
    .await
}

/// Start one long-lived alert computation session on Ypolo.
///
/// This creates the stable session/job id once.
/// Later encrypted_convoy/satellite/config updates are pushed with `PushProgramSessionInput`
/// under this same `session_id`.
pub async fn start_alert_session_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
    metadata: JobMetadata,
) -> Result<SubmittedComputeJob> {
    let manifest = crate::computation::manifest_v1();

    let spec = build_compute_job_spec(
        manifest.program_id,
        manifest.program_version,
        vec![],
        vec![],
        manifest.output_slots.len() as u32,
        vec![
            ("session.mode".into(), "streaming".into()),
            (
                "session.required_inputs".into(),
                crate::graph::required_session_inputs_v1(),
            ),
            ("runtime_kind".into(), "external-process-v1".into()),
        ],
    );

    submit_compute_job(net_client, ypolo_peer, session_id, spec, metadata).await?;

    Ok(SubmittedComputeJob {
        job_id: session_id,
        artifact_blob_ids: vec![],
    })
}

pub async fn push_alert_config_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
    config: AlertMathEncryptedConfigV1,
    observed_unix_ms: u64,
    sequence: u64,
) -> Result<()> {
    let payload = build_encrypted_config_payload_v1(config)?;

    push_program_session_input(
        net_client,
        ypolo_peer,
        session_id,
        SESSION_INPUT_CONFIG_V1,
        payload,
        observed_unix_ms,
        sequence,
    )
    .await
}

pub async fn push_convoy_payload_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
    convoy_payload_ct: Vec<u8>,
    observed_unix_ms: u64,
    sequence: u64,
) -> Result<()> {
    push_program_session_input(
        net_client,
        ypolo_peer,
        session_id,
        SESSION_INPUT_CONVOY_V1,
        convoy_payload_ct,
        observed_unix_ms,
        sequence,
    )
    .await
}

pub async fn push_satellite_payload_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
    satellite_payload_ct: Vec<u8>,
    observed_unix_ms: u64,
    sequence: u64,
) -> Result<()> {
    push_program_session_input(
        net_client,
        ypolo_peer,
        session_id,
        SESSION_INPUT_SATELLITE_V1,
        satellite_payload_ct,
        observed_unix_ms,
        sequence,
    )
    .await
}

pub async fn poll_alert_output_v1(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    session_id: JobId,
) -> Result<Option<AlertMathOutputV1>> {
    let status_resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::GetJobStatus { job_id: session_id },
        )
        .await?;

    match status_resp {
        FaceAResponse::JobStatus {
            state: JobState::Succeeded,
            ..
        } => {}

        FaceAResponse::JobStatus {
            state: JobState::Failed,
            last_error,
            ..
        } => {
            return Err(anyhow::anyhow!(
                "alert session failed: {}",
                last_error.unwrap_or_else(|| "unknown".into())
            ));
        }

        FaceAResponse::JobStatus { .. } => {
            return Ok(None);
        }

        FaceAResponse::Error { message } => {
            return Err(anyhow::anyhow!("GetJobStatus failed: {message}"));
        }

        other => {
            return Err(anyhow::anyhow!("unexpected GetJobStatus response: {other:?}"));
        }
    }

    let result_resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::GetJobResult { job_id: session_id },
        )
        .await?;

    let result = match result_resp {
        FaceAResponse::JobResult { result, .. } => result,
        FaceAResponse::Error { message } => {
            return Err(anyhow::anyhow!("GetJobResult failed: {message}"));
        }
        other => {
            return Err(anyhow::anyhow!("unexpected GetJobResult response: {other:?}"));
        }
    };

    let Some(JobResultPayload::Payload(payload)) = result else {
        return Ok(None);
    };

    let bytes = match payload {
        Payload::Inline(bytes) => bytes,
        Payload::BlobRef(blob_id) => {
            net_client
                .get_blob(ypolo_peer, blob_id)
                .await
                .map_err(|e| anyhow::anyhow!("get result blob failed: {e}"))?
        }
    };

    let output = decode_output_blob_v1(&bytes).map_err(|e| {
        anyhow::anyhow!(
            "decode AlertMathOutputV1 failed: {e}; result_bytes_len={}; first16={:02x?}",
            bytes.len(),
            &bytes[..bytes.len().min(16)]
        )
    })?;

    Ok(Some(output))
}