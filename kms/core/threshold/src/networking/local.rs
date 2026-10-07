// entirely re-write for ØMYSTIK
use crate::error::error_handler::anyhow_error_and_log;
use crate::execution::runtime::party::Role;

use super::*;
use constants::{
    NETWORK_TIMEOUT, NETWORK_TIMEOUT_ASYNC, NETWORK_TIMEOUT_BK, NETWORK_TIMEOUT_BK_SNS,
};

use dashmap::DashMap;
use futures_util::future::{join, join4};
use std::cmp::min;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};
use tokio::time::{Duration, Instant};

// IMPORTANT: import your enum
use crate::networking::p2p::NetworkMsgKind;

pub struct LocalNetworking {
    current_network_timeout: Mutex<Duration>,
    next_network_timeout: Mutex<Duration>,
    max_elapsed_time: Mutex<Duration>,

    pairwise_channels: SimulatedPairwiseChannels,
    pub owner: Role,

    pub network_round: Arc<Mutex<usize>>,

    // IMPORTANT: include kind so Send + EchoBatch in same round is allowed
    already_sent: Arc<Mutex<HashSet<(Role, usize, NetworkMsgKind)>>>,

    pub init_time: OnceLock<Instant>,
    network_mode: NetworkMode,
    delayed_party: Option<Duration>,
}

impl Default for LocalNetworking {
    fn default() -> Self {
        Self {
            current_network_timeout: Mutex::new(*NETWORK_TIMEOUT),
            next_network_timeout: Mutex::new(*NETWORK_TIMEOUT),
            max_elapsed_time: Mutex::new(Duration::ZERO),
            pairwise_channels: Default::default(),
            owner: Default::default(),
            network_round: Arc::new(Mutex::new(0)),
            already_sent: Arc::new(Mutex::new(HashSet::new())),
            init_time: OnceLock::new(),
            network_mode: NetworkMode::Sync,
            delayed_party: None,
        }
    }
}

#[derive(Default)]
pub struct LocalNetworkingProducer {
    pairwise_channels: SimulatedPairwiseChannels,
}

impl LocalNetworkingProducer {
    pub fn from_roles(roles: &HashSet<Role>) -> Self {
        let pairwise_channels: DashMap<
            (Role, Role, NetworkMsgKind),
            (Arc<UnboundedSender<LocalTaggedValue>>, Arc<Mutex<UnboundedReceiver<LocalTaggedValue>>>),
        > = DashMap::new();

        for &v1 in roles {
            for &v2 in roles {
                if v1 == v2 {
                    continue;
                }
                // Create one unbounded channel per (sender, receiver, kind)
                for &kind in NetworkMsgKind::ALL {
                    let (tx, rx) = unbounded_channel::<LocalTaggedValue>();
                    pairwise_channels.insert(
                        (v1, v2, kind),
                        (Arc::new(tx), Arc::new(Mutex::new(rx))),
                    );
                }
            }
        }

        Self {
            pairwise_channels: Arc::new(pairwise_channels),
        }
    }

    pub fn user_net(
        &self,
        owner: Role,
        network_mode: NetworkMode,
        delayed_party: Option<Duration>,
    ) -> LocalNetworking {
        let timeout = match network_mode {
            NetworkMode::Sync => *NETWORK_TIMEOUT,
            NetworkMode::Async => *NETWORK_TIMEOUT_ASYNC,
        };

        LocalNetworking {
            pairwise_channels: Arc::clone(&self.pairwise_channels),
            owner,
            network_mode,
            current_network_timeout: Mutex::new(timeout),
            next_network_timeout: Mutex::new(timeout),
            delayed_party,
            ..Default::default()
        }
    }
}

type SimulatedPairwiseChannels = Arc<
    DashMap<
        (Role, Role, NetworkMsgKind),
        (
            Arc<UnboundedSender<LocalTaggedValue>>,
            Arc<Mutex<UnboundedReceiver<LocalTaggedValue>>>,
        ),
    >,
>;

#[async_trait]
impl Networking for LocalNetworking {
    // Keep legacy send() working: it sends on kind=Other
    async fn send(&self, val: Arc<Vec<u8>>, receiver: &Role) -> anyhow::Result<()> {
        self.send_kind(val, receiver, NetworkMsgKind::Other).await
    }

    async fn receive(&self, sender: &Role) -> anyhow::Result<Vec<u8>> {
        self.receive_kind(sender, NetworkMsgKind::Other).await
    }

    async fn send_kind(
        &self,
        val: Arc<Vec<u8>>,
        receiver: &Role,
        kind: NetworkMsgKind,
    ) -> anyhow::Result<()> {
        let (tx, _) = self
            .pairwise_channels
            .get(&(self.owner, *receiver, kind))
            .ok_or_else(|| {
                anyhow_error_and_log(format!(
                    "LocalNetworking: missing channel owner={:?} receiver={:?} kind={:?}",
                    self.owner, receiver, kind
                ))
            })?
            .value()
            .clone();

        let net_round = *self.network_round.lock().await;

        let mut already_sent = self.already_sent.lock().await;
        if already_sent.contains(&(*receiver, net_round, kind)) {
            return Err(anyhow::anyhow!(
                "Trying to send twice to receiver={receiver:?} round={net_round} kind={kind:?}"
            ));
        }
        already_sent.insert((*receiver, net_round, kind));

        let tagged_value = LocalTaggedValue {
            send_counter: net_round,
            value: val.as_ref().clone(),
        };

        tx.send(tagged_value).map_err(|e| e.into())
    }

    async fn receive_kind(&self, sender: &Role, kind: NetworkMsgKind) -> anyhow::Result<Vec<u8>> {
        let (_, rx) = self
            .pairwise_channels
            .get(&(*sender, self.owner, kind))
            .ok_or_else(|| {
                anyhow_error_and_log(format!(
                    "LocalNetworking: missing channel sender={:?} owner={:?} kind={:?}",
                    sender, self.owner, kind
                ))
            })?
            .value()
            .clone();

        let mut rx = rx.lock().await;

        let mut tagged_value = rx.recv().await.ok_or_else(|| {
            anyhow_error_and_log("LocalNetworking: receive from closed channel")
        })?;

        let network_round: usize = *self.network_round.lock().await;

        while tagged_value.send_counter < network_round {
            tracing::debug!(
                "@ round {} - dropped value {:?} from round {} (kind={:?})",
                network_round,
                tagged_value.value[..min(tagged_value.value.len(), 16)].to_vec(),
                tagged_value.send_counter,
                kind
            );
            tagged_value = rx.recv().await.ok_or_else(|| {
                anyhow_error_and_log("LocalNetworking: receive from closed channel")
            })?;
        }

        Ok(tagged_value.value)
    }

    async fn increase_round_counter(&self) {
        if let Some(duration) = self.delayed_party {
            std::thread::sleep(duration);
        }

        let (mut max_elapsed_time, mut current_round_timeout, next_round_timeout, mut net_round) =
            join4(
                self.max_elapsed_time.lock(),
                self.current_network_timeout.lock(),
                self.next_network_timeout.lock(),
                self.network_round.lock(),
            )
            .await;

        *max_elapsed_time += *current_round_timeout;
        *current_round_timeout = *next_round_timeout;
        *net_round += 1;

        tracing::debug!(
            "changed network round to: {:?} on party: {:?}, with timeout: {:?}",
            *net_round,
            self.owner,
            *current_round_timeout
        )
    }

    async fn get_timeout_current_round(&self) -> Instant {
        let init_time = self.init_time.get_or_init(Instant::now);

        let (max_elapsed_time, network_timeout) =
            join(self.max_elapsed_time.lock(), self.current_network_timeout.lock()).await;

        *init_time + *network_timeout + *max_elapsed_time
    }

    async fn get_current_round(&self) -> usize {
        *self.network_round.lock().await
    }

    async fn set_timeout_for_next_round(&self, timeout: Duration) {
        match self.get_network_mode() {
            NetworkMode::Sync => {
                let mut next_network_timeout = self.next_network_timeout.lock().await;
                *next_network_timeout = timeout;
            }
            NetworkMode::Async => {
                tracing::warn!("LocalNetworking: set_timeout_for_next_round ignored in Async mode");
            }
        }
    }

    async fn set_timeout_for_bk(&self) {
        self.set_timeout_for_next_round(*NETWORK_TIMEOUT_BK).await
    }

    async fn set_timeout_for_bk_sns(&self) {
        self.set_timeout_for_next_round(*NETWORK_TIMEOUT_BK_SNS).await
    }

    fn get_network_mode(&self) -> NetworkMode {
        self.network_mode
    }

    #[cfg(feature = "choreographer")]
    async fn get_num_byte_sent(&self) -> usize {
        0
    }

    #[cfg(feature = "choreographer")]
    async fn get_num_byte_received(&self) -> anyhow::Result<usize> {
        Ok(0)
    }
}

#[derive(Debug, Clone)]
struct LocalTaggedValue {
    value: Vec<u8>,
    send_counter: usize,
}
