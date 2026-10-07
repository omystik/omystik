use anyhow::{Context, Result};
use futures::prelude::*;
use libp2p::{Multiaddr, StreamProtocol};
use std::{pin::Pin, sync::Arc};
use tokio::sync::Mutex;

use libp2p_common::kryphos_pylon::{self, Client, Event, EventLoop};

use kms_api::kms_service::v1::core_service_endpoint_server::CoreServiceEndpoint;

// These are your libp2p (non-gRPC) job executor + blob blob_resolver.
use kms_lib::libp2p::jobexecutor::{FsBlobResolver, FsPublicArtifactResolver, ThresholdKmsJobExecutor};
use mesh_gateway_wire::{FaceBRequest, FaceBResponse};
use kms_threshold::networking::p2p::crypto_gateway_service::CryptoJobExecutor;

use crate::KryphosConfig;
use std::path::PathBuf;

pub struct KryphosPylonManager {
    pub client: Client,
    pub event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
    pub event_loop: Arc<Mutex<EventLoop>>,
    pub swarm_task: tokio::task::JoinHandle<()>,
}

impl KryphosPylonManager {
    /// Start the Mesh B FaceB crypto gateway handling using an *in-process* KMS instance.
    ///
    /// IMPORTANT: This must reuse the already-constructed KMS from node_kryphos.
    pub async fn new(
        path_cfg: PathBuf,
        node_id: u32,
        kms: Arc<dyn CoreServiceEndpoint + Send + Sync>,
    ) -> Result<Self> {
        // Do NOT initialize tracing here (node_kryphos already does it).

        let cfg = KryphosConfig::load(path_cfg).await?;

        let node = cfg.self_node(node_id)?.clone();

        // Protocols
        let mpc_proto_static: &'static str =
            Box::leak(node.mpc_protocol.clone().into_boxed_str());
        let gateway_proto_static: &'static str =
            Box::leak(node.gateway_protocol.clone().into_boxed_str());

        let mpc_protocol = StreamProtocol::new(mpc_proto_static);
        let gateway_protocol = StreamProtocol::new(gateway_proto_static);


        // 1) Build libp2p common node (transport + protocol)
        let (mut client, event_stream, composite_event_loop) =
            kryphos_pylon::new(Some(node.secret_seed), mpc_protocol, gateway_protocol)
                .await
                .context("libp2p_common::kg::new")?;

        let event_loop = Arc::new(Mutex::new(composite_event_loop));

        // 2) Spawn swarm loop (project-standard pattern)
        let swarm_task = {
            let event_loop_clone = event_loop.clone();
            tokio::spawn(async move {
                event_loop_clone.lock().await.run().await;
            })
        };

        // 3) Listen (all configured addrs; fallback if none)
        match node.gateway_listen_addr.clone() {
            Some(addr) => {
                client.start_listening(addr).await.context("start_listening")?;
            }
            None => {
                client
                    .start_listening("/ip4/0.0.0.0/udp/0/quic-v1".parse::<Multiaddr>()?)
                    .await
                    .context("start_listening default")?;
            }
        }

        // 4) Build executor using a Sized KMS wrapper
        // - 6 March -
        let blob_resolver = Arc::new(FsBlobResolver::new(node.gateway_blob.clone()));
        let artifact_resolver = Arc::new(FsPublicArtifactResolver::new(node.mpc_public_storage.clone()));
        let exec = Arc::new(ThresholdKmsJobExecutor::new(kms,
            blob_resolver,
             artifact_resolver,
            ));
        // - -

        // 5) Mandatory handler: FaceBRequest -> FaceBResponse
        let handler: Arc<
            dyn Fn(libp2p::PeerId, FaceBRequest)
                -> futures::future::BoxFuture<'static, Result<FaceBResponse, anyhow::Error>>
            + Send
            + Sync
        > = Arc::new(move |peer, req: FaceBRequest| {
            let exec = exec.clone();
            Box::pin(async move {
                match exec.execute(peer, req).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => Ok(FaceBResponse::Error {
                        message: format!("executor failed: {e}"),
                    }),
                }
            })
        });

        client
            .set_compute_enc_handler(handler)
            .await
            .context("set_compute_enc_handler")?;

        Ok(Self {
            client,
            event_stream: Box::pin(event_stream),
            event_loop,
            swarm_task,
        })
    }
}
