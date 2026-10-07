use std::{fs,
    pin::Pin,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};
use futures::prelude::*;
use anyhow::{Context, Result, anyhow};
use libp2p::{PeerId, core::Multiaddr, StreamProtocol};
use tokio::{sync::mpsc, time::Duration};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use libp2p_common::kryphos::{self, Client, Event, EventLoop};

use futures::future::BoxFuture;
use kms_threshold::networking::p2p::protocol::WireMsg;
use kms_threshold::networking::p2p::router::{InboundRouter, Ack};
use kms_threshold::networking::session_runtime::Tag;
use kms_threshold::execution::runtime::party::MpcIdentity;

use dashmap::DashMap;
use kms_threshold::execution::runtime::party::Identity;

use crate::KryphosConfig;


pub struct KryphosManager {
    // ---- libp2p mesh layer ----
    pub client: Client,
    pub event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
    pub event_loop: Arc<Mutex<EventLoop>>,
    pub listen_address: Option<Multiaddr>,
    pub peer: Option<Multiaddr>,
    pub peer_by_identity: Arc<HashMap<Identity, PeerId>>,
    pub peer_to_mpc: Arc<HashMap<PeerId, MpcIdentity>>,
    pub required_peers: Vec<PeerId>,
    pub swarm_task: Option<tokio::task::JoinHandle<()>>,
    pub bootstrap_task: Option<JoinHandle<()>>,
    pub shutdown_token: CancellationToken, 
}


impl KryphosManager {
    
    pub async fn new(
        path_cfg: PathBuf, node_id: u32,
    ) -> Result<Self> {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();

        let shutdown_token = CancellationToken::new();
        let swarm_token = shutdown_token.clone();
        let bootstrap_token = shutdown_token.clone();

        let mut cfg = KryphosConfig::load(path_cfg).await?;

        // Build topology map Identity -> PeerId for all nodes
        cfg.build_topology()?;

        
        let peer_by_identity = Arc::clone(&cfg.peer_by_identity);
        let peer_to_mpc = Arc::clone(&cfg.peer_to_mpc);

        let node = cfg.self_node(node_id)?.clone();
        
        let listen_address: Option<Multiaddr> = node.listen_address;

        // For simplicity we use None for bootstrap/peer here.
        let peer: Option<Multiaddr> = None;

        // setting protocols (mpc, gateway)
        let mpc_proto_static: &'static str =
            Box::leak(node.mpc_protocol.clone().into_boxed_str());
        let gateway_proto_static: &'static str =
            Box::leak(node.gateway_protocol.clone().into_boxed_str());

        let mpc_protocol = StreamProtocol::new(mpc_proto_static);
        let gateway_protocol = StreamProtocol::new(gateway_proto_static);


        let (mut client, event_stream, composite_event_loop) =
            kryphos::new(Some(node.secret_seed), mpc_protocol, gateway_protocol).await?;

        let listen_addr_handle = composite_event_loop.actual_listen_addr.clone();
        let event_loop = Arc::new(Mutex::new(composite_event_loop));

        // run swarm
        let local_peer_id: PeerId = {
            let guard = event_loop.lock().await;
            guard.local_peer_id()
        };

        // bootstrap connection sanity check
        let connected: Arc<DashMap<PeerId, u32>> = {
            let guard = event_loop.lock().await;
            guard.connected_handle()
        };

        // spawn swarm loop
        let swarm_task: JoinHandle<()> = {
            let event_loop_clone = event_loop.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = swarm_token.cancelled() => {
                        //tokio::signal::ctrl_c().await
                    }
                    _ = async {
                         event_loop_clone.lock().await.run().await;
                    } => {}
                }
            })
        };

        // listen
        match &listen_address {
            Some(addr) => client.start_listening(addr.clone()).await?,
            None => client.start_listening("/ip4/0.0.0.0/udp/0/quic-v1".parse()?).await?, // 0.0.0.0
        }

        let non_self_bootstraps: Vec<(Multiaddr, PeerId)> = cfg
            .bootstraps
            .iter()
            .cloned()
            .filter(|(_addr, pid)| *pid != local_peer_id)
            .collect();


        let required_peers: Vec<PeerId> = cfg.bootstraps
            .iter()
            .map(|(_addr, pid)| *pid)
            .filter(|pid| *pid != local_peer_id)
            .collect();

        // bootstrap ensure-mesh loop
        let bootstrap_task: JoinHandle<()> = {
            // mandatory dial bootstrap       
            let bootstrap_parties: Vec<(Multiaddr, PeerId)> = non_self_bootstraps.clone();
            let mut bootstrap_client = client.clone();
            let connected2 = connected.clone();

            tokio::spawn(async move {
                let mut backoff = Duration::from_secs(1);
                let max_backoff = Duration::from_secs(30);

                loop {
                        tokio::select! {
                    _ = bootstrap_token.cancelled() => {
                        break;
                    }
                    _ = async {
                        let mut any_attempted = false;

                        for (addr, peer_id) in bootstrap_parties.iter().cloned() {
                            if peer_id == local_peer_id {
                                continue;
                            }

                            // if peer_id <= local_peer_id {
                            //     continue;
                            // }

                            if connected2.contains_key(&peer_id) {
                                continue;
                            }

                            any_attempted = true;

                            if let Err(e) = bootstrap_client.dial(peer_id, addr.clone()).await {
                                tracing::info!("ensure-mesh dial failed peer={peer_id} addr={addr}: {e}");
                            }
                        }

                        if !any_attempted {
                            tokio::time::sleep(Duration::from_secs(30)).await;
                            return;
                        }

                        tokio::time::sleep(backoff).await;
                        backoff = std::cmp::min(backoff * 2, max_backoff);
                    } => {}

                    }
                }
            })
        };

        //client.parties_mesh_ready(required_peers.clone()).await?;
        

        // tracing::info!("Mesh is READY (all required peers connected).");

        // TEST TO DEL
        
        // client.set_threshold_handler(Some(Arc::new(|peer: PeerId, data| -> BoxFuture<'static, Vec<u8>>{
        //     Box::pin(async move {
        //         println!("got threshold req from {peer}: {} bytes" , data.len());
        //     b"pong".to_vec()
        //     })
        // }))).await?;
                
        // for p in required_peers.clone() {
        //     let _ = client.threshold_request(p, b"hello".to_vec()).await?;
        // }
        
        // ---***---

        // advertise real bound addr if needed
        let addr_to_advertise = match &listen_address {
            Some(user_addr) => user_addr.clone(),
            None => loop {
                if let Some(addr) = listen_addr_handle.lock().await.clone() {
                    break addr;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        };

        client.advertise(local_peer_id, addr_to_advertise).await?;

        Ok(Self {
            client,
            event_stream: Box::pin(event_stream),
            event_loop,
            listen_address,
            peer,
            peer_by_identity,
            peer_to_mpc,
            required_peers,
            swarm_task: Some(swarm_task),
            bootstrap_task: Some(bootstrap_task),
            shutdown_token,
        })
    }

    pub async fn wait_mpc_mesh_ready(&mut self) -> anyhow::Result<()> {
        let required = self.required_peers.clone();
        self.client.parties_mesh_ready(required).await?;
        Ok(())
    }
    
    pub async fn install_threshold_handler(
        &mut self,
        router: Arc<InboundRouter>,
        peer_to_mpc: Arc<HashMap<PeerId, MpcIdentity>>,
    ) -> anyhow::Result<()> {
        self.client
            .set_threshold_handler(Some(Arc::new(move |peer: PeerId, data: Vec<u8>| -> BoxFuture<'static, Vec<u8>> {
                let router = Arc::clone(&router);
                let peer_to_mpc = Arc::clone(&peer_to_mpc);

                Box::pin(async move {
                    // 1) Resolve peer -> MPC identity (transport auth)
                    let expected_sender = match peer_to_mpc.get(&peer) {
                        Some(s) => s.clone(),
                        None => {
                            let ack = Ack::Rejected {
                                code: 401,
                                message: format!("unknown peer {peer} (no PeerId->MpcIdentity mapping)"),
                            };
                            return bc2wrap::serialize(&ack).unwrap_or_default();
                        }
                    };

                    // 2) Decode incoming WireMsg
                    let msg = match bc2wrap::deserialize_safe::<WireMsg>(&data) {
                        Ok(m) => m,
                        Err(e) => {
                            let ack = Ack::Rejected { code: 400, message: format!("bad WireMsg: {e}") };
                            return bc2wrap::serialize(&ack).unwrap_or_default();
                        }
                    };

                    // 3) Route
                    let ack: Ack = match msg {
                        WireMsg::SendValue { tag, value } => {
                            // Verify Tag.sender == expected sender
                            match bc2wrap::deserialize_safe::<Tag>(&tag) {
                                Ok(t) => {
                                    if t.sender() != &expected_sender {
                                        Ack::Rejected {
                                            code: 401,
                                            message: format!(
                                                "sender mismatch: tag.sender={} but transport={}",
                                                t.sender(), expected_sender
                                            ),
                                        }
                                    } else {
                                        router.handle_send_value(&tag, value).await
                                    }
                                }
                                Err(e) => Ack::Rejected { code: 400, message: format!("bad Tag: {e}") },
                            }
                        }
                        WireMsg::HealthCheck { tag } => router.handle_health_check(&tag),
                    };

                    // 4) Encode Ack as bytes
                    bc2wrap::serialize(&ack).unwrap_or_default()
                })
            })))
            .await?;

        Ok(())
    }

    pub fn stop_swarm(&mut self) {
        // signal everyone to stop
        self.shutdown_token.cancel();

        // backstop: abort tasks
        if let Some(handle) = self.bootstrap_task.take() {
            handle.abort();
        }
        if let Some(handle) = self.swarm_task.take() {
            handle.abort();
        }

        // optional: if your Client has a close/shutdown API, call it here
        // e.g. self.client.close().await; (if available)
    }

    pub async fn shutdown(&mut self) {
        self.shutdown_token.cancel();

        if let Some(handle) = self.bootstrap_task.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.swarm_task.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for KryphosManager {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
        if let Some(h) = self.bootstrap_task.take() { h.abort(); }
        if let Some(h) = self.swarm_task.take() { h.abort(); }
    }
}