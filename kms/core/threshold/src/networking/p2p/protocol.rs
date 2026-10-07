use super::router::{Ack, InboundRouter};
use crate::execution::runtime::party::MpcIdentity;
use async_trait::async_trait;
use libp2p::{
    request_response::{
        Behaviour as RequestResponse, Codec as RRCodec, Config as RRConfig, Event as RREvent,
        Message as RRMessage, ProtocolSupport,
    },
    swarm::NetworkBehaviour,
    PeerId, StreamProtocol,
};
use std::{collections::HashMap, io, sync::Arc};

/// Messages exchanged over the libp2p request/response protocol.
///
/// Required:
/// - SendValue { tag, value }
///
/// Optional:
/// - HealthCheck { tag }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WireMsg {
    SendValue { tag: Vec<u8>, value: Vec<u8> },
    HealthCheck { tag: Vec<u8> },
}

#[derive(Clone, Default)]
pub struct ThresholdCodec;

#[async_trait]
impl RRCodec for ThresholdCodec {
    type Protocol = StreamProtocol;
    type Request = WireMsg;
    type Response = Ack;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let bytes = read_len_delimited(io, 16 * 1024 * 1024).await?;
        bc2wrap::deserialize_safe::<WireMsg>(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let bytes = read_len_delimited(io, 2 * 1024 * 1024).await?;
        bc2wrap::deserialize_safe::<Ack>(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let bytes = bc2wrap::serialize(&req)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        write_len_delimited(io, &bytes).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let bytes = bc2wrap::serialize(&resp)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        write_len_delimited(io, &bytes).await
    }
}

async fn read_len_delimited<T: futures::AsyncRead + Unpin + Send>(
    reader: &mut T,
    max: usize,
) -> io::Result<Vec<u8>> {
    use futures::AsyncReadExt;

    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_len_delimited<T: futures::AsyncWrite + Unpin + Send>(
    writer: &mut T,
    bytes: &[u8],
) -> io::Result<()> {
    use futures::AsyncWriteExt;

    let len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame too large"))?;

    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

#[derive(Debug)]
pub enum ThresholdEvent {
    ReqRes(RREvent<WireMsg, Ack>),
}

impl From<RREvent<WireMsg, Ack>> for ThresholdEvent {
    fn from(e: RREvent<WireMsg, Ack>) -> Self {
        ThresholdEvent::ReqRes(e)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ThresholdEvent")]
pub struct ThresholdBehaviour {
    pub rr: RequestResponse<ThresholdCodec>,
}

impl ThresholdBehaviour {
    pub fn new() -> Self {
        let proto = StreamProtocol::new("/threshold-fhe/1.0.0");
        let cfg = RRConfig::default().with_request_timeout(std::time::Duration::from_secs(30));

        // IMPORTANT: use the 3-arg ctor that exists in your version.
        // This avoids "expected 2 args, found 3" and the IntoIterator error.
        let rr = RequestResponse::with_codec(
            ThresholdCodec::default(),
            [(proto, ProtocolSupport::Full)],
            cfg,
        );

        Self { rr }
    }

    /// Call this from your Swarm event loop; router/peer_map live outside the behaviour.
    pub async fn on_rr_event(
        &mut self,
        router: &InboundRouter,
        peer_map: &Arc<HashMap<PeerId, MpcIdentity>>,
        event: RREvent<WireMsg, Ack>,
    ) {
        if let RREvent::Message { peer, message } = event {
            if let RRMessage::Request { request, channel, .. } = message {
                let expected_sender = peer_map.get(&peer).cloned();

                let ack = match (request, expected_sender) {
                    (WireMsg::HealthCheck { tag }, Some(_sender)) => {
                        // If this is sync in your codebase, drop `.await`.
                        router.handle_health_check(&tag)
                    }
                    (WireMsg::SendValue { tag, value }, Some(sender)) => {
                        let ok = match bc2wrap::deserialize_safe::<crate::networking::session_runtime::Tag>(&tag) {
                            Ok(t) => t.sender == sender,
                            Err(_) => false,
                        };

                        if !ok {
                            Ack::Rejected { code: 401, message: "sender mismatch".to_string() }
                        } else {
                            router.handle_send_value(&tag, value).await
                        }
                    }
                    (_, None) => Ack::Rejected { code: 401, message: "unknown peer".to_string() },
                };

                let _ = self.rr.send_response(channel, ack);
            }
        }
    }
}
