use content_hashing::{
    blakecheck::{ send_file_with_hash, send_key },
    get_keys_dir,
};
use std::{collections::HashMap, error::Error, path::{Path, PathBuf}, pin::Pin, sync::Arc};
use futures::{prelude::*, StreamExt};
use libp2p::{PeerId, core::Multiaddr, multiaddr::Protocol};
use tracing_subscriber::EnvFilter;
use tracing::{self, debug}; 
use crate::komvos::{self, Client, Event, EventLoop};

use tokio::{sync::Mutex, time::Duration};
use tokio_util::compat::FuturesAsyncWriteCompatExt; // try to make compatibility between tokio and libp2p stream
use crate::{
    key_ops,
    node_primary::{NodeType, NodeRole, KeyType},
    node_services::{FileServices, KeyServices, spread_into_network}
};



pub struct KomvosManager {
    pub node_type: NodeType,
    pub node_role: NodeRole,
    pub client: Client,
    pub event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
    pub event_loop: Arc<Mutex<EventLoop>>,
    /// Address on which to listen (or `None` to listen on 0.0.0.0:0).
    pub listen_address: Option<Multiaddr>,
    /// Peer to dial (optional) => bootstrapping or relaying
    pub peer: Option<Multiaddr>,
    midcid_memory_register: Arc<Mutex<HashMap<String, (PathBuf, String)>>>,
    pylon_face_a_peers: Arc<Mutex<HashMap<PeerId, Vec<Multiaddr>>>>,
}

impl KomvosManager{

    pub fn key_services(&mut self) -> KeyServices<'_> {
        KeyServices {
            client: &mut self.client,
            peer: self.peer.as_ref(),
        }
    }

    pub fn file_services(&mut self) -> FileServices<'_> {
        FileServices {
            client: &mut self.client,
            peer: self.peer.as_ref(),
            midcid_memory_register: Arc::clone(&self.midcid_memory_register),
        }
    }

    // ask for key to mesh
    pub async fn ask_key(&mut self, key_type: Option<KeyType>) -> anyhow::Result<()> {
        self.key_services().ask_key(key_type).await
    }

    pub async fn new(
        secret_seed: Option<u8>,
        listen_address:Option<Multiaddr>,
        peer:Option<Multiaddr>,
        node_type: Option<NodeType>,
        node_role: Option<NodeRole>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {

        // Initialize logging.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        // setting default Autonomos type
        let node_type = node_type.unwrap_or_default();
        let node_role = node_role.unwrap_or_default();

        let (
            mut client,
            event_stream,
            composite_event_loop
        ) = komvos::new(secret_seed, node_type).await?;

        // Extract the shared listen address handle from the event loop.
        // This field is an Arc<Mutex<Option<Multiaddr>>> that is updated by the event loop.
        let listen_addr_handle = composite_event_loop.actual_listen_addr.clone();

        // Spawn the event loop using its spawn() method.
        let event_loop = Arc::new(Mutex::new(composite_event_loop));

        let local_peer_id: PeerId = {
        let guard = event_loop.lock().await;
            (*guard).local_peer_id()
            };
        {
        let event_loop_clone = event_loop.clone();
            tokio::spawn(async move {
               event_loop_clone.lock().await.run().await;
            });
        }

        // Listen on the user-provided address, or fallback to /ip4/0.0.0.0/udp/0/quic-v1.
        match &listen_address {
            Some(addr) => {
                client
                    .start_listening(addr.clone())
                    .await
                    .expect("Listening not to fail.");
            }
            None => {
                client
                    .start_listening("/ip4/0.0.0.0/udp/0/quic-v1".parse()?) // 192.168.1.42
                    .await
                    .expect("Listening not to fail.");
            }
        }

        // Dial a peer if specified.
        if let Some(addr) = &peer {
            // Extract the PeerId from the multiaddr (the /p2p/ component).
            let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
                return Err("Expect peer multiaddr to contain peer ID.".into());
            };
            client
                .dial(peer_id, addr.clone())
                .await
                .expect("Dial to succeed");
        }

         let addr_to_advertise = match &listen_address {
            Some(user_addr) => user_addr.clone(),
            None => {
                loop {
                    let maybe_addr = {
                        let guard = listen_addr_handle.lock().await;
                        (*guard).clone()
                    };
                    if let Some(addr) = maybe_addr {
                        // println!("Got ephemeral address: {}", addr);
                        break addr;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        };

        // 8) Advertise on the DHT
        client.advertise(local_peer_id, addr_to_advertise).await?;

        // checking existence of persistent mid and cid data and loading (necessary when node has been switched off)
        let current_persistent_midcid = FileServices::access_the_db().await?;
        
        Ok(Self {
            node_type,
            node_role,
            client,
            event_stream: Box::pin(event_stream),
            event_loop,
            listen_address,
            peer,
            midcid_memory_register: Arc::new(Mutex::new(current_persistent_midcid)),
            pylon_face_a_peers: Arc::new(Mutex::new(HashMap::new())),
        })

    }


    

    pub async fn launch_inbound_request_task(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let midcid_memory_register_clone = self.midcid_memory_register.clone();
        let pylon_face_a_peers_clone = self.pylon_face_a_peers.clone();

        // Take ownership of the current event stream and replace it with an empty stream.
        let mut event_stream = std::mem::replace(&mut self.event_stream, Box::pin(futures::stream::empty()));

        // Spawn a local task to process inbound file request events directly.
        tokio::spawn(async move {
            debug!("Waiting for inbound file requests...");
            while let Some(event) = event_stream.next().await {
                match event {
                    komvos::Event::InboundFileRequest { peer, file_name, stream } => {
                        debug!("Received inbound file request for: {}", file_name);

                        let midcid_memory_registry = midcid_memory_register_clone.lock().await;

                        if let Some((path, cid)) = midcid_memory_registry.get(&file_name) {
                            let path_cid = path.clone();
                            let local_cid = cid.clone();
                            let file_name_clone = file_name.clone();

                            // Use the stream directly.
                            let (_reader, writer) = stream.split();
                            let writer = writer.compat_write();

                            tokio::spawn(async move {
                                debug!("Handling file request from {:?} for file '{}'", peer, file_name_clone);

                                if let Err(e) = send_file_with_hash(writer, &path_cid, &local_cid).await {
                                    debug!("Error sending file: {} look path: {}", e, path_cid.display());
                                }
                            });
                        } else {
                            debug!("Peer {:?} requested unknown file {}", peer, file_name);
                        }
                    }
                    komvos::Event::InboundKeyRequest { peer, key_type, stream } => {

                        let keys_root = get_keys_dir();

                        let key_dir = Path::new(&keys_root).join(key_type.as_str());

                        match key_ops::single_filename(&key_dir) {
                            Ok(Some(key_name)) => {
                                let key_path = key_dir.join(&key_name);
                                println!("Serving key file: {}", key_path.display());

                                let (_reader, writer) = stream.split();
                                let writer = writer.compat_write();

                                tokio::spawn(async move {
                                    debug!("Handling key request from {:?} for key '{}'", peer, key_name.clone());
                                    if let Err(e) = send_key(writer, &key_path).await {
                                        eprintln!("Error sending key: {} look path: {}", e, key_path.display());
                                    }
                                });
                            }
                            Ok(None) => {
                                eprintln!("Could not resolve unique key file in {}", key_dir.display());
                            }
                            Err(e) => {
                                eprintln!("Failed to inspect {}: {}", key_dir.display(), e);
                            }
                        }
                    }

                    // for Pylon discovered
                    komvos::Event::PeerIdentified {
                        peer,
                        protocol_version,
                        agent_version: _,
                        listen_addrs,
                    } => {
                        if Self::is_pylon_face_a_protocol(&protocol_version) {
                            let mut guard = pylon_face_a_peers_clone.lock().await;
                            let entry = guard.entry(peer).or_insert_with(Vec::new);

                            for addr in listen_addrs {
                                let full_addr = if !addr.iter().any(|p| matches!(p, Protocol::P2p(_))) {
                                    addr.clone().with(Protocol::P2p(peer))
                                } else {
                                    addr.clone()
                                };

                                if !entry.iter().any(|a| a == &full_addr) {
                                    entry.push(full_addr);
                                }
                            }

                            debug!("Registered pylon face-a peer {}", peer);
                        }
                    }

                    // For simplicity, skip unimplemented events. other => todo!("{:?}", other),
                    other => {
                        tracing::warn!("Unexpected event_stream: {:?}", other);
                    }
                }
            }
        });

        Ok(())
    }


    /// Register this node with a rendezvous server.
    /// rendezvous_addr: multiaddr of the rendezvous server.
    /// rendezvous_peer: PeerId of the rendezvous server.
    /// namespace: the namespace in which to register.
    pub async fn register_rendezvous(&mut self, rendezvous_addr: Multiaddr, rendezvous_peer: PeerId, namespace: &'static str) -> Result<(), Box<dyn Error + Send + Sync>> {
        
        debug!("register_rendezvous fn has been called");
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        //self.event_loop.lock().await.swarm_mut().add_external_address(rendezvous_addr.clone());
        println!("rendezvous_addr: {}", rendezvous_addr);

        // Dial the rendezvous server if not already connected.
        self.client.register(rendezvous_peer, rendezvous_addr.clone(), namespace).await?;

        // Once connected, register.

        Ok(())
    }

    // Updating the HashMap of midcid to let Inbound Request been fulfilled
    pub async fn register_midcid_memory(&mut self, mid_as_hex: String, path_dir: PathBuf, cid_as_hex: String) -> Result<(), Box<dyn Error + Send + Sync>>{
       
        let path = format!("{}{}.bin", path_dir.display(), cid_as_hex);
        let path_cid = PathBuf::from(path);
        {
            let mut mirroring_midcid = self.midcid_memory_register.lock().await;
            mirroring_midcid.insert(mid_as_hex.clone(), (path_cid, cid_as_hex));
        }

        // pushing the brand new MID to the DHT
        let _publish_dht= spread_into_network(&mut self.client,mid_as_hex.to_string())
            .await
            .map_err(|e| -> Box<dyn Error + Send + Sync> { e.into() })?;
       
        Ok(())
    }

    pub async fn pylon_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        let guard = self.pylon_face_a_peers.lock().await;
        let mut out = Vec::new();
        for (peer, addrs) in guard.iter() {
            for addr in addrs {
                out.push((*peer, addr.clone()));
            }
        }
        out
    }

    fn is_pylon_face_a_protocol(protocol_version: &str) -> bool {
        protocol_version == "/pylon-face-a/0.1.0"
    }

    
}
    // TO DEL Discover peers via a rendezvous server.
    // namespace: the namespace to query.
    // rendezvous_peer: PeerId of the rendezvous server.
    // pub async fn discover_rendezvous(&mut self, namespace: &'static str, rendezvous_peer: PeerId) -> Result<(), Box<dyn Error + Send + Sync>> {
    //     let ns = rendezvous::Namespace::from_static(namespace);
    //     self.event_loop.lock().await.swarm_mut().behaviour_mut().rendezvous.discover(Some(ns), None, None, rendezvous_peer);
    //     Ok(())
    // }

