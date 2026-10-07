//! gRPC-based networking.

use super::gen::gnetworking_server::{Gnetworking, GnetworkingServer};
use super::gen::{HealthCheckRequest, HealthCheckResponse, SendValueRequest, SendValueResponse};
use super::sending_service::{GrpcSendingService, NetworkSession, SendingService};
use super::tls::extract_subject_from_cert;
use super::NetworkMode;
use crate::execution::runtime::party::{MpcIdentity, Role, RoleAssignment};
use crate::networking::constants::{
    DISCARD_INACTIVE_SESSION_INTERVAL_SECS, INITIAL_INTERVAL_MS, MAX_ELAPSED_TIME,
    MAX_EN_DECODE_MESSAGE_SIZE, MAX_INTERVAL, MAX_OPENED_INACTIVE_SESSIONS_PER_PARTY,
    MAX_WAITING_TIME_MESSAGE_QUEUE, MESSAGE_LIMIT, MULTIPLIER, NETWORK_TIMEOUT_ASYNC,
    NETWORK_TIMEOUT_BK, NETWORK_TIMEOUT_BK_SNS, NETWORK_TIMEOUT_LONG,
    SESSION_CLEANUP_INTERVAL_SECS, SESSION_STATUS_UPDATE_INTERVAL_SECS,
};
use crate::networking::health_check::HealthCheckSession;
use crate::networking::Networking;
use crate::session_id::SessionId;
use crate::networking::p2p::NetworkMsgKind; // Patch ØMYSTIK
use crate::networking::session_runtime::{
    HealthTag, MessageQueueStore, NetworkRoundValue, SessionStatus, SessionStore, Tag,
    start_background_cleaning_task,
};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use tokio::sync::{
    mpsc::{channel, Sender},
    Mutex, RwLock,
};
use tokio::time::{Duration, Instant};

use tonic::transport::server::TcpConnectInfo;
use tonic::transport::CertificateDer;
use x509_parser::parse_x509_certificate;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct CoreToCoreNetworkConfig {
    pub message_limit: u64,
    pub multiplier: f64,
    pub max_interval: u64,
    pub max_elapsed_time: Option<u64>,
    /// Initial interval for exponential backoff in milliseconds (default: 1000ms)
    pub initial_interval_ms: Option<u64>,
    pub network_timeout: u64,
    pub network_timeout_bk: u64,
    pub network_timeout_bk_sns: u64,
    pub max_en_decode_message_size: u64,
    /// Background interval for updating session status (default: 60)
    pub session_update_interval_secs: Option<u64>,
    /// Background interval for cleaning up completed sessions (default: 3600)
    pub session_cleanup_interval_secs: Option<u64>,
    /// Background interval for discarding inactive sessions (default: 900)
    pub discard_inactive_sessions_interval: Option<u64>,
    /// Maximum waiting time for trying to push the message in the queue (default: 60 seconds)
    pub max_waiting_time_for_message_queue: Option<u64>,
    /// Maximum number of "Inactive" sessions a party can open before I refuse to open more (default: 100)
    pub max_opened_inactive_sessions_per_party: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct OptionConfigWrapper {
    pub conf: Option<CoreToCoreNetworkConfig>,
}

impl OptionConfigWrapper {
    pub fn get_message_limit(&self) -> usize {
        if let Some(conf) = self.conf {
            conf.message_limit as usize
        } else {
            MESSAGE_LIMIT
        }
    }

    pub fn get_multiplier(&self) -> f64 {
        if let Some(conf) = self.conf {
            conf.multiplier
        } else {
            MULTIPLIER
        }
    }

    pub fn get_max_interval(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(conf.max_interval)
        } else {
            *MAX_INTERVAL
        }
    }

    pub fn get_max_elapsed_time(&self) -> Option<Duration> {
        if let Some(conf) = self.conf {
            conf.max_elapsed_time.map(Duration::from_secs)
        } else {
            *MAX_ELAPSED_TIME
        }
    }

    pub fn get_network_timeout(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(conf.network_timeout)
        } else {
            *NETWORK_TIMEOUT_LONG
        }
    }

    pub fn get_network_timeout_bk(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(conf.network_timeout_bk)
        } else {
            *NETWORK_TIMEOUT_BK
        }
    }

    pub fn get_network_timeout_bk_sns(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(conf.network_timeout_bk_sns)
        } else {
            *NETWORK_TIMEOUT_BK_SNS
        }
    }

    pub fn get_max_en_decode_message_size(&self) -> usize {
        if let Some(conf) = self.conf {
            conf.max_en_decode_message_size as usize
        } else {
            *MAX_EN_DECODE_MESSAGE_SIZE
        }
    }

    pub fn get_initial_interval(&self) -> Duration {
        if let Some(conf) = self.conf {
            if let Some(initial_interval_ms) = conf.initial_interval_ms {
                Duration::from_millis(initial_interval_ms)
            } else {
                Duration::from_millis(INITIAL_INTERVAL_MS)
            }
        } else {
            Duration::from_millis(INITIAL_INTERVAL_MS)
        }
    }

    pub fn get_session_update_interval(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(
                conf.session_update_interval_secs
                    .unwrap_or(SESSION_STATUS_UPDATE_INTERVAL_SECS),
            )
        } else {
            Duration::from_secs(SESSION_STATUS_UPDATE_INTERVAL_SECS)
        }
    }

    pub fn get_session_cleanup_interval(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(
                conf.session_cleanup_interval_secs
                    .unwrap_or(SESSION_CLEANUP_INTERVAL_SECS),
            )
        } else {
            Duration::from_secs(SESSION_CLEANUP_INTERVAL_SECS)
        }
    }

    pub fn get_discard_inactive_sessions_interval(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(
                conf.discard_inactive_sessions_interval
                    .unwrap_or(DISCARD_INACTIVE_SESSION_INTERVAL_SECS),
            )
        } else {
            Duration::from_secs(DISCARD_INACTIVE_SESSION_INTERVAL_SECS)
        }
    }

    pub fn get_max_opened_inactive_sessions_per_party(&self) -> u64 {
        if let Some(conf) = self.conf {
            conf.max_opened_inactive_sessions_per_party
                .unwrap_or(MAX_OPENED_INACTIVE_SESSIONS_PER_PARTY)
        } else {
            MAX_OPENED_INACTIVE_SESSIONS_PER_PARTY
        }
    }

    pub fn get_max_waiting_time_for_message_queue(&self) -> Duration {
        if let Some(conf) = self.conf {
            Duration::from_secs(
                conf.max_waiting_time_for_message_queue
                    .unwrap_or(MAX_WAITING_TIME_MESSAGE_QUEUE),
            )
        } else {
            Duration::from_secs(MAX_WAITING_TIME_MESSAGE_QUEUE) // Default to 60 seconds if not specified
        }
    }
}

//TODO: Most likely need this to create NetworkStack instead of GrpcNetworking
/// GrpcNetworkingManager is responsible for managing
/// channels and message queues between MPC parties.
#[derive(Debug, Clone)]
pub struct GrpcNetworkingManager {
    // Session reference storage to prevent premature cleanup under high concurrency
    pub(crate) session_store: Arc<SessionStore>,
    // Keeps tracks of how many sessions were opened by each party
    // NOTE: Always lock session_store before opened_sessions_tracker to prevent deadlocks
    pub opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    conf: OptionConfigWrapper,
    pub sending_service: GrpcSendingService,
    #[cfg(feature = "testing")]
    pub force_tls: bool,
}

pub type GrpcServer = GnetworkingServer<NetworkingImpl>;

impl GrpcNetworkingManager {
    /// Create a new server from the networking manager.
    /// The server can be used as a tower Service.
    pub fn new_server(
        &self,
        tls_extension: TlsExtensionGetter,
    ) -> GnetworkingServer<NetworkingImpl> {
        GnetworkingServer::new(NetworkingImpl::new(
            Arc::clone(&self.session_store),
            Arc::clone(&self.opened_sessions_tracker),
            self.conf.get_message_limit(),
            self.conf.get_max_opened_inactive_sessions_per_party(),
            self.conf.get_max_waiting_time_for_message_queue(),
            tls_extension,
            #[cfg(feature = "testing")]
            self.force_tls,
        ))
        .max_decoding_message_size(self.conf.get_max_en_decode_message_size())
        .max_encoding_message_size(self.conf.get_max_en_decode_message_size())
    }

    /// Starts a background task that periodically cleans up the session store, it wakes up at every update_interval.
    ///
    /// The task discards sessions that have been completed for longer than the cleanup interval
    /// and inactive session that have been inactive for longer than the discard_inactive_interval.
    ///
    /// It also updates the status of active sessions by checking if their weak references are still valid,
    /// and if not, marks them as completed.
    

    /// Owner should be the external address
    pub fn new(
        tls_conf: Option<tokio_rustls::rustls::client::ClientConfig>,
        conf: Option<CoreToCoreNetworkConfig>,
        peer_tcp_proxy: bool,
    ) -> anyhow::Result<Self> {
        #[cfg(feature = "testing")]
        let force_tls = tls_conf.is_some();
        #[cfg(feature = "testing")]
        if !force_tls {
            tracing::warn!("force_tls is DISABLED. Testing feature is enabled - this is NOT recommended in production environments.");
        }

        #[cfg(not(feature = "testing"))]
        if tls_conf.is_none() {
            return Err(crate::error::error_handler::anyhow_error_and_log(
                "TLS configuration must be provided in non-testing environments",
            ));
        }

        let conf = OptionConfigWrapper { conf };
        let session_store = Arc::new(SessionStore::default());

        // We need to spawn background cleanup task to remove dead weak references from session_store, otherwise they accumulate and eat RAM + perf
        let cleanup_session_store = Arc::clone(&session_store);
        let update_interval = conf.get_session_update_interval();
        let cleanup_interval = conf.get_session_cleanup_interval();
        let discard_inactive_interval = conf.get_discard_inactive_sessions_interval();
        start_background_cleaning_task(
            cleanup_session_store,
            update_interval,
            cleanup_interval,
            discard_inactive_interval,
        );

        Ok(GrpcNetworkingManager {
            session_store,
            opened_sessions_tracker: Arc::new(DashMap::new()),
            conf,
            sending_service: GrpcSendingService::new(tls_conf, conf, peer_tcp_proxy)?,
            #[cfg(feature = "testing")]
            force_tls,
        })
    }

    pub async fn make_healthcheck_session(
        &self,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
    ) -> anyhow::Result<HealthCheckSession> {
        let mut others = role_assignment.clone();

        // Removing self from the role_assignment map
        // as we only want to connect to others.
        // Store my own identity in the session
        let owner = match others.remove(&my_role) {
            Some(owner) => owner,
            None => {
                return Err(anyhow::anyhow!(
                    "My role {:?} not found in role assignment {:?}",
                    my_role,
                    role_assignment
                ));
            }
        };

        let mut connection_channels = HashMap::new();
        for (role, identity) in others.inner.into_iter() {
            let channel = self
                .sending_service
                .connect_to_party(identity.clone())
                .await?;
            connection_channels.insert((role, identity), channel);
        }

        Ok(HealthCheckSession::new(
            owner,
            my_role,
            context_id,
            // We use the same timeout in HealthCheck than
            // in Sync MPC protocols
            self.conf.get_network_timeout(),
            connection_channels,
        ))
    }

    /// Create a new session from the network manager.
    ///
    /// All the communication are performed using sessions.
    /// There may be multiple session in parallel,
    /// identified by different session IDs.
    pub async fn make_network_session(
        &self,
        session_id: SessionId,
        context_id: SessionId, // not the true context ID as it's a session ID derived from the context ID
        role_assignment: &RoleAssignment,
        my_role: Role,
        network_mode: NetworkMode,
    ) -> anyhow::Result<Arc<dyn Networking + Send + Sync + 'static>> {
        let party_count = role_assignment.len();
        let mut others = role_assignment.clone();

        // Removing self from the role_assignment map
        // as we only want to connect to others.
        // Store my own identity in the session
        let owner = match others.remove(&my_role) {
            Some(owner) => owner,
            None => {
                return Err(anyhow::anyhow!(
                    "My role {:?} not found in role assignment {:?}",
                    my_role,
                    role_assignment
                ));
            }
        };

        let timeout = match network_mode {
            NetworkMode::Async => *NETWORK_TIMEOUT_ASYNC,
            NetworkMode::Sync => self.conf.get_network_timeout(),
        };

        let session: Arc<NetworkSession> = match self.session_store.entry(session_id) {
            // Turn an inactive session into an active one
            dashmap::Entry::Occupied(mut status) => {
                let mutable_status = status.get_mut();

                let message_store = if let SessionStatus::Inactive(message_store) = mutable_status {
                    // Upgrade the message store from the uninitialized state to the initialized state
                    message_store.0.init(
                        self.conf.get_message_limit(),
                        &others,
                        Arc::clone(&self.opened_sessions_tracker),
                    );
                    message_store.clone()
                } else {
                    return Err(anyhow::anyhow!(
                        "Session {:?} already exists and is not inactive for {}",
                        session_id,
                        owner
                    ));
                };

                let connection_channel = self.sending_service.add_connections(&others).await?;

                let session: Arc<NetworkSession> = Arc::new(NetworkSession { // NEW : Arc<NetworkSession>
                    owner: owner.clone(),
                    session_id,
                    context_id,
                    sending_channels: connection_channel,
                    receiving_channels: message_store.0,
                    round_counter: tokio::sync::RwLock::new(0),
                    network_mode,
                    conf: self.conf,
                    init_time: OnceLock::new(),
                    current_network_timeout: RwLock::new(timeout),
                    next_network_timeout: RwLock::new(timeout),
                    max_elapsed_time: RwLock::new(Duration::ZERO),
                    pending_by_sender_kind: DashMap::new(), // Patch ØMYSTIK
                    #[cfg(feature = "choreographer")]
                    num_byte_sent: RwLock::new(0),
                });

                *mutable_status = SessionStatus::Active(Arc::downgrade(&session));

                session
            }
            dashmap::Entry::Vacant(vacant) => {
                let connection_channel = self.sending_service.add_connections(&others).await?;

                let message_queue = MessageQueueStore::new_initialized(
                    self.conf.get_message_limit(),
                    &others,
                    Arc::clone(&self.opened_sessions_tracker),
                );

                let session: Arc<NetworkSession> = Arc::new(NetworkSession { // NEW : Arc<NetworkSession>
                    owner: owner.clone(),
                    session_id,
                    context_id,
                    sending_channels: connection_channel,
                    receiving_channels: message_queue,
                    round_counter: tokio::sync::RwLock::new(0),
                    network_mode,
                    conf: self.conf,
                    init_time: OnceLock::new(),
                    current_network_timeout: RwLock::new(timeout),
                    next_network_timeout: RwLock::new(timeout),
                    max_elapsed_time: RwLock::new(Duration::ZERO),
                    pending_by_sender_kind: DashMap::new(),
                    #[cfg(feature = "choreographer")]
                    num_byte_sent: RwLock::new(0),
                });

                vacant.insert(SessionStatus::Active(Arc::downgrade(&session)));

                session
            }
        };

        tracing::info!(
            "[SESSION_CREATION] Starting session {:?} with {} parties. (Owner: {:?})",
            session_id,
            party_count,
            owner,
        );

        Ok(session as Arc<dyn Networking + Send + Sync + 'static >) // NEW : as Arc<dyn Networking>
    }
}

// NEW
#[async_trait::async_trait]
impl super::manager::NetworkingManager for GrpcNetworkingManager {
    async fn make_healthcheck_session(
        &self,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
    ) -> anyhow::Result<HealthCheckSession> {
        GrpcNetworkingManager::make_healthcheck_session(self, context_id, role_assignment, my_role).await
    }

    async fn make_network_session(
        &self,
        session_id: SessionId,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
        network_mode: NetworkMode,
    ) -> anyhow::Result<Arc<dyn super::Networking + Send + Sync + 'static >> {
        GrpcNetworkingManager::make_network_session(
            self, session_id, context_id, role_assignment, my_role, network_mode
        ).await
    }
}

// (InitializedMessageQueueStore is defined in session_runtime.rs; do not duplicate it here.)


// Because we can use a custom TCP Incoming, we need to specify how
// to extract the TLS extension from the incoming connection
#[derive(Default)]
pub enum TlsExtensionGetter {
    #[default]
    TlsConnectInfo,
    SslConnectInfo,
}

#[derive(Default)]
pub struct NetworkingImpl {
    session_store: Arc<SessionStore>,
    // key is the MPC identity
    opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
    channel_size_limit: usize,
    max_opened_inactive_sessions: u64,
    max_waiting_time_for_message_queue: Duration,
    tls_extension: TlsExtensionGetter,
    // We gate this behind the testing feature because in non-testing environments
    // we want to ALWAYS use TLS for security reasons.
    #[cfg(feature = "testing")]
    force_tls: bool,
}

impl NetworkingImpl {
    #[allow(clippy::too_many_arguments)]
    fn new(
        session_store: Arc<SessionStore>,
        opened_sessions_tracker: Arc<DashMap<MpcIdentity, u64>>,
        channel_size_limit: usize,
        max_opened_inactive_sessions: u64,
        max_waiting_time_for_message_queue: Duration,
        tls_extension: TlsExtensionGetter,
        #[cfg(feature = "testing")] force_tls: bool,
    ) -> Self {
        Self {
            session_store: session_store.clone(),
            opened_sessions_tracker: opened_sessions_tracker.clone(),
            channel_size_limit,
            max_opened_inactive_sessions,
            max_waiting_time_for_message_queue,
            tls_extension,
            #[cfg(feature = "testing")]
            force_tls,
        }
    }

    // Did not find a better soluton yet.
    // See https://github.com/hyperium/tonic/issues/2253
    #[allow(clippy::result_large_err)]
    /// Fetches the channel for the given session and tag.
    /// - If the session is inactive, it creates a new channel for the sender (assuming the sender hasn't opened too many channels for inactive sessions yet).
    /// - If the session is active, it returns the existing channel (assuming the sender is part of the session).
    /// - If the session is completed, it returns None
    ///   to indicate that the message can be accepted but will not be processed.
    fn fetch_tx_channel(
        &self,
        session_status: &SessionStatus,
        tag: &Tag,
        kind: NetworkMsgKind,
    ) -> Result<Option<Arc<Sender<NetworkRoundValue>>>, tonic::Status> {
        match session_status {
            SessionStatus::Completed(_) => {
                tracing::debug!(
                        "Session {:?} found in session_store but is completed. Will be removed by background cleanup.",
                        tag.session_id
                    );
                // We accept the message even if we won't do anything with it
                // to avoid blocking the sender
                Ok(None)
            }
            // Session is inactive, we may need to create a new channel for the sender
            SessionStatus::Inactive(message_queue) => {
                tracing::debug!(
                    "Session {:?} found in session_store but is inactive.",
                    tag.session_id
                );
                // Check if the sender already has a channel, if it does, return it
                // if it doesn't then create a new one.
                // Note that we need to do this atomically to avoid race conditions.
                match message_queue.0.entry((tag.sender.clone(), kind)).map_err(|e| {  // Patch ØMYSTIK
                    tonic::Status::internal(format!(
                        "Failed to access message queue for session {:?}: {}",
                        tag.session_id, e
                    ))
                })? {
                    dashmap::Entry::Occupied(occupied_entry) => {
                        Ok(Some(occupied_entry.get().0.clone()))
                    }
                    dashmap::Entry::Vacant(vacant_entry_tx) => {
                        let mut opened_session_tracker_entry = self
                            .opened_sessions_tracker
                            .entry(tag.sender.clone())
                            .or_insert(0);
                        if *opened_session_tracker_entry >= self.max_opened_inactive_sessions {
                            tracing::warn!(
                                "Too many inactive sessions opened by {:?}. Have {}, Max allowed: {}",
                                tag.sender,
                                *opened_session_tracker_entry,
                                self.max_opened_inactive_sessions
                            );
                            return Err(tonic::Status::new(
                                tonic::Code::ResourceExhausted,
                                format!(
                                    "Too many inactive sessions opened by {:?}. Have {}, Max allowed: {}",
                                    tag.sender, *opened_session_tracker_entry, self.max_opened_inactive_sessions
                                ),
                            ));
                        }
                        // Create a new channel for the sender
                        let (tx, rx) = channel::<NetworkRoundValue>(self.channel_size_limit);
                        let tx = Arc::new(tx);
                        vacant_entry_tx.insert((Arc::clone(&tx), Arc::new(Mutex::new(rx))));

                        // Update the opened sessions tracker
                        *opened_session_tracker_entry += 1;
                        Ok(Some(tx))
                    }
                }
            }
            // Session is active, we can proceed with sending the message
            SessionStatus::Active(weak_session) => {
                tracing::debug!(
                    "Session {:?} found in session_store and is active.",
                    tag.session_id
                );
                // Attempt to upgrade weak reference to strong reference
                if let Some(session) = weak_session.upgrade() {
                    // Explicit type breaks the inference cycle.
                    let maybe_tx: Option<Arc<Sender<NetworkRoundValue>>> = session
                        .receiving_channels
                        .get_tx(&tag.sender, kind)
                        .map_err(|boxed: Box<tonic::Status>| *boxed)?;

                    if let Some(tx) = maybe_tx {
                        Ok(Some(tx))
                    } else {
                        let available_roles: Vec<Role> = session
                            .receiving_channels
                            .iter_roles()
                            .map_err(|boxed: Box<tonic::Status>| *boxed)?;

                        tracing::warn!(
                            "Sender {:?} not found in session {:?}. Available senders: {:?}",
                            tag.sender,
                            tag.session_id,
                            available_roles
                        );

                        Err(tonic::Status::new(
                            tonic::Code::NotFound,
                            format!("Sender {:?} not found in session {:?}", tag.sender, tag.session_id),
                        ))
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }
}

// We do the measurement of received bytes here because
// some messages may never reach the application level
// (i.e. in the Networking trait)
#[cfg(feature = "choreographer")]
lazy_static::lazy_static! {
    pub static ref NETWORK_RECEIVED_MEASUREMENT: DashMap<SessionId,usize> =
        DashMap::new();
}

fn parse_identity_context_from_cert(
    certs: Arc<Vec<CertificateDer<'static>>>,
) -> Result<(String, SessionId), Box<tonic::Status>> {
    if certs.len() != 1 {
        // it shouldn't happen because we expect TLS certificates to
        // be signed by party CA certificates directly, without any
        // intermediate CAs
        tracing::warn!("Received more than one certificate from peer, checking the first one only");
    }

    parse_x509_certificate(certs[0].as_ref())
        .map_err(|e| Box::new(tonic::Status::new(tonic::Code::Aborted, e.to_string())))
        .and_then(|(_rem, cert)| {
            let context_id = u128::from_be_bytes(cert.serial.to_bytes_be().try_into().unwrap());
            extract_subject_from_cert(&cert)
                .map(|res| (res, SessionId::from(context_id)))
                .map_err(|e| Box::new(tonic::Status::new(tonic::Code::Aborted, e.to_string())))
        })
}

// Verify that the sender in the tag matches the identity extracted from the TLS certificate
fn sender_verification(
    #[cfg(feature = "testing")] force_tls: bool,
    tag_sender: &MpcIdentity,
    valid_tls_sender: Option<String>,
) -> Result<(), Box<tonic::Status>> {
    if let Some(sender) = valid_tls_sender {
        // tag.sender is an the MPC identity, this should match the CN in the certificate
        if sender != tag_sender.0 {
            return Err(Box::new(tonic::Status::new(
                tonic::Code::Unauthenticated,
                format!(
                    "wrong sender: expected {sender} to be in in tag {}",
                    tag_sender
                ),
            )));
        }
        tracing::debug!("TLS Check went fine for sender: {:?}", sender);
    } else {
        // With testing feature, TLS is optional
        #[cfg(feature = "testing")]
        {
            if force_tls {
                // If force_tls is enabled, we require a TLS certificate
                tracing::error!("Force TLS is enabled, but no certificate found in the request.");
                return Err(Box::new(tonic::Status::new(
                    tonic::Code::Unauthenticated,
                    "Could not find a TLS certificate in the request to verify user's identity."
                        .to_string(),
                )));
            } else {
                // since we log this on _every_ send call and only use this for testing builds, we use debug level to reduce log spam
                tracing::debug!("Force TLS is disabled, and no certificate found in the request.");
            }
        }

        // Without testing feature, TLS is mandatory
        #[cfg(not(feature = "testing"))]
        {
            tracing::error!(
                "Could not find a TLS certificate in the request to verify user's identity."
            );
            return Err(Box::new(tonic::Status::new(
                tonic::Code::Unauthenticated,
                "Could not find a TLS certificate in the request to verify user's identity."
                    .to_string(),
            )));
        }
    }
    Ok(())
}

#[async_trait]
impl Gnetworking for NetworkingImpl {
    async fn health_check(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> std::result::Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        // Perform the exact same check as we do for a "real" MPC message
        let valid_tls_sender_and_context = match self.tls_extension {
            TlsExtensionGetter::TlsConnectInfo => request
                .extensions()
                .get::<tonic::transport::server::TlsConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.peer_certs().map(parse_identity_context_from_cert)),
            TlsExtensionGetter::SslConnectInfo => request
                .extensions()
                .get::<tonic_tls::rustls::SslConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.peer_certs().map(parse_identity_context_from_cert)),
        }
        .transpose()
        .map_err(|boxed| *boxed)?;
        let request = request.into_inner();
        let health_tag = bc2wrap::deserialize_safe::<HealthTag>(&request.tag).map_err(|e| {
            tonic::Status::new(
                tonic::Code::Aborted,
                format!("failed to parse value: {} as a HealthTag", e),
            )
        })?;

        sender_verification(
            #[cfg(feature = "testing")]
            self.force_tls,
            &health_tag.sender,
            valid_tls_sender_and_context.map(|(sender, _)| sender),
        )
        .map_err(|e| *e)?;

        tracing::info!("Received a HealthPing from {}", health_tag.sender);
        Ok(tonic::Response::new(HealthCheckResponse::default()))
    }

    async fn send_value(
        &self,
        request: tonic::Request<SendValueRequest>,
    ) -> std::result::Result<tonic::Response<SendValueResponse>, tonic::Status> {
        // If TLS is enabled, A SAN may look like:
        // DNS:party1.com, IP Address:127.0.0.1, DNS:localhost, IP Address:192.168.0.1, IP Address:0:0:0:0:0:0:0:1
        // which is a collection of DNS names and IP addresses.
        // The DNS component must match the "tag" that's in the request for identity verification,
        // in this case it's party1.com.
        // We also require party1.com to be the subject and the issuer CN too,
        // since we're using self-signed certificates at the moment.
        let valid_tls_sender_and_context = match self.tls_extension {
            TlsExtensionGetter::TlsConnectInfo => request
                .extensions()
                .get::<tonic::transport::server::TlsConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.peer_certs().map(parse_identity_context_from_cert)),
            TlsExtensionGetter::SslConnectInfo => request
                .extensions()
                .get::<tonic_tls::rustls::SslConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.peer_certs().map(parse_identity_context_from_cert)),
        }
        .transpose()
        .map_err(|boxed| *boxed)?;

        let request = request.into_inner();
        let tag = bc2wrap::deserialize_safe::<Tag>(&request.tag).map_err(|e| {
            tonic::Status::new(
                tonic::Code::Aborted,
                format!("failed to parse value: {}", e),
            )
        })?;

        let kind = tag.kind; // Patch ØMYSTIK

    
        // Extract context ID
        // If TLS is used, it is taken from the certificate and is trusted
        let context_id = match valid_tls_sender_and_context {
            Some((_, context_id)) => context_id,
            None => tag.context_id,
        };

        sender_verification(
            #[cfg(feature = "testing")]
            self.force_tls,
            &tag.sender,
            valid_tls_sender_and_context.map(|(sender, _)| sender),
        )
        .map_err(|e| *e)?;

        tracing::debug!(
            "Starting session lookup for session_id={:?},context_id={:?}, sender={:?}, round={}",
            tag.session_id,
            context_id,
            tag.sender,
            tag.round_counter,
        );

        #[cfg(feature = "choreographer")]
        {
            match NETWORK_RECEIVED_MEASUREMENT.entry(tag.session_id) {
                dashmap::Entry::Occupied(mut occupied_entry) => {
                    let entry = occupied_entry.get_mut();
                    *entry += request.tag.len() + request.value.len()
                }
                dashmap::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(request.tag.len() + request.value.len());
                }
            };
        }

        // First try with only read lock to avoid blocking
        let tx = if let Some(session_status) = self.session_store.get(&tag.session_id) {
            match self.fetch_tx_channel(session_status.value(), &tag, kind)? { // Patch ØMYSTIK
                Some(tx) => tx,
                None => {
                    // If the session is completed or inactive, we return early
                    return Ok(tonic::Response::new(SendValueResponse::default()));
                }
            }
        } else {
            // We write lock the session store to create a new one
            match self.session_store.entry(tag.session_id) {
                dashmap::Entry::Occupied(occupied_entry) => {
                    // Can be occupied if ever state has changed by the time we reach this branch of the if statement
                    match self.fetch_tx_channel(occupied_entry.get(), &tag, kind)? { // Patch ØMYSTIK
                        Some(tx) => tx,
                        None => {
                            // If the session is completed or inactive, we return early
                            return Ok(tonic::Response::new(SendValueResponse::default()));
                        }
                    }
                }
                dashmap::Entry::Vacant(vacant_entry) => {
                    tracing::debug!(
                        "Session {:?} not found in session_store, creating a new inactive one.",
                        tag.session_id
                    );
                    let mut opened_session_tracker_entry = self
                        .opened_sessions_tracker
                        .entry(tag.sender.clone())
                        .or_insert(0);
                    if *opened_session_tracker_entry >= self.max_opened_inactive_sessions {
                        tracing::warn!(
                            "Too many inactive sessions opened by {:?}. Got {}, Max allowed: {}",
                            &tag.sender,
                            *opened_session_tracker_entry,
                            self.max_opened_inactive_sessions
                        );
                        return Err(tonic::Status::new(
                            tonic::Code::ResourceExhausted,
                            format!(
                                "Too many inactive sessions opened by {:?}. Got {}, Max allowed: {}",
                                tag.sender,*opened_session_tracker_entry, self.max_opened_inactive_sessions
                            ),
                        ));
                    }
                    // Create a new session with an inactive status
                    let channel_maps = DashMap::new();
                    let (tx, rx) = channel::<NetworkRoundValue>(self.channel_size_limit);
                    let tx = Arc::new(tx);
                    channel_maps.insert(
                        (tag.sender.clone(), kind),  // Patch ØMYSTIK
                        (Arc::clone(&tx), Arc::new(Mutex::new(rx))),
                    );

                    // Insert the new session into the store
                    vacant_entry.insert(SessionStatus::Inactive((
                        MessageQueueStore::new_uninitialized(channel_maps),
                        Instant::now(),
                    )));
                    *opened_session_tracker_entry += 1;
                    tx
                }
            }
        };

        // Send message - ignore send errors as receiver may have dropped
        let send_result = tokio::time::timeout(
            self.max_waiting_time_for_message_queue,
            tx.send(NetworkRoundValue {
                value: request.value,
                session_id: tag.session_id,
                context_id,
                round_counter: tag.round_counter,
                kind, // Patch ØMYSTIK
            }),
        )
        .await;

        if let Err(e) = send_result {
            tracing::warn!(
            "Failed to process value for session {:?}, sender {:?}, round {}. Queue has been full for {} seconds.",
            tag.session_id,
            &tag.sender,
            tag.round_counter,
            self.max_waiting_time_for_message_queue.as_secs()
        );

            return Err(tonic::Status::new(
                tonic::Code::ResourceExhausted,
                format!(
                    "Failed to process value for session {:?}, sender {:?}, round {}: {:?}",
                    tag.session_id, tag.sender, tag.round_counter, e
                ),
            ));
        }

        Ok(tonic::Response::new(SendValueResponse::default()))
    }
}