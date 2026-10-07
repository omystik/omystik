use anyhow::{anyhow, Context, Result};
use clap::Parser;
use node_kentr::{
    JobMetadata, KentrClusterConfig, KentrConfig, KomvosManager, Multiaddr, PeerId,
    YpoloClusterConfig, YpoloConfig,
};
use smart_program_alert::deploy_v1;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::time::{sleep, Duration};

#[derive(Debug, Parser)]
#[command(name = "deploy-alert")]
#[command(about = "Deploy satcon.alert_math smart-program to Ypolo")]
pub struct Args {
    /// Path to the Kentr mesh config TOML.
    #[arg(long)]
    pub kentr_cluster: PathBuf,

    /// Kentr node id used as the deploy client.
    #[arg(long)]
    pub kentr_id: u32,

    /// Path to the Ypolo mesh config TOML.
    #[arg(long)]
    pub ypolo_cluster: PathBuf,

    /// Ypolo node id receiving the deployed program.
    #[arg(long)]
    pub ypolo_id: u32,

    /// Path to the smart-program-alert runner binary as visible from the Ypolo host.
    ///
    /// Local demo example:
    ///   ./target/debug/smart-program-alert
    ///
    /// Production example:
    ///   /opt/osatcon/bin/smart-program-alert
    #[arg(long)]
    pub runner_command: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let kentr_cfg = load_kentr_config(&args.kentr_cluster, args.kentr_id)?;
    let ypolo_cfg = load_ypolo_config(&args.ypolo_cluster, args.ypolo_id)?;

    let ypolo_peer = PeerId::from_str(&ypolo_cfg.peer_id)
        .context("invalid Ypolo peer_id")?;

    let namespace_static: &'static str =
        Box::leak(kentr_cfg.namespace.clone().into_boxed_str());

    println!("Deploying satcon.alert_math@0.1.0 to Ypolo");
    println!("Kentr deploy client id={}", kentr_cfg.kentr_id);
    println!("Ypolo id={} peer={}", ypolo_cfg.ypolo_id, ypolo_peer);
    println!("Runner command={}", args.runner_command.display());

    let mut kentr_node = KomvosManager::new(
        Some(kentr_cfg.secret_seed),
        Some(kentr_cfg.listen_address.clone()),
        None,
        kentr_cfg.node_type,
        kentr_cfg.node_role,
    )
    .await
    .map_err(|e| anyhow!(e.to_string()))
    .context("create deploy Kentr KomvosManager")?;

    sleep(Duration::from_secs(3)).await;

    kentr_node
        .launch_inbound_request_task()
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("launch deploy Kentr inbound handler")?;

    let bootstrap_addr = Multiaddr::from_str(&kentr_cfg.syndesmos_address)
        .context("invalid Kentr syndesmos_address")?;

    let bootstrap_peer = PeerId::from_str(&kentr_cfg.syndesmos_peer_id_1)
        .context("invalid Kentr syndesmos_peer_id_1")?;

    kentr_node
        .register_rendezvous(
            bootstrap_addr,
            bootstrap_peer,
            namespace_static,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("register deploy Kentr endpoint to rendezvous")?;

    sleep(Duration::from_secs(3)).await;

    kentr_node
        .client
        .dial(ypolo_peer, ypolo_cfg.listen_address.clone())
        .await
        .context("dial Ypolo from deploy client")?;

    sleep(Duration::from_secs(1)).await;

    let deployed = deploy_v1(
        &mut kentr_node.client,
        ypolo_peer,
        args.runner_command.clone(),
        vec![
            ("deployed_by".into(), "smart-program-alert/deploy_alert".into()),
            ("runtime_kind".into(), "external-process-v1".into()),
            ("created_unix_ms".into(), now_unix_ms().to_string()),
        ],
    )
    .await
    .context("deploy smart-program-alert to Ypolo")?;

    println!("Deployment accepted.");
    println!("Program job/package deployed: {:?}", deployed);

    Ok(())
}

fn load_kentr_config(path: &Path, kentr_id: u32) -> Result<KentrConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: KentrClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.kentr
        .into_iter()
        .find(|p| p.kentr_id == kentr_id)
        .with_context(|| format!("kentr_id {} not found", kentr_id))
}

fn load_ypolo_config(path: &Path, ypolo_id: u32) -> Result<YpoloConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: YpoloClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.ypolo
        .into_iter()
        .find(|p| p.ypolo_id == ypolo_id)
        .with_context(|| format!("ypolo_id {} not found", ypolo_id))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}