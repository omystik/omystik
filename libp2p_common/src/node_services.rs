use crate::komvos::Client;
use crate::key_ops::{save_key_at_path, key_received_as_expected};
use crate::node_primary::KeyType;
use content_hashing::{get_keys_dir, get_storage_dir};
use db_access::get_midcid_pairs;
use libp2p::{PeerId, Multiaddr, multiaddr::Protocol};
use std::{error::Error, collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{fs, sync::Mutex, time::{Duration, sleep}};
use tracing::{self, debug}; 


/// Spread element (file, key, ...) name into the DHT
pub async fn spread_into_network(client: &mut Client, name: String) -> Result<(), Box<dyn Error + Send + Sync>> {

    // Advertise self as a provider of `name`.
    match client.start_providing(name.clone()).await {
        Ok(_) => {
            debug!("Successfully started providing '{}'.", name);
        }
        Err(e) => {
            debug!("Failed to start providing '{}': {}", name, e);
        }
    }
    Ok(())
}

pub struct KeyServices<'a> {
    pub client: &'a mut Client,
    pub peer: Option<&'a Multiaddr>,
}

impl<'a> KeyServices<'a> {

pub async fn get_key(&mut self, key_type: KeyType) -> Result<(Vec<u8>, String), Box<dyn Error + Send + Sync>> {
        
        let key_type = key_type.to_string();

        // pausing a bit to let some propagation inside the network
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let provider_map: HashMap<PeerId, Vec<Multiaddr>> = self.client.get_providers(key_type.clone()).await?;

        if provider_map.is_empty() {
            println!("No providers found for key {}.", key_type);
            return Err(format!("Could not find provider for key {}", key_type).into());
        }

        println!("\nMany here\n{:?}\n", provider_map);

        // Select the first provider that has at least one address.
        for (provider_peer_id, addrs) in provider_map.into_iter() {

            if addrs.is_empty() {
                continue;
            }
            // Pick the first address and attach the peer ID (if not already attached).
            let peer_addr = addrs.first()
                .map(|addr| {
                    if !addr.iter().any(|p| matches!(p, Protocol::P2p(_))) {
                        addr.clone().with(Protocol::P2p(provider_peer_id))
                    } else {
                        addr.clone()
                    }
                })
                .ok_or_else(|| format!("Provider {} has no addresses", provider_peer_id))?;

            println!("Trying provider: {:?}\nwith address: {:?}", provider_peer_id, peer_addr);

            // Now, extract the PeerId from the full multiaddr.
            let Some(Protocol::P2p(peer_id)) = peer_addr.iter().last() else {
                return Err("Determined peer address does not contain a PeerId.".into());
            };

            // Dial the peer
            if let Err(e) = self.client.dial(peer_id, peer_addr.clone()).await {
                debug!("Dialing provider {} failed: {:?}", provider_peer_id, e);
                continue;
            }

            // Optionally wait a short time for the DHT to propagate provider records.
            tokio::time::sleep(Duration::from_secs(3)).await;

            // Once connected, proceed to request the file.
            match self.client.request_key(peer_id, key_type.clone()).await {
                Ok((key_bytes, key_name)) => {
                    debug!("Provider {} succeeded with key: {}", provider_peer_id, key_name);

                    let output_path = std::path::Path::new(&get_keys_dir())
                        .join(&key_type)
                        .join(&key_name);

                    save_key_at_path(&output_path, &key_bytes).await?;

                    return Ok((key_bytes, key_name));
                }
                Err(e) => {
                    debug!("Provider {} failed: {:?}", provider_peer_id, e);
                }
            }


        }
    return Err("None of the providers returned key.".into());
    }


    // ask for key to mesh
    pub async fn ask_key(&mut self, key_type: Option<KeyType>) -> anyhow::Result<()> {

        let key_type = key_type.unwrap_or_default();

        let keys_folder = get_keys_dir();

        // del everything in the key dir.
        let mut entries = fs::read_dir(&keys_folder).await?;

        while let Some(entry) = entries.next_entry().await? {
            let child = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                fs::remove_dir_all(child).await?;
            } else {
                fs::remove_file(child).await?;
            }
        }

        // key is mandatory, so we loop it
        loop {
            // get_key
            match self.get_key(key_type.clone()).await {
                Ok((_key_bytes, key_name)) => {

                    // reconstruct key path for checking
                    let saved_path = std::path::Path::new(&get_keys_dir())
                        .join(&key_type.as_str())
                        .join(&key_name);

                    // check the key and spread to DHT
                    if key_received_as_expected(&saved_path, &key_name)? {
                        spread_into_network(self.client, key_type.to_string())
                            .await
                            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        println!("Key received successfully: {}", key_name);
                        return Ok(());
                    } else {
                        eprintln!(
                            "Key downloaded but validation failed at {}",
                            saved_path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("ask_key retry: {}", e);
                }
            }

            sleep(Duration::from_secs(5)).await;
        }
    }
}

pub struct FileServices<'a> {
    pub client: &'a mut Client,
    pub peer: Option<&'a Multiaddr>,
    pub midcid_memory_register: Arc<Mutex<HashMap<String, (PathBuf, String)>>>,
}
impl<'a> FileServices<'a> {

    // checking existence of persistent mid and cid data, return an HashMap
    pub async fn access_the_db() -> Result<HashMap::<String, (PathBuf, String)>, Box<dyn Error + Send + Sync>> {
        // pub const STORAGE_DIR: &str = "./data/content/";
        
        let tmp_constant_path = get_storage_dir();

        let mut file_registry = HashMap::<String, (PathBuf, String)>::new();

        let pairs = get_midcid_pairs().await?;
        
        // if pairs are empty just return empty hashmap        
        if pairs.is_empty() {
            return Ok(file_registry);
        }
        
        for (mid, cid) in &pairs {
            let path = format!("{}{}.bin", tmp_constant_path, cid);
            let path_cid = PathBuf::from(path);
            println!("MID: {} -> CID: {}", mid, cid);
            file_registry.insert(mid.to_string(), (path_cid, cid.to_string()));
        }
        
        //debug!("\nAccess DB MID: {}\nCID: {}", midhex, cidhex);
        Ok(file_registry)
    }

    // checking existence of persistent mid and push it to DHT
    pub async fn init_publishing_mid_to_dht(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // pub const STORAGE_DIR: &str = "./data/content/";

        let pairs = get_midcid_pairs().await?;
        
        // if pairs are empty just return        
        if pairs.is_empty() {
            return Ok(());
        }
        
        for (mid, _cid) in &pairs {

            // pushing the brand new MID to the DHT
            let _publish_dht= spread_into_network(self.client,mid.to_string())
                .await
                .map_err(|e| -> Box<dyn Error + Send + Sync> { e.into() })?;
        }
        Ok(())
    }

    pub async fn get_file(&mut self, name: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        
        // Case 1: A peer is already provided.
        if let Some(addr) = &self.peer {
        let peer_addr = addr.clone();
        // Extract the PeerId from the multiaddress.
        let Some(Protocol::P2p(peer_id)) = peer_addr.iter().last() else {
            return Err("Provided peer address does not contain a PeerId.".into());
        };
        // Dial the provided peer.
        self.client.dial(peer_id, peer_addr.clone()).await?;
        // Wait a short time.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        // Request the file.
        match self.client.request_file(peer_id, name.clone()).await {
            Ok((_file_bytes, cid)) => {
                debug!("Provider {} succeeded with CID: {}", peer_id, cid);
                return Ok(cid);
            }
            Err(e) => {
                debug!("Provider {} failed: {:?}", peer_id, e);
                return Err(e.into());
            }
        }
        
        } else {
            // pausing a bit to let some propagation inside the network
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            let provider_map: HashMap<PeerId, Vec<Multiaddr>> = self.client.get_providers(name.clone()).await?;

            if provider_map.is_empty() {
                println!("No providers found for file {}.", name);
                return Err(format!("Could not find provider for file {}", name).into());
            }

            println!("\nMany here\n{:?}\n", provider_map);

            // Select the first provider that has at least one address.
            for (provider_peer_id, addrs) in provider_map.into_iter() {

                if addrs.is_empty() {
                    continue;
                }
                // Pick the first address and attach the peer ID (if not already attached).
                let peer_addr = addrs.first()
                    .map(|addr| {
                        if !addr.iter().any(|p| matches!(p, Protocol::P2p(_))) {
                            addr.clone().with(Protocol::P2p(provider_peer_id))
                        } else {
                            addr.clone()
                        }
                    })
                    .ok_or_else(|| format!("Provider {} has no addresses", provider_peer_id))?;

                println!("Trying provider: {:?}\nwith address: {:?}", provider_peer_id, peer_addr);

                // Now, extract the PeerId from the full multiaddr.
                let Some(Protocol::P2p(peer_id)) = peer_addr.iter().last() else {
                    return Err("Determined peer address does not contain a PeerId.".into());
                };

                // Dial the peer
                if let Err(e) = self.client.dial(peer_id, peer_addr.clone()).await {
                    debug!("Dialing provider {} failed: {:?}", provider_peer_id, e);
                    continue;
                }

                // Optionally wait a short time for the DHT to propagate provider records.
                tokio::time::sleep(Duration::from_secs(3)).await;

                // Once connected, proceed to request the file.
                match self.client.request_file(peer_id, name.clone()).await {
                    Ok((_file_bytes, cid)) => {
                        debug!("Provider {} succeeded with CID: {}", provider_peer_id, cid);
                        return Ok(cid);
                    }
                    Err(e) => {
                        debug!("Provider {} failed: {:?}", provider_peer_id, e);
                    }
                }
            }
            return Err("None of the providers returned file.".into());
        }
    }

}