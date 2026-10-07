use std::{fs,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};
use anyhow::{self, Context, Result};
use libp2p::{PeerId, StreamProtocol, core::Multiaddr};
use serde::Deserialize;
use kms_threshold::execution::runtime::party::{Identity, MpcIdentity};

fn de_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<PeerId>().map_err(serde::de::Error::custom)
}

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

impl KryphosConfig {

    pub async fn load(path: impl Into<PathBuf>) -> Result<Self>{
        let path = path.into();
        let data = fs::read_to_string(&path).with_context(|| format!("failed to read .toml config file {:?}", path))?;
        let cfg: KryphosConfig = toml::from_str(&data).context("failed to parse .toml config file")?;
        Ok(cfg)
    }

    pub fn self_node(&self, node_id: u32) -> Result<&NodeConfig> {
        self.parties
            .iter()
            .find(|p| p.node_id == node_id)
            .with_context(|| format!("node_id {} not found in parties[]", node_id))
    }

    pub fn built_node_kms_identity(&self, node_id: u32) -> Result<Identity> {
               
        let node = self.self_node(node_id)?;

        let identity = Identity::new(node.mpc_host.clone(), node.mpc_port, Some(node.mpc_identity.clone()));
        
        Ok(identity)
    }

    pub fn build_topology(&mut self) -> Result<()> {
        
        let mut peer_map = HashMap::with_capacity(self.parties.len());
        let mut peer_to_mpc = HashMap::with_capacity(self.parties.len());

        self.bootstraps.clear();

        for party in &self.parties{
            // Identity (KMS view)
            let identity = self.built_node_kms_identity(party.node_id)?;
            peer_map.insert(identity, party.peer_id.clone());

            // PeerId -> MpcIdendity (transport auth)
            peer_to_mpc.insert(party.peer_id.clone(), MpcIdentity::new(&party.mpc_identity));

            // bootstrap hashmap
           if let Some(addr) = party.listen_address.as_ref() {
            self.bootstraps
                .push((addr.clone(), party.peer_id.clone()));
            }

        }
        self.peer_by_identity = Arc::new(peer_map);
        self.peer_to_mpc = Arc::new(peer_to_mpc);
        tracing::info!(
                    "peer_to_mpc {:?}",
                    self.peer_to_mpc,
                );
        Ok(())
    }

    pub fn get_kms_party_toml_path(&mut self, node_id: u32) -> Result<&PathBuf> {

        let node = self.self_node(node_id)?;

        let path_toml_party = &node.mpc_core_config_path;

        Ok(path_toml_party)
    }

    pub fn mpc_protocol(&self, node_id: u32) -> Result<StreamProtocol> {
        let node = self.self_node(node_id)?;
        // If StreamProtocol::new requires &'static str in your libp2p version:
        let proto: &'static str = Box::leak(node.mpc_protocol.clone().into_boxed_str());
        Ok(StreamProtocol::new(proto))
    }

    pub fn kryphos_gateway_bootstraps(&self, self_peer: PeerId) -> Result<Vec<Multiaddr>> {
        let mut out = Vec::new();
        for n in &self.parties {
            if n.peer_id == self_peer {
                continue;
            }
            let addr = n.gateway_listen_addr
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing gateway_listen_addr for node_id={}", n.node_id))?;

            // ensure /p2p suffix is present
            let with_p2p = if addr.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
                addr.clone()
            } else {
                addr.clone().with(libp2p::multiaddr::Protocol::P2p(n.peer_id))
            };
            out.push(with_p2p);
        }
        Ok(out)
    }

    pub fn self_gateway_listen_addr(&self, node_id: u32) -> Result<Multiaddr> {
        let node = self.self_node(node_id)?;
        node.gateway_listen_addr
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing gateway_listen_addr for self node_id={node_id}"))
    }


}