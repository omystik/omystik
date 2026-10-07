use anyhow::{Context, Result};

use clap::Parser;
use futures::future::pending;
use kms_lib::consts::SIGNING_KEY_ID;
use kms_lib::util::key_setup::ensure_client_keys_exist;
use kms_core_client::CCCommand;
use libp2p::{Multiaddr, PeerId};
use libp2p_common::{
    manager::KomvosManager,
    node_primary::{
        KentrConfig, KentrClusterConfig},
};
use node_kentr::{
    mesh_cmd::execute_mesh_or_fallback,
    setup_logging, CmdConfig};
use observability::conf::Settings;

use std::{
    path::{Path, PathBuf},
    {str::FromStr, sync::Arc}};
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};
use validator::Validate;

#[derive(Parser)]
#[command(name = "node-kentr")]
pub struct Args {
    /// Path to the Kentr mesh config TOML
    #[arg(long)]
    pub kentr_cluster: PathBuf,

    /// Kentr node id
    #[arg(long)]
    pub kentr_id: u32,

    /// args passed to core-client
    #[arg(last = true)]
    pub core_client_args: Vec<String>,

    /// test fetch keyset from pylon
    #[arg(long, default_value_t = false)]
    pub rehydrate_key: bool,
}

pub fn load_kentr(path: &Path, kentr_id: u32) -> Result<KentrConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: KentrClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.kentr
        .into_iter()
        .find(|p| p.kentr_id == kentr_id)
        .with_context(|| format!("kentr_id {} not found in config", kentr_id))
}

#[tokio::main]
async fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Parse CLI args
    // ------------------------------------------------------------
    let args = Args::parse();

    // ------------------------------------------------------------
    // Load mesh-level Kentr config
    // ------------------------------------------------------------
    let mesh_cfg = load_kentr(&args.kentr_cluster, args.kentr_id)?;

    // ------------------------------------------------------------
    // Logging (ONCE, early)
    // ------------------------------------------------------------
    setup_logging();

    println!("Launching Kentr node id={} …", mesh_cfg.kentr_id);

    // ------------------------------------------------------------
    // Start Kentr libp2p node
    // ------------------------------------------------------------
    let namespace_static: &'static str =
        Box::leak(mesh_cfg.namespace.clone().into_boxed_str());

    let kentr_node = Arc::new(Mutex::new(
        KomvosManager::new(
            Some(mesh_cfg.secret_seed),
            Some(mesh_cfg.listen_address.clone()),
            None, // no direct peer bootstrap
            mesh_cfg.node_type,
            mesh_cfg.node_role,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    ));

    // Allow swarm to bind
    sleep(Duration::from_secs(3)).await;

    // Inbound job handling
    {
        let node = kentr_node.clone();
        tokio::spawn(async move {
            if let Err(e) = node.lock().await.launch_inbound_request_task().await {
                eprintln!("Inbound handler error: {e}");
            }
        });
    }

    // ------------------------------------------------------------
    // Rendezvous registration
    // ------------------------------------------------------------
    let bootstrap_addr = Multiaddr::from_str(&mesh_cfg.syndesmos_address)
        .context("invalid syndesmos_address")?;
    let bootstrap_peer = PeerId::from_str(&mesh_cfg.syndesmos_peer_id_1)
        .context("invalid syndesmos_peer_id_1")?;

    kentr_node
        .lock()
        .await
        .register_rendezvous(
            bootstrap_addr,
            bootstrap_peer,
            namespace_static,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    kentr_node
        .lock()
        .await
        .file_services()
        .init_publishing_mid_to_dht()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!("Kentr node connected to mesh.");
    
    // Give rendezvous + identify time to discover/classify Mesh A peers.
    sleep(Duration::from_secs(3)).await;

    let discovered_pylons = {
        kentr_node.lock().await.pylon_peers().await
    };

    if discovered_pylons.is_empty() {
        eprintln!("warning: no Pylon Face-A peers discovered yet");
    }

    // ------------------------------------------------------------
    // KMS libp2p / Mesh bridge layer
    // ------------------------------------------------------------
    println!("Starting KMS Mesh Bridge v{}", env!("CARGO_PKG_VERSION"));

    let keys_folder = Path::new("keys");

    if args.rehydrate_key {
        let path_to_config = core_client_config_path(&args.core_client_args)?;

        node_kentr::key_rehydration::rehydrate_keyset_from_mesh_local_key(
            Arc::clone(&kentr_node),
            keys_folder,
            &path_to_config,
            discovered_pylons.clone(),
        )
        .await?;
   
    } else {
        let cmd_cfg = CmdConfig::parse_from(
            std::iter::once("core-client")
                .chain(args.core_client_args.iter().map(String::as_str)),
        );
        
        cmd_cfg.validate()?;

        ensure_client_keys_exist(Some(keys_folder), &SIGNING_KEY_ID, true).await;

        let net_client = { kentr_node.lock().await.client.clone() };

        let results =  execute_mesh_or_fallback(
            &cmd_cfg,
            keys_folder,
            net_client,
            discovered_pylons,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        
        for (opt_req_id, msg) in results {
            match opt_req_id {
                Some(req_id) => {
                    println!("{msg} - \"request_id\": \"{req_id}\"");
                }
                None => {
                    println!("{msg} - no request_id returned");
                }
            }
        }
        
        if matches!(
            &cmd_cfg.command,
            CCCommand::KeyGen(_) | CCCommand::KeyGenFresh(_) | CCCommand::CrsGen(_)
        ) {
            if let Err(e) = node_kentr::key_rehydration::check_keys_kentr(
                Arc::clone(&kentr_node),
                keys_folder).await
            {
                eprintln!("check_keys_kentr failed: {e}");
            }
        }
    }
    // ------------------------------------------------------------
    // Keep node alive
    // ------------------------------------------------------------
    pending::<()>().await;

    Ok(())
}


fn core_client_config_path(args: &[String]) -> Result<PathBuf> {
    for i in 0..args.len() {
        if args[i] == "-f" || args[i] == "--file-conf" {
            let Some(path) = args.get(i + 1) else {
                return Err(anyhow::anyhow!("missing value after {}", args[i]));
            };
            return Ok(PathBuf::from(path));
        }
    }

    Err(anyhow::anyhow!(
        "--rehydrate-key requires core-client config path: -- -f <config.toml>"
    ))
}