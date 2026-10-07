use anyhow::Result;
use libp2p::{Multiaddr, PeerId};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

use crate::types::PylonPeer;

#[derive(Clone, Default)]
pub struct PylonDirectory {
    inner: Arc<RwLock<HashMap<PeerId, PylonPeer>>>,
}

impl PylonDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn upsert(&self, peer_id: PeerId, addr: Multiaddr) {
        let mut g = self.inner.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        match g.get_mut(&peer_id) {
            Some(existing) => {
                if !existing.addrs.iter().any(|a| a == &addr) {
                    existing.addrs.push(addr);
                }
                existing.last_seen_unix_ms = now;
            }
            None => {
                g.insert(peer_id, PylonPeer {
                    peer_id,
                    addrs: vec![addr],
                    last_seen_unix_ms: now,
                });
            }
        }
    }

    pub async fn list(&self) -> Vec<PylonPeer> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn choose_one(&self) -> Option<PylonPeer> {
        let mut peers = self.list().await;
        peers.sort_by_key(|p| std::cmp::Reverse(p.last_seen_unix_ms));
        peers.into_iter().next()
    }

    pub async fn mark_seen(&self, peer_id: PeerId) {
        let mut g = self.inner.write().await;
        if let Some(p) = g.get_mut(&peer_id) {
            p.last_seen_unix_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
        }
    }
}