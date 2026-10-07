use std::path::PathBuf;

// use anyhow::Result;
use clap::Parser;
// use tracing_subscriber::EnvFilter;

// use node_kryphos::run_from_config;


#[derive(Debug, Parser)]
#[command(name = "meshfhe")]
#[command(about = "FHE MPC/KMS node (Mesh B)", long_about = None)]
struct Args {
    /// Path to partyX.toml for party
    #[arg(long)]
    config: PathBuf
}

// fn init_tracing() {
//     let filter = EnvFilter::try_from_default_env()
//         .unwrap_or_else(|_| EnvFilter::new("info,libp2p=info,node_fhe=info"));
//     tracing_subscriber::fmt().with_env_filter(filter).init();
// }

#[tokio::main]
async fn main() {

    // init_tracing();
    // let args = Args::parse();

    // All wiring (transport + RealExecutor) happens inside node_fhe::run_from_config.
    // run_from_config(args.config).await
}