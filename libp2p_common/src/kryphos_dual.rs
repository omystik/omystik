use anyhow::{anyhow, Error as AnyhowError, Result};
use async_trait::async_trait;
use dashmap::DashMap;

use futures::{
    channel::{mpsc, oneshot},
    future::BoxFuture,
    prelude::*,
    Stream as FuturesStream,
    StreamExt,
};
use libp2p::{
    identify,
    identity,
    kad,
    multiaddr::Protocol,
    ping,
    relay,
    request_response::{self, OutboundRequestId, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use mesh_gateway_wire::{decode, encode, FaceBRequest, FaceBResponse};

use std::{collections::HashMap, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle, time::Duration};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct ThresholdReq(pub Vec<u8>);
#[derive(Debug, Clone)]
pub struct ThresholdResp(pub Vec<u8>);

#[derive(Clone, Copy, Default)]
pub struct ThresholdCodec;

#[async_trait]
impl request_response::Codec for ThresholdCodec {
    type Protocol = StreamProtocol;
    type Request = ThresholdReq;
    type Response = ThresholdResp;

    async fn read_request<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Request> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(ThresholdReq(buf))
    }

    async fn read_response<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Response> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(ThresholdResp(buf))
    }

    async fn write_request<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, ThresholdReq(data): ThresholdReq
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }

    async fn write_response<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, ThresholdResp(data): ThresholdResp
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ComputeEncReq(pub Vec<u8>);
#[derive(Debug, Clone)]
pub struct ComputeEncResp(pub Vec<u8>);

#[derive(Clone, Copy, Default)]
pub struct ComputeEncCodec;

#[async_trait::async_trait]
impl request_response::Codec for ComputeEncCodec {
    type Protocol = StreamProtocol;
    type Request = ComputeEncReq;
    type Response = ComputeEncResp;

    async fn read_request<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Request> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(ComputeEncReq(buf))
    }

    async fn read_response<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T
    ) -> std::io::Result<Self::Response> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(ComputeEncResp(buf))
    }

    async fn write_request<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, ComputeEncReq(data): ComputeEncReq
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }

    async fn write_response<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self, _: &StreamProtocol, io: &mut T, ComputeEncResp(data): ComputeEncResp
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "DualEvent")]
pub struct DualBehaviour {
    pub relay: relay::Behaviour,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,

    // Side 1
    pub threshold_rr: request_response::Behaviour<ThresholdCodec>,

    // Side 2
    pub compute_enc_rr: request_response::Behaviour<ComputeEncCodec>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DualEvent {
    Relay(relay::Event),
    Ping(ping::Event),
    Identify(identify::Event),
    Kademlia(kad::Event),

    Threshold(request_response::Event<ThresholdReq, ThresholdResp>),
    ComputeEnc(request_response::Event<ComputeEncReq, ComputeEncResp>),
}

impl From<relay::Event> for DualEvent {
    fn from(e: relay::Event) -> Self {
        Self::Relay(e)
    }
}
impl From<ping::Event> for DualEvent {
    fn from(e: ping::Event) -> Self {
        Self::Ping(e)
    }
}
impl From<identify::Event> for DualEvent {
    fn from(e: identify::Event) -> Self {
        Self::Identify(e)
    }
}
impl From<kad::Event> for DualEvent {
    fn from(e: kad::Event) -> Self {
        Self::Kademlia(e)
    }
}

impl From<request_response::Event<ThresholdReq, ThresholdResp>> for DualEvent {
    fn from(e: request_response::Event<ThresholdReq, ThresholdResp>) -> Self {
        Self::Threshold(e)
    }
}
impl From<request_response::Event<ComputeEncReq, ComputeEncResp>> for DualEvent {
    fn from(e: request_response::Event<ComputeEncReq, ComputeEncResp>) -> Self {
        Self::ComputeEnc(e)
    }
}

#[derive(Debug)]
pub enum Event {
    // keep empty for now
}

enum Command {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Advertise {
        peer_id: PeerId,
        address: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },

    // Side 1: threshold
    ThresholdRequest {
        peer_id: PeerId,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<Vec<u8>, AnyhowError>>,
    },
    ThresholdRespond {
        channel: request_response::ResponseChannel<ThresholdResp>,
        data: Vec<u8>,
    },
    SetThresholdHandler {
        handler: Option<Arc<dyn Fn(PeerId, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync>>,
    },

    // Side 2: compute enc.
    ComputeEncRequest {
        peer_id: PeerId,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<Vec<u8>, AnyhowError>>,
        timeout: Duration,
    },
    ComputeEncRespond {
        channel: request_response::ResponseChannel<ComputeEncResp>,
        resp: FaceBResponse,
    },
    SetComputeEncHandler {
        handler: Option<
            Arc<
                dyn Fn(PeerId, FaceBRequest) -> BoxFuture<'static, Result<FaceBResponse, AnyhowError>>
                    + Send
                    + Sync,
            >,
        >,
    },

    PartiesMeshReady {
        required: Vec<PeerId>,
        sender: oneshot::Sender<Result<(), AnyhowError>>,
    },
}

pub async fn new(
    secret_key_seed: Option<u8>,
    threshold_proto: StreamProtocol,
    gateway_proto: StreamProtocol,
) -> Result<(Client, impl FuturesStream<Item = Event>, EventLoop), AnyhowError> {
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
        .with_behaviour(move |key| {
            let relay_behaviour =
                relay::Behaviour::new(key.public().to_peer_id(), Default::default());
            let ping_behaviour =
                ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(60)));
            let identify_behaviour = identify::Behaviour::new(identify::Config::new("/kryphos-node/0.1.0".to_string(),
                key.public(),
            ));

            let store = kad::store::MemoryStore::new(peer_id);
            let mut kademlia_behaviour = kad::Behaviour::new(peer_id, store);
            kademlia_behaviour.set_mode(Some(kad::Mode::Server));

            let threshold_rr = request_response::Behaviour::with_codec(
                ThresholdCodec,
                std::iter::once((threshold_proto, ProtocolSupport::Full)),
                request_response::Config::default(),
            );

            let compute_enc_rr = request_response::Behaviour::with_codec(
                ComputeEncCodec,
                std::iter::once((gateway_proto, ProtocolSupport::Full)),
                request_response::Config::default(),
            );

            Ok(DualBehaviour {
                relay: relay_behaviour,
                ping: ping_behaviour,
                identify: identify_behaviour,
                kademlia: kademlia_behaviour,
                threshold_rr,
                compute_enc_rr,
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(360)))
        .build();

    let (command_sender, command_receiver) = mpsc::channel(64);
    let (event_sender, event_receiver) = mpsc::channel(32);

    let actual_listen_addr: Arc<Mutex<Option<Multiaddr>>> = Arc::new(Mutex::new(None));

    Ok((
        Client {
            sender: command_sender.clone(),
        },
        event_receiver,
        EventLoop::new(
            swarm,
            command_sender,
            command_receiver,
            event_sender,
            actual_listen_addr,
        ),
    ))
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Command>,
}

impl Client {
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { addr, sender })
            .await
            .map_err(|e| anyhow!("StartListening send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("StartListening oneshot failed: {e}"))?
    }

    pub async fn advertise(&mut self, peer_id: PeerId, address: Multiaddr) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Advertise {
                peer_id,
                address,
                sender,
            })
            .await
            .map_err(|e| anyhow!("Advertise send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("Advertise oneshot failed: {e}"))?
    }

    pub async fn dial(&mut self, peer_id: PeerId, peer_addr: Multiaddr) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .map_err(|e| anyhow!("Dial send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("Dial oneshot failed: {e}"))?
    }

    // ---------------- Side 1 (threshold) ----------------

    pub async fn threshold_request(&mut self, peer_id: PeerId, data: Vec<u8>) -> Result<Vec<u8>, AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::ThresholdRequest { peer_id, data, sender })
            .await
            .map_err(|e| anyhow!("ThresholdRequest send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("ThresholdRequest oneshot failed: {e}"))?
    }

    pub async fn set_threshold_handler(
        &mut self,
        handler: Option<Arc<dyn Fn(PeerId, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync>>,
    ) -> Result<(), AnyhowError> {
        self.sender
            .send(Command::SetThresholdHandler { handler })
            .await
            .map_err(|e| anyhow!("SetThresholdHandler send failed: {e}"))?;
        Ok(())
    }

    // ---------------- Side 2 (compute enc.) ----------------

    pub async fn compute_enc_request(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::ComputeEncRequest {
                peer_id,
                data,
                sender,
                timeout,
            })
             .await
            .map_err(|e| anyhow::anyhow!("send ComputeEncRequest failed: {e}"))?;
        receiver
            .await
            .map_err(|e| anyhow::anyhow!("ComputeEncRequest oneshot failed: {e}"))?
    }

    pub async fn set_compute_enc_handler(
        &mut self,
        handler: Option<
            Arc<
                dyn Fn(PeerId, FaceBRequest) -> BoxFuture<'static, Result<FaceBResponse, AnyhowError>>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Result<(), AnyhowError> {
        self.sender
            .send(Command::SetComputeEncHandler { handler })
            .await
            .map_err(|e| anyhow!("SetComputeEncHandler send failed: {e}"))?;
        Ok(())
    }

    // ---------------- shared ----------------

    pub async fn parties_mesh_ready(&mut self, required: Vec<PeerId>) -> Result<(), AnyhowError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::PartiesMeshReady { required, sender })
            .await
            .map_err(|e| anyhow!("PartiesMeshReady send failed: {e}"))?;
        receiver.await.map_err(|e| anyhow!("PartiesMeshReady oneshot failed: {e}"))?
    }
}

pub struct EventLoop {
    swarm: Swarm<DualBehaviour>,
    command_sender: mpsc::Sender<Command>,
    command_receiver: mpsc::Receiver<Command>,
    _event_sender: mpsc::Sender<Event>,

    pub actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,

    pending_dial: HashMap<PeerId, Vec<oneshot::Sender<Result<(), AnyhowError>>>>,

    // Side 1 pending map
    pending_threshold: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<u8>, AnyhowError>>>,

    // Side 2 pending map
    pending_compute_enc: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<u8>, AnyhowError>>>,

    threshold_handler: Option<Arc<dyn Fn(PeerId, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync>>,
    compute_enc_handler: Option<
        Arc<
            dyn Fn(PeerId, FaceBRequest) -> BoxFuture<'static, Result<FaceBResponse, AnyhowError>>
                + Send
                + Sync,
        >,
    >,

    connected: Arc<DashMap<PeerId, u32>>,
    mesh_ready_parties: Vec<(Vec<PeerId>, oneshot::Sender<Result<(), AnyhowError>>)>,
}

impl EventLoop {
    fn new(
        swarm: Swarm<DualBehaviour>,
        command_sender: mpsc::Sender<Command>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        actual_listen_addr: Arc<Mutex<Option<Multiaddr>>>,
    ) -> Self {
        Self {
            swarm,
            command_sender,
            command_receiver,
            _event_sender: event_sender,
            actual_listen_addr,
            pending_dial: Default::default(),
            pending_threshold: Default::default(),
            pending_compute_enc: Default::default(),
            threshold_handler: None,
            compute_enc_handler: None,
            connected: Arc::new(DashMap::new()),
            mesh_ready_parties: Default::default(),
        }
    }

    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn connected_handle(&self) -> Arc<DashMap<PeerId, u32>> {
        Arc::clone(&self.connected)
    }

    fn is_mesh_ready(&self, required: &[PeerId]) -> bool {
        required
            .iter()
            .all(|p| self.connected.get(p).map(|v| *v > 0).unwrap_or(false))
    }

    fn wake_mesh_ready_parties(&mut self) {
        let pending = std::mem::take(&mut self.mesh_ready_parties);
        for (required, sender) in pending {
            if self.is_mesh_ready(&required) {
                let _ = sender.send(Ok(()));
            } else {
                self.mesh_ready_parties.push((required, sender));
            }
        }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                ev = self.swarm.select_next_some() => self.handle_swarm_event(ev).await,
                cmd = self.command_receiver.next() => match cmd {
                    Some(c) => self.handle_command(c).await,
                    None => return,
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<DualEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let full_addr = address.with(Protocol::P2p(*self.swarm.local_peer_id()));
                *self.actual_listen_addr.lock().await = Some(full_addr.clone());
                info!("kryphos_dual listening on {full_addr:?}");
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => {
                self.connected.insert(peer_id, num_established.get());
                info!("MPC CONNECTED to {peer_id} via {endpoint:?} (num={})", num_established);

                if let Some(waiters) = self.pending_dial.remove(&peer_id) {
                    for sender in waiters {
                        let _ = sender.send(Ok(()));
                    }
                }
                self.wake_mesh_ready_parties();
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                if num_established == 0 {
                    self.connected.remove(&peer_id);
                } else {
                    self.connected.insert(peer_id, num_established);
                }
                info!("MPC DISCONNECTED from {peer_id} cause={cause:?} (num={})", num_established);
                self.wake_mesh_ready_parties();
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(waiters) = self.pending_dial.remove(&peer_id) {
                        for sender in waiters {
                            let _ = sender.send(Err(anyhow!("Outgoing connection error to {peer_id}: {error}")));
                        }
                    }
                }
            }

            // -------- Side 1 events --------
            SwarmEvent::Behaviour(DualEvent::Threshold(ev)) => match ev {
                request_response::Event::Message { peer, message } => match message {
                    request_response::Message::Request { request, channel, .. } => {
                        // Respond with handler-produced bytes (non-blocking for swarm loop).
                        // The handler is expected to return immediately with transport-level Ack bytes;
                        // any heavy MPC work should be spawned inside the handler itself.
                        if let Some(handler) = self.threshold_handler.clone() {
                            let req_bytes = request.0;
                            let mut tx = self.command_sender.clone();
                            tokio::spawn(async move {
                                let resp_bytes = (handler)(peer, req_bytes).await;
                                let _ = tx
                                    .send(Command::ThresholdRespond {
                                        channel,
                                        data: resp_bytes,
                                    })
                                    .await;
                            });
                        } else {
                            // Fallback: keep previous behavior if no handler installed.
                            let _ = self
                                .swarm
                                .behaviour_mut()
                                .threshold_rr
                                .send_response(channel, ThresholdResp(Vec::new()));
                        }
                    }

                    request_response::Message::Response { request_id, response } => {
                        if let Some(sender) = self.pending_threshold.remove(&request_id) {
                            let _ = sender.send(Ok(response.0));
                        }
                    }
                },

                request_response::Event::OutboundFailure { request_id, error, .. } => {
                    if let Some(sender) = self.pending_threshold.remove(&request_id) {
                        let _ = sender.send(Err(anyhow!("Threshold outbound failure: {error}")));
                    }
                }

                request_response::Event::InboundFailure { peer, error, .. } => {
                    debug!("Threshold inbound failure from {peer}: {error}");
                }

                request_response::Event::ResponseSent { peer, .. } => {
                    debug!("Threshold response sent to {peer}");
                }
            },

            // -------- Side 2 events --------
            SwarmEvent::Behaviour(DualEvent::ComputeEnc(ev)) => match ev {
                request_response::Event::Message { peer, message } => match message {
                    request_response::Message::Request { request, channel, .. } => {
                        // decode FaceBRequest
                        let req = match decode::<FaceBRequest>(&request.0) {
                            Ok(r) => r,
                            Err(e) => {
                                let resp = FaceBResponse::Error {
                                    message: format!("decode FaceBRequest failed: {e}"),
                                };

                                self.send_faceb_response(channel, resp);
                                return;
                            }
                        };
                                
                        info!(target:"faceb", peer=%peer, "Face B server got request: {:?}", req);

                        if let Some(handler) = self.compute_enc_handler.clone() {
                            let mut tx = self.command_sender.clone();
                            tokio::spawn(async move {
                                let resp = match (handler)(peer, req).await {
                                    Ok(r) => r,
                                    Err(e) => FaceBResponse::Error { message: format!("handler failed: {e}") },
                                };

                                info!(target:"faceb", peer=%peer, "FaceB server sending response: {:?}", resp);

                                let _ = tx.send(Command::ComputeEncRespond { channel, resp }).await;
                            });
                        } else {
                            
                            let resp = FaceBResponse::Error { message: "no compute encrypted handler installed".into() };

                            info!(target:"faceb", peer=%peer, "Face B server sending response: {:?}", resp);
                            
                            self.send_faceb_response(channel, resp);
                        }
                    }
                        
                    request_response::Message::Response { request_id, response } => {
                        if let Some(sender) = self.pending_compute_enc.remove(&request_id) {                              
                            let _ = sender.send(Ok(response.0));
                        }
                    }
                },

                request_response::Event::OutboundFailure { request_id, error, .. } => {
                    if let Some(sender) = self.pending_compute_enc.remove(&request_id) {
                        let _ = sender.send(Err(anyhow!("ComputeEnc outbound failure: {error}")));
                    }
                }
                request_response::Event::InboundFailure { peer, error, .. } => {
                    debug!("ComputeEnc inbound failure from {peer}: {error}");
                }
                request_response::Event::ResponseSent { peer, .. } => {
                    debug!("ComputeEnc response sent to {peer}");
                }
            },

            // ignore other behaviour events for now
            SwarmEvent::Behaviour(DualEvent::Kademlia(_)) => {}
            SwarmEvent::Behaviour(DualEvent::Relay(_)) => {}
            SwarmEvent::Behaviour(DualEvent::Ping(_)) => {}
            SwarmEvent::Behaviour(DualEvent::Identify(_)) => {}
            other => debug!("Unhandled swarm event: {other:?}"),
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => {
                let res = self.swarm.listen_on(addr).map(|_| ()).map_err(|e| anyhow!("listen_on failed: {e}"));
                let _ = sender.send(res);
            }

            Command::Advertise { peer_id, address, sender } => {
                self.swarm.add_external_address(address.clone());
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, address);
                let _ = sender.send(Ok(()));
            }

            Command::Dial { peer_id, peer_addr, sender } => {
                use std::collections::hash_map::Entry;

                if self.connected.get(&peer_id).map(|v| *v > 0).unwrap_or(false) {
                    let _ = sender.send(Ok(()));
                    return;
                }

                match self.pending_dial.entry(peer_id) {
                    Entry::Occupied(mut e) => e.get_mut().push(sender),
                    Entry::Vacant(e) => {
                        e.insert(vec![sender]);

                        // add base addr to kad
                        let mut kad_addr = peer_addr.clone();
                        if matches!(kad_addr.iter().last(), Some(Protocol::P2p(_))) {
                            let _ = kad_addr.pop();
                        }
                        self.swarm.behaviour_mut().kademlia.add_address(&peer_id, kad_addr);

                        // dial with /p2p suffix
                        let dial_addr = {
                            let suffix = Protocol::P2p(peer_id);
                            if peer_addr.iter().last() == Some(suffix.clone()) {
                                peer_addr
                            } else {
                                peer_addr.with(suffix)
                            }
                        };

                        if let Err(err) = self.swarm.dial(dial_addr) {
                            if let Some(waiters) = self.pending_dial.remove(&peer_id) {
                                for s in waiters {
                                    let _ = s.send(Err(anyhow!("dial failed: {err}")));
                                }
                            }
                        }
                    }
                }
            }

            // Side 1
            Command::ThresholdRequest { peer_id, data, sender } => {
                let req_id = self.swarm.behaviour_mut().threshold_rr.send_request(&peer_id, ThresholdReq(data));
                self.pending_threshold.insert(req_id, sender);
            }
            Command::ThresholdRespond { channel, data } => {
                let _ = self.swarm.behaviour_mut().threshold_rr.send_response(channel, ThresholdResp(data));
            }
            Command::SetThresholdHandler { handler } => self.threshold_handler = handler,

            // Side 2
            Command::ComputeEncRequest { peer_id, data, sender, timeout: _timeout } => {
                // apply timeout at client layer by spawning a timer task
                let req_id = self.swarm.behaviour_mut().compute_enc_rr.send_request(&peer_id, ComputeEncReq(data));
                info!(target:"faceb", peer=%peer_id, request_id=?req_id, "Face B (server) ComputeEnc send_request");
                self.pending_compute_enc.insert(req_id, sender);

                // best-effort timeout: if still pending later, fail it
                // let pending_compute_enc = Arc::new(Mutex::new(()));
                // let mut tx = self.command_sender.clone();
                // let req_id_copy = req_id;
                // tokio::spawn(async move {
                //     tokio::time::sleep(timeout).await;
                //     // no direct access to map here; we handle timeout at Client by using tokio::time::timeout
                //     // so this is intentionally a no-op.
                //     let _ = tx.send(Command::SetComputeEncHandler { handler: None }).await;
                //     let _ = req_id_copy; // silence unused warnings if you remove the no-op later
                //     let _ = pending_compute_enc.lock().await;
                // });
            }
            Command::ComputeEncRespond { channel, resp } => {
                 self.send_faceb_response(channel, resp);
            }
            Command::SetComputeEncHandler { handler } => self.compute_enc_handler = handler,

            Command::PartiesMeshReady { required, sender } => {
                if self.is_mesh_ready(&required) {
                    let _ = sender.send(Ok(()));
                } else {
                    self.mesh_ready_parties.push((required, sender));
                }
            }
        }
    }


    fn send_faceb_response(
        &mut self,
        channel: request_response::ResponseChannel<ComputeEncResp>,
        resp: FaceBResponse,
    ) {
        match encode(&resp) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    tracing::error!(
                        target: "faceb",
                        "BUG: encoded FaceBResponse is empty; resp={:?}",
                        resp
                    );
                }

                let _ = self
                    .swarm
                    .behaviour_mut()
                    .compute_enc_rr
                    .send_response(channel, ComputeEncResp(bytes));
            }

            Err(e) => {
                tracing::error!(
                    target: "faceb",
                    "encode FaceBResponse failed before send_response: {e}; original resp={:?}",
                    resp
                );

                let fallback = FaceBResponse::Error {
                    message: format!("encode FaceBResponse failed before send_response: {e}"),
                };

                match encode(&fallback) {
                    Ok(bytes) => {
                        let _ = self
                            .swarm
                            .behaviour_mut()
                            .compute_enc_rr
                            .send_response(channel, ComputeEncResp(bytes));
                    }
                    Err(e2) => {
                        tracing::error!(
                            target: "faceb",
                            "fatal: could not encode fallback FaceBResponse::Error: {e2}"
                        );
                    }
                }
            }
        }
    }
}
