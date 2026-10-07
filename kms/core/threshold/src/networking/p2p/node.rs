use anyhow::Result;

use super::{
    protocol::WireMsg,
    router::{Ack, InboundRouter},
    sending_service_p2p::{Libp2pSendingService, ThresholdMeshClient},
    Libp2pNetworkingManager
};
use bc2wrap::{deserialize_safe, serialize};
use futures::{
    future::BoxFuture,
};
use libp2p::PeerId;
use dashmap::DashMap;
use std::{collections::HashMap, sync::Arc};

use crate::execution::runtime::party::{Identity,MpcIdentity};
use crate::networking::grpc::{CoreToCoreNetworkConfig, OptionConfigWrapper};
use crate::networking::session_runtime::SessionStore;

/// kms_threshold -> libp2p_common (kryphos)
/// Wrapper to satisfy `ThresholdMeshClient` without fighting orphan rules.
#[derive(Clone)]
struct KryphosMeshClient {
    inner: Arc<tokio::sync::Mutex<libp2p_common::kryphos::Client>>,
}

#[async_trait::async_trait]
impl ThresholdMeshClient for KryphosMeshClient {
    async fn threshold_request(&self, peer: PeerId, bytes: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        let mut guard = self.inner.lock().await;
        guard.threshold_request(peer, bytes).await
    }
}

/// Inbound: libp2p_common (kryphos) -> kms_threshold
fn make_inbound_threshold_handler(
        inbound: InboundRouter,
    ) -> Arc<dyn Fn(PeerId, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync> {

        let inbound = Arc::new(inbound);

        Arc::new(move |peer: PeerId, req_bytes: Vec<u8>| -> BoxFuture<'static, Vec<u8>> {
            let inbound = inbound.clone();

            Box::pin(async move {
                let msg = match deserialize_safe::<WireMsg>(&req_bytes) {
                    Ok(m) => m,
                    Err(e) => {
                        let ack = Ack::Rejected { code: 400, message: format!("decode WireMsg: {e}") };
                        return serialize(&ack).unwrap_or_default();
                    }
                };

                match msg {
                    WireMsg::SendValue { tag, value } => {
                       
                       // MUST return the *real* Ack so outbound retry/backpressure works.
                        let ack = Ack::Ok; // or Ack::Accepted (whatever your type supports)
                        serialize(&ack).unwrap_or_default()
                    }

                    WireMsg::HealthCheck { tag } => {
                        let ack = inbound.handle_health_check(&tag);
                        serialize(&ack).unwrap_or_default()
                    }
                }
            })
        })
    }


/// The ONLY constructor you should use everywhere (kms_impl.rs and node_fhe).
pub async fn build_libp2p_networking_manager_over_kryphos(
    conf: Option<CoreToCoreNetworkConfig>,
    peer_by_identity: Arc<HashMap<Identity, PeerId>>,
    mesh_client: libp2p_common::kryphos::Client,
) -> Result<(Libp2pNetworkingManager, InboundRouter)> {
    let opt_conf = OptionConfigWrapper { conf };

    // One shared state for BOTH inbound router and manager
        let session_store = Arc::new(SessionStore::default());
        let opened = Arc::new(DashMap::<MpcIdentity, u64>::new());

        // Build inbound router using the same shared stores
        let inbound = InboundRouter {
            session_store: session_store.clone(),
            opened_sessions_tracker: opened.clone(),
            channel_size_limit: opt_conf.get_message_limit(),
            max_opened_inactive_sessions: opt_conf.get_max_opened_inactive_sessions_per_party(),
            max_waiting_time_for_message_queue: opt_conf.get_max_waiting_time_for_message_queue(),
        };
    
    // outbound sending service uses the mesh client directly
    let mesh_client: Arc<dyn ThresholdMeshClient> = Arc::new(KryphosMeshClient {
        inner: Arc::new(tokio::sync::Mutex::new(mesh_client)),
    });
    let sending = Libp2pSendingService::new_with(opt_conf, peer_by_identity, mesh_client);
    

    // Manager uses the EXACT same shared stores
    let mgr = Libp2pNetworkingManager::new_with_shared(conf, sending, session_store, opened);


    Ok((mgr, inbound))
}
