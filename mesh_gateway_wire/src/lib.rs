// Shared wire types and codec utilities for the Mesh Gateway.
//
// This crate is intentionally small:
// - Defines the gateway request/response messages for Face A (Mesh A <-> Gateway)
//   and Face B (Gateway <-> Mesh B crypto service endpoint).
// - Provides bc2wrap-backed encode/decode helpers.
// - Exposes protocol constants.
//
// Serialization policy:
// - All messages are `serde` + `bc2wrap` serialized.
// - Over libp2p request_response we send/receive raw bytes (Vec<u8>), and decode
//   these bytes into the enums defined in `gateway`.

mod codec;
mod gateway;
mod store;
mod utils;

pub use crate::codec::{decode, encode};
pub use crate::gateway::{
    AggregatedJobResult, ArtifactAlias, ArtifactEntry, ArtifactKind, ArtifactManifest, ArtifactManifestEntry,
    ArtifactSignature, JobResultPayload,
    BlobId, CapabilityInfo, FaceABlobHeader, FaceARequest, FaceAResponse, JobId, JobMetadata,
    JobState, JobType, Payload,  CryptoJobState, CryptoOp, FaceBRequest, FaceBResponse, NodeType,
    PartyProtoResult, SignedArtifact, SignedManifest, VersionSelector,
};
pub use crate::store::{JobRecord, JobStore};
pub use crate::utils::{blake3_blob_id, verify_signed_artifact};

pub use compute_abi::{
    ComputeJobSpec, ComputeReceipt, InstalledProgramRecord, ProgramId, ProgramManifest,
    ProgramPackageManifest, ProgramVersion,
};
