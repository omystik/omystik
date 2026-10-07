use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
    StreamExt,
};
use libp2p::{
    identify,
    identity,
    ping,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use std::collections::HashMap;
use tokio::{task::JoinHandle, time::Duration};
use tracing::{debug, info};

use mesh_gateway_wire::{decode, encode, FaceBRequest, FaceBResponse};
use meshes_config::gateway_config::FACE_B_CRYPTO_PROTOCOL;

#[derive(Debug, Clone)]
pub struct CryptoReq(pub Vec<u8>);

#[derive(Debug, Clone)]
pub struct CryptoResp(pub Vec<u8>);

#[derive(Clone, Copy, Default)]
pub struct CryptoCodec;

#[async_trait]
impl request_response::Codec for CryptoCodec {
    type Protocol = StreamProtocol;
    type Request = CryptoReq;
    type Response = CryptoResp;

    async fn read_request<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(CryptoReq(buf))
    }

    async fn read_response<T: libp2p::futures::AsyncRead + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response> {
        let mut buf = Vec::new();
        libp2p::futures::AsyncReadExt::read_to_end(io, &mut buf).await?;
        Ok(CryptoResp(buf))
    }

    async fn write_request<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
        CryptoReq(data): CryptoReq,
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }

    async fn write_response<T: libp2p::futures::AsyncWrite + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
        CryptoResp(data): CryptoResp,
    ) -> std::io::Result<()> {
        libp2p::futures::AsyncWriteExt::write_all(io, &data).await?;
        libp2p::futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "FaceBEvent")]
struct FaceBBehaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    rr: request_response::Behaviour<CryptoCodec>,
}

#[derive(Debug)]
enum FaceBEvent {
    Ping(ping::Event),
    Identify(identify::Event),
    RR(request_response::Event<CryptoReq, CryptoResp>),
}

impl From<ping::Event> for FaceBEvent {
    fn from(e: ping::Event) -> Self {
        FaceBEvent::Ping(e)
    }
}

impl From<identify::Event> for FaceBEvent {
    fn from(e: identify::Event) -> Self {
        FaceBEvent::Identify(e)
    }
}

impl From<request_response::Event<CryptoReq, CryptoResp>> for FaceBEvent {
    fn from(e: request_response::Event<CryptoReq, CryptoResp>) -> Self {
        FaceBEvent::RR(e)
    }
}

#[derive(Debug)]
enum Command {
    Listen {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dial {
        peer: PeerId,
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dispatch {
        peer: PeerId,
        req: FaceBRequest,
        sender: oneshot::Sender<Result<FaceBResponse>>,
        _timeout: Duration, // reserved for later in-loop timeout wheel
    },
}

pub struct FaceBManager {
    pub client: FaceBClient,
    #[allow(dead_code)]
    loop_handle: JoinHandle<()>,
    bootstraps: Vec<Multiaddr>,
}

impl FaceBManager {
    pub async fn new(
        seed: Option<u8>,
        listen_addr: Option<Multiaddr>,
        bootstraps: Vec<Multiaddr>,
    ) -> Result<Self> {
        let (client, mut event_loop) = FaceBClient::build(seed).await?;

        let loop_handle = tokio::spawn(async move { event_loop.run().await });

        if let Some(a) = listen_addr {
            client.listen(a).await?;
        } else {
            client.listen("/ip4/0.0.0.0/udp/0/quic-v1".parse()?).await?;
        }

        for addr in &bootstraps {
            if let Some(peer) = peer_id_from_addr(&addr) {
                let _ = client.dial(peer, addr.clone()).await;
            }
        }

        Ok(Self { client, loop_handle, bootstraps })
    }

    // TO ENHANCE
    pub fn first_bootstrap_peer(&self) -> Option<PeerId> {
        self.bootstraps
            .first()
            .and_then(|a| peer_id_from_addr(a))
    }
    // - 6 March -
    pub fn bootstrap_peers(&self) -> Vec<PeerId> {
        self.bootstraps
            .iter()
            .filter_map(peer_id_from_addr)
            .collect()
    }
    // - -
}

#[derive(Clone)]
pub struct FaceBClient {
    tx: mpsc::Sender<Command>,
}

impl FaceBClient {
    async fn build(seed: Option<u8>) -> Result<(Self, FaceBEventLoop)> {
        let id_keys = match seed {
            Some(s) => {
                let mut bytes = [0u8; 32];
                bytes[0] = s;
                identity::Keypair::ed25519_from_bytes(bytes).unwrap()
            }
            None => identity::Keypair::generate_ed25519(),
        };

        let protocol = StreamProtocol::new(FACE_B_CRYPTO_PROTOCOL);

        let cfg = request_response::Config::default()
            .with_request_timeout(Duration::from_secs(600));

        let swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                let ping_behaviour =
                    ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));

                let identify_behaviour = identify::Behaviour::new(identify::Config::new(
                    "/pylon-face-b/0.1.0".to_string(),
                    key.public(),
                ));

                let rr = request_response::Behaviour::with_codec(
                    CryptoCodec,
                    std::iter::once((protocol.clone(), ProtocolSupport::Full)),
                    cfg.clone(),
                );

                Ok(FaceBBehaviour {
                    ping: ping_behaviour,
                    identify: identify_behaviour,
                    rr,
                })
    })?
    .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(360)))
    .build();

        let (tx, rx) = mpsc::channel(32);
        Ok((Self { tx }, FaceBEventLoop::new(swarm, rx)))
    }

    //// Clone the command sender locally so callers can use cloned FaceBClient
    /// handles concurrently without an external mutex.
    pub async fn listen(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        let mut tx = self.tx.clone();
        tx.send(Command::Listen { addr, sender }).await?;
        receiver.await?
    }

    pub async fn dial(&self, peer: PeerId, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        let mut tx = self.tx.clone();
        tx.send(Command::Dial { peer, addr, sender }).await?;
        receiver.await?
    }

pub async fn dispatch(
        &self,
        peer: PeerId,
        req: FaceBRequest,
        timeout: Duration,
    ) -> Result<FaceBResponse> {
        let (sender, receiver) = oneshot::channel();

        let mut tx = self.tx.clone();

        tx.send(Command::Dispatch {
            peer,
            req,
            sender,
            _timeout: timeout,
        })
        .await?;

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => Err(anyhow!("FaceB response channel closed: {e}")),
            Err(_) => Err(anyhow!("FaceB request timed out after {:?}", timeout)),
        }
    }
}
struct FaceBEventLoop {
    swarm: Swarm<FaceBBehaviour>,
    rx: mpsc::Receiver<Command>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pending_req: HashMap<request_response::OutboundRequestId, oneshot::Sender<Result<FaceBResponse>>>,
}

impl FaceBEventLoop {
    fn new(swarm: Swarm<FaceBBehaviour>, rx: mpsc::Receiver<Command>) -> Self {
        Self {
            swarm,
            rx,
            pending_dial: Default::default(),
            pending_req: Default::default(),
        }
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                ev = self.swarm.select_next_some() => self.on_swarm_event(ev).await,
                cmd = self.rx.next() => match cmd {
                    Some(c) => self.on_command(c).await,
                    None => return,
                }
            }
        }
    }

    async fn on_command(&mut self, cmd: Command) {
        match cmd {
            Command::Listen { addr, sender } => {
                let res = self
                    .swarm
                    .listen_on(addr)
                    .map(|_| ())
                    .map_err(|e| anyhow!("listen_on failed: {e}"));
                let _ = sender.send(res);
            }

            Command::Dial { peer, addr, sender } => {
                let dial_addr = if peer_id_from_addr(&addr).is_some() {
                    addr
                } else {
                    addr.with(libp2p::multiaddr::Protocol::P2p(peer))
                };
                match self.swarm.dial(dial_addr) {
                    Ok(()) => {
                        self.pending_dial.insert(peer, sender);
                    }
                    Err(e) => {
                        let _ = sender.send(Err(anyhow!("dial failed: {e}")));
                    }
                }
            }

            Command::Dispatch { peer, req, sender, _timeout: _ } => {
                let bytes = match encode(&req) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender.send(Err(anyhow!("encode FaceBRequest failed: {e}")));
                        return;
                    }
                };

                let id = self.swarm.behaviour_mut().rr.send_request(&peer, CryptoReq(bytes));
                info!(target:"faceb", peer=%peer, request_id=?id, "Face B (client) send_request");
                self.pending_req.insert(id, sender);

                // Proper timeout handling should be done inside the loop by tracking timestamps.
                // We intentionally omit it here to avoid non-compiling clones of oneshot senders.
            }
        }
    }

    async fn on_swarm_event(&mut self, ev: SwarmEvent<FaceBEvent>) {
        match ev {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Face B (client) listening on {address}");
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Face B (client) connected to {peer_id} via {endpoint:?}");
                if endpoint.is_dialer() {
                    if let Some(tx) = self.pending_dial.remove(&peer_id) {
                        let _ = tx.send(Ok(()));
                    }
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Face B (client) disconnected to {peer_id} cause {cause:?}");
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(tx) = self.pending_dial.remove(&peer_id) {
                        let _ = tx.send(Err(anyhow!("outgoing connection error: {error}")));
                    }
                }
            }

            SwarmEvent::Behaviour(FaceBEvent::RR(e)) => {
                
                info!(target:"faceb", "GW FaceB rr event: {:?}", e);
                match e {
                request_response::Event::Message { message, .. } => match message {
                    request_response::Message::Response { request_id, response } => {
                        if let Some(tx) = self.pending_req.remove(&request_id) {
                            let decoded = decode::<FaceBResponse>(&response.0)
                                .map_err(|e| anyhow!("decode FaceBResponse failed: {e}"));
                            let _ = tx.send(decoded);
                        }
                    }
                    request_response::Message::Request { .. } => {
                        // client-only
                    }
                },
                request_response::Event::OutboundFailure { request_id, error, .. } => {
                    if let Some(tx) = self.pending_req.remove(&request_id) {
                        let _ = tx.send(Err(anyhow!("outbound failure: {error}")));
                    }
                }
                request_response::Event::InboundFailure { peer, error, .. } => {
                    info!("FaceB (client) inbound failure from {peer}: {error}");
                }
                request_response::Event::ResponseSent { .. } => {}
                }
            },

            SwarmEvent::Behaviour(FaceBEvent::Ping(e)) => {
                debug!(target:"faceb", "ping event: {:?}", e);
            }
            SwarmEvent::Behaviour(FaceBEvent::Identify(e)) => {
                debug!(target:"faceb", "identify event: {:?}", e);
            }

            _ => {}
        }
    }
}

fn peer_id_from_addr(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| match p {
        libp2p::multiaddr::Protocol::P2p(peer) => Some(peer),
        _ => None,
    })
}

pub async fn faceb_smoke(
    client: &crate::face_b::FaceBClient,
    peer: PeerId,
) -> anyhow::Result<()> {
    let job_id = [0u8; 32]; // dummy
    let resp = tokio::time::timeout(
        Duration::from_secs(5),
        client.dispatch(peer, FaceBRequest::GetCryptoJobStatus { job_id }, Duration::from_secs(5)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Face B (client) moke timeout"))??;

    // Accept Error("unknown job_id") as success: it proves roundtrip.
    tracing::info!(target:"faceb", ?resp, "Face B (client) smoke response");
    Ok(())
}
