// Thin specialization of kryphos_dual for "kryphos" (threshold core-to-core bytes transport).

use anyhow::Error as AnyhowError;
use futures::{future::BoxFuture, Stream as FuturesStream};
use libp2p::{PeerId, StreamProtocol};
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

    pub async fn threshold_request(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, AnyhowError> {
        // kryphos_dual doesn’t enforce timeout at the swarm loop level; apply here.
        tokio::time::timeout(Duration::from_secs(120), self.inner.threshold_request(peer_id, data))
            .await
            .map_err(|_| anyhow::anyhow!("threshold_request timed out (peer:{peer_id})"))?
    }

    pub async fn set_threshold_handler(
        &mut self,
        handler: Option<Arc<dyn Fn(PeerId, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync>>,
    ) -> Result<(), AnyhowError> {
        self.inner.set_threshold_handler(handler).await
    }

    pub async fn parties_mesh_ready(&mut self, required: Vec<PeerId>) -> Result<(), AnyhowError> {
        self.inner.parties_mesh_ready(required).await
    }
}
