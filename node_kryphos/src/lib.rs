pub mod node_kryphos;
pub mod pylon;
pub mod kryphos_config;

use anyhow::ensure;
use clap::Parser;
use futures_util::future::OptionFuture;
use k256::ecdsa::SigningKey;
use kms_api::rpc_types::PubDataType;
use bc2wrap::{serialize, deserialize_safe};
use kms_lib::{
    conf::{
        init_conf, init_conf_kms_core_telemetry,
        threshold::{PeerConf, ThresholdPartyConf, TlsConf},
        CoreConfig,
    },
    consts::{DEFAULT_MPC_CONTEXT, SIGNING_KEY_ID},
    cryptography::{
        attestation::{make_security_module, SecurityModuleProxy, SecurityModule},
        internal_crypto_types::PrivateSigKey,
    },
    engine::{
        centralized::central_kms::RealCentralizedKms,
        run_server,
        threshold::service::{new_real_threshold_kms, P2pInputs},
    },
    grpc::MetaStoreStatusServiceImpl,
    vault::{
        aws::build_aws_sdk_config,
        keychain::{awskms::build_aws_kms_client, make_keychain_proxy},
        storage::{
            crypto_material::get_core_signing_key,
            make_storage,
            read_text_at_request_id,
            s3::build_s3_client,
            StorageCache,
            StorageType,
        },
        Vault,
    },
};

use std::{env, collections::HashMap, net::ToSocketAddrs, sync::Arc, thread, path::PathBuf};

use kms_api::kms_service::v1::core_service_endpoint_server::CoreServiceEndpoint;

use kms_threshold::{
    execution::runtime::party::{MpcIdentity, Role, Identity},
    networking::{
        session_runtime::{HealthTag, Tag},
        p2p::{
            protocol::WireMsg,
            router::{
                Ack, InboundRouter
            }
        },
        tls::{build_ca_certs_map, AttestedVerifier}},
    thread_handles::init_rayon_thread_pool,
};

use tokio::net::TcpListener;
use tokio_rustls::rustls::{
    client::{danger::DangerousClientConfigBuilder, ClientConfig},
    crypto::aws_lc_rs::default_provider as aws_lc_rs_default_provider,
    pki_types::{CertificateDer, PrivateKeyDer, UnixTime},
    server::ServerConfig,
    version::TLS13,
};
use webpki::{anchor_from_trusted_cert, EndEntityCert, KeyUsage};
use libp2p::PeerId;
use crate::node_kryphos::KryphosManager;
use kryphos_config::KryphosConfig;
use pylon::KryphosPylonManager;

#[derive(Parser)]
#[clap(name = "node-fhe")]
pub struct Args {
    /// Path to the Kryphos mesh config TOML (your new file that contains parties[])
    #[clap(long)]
    pub kryphos_config: PathBuf,

    /// MPC/KMS party id (the same id used by default_X.toml / ThresholdPartyConf.my_id, typically 1..=N)
    #[clap(long)]
    pub node_id: u32,
}

async fn make_mpc_listener(threshold_config: &ThresholdPartyConf) -> TcpListener {
    let mpc_socket_addr_str = format!(
        "{}:{}",
        threshold_config.listen_address, threshold_config.listen_port
    );
    let mpc_socket_addr = mpc_socket_addr_str
        .to_socket_addrs()
        .unwrap_or_else(|e| {
            panic!(
                "Wrong MPC IP Address: {} \n {:?}",
                threshold_config.listen_address, e
            )
        })
        .next()
        .unwrap_or_else(|| {
            panic!(
                "Failed to parse MPC IP Address: {}",
                threshold_config.listen_address
            )
        });

    TcpListener::bind(mpc_socket_addr)
        .await
        .unwrap_or_else(|e| panic!("Could not bind to {mpc_socket_addr} \n {e:?}"))
}

/// Communication between MPC parties can be optionally protected with mTLS.
/// This is unchanged vs kms-server.rs.
async fn build_tls_config(
    my_id: usize,
    peers: &[PeerConf],
    tls_config: &TlsConf,
    security_module: Option<Arc<SecurityModuleProxy>>,
    public_vault: &Vault,
    sk: &PrivateSigKey,
    #[cfg(feature = "insecure")] mock_enclave: bool,
) -> anyhow::Result<(ServerConfig, ClientConfig)> {
    let context_id = *DEFAULT_MPC_CONTEXT;
    aws_lc_rs_default_provider()
        .install_default()
        .unwrap_or_else(|_| panic!("Failed to load default crypto provider"));

    let ca_certs_list = peers
        .iter()
        .map(|peer| {
            peer.tls_cert
                .as_ref()
                .map(|cert| cert.into_pem(peer.party_id, peers))
                .unwrap_or_else(|| panic!("No CA certificate present for peer {}", peer.party_id))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let ca_certs = build_ca_certs_map(ca_certs_list.into_iter())?;

    let (cert, key, trusted_releases, pcr8_expected) = match tls_config {
        TlsConf::Manual { ref cert, ref key } => {
            let cert = cert.into_pem(my_id, peers)?;
            let key = key.into_pem()?;
            (cert, key, None, false)
        }
        TlsConf::SemiAuto { ref cert, ref trusted_releases } => {
            let security_module = security_module.as_ref().unwrap_or_else(|| {
                panic!("EIF signing certificate present but not security module, cannot construct TLS identity")
            });
            let eif_signing_cert_pem = cert.into_pem(my_id, peers)?;
            let (cert, key) = security_module
                .wrap_x509_cert(context_id, eif_signing_cert_pem, true)
                .await?;
            (cert, key, Some(Arc::new(trusted_releases.clone())), true)
        }
        TlsConf::FullAuto { ref trusted_releases } => {
            let security_module = security_module
                .as_ref()
                .unwrap_or_else(|| panic!("TLS identity and security module not present"));

            let ca_cert_bytes = read_text_at_request_id(
                public_vault,
                &SIGNING_KEY_ID,
                &PubDataType::CACert.to_string(),
            )
            .await?;
            let ca_cert = x509_parser::pem::parse_x509_pem(ca_cert_bytes.as_bytes())?.1;

            let (cert, key) = security_module
                .issue_x509_cert(context_id, &ca_cert, sk, true)
                .await?;

            EndEntityCert::try_from(&cert.contents.as_slice().into())?
                .verify_for_usage(
                    &[webpki::aws_lc_rs::ECDSA_P256K1_SHA256],
                    &[anchor_from_trusted_cert(&ca_cert.contents.as_slice().into())?],
                    &[],
                    UnixTime::now(),
                    KeyUsage::server_auth(),
                    None,
                    None,
                )
                .unwrap_or_else(|e| {
                    panic!("TLS certificate signed by enclave CA is invalid, cannot proceed: {e}")
                });

            (cert, key, Some(Arc::new(trusted_releases.clone())), false)
        }
    };

    let cert_chain = vec![CertificateDer::from_slice(cert.contents.as_slice()).into_owned()];
    let key_der = PrivateKeyDer::try_from(key.contents.as_slice())
        .unwrap_or_else(|e| panic!("Could not read TLS private key: {e}"))
        .clone_key();

    let verifier = Arc::new(AttestedVerifier::new(
        pcr8_expected,
        #[cfg(feature = "insecure")] mock_enclave,
    )?);

    verifier.add_context(context_id.derive_session_id()?, ca_certs, trusted_releases)?;

    let server_config = ServerConfig::builder_with_protocol_versions(&[&TLS13])
        .with_client_cert_verifier(verifier.clone())
        .with_single_cert(cert_chain.clone(), key_der.clone_key())?;

    let client_config = DangerousClientConfigBuilder {
        cfg: ClientConfig::builder_with_protocol_versions(&[&TLS13]),
    }
    .with_custom_certificate_verifier(verifier)
    .with_client_auth_cert(cert_chain, key_der)?;

    Ok((server_config, client_config))
}


pub fn run(args: Args) -> anyhow::Result<()> {
    // We need CoreConfig to size the runtime; it lives inside the per-party KMS TOML path
    let mut kry_cfg = {
        let data = std::fs::read_to_string(&args.kryphos_config)?;
        let cfg: KryphosConfig = toml::from_str(&data)?;
        cfg
    };

    let kms_party_toml = kry_cfg.get_kms_party_toml_path(args.node_id)?;
    let core_config = init_conf::<CoreConfig>(kms_party_toml.to_string_lossy().as_ref())?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(core_config.internal_config.unwrap_or_default().num_tokio_threads)
        .build()?;

    rt.block_on(async move { main_exec(args).await })
}

async fn main_exec(args: Args) -> anyhow::Result<()> {
    // Load kryphos config properly (async path)
    let mut kry_cfg = KryphosConfig::load(args.kryphos_config.clone()).await?;
    let kms_party_toml = kry_cfg.get_kms_party_toml_path(args.node_id)?.clone();

    // KMS telemetry + CoreConfig from that party TOML
    let (core_config, tracer_provider, meter_provider) =
        init_conf_kms_core_telemetry::<CoreConfig>(&kms_party_toml.to_string_lossy()).await?;

    // rayon pool used inside MPC protocols
    let num_rayon_threads = init_rayon_thread_pool(
        core_config.internal_config.clone().unwrap_or_default().num_rayon_threads,
    )
    .await?;

    tracing::info!("Starting node_fhe with core config: {:?}", &core_config);
    tracing::info!(
        "Multi-threading values: tokio::num_workers: {}, rayon_num_threads: {}, total_num_cpus: {}",
        tokio::runtime::Handle::current().metrics().num_workers(),
        num_rayon_threads,
        thread::available_parallelism()?.get(),
    );

    let party_role = core_config
        .threshold
        .as_ref()
        .map(|t| Role::indexed_from_one(t.my_id));

    // common AWS configuration
    let aws_sdk_config = match core_config.aws {
        Some(ref aws_config) => Some(
            build_aws_sdk_config(
                aws_config.region.clone(),
                aws_config.imds_endpoint.clone(),
                aws_config.sts_endpoint.clone(),
            )
            .await,
        ),
        None => None,
    };

    // AWS S3 client
    let need_s3_client = core_config
        .public_vault
        .as_ref()
        .map(|v| v.storage.is_s_3())
        .unwrap_or(false)
        || core_config
            .private_vault
            .as_ref()
            .map(|v| v.storage.is_s_3())
            .unwrap_or(false)
        || core_config
            .backup_vault
            .as_ref()
            .map(|v| v.storage.is_s_3())
            .unwrap_or(false);
    let s3_client = if need_s3_client {
        Some(
            build_s3_client(
                aws_sdk_config.as_ref().expect("AWS configuration must be provided"),
                core_config.aws.as_ref().and_then(|aws| aws.s3_endpoint.clone()),
            )
            .await?,
        )
    } else {
        None
    };

    // AWS KMS client
    let need_awskms_client = core_config
        .private_vault
        .as_ref()
        .and_then(|v| v.keychain.as_ref().map(|k| k.is_aws_kms()))
        .unwrap_or(false)
        || core_config
            .backup_vault
            .as_ref()
            .and_then(|v| v.keychain.as_ref().map(|k| k.is_aws_kms()))
            .unwrap_or(false);
    let awskms_client = if need_awskms_client {
        Some(
            build_aws_kms_client(
                aws_sdk_config.as_ref().expect("AWS configuration must be provided"),
                core_config.aws.and_then(|aws| aws.awskms_endpoint),
            )
            .await,
        )
    } else {
        None
    };

    // storage cache
    let public_storage_cache = core_config
        .public_vault
        .as_ref()
        .and_then(|v| v.storage_cache_size.and_then(|s| StorageCache::new(s).ok()));
    let private_storage_cache = core_config
        .private_vault
        .as_ref()
        .and_then(|v| v.storage_cache_size.and_then(|s| StorageCache::new(s).ok()));

    // security module
    let need_security_module = need_awskms_client
        || core_config
            .threshold
            .as_ref()
            .and_then(|t| t.tls.as_ref())
            .map(|tls| tls.is_semi_auto() || tls.is_full_auto())
            .unwrap_or(false);

    let mock_enclave = cfg!(feature = "insecure");

    let security_module = need_security_module
        .then(|| make_security_module(mock_enclave))
        .transpose()
        .inspect_err(|e| tracing::warn!("Could not initialize security module: {e}"))?
        .map(Arc::new);

    // public vault
    let public_storage = make_storage(
        core_config.public_vault.map(|v| v.storage),
        StorageType::PUB,
        party_role,
        public_storage_cache,
        s3_client.clone(),
    )
    .inspect_err(|e| tracing::warn!("Could not initialize public storage: {e}"))?;
    let public_vault = Vault { storage: public_storage, keychain: None };

    // private vault
    let private_storage = make_storage(
        core_config.private_vault.as_ref().map(|v| v.storage.clone()),
        StorageType::PRIV,
        party_role,
        private_storage_cache,
        s3_client.clone(),
    )
    .inspect_err(|e| tracing::warn!("Could not initialize private storage: {e}"))?;
    let private_keychain = OptionFuture::from(
        core_config
            .private_vault
            .as_ref()
            .and_then(|v| v.keychain.as_ref())
            .map(|k| {
                make_keychain_proxy(
                    k,
                    awskms_client.clone(),
                    security_module.as_ref().map(Arc::clone),
                    Some(&public_vault.storage),
                )
            }),
    )
    .await
    .transpose()
    .inspect_err(|e| tracing::warn!("Could not initialize private keychain: {e}"))?;
    let private_vault = Vault { storage: private_storage, keychain: private_keychain };

    // signing key
    let sk = get_core_signing_key(&private_vault).await?;

    // display verifying key / address
    let pk = SigningKey::verifying_key(sk.sk());
    tracing::info!("KMS verifying key is {}", hex::encode(pk.to_encoded_point(false).to_bytes()));
    tracing::info!("Public ethereum address is {}", alloy_signer::utils::public_key_to_address(pk));

    // backup vault
    let backup_storage = core_config
        .backup_vault
        .as_ref()
        .map(|v| {
            make_storage(
                Some(v.storage.clone()),
                StorageType::BACKUP,
                party_role,
                None,
                s3_client,
            )
        })
        .transpose()
        .inspect_err(|e| tracing::warn!("Could not initialize backup storage: {e}"))?;
    let backup_keychain = OptionFuture::from(
        core_config
            .backup_vault
            .as_ref()
            .and_then(|v| v.keychain.as_ref())
            .map(|k| {
                make_keychain_proxy(
                    k,
                    awskms_client.clone(),
                    security_module.as_ref().map(Arc::clone),
                    Some(&public_vault),
                )
            }),
    )
    .await
    .transpose()
    .inspect_err(|e| tracing::warn!("Could not initialize backup keychain: {e}"))?;
    let backup_vault = backup_storage.map(|storage| Vault { storage, keychain: backup_keychain });

    // service listener (blockchain-facing service)
    let service_socket_addr_str = format!(
        "{}:{}",
        core_config.service.listen_address, core_config.service.listen_port
    );
    let service_socket_addr = service_socket_addr_str
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    let service_listener = TcpListener::bind(service_socket_addr).await?;

    let mock_enclave = cfg!(feature = "insecure");
    match core_config.threshold {
        Some(threshold_config) => {
            // MPC listener
            let mpc_listener = make_mpc_listener(&threshold_config).await;

            // TLS for MPC link (optional)
            let tls_identity = match &threshold_config.tls {
                Some(tls_config) => Some(match &threshold_config.peers {
                    Some(peers) => {
                        build_tls_config(
                            threshold_config.my_id,
                            peers,
                            tls_config,
                            security_module.clone(),
                            &public_vault,
                            &sk,
                            mock_enclave,
                        )
                        .await?
                    }
                    None => panic!("TLS enabled but peer list not provided"),
                }),
                None => {
                    tracing::warn!("No TLS identity - using plaintext communication between MPC nodes");
                    None
                }
            };

            #[cfg(not(feature = "insecure"))]
            let need_peer_tcp_proxy = need_security_module;
            #[cfg(feature = "insecure")]
            let need_peer_tcp_proxy =
                need_security_module && !core_config.mock_enclave.is_some_and(|m| m);

            // ---- CRITICAL: start mesh node here and pass into kms_impl ----
            #[cfg(feature = "p2p")]
            let (router_tx, router_rx) = tokio::sync::oneshot::channel::<Arc<InboundRouter>>();
            
            #[cfg(feature = "p2p")]
            let mut kry = KryphosManager::new(args.kryphos_config.clone(), args.node_id).await?;

            #[cfg(feature = "p2p")]
            let p2p_inputs = Some(P2pInputs {
                mesh_client: kry.client.clone(),
                peer_by_identity: Arc::clone(&kry.peer_by_identity),
                inbound_router_tx: router_tx,
            });
                        
            #[cfg(not(feature = "p2p"))]
            let p2p_inputs: Option<P2pInputs> = None;

            // #[cfg(feature = "p2p")]
            // let my_mpc_identity = {
            //     let peers = threshold_config
            //         .peers
            //         .as_ref()
            //         .expect("threshold peers missing");

            //     let me = peers
            //         .iter()
            //         .find(|p| p.party_id == args.node_id as usize)
            //         .expect("cannot find self in peers");

            //     MpcIdentity::new(
            //         me.mpc_identity
            //             .as_deref()
            //             .expect("mpc_identity missing for self"),
            //     )
            // };

            let (kms, health_service, metastore_status_service) = new_real_threshold_kms(
                threshold_config,
                public_vault,
                private_vault,
                backup_vault,
                security_module,
                mpc_listener,
                sk,
                tls_identity,
                need_peer_tcp_proxy,
                false,
                core_config.rate_limiter_conf,
                std::future::pending(),
                #[cfg(feature = "p2p")]
                p2p_inputs,
            )
            .await?;

            #[cfg(feature = "p2p")]
            {
                let router: Arc<InboundRouter> = router_rx
                    .await
                    .map_err(|_| anyhow::anyhow!("p2p inbound router was not provided by kms_threshold"))?;

                // Use the prebuilt map from build_topology()
                let peer_to_mpc: Arc<HashMap<PeerId, MpcIdentity>> = Arc::clone(&kry.peer_to_mpc);

                // (Optional sanity log)
                tracing::info!(
                    "peer_to_mpc {:?}",
                    peer_to_mpc,
                );

                kry.install_threshold_handler(router, peer_to_mpc).await?;
            }

            // ---- NEW: in-process gateway wiring (mandatory) ----
            let kms = Arc::new(kms); // Arc<ConcreteThresholdKms>
            // Create a *separate* Arc handle typed as the trait object for the gateway.
            let kms_for_gateway: Arc<dyn CoreServiceEndpoint + Send + Sync> = kms.clone();

            let _gateway_manager = KryphosPylonManager::new(args.kryphos_config, args.node_id, Arc::clone(&kms_for_gateway)).await?;

            #[cfg(feature = "p2p")]
            {
                kry.wait_mpc_mesh_ready().await?;
                tracing::info!("MPC P2P Mesh is READY (all required peers connected)");
                // if let Some(target) = kry.required_peers.first().copied() {
                //     let ctx_session_id = DEFAULT_MPC_CONTEXT.derive_session_id()?;
                //     let ht = HealthTag::new(my_mpc_identity.clone(), ctx_session_id);

                //     let tag_bytes = serialize(&ht)?;
                //     let wire = WireMsg::HealthCheck { tag: tag_bytes };
                //     let req_bytes = serialize(&wire)?;

                //     tracing::info!("Sending threshold HealthCheck to peer {}", target);
                //     let resp_bytes = kry.client.threshold_request(target, req_bytes).await?;
                //     let ack = deserialize_safe::<Ack>(&resp_bytes)?;
                //     tracing::info!("HealthCheck Ack from {}: {:?}", target, ack);
                // }
            }

            let meta_store_status_service = Arc::new(metastore_status_service);
            run_server(
                core_config.service,
                service_listener,
                Arc::clone(&kms),
                meta_store_status_service,
                health_service,
                std::future::pending(),
            )
            .await?;

            // If run_server ever returns, you can gracefully stop the swarm:
            #[cfg(feature="p2p")]
            kry.stop_swarm();
        }

        None => {
            tracing::info!(
                "Starting centralized KMS server v{}...",
                env!("CARGO_PKG_VERSION")
            );

            let (kms, health_service) = RealCentralizedKms::new(
                public_vault,
                private_vault,
                backup_vault,
                security_module,
                sk,
                core_config.rate_limiter_conf,
            )
            .await?;

            let meta_store_status_service = Arc::new(MetaStoreStatusServiceImpl::new(
                Some(Arc::clone(kms.get_key_gen_meta_store())),
                Some(Arc::clone(kms.get_pub_dec_meta_store())),
                Some(Arc::clone(kms.get_user_dec_meta_store())),
                Some(Arc::clone(kms.get_crs_meta_store())),
                None,
                Some(Arc::clone(kms.get_custodian_meta_store())),
            ));

            run_server(
                core_config.service,
                service_listener,
                Arc::new(kms),
                meta_store_status_service,
                health_service,
                std::future::pending(),
            )
            .await?;
        }
    }

    // telemetry shutdown
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    if let Err(e) = tracer_provider.shutdown() {
        eprintln!("Error shutting down tracer provider: {e}");
    }
    if let Err(e) = meter_provider.shutdown() {
        eprintln!("Error shutting down meter provider: {e}");
    }

    Ok(())
}
