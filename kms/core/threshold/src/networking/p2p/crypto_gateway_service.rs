/*!
dead code
*/
use anyhow::Result;
use async_trait::async_trait;
// use futures::{prelude::*, StreamExt};
use libp2p::{
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    PeerId, StreamProtocol,
};
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

pub trait CryptoJobExecutor: Send + Sync + 'static {
    fn execute(
        &self,
        peer: PeerId,
        req: FaceBRequest,
    ) -> futures::future::BoxFuture<'static, Result<FaceBResponse>>;
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CryptoGatewayEvent")]
pub struct CryptoGatewayBehaviour {
    pub rr: request_response::Behaviour<CryptoCodec>,
}

#[derive(Debug)]
pub enum CryptoGatewayEvent {
    RR(request_response::Event<CryptoReq, CryptoResp>),
}
impl From<request_response::Event<CryptoReq, CryptoResp>> for CryptoGatewayEvent {
    fn from(e: request_response::Event<CryptoReq, CryptoResp>) -> Self {
        CryptoGatewayEvent::RR(e)
    }
}

/// Internal message from worker task back to swarm loop.
struct PendingResponse {
    channel: request_response::ResponseChannel<CryptoResp>,
    resp: FaceBResponse,
}

/// Service glue: handles request events, spawns executor work, and later sends responses.
///
/// Design constraint (correct for libp2p):
/// - `send_response` must happen on the task that owns the `Swarm` (i.e., in the swarm loop),
///   so workers only compute and send back `PendingResponse`s.
pub struct CryptoGatewayService {
    executor: std::sync::Arc<dyn CryptoJobExecutor>,
    resp_tx: tokio::sync::mpsc::Sender<PendingResponse>,
    resp_rx: tokio::sync::mpsc::Receiver<PendingResponse>,
}

impl CryptoGatewayService {
    pub fn new(executor: std::sync::Arc<dyn CryptoJobExecutor>) -> Self {
        let (resp_tx, resp_rx) = tokio::sync::mpsc::channel(64);
        Self {
            executor,
            resp_tx,
            resp_rx,
        }
    }

    pub fn behaviour() -> CryptoGatewayBehaviour {
        let protocol = StreamProtocol::new(FACE_B_CRYPTO_PROTOCOL);
        let cfg = request_response::Config::default();
        let rr = request_response::Behaviour::with_codec(
            CryptoCodec,
            std::iter::once((protocol, ProtocolSupport::Full)),
            cfg,
        );
        CryptoGatewayBehaviour { rr }
    }

    /// Drain completed worker responses without blocking.
    ///
    /// Must be called from the swarm loop task (or any place that has `&mut behaviour`).
    pub fn poll_completed(&mut self, behaviour: &mut CryptoGatewayBehaviour) {
        while let Ok(done) = self.resp_rx.try_recv() {
            let bytes = encode(&done.resp).unwrap_or_default();
            let _ = behaviour.rr.send_response(done.channel, CryptoResp(bytes));
        }
    }

    /// Handle a single request_response event.
    /// Call this from your swarm loop for `CryptoGatewayEvent::RR`.
    pub async fn on_event(
        &mut self,
        behaviour: &mut CryptoGatewayBehaviour,
        event: request_response::Event<CryptoReq, CryptoResp>,
    ) {
        match event {
            request_response::Event::Message { peer, message } => match message {
                request_response::Message::Request { request, channel, .. } => {
                    let req = match decode::<FaceBRequest>(&request.0) {
                        Ok(r) => r,
                        Err(e) => {
                            let resp = FaceBResponse::Error {
                                message: format!("decode FaceBRequest failed: {e}"),
                            };
                            let _ = behaviour.rr.send_response(
                                channel,
                                CryptoResp(encode(&resp).unwrap_or_default()),
                            );
                            return;
                        }
                    };

                    // Spawn worker task; send completion back to swarm loop via resp_tx.
                    let exec = self.executor.clone();
                    let tx = self.resp_tx.clone();
                    tokio::spawn(async move {
                        let resp = match exec.execute(peer, req).await {
                            Ok(r) => r,
                            Err(e) => FaceBResponse::Error {
                                message: format!("execute failed: {e}"),
                            },
                        };
                        // If the receiver is gone, ignore.
                        let _ = tx.send(PendingResponse { channel, resp }).await;
                    });
                }
                request_response::Message::Response { .. } => {}
            },

            request_response::Event::InboundFailure { peer, error, .. } => {
                tracing::warn!("crypto gateway inbound failure from {peer}: {error}");
            }
            request_response::Event::OutboundFailure { error, .. } => {
                tracing::warn!("crypto gateway outbound failure: {error}");
            }
            request_response::Event::ResponseSent { peer, .. } => {
                tracing::debug!("crypto gateway response sent to {peer}");
            }
        }

        // Opportunistic flush (non-blocking).
        self.poll_completed(behaviour);
    }
}

/* wiring example

let blobs_dir = std::path::PathBuf::from("/path/to/gateway/data/blobs"); // must match gateway data_dir/blobs
let resolver = std::sync::Arc::new(FsBlobResolver::new(blobs_dir));
let exec = std::sync::Arc::new(ThresholdKmsJobExecutor::new(kms_arc, resolver));
let mut svc = CryptoGatewayService::new(exec);
let mut beh = CryptoGatewayService::behaviour();

In swarm loop:
- on CryptoGatewayEvent::RR(e): svc.on_event(&mut beh, e).await;
- also periodically: svc.poll_completed(&mut beh);

*/
