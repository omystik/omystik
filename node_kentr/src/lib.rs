/// Core Client library
///
/// This library implements most functionalities to interact with deployed KMS cores.
/// This library also includes an associated CLI.
pub mod mesh_cmd;
pub mod key_artifacts;
pub mod key_rehydration;

// ─────────────────────────────────────────────────────────────
// Re-export nodes API
// ─────────────────────────────────────────────────────────────

pub use mesh_gateway_wire::JobMetadata;

pub use libp2p::{Multiaddr, PeerId};
pub use libp2p_common::{
    manager::KomvosManager,
    node_primary::{
        KentrConfig, KentrClusterConfig,
        YpoloConfig, YpoloClusterConfig,
        KeyType,
        NodeRole, NodeType},
    key_ops::single_filename,
};

pub use content_hashing::get_keys_dir;

pub use mesh_cmd::fetch_keyset_artifacts_from_pylon;
// ─────────────────────────────────────────────────────────────
// Re-export core-client orchestration API
// ─────────────────────────────────────────────────────────────

pub use kms_core_client::{
    execute_cmd,
    setup_logging,
    CmdConfig,
};

// Optional: re-export commonly used types for convenience
pub use kms_core_client::{
    CCCommand,
    CoreClientConfig,
    KmsType,
};

pub use kms_api::{
    KeyId,
    RequestId,
};

// Logging bootstrap (kept identical to core-client)
pub fn init_logging() {
    setup_logging();
}