pub mod face_a_blobs;
pub mod face_a_blobs_client;
pub mod face_a_jobs;
pub mod face_b;
pub mod kryphos_config_copy;
pub mod runtime;

use futures::channel::mpsc::channel;
use anyhow::{anyhow, Context, Result};
use futures::{prelude::*, StreamExt};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId, StreamProtocol};
use meshes_config::gateway_config::{FACE_A_BLOBS_PROTOCOL, GatewayConfig};
use mesh_gateway_wire::{BlobId, JobStore};
use std::{collections::HashSet, pin::Pin, sync::Arc};
use tokio::{sync::Mutex, time::Duration};
use tracing_subscriber::EnvFilter;

use crate::runtime::GatewayRuntime;

pub struct GatewayManager {
    // Face A (gateway-side networking)
    pub client_face_a_jobs: face_a_jobs::Client,
    pub event_stream_face_a_jobs: Pin<Box<dyn Stream<Item = face_a_jobs::Event> + Send>>,
    pub event_loop_face_a_jobs: Arc<Mutex<face_a_jobs::EventLoop>>,

    // Persistence/runtime
    pub store: JobStore,
    pub runtime: GatewayRuntime,

    // Optional Face B client (gateway -> kryphos_gateway)
    pub face_b: Option<face_b::FaceBManager>,

    // Discovered Mesh-A peers (exclude rendezvous nodes)
    pub mesh_a_peers: Arc<Mutex<HashSet<PeerId>>>,
}

impl GatewayManager {
    pub async fn new(cfg: GatewayConfig) -> Result<Self> {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        // Discovered peers set for blob push
        let mesh_a_peers: Arc<Mutex<HashSet<PeerId>>> = Arc::new(Mutex::new(HashSet::new()));

        // Rendezvous peer set (excluded from storage peers)
        let mut rv_peers: HashSet<PeerId> = HashSet::new();

        // Mesh A namespace (static)
        let namespace_owned = cfg.face_a.namespace.clone();
        let namespace_static: &'static str = Box::leak(namespace_owned.into_boxed_str());

        // ---- persistence ----
        let store = JobStore::open(cfg.face_a.data_dir.clone())
            .await
            .context("open JobStore")?;

        // ---- runtime ----
        let mut runtime = GatewayRuntime::new(store.clone());

        // ---- Face B ----
        let face_b_mgr = face_b::FaceBManager::new(
            cfg.face_b.secret_seed,
            cfg.face_b.listen_address.clone(),
            cfg.face_b.bootstraps.clone(),
        )
        .await
        .context("init FaceBManager")?;

        // - 6 March -
        let face_b_peers = face_b_mgr.bootstrap_peers();
        if face_b_peers.is_empty() {
            return Err(anyhow!("mesh_b.bootstraps is empty or missing /p2p/<peerId>"));
        }

        runtime = runtime.with_face_b(
            face_b_mgr.client.clone(),
            face_b_peers,
            cfg.face_b.num_majority,
            cfg.face_b.num_reconstruct,
            cfg.face_b.expect_all_responses,
        );
        // let face_b_peer = face_b_mgr
        //     .first_bootstrap_peer()
        //     .ok_or_else(|| anyhow!("mesh_b.bootstraps is empty or missing /p2p/<peerId>"))?;

        // runtime = runtime.with_face_b(face_b_mgr.client.clone(), face_b_peer);
        // - -
        // ---- Face A swarm (jobs + blob streams) ----
        let (mut client_face_a_jobs, event_stream_face_a_jobs, event_loop_face_a_jobs) =
            face_a_jobs::new(cfg.face_a.secret_seed)
                .await
                .context("face_a_jobs::new")?;

        // Wrap event loop so we can share it (needed by pusher)
        let event_loop_face_a_jobs = Arc::new(Mutex::new(event_loop_face_a_jobs));

        // ---- Build pusher NOW (we have mesh_a_peers + event_loop_face_a_jobs) ----
        {
            let mesh_a_peers_for_pusher = mesh_a_peers.clone();
            let event_loop_for_pusher = event_loop_face_a_jobs.clone();

            let pusher = Arc::new(
                move |blob_id: BlobId, bytes: Vec<u8>, k: usize| -> futures::future::BoxFuture<'static, Result<usize>> {
                    let mesh_a_peers2 = mesh_a_peers_for_pusher.clone();
                    let event_loop2 = event_loop_for_pusher.clone();

                    Box::pin(async move {
                        let peers: Vec<PeerId> = mesh_a_peers2.lock().await.iter().cloned().collect();
                        if peers.is_empty() {
                            tracing::warn!("no mesh-a peers discovered yet; keeping blob only on gateway");
                            return Ok(0);
                        }

                        let control = {
                            let mut guard = event_loop2.lock().await;
                            guard.swarm_mut().behaviour().stream.new_control()
                        };

                        let proto = StreamProtocol::new(FACE_A_BLOBS_PROTOCOL);

                        let mut ok = 0usize;
                        for p in peers {
                            if crate::face_a_blobs_client::put_blob(
                                control.clone(),
                                p,
                                proto.clone(),
                                blob_id,
                                &bytes,
                            )
                            .await
                            .is_ok()
                            {
                                ok += 1;
                                if ok >= k {
                                    break;
                                }
                            }
                        }

                        if ok == 0 {
                            return Err(anyhow!("failed to push blob to any mesh-a peer"));
                        }
                        Ok(ok)
                    })
                },
            );

            // inject into runtime BEFORE set_jobs_handler
            runtime = runtime.with_blob_pusher(pusher);
        }

        // Attach Face A jobs handler AFTER runtime is fully wired
        client_face_a_jobs
            .set_jobs_handler(Some(runtime.clone().make_face_a_handler()))
            .await
            .context("set_jobs_handler")?;

        // Extract shared listen address handle
        let listen_addr_handle = {
            let guard = event_loop_face_a_jobs.lock().await;
            guard.actual_listen_addr.clone()
        };

        let (ev_out_tx, ev_out_rx) = channel::<face_a_jobs::Event>(64);
        let (pin_act_tx, _pin_act_rx) = channel::<crate::face_a_blobs::PinAck>(64);
        
        let local_peer_id: PeerId = {
            let guard = event_loop_face_a_jobs.lock().await;
            guard.local_peer_id()
        };

        // ---- Blob stream accept loop ----
        let blob_protocol = StreamProtocol::new(FACE_A_BLOBS_PROTOCOL);

        {
            let mut guard = event_loop_face_a_jobs.lock().await;

            let mut incoming = guard
                .swarm_mut()
                .behaviour()
                .stream
                .new_control()
                .accept(blob_protocol)
                .context("accept FaceA blob_protocol")?;

            let store_clone = store.clone();
            let pin_act_tx2 = pin_act_tx.clone();
            tokio::spawn(async move {
                while let Some((_peer, stream)) = incoming.next().await {
                    let store2 = store_clone.clone();
                    let pin_act_tx3 = pin_act_tx2.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::face_a_blobs::handle_inbound_blob_stream(store2, stream, pin_act_tx3).await {
                            tracing::warn!("blob handler failed: {e}");
                        }
                    });
                }
            });
        }

        // Spawn Face A event loop
        {
            let event_loop_clone = event_loop_face_a_jobs.clone();
            tokio::spawn(async move {
                event_loop_clone.lock().await.run().await;
            });
        }

        // Start listening (Face A)
        client_face_a_jobs
            .start_listening(cfg.face_a.listen_address)
            .await
            .context("Face A start_listening")?;

        // Wait until swarm has actual bound address
        let addr_to_advertise: Multiaddr = loop {
            if let Some(addr) = listen_addr_handle.lock().await.clone() {
                break addr;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };

        // Advertise on the DHT
        let addr_to_advertise = Self::strip_p2p(addr_to_advertise);
        client_face_a_jobs
            .advertise(local_peer_id, addr_to_advertise)
            .await?;

        // Register to rendezvous nodes
        for rv_a in &cfg.face_a.rendezvous {
            client_face_a_jobs
                .register(rv_a.peer_id, rv_a.addr.clone(), namespace_static)
                .await
                .context("Face A register rendezvous")?;

            rv_peers.insert(rv_a.peer_id);
        }

        // ---- Event forwarding + discovery side-effects ----
        let mut evs = event_stream_face_a_jobs;
        let mesh_a_peers2 = mesh_a_peers.clone();
        let rv_peers_clone = rv_peers.clone();
        let mut ev_out_tx = ev_out_tx;

        tokio::spawn(async move {
            while let Some(ev) = evs.next().await {
                // Only act on discovered peers
                let face_a_jobs::Event::Discovered { peer } = &ev; 
                if !rv_peers_clone.contains(peer) {
                    mesh_a_peers2.lock().await.insert(*peer);
                }

                // Forward to public stream
                if ev_out_tx.send(ev).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            client_face_a_jobs,
            event_stream_face_a_jobs: Box::pin(ev_out_rx),
            event_loop_face_a_jobs,
            store,
            runtime,
            face_b: Some(face_b_mgr),
            mesh_a_peers,
        })
    }

    pub fn strip_p2p(mut addr: Multiaddr) -> Multiaddr {
        while matches!(addr.iter().last(), Some(Protocol::P2p(_))) {
            let _ = addr.pop();
        }
        addr
    }

    pub async fn push_blob_to_mesh_a(&self, blob_id: BlobId, bytes: &[u8], k: usize) -> Result<usize> {

        let peers: Vec<PeerId> = self.mesh_a_peers.lock().await.iter().cloned().collect();
        if peers.is_empty() {
            tracing::warn!("no mesh-a peers discovered yet; skipping proactive blob push");
            return Ok(0);
        }

        // get stream control from the Face A swarm
        let control = {
            let mut guard = self.event_loop_face_a_jobs.lock().await;
            guard.swarm_mut().behaviour().stream.new_control()
        };

        let proto = StreamProtocol::new(FACE_A_BLOBS_PROTOCOL);
        let mut ok = 0usize;

        for p in peers {
            if crate::face_a_blobs_client::put_blob(control.clone(), p, proto.clone(), blob_id, bytes).await.is_ok() {
                ok += 1;
                if ok >= k { break; }
            }
        }

        if ok == 0 {
            return Err(anyhow!("failed to push blob to any mesh-a peer"));
        }
        Ok(ok)
    }
}

