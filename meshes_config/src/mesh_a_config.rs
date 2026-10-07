use dotenv;
use std::env;
use std::error::Error;
use std::io::{self, Write};
use libp2p::{Multiaddr, PeerId};

pub const SYN_AUTO_PROTOCOL: &str = "/file-exchange/1";

/// Struct to hold node configuration
pub struct MeshAConfig {
    pub secret_seed: Option<u8>,
    pub listen_addr: Option<Multiaddr>,
    pub fix_addr_1: Option<Multiaddr>,
    pub _fix_addr_2: Option<Multiaddr>,
    pub peer_id_1: Option<PeerId>,
    pub _peer_id_2: Option<PeerId>,
    pub namespace: Option<String>,
}

/// Function to load the appropriate environment variables
pub fn load_node_config(chosen_node: &str) -> Result<MeshAConfig, Box<dyn Error>> {
    // Ask the user which type of node they want to launch
    
    // Load the correct .env file if this is a bootstrap node
    match chosen_node {

        "1" => {
            println!("Select Syndesmos node number: (1 = Syndesmos_2, 2 = Syndesmos_2)");
            io::stdout().flush()?; // Ensure prompt is printed

            let mut node_input = String::new();
            io::stdin().read_line(&mut node_input)?;
            let node_choice = node_input.trim();

            match node_choice {
                "1" => {
                    dotenv::from_filename(".env.mesha.bootstrap1").ok();
                    println!("Loaded environment file: .env.mesha.bootstrap1");
                }
                "2" => {
                    dotenv::from_filename(".env.mesha.bootstrap2").ok();
                    println!("Loaded environment file: .env.mesha.bootstrap2");
                }
                _ => {
                    eprintln!("Invalid slection.");
                    return Err("Invalid syndesmos number.".into());
                }
            }
        }
        "2" => {
            dotenv::from_filename(".env").ok();
            println!("Running as a regular node.");
            //dotenv().ok(); // Load a default .env file if needed
        }
        _ => {
            eprintln!("Invalid choice. Enter 1, 2, or press Enter for a regular node.");
            return Err("Invalid node type selection".into());
        }
    }

    // Read environment variables
    let secret_seed = env::var("SECRET_SEED").ok().and_then(|s| s.parse::<u8>().ok());
    let listen_addr = env::var("LISTEN_ADDRESS").ok().and_then(|s| s.parse::<Multiaddr>().ok());
    let fix_addr_1 = env::var("FIX_ADDR_1" ).ok().and_then(|s|s.parse::<Multiaddr>().ok());
    let _fix_addr_2 = env::var("FIX_ADDR_2" ).ok().and_then(|s|s.parse::<Multiaddr>().ok());
    let peer_id_1 = env::var("PEER_ID_1").ok().and_then(|s|s.parse::<PeerId>().ok());
    let _peer_id_2 = env::var("PEER_ID_2").ok().and_then(|s|s.parse::<PeerId>().ok());
    let namespace = env::var("NAMESPACE_MESH_A").ok().and_then(|s|s.parse::<String>().ok());

    Ok(MeshAConfig {
        secret_seed,
        listen_addr,
        fix_addr_1,
        _fix_addr_2,
        peer_id_1,
        _peer_id_2,
        namespace,
    })
}
