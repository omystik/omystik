// use meshes_config; // Our configuration module

// use std::{error::Error, sync::Arc};
// use std::io::{self, BufRead, Write};
// use tokio::{sync::Mutex, time::{sleep, Duration}};

// use libp2p::Multiaddr;
use tracing_subscriber::EnvFilter;
// use futures::future::pending; //Stream


#[tokio::main]
async fn main() {
    // Initialize logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Ask the user which node type to launch.
    

}


