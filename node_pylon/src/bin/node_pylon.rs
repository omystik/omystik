use anyhow::{anyhow, Context, Result};
use clap::Parser;
use libp2p::{Multiaddr, PeerId};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use meshes_config::gateway_config::{GatewayClusterConfig, GatewayConfig};
use node_pylon::kryphos_config_copy::KryphosConfig;
use node_pylon::GatewayManager;


#[derive(Parser, Debug)]
#[clap(name = "node-gateway")]
pub struct Args {
    /// Face A gateway cluster (Mesh A <-> Gateway)
    #[clap(long)]
    pub gateway_cluster: PathBuf,

    /// MPC cluster (Mesh B kryphos nodes) used to derive Face B bootstraps
    #[clap(long)]
    pub mpc_cluster: PathBuf,

    #[clap(long)]
    pub gateway_id: u32,
}

pub fn load_gateway_cluster(path: &PathBuf, gateway_id: u32) -> Result<GatewayConfig> {
    let data = std::fs::read_to_string(path).with_context(|| format!("read {:?}", path.display()))?;
    let cfg: GatewayClusterConfig =
        toml::from_str(&data).with_context(|| format!("parse TOML {:?}", path.display()))?;
    
    let gateway_pair = cfg.gateway_cluster
        .into_iter()
        .find(|p| p.gateway_id == gateway_id)
        .with_context(|| format!("gateway_id {} not found in mesh A[]", gateway_id))?;

    Ok(gateway_pair)
    }

    fn load_mpc_cluster(path: &PathBuf) -> Result<KryphosConfig> {
    let data = std::fs::read_to_string(path).with_context(|| format!("read {:?}", path.display()))?;
    let cfg: KryphosConfig =
        toml::from_str(&data).with_context(|| format!("parse TOML {:?}", path.display()))?;
    Ok(cfg)
}

fn ensure_p2p_suffix(addr: Multiaddr, peer: PeerId) -> Multiaddr {
    // add /p2p/<peer> if missing
    if addr.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
        addr
    } else {
        addr.with(libp2p::multiaddr::Protocol::P2p(peer))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // loading clusters
    let gateway_pair = load_gateway_cluster(&args.gateway_cluster, args.gateway_id)?;
    let kryphos_server = load_mpc_cluster(&args.mpc_cluster)?;

    // ---- Face A (gateway server) ----
    let face_a = gateway_pair.face_a;
    // ---- Face B (gateway client) ----
    let mut face_b = gateway_pair.face_b;

    // ---- Face B (client) bootstraps derived from MPC cluster ----
     // Build bootstraps from ALL kryphos nodes' gateway_listen_addr
    let mut kryphos_gateway_listen_addresses: Vec<Multiaddr> = Vec::new();
    for n in &kryphos_server.parties {
        let addr = n.gateway_listen_addr.clone().ok_or_else(|| {
            anyhow!(
                "kryphos_cluster gateway_id {} missing gateway_listen_addr",
                n.node_id
            )
        })?;
        kryphos_gateway_listen_addresses.push(ensure_p2p_suffix(addr, n.peer_id));
    }

    face_b.bootstraps = kryphos_gateway_listen_addresses;

    let cfg = GatewayConfig { 
        gateway_id: args.gateway_id,
        face_a,
        face_b
    };

    let _mgr = GatewayManager::new(cfg).await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
