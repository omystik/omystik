use serde::{Deserialize, Deserializer};
use std::{path::PathBuf, str::FromStr};
use libp2p::{Multiaddr, PeerId};

/// Face A (Mesh A <-> Gateway) job-control protocol identifier.
pub const FACE_A_JOBS_PROTOCOL: &str = "/mesh/gateway/jobs/1";

/// Face A blob transfer protocol identifier (Mesh A <-> Gateway).
pub const FACE_A_BLOBS_PROTOCOL: &str = "/mesh/gateway/blob/1";

/// Face B (Gateway <-> Mesh B) crypto-service protocol identifier.
pub const FACE_B_CRYPTO_PROTOCOL: &str = "/mesh/gateway/crypto/1";

/// MESH A namespace
pub const NAMESPACE_MESH_A: &str = "/mesh-a";

fn de_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<PeerId>().map_err(serde::de::Error::custom)
}

fn de_peerid_vec<'de, D>(d: D) -> Result<Vec<PeerId>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Vec::<String>::deserialize(d)?;
    v.into_iter()
        .map(|s| s.parse::<PeerId>().map_err(serde::de::Error::custom))
        .collect()
}

fn default_rendezvous_bootstraps_de() -> Vec<RendezvousABootstrap> {
    default_rendezvous_bootstraps()
        .into_iter()
        .map(|r| RendezvousABootstrap { peer_id: r.peer_id, addr: r.addr })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct RendezvousABootstrap {
    #[serde(deserialize_with = "de_peer_id")]
    pub peer_id: PeerId,
    pub addr: Multiaddr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayClusterConfig {
    #[serde(rename = "gateway")]
    pub gateway_cluster: Vec<GatewayConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    // gateway id pair of (face_a-face_b)
    pub gateway_id: u32,
    pub face_a: FaceAConfig,
    pub face_b: FaceBConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FaceAConfig { // face a server
    /// Optional seed for deterministic PeerId.
    pub secret_seed: Option<u8>,
    #[serde(deserialize_with = "de_peer_id")]
    // Libp2p PeerId in base58 form.
    pub peer_id: PeerId,
    pub listen_address: Multiaddr,   
    #[serde(default, deserialize_with = "de_peerid_vec")]
    pub allow_peers: Vec<PeerId>,
    #[serde(default)]
    pub data_dir: PathBuf,
    #[serde(default= "default_blob_protocol")]
    pub protocol: String,
    #[serde(default= "default_namespace_job")]
    pub namespace: String,
    #[serde(default = "default_rendezvous_bootstraps_de")]
    pub rendezvous: Vec<RendezvousABootstrap>,
}

fn default_blob_protocol() -> String {
    FACE_A_BLOBS_PROTOCOL.to_string()
}

fn default_namespace_job() -> String {
    NAMESPACE_MESH_A.to_string()
}

pub fn default_rendezvous_bootstraps() -> Vec<RendezvousABootstrap> {
    vec![
        RendezvousABootstrap {
            addr: Multiaddr::from_str("/ip4/127.0.0.1/udp/9008/quic-v1").expect("FIX_ADDR_1"), // local mac 192.168.1.42, Pi zero 2: 1 192.168.1.125
            peer_id: PeerId::from_str("12D3KooWLoUkbxkEzB7GNZVrAWKHLKPKQ9HLvEdkZwBieQuCcuhZ")
                .expect("PEER_ID_1"),
        },
        RendezvousABootstrap {
            addr: Multiaddr::from_str("/ip4/127.0.0.1/udp/9002/quic-v1").expect("FIX_ADDR_2"), // local mac 192.168.1.42, Pi zero 2: 1 192.168.1.125
            peer_id: PeerId::from_str("12D3KooWMzg8FK5YW9SSov78gYudoeJxsWy4GbjMg7fA6JKv2X8T")
                .expect("PEER_ID_2"),
        },
    ]
}

// ------ Face B ------

#[derive(Debug, Clone, Deserialize)]
pub struct FaceBConfig { // face b client
    /// optional deterministic PeerId for the gateway's FaceB *client* swarm
    pub secret_seed: Option<u8>,
    #[serde(deserialize_with = "de_peer_id")]
    // Libp2p PeerId in base58 form.
    pub peer_id: PeerId,
    /// local listen for the FaceB *client* swarm (can be /udp/0/quic-v1)
    pub listen_address: Option<Multiaddr>,
    /// remote Mesh-B FaceB service endpoints (must include /p2p/<peerId>)
    #[serde(default)]
    pub bootstraps: Vec<Multiaddr>,
    #[serde(default= "default_crypto_protocol")]
    pub protocol: String,
    // pub request_timeout_ms: u64,
    // pub retry_initial_ms: u64,
    // pub retry_max_ms: u64,
    // pub retry_max_attempts: u32,
    // - 6 March -
    pub num_majority: usize,
    pub num_reconstruct: usize,
    pub expect_all_responses: bool,
    // - -
}

fn default_crypto_protocol() -> String {
    FACE_B_CRYPTO_PROTOCOL.to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerAddr {
    #[serde(deserialize_with = "de_peer_id")]
    pub peer_id: PeerId,
    pub addr: Multiaddr,
}