use super::protocol::WireMsg;
use super::router::Ack;
use crate::execution::runtime::party::{Identity, Role, RoleAssignment};
use crate::networking::grpc::OptionConfigWrapper;
use crate::networking::sending_service::{ArcSendValueRequest, SendingService};
use async_trait::async_trait;
use libp2p::PeerId;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::{sleep, timeout, Duration};

/// Outbound sender for libp2p. It mirrors GrpcSendingService by returning per-peer local channels.
///
/// IMPORTANT:
/// - Each sender task serializes `WireMsg` into bytes and sends via
///   `mesh_client.threshold_request(peer_id, bytes)`
/// - `threshold_request` returns raw response bytes, which must decode into `Ack`.
/// - Retries use exponential backoff with jitter (bounded attempts).
#[derive(Clone)]
pub struct Libp2pSendingService {
    pub config: OptionConfigWrapper,
    /// Map Identity -> PeerId used by libp2p
    pub peer_by_identity: Arc<HashMap<Identity, PeerId>>,
    /// Mesh client used to perform threshold requests over libp2p.
    pub mesh_client: Arc<dyn ThresholdMeshClient>,
}

/// Minimal trait to decouple this sender from your concrete libp2p client.
///
/// Must match kryphos.rs:
/// `async fn threshold_request(&mut self, peer: PeerId, data: Vec<u8>) -> Result<Vec<u8>, AnyhowError>`
#[async_trait]
pub trait ThresholdMeshClient: Send + Sync {
    async fn threshold_request(&self, peer: PeerId, bytes: Vec<u8>) -> anyhow::Result<Vec<u8>>;
}

fn is_transient_ack(ack: &Ack) -> bool {
    match ack {
        Ack::Ok => false,
        Ack::DroppedCompletedOrClosed => false,
        Ack::Rejected { code, .. } => *code == 429 || *code >= 500,
    }
}

fn ack_code_and_msg(ack: &Ack) -> (u16, String) {
    match ack {
        Ack::Ok => (200, "ok".to_string()),
        Ack::DroppedCompletedOrClosed => (204, "dropped (completed/closed)".to_string()),
        Ack::Rejected { code, message } => (*code, message.clone()),
    }
}

fn backoff_delay(attempt: usize, base: Duration, max: Duration) -> Duration {
    // exponential: base * 2^(attempt-1), capped at max
    let pow: u32 = attempt.saturating_sub(1).min(31) as u32;
    let factor: u32 = 1u32.checked_shl(pow).unwrap_or(u32::MAX);
    let mut d = base.saturating_mul(factor);
    if d > max {
        d = max;
    }

    // jitter: 0..=d/4 (no extra deps)
    let jitter_max = d / 4;
    if jitter_max.is_zero() {
        return d;
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_nanos(0))
        .subsec_nanos() as u64;

    let jm = jitter_max.as_nanos().min(u64::MAX as u128) as u64;
    let jitter = Duration::from_nanos(nanos % (jm.saturating_add(1)));
    d + jitter
}

fn encode_wire_msg(msg: &WireMsg) -> anyhow::Result<Vec<u8>> {
    Ok(bc2wrap::serialize(msg)?)
}

fn decode_ack(resp_bytes: &[u8]) -> Ack {
    match bc2wrap::deserialize_safe::<Ack>(resp_bytes) {
        Ok(a) => a,
        Err(e) => Ack::Rejected {
            code: 500,
            message: format!("bad ack decode: {e}"),
        },
    }
}

#[async_trait]
impl SendingService for Libp2pSendingService {
    fn new(
        _tls_certs: Option<tokio_rustls::rustls::client::ClientConfig>,
        _conf: OptionConfigWrapper,
        _peer_tcp_proxy: bool,
    ) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!(
            "Libp2pSendingService requires constructor with peer map + mesh_client; use Libp2pSendingService::new_with()"
        ))
    }

    async fn add_connection(
        &self,
        other: Identity,
    ) -> anyhow::Result<UnboundedSender<ArcSendValueRequest>> {
        let peer = self
            .peer_by_identity
            .get(&other)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no PeerId for identity {other}"))?;

        let (tx, rx): (
            UnboundedSender<ArcSendValueRequest>,
            UnboundedReceiver<ArcSendValueRequest>,
        ) = unbounded_channel();

        self.spawn_peer_sender(peer, rx);

        Ok(tx)
    }

    async fn add_connections(
        &self,
        others: &RoleAssignment,
    ) -> anyhow::Result<HashMap<Role, UnboundedSender<ArcSendValueRequest>>> {
        let mut out = HashMap::with_capacity(others.len());

        for (role, identity) in others.iter() {
            let peer = self
                .peer_by_identity
                .get(identity)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no PeerId for identity {identity} (role={role:?})"))?;

            let (tx, rx) = unbounded_channel::<ArcSendValueRequest>();
            self.spawn_peer_sender(peer, rx);
            out.insert(*role, tx);
        }

        Ok(out)
    }
}

impl Libp2pSendingService {
    pub fn new_with(
        conf: OptionConfigWrapper,
        peer_by_identity: Arc<HashMap<Identity, PeerId>>,
        mesh_client: Arc<dyn ThresholdMeshClient>,
    ) -> Self {
        Self {
            config: conf,
            peer_by_identity,
            mesh_client,
        }
    }

    fn spawn_peer_sender(&self, peer: PeerId, mut rx: UnboundedReceiver<ArcSendValueRequest>) {
        let mesh_client = self.mesh_client.clone();
        let conf = self.config.clone();

        // Defaults: bounded retries + bounded backoff + per-attempt timeout
        let max_attempts_default: usize = 5;
        let per_attempt_timeout_default = Duration::from_secs(5);
        let base_backoff = Duration::from_millis(50);
        let max_backoff = Duration::from_secs(2);

        tokio::spawn(async move {
            let max_attempts: usize = conf.conf.as_ref().and_then(|_c| None).unwrap_or(max_attempts_default);
            let per_attempt_timeout: Duration = conf
                .conf
                .as_ref()
                .and_then(|_c| None)
                .unwrap_or(per_attempt_timeout_default);

            while let Some(req) = rx.recv().await {
                // NOTE: requires ArcSendValueRequest getters (see patch below)
                let wire = WireMsg::SendValue {
                    tag: req.tag.as_ref().clone(),
                    value: req.value.as_ref().clone(),
                };

                let bytes = match bc2wrap::serialize(&wire) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!("serialize failed; terminating sender: {:#}", e);
                        break;
                    }
                };

                let mut attempt: usize = 1;
                loop {
                    // 1) perform threshold_request with per-attempt timeout
                    let resp_res: anyhow::Result<Vec<u8>> = match timeout(
                        per_attempt_timeout,
                        mesh_client.threshold_request(peer, bytes.clone()),
                    )
                    .await
                    {
                        Ok(inner) => inner,
                        Err(_) => Err(anyhow::anyhow!(
                            "threshold_request timeout after {:?}",
                            per_attempt_timeout
                        )),
                    };

                    // 2) convert transport errors to a retryable Ack
                    let ack: Ack = match resp_res {
                        Ok(resp_bytes) => decode_ack(&resp_bytes),
                        Err(e) => {
                            tracing::warn!(
                                "MPC OUTBOUND peer={} transport error (attempt={}/{}): {:#}",
                                peer,
                                attempt,
                                max_attempts,
                                e
                            );
                            Ack::Rejected {
                                code: 503,
                                message: format!("transport error: {e:#}"),
                            }
                        }
                    };

                    // 3) stop on non-transient
                    if !is_transient_ack(&ack) {
                        let (code, msg_txt) = ack_code_and_msg(&ack);
                        if code != 200 && code != 204 {
                            tracing::warn!(
                                "MPC OUTBOUND peer={} non-retryable ack code={} msg={}",
                                peer,
                                code,
                                msg_txt
                            );
                        }
                        break;
                    }

                    // 4) retry transient, bounded attempts
                    let (code, msg_txt) = ack_code_and_msg(&ack);
                    if attempt >= max_attempts {
                        tracing::warn!(
                            "MPC OUTBOUND peer={} transient ack code={} msg={} attempts_exhausted={}",
                            peer,
                            code,
                            msg_txt,
                            attempt
                        );
                        break;
                    }

                    let delay = backoff_delay(attempt, base_backoff, max_backoff);
                    tracing::warn!(
                        "MPC OUTBOUND peer={} transient ack code={} msg={} retry_in_ms={} attempt={}/{}",
                        peer,
                        code,
                        msg_txt,
                        delay.as_millis(),
                        attempt,
                        max_attempts
                    );

                    sleep(delay).await;
                    attempt += 1;
                }
            }
        });
    }
}
