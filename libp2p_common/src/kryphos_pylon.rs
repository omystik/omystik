// Thin specialization of kryphos_dual for Face B crypto gateway transport (typed FaceBRequest/FaceBResponse).

use anyhow::{anyhow, Error as AnyhowError, Result};
use futures::{future::BoxFuture, Stream as FuturesStream};
use libp2p::{PeerId, StreamProtocol};
use mesh_gateway_wire::{decode, encode, FaceBRequest, FaceBResponse};
use std::sync::Arc;
use tokio::time::Duration;

use crate::kryphos_dual;

pub type Event = kryphos_dual::Event;
pub type EventLoop = kryphos_dual::EventLoop;

#[derive(Clone)]
pub struct Client {
    inner: kryphos_dual::Client,
}

pub async fn new(
    secret_key_seed: Option<u8>,
    threshold_proto: StreamProtocol,
    gateway_proto: StreamProtocol,
) -> Result<(Client, impl FuturesStream<Item = Event>, EventLoop), AnyhowError> {
    let (inner, events, loop_) = kryphos_dual::new(secret_key_seed, threshold_proto, gateway_proto).await?;
    Ok((Client { inner }, events, loop_))
}

impl Client {
    pub async fn start_listening(&mut self, addr: libp2p::Multiaddr) -> Result<(), AnyhowError> {
        self.inner.start_listening(addr).await
    }

    pub async fn advertise(&mut self, peer_id: PeerId, address: libp2p::Multiaddr) -> Result<(), AnyhowError> {
        self.inner.advertise(peer_id, address).await
    }

    pub async fn dial(&mut self, peer_id: PeerId, peer_addr: libp2p::Multiaddr) -> Result<(), AnyhowError> {
        self.inner.dial(peer_id, peer_addr).await
    }

    pub async fn compute_enc_request(
        &mut self,
        peer_id: PeerId,
        req: FaceBRequest,
        timeout: Duration,
    ) -> Result<FaceBResponse, AnyhowError> {
        let bytes = encode(&req).map_err(|e| anyhow!("encode FaceBRequest failed: {e}"))?;

        let resp_bytes = tokio::time::timeout(
            timeout,
            self.inner.compute_enc_request(peer_id, bytes, timeout)
        )
            .await
            .map_err(|_| anyhow!("compute_enc_request timed out"))??;

        decode::<FaceBResponse>(&resp_bytes.as_ref()).map_err(|e| anyhow!("decode FaceBResponse failed: {e}"))
    }

    /// Install server-side handler for inbound FaceBRequest.
    ///
    /// The handler is **required** by node_kryphos_gateway; if you want “no handler installed”
    /// semantics, pass a handler that returns FaceBResponse::Error.
    pub async fn set_compute_enc_handler(
        &mut self,
        handler: Arc<
            dyn Fn(PeerId, FaceBRequest) -> BoxFuture<'static, Result<FaceBResponse, AnyhowError>>
                + Send
                + Sync,
        >,
    ) -> Result<(), AnyhowError> {
            self.inner.set_compute_enc_handler(Some(handler)).await
    }

    pub async fn parties_mesh_ready(&mut self, required: Vec<PeerId>) -> Result<(), AnyhowError> {
        self.inner.parties_mesh_ready(required).await
    }
}
