use libp2p::{Multiaddr, PeerId};

pub use kms_api::{KeyId, RequestId};
pub use kms_core_client::FheType;

/// A discovered Pylon candidate in Mesh A.
///
/// This is intentionally lightweight. We only track:
/// - peer identity
/// - currently known addresses
/// - last-seen timestamp
///
/// More advanced scoring (RTT, failures, health) can be added later.
#[derive(Debug, Clone)]
pub struct PylonPeer {
    pub peer_id: PeerId,
    pub addrs: Vec<Multiaddr>,
    pub last_seen_unix_ms: u64,
}