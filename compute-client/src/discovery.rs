use anyhow::{anyhow, Result};
use futures::StreamExt;
use libp2p::PeerId;
use libp2p_common::komvos::{Client as FaceAClient, Event};
use mesh_gateway_wire::{FaceARequest, FaceAResponse, NodeType};
use tokio::time::{timeout, Duration};

pub async fn discover_available_ypolo(
    net_client: &mut FaceAClient,
    event_stream: &mut (impl futures::Stream<Item = Event> + Unpin),
    bootstrap_addr: libp2p::Multiaddr,
    bootstrap_peer: PeerId,
    namespace: &'static str,
    timeout_duration: Duration,
) -> Result<PeerId> {
    net_client
        .register(bootstrap_peer, bootstrap_addr, namespace)
        .await?;

    let found = timeout(timeout_duration, async {
        loop {
            let Some(event) = event_stream.next().await else {
                return Err(anyhow!("event stream closed during discovery"));
            };

            match event {
                Event::PeerIdentified { peer, .. } => {
                    let resp = net_client
                        .jobs_request(peer, FaceARequest::GetCapabilities)
                        .await;

                    let Ok(resp) = resp else {
                        continue;
                    };

                    if let FaceAResponse::Capabilities { info } = resp {
                        let is_ypolo = info.roles.iter().any(|r| matches!(r, NodeType::Ypolo));
                        if is_ypolo {
                            return Ok(peer);
                        }
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .map_err(|_| anyhow!("timed out discovering a node_ypolo"))??;

    Ok(found)
}