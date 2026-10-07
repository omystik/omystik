use libp2p::{Multiaddr, PeerId};
use std::{
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};
use serde::{Serialize, Deserialize};

fn de_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<PeerId>().map_err(serde::de::Error::custom)
}

// ######### COPY from "../kms/core/threshold/src/execution/runtime/party.rs"
/// The identity of a MPC party.
///
/// When TLS is used, this must be the subject CN in the x509 certificate.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct MpcIdentity(pub(crate) String);

impl std::fmt::Display for MpcIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// For ØMYSTIK
impl MpcIdentity {
    pub fn new(s: impl Into<String>) -> Self {
        MpcIdentity(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Runtime identity of party.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Identity {
    hostname: String,
    port: u16,
    mpc_identity: Option<MpcIdentity>,
}

impl std::fmt::Display for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.hostname, self.port)
    }
}

impl Identity {
    /// Create a new Identity with the given hostname and port.
    pub fn new(hostname: String, port: u16, mpc_identity: Option<String>) -> Self {
        Identity {
            hostname,
            port,
            mpc_identity: mpc_identity.map(MpcIdentity),
        }
    }

    /// Get the hostname part of the identity.
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    /// Get the port part of the identity.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the MPC identity part of the identity, defaults to hostname if not set
    pub fn mpc_identity(&self) -> MpcIdentity {
        self.mpc_identity
            .clone()
            .unwrap_or(MpcIdentity(format!("{}:{}", &self.hostname, self.port)))
    }
}
// #########


// One party in the FHE mesh (including ourselves).
#[derive(Debug, Deserialize, Clone)]
pub struct NodeConfig{
    // Party id as used by KMS / MPC (e.g. 1..=5).
    pub node_id: u32,
    // mpc_host for Kms Impl.
    pub mpc_host: String,
    // mpc_port for Kms Impl.
    pub mpc_port: u16,
    // mpc_identity for Kms Impl.
    pub mpc_identity: String,
    // Secret seed to create libp2p node peer_id
    pub secret_seed: u8,
    #[serde(deserialize_with = "de_peer_id")]
    // Libp2p PeerId in base58 form.
    pub peer_id: PeerId,
    // One party in the FHE mesh (including ourselves).
    pub listen_address: Option<Multiaddr>,
    // party side of node
    pub mpc_protocol: String,
    // path_to_KMS/.../config/default_X.toml per party
    pub mpc_core_config_path: PathBuf,
    //  path_to_public_material/../keys per party
    pub mpc_public_storage: PathBuf,
    // pylon of kryphos
    pub gateway_listen_addr: Option<Multiaddr>,
    // gateway side of node
    pub gateway_protocol: String,
    // storage gateway
    pub gateway_blob: PathBuf,   
}
// Per-context access policy: which parties are allowed to serve that context.
// #[derive(Debug, Deserialize, Clone)]
// pub struct ContextAccess{
//     // The context id hex string as used in `RequestId.request_id`.
//     pub context_id_hex: String,
//     // Party ids that are allowed to handle this context.
//     pub allowed_nodes: Vec<u32>,
// }

// Configuration for one node_fhe instance (one MPC party).
#[derive(Debug, Deserialize, Clone)]
pub struct KryphosConfig{
    #[serde(rename = "node")]
    // List of all parties in the FHE mesh (including ourselves).
    pub parties: Vec<NodeConfig>,
    // Per-context access policies.
    // #[serde(rename = "contexts")]
    // pub contexts: Vec<ContextAccess>,
    #[serde(skip, default)]
    // HashMap Identity Peer Id for KMS Impl
    pub peer_by_identity: Arc<HashMap<Identity, PeerId>>,
    #[serde(skip, default)]
    pub peer_to_mpc: Arc<HashMap<PeerId, MpcIdentity>>,
    #[serde(skip, default)]
    // list of bootstrap kryphos nodes
    pub bootstraps: Vec<(Multiaddr, PeerId)>,
    // Specific to [feature = "insecure"] Kms-server like
    pub mock_enclave: Option<bool>,
}
