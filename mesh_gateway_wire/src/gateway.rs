use compute_abi::{ProgramId, ProgramRuntimeKind, ProgramVersion};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

fn de_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<PeerId>().map_err(serde::de::Error::custom)
}

fn ser_peer_id<S>(peer: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&peer.to_string())
}

// FACE A

/// JobId used for idempotency (caller-chosen; should be unique).
pub type JobId = [u8; 32];

/// BlobId is a content-addressed identifier controlled by the gateway path.
pub type BlobId = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    // Mesh B - mpc threshold
    VerfKey,
    VerfAddress,
    PublicKey,
    PublicKeyMetadata,
    ServerKey,
    Crs,

    // Mesh C - computing
    ProgramManifest,
    ComputeReceipt,
    EncryptedInput,
    EncryptedOutput,
    PublicParams,
}

impl ArtifactKind {
    pub fn as_storage_dir(&self) -> &'static str {
        match self {
            // Mesh B - mpc threshold
            ArtifactKind::VerfKey => "VerfKey",
            ArtifactKind::VerfAddress => "VerfAddress",
            ArtifactKind::PublicKey => "PublicKey",
            ArtifactKind::PublicKeyMetadata => "PublicKeyMetadata",
            ArtifactKind::ServerKey => "ServerKey",
            ArtifactKind::Crs => "CRS",

            // Mesh C - computing
            ArtifactKind::ProgramManifest => "ProgramManifest",
            ArtifactKind::ComputeReceipt => "ComputeReceipt",
            ArtifactKind::EncryptedInput => "EncryptedInput",
            ArtifactKind::EncryptedOutput => "EncryptedOutput",
            ArtifactKind::PublicParams => "PublicParams",
        }
    }
}

/// Serialized and stored by the gateway as the final result of a job.
/// `proto_results` are per-party protobuf result messages, encoded with prost,
/// in deterministic party order.
///
/// Kentr decodes these using the original kms_api protobuf structs and then
/// reuses the existing verification / reconstruction logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyProtoResult {
    #[serde(
        serialize_with = "ser_peer_id",
        deserialize_with = "de_peer_id"
    )]
    pub peer: PeerId,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedJobResult {
    pub job_id: JobId,
    pub job_type: JobType,
    pub proto_results: Vec<PartyProtoResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifestEntry {
    pub kind: ArtifactKind,
    pub blob_id: BlobId,
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct ArtifactManifest {
//     pub id: [u8; 32], // key_id, crs_id, or signing_key_id
//     pub entries: Vec<ArtifactManifestEntry>,
//     pub metadata: Vec<(String, String)>,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    /// Small payload directly inline.
    Inline(Vec<u8>),

    /// Large payload referenced by BlobId. The blob must be uploaded separately
    /// via Face A blob protocol.
    BlobRef(BlobId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobResultPayload {
    /// Legacy / generic result bytes, usually aggregated protobuf output.
    Payload(Payload),

    /// Published public artifact-set manifest CID.
    PublishedManifest {
        manifest_cid: BlobId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobType {
    // Mesh B - mpc threshold
    KeygenPreproc,
    Keygen,
    PublicDecrypt,
    UserDecrypt,
    CrsGen,
    ReshareInit,
    ReshareResult,

    // Mesh C - computing
    FheComputing,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobMetadata {
    pub created_unix_ms: u64,
    pub client_hint: Option<String>,
}

impl JobMetadata {
    pub fn to_wire(&self) -> Vec<(String, String)> {
        let mut v = vec![("created_unix_ms".into(), self.created_unix_ms.to_string())];
        if let Some(h) = &self.client_hint {
            v.push(("client_hint".into(), h.clone()));
        }
        v
    }

    pub fn from_wire(w: &[(String, String)]) -> Self {
        let mut out = JobMetadata::default();
        for (k, v) in w {
            match k.as_str() {
                "created_unix_ms" => out.created_unix_ms = v.parse().unwrap_or(0),
                "client_hint" => out.client_hint = Some(v.clone()),
                _ => {}
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobState {
    Accepted,
    Queued,
    Dispatching,
    InProgress,
    Succeeded,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaceARequest {
    SubmitJob {
        job_id: JobId,
        job_type: JobType,
        payload: Payload,
        metadata: JobMetadata,
    },
    UpdateJobInput {
        job_id: JobId,
        payload: Payload,
        metadata: JobMetadata,
    },
    GetJobStatus {
        job_id: JobId,
    },
    GetJobResult {
        job_id: JobId,
    },

    // Current artifact API
    GetArtifact {
        id: [u8; 32],
        kind: ArtifactKind,
    },
    GetManifest {
        id: [u8; 32],
    },

    // V2 artifact discovery API
    ResolveArtifactAlias {
        alias: ArtifactAlias,
    },
    GetManifestByCid {
        manifest_cid: BlobId,
    },

    GetCapabilities,

    AckArtifactPinned {
        key_id: [u8; 32],
        blob_id: BlobId,
        #[serde(
            serialize_with = "ser_peer_id",
            deserialize_with = "de_peer_id"
        )]
        provider: PeerId,
    },

    RegisterArtifact {
        id: [u8; 32],
        kind: ArtifactKind,
        blob_id: BlobId,
    },

    DeployProgram {
        program_id: ProgramId,
        program_version: ProgramVersion,
        package_blob_id: BlobId,
        manifest_blob_id: BlobId,
        runtime_kind: ProgramRuntimeKind,
        metadata: Vec<(String, String)>,
    },

    GetProgramInstallStatus {
        program_id: ProgramId,
        program_version: ProgramVersion,
    },

    ListInstalledPrograms,

    PushProgramSessionInput {
        session_id: JobId,
        input_kind: String,
        payload: Payload,
        observed_unix_ms: u64,
        sequence: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaceAResponse {
    AckJob {
        job_id: JobId,
        accepted: bool,
        reason: Option<String>,
    },
    UpdateJobInputAck {
        job_id: JobId,
        accepted: bool,
        reason: Option<String>,
    },
    JobStatus {
        job_id: JobId,
        state: JobState,
        attempt: u32,
        last_error: Option<String>,
    },
    JobResult {
        job_id: JobId,
        /// Usually BlobRef for large results; Inline for small.
        result: Option<JobResultPayload>,
    },

    // Current artifact API
    Artifact {
        id: [u8; 32],
        kind: ArtifactKind,
        payload: Payload,
    },
    Manifest {
        id: [u8; 32],
        payload: Payload, // Inline(encoded manifest) or BlobRef
    },

    // V2 artifact discovery API
    ResolvedAlias {
        alias: ArtifactAlias,
        manifest_cid: BlobId,
    },
    ManifestV2 {
        manifest_cid: BlobId,
        payload: Payload, // Inline(encoded SignedManifest) or BlobRef
    },

    Capabilities {
        info: CapabilityInfo,
    },

    ArtifactAck {
        ok: bool,
        message: Option<String>,
    },

    RegisterArtifactAck {
        ok: bool,
        message: Option<String>,
    },

    DeployProgramAck {
        accepted: bool,
        reason: Option<String>,
    },

    ProgramInstallStatus {
        program_id: ProgramId,
        program_version: ProgramVersion,
        installed: bool,
        package_blob_id: Option<BlobId>,
        manifest_blob_id: Option<BlobId>,
        runtime_kind: Option<ProgramRuntimeKind>,
        metadata: Vec<(String, String)>,
    },

    InstalledPrograms {
        programs: Vec<compute_abi::InstalledProgramRecord>,
    },

    Error {
        message: String,
    },

    PushProgramSessionAck {
        session_id: JobId,
        input_kind: String,
        payload: Payload,
        observed_unix_ms: u64,
        sequence: u64,
    },
}

/// Face A blob protocol control messages (sent as a small framed header).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaceABlobHeader {
    Put {
        blob_id: BlobId,
        size: u64,
    },
    Get {
        blob_id: BlobId,
    },
    Ack {
        ok: bool,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeygenResult {
    pub key_id: [u8; 32], // stable identifier for pubkey material
    pub public_key: Vec<u8>, // raw bytes (or a serialized TFHE public key)
    pub metadata: Vec<(String, String)>,
}

// FACE B

/// Crypto operation requested from Mesh B service.
/// Keep this small and stable; the bytes payload can contain operation-specific formats
/// (e.g. bc2wrap(bincode(prost message))).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoOp {
    KeygenPreproc,
    Keygen,
    PublicDecrypt,
    UserDecrypt,
    CrsGen,
    ReshareInit,
    ReshareResult,
}

/// Gateway -> Mesh B (service) request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaceBRequest {
    /// Single-shot RPC-style call:
    /// - accepted/rejected is immediate
    /// - result can be returned immediately (simple ops) or later (long ops)
    DispatchCryptoJob {
        job_id: JobId,
        op: CryptoOp,
        payload: Payload,
        /// Optional opaque metadata (caller-controlled).
        /// Useful for routing/versioning without hardcoding fields everywhere.
        metadata: Vec<(String, String)>,
    },

    /// Query job status.
    GetCryptoJobStatus {
        job_id: JobId,
    },

    /// Fetch job result.
    GetCryptoJobResult {
        job_id: JobId,
    },

    GetArtifact {
        id: [u8; 32],
        kind: ArtifactKind,
    },
}

/// Mesh B -> Gateway responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaceBResponse {
    /// Immediate ack to DispatchCryptoJob.
    CryptoAck {
        job_id: JobId,
        accepted: bool,
        reason: Option<String>,
    },

    /// Status response.
    CryptoJobStatus {
        job_id: JobId,
        state: CryptoJobState,
        progress: Option<u8>, // 0..=100
        last_error: Option<String>,
    },

    /// Result response.
    CryptoResult {
        job_id: JobId,
        result: Payload,
    },

    Artifact {
        id: [u8; 32],
        kind: ArtifactKind,
        payload: Payload,
    },

    Error {
        message: String,
    },
}

/// Mesh B internal job lifecycle (visible to gateway).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoJobState {
    Accepted,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedArtifact {
    pub key_id: [u8; 32],
    pub blob_id: BlobId,
    pub sig: Vec<u8>, // signature over blob_id (or over (key_id||blob_id))
    pub signer: Vec<u8>, // signer public key bytes (ed25519)
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Autonomos,
    Kentr,
    KryphosPylon,
    Pylon,
    Syndesmos,
    Ypolo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub roles: Vec<NodeType>,
    pub protocols: Vec<String>,
    pub version: String,
}

// V2 artifact discovery / publication types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactAlias {
    pub namespace: String,
    pub logical_name: String,
    pub version_selector: VersionSelector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionSelector {
    Latest,
    Exact(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub id: [u8; 32],
    pub version: u64,
    pub entries: Vec<ArtifactEntry>,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub kind: ArtifactKind,
    pub cid: BlobId,
    pub byte_len: u64,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest: ArtifactManifest,
    pub signatures: Vec<ArtifactSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSignature {
    pub signer_key: Vec<u8>,
    pub algorithm: String,
    pub sig: Vec<u8>,
}