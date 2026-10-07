use anyhow::{anyhow, Result};
use libp2p::PeerId;
use libp2p_common::komvos::Client as FaceAClient;
use mesh_gateway_wire::{ArtifactKind, FaceARequest, FaceAResponse, JobId, JobResultPayload, JobState, Payload};
use tokio::time::{sleep, Duration, Instant};

pub async fn wait_for_job_result(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    job_id: JobId,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<Option<JobResultPayload>> {
    let deadline = Instant::now() + timeout;

    loop {
        if Instant::now() >= deadline {
            return Err(anyhow!("timeout waiting for job result"));
        }

        let status_resp = net_client
            .jobs_request(ypolo_peer, FaceARequest::GetJobStatus { job_id })
            .await?;

        match status_resp {
            FaceAResponse::JobStatus {
                state: JobState::Succeeded,
                ..
            } => {
                let result_resp = net_client
                    .jobs_request(ypolo_peer, FaceARequest::GetJobResult { job_id })
                    .await?;

                match result_resp {
                    FaceAResponse::JobResult { result, .. } => return Ok(result),
                    FaceAResponse::Error { message } => {
                        return Err(anyhow!("GetJobResult failed: {message}"));
                    }
                    other => return Err(anyhow!("unexpected GetJobResult response: {other:?}")),
                }
            }

            FaceAResponse::JobStatus {
                state: JobState::Failed,
                last_error,
                ..
            } => {
                return Err(anyhow!(
                    "job failed: {}",
                    last_error.unwrap_or_else(|| "unknown".into())
                ));
            }

            FaceAResponse::JobStatus { .. } => {
                sleep(poll_interval).await;
            }

            FaceAResponse::Error { message } => {
                return Err(anyhow!("GetJobStatus failed: {message}"));
            }

            other => return Err(anyhow!("unexpected GetJobStatus response: {other:?}")),
        }
    }
}

pub async fn fetch_receipt_bytes(
    net_client: &mut FaceAClient,
    ypolo_peer: PeerId,
    job_id: JobId,
) -> Result<Vec<u8>> {
    let receipt_resp = net_client
        .jobs_request(
            ypolo_peer,
            FaceARequest::GetArtifact {
                id: job_id,
                kind: ArtifactKind::ComputeReceipt,
            },
        )
        .await?;

    match receipt_resp {
        FaceAResponse::Artifact {
            payload: Payload::BlobRef(blob_id),
            ..
        } => net_client
            .get_blob(ypolo_peer, blob_id)
            .await
            .map_err(Into::into),

        FaceAResponse::Artifact {
            payload: Payload::Inline(bytes),
            ..
        } => Ok(bytes),

        FaceAResponse::Error { message } => Err(anyhow!("GetArtifact failed: {message}")),

        other => Err(anyhow!("unexpected receipt response: {other:?}")),
    }
}