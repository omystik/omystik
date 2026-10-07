use serde::Deserialize;
use std::fmt;
use libp2p::{Multiaddr, PeerId};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Syndesmos,
    Autonomos,
    Kentr,
    Ypolo,
}

impl Default for NodeType {
    fn default() -> Self {
        Self::Autonomos
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Admin,
    NonAdmin,
}

impl Default for NodeRole {
    fn default() -> Self {
        Self::NonAdmin
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    PublicKey,
    PublicKeyMetadata,
    ServerKey,
}

impl Default for KeyType {
    fn default() -> Self {
        Self::PublicKey
    }
}

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self{
            Self::PublicKey => "PublicKey",
            Self::PublicKeyMetadata => "PublicKeyMetadata",
            Self::ServerKey => "ServerKey",
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/*
 _____           _                  _ 
|  __ \         | |                | |
| |__) | __ ___ | |_ ___   ___ ___ | |
|  ___/ '__/ _ \| __/ _ \ / __/ _ \| |
| |   | | | (_) | || (_) | (_| (_) | |
|_|   |_|  \___/ \__\___/ \___\___/|_|
*/
/// MESH A
/// file exchange
pub const FILE_EX_PROTOCOL: &str = "/file-exchange/1";

/// key
pub const KEY_RAW_PROTOCOL: &str = "/key/1";

/// namespace
pub const NAMESPACE_MESH_A: &str = "/mesh-a";


/*
   _____                 _                               
  / ____|               | |                              
 | (___  _   _ _ __   __| | ___  ___ _ __ ___   ___  ___ 
  \___ \| | | | '_ \ / _` |/ _ \/ __| '_ ` _ \ / _ \/ __|
  ____) | |_| | | | | (_| |  __/\__ \ | | | | | (_) \__ \
 |_____/ \__, |_| |_|\__,_|\___||___/_| |_| |_|\___/|___/
          __/ |                                          
         |___/                                           
*/
fn de_peer_id<'de, D>(deserializer: D) -> Result<Option<PeerId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Deserialize as Option<String> so missing/null works automatically
    let opt = Option::<String>::deserialize(deserializer)?;

    match opt {
        None => Ok(None),
        Some(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            s.parse::<PeerId>()
                .map(Some)
                .map_err(serde::de::Error::custom)
        }
    }
}

fn default_namespace() -> Option<String> {
    Some(NAMESPACE_MESH_A.to_string())
}

#[derive(Debug, Deserialize)]
pub struct SyndesmosClusterConfig {
    pub syndesmos: Vec<AutonomosConfig>,
}

/// Struct to hold node configuration
#[derive(Debug, Deserialize)]
pub struct SyndesmosConfig {
    pub syndesmos_id: u32,
    pub secret_seed: Option<u8>,
    pub listen_addr: Option<Multiaddr>,
    pub fix_addr: Option<Multiaddr>,
    #[serde(default, deserialize_with = "de_peer_id")]
    pub peer_id: Option<PeerId>,
    #[serde(default= "default_namespace")]
    pub namespace: Option<String>,
}

/*
                _                                        
     /\        | |                                       
    /  \  _   _| |_ ___  _ __   ___  _ __ ___   ___  ___ 
   / /\ \| | | | __/ _ \| '_ \ / _ \| '_ ` _ \ / _ \/ __|
  / ____ \ |_| | || (_) | | | | (_) | | | | | | (_) \__ \
 /_/    \_\__,_|\__\___/|_| |_|\___/|_| |_| |_|\___/|___/
                                                         
                                                         
*/
// Struct to hold node configuration

#[derive(Debug, Deserialize)]
pub struct AutonomosClusterConfig {
    pub autonomos: Vec<AutonomosConfig>,
}

#[derive(Debug, Deserialize)]
pub struct AutonomosConfig {
    pub autonomos_id: u32,
    pub secret_seed: Option<u8>,
    pub listen_addr: Option<Multiaddr>,
    pub fix_addr: Option<Multiaddr>,
    #[serde(default, deserialize_with = "de_peer_id")]
    pub peer_id: Option<PeerId>,
    #[serde(default, deserialize_with = "de_peer_id")]
    pub syndesmos_peer: Option<PeerId>,
    pub syndesmos_addr: Option<Multiaddr>,
    #[serde(default= "default_namespace")]
    pub namespace: Option<String>,
}

/*
   _____                  _____ _ _            _   
  / ____|                / ____| (_)          | |  
 | |     ___  _ __ ___  | |    | |_  ___ _ __ | |_ 
 | |    / _ \| '__/ _ \ | |    | | |/ _ \ '_ \| __|
 | |___| (_) | | |  __/ | |____| | |  __/ | | | |_ 
  \_____\___/|_|  \___|  \_____|_|_|\___|_| |_|\__|
                                                   
*/
#[derive(Debug, Deserialize)]
pub struct KentrClusterConfig {
    pub kentr: Vec<KentrConfig>,
}

#[derive(Debug, Deserialize)]
pub struct KentrConfig {
    pub kentr_id: u32,
    pub secret_seed: u8,
    pub listen_address: Multiaddr,
    pub node_type: Option<NodeType>,
    pub node_role: Option<NodeRole>,
    pub namespace: String,
    pub syndesmos_peer_id_1: String,
    pub syndesmos_address: String,
}

/*
 __     __          _       
 \ \   / /         | |      
  \ \_/ / __   ___ | | ___  
   \   / '_ \ / _ \| |/ _ \ 
    | || |_) | (_) | | (_) |
    |_|| .__/ \___/|_|\___/ 
       | |                  
       |_|                  
*/
#[derive(Debug, Deserialize)]
pub struct YpoloClusterConfig {
    pub ypolo: Vec<YpoloConfig>,
}

#[derive(Debug, Deserialize)]
pub struct YpoloConfig {
    pub ypolo_id: u32,
    pub peer_id: String,
    pub listen_address: Multiaddr,
}


// fn default_rendezvous_bootstraps_de() -> Vec<RendezvousABootstrap> {
//     default_rendezvous_bootstraps()
//         .into_iter()
//         .map(|r| RendezvousABootstrap { peer_id: r.peer_id, addr: r.addr })
//         .collect()
// }

// #[derive(Debug, Clone, Deserialize)]
// pub struct RendezvousABootstrap {
//     #[serde(deserialize_with = "de_peer_id")]
//     pub peer_id: Option<PeerId>,
//     pub addr: Multiaddr,
// }



// pub fn default_rendezvous_bootstraps() -> Vec<RendezvousABootstrap> {
//     vec![
//         RendezvousABootstrap {
//             addr: Multiaddr::from_str("/ip4/192.168.1.42/udp/9008/quic-v1").expect("FIX_ADDR_1"),
//             peer_id: Some(PeerId::from_str("12D3KooWLoUkbxkEzB7GNZVrAWKHLKPKQ9HLvEdkZwBieQuCcuhZ")
//                 .expect("PEER_ID_1")),
//         },
//         RendezvousABootstrap {
//             addr: Multiaddr::from_str("/ip4/192.168.1.42/udp/9002/quic-v1").expect("FIX_ADDR_2"),
//             peer_id: Some(PeerId::from_str("12D3KooWMzg8FK5YW9SSov78gYudoeJxsWy4GbjMg7fA6JKv2X8T")
//                 .expect("PEER_ID_2")),
//         },
//     ]
// }