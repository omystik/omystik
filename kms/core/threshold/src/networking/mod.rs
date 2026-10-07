//! Networking traits and implementations.
use crate::execution::runtime::party::Role;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

pub mod constants;
pub mod grpc;
pub mod health_check;
pub mod local;
pub mod sending_service;
pub mod tls;
pub mod value;

// NEW ØMYSTIK
pub mod manager;
pub mod p2p;
pub mod session_runtime;
use crate::networking::p2p::NetworkMsgKind;

mod gen {
    #![allow(clippy::derive_partial_eq_without_eq)]
    tonic::include_proto!("ddec_networking");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetworkMode {
    Sync,
    Async,
}

/// Requirements for networking interface.
#[async_trait]
pub trait Networking: Send + Sync + 'static {
    async fn send(&self, value: Arc<Vec<u8>>, receiver: &Role) -> anyhow::Result<()>;

    /// NEW ØMYSTIK : receive from a demuxed queue keyed by message kind.
    /// Default implementation preserves backward compatibility:
    /// implementations that don't demux will behave as "Other".
    async fn send_kind(
        &self,
        value: Arc<Vec<u8>>,
        receiver: &Role,
        kind: NetworkMsgKind,
    ) -> anyhow::Result<()>;

    async fn receive(&self, sender: &Role) -> anyhow::Result<Vec<u8>>;

    /// NEW ØMYSTIK : receive from a demuxed queue keyed by message kind.
    /// Default implementation preserves backward compatibility:
    /// implementations that don't demux will behave as "Other".
    async fn receive_kind(
        &self,
        sender: &Role,
        kind: NetworkMsgKind,
    ) -> anyhow::Result<Vec<u8>>;

    /// Increase the round counter
    ///
    /// __NOTE__: We always assume this is called right before sending happens
    async fn increase_round_counter(&self);

    ///Used to compute the timeout in network functions
    async fn get_timeout_current_round(&self) -> Instant;

    async fn get_current_round(&self) -> usize;

    #[cfg(feature = "choreographer")]
    async fn get_num_byte_sent(&self) -> usize;

    #[cfg(feature = "choreographer")]
    async fn get_num_byte_received(&self) -> anyhow::Result<usize>;

    /// Method to set a different timeout than the one set at construction, effective for the next round.
    ///
    /// __NOTE__: If the network mode is Async, this has no effect
    async fn set_timeout_for_next_round(&self, timeout: Duration);

    /// Method to set the timeout for distributed generation of the TFHE bootstrapping key
    ///
    /// Useful mostly to use parameters given by config file in grpc networking
    /// Rely on [`Networking::set_timeout_for_next_round`]
    async fn set_timeout_for_bk(&self);

    /// Method to set the timeout for distributed generation of the TFHE switch and squash bootstrapping key
    ///
    /// Useful mostly to use parameters given by config file in grpc networking
    /// Rely on [`Networking::set_timeout_for_next_round`]
    async fn set_timeout_for_bk_sns(&self);

    fn get_network_mode(&self) -> NetworkMode;
}
