use anyhow::{Context, Result};
use clap::Parser;
use content_hashing::get_keys_dir;
use futures::future::pending;
use libp2p::{Multiaddr, PeerId};
use libp2p_common::{key_ops::single_filename, node_primary::KeyType};
use serde::Deserialize;
use std::{path::{Path, PathBuf}, str::FromStr, sync::Arc};
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};
use tracing_subscriber::EnvFilter;

use node_ypolo::node_ypolo::YpoloManager;

// DUPLICATE from kombos.rs
// --- blob storage
// pub const BLOB_STORAGE_DIR: &str = "./data/blob/";
pub fn get_storage_blob() -> String {
    std::env::var("BLOB_STORAGE_DIR").unwrap_or_else(|_| "./data/blob/".to_string())
}

#[derive(Debug, Parser)]
#[command(name = "node_ypolo")]
#[command(about = "Mesh C generic FHE compute node")]
pub struct Args {
    /// Path to the ypolo mesh config TOML
    #[arg(long)]
    pub ypolo_cluster: PathBuf,

    /// ypolo node id
    #[arg(long)]
    pub ypolo_id: u32,
}

#[derive(Debug, Deserialize)]
struct ClusterConfig {
    ypolo: Vec<YpoloConfig>,
}

#[derive(Debug, Deserialize)]
pub struct YpoloConfig {
    pub ypolo_id: u32,
    pub secret_seed: u8,
    pub listen_address: Multiaddr,
    pub namespace: String,
    pub syndesmos_peer_id: String,
    pub syndesmos_address: String,
}

pub fn load_ypolo(path: &Path, ypolo_id: u32) -> Result<YpoloConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: ClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.ypolo
        .into_iter()
        .find(|p| p.ypolo_id == ypolo_id)
        .with_context(|| format!("ypolo_id {} not found in config", ypolo_id))
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let args = Args::parse();
    let mesh_cfg = load_ypolo(&args.ypolo_cluster, args.ypolo_id)?;

    println!("Launching Ypolo node id={} …", mesh_cfg.ypolo_id);

    let storage_root = get_storage_blob();

    let ypolo_node = Arc::new(Mutex::new(
        YpoloManager::new(
            Some(mesh_cfg.secret_seed),
            Some(mesh_cfg.listen_address.clone()),
            None,
            PathBuf::from(storage_root),
        )
        .await?,
    ));

    sleep(Duration::from_secs(2)).await;

    let namespace_static: &'static str =
        Box::leak(mesh_cfg.namespace.clone().into_boxed_str());

    let bootstrap_addr = Multiaddr::from_str(&mesh_cfg.syndesmos_address)
        .context("invalid syndesmos_address")?;
    let bootstrap_peer = PeerId::from_str(&mesh_cfg.syndesmos_peer_id)
        .context("invalid syndesmos_peer_id")?;

    ypolo_node
        .lock()
        .await
        .client
        .register(bootstrap_peer, bootstrap_addr, namespace_static)
        .await?;

    println!(
        "Ypolo node id={} registered on namespace {}",
        mesh_cfg.ypolo_id, mesh_cfg.namespace
    );

    sleep(Duration::from_secs(3)).await;

    {
        let mut guard = ypolo_node.lock().await;
        guard.ask_key().await?;
    }

    let keys_root = get_keys_dir();
    let server_key_dir = Path::new(&keys_root).join(KeyType::ServerKey.to_string());

    let key_name = single_filename(&server_key_dir)?.ok_or_else(|| {
        anyhow::anyhow!(
            "expected exactly one ServerKey in {}",
            server_key_dir.display()
        )
    })?;

    let server_key_path = server_key_dir.join(key_name);

    {
        let mut guard = ypolo_node.lock().await;

        guard.configure_tfhe_executor(server_key_path);
        println!("Local TFHE/external-process executors are configured.");

        guard.install_jobs_handler().await?;
        guard.launch_event_task().await?;
    }

    println!("Ypolo node launched and connected to the network.");

    pending::<()>().await;

    #[allow(unreachable_code)]
    Ok(())
}