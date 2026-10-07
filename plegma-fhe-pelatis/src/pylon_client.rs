use anyhow::{anyhow, Result};
use libp2p::PeerId;
use mesh_gateway_wire::{ArtifactKind, BlobId, FaceARequest, FaceAResponse, JobId, JobResultPayload, JobMetadata, JobState, JobType, Payload};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::RwLock,
    time::{sleep, Duration},
};

use crate::{artifact_cache::ArtifactCache, types::PylonPeer};

pub enum ResolvedJobResult {
    Payload(Payload),
    PublishedManifest { manifest_cid: BlobId },
    }
/// Mesh-side client for talking to Pylon over Face A.
///
/// This is intentionally small and transport-focused:
/// - track discovered Pylon peers
/// - choose one candidate
/// - submit jobs
/// - poll results
/// - fetch/cache public artifacts
///
/// Higher-level orchestration (decode AggregatedJobResult, reuse kms_core_client
/// verification/reconstruction logic) will sit on top of this.
#[derive(Clone)]
pub struct MeshGatewayClient {
    net: libp2p_common::komvos::Client,
    pylons: Arc<RwLock<HashMap<PeerId, PylonPeer>>>,
    cache: ArtifactCache,
    cache_party_ids: Vec<usize>,
    poll_interval: Duration,
}

impl MeshGatewayClient {
    pub fn new(
        net: libp2p_common::komvos::Client,
        cache: ArtifactCache,
        cache_party_ids: Vec<usize>,
    ) -> Self {
        Self {
            net,
            pylons: Arc::new(RwLock::new(HashMap::new())),
            cache,
            cache_party_ids,
            poll_interval: Duration::from_millis(500),
        }
    }

    pub fn cache(&self) -> &ArtifactCache {
        &self.cache
    }

    pub fn cache_party_ids(&self) -> &[usize] {
        &self.cache_party_ids
    }

    /// Insert or refresh a Pylon candidate discovered through Mesh A discovery.
    ///
    /// The caller is responsible for deciding which discovered peers are actual
    /// Pylons. At first, this can be fed from your existing discovery flow.
    /// Later, add protocol/capability probing before inserting.
    pub async fn upsert_pylon(&self, peer_id: PeerId, addrs: Vec<libp2p::Multiaddr>) {
        let now = now_unix_ms();

        let mut g = self.pylons.write().await;
        match g.get_mut(&peer_id) {
            Some(existing) => {
                for addr in addrs {
                    if !existing.addrs.iter().any(|a| a == &addr) {
                        existing.addrs.push(addr);
                    }
                }
                existing.last_seen_unix_ms = now;
            }
            None => {
                g.insert(
                    peer_id,
                    PylonPeer {
                        peer_id,
                        addrs,
                        last_seen_unix_ms: now,
                    },
                );
            }
        }
    }

    pub async fn remove_pylon(&self, peer_id: &PeerId) {
        self.pylons.write().await.remove(peer_id);
    }

    pub async fn list_pylons(&self) -> Vec<PylonPeer> {
        self.pylons.read().await.values().cloned().collect()
    }

    /// Current simple selection policy:
    /// - most recently seen candidate wins
    ///
    /// This is intentionally minimal. Health-aware scoring can be added later.
    pub async fn choose_pylon(&self) -> Result<PeerId> {
        let mut peers: Vec<_> = self.pylons.read().await.values().cloned().collect();
        peers.sort_by_key(|p| std::cmp::Reverse(p.last_seen_unix_ms));

        peers
            .into_iter()
            .next()
            .map(|p| p.peer_id)
            .ok_or_else(|| anyhow!("no Pylon peer available"))
    }

    pub async fn get_blob_from_pylon(
        &mut self,
        pylon: PeerId,
        blob_id: [u8; 32],
    ) -> Result<Vec<u8>> {
        self.net.get_blob(pylon, blob_id).await.map_err(Into::into)
    }

    pub async fn submit_job(
        &mut self,
        job_id: JobId,
        job_type: JobType,
        payload: Payload,
        metadata: JobMetadata,
    ) -> Result<()> {
        let pylon = self.choose_pylon().await?;
        let req = FaceARequest::SubmitJob {
            job_id,
            job_type,
            payload,
            metadata,
        };

        let resp = self.net.jobs_request(pylon, req).await?;
        match resp {
            FaceAResponse::AckJob { accepted: true, .. } => Ok(()),
            FaceAResponse::AckJob { accepted: false, reason, .. } => {
                Err(anyhow!("job rejected by Pylon: {:?}", reason))
            }
            FaceAResponse::Error { message } => Err(anyhow!("Pylon returned error: {message}")),
            other => Err(anyhow!("unexpected SubmitJob response: {other:?}")),
        }
    }

    pub async fn get_job_status(&mut self, job_id: JobId) -> Result<FaceAResponse> {
        let pylon = self.choose_pylon().await?;
        self.net
            .jobs_request(pylon, FaceARequest::GetJobStatus { job_id })
            .await
            .map_err(Into::into)
    }

    pub async fn get_job_result(&mut self, job_id: JobId) -> Result<(PeerId, Option<JobResultPayload>)> {
        let pylon = self.choose_pylon().await?;
        let resp = self
            .net
            .jobs_request(pylon, FaceARequest::GetJobResult { job_id })
            .await?;

        match resp {
            FaceAResponse::JobResult { result, .. } => Ok((pylon, result)),
            FaceAResponse::Error { message } => Err(anyhow!("Pylon returned error: {message}")),
            other => Err(anyhow!("unexpected GetJobResult response: {other:?}")),
        }
    }

    /// Poll until the gateway has a job result or the retry budget is exhausted.
    ///
    /// This mirrors the old core-client polling cadence:
    /// - 500 ms between attempts
    /// - bounded by max_iter
    pub async fn poll_job_result(
        &mut self,
        job_id: JobId,
        max_iter: usize,
    ) -> Result<Option<JobResultPayload>> {
        for _ in 0..max_iter {
            if let (_pylon_peer, Some(payload) )= self.get_job_result(job_id).await? {
                return Ok(Some(payload));
            }
            sleep(self.poll_interval).await;
        }
        Ok(None)
    }

    pub async fn wait_job_result(&mut self, job_id: JobId) -> Result<(PeerId, JobResultPayload)> {
        loop {
            let status = self.get_job_status(job_id).await?;

            match status {
                FaceAResponse::JobStatus {
                    state,
                    last_error,
                    ..
                } => match state {
                    JobState::Succeeded => {
                        let (pylon, maybe_payload) = self.get_job_result(job_id).await?;
                        let payload = maybe_payload.ok_or_else(|| {
                            anyhow!("job succeeded but returned no result payload")
                        })?;
                        return Ok((pylon, payload));
                    }

                    JobState::Failed | JobState::Expired => {
                        return Err(anyhow!(
                            "job reached terminal state {:?}: {}",
                            state,
                            last_error.unwrap_or_else(|| "no error message".to_string())
                        ));
                    }

                    JobState::Accepted
                    | JobState::Queued
                    | JobState::Dispatching
                    | JobState::InProgress => {
                        sleep(Duration::from_millis(500)).await;
                    }
                },

                FaceAResponse::Error { message } => {
                    return Err(anyhow!("GetJobStatus failed: {message}"));
                }

                other => {
                    return Err(anyhow!("unexpected GetJobStatus response: {other:?}"));
                }
            }
        }
    }


    pub async fn wait_job_result_resolved(
        &mut self,
        job_id: JobId,
    ) -> Result<(PeerId, ResolvedJobResult)> {
        let (peer, result) = self.wait_job_result(job_id).await?;

        let resolved = match result {
            JobResultPayload::Payload(p) => ResolvedJobResult::Payload(p),
            JobResultPayload::PublishedManifest { manifest_cid } => {
                ResolvedJobResult::PublishedManifest { manifest_cid }
            }
        };

        Ok((peer, resolved))
    }

    /// Fetch a typed public artifact from Pylon and cache it locally.
    ///
    /// Behavior:
    /// - if already cached, returns cached bytes
    /// - otherwise queries Pylon via Face A
    /// - if Pylon returns BlobRef, fetches bytes through Face A blob plane
    /// - writes into local cache
    pub async fn fetch_artifact(
        &mut self,
        id: [u8; 32],
        kind: ArtifactKind,
    ) -> Result<Vec<u8>> {
        if self
            .cache
            .has_in_any_party(&self.cache_party_ids, &id, kind.clone())
            .await
        {
            return self
                .cache
                .get_from_any_party(&self.cache_party_ids, &id, kind)
                .await;
        }

        let pylon = self.choose_pylon().await?;
        let resp = self
            .net
            .jobs_request(
                pylon,
                FaceARequest::GetArtifact {
                    id,
                    kind: kind.clone(),
                },
            )
            .await?;

        let payload = match resp {
            FaceAResponse::Artifact { payload, .. } => payload,
            FaceAResponse::Error { message } => {
                return Err(anyhow!("artifact fetch failed: {message}"))
            }
            other => {
                return Err(anyhow!("unexpected GetArtifact response: {other:?}"))
            }
        };

        let bytes = match payload {
            Payload::Inline(b) => b,
            Payload::BlobRef(blob_id) => self.net.get_blob(pylon, blob_id).await?,
        };

        self.cache
            .put_for_parties(&self.cache_party_ids, &id, kind, &bytes)
            .await?;
        Ok(bytes)
    }

    pub async fn fetch_manifest(&mut self, id: [u8; 32]) -> Result<mesh_gateway_wire::ArtifactManifest> {
        let pylon = self.choose_pylon().await?;
        let resp = self
            .net
            .jobs_request(pylon, FaceARequest::GetManifest { id })
            .await?;

        let payload = match resp {
            FaceAResponse::Manifest { payload, .. } => payload,
            FaceAResponse::Error { message } => {
                return Err(anyhow!("manifest fetch failed: {message}"))
            }
            other => {
                return Err(anyhow!("unexpected GetManifest response: {other:?}"))
            }
        };

        let bytes = match payload {
            Payload::Inline(b) => b,
            Payload::BlobRef(blob_id) => self.net.get_blob(pylon, blob_id).await?,
        };

        mesh_gateway_wire::decode::<mesh_gateway_wire::ArtifactManifest>(&bytes)
            .map_err(|e| anyhow!("failed to decode ArtifactManifest: {e}"))
    }

    /// Materialize all manifest entries into the local cache.
    pub async fn materialize_manifest(&mut self, id: [u8; 32]) -> Result<()> {
        let manifest = self.fetch_manifest(id).await?;
        for entry in manifest.entries {
            let _ = self.fetch_artifact(id, entry.kind).await?;
        }
        Ok(())
    }

        pub async fn choose_pylon_public(&self) -> Result<PeerId> {
        self.choose_pylon().await
    }

    pub fn raw_client_mut(&mut self) -> &mut libp2p_common::komvos::Client {
        &mut self.net
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}