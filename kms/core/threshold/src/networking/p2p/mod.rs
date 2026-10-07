pub mod node;
pub mod protocol;
pub mod router;
pub mod sending_service_p2p;
pub mod crypto_gateway_service;

use crate::execution::runtime::party::{Role, RoleAssignment, MpcIdentity};
use crate::networking::grpc::{CoreToCoreNetworkConfig, OptionConfigWrapper};
use crate::networking::session_runtime::{MessageQueueStore, SessionStore, SessionStatus, start_background_cleaning_task};
use crate::networking::health_check::HealthCheckSession;
use crate::networking::sending_service::NetworkSession;
use crate::networking::{NetworkMode, Networking};
use crate::networking::manager::NetworkingManager;
use crate::session_id::SessionId;

use dashmap::DashMap;
use std::{sync::{Arc, OnceLock}, time::Duration};
use tokio::sync::RwLock;

use router::InboundRouter;
use sending_service_p2p::Libp2pSendingService;
use crate::networking::sending_service::SendingService;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NetworkMsgKind {
    Send,
    EchoBatch,
    VoteBatch,
    Other,
}

impl NetworkMsgKind {
    pub const ALL: &'static [NetworkMsgKind] = &[
        NetworkMsgKind::Send,
        NetworkMsgKind::EchoBatch,
        NetworkMsgKind::VoteBatch,
        NetworkMsgKind::Other,
    ];
}


#[derive(Clone)]
pub struct Libp2pNetworkingManager {
    pub session_store: Arc<SessionStore>,
    pub opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    pub conf: OptionConfigWrapper,
    pub sending_service: Libp2pSendingService,
}

impl Libp2pNetworkingManager {
    pub fn new(
        conf: Option<CoreToCoreNetworkConfig>,
        sending_service: Libp2pSendingService,
    ) -> Self {
        let conf = OptionConfigWrapper { conf };
        let session_store = Arc::new(SessionStore::default());
        let opened_sessions_tracker = Arc::new(DashMap::new());

        // Reuse the exact same cleaning loop as the gRPC transport.
        start_background_cleaning_task(
            Arc::clone(&session_store),
            conf.get_session_update_interval(),
            conf.get_session_cleanup_interval(),
            conf.get_discard_inactive_sessions_interval(),
        );

        Self {
            session_store,
            opened_sessions_tracker,
            conf,
            sending_service,
        }
    }

    pub fn inbound_router(&self) -> InboundRouter {
        InboundRouter {
            session_store: Arc::clone(&self.session_store),
            opened_sessions_tracker: Arc::clone(&self.opened_sessions_tracker),
            channel_size_limit: self.conf.get_message_limit(),
            max_opened_inactive_sessions: self.conf.get_max_opened_inactive_sessions_per_party(),
            max_waiting_time_for_message_queue: self.conf.get_max_waiting_time_for_message_queue(),
        }
    }

    pub fn new_with_shared(
        conf: Option<CoreToCoreNetworkConfig>,
        sending_service: Libp2pSendingService,
        session_store: Arc<SessionStore>,
        opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    ) -> Self {
        let conf = OptionConfigWrapper { conf };

        start_background_cleaning_task(
            Arc::clone(&session_store),
            conf.get_session_update_interval(),
            conf.get_session_cleanup_interval(),
            conf.get_discard_inactive_sessions_interval(),
        );

        Self {
            session_store,
            opened_sessions_tracker,
            conf,
            sending_service,
        }
    }
// cleanup loop is shared in networking/session_runtime.rs (start_background_cleaning_task)
    
    
}

#[async_trait::async_trait]
impl NetworkingManager for Libp2pNetworkingManager {
    async fn make_healthcheck_session(
        &self,
        _context_id: SessionId,
        _role_assignment: &RoleAssignment,
        _my_role: Role,
    ) -> anyhow::Result<HealthCheckSession> {
        // If you require HealthCheckSession semantics identical to gRPC, you can:
        // - either reimplement HealthCheckSession over libp2p, or
        // - implement a minimal shim that pings peers using WireMsg::HealthCheck.
        Err(anyhow::anyhow!("healthcheck over libp2p not implemented yet"))
    }

    async fn make_network_session(
        &self,
        session_id: SessionId,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
        network_mode: NetworkMode,
    ) -> anyhow::Result<Arc<dyn Networking  + Send + Sync + 'static>> {
        let party_count = role_assignment.len();
        let mut others = role_assignment.clone();

        let owner = others.remove(&my_role).ok_or_else(|| {
            anyhow::anyhow!("My role {:?} not found in role assignment", my_role)
        })?;

        let timeout: Duration = match network_mode {
            NetworkMode::Async => Duration::from_secs(30),
            NetworkMode::Sync => self.conf.get_network_timeout(),
        };


        let session = match self.session_store.entry(session_id) {
            dashmap::Entry::Occupied(mut status) => {
                let mutable = status.get_mut();
                let message_store = if let SessionStatus::Inactive(store) = mutable {
                    store.0.init(
                        self.conf.get_message_limit(),
                        &others,
                        Arc::clone(&self.opened_sessions_tracker),
                    );
                    store.clone()
                } else {
                    return Err(anyhow::anyhow!("Session {:?} exists and is not inactive", session_id));
                };

                let sending_channels = self.sending_service.add_connections(&others).await?;

                let session = Arc::new(NetworkSession {
                    owner: owner.clone(),
                    session_id,
                    context_id,
                    sending_channels,
                    receiving_channels: message_store.0,
                    round_counter: tokio::sync::RwLock::new(0),
                    #[cfg(feature = "choreographer")]
                    num_byte_sent: RwLock::new(0),
                    network_mode,
                    conf: self.conf,
                    init_time: OnceLock::new(),
                    current_network_timeout: RwLock::new(timeout),
                    next_network_timeout: RwLock::new(timeout),
                    max_elapsed_time: RwLock::new(Duration::ZERO),
                    pending_by_sender_kind: DashMap::new(),
                });

                *mutable = SessionStatus::Active(Arc::downgrade(&session));
                session
            }
            dashmap::Entry::Vacant(vacant) => {
                let sending_channels = self.sending_service.add_connections(&others).await?;

                let message_queue = MessageQueueStore::new_initialized(
                    self.conf.get_message_limit(),
                    &others,
                    Arc::clone(&self.opened_sessions_tracker),
                );

                let session = Arc::new(NetworkSession {
                    owner: owner.clone(),
                    session_id,
                    context_id,
                    sending_channels,
                    receiving_channels: message_queue,
                    round_counter: tokio::sync::RwLock::new(0),
                    #[cfg(feature = "choreographer")]
                    num_byte_sent: RwLock::new(0),
                    network_mode,
                    conf: self.conf,
                    init_time: OnceLock::new(),
                    current_network_timeout: RwLock::new(timeout),
                    next_network_timeout: RwLock::new(timeout),
                    max_elapsed_time: RwLock::new(Duration::ZERO),
                    pending_by_sender_kind: DashMap::new(),
                });

                vacant.insert(SessionStatus::Active(Arc::downgrade(&session)));
                session
            }
        };

        tracing::info!(
            "[SESSION_CREATION] Starting libp2p session {:?} with {} parties. (Owner: {:?})",
            session_id,
            party_count,
            owner,
        );

        Ok(session as Arc<dyn Networking + Send + Sync + 'static>)
    }
}
