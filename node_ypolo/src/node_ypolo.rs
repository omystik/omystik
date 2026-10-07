use anyhow::{anyhow, Context, Result};
use compute_abi::{ComputeJobSpec, InputPayload, InstalledProgramRecord, ProgramRuntimeKind};
use compute_core::{ComputeArtifactInput, ComputeExecutor, ComputeSessionInput, ExecutorInputs};
use compute_program::runtime_kind_is_supported;
use content_hashing::{get_keys_dir, blakecheck::send_key};
use futures::{future::BoxFuture, prelude::*, StreamExt};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use libp2p_common::{
    komvos::{self, Client, Event, EventLoop},
    jobscodec::write_frame_blob, 
    key_ops,
    node_services::KeyServices,
    node_primary::{KeyType, NodeRole, NodeType},
};


use mesh_gateway_wire::{
    blake3_blob_id, decode, encode, ArtifactKind, CapabilityInfo, FaceABlobHeader, FaceARequest,
    FaceAResponse, JobId, JobRecord, JobResultPayload, JobState, JobStore, JobType, Payload, NodeType as Gateway_NodeType
};
use std::collections::HashMap;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{sync::Mutex, time::Duration};
use tokio_util::compat::FuturesAsyncWriteCompatExt; // try to make compatibility between tokio and libp2p stream

use tracing::{debug, warn};

use crate::artifacts::load_artifact_bytes;
use crate::executor_tfhe::TfheExecutor;
use crate::executor_external_process::ExternalProcessExecutor;
use crate::program_manifest::{
    load_program_manifest_by_blob_id, validate_program_invocation,
    validate_program_manifest_for_install,
};
use crate::receipt::{build_receipt, encode_receipt};

#[derive(Debug, Clone)]
struct ProgramSessionInputEntry {
    bytes: Vec<u8>,
    observed_unix_ms: u64,
    sequence: u64,
}

#[derive(Debug, Default)]
struct ProgramSessionState {
    inputs: HashMap<String, ProgramSessionInputEntry>,
}

type ProgramSessions = Arc<Mutex<HashMap<JobId, ProgramSessionState>>>;

pub struct YpoloManager {
    pub node_type: NodeType,
    pub node_role: NodeRole,
    pub client: Client,
    pub event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
    pub event_loop: Arc<Mutex<EventLoop>>,
    pub listen_address: Option<Multiaddr>,
    pub peer: Option<Multiaddr>,
    store: JobStore,
    executors: Arc<HashMap<ProgramRuntimeKind, Arc<dyn ComputeExecutor>>>,
    sessions: ProgramSessions,
}

impl YpoloManager {

    pub fn key_services(&mut self) -> KeyServices<'_> {
        KeyServices {
            client: &mut self.client,
            peer: self.peer.as_ref(),
        }
    }

    pub async fn new(
        secret_seed: Option<u8>,
        listen_address: Option<Multiaddr>,
        peer: Option<Multiaddr>,
        data_dir: PathBuf,

    ) -> Result<Self> {

        // setting default Ypolo type
        let node_role = NodeRole::NonAdmin;
        let node_type = NodeType::Ypolo;
        
        let (mut client, event_stream, composite_event_loop) =
            komvos::new(secret_seed, node_type).await?;

        
        
        let listen_addr_handle = composite_event_loop.actual_listen_addr.clone();
        let event_loop = Arc::new(Mutex::new(composite_event_loop));
        let store = JobStore::open(data_dir).await?;

        {
            let event_loop_clone = event_loop.clone();
            tokio::spawn(async move {
                event_loop_clone.lock().await.run().await;
            });
        }

        let local_peer_id: PeerId = {
            let guard = event_loop.lock().await;
            guard.local_peer_id()
        };

        match &listen_address {
            Some(addr) => {
                client.start_listening(addr.clone()).await?;
            }
            None => {
                client
                    .start_listening("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)
                    .await?;
            }
        }

        if let Some(addr) = &peer {
            let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
                return Err(anyhow!("Expected peer multiaddr to contain peer ID"));
            };
            client.dial(peer_id, addr.clone()).await?;
        }

        let addr_to_advertise = match &listen_address {
            Some(user_addr) => user_addr.clone(),
            None => loop {
                let maybe_addr = {
                    let guard = listen_addr_handle.lock().await;
                    (*guard).clone()
                };
                if let Some(addr) = maybe_addr {
                    break addr;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        };

        client.advertise(local_peer_id, addr_to_advertise).await?;

        let executors = Arc::new(HashMap::new());

        let sessions: ProgramSessions = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self {
            client,
            event_stream: Box::pin(event_stream),
            event_loop,
            listen_address,
            peer,
            store,
            executors,
            sessions,
            node_role,
            node_type,
        })
    }

    pub async fn install_jobs_handler(&mut self) -> Result<()> {
        let handler = self.make_face_a_handler();
        self.client.set_jobs_handler(Some(handler)).await
    }

    pub async fn launch_event_task(&mut self) -> Result<()> {
        let store = self.store.clone();
        let mut event_stream =
            std::mem::replace(&mut self.event_stream, Box::pin(futures::stream::empty()));

        tokio::spawn(async move {
            while let Some(event) = event_stream.next().await {
                match event {
                    Event::InboundFileRequest { peer, file_name, .. } => {
                        debug!(
                            "Ypolo ignoring inbound file request from {} for '{}'",
                            peer,
                            file_name
                        );
                    }
                    Event::InboundBlobRequest {
                        peer,
                        header,
                        data: _,
                        stream,
                    } => match header {
                        FaceABlobHeader::Get { blob_id } => {
                            let Some(mut stream) = stream else {
                                warn!("missing stream for inbound blob get from {}", peer);
                                continue;
                            };

                            let bytes = match store.read_blob(&blob_id).await {
                                Ok(b) => b,
                                Err(e) => {
                                    let ack = FaceABlobHeader::Ack {
                                        ok: false,
                                        message: Some(format!("missing blob: {e}")),
                                    };
                                    if let Ok(ack_bytes) = encode(&ack) {
                                        let _ = write_frame_blob(&mut stream, &ack_bytes).await;
                                    }
                                    let _ = stream.close().await;
                                    continue;
                                }
                            };

                            let ack = FaceABlobHeader::Ack {
                                ok: true,
                                message: None,
                            };
                            match encode(&ack) {
                                Ok(ack_bytes) => {
                                    if let Err(e) = write_frame_blob(&mut stream, &ack_bytes).await
                                    {
                                        warn!("failed to write blob ack to {}: {}", peer, e);
                                        let _ = stream.close().await;
                                        continue;
                                    }
                                }
                                Err(e) => {
                                    warn!("failed to encode blob ack: {}", e);
                                    let _ = stream.close().await;
                                    continue;
                                }
                            }

                            if let Err(e) = stream.write_all(&bytes).await {
                                warn!("failed to stream blob bytes to {}: {}", peer, e);
                            }
                            let _ = stream.close().await;
                        }

                        FaceABlobHeader::Put { blob_id, size } => {
                            debug!(
                                "Inbound blob put already consumed by ypolo transport layer: peer={}, blob={:?}, size={}",
                                peer, blob_id, size
                            );
                        }

                        FaceABlobHeader::Ack { .. } => {
                            debug!("Ignoring inbound blob Ack event from {}", peer);
                        }
                    },

                    Event::InboundKeyRequest { peer, key_type, stream } => {

                        let keys_root = get_keys_dir();

                        let key_dir = Path::new(&keys_root).join(key_type.as_str());

                        match key_ops::single_filename(&key_dir) {
                            Ok(Some(key_name)) => {
                                let key_path = key_dir.join(&key_name);
                                println!("Serving key file: {}", key_path.display());

                                let (_reader, writer) = stream.split();
                                let writer = writer.compat_write();

                                tokio::spawn(async move {
                                    debug!("Handling file request from {:?} for file '{}'", peer, key_name.clone());
                                    if let Err(e) = send_key(writer, &key_path).await {
                                        eprintln!("Error sending key: {} look path: {}", e, key_path.display());
                                    }
                                });
                            }
                            Ok(None) => {
                                eprintln!("Could not resolve unique key file in {}", key_dir.display());
                            }
                            Err(e) => {
                                eprintln!("Failed to inspect {}: {}", key_dir.display(), e);
                            }
                        }
                    }
                    Event::PeerIdentified {
                        peer,
                        protocol_version,
                        agent_version,
                        listen_addrs,
                    } => {
                        debug!(
                            "PeerIdentified peer={} protocol={} agent={} addrs={:?}",
                            peer, protocol_version, agent_version, listen_addrs
                        );
                    }                   
                    Event::Discovered { peer } => {
                        debug!("Discovered peer {}", peer);
                    }
                }
            }
        });

        Ok(())
    }

    pub fn make_face_a_handler(
        &self,
    ) -> Arc<dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync> {
        let store = self.store.clone();
        let executors = self.executors.clone();
        let sessions = self.sessions.clone();

        Arc::new(move |_peer, req| {
            let store = store.clone();
            let executors = executors.clone();
            let sessions = sessions.clone();
            Box::pin(async move { handle_face_a_request(store, executors, sessions, req).await })
        })
    }

    // ask for key to mesh
    pub async fn ask_key(&mut self) -> anyhow::Result<()> {
        
        self.key_services()
            .ask_key(Some(KeyType::ServerKey))
            .await
    }

    pub fn configure_tfhe_executor(&mut self, server_key_path: PathBuf) {
        let mut executors_map: HashMap<ProgramRuntimeKind, Arc<dyn ComputeExecutor>> =
            (*self.executors).clone();

        executors_map.insert(
            ProgramRuntimeKind::ComputeGraphV1,
            Arc::new(TfheExecutor::new(server_key_path.clone())) as Arc<dyn ComputeExecutor>,
        );

        executors_map.insert(
            ProgramRuntimeKind::ExternalProcessV1,
            Arc::new(ExternalProcessExecutor::new(
                self.store.clone(),
                server_key_path,
                PathBuf::from("./data/ypolo-external-work"),
            )) as Arc<dyn ComputeExecutor>,
        );

        self.executors = Arc::new(executors_map);
    }
}

async fn handle_face_a_request(
    store: JobStore,
    executors: Arc<HashMap<ProgramRuntimeKind, Arc<dyn ComputeExecutor>>>,
    sessions: ProgramSessions,
    req: FaceARequest,
) -> FaceAResponse {
    match req {
        FaceARequest::SubmitJob {
            job_id,
            job_type,
            payload,
            metadata,
        } => {
            match store.load_job(&job_id).await {
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

            if !matches!(job_type, JobType::FheComputing) {
                return FaceAResponse::AckJob {
                    job_id,
                    accepted: false,
                    reason: Some("node_ypolo only accepts JobType::FheComputing".into()),
                };
            }

            let compute_spec = match decode_compute_job_spec(&store, &payload).await {
                Ok(spec) => spec,
                Err(e) => {
                    return FaceAResponse::AckJob {
                        job_id,
                        accepted: false,
                        reason: Some(format!("invalid compute payload: {e}")),
                    };
                }
            };
            tracing::info!(
                target: "ypolo_session",
                job_id = %hex::encode(job_id),
                metadata = ?compute_spec.metadata,
                required = ?required_session_input_kinds_from_spec(&compute_spec),
                "SubmitJob ComputeJobSpc session metadata"
            );
            
            let installed = match store
                .get_installed_program(&compute_spec.program_id, &compute_spec.program_version)
                .await
            {
                Ok(Some(rec)) => rec,
                Ok(None) => {
                    return FaceAResponse::AckJob {
                        job_id,
                        accepted: false,
                        reason: Some("program is not installed".into()),
                    };
                }
                Err(e) => {
                    return FaceAResponse::AckJob {
                        job_id,
                        accepted: false,
                        reason: Some(format!("failed to load installed program: {e}")),
                    };
                }
            };

            let manifest = match load_program_manifest_by_blob_id(&store, &installed.manifest_blob_id).await
            {
                Ok(m) => m,
                Err(e) => {
                    return FaceAResponse::AckJob {
                        job_id,
                        accepted: false,
                        reason: Some(format!("failed to load installed program manifest: {e}")),
                    };
                }
            };

            if let Err(e) = validate_program_invocation(&compute_spec, &manifest) {
                return FaceAResponse::AckJob {
                    job_id,
                    accepted: false,
                    reason: Some(format!("program invocation validation failed: {e}")),
                };
            }

            for input in &compute_spec.inputs {
                if let InputPayload::BlobRef(blob_id) = input {
                    if !store.blob_exists(blob_id).await {
                        return FaceAResponse::AckJob {
                            job_id,
                            accepted: false,
                            reason: Some(format!("missing input blob: {:?}", blob_id)),
                        };
                    }
                }
            }

            for artifact in &compute_spec.artifacts {
                let kind = match parse_artifact_kind(&artifact.artifact_kind) {
                    Ok(k) => k,
                    Err(e) => {
                        return FaceAResponse::AckJob {
                            job_id,
                            accepted: false,
                            reason: Some(format!("invalid artifact kind: {e}")),
                        };
                    }
                };

                let exists = match store.get_artifact(&artifact.artifact_id, kind).await {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(e) => {
                        return FaceAResponse::AckJob {
                            job_id,
                            accepted: false,
                            reason: Some(format!("artifact lookup failed: {e}")),
                        };
                    }
                };

                if !exists {
                    return FaceAResponse::AckJob {
                        job_id,
                        accepted: false,
                        reason: Some(format!(
                            "missing artifact kind={} id={}",
                            artifact.artifact_kind,
                            hex::encode(artifact.artifact_id)
                        )),
                    };
                }
            }

            let rec = JobRecord {
                job_id,
                job_type,
                payload,
                metadata,
                state: JobState::Queued,
                attempt: 0,
                last_error: None,
                result: None,
            };

            if let Err(e) = store.save_job(&rec).await {
                return FaceAResponse::AckJob {
                    job_id,
                    accepted: false,
                    reason: Some(format!("persist failed: {e}")),
                };
            }

            let required_session_inputs = required_session_input_kinds_from_spec(&compute_spec);

            if required_session_inputs.is_empty() {
                let sessions_clone = sessions.clone();

                tokio::spawn(async move {
                    if let Err(e) =
                        execute_compute_job_locally(
                            store.clone(),
                            executors.clone(),
                            sessions_clone,
                            job_id,
                        )
                        .await
                    {
                        let _ = fail_job(store.clone(), job_id, e).await;
                    }
                });
            } else {
                debug!(
                    "registered streaming compute session {}; waiting for required inputs: {:?}",
                    hex::encode(job_id),
                    required_session_inputs
                );
            }

            FaceAResponse::AckJob {
                job_id,
                accepted: true,
                reason: None,
            }
        },

        FaceARequest::UpdateJobInput {
            job_id,
            payload,
            metadata,
        } => {
            let existing = match store.load_job(&job_id).await {
                Ok(Some(rec)) => rec,
                Ok(None) => {
                    return FaceAResponse::UpdateJobInputAck {
                        job_id,
                        accepted: false,
                        reason: Some("unknown session/job_id".into()),
                    };
                }
                Err(e) => {
                    return FaceAResponse::UpdateJobInputAck {
                        job_id,
                        accepted: false,
                        reason: Some(format!("load job failed: {e}")),
                    };
                }
            };

            if !matches!(existing.job_type, JobType::FheComputing) {
                return FaceAResponse::UpdateJobInputAck {
                    job_id,
                    accepted: false,
                    reason: Some("UpdateJobInput only supports JobType::FheComputing".into()),
                };
            }

            if matches!(existing.state, JobState::InProgress) {
                return FaceAResponse::UpdateJobInputAck {
                    job_id,
                    accepted: false,
                    reason: Some("previous compute attempt still in progress".into()),
                };
            }

            if let Err(e) = validate_compute_payload_for_execution(&store, &payload).await {
                return FaceAResponse::UpdateJobInputAck {
                    job_id,
                    accepted: false,
                    reason: Some(format!("invalid updated compute payload: {e}")),
                };
            }

            if let Err(e) = store
                .update_state(&job_id, |rec| {
                    rec.payload = payload.clone();
                    rec.metadata = metadata.clone();
                    rec.state = JobState::Queued;
                    rec.result = None;
                    rec.last_error = None;
                })
                .await
            {
                return FaceAResponse::UpdateJobInputAck {
                    job_id,
                    accepted: false,
                    reason: Some(format!("failed to update session payload: {e}")),
                };
            }

            let sessions_clone = sessions.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    execute_compute_job_locally(
                        store.clone(),
                        executors.clone(),
                        sessions_clone,
                        job_id,
                    )
                    .await
                {
                    let _ = fail_job(store.clone(), job_id, e).await;
                }
            });

            FaceAResponse::UpdateJobInputAck {
                job_id,
                accepted: true,
                reason: None,
            }
        },

        FaceARequest::GetJobStatus { job_id } => match store.load_job(&job_id).await {
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

        FaceARequest::GetJobResult { job_id } => match store.load_job(&job_id).await {
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

        FaceARequest::GetArtifact { id, kind } => match store.get_artifact(&id, kind.clone()).await
        {
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

        FaceARequest::GetManifest { id } => match store.get_manifest_by_id(&id).await {
            Ok(Some(m)) => match encode(&m) {
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

        FaceARequest::GetCapabilities => FaceAResponse::Capabilities {
            info: CapabilityInfo {
                roles: vec![Gateway_NodeType::Ypolo],
                protocols: vec![
                    meshes_config::gateway_config::FACE_A_JOBS_PROTOCOL.to_string(),
                    meshes_config::gateway_config::FACE_A_BLOBS_PROTOCOL.to_string(),
                ],
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        },

        FaceARequest::ResolveArtifactAlias { alias } => {
            match store.get_alias(&alias).await {
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
            match store.get_signed_manifest(&manifest_cid).await {
                Ok(Some(m)) => match encode(&m) {
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

        FaceARequest::AckArtifactPinned { .. } => FaceAResponse::ArtifactAck {
            ok: false,
            message: Some("node_ypolo does not use artifact pin acks in MVP".into()),
        },

        FaceARequest::RegisterArtifact { id, kind, blob_id } => {
            match store.set_artifact(&id, kind, &blob_id).await {
                Ok(()) => FaceAResponse::RegisterArtifactAck {
                    ok: true,
                    message: None,
                },
                Err(e) => FaceAResponse::RegisterArtifactAck {
                    ok: false,
                    message: Some(format!("failed to register artifact: {e}")),
                },
            }
        }

        FaceARequest::DeployProgram {
            program_id,
            program_version,
            package_blob_id,
            manifest_blob_id,
            runtime_kind,
            metadata,
        } => {
            if !store.blob_exists(&package_blob_id).await {
                return FaceAResponse::DeployProgramAck {
                    accepted: false,
                    reason: Some("missing program package blob".into()),
                };
            }

            if !store.blob_exists(&manifest_blob_id).await {
                return FaceAResponse::DeployProgramAck {
                    accepted: false,
                    reason: Some("missing program manifest blob".into()),
                };
            }

            let manifest = match load_program_manifest_by_blob_id(&store, &manifest_blob_id).await {
                Ok(m) => m,
                Err(e) => {
                    return FaceAResponse::DeployProgramAck {
                        accepted: false,
                        reason: Some(format!("failed to decode program manifest: {e}")),
                    };
                }
            };

            if let Err(e) = validate_program_manifest_for_install(
                &manifest,
                &program_id,
                &program_version,
                &runtime_kind,
            ) {
                return FaceAResponse::DeployProgramAck {
                    accepted: false,
                    reason: Some(format!("program install validation failed: {e}")),
                };
            }

            if !runtime_kind_is_supported(&runtime_kind) {
                return FaceAResponse::DeployProgramAck {
                    accepted: false,
                    reason: Some(format!("program is not running on this node {:?}",
                runtime_kind)),
                };
            }

            let record = InstalledProgramRecord {
                program_id,
                program_version,
                package_blob_id,
                manifest_blob_id,
                runtime_kind,
                installed_unix_ms: now_unix_ms(),
                metadata,
            };
            
            match store.install_program(&record).await {
                Ok(()) => FaceAResponse::DeployProgramAck {
                    accepted: true,
                    reason: None,
                },
                Err(e) => FaceAResponse::DeployProgramAck {
                    accepted: false,
                    reason: Some(format!("failed to install program: {e}")),
                },
            }
        }

        FaceARequest::GetProgramInstallStatus {
            program_id,
            program_version,
        } => match store.get_installed_program(&program_id, &program_version).await {
            Ok(Some(rec)) => FaceAResponse::ProgramInstallStatus {
                program_id: rec.program_id,
                program_version: rec.program_version,
                installed: true,
                package_blob_id: Some(rec.package_blob_id),
                manifest_blob_id: Some(rec.manifest_blob_id),
                runtime_kind: Some(rec.runtime_kind),
                metadata: rec.metadata,
            },
            Ok(None) => FaceAResponse::ProgramInstallStatus {
                program_id,
                program_version,
                installed: false,
                package_blob_id: None,
                manifest_blob_id: None,
                runtime_kind: None,
                metadata: vec![],
            },
            Err(e) => FaceAResponse::Error {
                message: format!("program install status load failed: {e}"),
            },
        },

        FaceARequest::ListInstalledPrograms => match store.list_installed_programs().await {
            Ok(programs) => FaceAResponse::InstalledPrograms { programs },
            Err(e) => FaceAResponse::Error {
                message: format!("list installed programs failed: {e}"),
            },
        },

        FaceARequest::PushProgramSessionInput {
            session_id,
            input_kind,
            payload,
            observed_unix_ms,
            sequence,
        } => {
            match push_program_session_input(
                &store,
                sessions.clone(),
                session_id,
                input_kind.clone(),
                payload,
                observed_unix_ms,
                sequence,
            )
            .await
            {
                Ok(()) => {
                    let should_spawn = match maybe_prepare_session_compute(
                        &store,
                        sessions.clone(),
                        session_id,
                    )
                    .await
                    {
                        Ok(true) => true,
                        Ok(false) => false,
                        Err(e) => {
                            return FaceAResponse::Error {
                                message: format!("session computing preparation failed: {e}"),
                            };
                        }
                    };

                    if should_spawn {
                        let store_for_exec = store.clone();
                        let store_for_fail = store.clone();
                        let executors_for_exec = executors.clone();
                        let sessions_for_exec = sessions.clone();

                        tokio::spawn(async move {
                            if let Err(e) =
                                execute_compute_job_locally(
                                    store_for_exec,
                                    executors_for_exec,
                                    sessions_for_exec,
                                    session_id,
                                )
                                .await
                            {
                                let _ = fail_job(store_for_fail, session_id, e).await;
                            }
                        });
                    }

                    FaceAResponse::PushProgramSessionAck {
                        session_id,
                        input_kind,
                        payload: Payload::Inline(vec![]),
                        observed_unix_ms,
                        sequence,
                    }
                }

                Err(e) => FaceAResponse::Error {
                    message: format!("PushProgramSessionInput failed: {e}"),
                },
            }
            
        }
    }
}

async fn maybe_prepare_session_compute(
    store: &JobStore,
    sessions: ProgramSessions,
    session_id: JobId,
) -> Result<bool> {
    let rec = store
        .load_job(&session_id)
        .await
        .with_context(|| {
            format!(
                "load session job record failed for session_id={}",
                hex::encode(session_id)
            )
        })?
        .ok_or_else(|| anyhow!("unknown session_id/job_id: {}", hex::encode(session_id)))?;

    if matches!(rec.state, JobState::InProgress | JobState::Dispatching) {
        return Ok(false);
    }

    let compute_spec = decode_compute_job_spec(store, &rec.payload)
        .await
        .with_context(|| {
            format!(
                "decode ComputeJobSpec for session_id={} failed; payload={:?}",
                hex::encode(session_id),
                rec.payload
            )
        })?;

    let required = required_session_input_kinds_from_spec(&compute_spec);
    
    tracing::info!(
        target: "ypolo_session",
        session_id = %hex::encode(session_id),
        required = ?required,
        "checking session input readiness"
    );

    if required.is_empty() {
        return Ok(false);
    }

    let ready = {
        let guard = sessions.lock().await;
        let Some(state) = guard.get(&session_id) else {
            tracing::info!(
                target: "ypolo_session",
                session_id = %hex::encode(session_id),
                "no session state yet"
            );
            return Ok(false);
        };

        let present = state.inputs.keys().cloned().collect::<Vec<_>>();
        tracing::info!(
            target: "ypolo_session",
            session_id = %hex::encode(session_id),
            present = ?present,
            required = ?required,
            "session input readiness state"
        );

        required.iter().all(|kind| state.inputs.contains_key(kind))
    };

    if !ready {
        return Ok(false);
    }

    store
        .update_state(&session_id, |rec| {
            rec.state = JobState::Dispatching;
            rec.result = None;
            rec.last_error = None;
        })
        .await
        .with_context(|| {
            format!(
                "update session job state to Queued failed for session_id={}",
                hex::encode(session_id)
            )
        })?;

    Ok(true)
}

fn required_session_input_kinds_from_spec(spec: &ComputeJobSpec) -> Vec<String> {
    spec.metadata
        .iter()
        .find(|(k, _)| k == "session.required_inputs")
        .map(|(_, v)| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

async fn push_program_session_input(
    store: &JobStore,
    sessions: ProgramSessions,
    session_id: JobId,
    input_kind: String,
    payload: Payload,
    observed_unix_ms: u64,
    sequence: u64,
) -> Result<()> {
    if input_kind.trim().is_empty() {
        return Err(anyhow!("input_kind must not be empty"));
    }

    let bytes = match payload {
        Payload::Inline(bytes) => {
            if bytes.is_empty() {
                return Err(anyhow!("session input payload must not be empty"));
            }
            bytes
        }
        Payload::BlobRef(_) => {
            return Err(anyhow!(
                "session input payload must be inline for low-latency live updates"
            ));
        }
    };

    let rec = store
        .load_job(&session_id)
        .await?
        .ok_or_else(|| anyhow!("unknown session_id/job_id: {}", hex::encode(session_id)))?;

    if !matches!(rec.job_type, JobType::FheComputing) {
        return Err(anyhow!(
            "session_id does not refer to a FheComputing job: {}",
            hex::encode(session_id)
        ));
    }

    let mut guard = sessions.lock().await;
    let state = guard.entry(session_id).or_default();

    if let Some(existing) = state.inputs.get(&input_kind) {
        if sequence <= existing.sequence {
            return Ok(());
        }
    }

    state.inputs.insert(
        input_kind.clone(),
        ProgramSessionInputEntry {
            bytes,
            observed_unix_ms,
            sequence,
        },
    );

    let current_keys = state.inputs.keys().cloned().collect::<Vec<_>>();

    tracing::info!(
        target: "ypolo_session",
        session_id = %hex::encode(session_id),
        input_kind = %input_kind,
        sequence,
        observed_unix_ms,
        inputs = ?current_keys,
        "stored program session input"
    );

    Ok(())
}

async fn validate_compute_payload_for_execution(
    store: &JobStore,
    payload: &Payload,
) -> Result<()> {
    let compute_spec = decode_compute_job_spec(store, payload).await?;

    let installed = store
        .get_installed_program(&compute_spec.program_id, &compute_spec.program_version)
        .await?
        .ok_or_else(|| anyhow!("program is not installed"))?;

    let manifest = load_program_manifest_by_blob_id(store, &installed.manifest_blob_id).await?;
    validate_program_invocation(&compute_spec, &manifest)?;

    for input in &compute_spec.inputs {
        if let InputPayload::BlobRef(blob_id) = input {
            if !store.blob_exists(blob_id).await {
                return Err(anyhow!("missing input blob: {:?}", blob_id));
            }
        }
    }

    for artifact in &compute_spec.artifacts {
        let kind = parse_artifact_kind(&artifact.artifact_kind)?;
        let exists = store
            .get_artifact(&artifact.artifact_id, kind)
            .await?
            .is_some();

        if !exists {
            return Err(anyhow!(
                "missing artifact kind={} id={}",
                artifact.artifact_kind,
                hex::encode(artifact.artifact_id)
            ));
        }
    }

    Ok(())
}

async fn decode_compute_job_spec(store: &JobStore, payload: &Payload) -> Result<ComputeJobSpec> {
    let bytes = match payload {
        Payload::Inline(bytes) => bytes.clone(),
        Payload::BlobRef(blob_id) => store.read_blob(blob_id).await?,
    };

    let spec: ComputeJobSpec =
        decode(&bytes).map_err(|e| anyhow!("decode ComputeJobSpec failed: {e}"))?;
    Ok(spec)
}

async fn execute_compute_job_locally(
    store: JobStore,
    executors: Arc<HashMap<ProgramRuntimeKind, Arc<dyn ComputeExecutor>>>,
    sessions: ProgramSessions,
    session_id: JobId,
) -> Result<()> {
    let started_unix_ms = now_unix_ms();

    store
        .update_state(&session_id, |rec| {
            rec.state = JobState::InProgress;
            rec.attempt = rec.attempt.saturating_add(1);
            rec.last_error = None;
        })
        .await?;

    let rec = store
        .load_job(&session_id)
        .await?
        .ok_or_else(|| anyhow!("job disappeared after prepare"))?;

    let compute_spec = decode_compute_job_spec(&store, &rec.payload).await?;

    let installed = store
        .get_installed_program(&compute_spec.program_id, &compute_spec.program_version)
        .await?
        .ok_or_else(|| anyhow!("program is not installed"))?;

    let manifest = load_program_manifest_by_blob_id(&store, &installed.manifest_blob_id).await?;
    validate_program_invocation(&compute_spec, &manifest)?;

    let input_bytes = materialize_inputs(&store, &compute_spec.inputs).await?;

    let mut artifacts = Vec::with_capacity(compute_spec.artifacts.len());
    for artifact in &compute_spec.artifacts {
        let kind = parse_artifact_kind(&artifact.artifact_kind)?;
        let bytes = load_artifact_bytes(&store, &artifact.artifact_id, kind).await?;

        artifacts.push(ComputeArtifactInput {
            artifact_id: artifact.artifact_id,
            artifact_kind: artifact.artifact_kind.clone(),
            bytes,
        });
    }

    let artifact_blob_ids = resolve_artifact_blob_ids(&store, &compute_spec).await?;

    let executor = executors
        .get(&installed.runtime_kind)
        .cloned()
        .ok_or_else(|| anyhow!("unsupported runtime kind: {:?}", installed.runtime_kind))?;

    let session_inputs = materialize_session_inputs(&sessions, session_id).await;

    tracing::info!(
        target: "ypolo_session",
        session_id = %hex::encode(session_id),
        session_inputs = ?session_inputs.iter().map(|i| (&i.input_kind, i.sequence, i.bytes.len())).collect::<Vec<_>>(),
        "materialized session inputs for execution"
    );

    let output = executor
        .execute(
            compute_core::ComputeContext { job_id: session_id },
            ExecutorInputs {
                spec: compute_spec.clone(),
                inputs: input_bytes.clone(),
                session_inputs,
                artifacts,
            },
        )
        .await?;

    if output.outputs.is_empty() {
        return Err(anyhow!("executor returned no outputs"));
    }

    let mut output_blob_ids = Vec::with_capacity(output.outputs.len());
    for out_bytes in &output.outputs {
        let blob_id = store_result_blob(&store, out_bytes).await?;
        output_blob_ids.push(blob_id);
    }

    let finished_unix_ms = now_unix_ms();

    let receipt = build_receipt(
        session_id,
        &compute_spec,
        installed.manifest_blob_id,
        &input_bytes,
        output_blob_ids.clone(),
        artifact_blob_ids,
        started_unix_ms,
        finished_unix_ms,
    );

    let receipt_bytes = encode_receipt(&receipt)?;
    let receipt_blob_id = store_result_blob(&store, &receipt_bytes).await?;

    store
        .set_artifact(&session_id, ArtifactKind::ComputeReceipt, &receipt_blob_id)
        .await?;

    let primary_result = output_blob_ids
        .first()
        .copied()
        .ok_or_else(|| anyhow!("missing primary output blob id"))?;

    succeed_job(
        store,
        session_id,
        Some(JobResultPayload::Payload(Payload::BlobRef(primary_result))),
    )
    .await
}

async fn materialize_inputs(
    store: &JobStore,
    inputs: &[InputPayload],
) -> Result<Vec<Vec<u8>>> {
    let mut out = Vec::with_capacity(inputs.len());

    for input in inputs {
        match input {
            InputPayload::Inline(bytes) => out.push(bytes.clone()),
            InputPayload::BlobRef(blob_id) => out.push(store.read_blob(blob_id).await?),
        }
    }

    Ok(out)
}

async fn materialize_session_inputs(
    sessions: &ProgramSessions,
    job_id: JobId,
) -> Vec<ComputeSessionInput> {
    let guard = sessions.lock().await;

    let Some(state) = guard.get(&job_id) else {
        return vec![];
    };

    let mut out: Vec<ComputeSessionInput> = state
        .inputs
        .iter()
        .map(|(input_kind, entry)| ComputeSessionInput {
            input_kind: input_kind.clone(),
            bytes: entry.bytes.clone(),
            observed_unix_ms: entry.observed_unix_ms,
            sequence: entry.sequence,
        })
        .collect();

    out.sort_by(|a, b| a.input_kind.cmp(&b.input_kind));

    out
}

async fn store_result_blob(store: &JobStore, bytes: &[u8]) -> Result<[u8; 32]> {
    let blob_id = blake3_blob_id(bytes);
    if !store.blob_exists(&blob_id).await {
        store.write_blob(&blob_id, bytes).await?;
    }
    Ok(blob_id)
}

async fn succeed_job(store: JobStore, job_id: JobId, result: Option<JobResultPayload>) -> Result<()> {
    store
        .update_state(&job_id, |rec| {
            rec.state = JobState::Succeeded;
            rec.result = result;
            rec.last_error = None;
        })
        .await
}

async fn fail_job(store: JobStore, job_id: JobId, err: anyhow::Error) -> Result<()> {
    store
        .update_state(&job_id, |rec| {
            rec.state = JobState::Failed;
            rec.last_error = Some(err.to_string());
        })
        .await
}

async fn resolve_artifact_blob_ids(
    store: &JobStore,
    spec: &ComputeJobSpec,
) -> Result<Vec<[u8; 32]>> {
    let mut out = Vec::with_capacity(spec.artifacts.len());

    for artifact in &spec.artifacts {
        let kind = parse_artifact_kind(&artifact.artifact_kind)?;
        let blob_id = store
            .get_artifact(&artifact.artifact_id, kind)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "artifact blob id not found: kind={}, id={}",
                    artifact.artifact_kind,
                    hex::encode(artifact.artifact_id)
                )
            })?;
        out.push(blob_id);
    }

    Ok(out)
}

fn parse_artifact_kind(s: &str) -> Result<ArtifactKind> {
    match s {
        "VerfKey" => Ok(ArtifactKind::VerfKey),
        "VerfAddress" => Ok(ArtifactKind::VerfAddress),
        "PublicKey" => Ok(ArtifactKind::PublicKey),
        "PublicKeyMetadata" => Ok(ArtifactKind::PublicKeyMetadata),
        "ServerKey" => Ok(ArtifactKind::ServerKey),
        "Crs" | "CRS" => Ok(ArtifactKind::Crs),
        "ProgramManifest" => Ok(ArtifactKind::ProgramManifest),
        "ComputeReceipt" => Ok(ArtifactKind::ComputeReceipt),
        "EncryptedInput" => Ok(ArtifactKind::EncryptedInput),
        "EncryptedOutput" => Ok(ArtifactKind::EncryptedOutput),
        "PublicParams" => Ok(ArtifactKind::PublicParams),
        other => Err(anyhow!("unsupported artifact kind string: {other}")),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}