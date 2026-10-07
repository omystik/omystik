use anyhow::{anyhow, Result, Context};
use futures::future::BoxFuture;
use hex;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
};
use tokio::{fs, sync::RwLock};
use tonic::Request;
use prost::Message;
use mesh_gateway_wire::{CryptoJobState, CryptoOp, FaceBRequest, FaceBResponse, JobId, Payload, ArtifactKind};

// KMS API (proto types) + tonic service trait
use kms_api::kms::v1::{
    CrsGenRequest, InitiateResharingRequest, KeyGenPreprocRequest, KeyGenRequest,
    PublicDecryptionRequest, RequestId, UserDecryptionRequest,
};
use kms_api::kms_service::v1::core_service_endpoint_server::CoreServiceEndpoint;

// Your trait (Mesh B side)
use threshold_fhe::networking::p2p::crypto_gateway_service::CryptoJobExecutor;

pub trait BlobResolver: Send + Sync + 'static {
    fn resolve(&self, blob_id: &[u8; 32]) -> BoxFuture<'static, Result<Vec<u8>>>;
}

// - 6 March -
#[derive(Clone, Debug)]
struct JobTracker {
    op: CryptoOp,
    state: CryptoJobState,
    last_error: Option<String>,
}

pub trait PublicArtifactResolver: Send + Sync + 'static {
    fn resolve_artifact(
        &self,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> BoxFuture<'static, Result<Vec<u8>>>;
}
// - -

/// Executor that runs FaceB crypto jobs by calling the in-process KMS endpoint
pub struct ThresholdKmsJobExecutor<R, A> {
    kms: Arc<dyn CoreServiceEndpoint + Send + Sync>,
    blobs: Arc<R>,
    artifacts: Arc<A>, // - 6 March -
    jobs: Arc<RwLock<HashMap<JobId, JobTracker>>>,
}

impl<R, A> ThresholdKmsJobExecutor<R, A> {
    pub fn new(
        kms: Arc<dyn CoreServiceEndpoint + Send + Sync>,
        blobs: Arc<R>,
        artifacts: Arc<A>, // - 6 March -
    ) -> Self {
        Self {
            kms,
            blobs,
            artifacts,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<R, A> CryptoJobExecutor for ThresholdKmsJobExecutor<R, A>
where
    R: BlobResolver,
    A: PublicArtifactResolver, // - 6 March -
{
    fn execute(
        &self,
        _peer: libp2p::PeerId,
        req: FaceBRequest,
    ) -> BoxFuture<'static, Result<FaceBResponse>> {
        let kms = Arc::clone(&self.kms);
        let blobs = Arc::clone(&self.blobs);
        let artifacts = Arc::clone(&self.artifacts); // - 6 March -
        let jobs = Arc::clone(&self.jobs);

        Box::pin(async move {
            match req {
                FaceBRequest::DispatchCryptoJob {
                    job_id,
                    op,
                    payload,
                    metadata: _,
                } => submit_job(&*kms, &*blobs, &*jobs, job_id, op, payload).await,

                FaceBRequest::GetCryptoJobStatus { job_id } => {
                    get_status(&*kms, &*jobs, job_id).await
                }

                FaceBRequest::GetCryptoJobResult { job_id } => {
                    get_result(&*kms, &*jobs, job_id).await
                }

                FaceBRequest::GetArtifact { id, kind } => {
                    let bytes = artifacts.resolve_artifact(&id, kind.clone()).await?;
                    Ok(FaceBResponse::Artifact {
                        id,
                        kind,
                        payload: Payload::Inline(bytes),
                    })
                }
            }
        })
    }
}

// -------------------- helpers --------------------

fn map_status(s: tonic::Status) -> anyhow::Error {
    anyhow!("kms call failed: code={:?} message={}", s.code(), s.message())
}

fn to_request_id(job_id: &JobId) -> RequestId {
    RequestId {
        request_id: hex::encode(job_id),
    }
}

async fn payload_bytes<R: BlobResolver>(r: &R, p: Payload) -> Result<Vec<u8>> {
    match p {
        Payload::Inline(b) => Ok(b),
        Payload::BlobRef(id) => r.resolve(&id).await,
    }
}

async fn submit_job<K, R>(
    kms: &K,
    blobs: &R,
    jobs: &RwLock<HashMap<JobId, JobTracker>>,
    job_id: JobId,
    op: CryptoOp,
    payload: Payload,
) -> Result<FaceBResponse>
where
    K: CoreServiceEndpoint + ?Sized,
    R: BlobResolver,
{
    // Record op for deterministic GetCryptoJobResult.
    jobs.write().await.insert(
        job_id, 
        JobTracker { op: op.clone(),
            state: CryptoJobState::Accepted,
            last_error: None,
        },
    );

    let req_id = to_request_id(&job_id);
    let bytes = payload_bytes(blobs, payload).await?;

        let dispatch_res: Result<()> = match op {
        CryptoOp::KeygenPreproc => {
            let mut req = KeyGenPreprocRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.key_gen_preproc(Request::new(req)).await.map_err(map_status)?;
            Ok(())
        }

        CryptoOp::Keygen => {
            let mut req = KeyGenRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.key_gen(Request::new(req)).await.map_err(map_status)?;
            Ok(())
        }

        CryptoOp::PublicDecrypt => {
            let mut req = PublicDecryptionRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.public_decrypt(Request::new(req)).await.map_err(map_status)?;
            Ok(())
        }

        CryptoOp::UserDecrypt => {
            let mut req = UserDecryptionRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.user_decrypt(Request::new(req)).await.map_err(map_status)?;
            Ok(())
        }

        CryptoOp::CrsGen => {
            let mut req = CrsGenRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.crs_gen(Request::new(req)).await.map_err(map_status)?;
            Ok(())
        }

        CryptoOp::ReshareInit => {
            let mut req = InitiateResharingRequest::decode(bytes.as_slice())?;
            req.request_id = Some(req_id);
            kms.initiate_resharing(Request::new(req))
                .await
                .map_err(map_status)?;
            Ok(())
        }

        CryptoOp::ReshareResult => {
            return Ok(FaceBResponse::CryptoAck {
                job_id,
                accepted: false,
                reason: Some("ReshareResult is not dispatchable; use GetCryptoJobResult".into()),
            });
        }
    };

    match dispatch_res {
        Ok(()) => {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.get_mut(&job_id) {
                job.state = CryptoJobState::Running;
                job.last_error = None;
            }
        }
        Err(e) => {
            let msg = e.to_string();
            let mut guard = jobs.write().await;
            if let Some(job) = guard.get_mut(&job_id) {
                job.state = CryptoJobState::Failed;
                job.last_error = Some(msg.clone());
            }
            return Err(anyhow!(msg));
        }
    }

    Ok(FaceBResponse::CryptoAck {
        job_id,
        accepted: true,
        reason: None,
    })
}

// - 6 March -
async fn get_status<K>(
    kms: &K,
    jobs: &RwLock<HashMap<JobId, JobTracker>>,
    job_id: JobId,
) -> Result<FaceBResponse>
where
    K: CoreServiceEndpoint + ?Sized,
{
        let tracker = match jobs.read().await.get(&job_id).cloned() {
        Some(job) => job,
        None => {
            return Ok(FaceBResponse::Error {
                message: "unknown job_id".into(),
            });
        }
    };

    if matches!(tracker.state, CryptoJobState::Failed) {
        return Ok(FaceBResponse::CryptoJobStatus {
            job_id,
            state: CryptoJobState::Failed,
            progress: None,
            last_error: tracker.last_error,
        });
    }

    let op = tracker.op;

    let rid = Request::new(to_request_id(&job_id));

    let probe = match op {
        CryptoOp::KeygenPreproc => kms.get_key_gen_preproc_result(rid).await.map(|_| ()),
        CryptoOp::Keygen => kms.get_key_gen_result(rid).await.map(|_| ()),
        CryptoOp::PublicDecrypt => kms.get_public_decryption_result(rid).await.map(|_| ()),
        CryptoOp::UserDecrypt => kms.get_user_decryption_result(rid).await.map(|_| ()),
        CryptoOp::CrsGen => kms.get_crs_gen_result(rid).await.map(|_| ()),
        CryptoOp::ReshareInit | CryptoOp::ReshareResult => kms.get_resharing_result(rid).await.map(|_| ()),
    };

    match probe {
        Ok(_) => {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.get_mut(&job_id) {
                job.state = CryptoJobState::Succeeded;
                job.last_error = None;
            }

            Ok(FaceBResponse::CryptoJobStatus {
                job_id,
                state: CryptoJobState::Succeeded,
                progress: Some(100),
                last_error: None,
            })
        }
        Err(status) if status.code() == tonic::Code::Unavailable => {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.get_mut(&job_id) {
                job.state = CryptoJobState::Running;
                job.last_error = None;
            }

            Ok(FaceBResponse::CryptoJobStatus {
                job_id,
                state: CryptoJobState::Running,
                progress: None,
                last_error: None,
            })
        }
        Err(status) => {
            let msg = format!(
                "kms call failed: code={:?} message={}",
                status.code(),
                status.message()
            );

            let mut guard = jobs.write().await;
            if let Some(job) = guard.get_mut(&job_id) {
                job.state = CryptoJobState::Failed;
                job.last_error = Some(msg.clone());
            }

            Ok(FaceBResponse::CryptoJobStatus {
                job_id,
                state: CryptoJobState::Failed,
                progress: None,
                last_error: Some(msg),
            })
        }
    }
}
// - -

async fn get_result<K>(
    kms: &K,
    jobs: &RwLock<HashMap<JobId, JobTracker>>,
    job_id: JobId,
) -> Result<FaceBResponse>
where
    K: CoreServiceEndpoint + ?Sized,
{
    let tracker = jobs
        .read()
        .await
        .get(&job_id)
        .cloned()
        .ok_or_else(|| anyhow!("unknown job_id (missing tracker)"))?;

    let op = tracker.op;
    let rid = Request::new(to_request_id(&job_id));

    let bytes = match op {
        CryptoOp::KeygenPreproc => kms
            .get_key_gen_preproc_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),

        CryptoOp::Keygen => kms
            .get_key_gen_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),

        CryptoOp::PublicDecrypt => kms
            .get_public_decryption_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),

        CryptoOp::UserDecrypt => kms
            .get_user_decryption_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),

        CryptoOp::CrsGen => kms
            .get_crs_gen_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),

        CryptoOp::ReshareInit | CryptoOp::ReshareResult => kms
            .get_resharing_result(rid)
            .await
            .map_err(map_status)?
            .into_inner()
            .encode_to_vec(),
    };

    Ok(FaceBResponse::CryptoResult {
        job_id,
        result: Payload::Inline(bytes),
    })
}


pub struct FsBlobResolver {
    blobs_dir: PathBuf,
}

impl FsBlobResolver {
    pub fn new(blobs_dir: impl Into<PathBuf>) -> Self {
        Self { blobs_dir: blobs_dir.into() }
    }

    fn blob_path(&self, blob_id: &[u8; 32]) -> PathBuf {
        self.blobs_dir.join(format!("{}.bin", hex32(*blob_id)))
    }
}

impl BlobResolver for FsBlobResolver {
    fn resolve(&self, blob_id: &[u8; 32]) -> BoxFuture<'static, Result<Vec<u8>>> {
        let path = self.blob_path(blob_id);
        Box::pin(async move {
            fs::read(&path)
                .await
                .with_context(|| format!("read blob {}", path.display()))
        })
    }
}

// - 6 March -

pub struct FsPublicArtifactResolver {
    root: PathBuf,
}

impl FsPublicArtifactResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn artifact_path(&self, id: &[u8; 32], kind: ArtifactKind) -> PathBuf {
        self.root
            .join(kind.as_storage_dir())
            .join(hex32(*id))
    }
}

impl PublicArtifactResolver for FsPublicArtifactResolver {
    fn resolve_artifact(
        &self,
        id: &[u8; 32],
        kind: ArtifactKind,
    ) -> BoxFuture<'static, Result<Vec<u8>>> {
        let path = self.artifact_path(id, kind);
        Box::pin(async move {
            fs::read(&path)
                .await
                .with_context(|| format!("read artifact {}", path.display()))
        })
    }
}

// - -

/// Must match pylon_node/src/store.rs exactly.
fn hex32(id: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in id {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}