use anyhow::{anyhow, Result};
use futures::future::{BoxFuture, Future, join_all};
use mesh_gateway_wire::{
    ArtifactAlias, ArtifactEntry, ArtifactKind, ArtifactManifest, ArtifactSignature,
    BlobId, CapabilityInfo, CryptoJobState, CryptoOp, FaceARequest, FaceAResponse,
    FaceBRequest, FaceBResponse, JobId, JobRecord, JobResultPayload, JobState,
    JobStore, JobType, NodeType, Payload, SignedManifest, VersionSelector,
    blake3_blob_id,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, warn};

use crate::face_b::FaceBClient;

type BlobPusher = Arc<
    dyn Fn(BlobId, Vec<u8>, usize) -> Pin<Box<dyn Future<Output = Result<usize>> + Send>>
        + Send
        + Sync
>;
// Face-A runtime:
// - persists jobs
// - validates blob references
// - dispatches to Face B (Mesh B)
// - polls status/result
// - stores result as gateway-native blob (BlobId = blake3(content))
#[derive(Clone)]
pub struct GatewayRuntime {
    store: JobStore,

    // Optional Face B wiring.
    face_b: Option<FaceBClient>,
    // - 6 March -
    face_b_peers: Vec<libp2p::PeerId>,
    num_majority: usize,
    num_reconstruct: usize,
    expect_all_default: bool,
    // -
    dispatch_timeout: Duration,
    poll_interval: Duration,
    poll_deadline: Duration,
    blob_pusher: Option<BlobPusher>,
}

impl GatewayRuntime {
    // Face A-only runtime (no Face B dispatch).
    pub fn new(store: JobStore) -> Self {
        Self {
            store,
            face_b: None,
            face_b_peers: Vec::new(),
            dispatch_timeout: Duration::from_secs(300),
            poll_interval: Duration::from_secs(2),
            poll_deadline: Duration::from_secs(0),
            blob_pusher: None,
            num_majority: 1,
            num_reconstruct: 1,
            expect_all_default: false,
        }
    }

    pub fn with_blob_pusher(mut self, pusher: BlobPusher) -> Self {
        self.blob_pusher = Some(pusher);
        self
    }

    // Enable Face B dispatching.
    //
    // `face_b_peer` is the Mesh B gateway peer (or a specific Mesh B service peer).
    pub fn with_face_b(
        mut self,
        client: FaceBClient,
        face_b_peers: Vec<libp2p::PeerId>,
        num_majority: usize,
        num_reconstruct: usize,
        expect_all_default: bool,
    ) -> Self {
        self.face_b = Some(client);
        // - 6 March -
        self.face_b_peers = face_b_peers; // Some(face_b_peer); 
        self.num_majority = num_majority;
        self.num_reconstruct = num_reconstruct;
        self.expect_all_default = expect_all_default;
        // - -
        self
    }

    // Optional tuning.
    pub fn with_timeouts(mut self, dispatch_timeout: Duration, poll_interval: Duration, poll_deadline: Duration) -> Self {
        self.dispatch_timeout = dispatch_timeout;
        self.poll_interval = poll_interval;
        self.poll_deadline = poll_deadline;
        self
    }

    // Handler closure that can be attached to Face A request_response protocol.
    pub fn make_face_a_handler(
        self,
    ) -> Arc<dyn Fn(libp2p::PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync> {
        Arc::new(move |_peer, req| {
            let rt = self.clone();
            Box::pin(async move { rt.handle_face_a(req).await })
        })
    }

    async fn handle_face_a(&self, req: FaceARequest) -> FaceAResponse {
        match req {
            FaceARequest::SubmitJob {
                job_id,
                job_type,
                payload,
                metadata,
            } => {
                // Idempotency: if job exists, acknowledge as accepted (dedup).
                match self.store.load_job(&job_id).await {
                    Ok(Some(existing)) => {
                        return FaceAResponse::AckJob {
                            job_id,
                            accepted: true,
                            reason: Some(format!("dedup; state={:?}", existing.state)),
                        };
                    }
                    Ok(None) => {}
                    Err(e) => {
                        return FaceAResponse::AckJob {
                            job_id,
                            accepted: false,
                            reason: Some(format!("load job failed: {e}")),
                        };
                    }
                }

                // If payload references a blob, accept job but mark "waiting" if blob not present yet.
                let (state, last_error) = match &payload {
                    Payload::Inline(_) => (JobState::Queued, None),
                    Payload::BlobRef(blob_id) => {
                        if self.store.blob_exists(blob_id).await {
                            (JobState::Queued, None)
                        } else {
                            (JobState::Queued, Some("waiting for blob upload".to_string()))
                        }
                    }
                };

                let rec = JobRecord {
                    job_id,
                    job_type,
                    payload,
                    metadata,
                    state,
                    attempt: 0,
                    last_error,
                    result: None,
                };

                if let Err(e) = self.store.save_job(&rec).await {
                    return FaceAResponse::AckJob {
                        job_id: rec.job_id,
                        accepted: false,
                        reason: Some(format!("persist failed: {e}")),
                    };
                }

                // Fire-and-forget: attempt dispatch to Face B if configured.
                // This does not block the Face A handler.
                self.spawn_dispatch_if_possible(rec.job_id);

                FaceAResponse::AckJob {
                    job_id: rec.job_id,
                    accepted: true,
                    reason: None,
                }
            }



            FaceARequest::GetJobStatus { job_id } => match self.store.load_job(&job_id).await {
                Ok(Some(rec)) => FaceAResponse::JobStatus {
                    job_id: rec.job_id,
                    state: rec.state,
                    attempt: rec.attempt,
                    last_error: rec.last_error,
                },
                Ok(None) => FaceAResponse::Error {
                    message: "unknown job_id".into(),
                },
                Err(e) => FaceAResponse::Error {
                    message: format!("load failed: {e}"),
                },
            },

            FaceARequest::GetJobResult { job_id } => match self.store.load_job(&job_id).await {
                Ok(Some(rec)) => FaceAResponse::JobResult {
                    job_id: rec.job_id,
                    result: rec.result,
                },
                Ok(None) => FaceAResponse::Error {
                    message: "unknown job_id".into(),
                },
                Err(e) => FaceAResponse::Error {
                    message: format!("load failed: {e}"),
                },
            },

            // - 6 March -
            FaceARequest::GetArtifact { id, kind } => match self.store.get_artifact(&id, kind.clone()).await {
                Ok(Some(blob_id)) => FaceAResponse::Artifact {
                    id,
                    kind,
                    payload: Payload::BlobRef(blob_id),
                },
                Ok(None) => FaceAResponse::Error {
                    message: "unknown artifact".into(),
                },
                Err(e) => FaceAResponse::Error {
                    message: format!("load failed: {e}"),
                },
            },

            FaceARequest::GetManifest { id } => match self.store.get_manifest_by_id(&id).await {
                Ok(Some(m)) => match mesh_gateway_wire::encode(&m) {
                    Ok(bytes) => FaceAResponse::Manifest {
                        id,
                        payload: Payload::Inline(bytes),
                    },
                    Err(e) => FaceAResponse::Error {
                        message: format!("encode manifest failed: {e}"),
                    },
                },
                Ok(None) => FaceAResponse::Error {
                    message: "unknown manifest".into(),
                },
                Err(e) => FaceAResponse::Error {
                    message: format!("load failed: {e}"),
                },
            },

            FaceARequest::ResolveArtifactAlias { alias } => {
                match self.store.get_alias(&alias).await {
                    Ok(Some(manifest_cid)) => FaceAResponse::ResolvedAlias {
                        alias,
                        manifest_cid,
                    },
                    Ok(None) => FaceAResponse::Error {
                        message: "unknown artifact alias".into(),
                    },
                    Err(e) => FaceAResponse::Error {
                        message: format!("alias lookup failed: {e}"),
                    },
                }
            },

            FaceARequest::GetManifestByCid { manifest_cid } => {
                match self.store.get_signed_manifest(&manifest_cid).await {
                    Ok(Some(m)) => match mesh_gateway_wire::encode(&m) {
                        Ok(bytes) => FaceAResponse::ManifestV2 {
                            manifest_cid,
                            payload: Payload::Inline(bytes),
                        },
                        Err(e) => FaceAResponse::Error {
                            message: format!("encode signed manifest failed: {e}"),
                        },
                    },
                    Ok(None) => FaceAResponse::Error {
                        message: "unknown manifest cid".into(),
                    },
                    Err(e) => FaceAResponse::Error {
                        message: format!("manifest lookup failed: {e}"),
                    },
                }
            },

            // FaceARequest::GetPublicKey { key_id } => match self.store.get_public_key(&key_id).await {
            //     Ok(Some(blob_id)) => FaceAResponse::PublicKey { 
            //         key_id,
            //         key: Payload::BlobRef(blob_id),
            //     },
            //     Ok(None) => FaceAResponse::Error {
            //         message: "unknown key_id".into() 
            //     },
            //     Err(e) => FaceAResponse::Error { message: format!("load failed {e}") }
            // }

            // - -
            FaceARequest::AckArtifactPinned { key_id, blob_id, provider } => {
            // store ack count / provider list
            if let Err(e) = self.store.record_pin_ack(&key_id, &blob_id, provider).await {
                return FaceAResponse::Error { message: format!("record ack failed: {e}") };
            }
            
            // - 6 March -
            // FaceAResponse::AckJob { job_id: key_id, accepted: true, reason: Some("ack recorded".into()) }
            FaceAResponse::ArtifactAck { ok: true, message: Some("ack recorded".into()),
            }            
        },

            FaceARequest::GetCapabilities => FaceAResponse::Capabilities {
            info: CapabilityInfo {
                roles: vec![NodeType::Pylon],
                protocols: vec![
                    meshes_config::gateway_config::FACE_A_JOBS_PROTOCOL.to_string(),
                    meshes_config::gateway_config::FACE_A_BLOBS_PROTOCOL.to_string(),
                ],
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        },
        // - -

        FaceARequest::UpdateJobInput { job_id, .. } => {
            FaceAResponse::UpdateJobInputAck {
                job_id,
                accepted: false,
                reason: Some(
                    "UpdateJobInput is not handled by node_pylon; send it to node_ypolo / Mesh C"
                        .into(),
                ),
            }
        }

        FaceARequest::RegisterArtifact { id, kind, blob_id } => {
            match self.store.set_artifact(&id, kind, &blob_id).await {
                Ok(()) => FaceAResponse::RegisterArtifactAck {
                    ok: true,
                    message: None,
                },
                Err(e) => FaceAResponse::RegisterArtifactAck {
                    ok: false,
                    message: Some(format!("failed to register artifact: {e}")),
                },
            }
        },

        FaceARequest::DeployProgram { .. } => FaceAResponse::Error {
            message: "DeployProgram is not handled by node_pylon; send it to node_ypolo / Mesh C".into(),
        },

        FaceARequest::GetProgramInstallStatus { .. } => FaceAResponse::Error {
            message: "GetProgramInstallStatus is not handled by node_pylon; query node_ypolo / Mesh C".into(),
        },

        FaceARequest::ListInstalledPrograms => FaceAResponse::Error {
            message: "ListInstalledPrograms is not handled by node_pylon; query node_ypolo / Mesh C".into(),
        },

        FaceARequest::PushProgramSessionInput {
            session_id,
            input_kind,
            payload: _,
            observed_unix_ms,
            sequence,
        } => FaceAResponse::PushProgramSessionAck {
            session_id,
            input_kind,
            payload: Payload::Inline(vec![]),
            observed_unix_ms,
            sequence,
        },
    }

}

    fn spawn_dispatch_if_possible(&self, job_id: JobId) {
        let Some(face_b) = self.face_b.clone() else {
            debug!("Face B not configured; job remains queued: {:?}", job_id);
            return;
        };

        // - 6 March -
        if self.face_b_peers.is_empty() {
            debug!("Face B peers not configured; job remains queued: {:?}", job_id);
            return;
        }
        // let Some(face_b_peer) = self.face_b_peer else {
        //     debug!("Face B peer not configured; job remains queued: {:?}", job_id);
        //     return;
        // };
        // - -

        let rt = self.clone();
        tokio::spawn(async move {
            if let Err(e) = rt.dispatch_job_to_mesh_b(face_b, job_id).await { // - 6 March -
                warn!("dispatch_job_to_mesh_b failed job_id={:?}: {e}", job_id);
                // Ensure job is marked failed unless someone else changed it.
                let _ = rt.fail_job(job_id, e).await;
            }
        });
    }

    // Validate a job is dispatchable and transition it to Dispatching.
    //
    // Avoids requiring JobState: PartialEq by using `matches!`.
    pub async fn try_prepare_job_for_dispatch(&self, job_id: JobId) -> Result<bool> {
        let mut rec = self
            .store
            .load_job(&job_id)
            .await?
            .ok_or_else(|| anyhow!("unknown job_id"))?;

        // Only queued jobs can be prepared.
        if !matches!(rec.state, JobState::Queued) {
            return Ok(false);
        }

        // BlobRef payload must exist locally first (store-and-forward).
        if let Payload::BlobRef(blob_id) = &rec.payload {
            if !self.store.blob_exists(blob_id).await {
                rec.last_error = Some("waiting for blob upload".into());
                self.store.save_job(&rec).await?;
                return Ok(false);
            }
        }

        rec.last_error = None;
        rec.state = JobState::Dispatching;
        rec.attempt = rec.attempt.saturating_add(1);
        self.store.save_job(&rec).await?;
        Ok(true)
    }

    // - 22 May -
    async fn dispatch_job_to_mesh_b(
        &self,
        face_b: FaceBClient,
        job_id: JobId,
    ) -> Result<()> {
        if !self.try_prepare_job_for_dispatch(job_id).await? {
            return Ok(());
        }

        let rec = self
            .store
            .load_job(&job_id)
            .await?
            .ok_or_else(|| anyhow!("job disappeared after prepare"))?;

        let outbound_payload = match &rec.payload {
            Payload::Inline(b) => Payload::Inline(b.clone()),
            Payload::BlobRef(blob_id) => {
                let bytes = self
                    .store
                    .read_blob(blob_id)
                    .await
                    .map_err(|e| anyhow!("read_blob for payload failed: {e}"))?;
                Payload::Inline(bytes)
            }
        };

        let op = map_job_type_to_crypto_op(&rec.job_type)?;

        let expect_all = meta_bool(
            &rec.metadata.to_wire(),
            "expect_all_responses",
            self.expect_all_default,
        );

        let policy = threshold_policy(
            &rec.job_type,
            self.num_majority,
            self.num_reconstruct,
            expect_all,
            self.face_b_peers.len(),
        );

        let needed = match policy {
            ThresholdPolicy::All => self.face_b_peers.len(),
            ThresholdPolicy::Count(n) => n,
        };

        // ------------------------------------------------------------
        // 1) Dispatch to all Face-B peers
        // ach future owns a cloned command
        // handle, and FaceBEventLoop tracks concurrent request IDs.
        // ------------------------------------------------------------
        let mut successful_results: Vec<(libp2p::PeerId, Vec<u8>)> = Vec::new();
        let mut failed_peers: Vec<libp2p::PeerId> = Vec::new();

        let dispatch_started = Instant::now();

        let dispatch_futs = self.face_b_peers.iter().copied().map(|peer| {
            let client = face_b.clone();
            let op = op.clone();
            let payload = outbound_payload.clone();
            let metadata = rec.metadata.to_wire();
            let timeout = self.dispatch_timeout;

            async move {
                let peer_started = Instant::now();

                tracing::info!(
                    target: "faceb",
                    %peer,
                    ?job_id,
                    since_dispatch_start_ms = dispatch_started.elapsed().as_millis(),
                    "launching FaceB DispatchCryptoJob"
                );

                let dispatch_req = FaceBRequest::DispatchCryptoJob {
                    job_id,
                    op,
                    payload,
                    metadata,
                };

                let ack = client.dispatch(peer, dispatch_req, timeout).await;
                let elapsed_ms = peer_started.elapsed().as_millis();

                tracing::info!(
                    target: "faceb",
                    %peer,
                    ?job_id,
                    elapsed_ms,
                    "FaceB DispatchCryptoJob completed"
                );

                (peer, elapsed_ms, ack)
            }
        });

        let dispatch_acks = join_all(dispatch_futs).await;

        for (peer, elapsed_ms, ack) in dispatch_acks {
            match ack {
                Ok(FaceBResponse::CryptoAck { accepted: true, .. }) => {
                    tracing::info!(target:"faceb", %peer, ?job_id, "dispatch accepted");
                }

                Ok(FaceBResponse::CryptoAck { accepted: false, reason, .. }) => {
                    tracing::warn!(target:"faceb", %peer, ?job_id, "dispatch rejected: {:?}", reason);
                    failed_peers.push(peer);
                }

                Ok(FaceBResponse::Error { message }) => {
                    tracing::warn!(target:"faceb", %peer, ?job_id, "dispatch error: {}", message);
                    failed_peers.push(peer);
                }

                Err(e) => {
                    tracing::warn!(
                        target:"faceb",
                        %peer,
                        ?job_id,
                        "dispatch outcome unknown; will poll status anyway: {e}"
                    );
                    // Do NOT push failed_peers here.
                }

                Ok(other) => {
                    tracing::warn!(
                        target:"faceb",
                        %peer,
                        ?job_id,
                        "unexpected dispatch response; treating as unknown and polling: {:?}",
                        other
                    );
                    // Do NOT push failed_peers here either.
                }
            }
        }
        // Early impossibility check after dispatch phase.
        
        let remaining_possible = self.face_b_peers.len() - failed_peers.len();
        
        match policy {
            ThresholdPolicy::All => {
                if !failed_peers.is_empty() {
                    return Err(anyhow!(
                        "job requires all parties but at least one dispatch failed"
                    ));
                }
            }
            ThresholdPolicy::Count(_) => {
                if remaining_possible < needed {
                    return Err(anyhow!(
                        "job cannot reach required success threshold after dispatch: possible={}, need={}",
                        remaining_possible,
                        needed
                    ));
                }
            }
        }

        // ------------------------------------------------------------
        // 2) Poll forever, or until success/impossibility.
        //
        // Transport failures during status/result polling remain
        // non-terminal, except when the peer explicitly reports Failed
        // or returns a terminal result-shape error.
        // ------------------------------------------------------------
        loop {
            let poll_started = Instant::now();

            // ------------------------------------------------------------
            // 2.a) Poll all still-active peers in parallel.
            // ------------------------------------------------------------
            let status_futs = self
                .face_b_peers
                .iter()
                .copied()
                .filter(|peer| {
                    !successful_results.iter().any(|(p, _)| p == peer)
                        && !failed_peers.contains(peer)
                })
                .map(|peer| {
                    let client = face_b.clone();
                    let timeout = self.dispatch_timeout;

                    async move {
                        let peer_started = Instant::now();

                        tracing::debug!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            since_poll_start_ms = poll_started.elapsed().as_millis(),
                            "polling FaceB crypto job status"
                        );

                        let status = client
                            .dispatch(
                                peer,
                                FaceBRequest::GetCryptoJobStatus { job_id },
                                timeout,
                            )
                            .await;

                        let elapsed_ms = peer_started.elapsed().as_millis();

                        (peer, elapsed_ms, status)
                    }
                });

            let statuses = join_all(status_futs).await;

            let mut peers_ready_for_result: Vec<libp2p::PeerId> = Vec::new();

            for (peer, elapsed_ms, status) in statuses {
                match status {
                    Ok(FaceBResponse::CryptoJobStatus {
                        state: CryptoJobState::Succeeded,
                        ..
                    }) => {
                        tracing::info!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "peer reports succeeded"
                        );

                        if !successful_results.iter().any(|(p, _)| *p == peer)
                            && !failed_peers.contains(&peer)
                        {
                            peers_ready_for_result.push(peer);
                        }
                    }

                    Ok(FaceBResponse::CryptoJobStatus {
                        state: CryptoJobState::Failed,
                        last_error,
                        ..
                    }) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "peer failed: {:?}",
                            last_error
                        );

                        if !failed_peers.contains(&peer) {
                            failed_peers.push(peer);
                        }
                    }

                    Ok(FaceBResponse::CryptoJobStatus {
                        state: CryptoJobState::Accepted | CryptoJobState::Running,
                        ..
                    }) => {
                        tracing::debug!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "peer still running"
                        );
                    }

                    Ok(FaceBResponse::Error { message }) => {
                        tracing::debug!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "status says not ready or unknown: {}",
                            message
                        );
                        // Non-terminal by current policy.
                    }

                    Ok(other) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "unexpected status response: {:?}",
                            other
                        );
                        // Non-terminal by current policy.
                    }

                    Err(e) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "status transport failure: {e}"
                        );
                        // Non-terminal transport failure.
                    }
                }
            }

            // ------------------------------------------------------------
            // 2.b) Fetch all ready results in parallel.
            // ------------------------------------------------------------
            let result_futs = peers_ready_for_result.into_iter().map(|peer| {
                let client = face_b.clone();
                let timeout = self.dispatch_timeout;

                async move {
                    let result_started = Instant::now();

                    tracing::info!(
                        target: "faceb",
                        %peer,
                        ?job_id,
                        "fetching FaceB crypto job result"
                    );

                    let result = client
                        .dispatch(
                            peer,
                            FaceBRequest::GetCryptoJobResult { job_id },
                            timeout,
                        )
                        .await;

                    let elapsed_ms = result_started.elapsed().as_millis();

                    (peer, elapsed_ms, result)
                }
            });

            let results = join_all(result_futs).await;

            for (peer, elapsed_ms, result) in results {
                match result {
                    Ok(FaceBResponse::CryptoResult {
                        result: Payload::Inline(bytes),
                        ..
                    }) => {
                        tracing::info!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "collected successful result"
                        );

                        if !successful_results.iter().any(|(p, _)| *p == peer) {
                            successful_results.push((peer, bytes));
                        }
                    }

                    Ok(FaceBResponse::CryptoResult {
                        result: Payload::BlobRef(_),
                        ..
                    }) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "result returned BlobRef but gateway cannot fetch it yet"
                        );
                    }

                    Ok(FaceBResponse::Error { message }) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "result error: {}",
                            message
                        );
                    }

                    Ok(other) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "unexpected result response: {:?}",
                            other
                        );
                    }

                    Err(e) => {
                        tracing::warn!(
                            target: "faceb",
                            %peer,
                            ?job_id,
                            elapsed_ms,
                            "result transport failure: {e}"
                        );
                        // Non-terminal. Peer already reported Succeeded.
                        // Try fetching again in a later poll round.
                    }
                }
            }

            // ------------------------------------------------------------
            // 2.c) Threshold checks.
            // ------------------------------------------------------------
            if successful_results.len() >= needed {
                break;
            }

            let remaining_possible = self.face_b_peers.len() - failed_peers.len();

            if remaining_possible < needed {
                return Err(anyhow!(
                    "insufficient successful FaceB results: got {}, need {}, remaining possible {}",
                    successful_results.len(),
                    needed,
                    remaining_possible
                ));
            }

            if matches!(policy, ThresholdPolicy::All) && !failed_peers.is_empty() {
                return Err(anyhow!("job requires all parties but at least one party failed"));
            }

            sleep(self.poll_interval).await;
        }

        // ------------------------------------------------------------
        // 3) Store aggregated protobuf results
        // ------------------------------------------------------------
        let aggregate = mesh_gateway_wire::AggregatedJobResult {
            job_id,
            job_type: rec.job_type.clone(),
            proto_results: successful_results
                .iter()
                .map(|(peer, bytes)| mesh_gateway_wire::PartyProtoResult {
                    peer: *peer,
                    bytes: bytes.clone(),
                })
                .collect(),
        };

        let agg_bytes = mesh_gateway_wire::encode(&aggregate)?;
        let agg_blob_id = self.store_result_blob(&agg_bytes).await?;

        // ------------------------------------------------------------
        // 4) Publish public artifacts for keygen / crsgen
        // ------------------------------------------------------------
        let final_result = match rec.job_type {
            JobType::Keygen => {
                let manifest_cid = self
                    .publish_keygen_artifacts(face_b.clone(), job_id, successful_results[0].0)
                    .await?;
                Some(JobResultPayload::PublishedManifest { manifest_cid })
            }
            JobType::CrsGen => {
                let manifest_cid = self
                    .publish_crs_artifact(face_b.clone(), job_id, successful_results[0].0)
                    .await?;
                Some(JobResultPayload::PublishedManifest { manifest_cid })
            }
            _ => Some(JobResultPayload::Payload(Payload::BlobRef(agg_blob_id))),
        };

        self.succeed_job(job_id, final_result).await?;
        Ok(())
    }
    // - -


    // Store result bytes as a gateway-native blob and return its BlobId.
    //
    // BlobId is BLAKE3(content) as 32 bytes, used for dedup.
    pub async fn store_result_blob(&self, bytes: &[u8]) -> Result<BlobId> {
        let blob_id = blake3_blob_id(bytes);
        if !self.store.blob_exists(&blob_id).await {
            self.store.write_blob(&blob_id, bytes).await?;
        }
        Ok(blob_id)
    }

    // - 6 March -
    async fn fetch_faceb_artifact(
        &self,
        face_b: FaceBClient,
        peer: libp2p::PeerId,
        id: [u8; 32],
        kind: mesh_gateway_wire::ArtifactKind,
    ) -> Result<Vec<u8>> {
        let resp = {
            let c = face_b.clone();
            c.dispatch(
                peer,
                mesh_gateway_wire::FaceBRequest::GetArtifact { id, kind: kind.clone() },
                self.dispatch_timeout,
            )
            .await?
        };

        match resp {
            mesh_gateway_wire::FaceBResponse::Artifact { payload: Payload::Inline(bytes), .. } => Ok(bytes),
            mesh_gateway_wire::FaceBResponse::Artifact { payload: Payload::BlobRef(_), .. } => {
                Err(anyhow!("FaceB artifact BlobRef not implemented"))
            }
            mesh_gateway_wire::FaceBResponse::Error { message } => Err(anyhow!("FaceB artifact error: {message}")),
            other => Err(anyhow!("unexpected FaceB artifact response: {other:?}")),
        }
    }

    async fn publish_artifact_bytes(
        &self,
        id: [u8; 32],
        kind: mesh_gateway_wire::ArtifactKind,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let blob_id = mesh_gateway_wire::blake3_blob_id(&bytes);
        if !self.store.blob_exists(&blob_id).await {
            self.store.write_blob(&blob_id, &bytes).await?;
        }

        // First make the artifact visible through the gateway store.
        self.store.set_artifact(&id, kind.clone(), &blob_id).await?;

        // Then try proactive push as a best-effort optimization.
        let pushed = match &self.blob_pusher {
            Some(p) => match p(blob_id, bytes, 3).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!(
                        target: "artifacts",
                        ?id,
                        ?kind,
                        ?blob_id,
                        "proactive blob push skipped/failed: {e}"
                    );
                    0
                }
            },
            None => {
                tracing::debug!(
                    target: "artifacts",
                    ?id,
                    ?kind,
                    ?blob_id,
                    "blob_pusher not configured; artifact remains gateway-pull only"
                );
                0
            }
        };

        tracing::info!(
            target: "artifacts",
            ?id,
            ?kind,
            ?blob_id,
            pushed,
            "artifact published"
        );

        Ok(())
    }

    async fn publish_keygen_artifacts(
        &self,
        face_b: FaceBClient,
        key_id: [u8; 32],
        peer: libp2p::PeerId,
    ) -> Result<BlobId> {
        for kind in [
            ArtifactKind::PublicKey,
            ArtifactKind::PublicKeyMetadata,
            ArtifactKind::ServerKey,
        ] {
            let bytes = self
                .fetch_faceb_artifact(face_b.clone(), peer, key_id, kind.clone())
                .await?;
            self.publish_artifact_bytes(key_id, kind, bytes).await?;
        }

        self.publish_keygen_manifest(key_id).await
    }

    async fn publish_crs_artifact(
        &self,
        face_b: FaceBClient,
        crs_id: [u8; 32],
        peer: libp2p::PeerId,
    ) -> Result<BlobId> {
        let bytes = self
            .fetch_faceb_artifact(face_b, peer, crs_id, ArtifactKind::Crs)
            .await?;

        self.publish_artifact_bytes(crs_id, ArtifactKind::Crs, bytes)
            .await?;

        self.publish_crs_manifest(crs_id).await
    }
    // - -
    async fn publish_manifest_for_id(
        &self,
        id: [u8; 32],
        version: u64,
        alias: ArtifactAlias,
    ) -> Result<BlobId> {
        let existing = self
            .store
            .get_manifest_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("missing existing manifest for published artifact set"))?;

        let mut entries = Vec::with_capacity(existing.manifest.entries.len());

        for entry in existing.manifest.entries {
            let bytes = self
                .store
                .read_blob(&entry.cid)
                .await
                .map_err(|e| anyhow!("read_blob for manifest entry failed: {e}"))?;

            entries.push(ArtifactEntry {
                kind: entry.kind,
                cid: entry.cid,
                byte_len: bytes.len() as u64,
                media_type: "application/octet-stream".into(),
            });
        }

        let manifest = ArtifactManifest {
            id,
            version,
            entries,
            metadata: existing.manifest.metadata,
        };

        let signed = SignedManifest {
            manifest,
            signatures: Vec::<ArtifactSignature>::new(),
        };

        let manifest_bytes = mesh_gateway_wire::encode(&signed)?;
        let manifest_cid = blake3_blob_id(&manifest_bytes);

        self.store
            .set_signed_manifest(&manifest_cid, &signed)
            .await?;
        self.store.set_alias(&alias, &manifest_cid).await?;

        tracing::info!(
            target: "artifacts",
            ?id,
            ?manifest_cid,
            namespace = %alias.namespace,
            logical_name = %alias.logical_name,
            "published v2 signed manifest and alias"
        );

        Ok(manifest_cid)
    }

    async fn publish_keygen_manifest(
        &self,
        key_id: [u8; 32],
    ) -> Result<BlobId> {
        let alias = ArtifactAlias {
            namespace: "mesh_b_keyset".into(),
            logical_name: hex32(key_id),
            version_selector: VersionSelector::Latest,
        };

        self.publish_manifest_for_id(key_id, 1, alias).await
    }

    async fn publish_crs_manifest(
        &self,
        crs_id: [u8; 32],
    ) -> Result<BlobId> {
        let alias = ArtifactAlias {
            namespace: "mesh_b_crs".into(),
            logical_name: hex32(crs_id),
            version_selector: VersionSelector::Latest,
        };

        self.publish_manifest_for_id(crs_id, 1, alias).await
    }
    
    // Mark job succeeded with an optional result payload.
    pub async fn succeed_job(&self, job_id: JobId, result: Option<JobResultPayload>) -> Result<()> {
        let mut rec = self
            .store
            .load_job(&job_id)
            .await?
            .ok_or_else(|| anyhow!("unknown job_id"))?;

        rec.state = JobState::Succeeded;
        rec.result = result;
        rec.last_error = None;
        self.store.save_job(&rec).await?;
        Ok(())
    }

    // Mark job failed and preserve error message.
    pub async fn fail_job(&self, job_id: JobId, err: anyhow::Error) -> Result<()> {
        let mut rec = self
            .store
            .load_job(&job_id)
            .await?
            .ok_or_else(|| anyhow!("unknown job_id"))?;

        rec.state = JobState::Failed;
        rec.last_error = Some(err.to_string());
        self.store.save_job(&rec).await?;
        Ok(())
    }
}


fn hex32(id: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in id {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

// Map FaceA JobType -> Mesh B CryptoOp.
//
// Adjust this mapping to your real `JobType` variants.
// The goal: job_type is Mesh A semantic, op is Mesh B crypto operation.
fn map_job_type_to_crypto_op(t: &JobType) -> Result<CryptoOp> {
    // Replace these with your actual JobType variants.
    // Keep it exhaustive: failing here is better than silent misrouting.
    match t {
        JobType::KeygenPreproc => Ok(CryptoOp::KeygenPreproc),
        JobType::Keygen => Ok(CryptoOp::Keygen),
        JobType::PublicDecrypt => Ok(CryptoOp::PublicDecrypt),
        JobType::UserDecrypt => Ok(CryptoOp::UserDecrypt),
        JobType::CrsGen => Ok(CryptoOp::CrsGen),
        JobType::ReshareInit => Ok(CryptoOp::ReshareInit),
        JobType::ReshareResult => Ok(CryptoOp::ReshareResult),
        // Mesh C must not be routed to Mesh B.
        JobType::FheComputing => Err(anyhow!(
            "JobType::FheComputing is a Mesh C job and must not be dispatched by node_pylon to Face B"
        )),
    }
}

// - 6 March -
#[derive(Clone, Copy)]
enum ThresholdPolicy {
    All,
    Count(usize),
}

fn threshold_policy(
    job_type: &JobType,
    num_majority: usize,
    num_reconstruct: usize,
    expect_all: bool,
    num_parties: usize,
) -> ThresholdPolicy {
    if expect_all {
        return ThresholdPolicy::All;
    }

    match job_type {
        JobType::KeygenPreproc => ThresholdPolicy::All,
        JobType::Keygen => ThresholdPolicy::Count(num_majority),
        JobType::CrsGen => ThresholdPolicy::Count(num_majority),
        JobType::PublicDecrypt => ThresholdPolicy::Count(num_majority),
        JobType::UserDecrypt => ThresholdPolicy::Count(num_reconstruct),
        JobType::ReshareInit | JobType::ReshareResult => ThresholdPolicy::All,

        // Mesh C must never use Face-B threshold policy.
        JobType::FheComputing => {
            unreachable!("JobType::FheComputing must never reach node_pylon Face-B threshold policy")
        }
    }
}

fn meta_bool(meta: &[(String, String)], key: &str, default: bool) -> bool {
    meta.iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse::<bool>().ok())
        .unwrap_or(default)
}
// -