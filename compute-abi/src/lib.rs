use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type BlobId = [u8; 32];
pub type ArtifactId = [u8; 32];
pub type JobId = [u8; 32];
pub type Hash32 = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProgramId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProgramVersion(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProgramRuntimeKind {
    NativeBuiltin,
    WasmModule,
    ComputeGraphV1,
    ExternalProcessV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputPayload {
    /// Small input bytes carried inline in the invocation spec.
    Inline(Vec<u8>),

    /// Large input bytes already uploaded through Face A blob protocol.
    BlobRef(BlobId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJobSpec {
    /// Target installed smart-program.
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,

    /// Runtime invocation inputs.
    pub inputs: Vec<InputPayload>,

    /// Extra artifacts needed by the executor/program.
    pub artifacts: Vec<ArtifactBinding>,

    /// Expected number of outputs from this invocation.
    pub expected_outputs: u32,

    /// Caller-controlled metadata.
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactBinding {
    pub artifact_id: ArtifactId,
    pub artifact_kind: String,
    pub blob_id_hint: Option<BlobId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramManifest {
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,
    pub abi_version: String,
    pub executor_backend: String,
    pub required_artifacts: Vec<ProgramArtifactRequirement>,
    pub input_slots: Vec<ProgramSlot>,
    pub output_slots: Vec<ProgramSlot>,
    pub compatibility: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramArtifactRequirement {
    pub kind: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramSlot {
    pub name: String,
    pub encoding: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeReceipt {
    pub job_id: JobId,
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,

    /// The manifest of the installed program actually used.
    pub program_manifest_blob_id: BlobId,

    /// Hashes of the materialized runtime inputs.
    pub input_hashes: Vec<Hash32>,

    pub output_blob_ids: Vec<BlobId>,
    pub artifact_blob_ids: Vec<BlobId>,
    pub worker_version: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub metadata: Vec<(String, String)>,
}

/// Deployable smart-program package manifest.
///
/// The package bytes are interpreted according to `runtime_kind`.
///
/// Current production target:
/// - `ProgramRuntimeKind::ComputeGraphV1` => encoded `ComputeGraphV1Package`
///
/// Other possible runtimes:
/// - `WasmModule`
/// - `NativeBuiltin` only for local/native deployments, not generic production
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramPackageManifest {
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,
    pub manifest_blob_id: BlobId,
    pub runtime_kind: ProgramRuntimeKind,
    pub entrypoint: String,
    pub metadata: Vec<(String, String)>,
}

/// Persistent install record stored by a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProgramRecord {
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,
    pub package_blob_id: BlobId,
    pub manifest_blob_id: BlobId,
    pub runtime_kind: ProgramRuntimeKind,
    pub installed_unix_ms: u64,
    pub metadata: Vec<(String, String)>,
}

/// Deployable generic compute graph package.
///
/// This is the production-oriented runtime package for agnostic Ypolo execution.
/// Domain crates build this graph; Ypolo only interprets the generic operations.
///
/// Design rule:
/// - this struct must not contain domain-specific Rust types
/// - all program-specific meaning stays in slot names, encodings, and metadata
/// - runtime input bytes are selected through input bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphV1Package {
    pub graph_id: Hash32,
    pub program_id: ProgramId,
    pub program_version: ProgramVersion,

    /// Declares how runtime/session payloads are bound into graph variables.
    pub inputs: Vec<ComputeGraphInputBindingV1>,

    /// Declares graph outputs and their external encoding.
    pub outputs: Vec<ComputeGraphOutputBindingV1>,

    /// Ordered graph operations.
    pub ops: Vec<ComputeGraphOpV1>,

    /// Package-level metadata.
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphInputBindingV1 {
    /// Program-local symbolic slot name.
    ///
    /// Example:
    /// - "convoy_payload"
    /// - "satellite_payload"
    /// - "alert_config"
    pub slot_name: String,

    /// Source selector for the bytes.
    pub source: ComputeGraphInputSourceV1,

    /// Application/program encoding label.
    ///
    /// Example:
    /// - "osatcon.alert_math.encrypted_convoy_point.v1"
    /// - "osatcon.alert_math.encrypted_satellite_batch.v1"
    /// - "osatcon.alert_math.config.v1"
    pub encoding: String,

    /// Whether the payload contains encrypted data.
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeGraphInputSourceV1 {
    /// Use ComputeJobSpec.inputs[index].
    DirectInput {
        index: u32,
    },

    /// Use a live session input pushed through Face A.
    SessionInput {
        input_kind: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphOutputBindingV1 {
    /// Program-local symbolic output slot name.
    pub slot_name: String,

    /// Application/program output encoding label.
    pub encoding: String,

    /// Whether the output is encrypted.
    pub encrypted: bool,
}

/// Generic typed graph operations.
///
/// This enum must stay generic. Domain crates may compose these ops into
/// application-specific graphs, but Ypolo must not need to know the domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeGraphOpV1 {
    AnySquaredDistanceLeRadiusI64 {
        point_x: String,
        point_y: String,
        candidate_batch: String,
        radius_sq_clear: String,
        max_scan_clear: String,
        output: String,
    },
    BindInputSlotBytes {
        input_slot: String,
        output_var: String,
    },
    /// Decode a bc2wrap/mesh_gateway_wire encoded struct field into graph variables.
    ///
    /// This is intentionally still generic: it names the wire encoding and fields,
    /// but it does not reference a Rust type.
    DecodeStructFields {
        input_slot: String,
        encoding: String,
        fields: Vec<ComputeGraphDecodedFieldV1>,
    },

    /// Deserialize an encrypted i64 ciphertext from bytes into a graph value.
    DeserializeFheInt64 {
        input_var: String,
        output_var: String,
    },

    /// Deserialize an encrypted boolean ciphertext from bytes into a graph value.
    DeserializeFheBool {
        input_var: String,
        output_var: String,
    },

    /// Serialize an encrypted boolean graph value to bytes.
    SerializeFheBool {
        input_var: String,
        output_var: String,
    },

    /// Encrypted signed i64 subtraction.
    SubFheInt64 {
        lhs: String,
        rhs: String,
        output: String,
    },

    /// Encrypted signed i64 multiplication.
    MulFheInt64 {
        lhs: String,
        rhs: String,
        output: String,
    },

    /// Encrypted signed i64 addition.
    AddFheInt64 {
        lhs: String,
        rhs: String,
        output: String,
    },

    /// Compare encrypted signed i64 value with clear signed i64 value.
    LeFheInt64Clear {
        lhs: String,
        rhs_clear: String,
        output: String,
    },

    /// Boolean OR over encrypted booleans.
    OrFheBool {
        lhs: String,
        rhs: String,
        output: String,
    },

    /// Create a trivial encrypted boolean constant.
    TrivialFheBool {
        value: bool,
        output: String,
    },

    /// Encode output bytes into the program output envelope.
    EncodeStructFields {
        output_slot: String,
        encoding: String,
        fields: Vec<ComputeGraphEncodedFieldV1>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphDecodedFieldV1 {
    /// Field name inside the encoded input structure.
    pub field_name: String,

    /// Graph variable that receives this field.
    pub output_var: String,

    /// Field encoding/type.
    pub field_kind: ComputeGraphFieldKindV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphEncodedFieldV1 {
    /// Field name inside the encoded output structure.
    pub field_name: String,

    /// Graph variable used as the field value.
    pub input_var: String,

    /// Field encoding/type.
    pub field_kind: ComputeGraphFieldKindV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeGraphFieldKindV1 {
    Bytes,
    I64,
    U64,
    Usize,
    String,
    MetadataVec,
}

/// Generic graph payload pushed as session input bytes.
///
/// This prevents Ypolo from needing to know application structs such as
/// `EncryptedConvoyPointV1` or `EncryptedSatelliteBatchV1`.
///
/// Domain crates build these payloads. Ypolo only decodes this generic envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphPayloadV1 {
    pub encoding: String,
    pub fields: Vec<ComputeGraphPayloadFieldV1>,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeGraphPayloadFieldV1 {
    pub name: String,
    pub value: ComputeGraphPayloadValueV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeGraphPayloadValueV1 {
    Bytes(Vec<u8>),
    BytesVec(Vec<Vec<u8>>),
    I64(i64),
    U64(u64),
    Usize(usize),
    String(String),
    MetadataVec(Vec<(String, String)>),
}

/// Package bytes for ProgramRuntimeKind::ExternalProcessV1.
///
/// Ypolo decodes this package, then launches `command` with:
/// - --request <path>
/// - --response <path>
///
/// The command owns all program-specific logic and dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessPackageV1 {
    pub command: PathBuf,
    pub env: Vec<(String, String)>,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessInvocationV1 {
    pub job_id: [u8; 32],
    pub program_id: String,
    pub program_version: String,
    pub server_key_path: PathBuf,
    pub inputs: Vec<Vec<u8>>,
    pub session_inputs: Vec<ExternalProcessSessionInputV1>,
    pub artifacts: Vec<ExternalProcessArtifactInputV1>,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessSessionInputV1 {
    pub input_kind: String,
    pub bytes: Vec<u8>,
    pub observed_unix_ms: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessArtifactInputV1 {
    pub artifact_id: [u8; 32],
    pub artifact_kind: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessResponseV1 {
    pub outputs: Vec<Vec<u8>>,
    pub metadata: Vec<(String, String)>,
}