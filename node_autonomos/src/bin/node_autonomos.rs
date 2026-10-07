use anyhow::{Result, Context};
use clap::Parser;
use std::{error::Error, sync::Arc};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use tokio::{sync::Mutex, time::{sleep, Duration}};
use libp2p::Multiaddr;
use libp2p_common::{
    manager::KomvosManager,
    node_services::spread_into_network,
    node_primary::{
        AutonomosConfig, AutonomosClusterConfig, // Our configuration module
    }
};
use tracing_subscriber::EnvFilter;
use futures::future::pending; //Stream

// Optionally, also import your node command functions.
use persistency_utils;

use content_hashing::get_storage_dir;
// prompt the command
fn print_prompt_command() -> io::Result<()>{
    println!(
            "Enter command: (store <file_name> <file_type> <secret> <file_path>\
            | spread <mid (metadata id)>\
            | call <file_name> <file_type> <secret>\
            | get <file_name> <file_type> <secret> <private_key> <nonce>\
            | exit)"
        );
    io::stdout().flush()
}

#[derive(Parser)]
#[command(name = "node-autonomos")]
pub struct Args {
    /// Path to the Autonomos mesh autonomos_config TOML
    #[arg(long)]
    pub mesh_a_cluster: PathBuf,

    // Autonomos node id
    #[arg(long)]
    pub autonomos_id: u32,
}

pub fn load_node_config(path: &Path, node_id: u32) -> Result<AutonomosConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: AutonomosClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    let node_autonomos = cfg.autonomos
        .into_iter()
        .find(|p| p.autonomos_id == node_id)
        .with_context(|| format!("node_id {} not found in autonomos autonomos_config", node_id))?;

    Ok(node_autonomos)
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Initialize logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Load configuration from our node_config module.
    let autonomos_config= load_node_config(
        &args.mesh_a_cluster, 
        args.autonomos_id,
    )?;

    let listen_address: Option<Multiaddr> = None;
    // For simplicity we use None for bootstrap/peer here.
    let peer: Option<Multiaddr> = None;

    println!("Launching Autonomos Node ...");

    // adding the common namespace
    let namespace = autonomos_config.namespace.expect("Check if Autonomous node has a namespace set up");
    // Convert the String into a &'static str by leaking it.
    let namespace_static: &'static str = Box::leak(namespace.into_boxed_str());

    println!("NAMESPACE: {:?}", namespace_static);

    let autonomos_node =
        Arc::new(Mutex::new(KomvosManager::new(autonomos_config.secret_seed, listen_address.clone(), peer, None, None).await?));
    
    // Wait a moment for listening to be established.
    sleep(Duration::from_secs(5)).await;
    println!("Node launched and connected to the network.");


    // Spawn the event loop.
    // {
    //     let event_loop_arc = autonomos_node.lock().await.event_loop.clone();
    //     tokio::spawn(async move {
    //         event_loop_arc.lock().await.run().await;
    //     });
    // }

    // Spawn the inbound answer loop as a separate task.
    let autonomos_clone = autonomos_node.clone(); // <-- clone the Arc!
    tokio::spawn(async move {
        let mut auto = autonomos_clone.lock().await;
        if let Err(e) = auto.launch_inbound_request_task().await {
            eprintln!("Error transmitting file in inbound handler: {}", e);
        }
    });
    // ------------------------------------------------------------
    // Rendezvous registration
    // ------------------------------------------------------------
    let bootstrap_addr = autonomos_config.syndesmos_addr
        .context("invalid syndesmos_address")?;
    let bootstrap_peer = autonomos_config.syndesmos_peer
        .context("invalid syndesmos_peer_id")?;
    // regular node register to rendezvous node            
    let _register = autonomos_node.lock().await.register_rendezvous(
        bootstrap_addr,
        bootstrap_peer,
        namespace_static).await;
   
    // publish persistent mid if any to DHT
    let _publish_to_dht = autonomos_node
        .lock()
        .await
        .file_services()
        .init_publishing_mid_to_dht()
        .await;
     
    // Give DHT/provider records a short propagation window.
    sleep(Duration::from_secs(3)).await;

    // ask for default key: KeyType::PublicKey
    {
        let mut guard = autonomos_node.lock().await;
        guard.ask_key(None).await?;
    }

    println!("Autonomos node launched and connected to the network.");
    
    // Command-line interface loop for the autonomos node.
    command_loop_autonomos(autonomos_node).await?;

    // Keep the node running indefinitely.
    pending::<()>().await;

    Ok(())
}

/// Command loop for Autonomos (regular node)
async fn command_loop_autonomos(

    autonomos: Arc<Mutex<KomvosManager>>

) -> Result<(), Box<dyn Error + Send + Sync>> {
    
    // Command-line interface.
    let _print_command = print_prompt_command()?;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line?;
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "store" if parts.len() == 5 => {
                let file_name = parts[1];
                let file_type = parts[2];
                let secret = parts[3];
                let file_path = parts[4];

                let tmp_constant_path = std::path::PathBuf::from(get_storage_dir());
                match persistency_utils::store_file(
                    file_name,
                    file_type,
                    secret,
                    file_path
                )
                .await
                {
                    Ok((private_key, nonce, mid_as_hex, cid_as_hex)) => {
                        println!("🔍 private_key: {:?}", private_key);
                        println!("🔍 nonce: {:?}", nonce);

                        //Clone the Arc to call spread_ntwk
                        {
                            let autonomos_clone = Arc::clone(&autonomos);
                            let mid = mid_as_hex.clone();
                            let cid = cid_as_hex.clone();
                            let result = {
                                let mut auto = autonomos_clone.lock().await;
                                auto.register_midcid_memory(mid.to_string(), tmp_constant_path, cid).await
                            };
                            match result {
                            Ok(()) => println!("Registered new midcid pair successfully."),
                            Err(e) => eprintln!("Error Registering midcid pair: {}", e),
                            }
                        }
                            
                    }
                    Err(e) => eprintln!("Error storing file: {}", e),
                }
            }
            "spread" => {
                if parts.len() == 2 {
                    let mid_as_hex = parts[1].to_string();
                    // let path = persistency_utils::local_path_to_cid(&mid_as_hex).await?;
                    // let cid_as_hex = persistency_utils::midhex_to_cidhex(&mid_as_hex).await?;
                    
                    //Clone the Arc to call spread_ntwk
                    {
                        let autonomos_clone = Arc::clone(&autonomos);
                        let mid = mid_as_hex.clone();
                        let result = {
                            let mut auto = autonomos_clone.lock().await;
                            spread_into_network(&mut auto.client, mid.to_string()).await
                        };
                        match result {
                        Ok(()) => println!("File spreaded on DHT successfully!"),
                        Err(e) => eprintln!("Error spreaded on DHT: {}", e),
                        }
                    }
                }
            }
            "call" => {
                if parts.len() == 4 {
                    let file_name = parts[1];
                    let file_type = parts[2];
                    let secret = parts[3];

                    match persistency_utils::propose_mid_ntwk(file_name, file_type, secret).await {
                        
                        Ok(returned_mid_hex) => {
                            println!("call - Debug: Returned MID: {:?}", returned_mid_hex);
                            {
                                let autonomos_clone = Arc::clone(&autonomos);
                                let returned_mid = returned_mid_hex.clone();
                                let result = {
                                    let mut auto = autonomos_clone
                                        .lock()
                                        .await;
                                    auto.file_services()
                                        .get_file(returned_mid)
                                        .await
                                };
                                match result {
                                    Ok(cid_as_hex) => {
                                        println!("File transmitted successfully!");

                                        // create persistently MID
                                        match persistency_utils::create_persist_mid_ntwk(file_name, file_type, secret).await{
                                            Ok(persistent_mid) => {
                                                println!("Persistent MID {} creation succeeded!", persistent_mid);
                                            },
                                            Err(e) => eprintln!("Error Persistent MID creation: {}", e),
                                        }

                                        // add MID/CID pair to local sparse merkle tree
                                        match persistency_utils::import_ntwk_file(&returned_mid_hex, &cid_as_hex).await {
                                            Ok(()) => println!("Import succeeded!"),
                                            Err(e) => eprintln!("Error importing file: {}", e),
                                        }
                                    }
                                    Err(e) => eprintln!("Error transmitting file: {}", e),
                                }
                            }
                        }
                        Err(e) => eprintln!("Error retrieving file: {}", e),
                    }
                }
            }
            "get" => {
                println!("🔍 Raw Input: '{}'", input);
                let parts: Vec<&str> = input.trim().splitn(6, ' ').collect();
                if parts.len() < 6 {
                    eprintln!("Error: 'get' command requires at least 6 arguments but got {}", parts.len());
                    continue;
                }
                let file_name = parts[1];
                let file_type = parts[2];
                let secret = parts[3];
                let private_key = parts[4];
                let nonce = parts[5];
                match persistency_utils::get_cid_file(file_name, file_type, secret).await {
                    Ok(returned_cid) => {
                        println!("get - Debug: Returned CID: {:?}", returned_cid);
                        let decrypt_path = format!(
                            "./data/decrypt_content/{}{}",
                            file_name, file_type
                        );
                        persistency_utils::get_decrypt_file(private_key, nonce, &returned_cid, &decrypt_path)
                            .await?;
                        println!("File retrieved successfully.")
                    }
                    Err(e) => eprintln!("Error retrieving file: {}", e),
                }
            }
            "exit" => break,
            _ => println!("Unknown command. Usage: store <file_name> <file_type> <secret> <file_path>\
                           | spread <mid (metadata id)>\
                           | call <file_name> <file_type> <secret>\
                           | get <file_name> <file_type> <secret> <private_key> <nonce>\
                           | exit"),
        }
        let _print_command = print_prompt_command()?;
    }
    Ok(())
}