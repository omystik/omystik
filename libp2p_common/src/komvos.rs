use anyhow::{anyhow, Error as AnyhowError, Context};
use content_hashing::{
    blakecheck::{ receive_file_with_hash, receive_key },
    get_storage_dir};

// STORAGE_DIR
// use std::io::Write;
use libp2p::kad::KBucketKey;
use tracing::{debug, error, info};

use std::{
    collections::{HashMap, hash_map}, path::PathBuf, sync::Arc
};

use futures::{
    Stream as FuturesStream, StreamExt, channel::{self, mpsc::{self, channel}, oneshot}, prelude::*
};

use libp2p::{
    identify,
    identity,
    kad,
    futures::{AsyncWriteExt, AsyncReadExt, future::BoxFuture,},
    Multiaddr,
    multiaddr::Protocol,
    PeerId,
    ping,
    relay,
    rendezvous,
    request_response,
    StreamProtocol,
    Stream,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    
};

use libp2p_stream as stream;
//use rand::RngCore;
use tokio::{task::JoinHandle, sync::Mutex, time::{self, Duration}};

use tokio_util::compat::FuturesAsyncReadCompatExt; // try to make compatibility between tokio and libp2p stream

/// ---exchange---
/// file
use crate::node_primary::{FILE_EX_PROTOCOL, KEY_RAW_PROTOCOL};
const FILE_PROTOCOL : StreamProtocol = StreamProtocol::new(FILE_EX_PROTOCOL);

/// key
const KEY_PROTOCOL : StreamProtocol = StreamProtocol::new(KEY_RAW_PROTOCOL);

// ---pylon face a (server)---
use crate::{jobscodec::{
    JobsCodec, JobsReq, JobsResp,
    read_frame_blob, write_frame_blob,
}, node_primary::NodeType}; // job side for pylon exchange
use mesh_gateway_wire::{
    FaceABlobHeader, FaceARequest, FaceAResponse, decode, encode,
    blake3_blob_id, JobStore
};

use meshes_config::gateway_config::{FACE_A_BLOBS_PROTOCOL, FACE_A_JOBS_PROTOCOL};

const PYLON_BLOBS_PROTOCOL: StreamProtocol = StreamProtocol::new(FACE_A_BLOBS_PROTOCOL);
const PYLON_JOBS_PROTOCOL: StreamProtocol = StreamProtocol::new(FACE_A_JOBS_PROTOCOL);

// --- blob storage
// pub const BLOB_STORAGE_DIR: &str = "./data/blob/";
pub fn get_storage_blob() -> String {
    std::env::var("BLOB_STORAGE_DIR").unwrap_or_else(|_| "./data/blob/".to_string())
}


/// We define a "flattened" event type so that we can handle both in one `SwarmEvent`.
/// [`NetworkBehaviour`] Composite network behaviour that combines relay, ping, identify, Kademlia (DHT), Streams.
/// and a stream-based large data exchange and small data request response protocol 
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CompositeEvent")]
pub struct CompositeBehaviour {
    pub relay: relay::Behaviour,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub rendezvous_server: rendezvous::server::Behaviour,
    pub stream: stream::Behaviour,
    pub jobs_rr: request_response::Behaviour<JobsCodec>,
}

/// Unified event type combining all protocols.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum CompositeEvent {
    Relay(relay::Event),
    Ping(ping::Event),
    Identify(identify::Event),
    Jobs(request_response::Event<JobsReq, JobsResp>),
    Kademlia(kad::Event),
    RendezvousClient(rendezvous::client::Event),
    RendezvousServer(rendezvous::server::Event),
    // Note: The stream behaviour does not emit events by default.
    Stream(()),

}

impl From<relay::Event> for CompositeEvent {
    fn from(event: relay::Event) -> Self {
        CompositeEvent::Relay(event)
    }
}

impl From<ping::Event> for CompositeEvent {
    fn from(event: ping::Event) -> Self {
        CompositeEvent::Ping(event)
    }
}

impl From<identify::Event> for CompositeEvent {
    fn from(event: identify::Event) -> Self {
        CompositeEvent::Identify(event)
    }
}

impl From<kad::Event> for CompositeEvent {
    fn from(event: kad::Event) -> Self {
        CompositeEvent::Kademlia(event)
    }
}

impl From<rendezvous::client::Event> for CompositeEvent {
    fn from(event: rendezvous::client::Event) -> Self {
        CompositeEvent::RendezvousClient(event)
    }
}

impl From<rendezvous::server::Event> for CompositeEvent {
    fn from(event: rendezvous::server::Event) -> Self {
        CompositeEvent::RendezvousServer(event)
    }
}
impl From<()> for CompositeEvent {
    fn from(_: ()) -> Self {
        CompositeEvent::Stream(())
    }
}

impl From<request_response::Event<JobsReq, JobsResp>> for CompositeEvent {
    fn from(event: request_response::Event<JobsReq, JobsResp>) -> Self {
        CompositeEvent::Jobs(event)
    }
}

/// Commands sent from `Client` to the `EventLoop`.
enum Command {
    // Pin ack (gateway > pin > gateway)
    AckPinned {
        pylon: PeerId,
        key_id: [u8; 32],
        blob_id: [u8; 32],
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Advertise{
        peer_id: PeerId,
        address: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    GetProviders {
        file_name: String,
        sender: oneshot::Sender<Result<HashMap<PeerId, Vec<Multiaddr>>, AnyhowError>>,
    },
    // pylon job
    JobsRequest {
        pylon: PeerId,
        req: FaceARequest,
        sender: oneshot::Sender<Result<FaceAResponse, AnyhowError>>,
    },
    PutBlob {
        pylon_peer: PeerId,
        blob_id: [u8; 32],
        data: Vec<u8>,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Register{
        peer_id: PeerId,
        peer_addr: Multiaddr,
        namespace: &'static str,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    // Stream-based file request
    RequestFile {
        peer: PeerId,
        file_name: String,
        sender: oneshot::Sender<Result<(Vec<u8>, String), AnyhowError>>,
    },
    // Stream-based key request
    RequestKey {
        peer: PeerId,
        key_type: String,
        sender: oneshot::Sender<Result<(Vec<u8>, String), AnyhowError>>,
    },
    // pylon blob
    RequestBlob {
        pylon_peer: PeerId,
        blob_id: [u8; 32],
        sender: oneshot::Sender<Result<Vec<u8>, AnyhowError>>,
    },
    SetJobsHandler {
        handler: Option<
            Arc<
                dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse>
                    + Send
                    + Sync,
            >,
        >,
    },
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    StartProviding {
        file_name: String,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
}

/// Events emitted by the `EventLoop` to user-land.
#[derive(Debug)]
pub enum Event {
    Discovered {
        peer: PeerId,
    },
    /// A peer opened a stream requesting a file, blob
    InboundFileRequest {
        peer: PeerId,
        file_name: String,
        stream: Stream,
    },
    InboundKeyRequest {
        peer: PeerId,
        key_type: String,
        stream: Stream,
    },
    InboundBlobRequest {
        peer: PeerId,
        header: FaceABlobHeader,
        data: Option<Vec<u8>>,
        stream: Option<Stream>,
    },
    PeerIdentified {
        peer: PeerId,
        protocol_version: String,
        agent_version: String,
        listen_addrs: Vec<Multiaddr>,
    },
}

/// Creates the network components, namely:
///
/// 1. The [`Client`] to interact with the network from anywhere in your app.
/// 2. The [`Event`] stream, e.g. for inbound connections.
/// 3. The [`EventLoop`] task driving the network itself.

pub async fn new(
    secret_key_seed: Option<u8>,
    node_type: NodeType,
) -> Result<(Client, impl FuturesStream<Item = Event>, EventLoop), AnyhowError> {
    // Create key pair from seed (or generate one).
    let id_keys = match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            identity::Keypair::ed25519_from_bytes(bytes).unwrap()
        },
        None => identity::Keypair::generate_ed25519(),
    };
    let peer_id = id_keys.public().to_peer_id();

    let identify_node_protocol = match node_type{
        NodeType::Autonomos => "/autonomos/0.1.0",
        NodeType::Kentr => "/kentr/0.1.0",
        NodeType::Syndesmos => "/syndesmos/0.1.0",
        NodeType::Ypolo => "/ypolo-face-a/0.1.0",
    }
    .to_string();

    // Build a Swarm that uses QUIC and our composite behaviour.
    let swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let relay_behaviour = relay::Behaviour::new(key.public().to_peer_id(), Default::default());
            let ping_behaviour = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));
            let identify_behaviour = identify::Behaviour::new(identify::Config::new(identify_node_protocol, key.public()));
            let store = kad::store::MemoryStore::new(key.public().to_peer_id());
            let mut kademlia_behaviour = kad::Behaviour::new(peer_id, store);
            kademlia_behaviour.set_mode(Some(kad::Mode::Server));
            let rendezvous_client_behaviour = rendezvous::client::Behaviour::new(key.clone());
            let rendezvous_server_behaviour = rendezvous::server::Behaviour::new(rendezvous::server::Config::default());
            let stream_behaviour = stream::Behaviour::new();
            let jobs_rr = request_response::Behaviour::with_codec(
                JobsCodec,
                std::iter::once((PYLON_JOBS_PROTOCOL, libp2p::request_response::ProtocolSupport::Full)),
                request_response::Config::default(),
            );
            CompositeBehaviour {
                relay: relay_behaviour,
                ping: ping_behaviour,
                identify: identify_behaviour,
                kademlia: kademlia_behaviour,
                stream: stream_behaviour,
                rendezvous_client: rendezvous_client_behaviour,
                rendezvous_server: rendezvous_server_behaviour,
                jobs_rr,
            }
        })?
        .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // Set up channels for client commands and event notifications.
    let (command_sender, command_receiver) = mpsc::channel(32);
    let (event_sender, event_receiver) = mpsc::channel(32);

    // Create the shared variable for the actual listen address.
    let actual_listen_addr: Arc<Mutex<Option<Multiaddr>>> = Arc::new(Mutex::new(None));

    Ok((
        Client { sender: command_sender },
        event_receiver,
        EventLoop::new(node_type, swarm, command_receiver, event_sender, actual_listen_addr.clone()),
    ))
}

/// The client to control the network layer from your application.
#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Command>,
}

impl Client {
    /// Listen for incoming connections on the given address.
    pub async fn start_listening(
        &mut self,
        addr: Multiaddr,
    ) -> Result<(), AnyhowError> {

        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { addr, sender })
            .await
            .map_err(|e| anyhow!("Failed to send StartListening command: {}", e))?;

        receiver
            .await
            .map_err(|e| anyhow!("StartListening command failed to receive response: {}", e))?
    }

    /// Advertising the local address to the DHT
    pub async fn advertise(
        &mut self,
        peer_id: PeerId,
        address: Multiaddr,
        ) -> Result<(), AnyhowError> {
            let (sender, receiver) = oneshot::channel();
            self.sender
                .send(Command::Advertise { peer_id, address, sender })
                .await
                .map_err(|e| anyhow!("Failed to send Advertising command: {}", e))?;
            receiver.await.map_err(|e| anyhow!("Advertising command failed: {}", e))?
    }

    /// Register helper
    pub async fn register(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
        namespace: &'static str,
    ) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Register {
                peer_id,
                peer_addr,
                namespace,
                sender,
            })
            .await
            .map_err(|e| anyhow!("Failed to send Register command: {}", e))?;
        receiver
            .await
            .map_err(|e| anyhow!("Register command failed to receive response: {}", e))?
    }

    /// Dial the given peer at the given address.
    pub async fn dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .map_err(|e| anyhow!("Failed to send Dial command: {}", e))?;
        receiver
            .await
            .map_err(|e| anyhow!("Dial command failed to receive response: {}", e))?
    }

    /// Advertise the local node as the provider of the given file on the DHT.
    pub async fn start_providing(&mut self, file_name: String
        ) -> Result<(), AnyhowError> {
        
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartProviding { file_name, sender })
            .await
            .map_err(|e| anyhow!("Failed to send StartProviding command: {}", e))?;
        receiver
            .await
            .map_err(|e| anyhow!("StartProviding command failed to receive response: {}", e))?
    }

   
    /// Find providers for the given file on the DHT.
    pub async fn get_providers(&mut self, file_name: String) -> Result<HashMap<PeerId, Vec<Multiaddr>>, AnyhowError> {
        // Create a oneshot channel to receive the provider map.
        let (sender, receiver) = oneshot::channel();

        // Send the GetProviders command along with the sender half.
        self.sender
            .send(Command::GetProviders { file_name, sender })
            .await
            .map_err(|e| anyhow!("Failed to send GetProviders command: {}", e))?;
        
        // Await the result from the event handler.
        receiver
            .await
            .map_err(|e| anyhow!("GetProviders command failed to receive response: {}", e))?
    }

    // ------------------------------------------------------------------------
    // Below are simple examples of "stream-based" requests for a file.
    // Own logic:
    //   1) Open a stream.
    //   2) Send a "request" (e.g. the file_name).
    //   3) Receive the file's contents (and/or partial chunks).
    //   4) Possibly chunk large data, handle backpressure, etc.
    // ------------------------------------------------------------------------

    /// Open a stream to request file data from `peer`.
    /// Returns the raw bytes of the file.
    pub async fn request_file(
        &mut self,
        peer: PeerId,
        file_name: String,
    ) -> Result<(Vec<u8>, String), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestFile { peer, file_name, sender })
            .await
            .unwrap();
        receiver.await.unwrap()
    }

    /// Open a stream to request key from `peer`.
    pub async fn request_key(
        &mut self,
        peer: PeerId,
        key_type: String,
    ) -> Result<(Vec<u8>, String), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestKey { peer, key_type, sender })
            .await
            .unwrap();
        receiver.await.unwrap()
    }


    pub async fn get_blob(&mut self, pylon_peer: PeerId, blob_id: [u8; 32]) -> Result<Vec<u8>, AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Command::RequestBlob { pylon_peer, blob_id, sender }).await
            .map_err(|e| anyhow!("send RequestBlob failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("RequestBlob oneshot failed: {e}"))?
    }

    pub async fn put_blob(&mut self, pylon_peer: PeerId, blob_id: [u8; 32], data: Vec<u8>) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Command::PutBlob { pylon_peer, blob_id, data, sender }).await
            .map_err(|e| anyhow!("send PutBlob failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("PutBlob oneshot failed: {e}"))?
    }

    pub async fn jobs_request(
        &mut self,
        pylon: PeerId,
        req: FaceARequest,
    ) -> Result<FaceAResponse, AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::JobsRequest { pylon, req, sender })
            .await
            .map_err(|e| anyhow!("JobsRequest send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("JobsRequest oneshot failed: {e}"))?
    }

    pub async fn set_jobs_handler(
        &mut self,
        handler: Option<
            Arc<
                dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Result<(), AnyhowError> {
        self.sender
            .send(Command::SetJobsHandler { handler })
            .await
            .map_err(|e| anyhow!("Failed to send SetJobsHandler command: {e}"))?;
        Ok(())
    }

    pub async fn ack_pinned(
        &mut self,
        pylon: PeerId,
        key_id: [u8; 32],
        blob_id: [u8; 32],
    ) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::AckPinned {
                pylon,
                key_id,
                blob_id,
                sender
            })
            .await
            .map_err(|e| anyhow!("send AckPinned failed: {e}"))?;
        
        receiver.await.map_err(|e| anyhow!("AckPinned response failed: {e}"))?
    }

}

/// Main event loop that drives the Swarm and handles commands from the `Client`.
pub struct EventLoop {
    node_type: NodeType,
    swarm: Swarm<CompositeBehaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    pub actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,
    jobs_handler: Option<
        Arc<dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync>,
    >,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<(), AnyhowError>>>,
    pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<(), AnyhowError>>>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<Result<HashMap<PeerId, Vec<Multiaddr>>, AnyhowError>>>,
    pending_register: HashMap<PeerId, (&'static str, oneshot::Sender<Result<(), AnyhowError>>)>,
    pending_periodic_discovery: HashMap<PeerId, rendezvous::Namespace>,
    pending_jobs: HashMap<request_response::OutboundRequestId, oneshot::Sender<Result<FaceAResponse, AnyhowError>>>,
    rendezvous_cookie: HashMap<PeerId, rendezvous::Cookie>,
    _pending_ackpinned: HashMap<request_response::OutboundRequestId, oneshot::Sender<Result<(), AnyhowError>>>,
}

impl EventLoop {
    fn new(
        node_type: NodeType,
        swarm: Swarm<CompositeBehaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,
    ) -> Self {

        Self {
            node_type,
            swarm,
            command_receiver,
            event_sender,
            actual_listen_addr,
            jobs_handler: None,
            pending_dial: Default::default(),
            pending_start_providing: Default::default(),
            pending_get_providers: Default::default(),
            pending_register: Default::default(),
            pending_periodic_discovery: Default::default(),
            pending_jobs: Default::default(),
            rendezvous_cookie: Default::default(),
            _pending_ackpinned: Default::default(),
        }
    }

    /// Spawns the event loop on a tokio task.  
    /// Or you can just call `run()` directly.
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    pub fn swarm_mut(&mut self) -> &mut libp2p::Swarm<CompositeBehaviour> {
        &mut self.swarm
    }

    pub async fn run(&mut self) {

        // ---- persistence ----
        let storage_blob_path = PathBuf::from(get_storage_blob());

        let store_job = match JobStore::open(storage_blob_path.clone())
            .await
            .with_context(|| format!("open JobStore at {}", storage_blob_path.display()))
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("{e:#}");
                return;
            }
        };

        // Syndesmos <> Autonomos
        let mut incoming_files = match self.swarm.behaviour().stream.new_control().accept(FILE_PROTOCOL){

            Ok(s) => s,
            Err(e) => {
                tracing::error!("accept file protocol failed: {e}");
                return; // or continue; depending on where you are
            }
        };

        // Mesh A <> Autonomos
        let mut incoming_key = match self.swarm.behaviour().stream.new_control().accept(KEY_PROTOCOL){

            Ok(s) => s,
            Err(e) => {
                tracing::error!("accept key protocol failed: {e}");
                return; // or continue; depending on where you are
            }
        };

        // Autonomous <> Pylon

        let mut incoming_blobs = match self.swarm.behaviour().stream.new_control().accept(PYLON_BLOBS_PROTOCOL){
            
            Ok(s) => s,
            Err(e) => {
                tracing::error!("accept blob protocol failed: {e}");
                return; // or continue; depending on where you are
            }
        };

        let (pin_ack_tx, mut pin_ack_rx) = channel::<(PeerId, [u8; 32], [u8; 32])>(64);

        // spawn archeio loop
        let event_sender_clone_archeio = self.event_sender.clone();
        tokio::spawn(async move {
            debug!("Listening for incoming_files protocol streams.");
            while let Some((peer, stream)) = incoming_files.next().await {
                debug!("Received new stream from peer: {}", peer);
                tokio::spawn(Self::handle_inbound_stream_archeio(peer, stream, event_sender_clone_archeio.clone()));
            }
        });

        // spawn key loop
        let event_sender_clone_key = self.event_sender.clone();
        tokio::spawn(async move {
            debug!("Listening for incoming_file protocol streams focused on key.");
            while let Some((peer, stream)) = incoming_key.next().await {
                debug!("Received new stream from peer: {}", peer);
                tokio::spawn(Self::handle_inbound_stream_key(peer, stream, event_sender_clone_key.clone()));
            }
        });
    

        // spawn blob loop
        let store_job_for_blob_loop = store_job.clone();
        let pin_act_tx2 = pin_ack_tx.clone();
        let event_sender_clone_blob = self.event_sender.clone();
        tokio::spawn(async move {
            debug!("Listening for incoming_blobs protocol streams.");
            while let Some((peer, stream)) = incoming_blobs.next().await {
                debug!("Received new stream from peer: {}", peer);
                let store2 = store_job_for_blob_loop.clone();
                let pin_act_tx3 = pin_act_tx2.clone();
                let ev2 = event_sender_clone_blob.clone();
                tokio::spawn(Self::handle_inbound_stream_blob(peer, stream, ev2, pin_act_tx3, store2));
            }
        });

        

        // periodic discovery interval.
        let mut discovery_interval = time::interval(Duration::from_secs(20));
        
        loop {
            tokio::select! {
                // Drive the swarm forward.
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                },

                // Handle user commands.
                cmd = self.command_receiver.next() => match cmd {
                    Some(c) => self.handle_command(c).await,
                    None => return, // channel closed => shutdown
                },
                maybe_pin = pin_ack_rx.next() => {
                    
                    if let Some((pylon, key_id, blob_id)) = maybe_pin {
                        if let Err(e) = self.send_ack_pinned(pylon, key_id, blob_id).await {
                            debug!("AckPinned failed: {e}");
                        }
                    }
                },
                // periodic discovery
                _ = discovery_interval.tick() => {

                    // iterate over all registrated periodic discovery entries.
                    for (&peer_id, ns) in &self.pending_periodic_discovery {

                        let cookie = self.rendezvous_cookie.get(&peer_id).cloned();
                        
                        let _periodic_discovery = self.swarm.behaviour_mut().rendezvous_client.discover(
                                Some(ns.clone()),
                                cookie,
                                None,
                                peer_id,
                            );
                            tracing::info!("Periodic discovery query initiated on rendezvous node {}", peer_id);
                    }
                }
            }
        }
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.swarm.local_peer_id().clone()
    }

    /// Returns the known multiaddresses for the given `peer_id` by iterating over the routing table.
    pub fn get_peer_addresses(&mut self, target: &PeerId) -> Vec<Multiaddr> {
        
        // Convert your target PeerId into a KBucket key.
        let target_key = KBucketKey::new(target.to_bytes());
        
        let mut addrs = Vec::new();

        // Iterate over all buckets in the routing table.
        for bucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            // Each bucket contains entries that have a key and associated addresses.
            for entry in bucket.iter() {
                // Compare the entry's peer id with the target.
                if *entry.node.key == target_key {
                    // `entry.node.value()` is typically a collection of Multiaddrs.
                    // The API might vary; adjust as needed.
                    addrs.extend(entry.node.value.iter().cloned());
                }
            }
        }
        addrs
    }

    /// reading from an inbound stream and writing a response.
    async fn handle_inbound_stream_archeio(
        peer: PeerId,
        mut stream: Stream,
        mut event_sender: mpsc::Sender<Event>,
    ) {
        // archeio flow:
        //   1) read the "file name" the remote wants
        //   2) find it on disk
        //   3) chunk & send it
        
        // read the file name from the substream
        debug!("Inbound: Attempting to read from the stream...");

        let mut buf = Vec::new();

        if let Err(e) = stream.read_to_end(&mut buf).await {
            debug!("Inbound: Failed to read inbound stream: {e}");
            return;
        } else {
            debug!("Inbound: Successfully read from the stream. {}.", buf.len());
        }
        let file_name = String::from_utf8_lossy(&buf).to_string();
        debug!("Inbound: Writing file data for '{file_name}'...");

        // we pass the stream & file-name to the user-land as an event (lib.rs).
        debug!("Emitting InboundFileRequest event for file '{file_name}' from peer {peer}");
        if let Err(e) = event_sender.send(Event::InboundFileRequest { peer, file_name, stream }).await {
            debug!("Failed to send inbound stream event: {e}");
        }
    }

    /// reading from an inbound stream and writing a response.
    async fn handle_inbound_stream_key(
        peer: PeerId,
        mut stream: Stream,
        mut event_sender: mpsc::Sender<Event>,
    ) {
        // key flow:
        //   1) read the "key name" the remote wants
        //   2) find it on disk
        //   3) chunk & send it
        
        // read the key name from the substream
        debug!("Inbound: Attempting to read from the stream...");

        let mut buf = Vec::new();

        if let Err(e) = stream.read_to_end(&mut buf).await {
            debug!("Inbound: Failed to read inbound stream: {e}");
            return;
        } else {
            debug!("Inbound: Successfully read from the stream. {}.", buf.len());
        }
        let key_type = String::from_utf8_lossy(&buf).to_string();
        debug!("Inbound: Writing key data for '{key_type}'...");

        // we pass the stream & key-name to the user-land as an event (lib.rs).
        debug!("Emitting InboundKeyRequest event for key '{key_type}' from peer {peer}");
        if let Err(e) = event_sender.send(Event::InboundKeyRequest { peer, key_type, stream }).await {
            debug!("Failed to send inbound stream event: {e}");
        }
    }

    async fn handle_inbound_stream_blob(
        peer: PeerId,
        mut stream: Stream,
        mut event_sender: mpsc::Sender<Event>,
        mut pin_ack_tx: channel::mpsc::Sender<(PeerId, [u8; 32], [u8; 32])>,
        store: JobStore,
    ) {
        // 1) read framed blob header (on the full stream, no split)
        let hdr_bytes = match read_frame_blob(&mut stream).await {
            Ok(b) => b,
            Err(e) => {
                debug!("Blob inbound: failed to read header frame from {peer}: {e}");
                return;
            }
        };

        let hdr: FaceABlobHeader = match decode(&hdr_bytes) {
            Ok(h) => h,
            Err(e) => {
                debug!("Blob inbound: decode FaceABlobHeader failed from {peer}: {e}");
                // best-effort error ack
                let ack = FaceABlobHeader::Ack { ok: false, message: Some(format!("bad header: {e}")) };
                if let Ok(bytes) = encode(&ack) {
                    let _ = write_frame_blob(&mut stream, &bytes).await;
                    let _ = stream.close().await;
                }
                return;
            }
        };

        match hdr.clone() {
            FaceABlobHeader::Put { blob_id, size } => {
                let size_usize = match usize::try_from(size) {
                    Ok(s) => s,
                    Err(_) => {
                        let ack = FaceABlobHeader::Ack {
                            ok: false,
                            message: Some("size too large".into()),
                        };
                        if let Ok(bytes) = encode(&ack) {
                            let _ = write_frame_blob(&mut stream, &bytes).await;
                            let _ = stream.close().await;
                        }
                        return;
                    }
                };

                let mut buf = vec![0u8; size_usize];
                if let Err(e) = stream.read_exact(&mut buf).await {
                    debug!("Blob inbound: read_exact({size_usize}) failed from {peer}: {e}");

                    let ack = FaceABlobHeader::Ack {
                        ok: false,
                        message: Some(format!("read blob bytes failed: {e}")),
                    };

                    if let Ok(bytes) = encode(&ack) {
                        let _ = write_frame_blob(&mut stream, &bytes).await;
                        let _ = stream.close().await;
                    }
                    return;
                }

                debug!(
                    "Blob inbound: raw Put blob_id={:?} size={} from {}",
                    blob_id, size, peer
                );

                let computed_blob_id = blake3_blob_id(&buf);
                if computed_blob_id != blob_id {
                    let ack = FaceABlobHeader::Ack {
                        ok: false,
                        message: Some(format!(
                            "blob_id mismatch: header={}, computed={}",
                            hex::encode(blob_id),
                            hex::encode(computed_blob_id)
                        )),
                    };

                    if let Ok(bytes) = encode(&ack) {
                        let _ = write_frame_blob(&mut stream, &bytes).await;
                    }
                    let _ = stream.close().await;
                    return;
                }

                if !store.blob_exists(&blob_id).await {
                    if let Err(e) = store.write_blob(&blob_id, &buf).await {
                        let ack = FaceABlobHeader::Ack {
                            ok: false,
                            message: Some(format!("store blob failed: {e}")),
                        };

                        if let Ok(bytes) = encode(&ack) {
                            let _ = write_frame_blob(&mut stream, &bytes).await;
                        }
                        let _ = stream.close().await;
                        return;
                    }
                }

                let _ = event_sender
                    .send(Event::InboundBlobRequest {
                        peer,
                        header: FaceABlobHeader::Put { blob_id, size },
                        data: Some(buf),
                        stream: None,
                    })
                    .await;

                let ack = FaceABlobHeader::Ack {
                    ok: true,
                    message: None,
                };

                if let Ok(bytes) = encode(&ack) {
                    let _ = write_frame_blob(&mut stream, &bytes).await;
                }

                let _ = stream.close().await;
            }

            FaceABlobHeader::Get { blob_id } => {
                debug!("Blob inbound: Get blob_id={:?} from {peer}", blob_id);

                // We DO NOT reply here. We hand the *same* stream to user-land.
                // User-land must:
                //   1) write_frame_blob(Ack ok=true/false)
                //   2) if ok=true, write_all(blob_bytes)
                //   3) close
                if let Err(e) = event_sender
                    .send(Event::InboundBlobRequest {
                        peer,
                        header: FaceABlobHeader::Get { blob_id },
                        data: None,
                        stream: Some(stream), // <-- critical
                    })
                    .await
                {
                    debug!("Blob inbound: failed to emit Get event to user-land: {e}");

                    // If user-land can't receive it, best-effort error ack to avoid remote hang:
                    // (we no longer have the stream because send failed *without moving it* only if we keep it;
                    // but here it WOULD have moved. So we only do this if you want: restructure to try_send + fallback.)
                }
            }

            FaceABlobHeader::Ack { .. } => {
                // invalid inbound request
                let ack = FaceABlobHeader::Ack { ok: false, message: Some("client sent Ack as request".into()) };
                if let Ok(bytes) = encode(&ack) {
                    let _ = write_frame_blob(&mut stream, &bytes).await;
                }
                let _ = stream.close().await;
            }
        }
    }

    // client rendezvous
    async fn handle_rendezvous_client_event(
        &mut self,
        event: rendezvous::client::Event,
    ) {
        match event {
            rendezvous::client::Event::Registered {
                ttl,
                namespace,
                rendezvous_node,
            } => {
                tracing::info!(
                    "Registered for namespace '{}' at rendezvous point {} for the next {} seconds",
                    namespace,
                    rendezvous_node,
                    ttl
                );
            }

            rendezvous::client::Event::RegisterFailed {
                rendezvous_node,
                namespace,
                error,
            } => {
                error!(
                    "Failed to register: rendezvous_node={}, namespace={}, error_code={:?}",
                    rendezvous_node,
                    namespace,
                    error
                );
            }

            rendezvous::client::Event::Discovered {
                registrations,
                cookie: new_cookie,
                rendezvous_node,
                ..
            } => {
                self.rendezvous_cookie.insert(rendezvous_node, new_cookie);

                let local_peer_id = *self.swarm.local_peer_id();

                for registration in registrations {
                    for address in registration.record.addresses() {
                        let peer = registration.record.peer_id();

                        if peer == local_peer_id {
                            continue;
                        }

                        let _ = self.event_sender.send(Event::Discovered { peer }).await;

                        let p2p_suffix = Protocol::P2p(peer);
                        let address_with_p2p =
                            if !address.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
                                address.clone().with(p2p_suffix)
                            } else {
                                address.clone()
                            };

                        let _ = self.swarm.dial(address_with_p2p.clone());

                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer, address_with_p2p);
                    }
                }
            }

            other => {
                debug!("Unhandled rendezvous client event: {:?}", other);
            }
        }
    }

    async fn handle_rendezvous_server_event(
        &mut self,
        event: rendezvous::server::Event,
    ) {
        match event {
            rendezvous::server::Event::PeerRegistered { peer, registration } => {
                tracing::info!(
                    "Peer {} registered for namespace '{}'",
                    peer,
                    registration.namespace,
                );
            }

            rendezvous::server::Event::DiscoverServed {
                enquirer,
                registrations,
            } => {
                tracing::info!(
                    "Served peer {} with {} registrations",
                    enquirer,
                    registrations.len(),
                );
            }

            other => {
                debug!("Unhandled rendezvous server event: {:?}", other);
            }
        }
    }

    async fn handle_jobs_event(
        &mut self,
        e: request_response::Event<JobsReq, JobsResp>,
    ) {
        info!(target:"facea", "Autonomos jobs rr event {:?}", e);

        match e {
            request_response::Event::Message { peer, message } => match message {
                request_response::Message::Response { request_id, response } => {
                    if let Some(tx) = self.pending_jobs.remove(&request_id) {
                        let decoded = decode::<FaceAResponse>(&response.0)
                            .map_err(|e| anyhow!("decode FaceAResponse failed: {e}"));
                        let _ = tx.send(decoded);
                    } else {
                        info!(target:"facea", %peer, ?request_id, "jobs response but but no pending sender")
                    }
                }

                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    info!(
                        target: "facea",
                        %peer,
                        ?request_id,
                        "inbound FaceA jobs request"
                    );

                    let response = match decode::<FaceARequest>(&request.0) {
                        Ok(req) => {
                            match &self.jobs_handler {
                                Some(handler) => {
                                    handler(peer, req).await
                                }
                                None => {
                                    FaceAResponse::Error {
                                        message: "no FaceA jobs handler installed on this node".into(),
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            FaceAResponse::Error {
                                message: format!("decode FaceARequest failed: {e}"),
                            }
                        }
                    };

                    let response_bytes = match encode(&response) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            let fallback = FaceAResponse::Error {
                                message: format!("encode FaceAResponse failed: {e}"),
                            };

                            match encode(&fallback) {
                                Ok(bytes) => bytes,
                                Err(_) => Vec::new(),
                            }
                        }
                    };

                    if let Err(e) = self
                        .swarm
                        .behaviour_mut()
                        .jobs_rr
                        .send_response(channel, JobsResp(response_bytes))
                    {
                        tracing::warn!(
                            target: "facea",
                            %peer,
                            ?request_id,
                            "failed to send FaceA jobs response: {:?}",
                            e
                        );
                    }
                }
            },

            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                if let Some(tx) = self.pending_jobs.remove(&request_id) {
                    let _ = tx.send(Err(anyhow!("jobs outbound failure: {error}")));
                } else {
                    info!(target:"facea", %peer, ?request_id, "")
                }
            }

            request_response::Event::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                tracing::warn!(
                    target:"facea",
                    %peer,
                    ?request_id,
                    "jobs inbound failure: {error}"
                );
            }

            request_response::Event::ResponseSent { peer, request_id } => {
                tracing::debug!(
                    target:"facea",
                    %peer,
                    ?request_id,
                    "jobs response sent"
                );
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<CompositeEvent>) {

        match event {

            SwarmEvent::Behaviour(CompositeEvent::RendezvousClient(event)) => {
                //debug!("Rendezvous client event: {:?}", event);
                self.handle_rendezvous_client_event(event).await;
            }
            SwarmEvent::Behaviour(CompositeEvent::RendezvousServer(event)) => {
                self.handle_rendezvous_server_event(event).await;
            }
            SwarmEvent::Behaviour(CompositeEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::StartProviding(_),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending_start_providing.remove(&id) {
                    let _ = sender.send(Ok(()));
                    print!("Providing request completed.");
                }
            }

            SwarmEvent::Behaviour(CompositeEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result:
                        kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                            providers,
                            ..
                        })),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    let mut provider_addrs = HashMap::new();

                    for provider in &providers {
                        let addrs = self.get_peer_addresses(provider);
                        provider_addrs.insert(*provider, addrs);
                    }

                    let _ = sender.send(Ok(provider_addrs));

                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .map(|mut q| q.finish());
                }
            }

            SwarmEvent::Behaviour(CompositeEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::GetProviders(Ok(
                        kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. }
                    )),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    let _ = sender.send(Ok(HashMap::new()));
                }
            }

            SwarmEvent::Behaviour(CompositeEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::GetProviders(Err(e)),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    let _ = sender.send(Err(anyhow!("get_providers failed: {}", e)));
                }
            }

            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                let full_addr = address.with(Protocol::P2p(local_peer_id));

                *self.actual_listen_addr.lock().await = Some(full_addr.clone());
                println!("Local node is listening on {:?}", full_addr);
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                if endpoint.is_dialer() {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }

                if let Some((namespace, reg_sender)) = self.pending_register.remove(&peer_id) {
                    if matches!(self.node_type, NodeType::Syndesmos) {
                        let _ = reg_sender.send(Err(anyhow!(
                            "Syndesmos is the rendezvous server; it should not register as a rendezvous client"
                        )));
                        return;
                    }

                    let ns = rendezvous::Namespace::from_static(namespace);

                    let register_result = self
                        .swarm
                        .behaviour_mut()
                        .rendezvous_client
                        .register(ns.clone(), peer_id, None)
                        .map_err(|e| e.into());

                    if let Err(ref error) = register_result {
                        error!(
                            "Failed to register with rendezvous node {}: {}",
                            peer_id, error
                        );
                    } else {
                        let _initial_discovery = self
                            .swarm
                            .behaviour_mut()
                            .rendezvous_client
                            .discover(Some(ns.clone()), None, None, peer_id);

                        self.pending_periodic_discovery.insert(peer_id, ns);
                    }

                    let _ = reg_sender.send(register_result);
                }
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(anyhow!(
                            "Outgoing connection error to {}: {}",
                            peer_id,
                            error
                        )));
                    }
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                tracing::info!("Disconnected from {}", peer_id);
            }

            SwarmEvent::Behaviour(CompositeEvent::Jobs(e)) => {
                self.handle_jobs_event(e).await;
            }

            SwarmEvent::Behaviour(CompositeEvent::Identify(event)) => {
                match event {
                    identify::Event::Received { peer_id, info, .. } => {
                        // Learn this peer's listen addresses for future DHT provider dialing.
                        for addr in &info.listen_addrs {
                            let full_addr = if !addr.iter().any(|p| matches!(p, Protocol::P2p(_))) {
                                addr.clone().with(Protocol::P2p(peer_id))
                            } else {
                                addr.clone()
                            };

                            self.swarm
                                .behaviour_mut()
                                .kademlia
                                .add_address(&peer_id, full_addr);
                        }

                        // Then notify user-land.
                        let _ = self
                            .event_sender
                            .send(Event::PeerIdentified {
                                peer: peer_id,
                                protocol_version: info.protocol_version.clone(),
                                agent_version: info.agent_version.clone(),
                                listen_addrs: info.listen_addrs.clone(),
                            })
                            .await;
                    }
                    other => {
                        debug!("Identify event: {:?}", other);
                    }
                }
            }

            other => {
                debug!("Unhandled event: {:?}", other);
            }
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => {

                let result = self.swarm
                    .listen_on(addr)
                    .map(|_listener_id| ())
                    .map_err(|e| anyhow!("Failed to listen on address: {}", e));
                let _ = sender.send(result);
            }
            Command::Advertise {
                peer_id,
                address,
                sender
            } => {
                
                self.swarm.add_external_address(address.clone());
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, address);
                let _ = sender.send(Ok(()));
            }
            Command::Register {
                peer_id,
                peer_addr,
                namespace,
                sender 
            } => {
                //self.swarm.add_external_address(peer_addr.clone());

                // Dial the rendezvous server.
                let dial_result = self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id)));
                if let Err(e) = dial_result {
                    let _ = sender.send(Err(anyhow!("Failed to dial for registration: {}", e)));
                    return;
                }

                // Instead of immediately calling register, store the registration request.
                // The ConnectionEstablished event will pick it up.
                self.pending_register.insert(peer_id, (namespace, sender));
            }
            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                    
                    // Add to kademlia so we remember addresses for peer.
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, peer_addr.clone());

                    // Actually dial it:
                    let dial_res = self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id)));
                    match dial_res {
                        Ok(()) => {
                            e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(anyhow!("Failed to dial peer {}: {}", peer_id, e)));
                        }
                    }
                } else {
                    // Already dialing this peer. Possibly return an error or ignore.
                    let _ = sender.send(Err(anyhow!("Already dialing peer {}", peer_id)));
                }
            }
            Command::StartProviding { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(file_name.clone().into_bytes().into())
                    .map_err(|e| anyhow!("Failed to start providing file '{}': {}", file_name, e))
                    .expect("No store error.");
                self.pending_start_providing.insert(query_id, sender);
            }
            Command::GetProviders { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(file_name.into_bytes().into());
                self.pending_get_providers.insert(query_id, sender);
            }
            Command::RequestFile { peer, file_name, sender } => {

                let mut control = self.swarm.behaviour_mut().stream.new_control();
                let open_result = control.open_stream(peer, FILE_PROTOCOL).await;
                let res =
                    match open_result {
                        Ok(stream) => {
                            // debug!("Successfully opened stream to peer: {}", peer);
                            debug!("Writing file name '{}' to the stream...", file_name);

                            // Split the stream into read and write halves
                            let (reader, mut writer) = stream.split();

                            // Conversion from libp2p to tokio
                            let reader = reader.compat(); 
                            if let Err(e) = writer.write_all(file_name.as_bytes()).await {
                                //debug!("Failed to write file name to stream: {e}");
                                Err(anyhow!("Failed to write file name to stream: {}", e))

                            } else {

                                debug!("Written file name '{}' to stream.", file_name);
                                // 2) Shutdown the write side to signal EOF
                                if let Err(e) = writer.flush().await {
                                    //debug!("Failed to shutdown write half of the stream: {e}");
                                    Err(anyhow!("Failed to shutdown write half of the stream: {}", e))
                                    
                                } else {
                                    writer.close().await.unwrap();

                                    debug!("Write side of the stream successfully closed.");

                                    let directory_path = format!("{}", get_storage_dir());
                                    let dir_path = std::path::Path::new(&directory_path);
                                    //debug!("Calling receive_file_with_hash for file: {:?}", dir_path);

                                    match receive_file_with_hash(reader, dir_path).await {
                                        Ok(cid) => {
                                            debug!("File received successfully at {:?}", dir_path);
                                            
                                            let final_path = format!("{}{}.bin", dir_path.display(), cid);

                                            match std::fs::read(final_path) {
                                                Ok(file_bytes) => {
                                                    //debug!("File read into memory successfully. Bytes: {}", file_bytes.len());
                                                    Ok((file_bytes, cid))
                                                }
                                                Err(e) => {
                                                    //debug!("Failed to read file from disk: {e}");
                                                    Err(anyhow!("Failed to read file from disk: {}", e))
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            //debug!("Error during receive_file_with_hash: {e}");
                                            Err(anyhow!("Error during receive_file_with_hash: {}", e))
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            //debug!("Failed to open stream to peer {}: {e}", peer);
                            Err(anyhow!("Failed to open stream to peer {}: {}", peer, e))
                        }
                    };

                let _ = sender.send(res);
            }

            Command::RequestKey { peer, key_type, sender } => {

                let mut control = self.swarm.behaviour_mut().stream.new_control();
                let open_result = control.open_stream(peer, KEY_PROTOCOL).await;

                let res = match open_result {
                    Ok(stream) => {
                        debug!("Writing key name '{}' to the stream...", key_type);

                        let (reader, mut writer) = stream.split();
                        let mut reader = reader.compat();

                        if let Err(e) = writer.write_all(key_type.as_bytes()).await {
                            Err(anyhow!("Failed to write key name to stream: {}", e))
                        } else if let Err(e) = writer.flush().await {
                            Err(anyhow!("Failed to flush key name to stream: {}", e))
                        } else if let Err(e) = writer.close().await {
                            Err(anyhow!("Failed to close write half of stream: {}", e))
                        } else {
                            match receive_key(&mut reader).await {
                                Ok((key_bytes, key_name)) => Ok((key_bytes, key_name)),
                                Err(e) => Err(anyhow!("Error during receive_key: {}", e)),
                            }
                        }
                    }
                    Err(e) => Err(anyhow!("Failed to open stream to peer {}: {}", peer, e)),
                };
                let _ = sender.send(res);

            }

            Command::RequestBlob { pylon_peer, blob_id, sender } => {
                let mut control = self.swarm.behaviour_mut().stream.new_control();

                let res: Result<Vec<u8>, AnyhowError> = async {
                    let stream = control
                        .open_stream(pylon_peer, PYLON_BLOBS_PROTOCOL)
                        .await
                        .map_err(|e| anyhow!("open_stream(blob) failed: {e}"))?;

                    let (mut r, mut w) = stream.split();

                    // 1) Send framed Get header
                    let hdr = FaceABlobHeader::Get { blob_id };
                    let hdr_bytes = encode(&hdr).map_err(|e| anyhow!("encode FaceABlobHeader failed: {e}"))?;
                    write_frame_blob(&mut w, &hdr_bytes).await.map_err(|e| anyhow!("write_frame(Get) failed: {e}"))?;
                    w.flush().await.map_err(|e| anyhow!("flush(Get) failed: {e}"))?;

                    // 2) Read framed Ack
                    let ack_bytes = read_frame_blob(&mut r).await.map_err(|e| anyhow!("read_frame(Ack) failed: {e}"))?;
                    let ack: FaceABlobHeader = decode(&ack_bytes).map_err(|e| anyhow!("decode Ack failed: {e}"))?;

                    match ack {
                        FaceABlobHeader::Ack { ok: true, .. } => {
                            // 3) Read blob bytes until EOF (gateway closes after writing)
                            let mut buf = Vec::new();
                            r.read_to_end(&mut buf).await.map_err(|e| anyhow!("read_to_end(blob) failed: {e}"))?;
                            Ok(buf)
                        }
                        FaceABlobHeader::Ack { ok: false, message } => {
                            Err(anyhow!("gateway Get rejected: {}", message.unwrap_or_else(|| "unknown".into())))
                        }
                        other => Err(anyhow!("expected Ack after Get, got {other:?}")),
                    }
                }.await;

                let _ = sender.send(res);
            }

            Command::PutBlob { pylon_peer, blob_id, data, sender } => {
                let mut control = self.swarm.behaviour_mut().stream.new_control();

                let res: Result<(), AnyhowError> = async {
                    let stream = control
                        .open_stream(pylon_peer, PYLON_BLOBS_PROTOCOL)
                        .await
                        .map_err(|e| anyhow!("open_stream(blob) failed: {e}"))?;

                    let (mut r, mut w) = stream.split();

                    // 1) Send framed Put header
                    let hdr = FaceABlobHeader::Put { blob_id, size: data.len() as u64 };
                    let hdr_bytes = encode(&hdr).map_err(|e| anyhow!("encode FaceABlobHeader failed: {e}"))?;
                    write_frame_blob(&mut w, &hdr_bytes).await.map_err(|e| anyhow!("write_frame(Put) failed: {e}"))?;

                    // 2) Send EXACTLY size raw bytes (no framing)
                    w.write_all(&data).await.map_err(|e| anyhow!("write_all(blob) failed: {e}"))?;
                    w.flush().await.map_err(|e| anyhow!("flush(Put) failed: {e}"))?;
                    w.close().await.ok(); // signals EOF on write half; gateway reads exact size anyway

                    // 3) Read framed Ack
                    let ack_bytes = read_frame_blob(&mut r).await.map_err(|e| anyhow!("read_frame(Ack) failed: {e}"))?;
                    let ack: FaceABlobHeader = decode(&ack_bytes).map_err(|e| anyhow!("decode Ack failed: {e}"))?;

                    match ack {
                        FaceABlobHeader::Ack { ok: true, message } => {
                            if let Some(m) = message {
                                tracing::info!(target:"mesh_a", "PutBlob ack message: {m}");
                            }
                            Ok(())
                        }
                        FaceABlobHeader::Ack { ok: false, message } => {
                            Err(anyhow!("gateway Put rejected: {}", message.unwrap_or_else(|| "unknown".into())))
                        }
                        other => Err(anyhow!("expected Ack after Put, got {other:?}")),
                    }
                }.await;

                let _ = sender.send(res);
            }

            Command::JobsRequest { pylon, req, sender } => {
                let bytes = match encode(&req){
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender.send(Err(anyhow!("encode FaceARequest failed: {e}")));
                        return;
                    }
                };
                let rid = self.swarm.behaviour_mut().jobs_rr.send_request(&pylon, JobsReq(bytes));
                info!(target:"facea", peer=%pylon, request_id=?rid, "Autonomous jobs send_request");
                self.pending_jobs.insert(rid, sender);
            }

            Command::AckPinned { pylon, key_id, blob_id, sender } => {

                let req = FaceARequest::AckArtifactPinned { key_id, blob_id, provider: *self.swarm.local_peer_id(), };

                // encode + send exactly like JobsRequest, but we only care about "delivered to RR layer".
                let bytes = match encode(&req) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender.send(Err(anyhow!("encode AckArtifactPinned failed: {e}")));
                        return;
                    }
                };

                let rid = self.swarm
                    .behaviour_mut()
                    .jobs_rr
                    .send_request(&pylon, JobsReq(bytes));

                info!(target:"facea", peer=%pylon, request_id=?rid, "Autonomous AckPinned send_request");

                // self.pending_ackpinned.insert(rid, sender);

                let _ = sender.send(Ok(()));
            }

            Command::SetJobsHandler { handler } => {
                self.jobs_handler = handler;
            }
        }
    }


    async fn send_ack_pinned(
        &mut self,
        pylon: PeerId,
        key_id: [u8; 32],
        blob_id: [u8; 32],
    ) -> Result<(), AnyhowError> {
        let req = FaceARequest::AckArtifactPinned {
            key_id,
            blob_id,
            provider: *self.swarm.local_peer_id(),
        };

        let bytes = encode(&req).map_err(|e| anyhow!("encode AckArtifactPinned failed: {e}"))?;
        self.swarm
            .behaviour_mut()
            .jobs_rr
            .send_request(&pylon, JobsReq(bytes));

        Ok(())
    }


}