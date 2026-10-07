use anyhow::{anyhow, Error as AnyhowError};

use futures::{channel::{mpsc, oneshot}, prelude::*, Stream as FuturesStream, StreamExt, future::BoxFuture};
use libp2p::{
    identify, identity, kad, ping, relay, rendezvous,
    Multiaddr, multiaddr::Protocol, PeerId, StreamProtocol,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    request_response::{self, ProtocolSupport},
};
use std::{collections::HashMap, sync::Arc};
use tokio::{task::JoinHandle, sync::Mutex, time::{self, Duration}};
use tracing::{debug, info, error};

use mesh_gateway_wire::{decode, encode, FaceARequest, FaceAResponse};
use meshes_config::gateway_config::FACE_A_JOBS_PROTOCOL;

use libp2p_common::jobscodec::{JobsCodec, JobsReq, JobsResp};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "FaceAEvent")]
pub struct FaceABehaviour {
    pub relay: relay::Behaviour,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub jobs_rr: request_response::Behaviour<JobsCodec>,
    pub stream: libp2p_stream::Behaviour, // blob protocol will ride on this
    pub rendezvous: rendezvous::client::Behaviour,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum FaceAEvent {
    Relay(relay::Event),
    Ping(ping::Event),
    Identify(identify::Event),
    Kademlia(kad::Event),
    Jobs(request_response::Event<JobsReq, JobsResp>),
    Stream(()),
    Rendezvous(rendezvous::client::Event),
}

impl From<relay::Event> for FaceAEvent { fn from(e: relay::Event) -> Self { Self::Relay(e) } }
impl From<ping::Event> for FaceAEvent { fn from(e: ping::Event) -> Self { Self::Ping(e) } }
impl From<identify::Event> for FaceAEvent { fn from(e: identify::Event) -> Self { Self::Identify(e) } }
impl From<kad::Event> for FaceAEvent { fn from(e: kad::Event) -> Self { Self::Kademlia(e) } }
impl From<request_response::Event<JobsReq, JobsResp>> for FaceAEvent {
     fn from(e: request_response::Event<JobsReq, JobsResp>) -> Self { Self::Jobs(e) } }
impl From<()> for FaceAEvent { fn from(_: ()) -> Self { Self::Stream(()) } }
impl From<rendezvous::client::Event> for FaceAEvent {
    fn from(event: rendezvous::client::Event) -> Self {
        FaceAEvent::Rendezvous(event)
    }
}

enum Command {
    StartListening { addr: Multiaddr, sender: oneshot::Sender<Result<(), AnyhowError>> },
    Dial { peer_id: PeerId, peer_addr: Multiaddr, sender: oneshot::Sender<Result<(), AnyhowError>> },
    SetJobsHandler { handler: Option<Arc<dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync>> },
    JobsRespond { channel: request_response::ResponseChannel<JobsResp>, data: Vec<u8> },
    Register{
        peer_id: PeerId,
        peer_addr: Multiaddr,
        namespace: &'static str,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Advertise{ peer_id: PeerId, address: Multiaddr,sender: oneshot::Sender<Result<(), AnyhowError>> },
}

#[derive(Debug)]
pub enum Event {
    // currently unused; keep for future
    Discovered { peer: PeerId },
}

pub async fn new(secret_key_seed: Option<u8>) -> Result<(Client, impl FuturesStream<Item = Event>, EventLoop), AnyhowError> {
    let id_keys = match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            identity::Keypair::ed25519_from_bytes(bytes).unwrap()
        }
        None => identity::Keypair::generate_ed25519(),
    };
    let peer_id = id_keys.public().to_peer_id();

    let swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let relay_behaviour = relay::Behaviour::new(key.public().to_peer_id(), Default::default());
            let ping_behaviour = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));
            let identify_behaviour = identify::Behaviour::new(identify::Config::new("/pylon-face-a/0.1.0".to_string(), key.public()));

            let store = kad::store::MemoryStore::new(peer_id);
            let mut kademlia_behaviour = kad::Behaviour::new(peer_id, store);
            kademlia_behaviour.set_mode(Some(kad::Mode::Server));

            let jobs_rr = request_response::Behaviour::with_codec(
                JobsCodec,
                std::iter::once((StreamProtocol::new(FACE_A_JOBS_PROTOCOL), ProtocolSupport::Full)),
                request_response::Config::default(),
            );
            let stream = libp2p_stream::Behaviour::new();
            let rendezvous_behaviour = rendezvous::client::Behaviour::new(key.clone());
            Ok(FaceABehaviour {
                relay: relay_behaviour,
                ping: ping_behaviour,
                identify: identify_behaviour,
                kademlia: kademlia_behaviour,
                jobs_rr,
                stream,
                rendezvous: rendezvous_behaviour,
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(360)))
        .build();

    let (command_sender, command_receiver) = mpsc::channel(32);
    let (event_sender, event_receiver) = mpsc::channel(32);

    let actual_listen_addr: Arc<Mutex<Option<Multiaddr>>> = Arc::new(Mutex::new(None));

    Ok((
        Client { sender: command_sender.clone() },
        event_receiver,
        EventLoop::new(swarm, command_sender, command_receiver, event_sender, actual_listen_addr),
    ))
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Command>,
}

impl Client {
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Command::StartListening { addr, sender }).await
            .map_err(|e| anyhow!("Failed to send StartListening: {e}"))?;
        receiver.await.map_err(|e| anyhow!("StartListening response failed: {e}"))?
    }

    pub async fn dial(&mut self, peer_id: PeerId, peer_addr: Multiaddr) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Command::Dial { peer_id, peer_addr, sender }).await
            .map_err(|e| anyhow!("Failed to send Dial: {e}"))?;
        receiver.await.map_err(|e| anyhow!("Dial response failed: {e}"))?
    }

    pub async fn set_jobs_handler(
        &mut self,
        handler: Option<Arc<dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync>>,
    ) -> Result<(), AnyhowError> {
        self.sender.send(Command::SetJobsHandler { handler }).await
            .map_err(|e| anyhow!("Failed to send SetJobsHandler: {e}"))?;
        Ok(())
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
}

pub struct EventLoop {
    swarm: Swarm<FaceABehaviour>,
    command_sender: mpsc::Sender<Command>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    pub actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<(), AnyhowError>>>,
    jobs_handler: Option<Arc<dyn Fn(PeerId, FaceARequest) -> BoxFuture<'static, FaceAResponse> + Send + Sync>>,
    pending_register: HashMap<PeerId, (&'static str, oneshot::Sender<Result<(), AnyhowError>>)>,
    pending_periodic_discovery: HashMap<PeerId, rendezvous::Namespace>,
    rendezvous_cookie: HashMap<PeerId, rendezvous::Cookie>,
}

impl EventLoop {
    fn new(
        swarm: Swarm<FaceABehaviour>,
        command_sender: mpsc::Sender<Command>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,
    ) -> Self {
        Self {
            swarm,
            command_sender,
            command_receiver,
            event_sender,
            actual_listen_addr,
            pending_dial: Default::default(),
            jobs_handler: None,
            pending_register: Default::default(),
            pending_periodic_discovery: Default::default(),
            rendezvous_cookie: Default::default(),
        }
    }

    pub fn swarm_mut(&mut self) -> &mut libp2p::Swarm<FaceABehaviour> {
        &mut self.swarm
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn stream_control(&self) -> libp2p_stream::Control {
        self.swarm.behaviour().stream.new_control()
    }

    pub async fn run(&mut self) {

        // periodic discovery interval.
        let mut discovery_interval = time::interval(Duration::from_secs(20));

        loop {
            tokio::select! {
                // Drive the swarm forward.
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
                // Handle user commands.
                cmd = self.command_receiver.next() => match cmd {
                    Some(c) => self.handle_command(c).await,
                    None => return,
                },
                // periodic discovery
                _ = discovery_interval.tick() => {
                    // iterate over all registrated periodic discovery entries.
                    for (&peer_id, ns) in &self.pending_periodic_discovery {

                        let cookie = self.rendezvous_cookie.get(&peer_id).cloned();

                        let _periodic_discovery = self.swarm.behaviour_mut().rendezvous.discover(
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

    async fn handle_swarm_event(&mut self, event: SwarmEvent<FaceAEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let full_addr = address.clone().with(libp2p::multiaddr::Protocol::P2p(*self.swarm.local_peer_id()));
                *self.actual_listen_addr.lock().await = Some(full_addr.clone());
                info!("FaceA (server) listening on {full_addr:?}");
                
                self.swarm.add_external_address(address);
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("FaceA (server) CONNECTED to {peer_id} via {endpoint:?}");
                if endpoint.is_dialer() {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }

                if let Some((namespace, reg_sender)) = self.pending_register.remove(&peer_id){
                        let ns = rendezvous::Namespace::from_static(namespace);
                        let register_result = self.swarm.behaviour_mut().rendezvous.register(
                            ns.clone(),
                            peer_id, // Using the established peer_id as the rendezvous target.
                            None,
                        ).map_err(|e| e.into()); // Convert RegisterError into anyhow::Error
                        if let Err(ref error) = register_result {
                            error!(
                                "Failed to register with rendezvous node {}: {}",
                                peer_id,
                                error
                            );
                        } else {
                            //tracing::info!("Successfully registered with rendezvous node {}", peer_id);
                            // Discard the result from the initial discovery call.
                            let _initial_discovery = self.swarm.behaviour_mut().rendezvous.discover(
                                Some(ns.clone()),
                                None,
                                None,
                                peer_id,
                            );
                            //tracing::info!("Discovery query initiated on rendezvous node {}", peer_id);

                            // Store for periodic discovery.
                            self.pending_periodic_discovery.insert(peer_id, ns);
                        }
                        
                        // Inform the pending registration command about the result of registration.
                        let _ = reg_sender.send(register_result);
                    }
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                tracing::info!("Disconnected from {}", peer_id);
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(anyhow!("Outgoing connection error to {peer_id}: {error}")));
                    }
                }
            }

            SwarmEvent::Behaviour(FaceAEvent::Jobs(ev)) => {
                match ev {
                    request_response::Event::Message { peer, message } => match message {
                        request_response::Message::Request { request, channel, .. } => {

                            let handler = self.jobs_handler.clone();
                            let mut tx = self.command_sender.clone();
                            
                            tokio::spawn(async move {
                                let resp = match handler {
                                    Some(h) => match decode::<FaceARequest>(&request.0) {
                                     
                                            Ok(req) => (h)(peer, req).await,
                                            Err(e) => FaceAResponse::Error {
                                                message: format!("decode FaceARequest: {e}")
                                        },
                                    }
                                    None => FaceAResponse::Error {
                                        message: "no jobs handler".into()
                                    },
                                };

                                let bytes = match encode(&resp) {
                                    Ok(b) => b,
                                    Err(e) =>{
                                        let fallback = FaceAResponse::Error {
                                            message: format!("encode FaceAResponse failed: {e}"),
                                        };
                                        encode(&fallback).unwrap_or_default()
                                }
                            };

                            let _ = tx.send(Command::JobsRespond { channel, data: bytes }).await;
                            });
                        }

                        request_response::Message::Response { request_id: _, response: _ } => {
                            // Gateway FaceA server doesn't initiate outbound in this minimal implementation.
                        }
                    },
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        debug!("Jobs inbound failure from {peer}: {error}");
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        debug!("Jobs response sent to {peer}");
                    }
                    request_response::Event::OutboundFailure { request_id: _, error, .. } => {
                        debug!("Jobs outbound failure: {error}");
                    }
                }
            }

            SwarmEvent::Behaviour(FaceAEvent::Rendezvous(event)) => {
                //debug!("Rendezvous client event: {:?}", event);

                match event { 
                    rendezvous::client::Event::Registered {
                        ttl,
                        namespace,
                        rendezvous_node } => { 
                            tracing::info!("Registered for namespace '{}' at rendezvous point {} for the next {} seconds",
                            namespace,
                            rendezvous_node,
                            ttl
                        );
                    },
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
                    },
                    rendezvous::client::Event::Discovered {
                        registrations,
                        cookie: new_cookie,
                        rendezvous_node,
                        ..
                    } => {

                        self.rendezvous_cookie.insert(rendezvous_node, new_cookie);

                        let local_peer_id = self.swarm.local_peer_id().clone();

                        for registration in registrations {
                            for address in registration.record.addresses() {

                                let peer = registration.record.peer_id();
                                //tracing::info!(%peer, %address, "Discovered peer");

                                if peer == local_peer_id {
                                    //tracing::info!(%peer, "Skipping dialing self");
                                    continue;
                                }

                                // tell GatewayManager about this peer
                                let _ = self.event_sender.send(Event::Discovered { peer }).await;

                                let p2p_suffix = Protocol::P2p(peer);
                                let address_with_p2p =
                                    if !address.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
                                        address.clone().with(p2p_suffix.clone())
                                    } else {
                                        address.clone()
                                    };
                                
                                //println!("YOUR NEIGHBORS \n {} \n", address_with_p2p);
                                //if let Err(e) =
                                let _ = self.swarm.dial(address_with_p2p.clone()); //{
                                //    error!("Dial failed: {}", e);
                                //}
                                self.swarm.behaviour_mut().kademlia.add_address(&peer, address_with_p2p);
                            }
                        }
                },
                    _ => {}
                }
                
            },


            other => {
                debug!("Unhandled FaceA swarm event: {other:?}");
            }
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => {
                let res = self.swarm.listen_on(addr)
                    .map(|_| ())
                    .map_err(|e| anyhow!("listen_on failed: {e}"));
                let _ = sender.send(res);
            }

            Command::Dial { peer_id, peer_addr, sender } => {
                use std::collections::hash_map::Entry;

                match self.pending_dial.entry(peer_id) {
                    Entry::Occupied(_) => {
                        let _ = sender.send(Err(anyhow!("Already dialing {peer_id}")));
                    }
                    Entry::Vacant(e) => {
                        // add base addr to kad
                        let mut kad_addr = peer_addr.clone();
                        if matches!(kad_addr.iter().last(), Some(libp2p::multiaddr::Protocol::P2p(_))) {
                            let _ = kad_addr.pop();
                        }
                        self.swarm.behaviour_mut().kademlia.add_address(&peer_id, kad_addr);

                        // dial with /p2p suffix
                        let dial_addr = {
                            let suffix = libp2p::multiaddr::Protocol::P2p(peer_id);
                            if peer_addr.iter().last() == Some(suffix.clone()) {
                                peer_addr
                            } else {
                                peer_addr.with(suffix)
                            }
                        };

                        if let Err(err) = self.swarm.dial(dial_addr) {
                            let _ = sender.send(Err(anyhow!("dial failed: {err}")));
                        } else {
                            e.insert(sender);
                        }
                    }
                }
            }

            Command::SetJobsHandler { handler } => {
                self.jobs_handler = handler;
            }

            Command::JobsRespond { channel, data } => {
                let _ = self.swarm.behaviour_mut().jobs_rr.send_response(channel, JobsResp(data));
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
        }
    }
}
