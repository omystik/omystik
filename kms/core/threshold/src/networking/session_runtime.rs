//! Transport-neutral session/runtime primitives shared by all transports (gRPC, libp2p, ...).

use crate::execution::runtime::party::{MpcIdentity, Role, RoleAssignment};
use crate::networking::p2p::NetworkMsgKind;
use crate::networking::sending_service::NetworkSession;
use crate::session_id::SessionId;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Weak};

use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    Mutex,
};
use tokio::time::{Duration, Instant};

/// We need a counter for each value sent over the local queues
/// so that messages that haven't been picked up using receive() calls will get dropped.
#[derive(Debug)]
pub struct NetworkRoundValue {
    pub value: Vec<u8>,
    pub session_id: SessionId,
    pub context_id: SessionId,
    pub round_counter: usize,
    pub kind: NetworkMsgKind,
}

#[derive(Debug, Clone)]
pub(crate) struct InitializedMessageQueueStore {
    // (mpc_identity, kind) -> tx
    tx: DashMap<(MpcIdentity, NetworkMsgKind), Arc<Sender<NetworkRoundValue>>>,
    // (role, kind) -> rx
    rx: DashMap<(Role, NetworkMsgKind), Arc<Mutex<Receiver<NetworkRoundValue>>>>,
}

#[allow(clippy::type_complexity)]
#[derive(Debug, Clone)]
pub(crate) enum MessageQueueStore {
    Uninitialized(
        DashMap<
            (MpcIdentity, NetworkMsgKind),
            (
                Arc<Sender<NetworkRoundValue>>,
                Arc<Mutex<Receiver<NetworkRoundValue>>>,
            ),
        >,
    ),
    Initialized(InitializedMessageQueueStore),
}

impl MessageQueueStore {
    // We allocate queues for these kinds, plus "Other" as a safety bucket.
    const ALL_KINDS: [NetworkMsgKind; 4] = [
        NetworkMsgKind::Send,
        NetworkMsgKind::EchoBatch,
        NetworkMsgKind::VoteBatch,
        NetworkMsgKind::Other,
    ];

    #[allow(clippy::type_complexity)]
    pub(crate) fn new_uninitialized(
        channel_maps: DashMap<
            (MpcIdentity, NetworkMsgKind),
            (
                Arc<Sender<NetworkRoundValue>>,
                Arc<Mutex<Receiver<NetworkRoundValue>>>,
            ),
        >,
    ) -> Self {
        MessageQueueStore::Uninitialized(channel_maps)
    }

    pub(crate) fn new_initialized(
        channel_size_limit: usize,
        others: &RoleAssignment,
        opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    ) -> Self {
        let mut out = Self::new_uninitialized(DashMap::new());
        out.init(channel_size_limit, others, opened_sessions_tracker);
        out
    }

    pub(crate) fn init(
        &mut self,
        channel_size_limit: usize,
        others: &RoleAssignment,
        opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    ) {
        let channel_maps = match self {
            MessageQueueStore::Uninitialized(m) => m,
            MessageQueueStore::Initialized(_) => {
                tracing::warn!("MessageQueueStore is already initialized");
                return;
            }
        };

        let tx_map: DashMap<(MpcIdentity, NetworkMsgKind), Arc<Sender<NetworkRoundValue>>> =
            DashMap::new();
        let rx_map: DashMap<(Role, NetworkMsgKind), Arc<Mutex<Receiver<NetworkRoundValue>>>> =
            DashMap::new();

        for (role, identity) in others.iter() {
            let mpc_id = identity.mpc_identity();

            // We may find 0..N pre-created (mpc_id, kind) entries.
            // But opened_sessions_tracker is session-level, so decrement at most once per mpc_id.
            let mut consumed_inactive_entry_for_mpc = false;

            for kind in Self::ALL_KINDS {
                let key = (mpc_id.clone(), kind);

                if let Some(entry) = channel_maps.get(&key) {
                    // Decrement once per mpc_id, not once per kind.
                    if !consumed_inactive_entry_for_mpc {
                        consumed_inactive_entry_for_mpc = true;
                        opened_sessions_tracker
                            .entry(mpc_id.clone())
                            .and_modify(|count| *count = count.saturating_sub(1))
                            .or_insert(0);
                    }

                    let (tx, rx) = entry.value();
                    tx_map.insert((mpc_id.clone(), kind), tx.clone());
                    rx_map.insert((*role, kind), rx.clone());
                } else {
                    let (tx, rx) = channel::<NetworkRoundValue>(channel_size_limit);
                    tx_map.insert((mpc_id.clone(), kind), Arc::new(tx));
                    rx_map.insert((*role, kind), Arc::new(Mutex::new(rx)));
                }
            }
        }

        *self = MessageQueueStore::Initialized(InitializedMessageQueueStore {
            tx: tx_map,
            rx: rx_map,
        });
    }

    pub(crate) fn get_tx(
        &self,
        mpc_identity: &MpcIdentity,
        kind: NetworkMsgKind,
    ) -> Result<Option<Arc<Sender<NetworkRoundValue>>>, Box<tonic::Status>> {
        match self {
            MessageQueueStore::Initialized(store) => Ok(store
                .tx
                .get(&(mpc_identity.clone(), kind))
                .map(|e| e.value().clone())),
            MessageQueueStore::Uninitialized(_) => Err(Box::new(tonic::Status::internal(
                format!(
                    "trying to get tx message queue for {:?} while it is not initialized",
                    mpc_identity
                ),
            ))),
        }
    }

    pub(crate) fn get_rx(
        &self,
        role: &Role,
        kind: NetworkMsgKind,
    ) -> anyhow::Result<Option<Arc<Mutex<Receiver<NetworkRoundValue>>>>> {
        match self {
            MessageQueueStore::Initialized(store) => Ok(store.rx.get(&(*role, kind)).map(|e| e.value().clone())),
            MessageQueueStore::Uninitialized(_) => Err(anyhow::anyhow!(
                "trying to get rx message queue for role {role:?} while it is not initialized",
            )),
        }
    }

    // this must be performed on the uninitialized message queue
    #[allow(clippy::type_complexity)]
    pub(crate) fn entry(
        &self,
        key: (MpcIdentity, NetworkMsgKind),
    ) -> anyhow::Result<
        dashmap::mapref::entry::Entry<
            '_,
            (MpcIdentity, NetworkMsgKind),
            (
                Arc<Sender<NetworkRoundValue>>,
                Arc<Mutex<Receiver<NetworkRoundValue>>>,
            ),
        >,
    > {
        match self {
            MessageQueueStore::Uninitialized(inner) => Ok(inner.entry(key)),
            MessageQueueStore::Initialized(_) => Err(anyhow::anyhow!(
                "entry() can only be performed on uninitialized message queue"
            )),
        }
    }

    /// Returns the roles present in this store (unique roles; kinds are internal).
    pub(crate) fn iter_roles(&self) -> Result<Vec<Role>, Box<tonic::Status>> {
        match self {
            MessageQueueStore::Uninitialized(_) => Err(Box::new(tonic::Status::internal(
                "trying to iterate roles when message queue is not initialized",
            ))),
            MessageQueueStore::Initialized(inner) => {
                use std::collections::HashSet;
                let mut seen = HashSet::new();
                let mut out = Vec::new();
                for entry in inner.rx.iter() {
                    let (role, _kind) = *entry.key();
                    if seen.insert(role) {
                        out.push(role);
                    }
                }
                Ok(out)
            }
        }
    }
}

pub type SessionStore = DashMap<SessionId, SessionStatus>;

#[derive(Debug)]
/// Represents the status of a session in the session store.
/// It can be:
/// - Completed: The session has been completed and the timestamp of completion is stored.
/// - Inactive: The session is inactive (I haven't yet heard about the request) and has a message queue store for senders.
/// - Active: The session is active (I know about the request) and holds a weak reference to the `NetworkSession`.
pub enum SessionStatus {
    Completed(Instant),
    Inactive((MessageQueueStore, Instant)),
    Active(Weak<NetworkSession>),
}

/// Starts a background task that periodically cleans up the session store, it wakes up at every update_interval.
///
/// The task discards sessions that have been completed for longer than the cleanup interval
/// and inactive session that have been inactive for longer than the discard_inactive_interval.
///
/// It also updates the status of active sessions by checking if their weak references are still valid,
/// and if not, marks them as completed.
pub(crate) fn start_background_cleaning_task(
    session_store: Arc<SessionStore>,
    update_interval: Duration,
    cleanup_interval: Duration,
    discard_inactive_interval: Duration,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(update_interval);
        loop {
            interval.tick().await;

            session_store.retain(|session_id, status| match status {
                SessionStatus::Completed(started) => started.elapsed() < cleanup_interval,
                SessionStatus::Inactive((_, started)) => {
                    if started.elapsed() > discard_inactive_interval {
                        tracing::warn!(
                            "Discarding Inactive session {:?} after {:?} seconds. We never heard about such session.",
                            session_id,
                            started.elapsed().as_secs()
                        );
                        false
                    } else {
                        true
                    }
                }
                SessionStatus::Active(session) => {
                    if session.upgrade().is_none() {
                        *status = SessionStatus::Completed(Instant::now());
                    }
                    true
                }
            });
        }
    });
}

fn default_network_msg_kind() -> NetworkMsgKind {
    NetworkMsgKind::Other
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tag {
    pub(crate) session_id: SessionId,
    pub(crate) sender: MpcIdentity,
    pub(crate) context_id: SessionId,
    pub(crate) round_counter: usize,
    #[serde(default = "default_network_msg_kind")]
    pub(crate) kind: NetworkMsgKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HealthTag {
    pub(crate) sender: MpcIdentity,
    pub(crate) context_id: SessionId,
}

impl Tag {
    /// Public constructor for crates outside `kms_threshold`.
    pub fn new(
        session_id: SessionId,
        sender: MpcIdentity,
        context_id: SessionId,
        round_counter: usize,
        kind: NetworkMsgKind,
    ) -> Self {
        Self {
            session_id,
            sender,
            context_id,
            round_counter,
            kind,
        }
    }

    // Optional getters (handy for logging/debugging outside the crate)
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub fn sender(&self) -> &MpcIdentity {
        &self.sender
    }
    pub fn context_id(&self) -> &SessionId {
        &self.context_id
    }
    pub fn round_counter(&self) -> usize {
        self.round_counter
    }
    pub fn kind(&self) -> NetworkMsgKind {
        self.kind
    }
}

impl HealthTag {
    /// Public constructor for crates outside `kms_threshold`.
    pub fn new(sender: MpcIdentity, context_id: SessionId) -> Self {
        Self { sender, context_id }
    }

    // Optional getters
    pub fn sender(&self) -> &MpcIdentity {
        &self.sender
    }
    pub fn context_id(&self) -> &SessionId {
        &self.context_id
    }
}
