use crate::execution::runtime::party::MpcIdentity;
use crate::networking::session_runtime::{
    HealthTag, MessageQueueStore, NetworkRoundValue, SessionStatus, SessionStore, Tag,
};
use crate::networking::p2p::NetworkMsgKind;
use dashmap::DashMap;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};

/// Ack returned to sender over libp2p (mirrors tonic Status in intent).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Ack {
    Ok,
    DroppedCompletedOrClosed,
    Rejected { code: u16, message: String },
}

/// Libp2p inbound "threshold request" payload for SendValue-style messages.
/// This is the libp2p equivalent of gRPC `send_value(tag_bytes, value_bytes)`,
/// but arrives as a single blob that must be decoded into (tag, payload).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThresholdInboundSendValue {
    pub tag: Vec<u8>,
    pub payload: Vec<u8>,
}

#[derive(Clone)]
pub struct InboundRouter {
    pub session_store: Arc<SessionStore>,
    pub opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    pub channel_size_limit: usize,
    pub max_opened_inactive_sessions: u64,
    pub max_waiting_time_for_message_queue: Duration,
}

impl InboundRouter {
    /// Inbound entrypoint for libp2p threshold requests that are a single encoded blob.
    ///
    /// Decodes into (Tag bytes, payload bytes), then routes exactly like gRPC `send_value`.
    /// Includes:
    /// - ResourceExhausted-style limits (inactive sessions per sender; queue wait timeout)
    /// - max waiting time for the destination message queue
    pub async fn handle_inbound_threshold_request(&self, req_bytes: &[u8]) -> Ack {
        let decoded = match bc2wrap::deserialize_safe::<ThresholdInboundSendValue>(req_bytes) {
            Ok(v) => v,
            Err(e) => {
                return Ack::Rejected {
                    code: 400,
                    message: format!("bad inbound request: {e}"),
                }
            }
        };

        self.handle_send_value(&decoded.tag, decoded.payload).await
    }

    /// Enqueue an MPC message. Sender identity must already be validated
    /// against the transport identity (PeerId->MpcIdentity mapping).
    pub async fn handle_send_value(&self, tag_bytes: &[u8], value: Vec<u8>) -> Ack {
        // 1) Decode tag
        let tag = match bc2wrap::deserialize_safe::<Tag>(tag_bytes) {
            Ok(t) => t,
            Err(e) => {
                return Ack::Rejected {
                    code: 400,
                    message: format!("bad tag: {e}"),
                }
            }
        };

        // 2) Kind MUST come from tag (source of truth)
        let kind = tag.kind;

        tracing::debug!(
            session_id = ?tag.session_id,
            context_id = ?tag.context_id,
            round_counter = tag.round_counter,
            value_len = value.len(),
            kind = ?kind,
            "MPC ROUTER received SendValue"
        );

        // 3) Find or create tx channel for this (session_id, sender, kind),
        // respecting SessionStatus: Inactive / Active / Completed.
        let tx: Arc<Sender<NetworkRoundValue>> = match self.get_or_create_tx(&tag, kind) {
            Ok(Some(tx)) => tx,
            Ok(None) => return Ack::DroppedCompletedOrClosed,
            Err((code, msg)) => return Ack::Rejected { code, message: msg },
        };

        // 4) Push into queue with max waiting time (ResourceExhausted-like on timeout).
        let send_res = timeout(
            self.max_waiting_time_for_message_queue,
            tx.send(NetworkRoundValue {
                value,
                session_id: tag.session_id,
                context_id: tag.context_id,
                round_counter: tag.round_counter,
                kind,
            }),
        )
        .await;

        match send_res {
            Ok(Ok(_)) => Ack::Ok,
            Ok(Err(_closed)) => Ack::DroppedCompletedOrClosed,
            Err(_elapsed) => Ack::Rejected {
                code: 429,
                message: format!(
                    "queue full for {}s",
                    self.max_waiting_time_for_message_queue.as_secs()
                ),
            },
        }
    }

    pub fn handle_health_check(&self, tag_bytes: &[u8]) -> Ack {
        match bc2wrap::deserialize_safe::<HealthTag>(tag_bytes) {
            Ok(_tag) => Ack::Ok,
            Err(e) => Ack::Rejected {
                code: 400,
                message: format!("bad health tag: {e}"),
            },
        }
    }

    fn get_or_create_tx(
        &self,
        tag: &Tag,
        kind: NetworkMsgKind,
    ) -> Result<Option<Arc<Sender<NetworkRoundValue>>>, (u16, String)> {
        // Fast path: session exists; refresh inactive timestamp if applicable.
        if let Some(mut session_status) = self.session_store.get_mut(&tag.session_id) {
            if let SessionStatus::Inactive((_mq, started)) = session_status.value_mut() {
                // keepalive: receiving traffic means the session is still "relevant"
                *started = Instant::now();
            }
            return self.fetch_tx_channel(session_status.value(), tag, kind);
        }

        // Slow path: create inactive session entry
        match self.session_store.entry(tag.session_id) {
            dashmap::Entry::Occupied(mut occupied) => {
                if let SessionStatus::Inactive((_mq, started)) = occupied.get_mut() {
                    *started = Instant::now();
                }
                self.fetch_tx_channel(occupied.get(), tag, kind)
            }

            dashmap::Entry::Vacant(vacant) => {
                // IMPORTANT: this counter tracks inactive sessions opened by a sender.
                let mut opened_entry = self
                    .opened_sessions_tracker
                    .entry(tag.sender.clone())
                    .or_insert(0);

                // ResourceExhausted-like guard
                if *opened_entry >= self.max_opened_inactive_sessions {
                    return Err((
                        429,
                        format!(
                            "too many inactive sessions opened by {:?} (have {}, max {})",
                            tag.sender, *opened_entry, self.max_opened_inactive_sessions
                        ),
                    ));
                }

                // Create the initial (sender, kind) channel map for this inactive session.
                let channel_maps: DashMap<
                    (MpcIdentity, NetworkMsgKind),
                    (
                        Arc<Sender<NetworkRoundValue>>,
                        Arc<Mutex<tokio::sync::mpsc::Receiver<NetworkRoundValue>>>,
                    ),
                > = DashMap::new();

                let (tx, rx) = channel::<NetworkRoundValue>(self.channel_size_limit);
                let tx = Arc::new(tx);

                channel_maps.insert(
                    (tag.sender.clone(), kind),
                    (Arc::clone(&tx), Arc::new(Mutex::new(rx))),
                );

                vacant.insert(SessionStatus::Inactive((
                    MessageQueueStore::new_uninitialized(channel_maps),
                    Instant::now(),
                )));

                // Increment only once per inactive session creation.
                *opened_entry += 1;

                Ok(Some(tx))
            }
        }
    }

    fn fetch_tx_channel(
        &self,
        session_status: &SessionStatus,
        tag: &Tag,
        kind: NetworkMsgKind,
    ) -> Result<Option<Arc<Sender<NetworkRoundValue>>>, (u16, String)> {
        match session_status {
            SessionStatus::Completed(_) => Ok(None),

            SessionStatus::Inactive(message_queue) => {
                match message_queue
                    .0
                    .entry((tag.sender.clone(), kind))
                    .map_err(|e| (500, format!("message queue access failed: {e}")))? {
                    dashmap::Entry::Occupied(occ) => Ok(Some(occ.get().0.clone())),
                    dashmap::Entry::Vacant(vac) => {
                        // NOTE: The ResourceExhausted limit here is session-level
                        // (max_opened_inactive_sessions) to match gRPC behavior.
                        // We do not count per-kind queues against that limit.

                        let (tx, rx) = channel::<NetworkRoundValue>(self.channel_size_limit);
                        let tx = Arc::new(tx);
                        vac.insert((Arc::clone(&tx), Arc::new(Mutex::new(rx))));
                        Ok(Some(tx))
                    }
                }
            }

            SessionStatus::Active(weak_session) => {
                if let Some(session) = weak_session.upgrade() {
                    // MUST use kind for demux
                    match session.receiving_channels.get_tx(&tag.sender, kind) {
                        Ok(Some(tx)) => Ok(Some(tx)),
                        Ok(None) => Err((
                            404,
                            format!(
                                "sender {:?} (kind {:?}) not in session {:?}",
                                tag.sender, kind, tag.session_id
                            ),
                        )),
                        Err(boxed) => Err((500, boxed.to_string())),
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }
}
