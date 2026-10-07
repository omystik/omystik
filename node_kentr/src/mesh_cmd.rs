use anyhow::{anyhow, Result};
use alloy_primitives::Address;
use plegma_fhe_pelatis::{
    build_crsgen_request, build_keygen_request, build_preproc_request,
    build_public_decrypt_request, build_user_decrypt_request,
    ArtifactCache, MeshCoreClientBridge, MeshGatewayClient,
};
use observability::conf::Settings;
use rand::SeedableRng;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kms_api::kms::v1::{
    CiphertextFormat, FheParameter, TypedCiphertext, TypedPlaintext};
use kms_api::{KeyId, RequestId};
use kms_core_client::{
    fetch_ctxt_from_file, CCCommand, CipherArguments, CmdConfig, CoreClientConfig, CoreConf,
    EncryptionResult, KmsType,
};
use kms_lib::client::client_wasm::Client;
use kms_lib::consts::{DEFAULT_PARAM, SIGNING_KEY_ID, TEST_PARAM};
use kms_lib::util::key_setup::test_tools::TestingPlaintext;
use kms_lib::vault::storage::{file::FileStorage, StorageType};
use threshold_fhe::execution::runtime::party::Role;
use libp2p::{PeerId, Multiaddr};
use libp2p_common::komvos::Client as KentrNetClient;

use mesh_gateway_wire::{
    ArtifactKind, FaceARequest, FaceAResponse, Payload,
};

pub async fn execute_mesh_or_fallback(
    cmd_cfg: &CmdConfig,
    destination_prefix: &Path,
    net_client: KentrNetClient,
    discovered_pylons: Vec<(PeerId, Multiaddr)>,
) -> Result<Vec<(Option<RequestId>, String)>, Box<dyn std::error::Error + 'static>> {
    match &cmd_cfg.command {
        CCCommand::PreprocKeyGen(_)
        | CCCommand::KeyGen(_)
        | CCCommand::KeyGenFresh(_)
        | CCCommand::CrsGen(_)
        | CCCommand::Encrypt(_)
        | CCCommand::PublicDecrypt(_)
        | CCCommand::UserDecrypt(_) => {}
        _ => {
            return kms_core_client::execute_cmd(cmd_cfg, destination_prefix).await;
        }
    }

    let cc_conf = load_core_client_conf(cmd_cfg)?;
    bootstrap_verification_material_from_config(&cc_conf, destination_prefix).await?;

    let cache_party_ids: Vec<usize> = cc_conf.cores.iter().map(|c| c.party_id).collect();

    let artifact_replica_party_id = *cache_party_ids
        .first()
        .ok_or_else(|| anyhow!("no cores configured"))?;

    let gateway = MeshGatewayClient::new(
        net_client,
        ArtifactCache::new(destination_prefix.to_path_buf()),
        cache_party_ids,
    );

    if discovered_pylons.is_empty() {
        return Err("no Pylon peers discovered from Mesh A".into());
    }

    for (peer, addr) in discovered_pylons {
        gateway.upsert_pylon(peer, vec![addr]).await;
    }

    let internal_client = build_internal_client(&cc_conf, destination_prefix).await?;
    let kms_addrs = read_kms_addresses_from_local(destination_prefix, &cc_conf).await?;

    let mut bridge = MeshCoreClientBridge::new(gateway, internal_client, kms_addrs);

    let mut rng = aes_prng::AesRng::from_entropy();
    let param = cc_conf.fhe_params.unwrap_or(FheParameter::Default);

    let out = match &cmd_cfg.command {
        CCCommand::PreprocKeyGen(_) => {
            let req_id = RequestId::new_random(&mut rng);
            let req = build_preproc_request(&mut bridge.internal_client, &req_id, param)?;
            let rid = bridge.do_preproc_keygen(req_id, req).await?;
            vec![(Some(rid), "preproc done".to_string())]
        }

        CCCommand::KeyGen(args) => {
            let req_id = RequestId::new_random(&mut rng);
            let req = build_keygen_request(
                &mut bridge.internal_client,
                &req_id,
                &args.preproc_id,
                param,
                &args.shared_args,
            )?;
            let rid = bridge
                .do_keygen(
                    destination_prefix,
                    req_id,
                    args.preproc_id,
                    req,
                    artifact_replica_party_id,
                )
                .await?;
            vec![(Some(rid), "keygen done".to_string())]
        }

        CCCommand::KeyGenFresh(args) => {
            let preproc_id = run_preproc_keygen(&mut bridge, &mut rng, param).await?;
            let keygen_id = run_keygen(
                &mut bridge,
                destination_prefix,
                &mut rng,
                param,
                preproc_id,
                &args.shared_args,
                artifact_replica_party_id,
            ).await?;

            vec![
                (Some(preproc_id), "preproc done".to_string()),
                (Some(keygen_id), "keygen done".to_string()),
            ]
        }

        CCCommand::CrsGen(args) => {
            let req_id = RequestId::new_random(&mut rng);
            let req = build_crsgen_request(
                &mut bridge.internal_client,
                &req_id,
                Some(args.max_num_bits),
                param,
            )?;
            let rid = bridge
                .do_crsgen(destination_prefix, req_id, req, artifact_replica_party_id)
                .await?;
            vec![(Some(rid), "crsgen done".to_string())]
        }

        CCCommand::Encrypt(cipher_params) => {
            bridge
                .do_encrypt(destination_prefix, artifact_replica_party_id, cipher_params.clone())
                .await?;
            vec![(None, "Encryption generated".to_string())]
        }

        CCCommand::PublicDecrypt(cipher_args) => {
            let EncryptionResult {
                cipher,
                ct_format,
                plaintext,
                key_id,
            } = match cipher_args {
                CipherArguments::FromFile(f) => fetch_ctxt_from_file(f.input_path.clone()).await?,
                CipherArguments::FromArgs(args) => {
                    bridge
                        .do_encrypt(destination_prefix, artifact_replica_party_id, args.clone())
                        .await?
                }
            };

            let batch_size = cipher_args.get_batch_size();
            let ciphertexts = vec![
                TypedCiphertext {
                    ciphertext: cipher,
                    fhe_type: plaintext.fhe_type,
                    external_handle: vec![23u8; 32],
                    ciphertext_format: ct_format.into(),
                };
                batch_size
            ];

            let req_id = RequestId::new_random(&mut rng);
            let req = build_public_decrypt_request(
                &mut bridge.internal_client,
                ciphertexts,
                &req_id,
                &key_id,
            )?;

            let responses = bridge
                .do_public_decrypt(req, Some(plaintext.clone()))
                .await?;

            vec![(
                Some(req_id),
                format!("public decrypt verified; {} response(s)", responses.len()),
            )]
        }

        CCCommand::UserDecrypt(cipher_args) => {
            let EncryptionResult {
                cipher,
                ct_format,
                plaintext,
                key_id,
            } = match cipher_args {
                CipherArguments::FromFile(f) => fetch_ctxt_from_file(f.input_path.clone()).await?,
                CipherArguments::FromArgs(args) => {
                    bridge
                        .do_encrypt(destination_prefix, artifact_replica_party_id, args.clone())
                        .await?
                }
            };

            let batch_size = cipher_args.get_batch_size();
            let ciphertexts = vec![
                TypedCiphertext {
                    ciphertext: cipher,
                    fhe_type: plaintext.fhe_type,
                    external_handle: vec![23u8; 32],
                    ciphertext_format: ct_format.into(),
                };
                batch_size
            ];

            let req_id = RequestId::new_random(&mut rng);
            let (req, parsed, enc_pk, enc_sk) = build_user_decrypt_request(
                &mut bridge.internal_client,
                ciphertexts,
                &req_id,
                &key_id,
            )?;

            let plaintexts = bridge
                .do_user_decrypt(req, parsed, enc_pk, enc_sk)
                .await?;

            let first = plaintexts
                .first()
                .cloned()
                .ok_or_else(|| anyhow!("user decrypt returned no plaintexts"))?;

            let original = TestingPlaintext::try_from(plaintext.clone())?;
            let reconstructed = TestingPlaintext::try_from(first.clone())?;
            if original != reconstructed {
                return Err(anyhow!(
                    "user decrypt plaintext mismatch: expected {:?}, got {:?}",
                    original,
                    reconstructed
                )
                .into());
            }

            vec![(
                Some(req_id),
                format!("User decrypted Plaintext {:?}", reconstructed),
            )]
        }

        _ => unreachable!(),
    };

    Ok(out)
}

fn load_core_client_conf(cmd_cfg: &CmdConfig) -> Result<CoreClientConfig> {
    let path_to_config = cmd_cfg
        .file_conf
        .clone()
        .ok_or_else(|| anyhow!("CmdConfig.file_conf is required"))?;

    let cc_conf: CoreClientConfig = Settings::builder()
        .path(&path_to_config)
        .env_prefix("CORE_CLIENT")
        .build()
        .init_conf()?;

    Ok(cc_conf)
}

async fn build_internal_client(
    cc_conf: &CoreClientConfig,
    destination_prefix: &Path,
) -> Result<Client> {
    let num_parties = cc_conf.cores.len();
    let mut pub_storage: HashMap<u32, FileStorage> = HashMap::with_capacity(num_parties);

    let client_storage: FileStorage =
        FileStorage::new(Some(destination_prefix), StorageType::CLIENT, None).unwrap();

    match cc_conf.kms_type {
        KmsType::Centralized => {
            pub_storage.insert(
                1,
                FileStorage::new(Some(destination_prefix), StorageType::PUB, None).unwrap(),
            );
        }
        KmsType::Threshold => {
            for cur_core in &cc_conf.cores {
                pub_storage.insert(
                    cur_core.party_id as u32,
                    FileStorage::new(
                        Some(destination_prefix),
                        StorageType::PUB,
                        Some(Role::indexed_from_one(cur_core.party_id)),
                    )
                    .unwrap(),
                );
            }
        }
    }

    let param = cc_conf.fhe_params.unwrap_or(FheParameter::Default);
    let client_param = match param {
        FheParameter::Test => TEST_PARAM,
        _ => DEFAULT_PARAM,
    };

    let client = Client::new_client(
        client_storage,
        pub_storage,
        &client_param,
        cc_conf.decryption_mode,
    )
    .await
    .map_err(|e| anyhow!("failed to build internal KMS client: {e}"))?;

    Ok(client)
}

async fn bootstrap_verification_material_from_config(
    cc_conf: &CoreClientConfig,
    destination_prefix: &Path,
) -> Result<()> {
    for core in &cc_conf.cores {
        copy_party_pub_artifact(
            core,
            destination_prefix,
            "VerfKey",
            &SIGNING_KEY_ID.to_string(),
        )
        .await?;
        copy_party_pub_artifact(
            core,
            destination_prefix,
            "VerfAddress",
            &SIGNING_KEY_ID.to_string(),
        )
        .await?;
    }
    Ok(())
}

async fn copy_party_pub_artifact(
    core: &CoreConf,
    destination_prefix: &Path,
    kind: &str,
    id: &str,
) -> Result<()> {
    let src = PathBuf::from(&core.s3_endpoint)
        .join(&core.object_folder)
        .join(kind)
        .join(id);

    let dst = destination_prefix
        .join(format!("PUB-p{}", core.party_id))
        .join(kind)
        .join(id);

    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let bytes = tokio::fs::read(&src)
        .await
        .map_err(|e| anyhow!("failed to read bootstrap artifact {:?}: {e}", src))?;

    tokio::fs::write(&dst, bytes)
        .await
        .map_err(|e| anyhow!("failed to write bootstrap artifact {:?}: {e}", dst))?;

    Ok(())
}

async fn read_kms_addresses_from_local(
    destination_prefix: &Path,
    cc_conf: &CoreClientConfig,
) -> Result<Vec<Address>> {
    let mut out = Vec::with_capacity(cc_conf.cores.len());

    for core in &cc_conf.cores {
        let path = destination_prefix
            .join(format!("PUB-p{}", core.party_id))
            .join("VerfAddress")
            .join(SIGNING_KEY_ID.to_string());

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("failed to read KMS address {:?}: {e}", path))?;

        let addr = Address::parse_checksummed(content.trim(), None)
            .map_err(|e| anyhow!("invalid KMS address in {:?}: {e}", path))?;

        out.push(addr);
    }

    Ok(out)
}

async fn run_preproc_keygen(
    bridge: &mut MeshCoreClientBridge,
    rng: &mut aes_prng::AesRng,
    param: FheParameter,
) -> Result<RequestId> {
    let req_id = RequestId::new_random(rng);
    let req = build_preproc_request(&mut bridge.internal_client, &req_id, param)?;
    bridge.do_preproc_keygen(req_id, req).await
}

async fn run_keygen(
    bridge: &mut MeshCoreClientBridge,
    destination_prefix: &Path,
    rng: &mut aes_prng::AesRng,
    param: FheParameter,
    preproc_id: RequestId,
    shared_args: &kms_core_client::SharedKeyGenParameters,
    artifact_replica_party_id: usize,
) -> Result<RequestId> {
    let req_id = RequestId::new_random(rng);
    let req = build_keygen_request(
        &mut bridge.internal_client,
        &req_id,
        &preproc_id,
        param,
        shared_args,
    )?;
    bridge
        .do_keygen(
            destination_prefix,
            req_id,
            preproc_id,
            req,
            artifact_replica_party_id,
        )
        .await
}

pub async fn user_decrypt_bool_ciphertext_via_mesh(
    cmd_cfg: &CmdConfig,
    destination_prefix: &Path,
    net_client: KentrNetClient,
    discovered_pylons: Vec<(PeerId, Multiaddr)>,
    key_id: KeyId,
    ciphertext: Vec<u8>,
) -> Result<bool, Box<dyn std::error::Error + 'static>> {
    let cc_conf = load_core_client_conf(cmd_cfg)?;
    bootstrap_verification_material_from_config(&cc_conf, destination_prefix).await?;

    let cache_party_ids: Vec<usize> = cc_conf.cores.iter().map(|c| c.party_id).collect();

    let gateway = MeshGatewayClient::new(
        net_client,
        ArtifactCache::new(destination_prefix.to_path_buf()),
        cache_party_ids,
    );

    if discovered_pylons.is_empty() {
        return Err("no Pylon peers discovered from Mesh A".into());
    }

    for (peer, addr) in discovered_pylons {
        gateway.upsert_pylon(peer, vec![addr]).await;
    }

    let internal_client = build_internal_client(&cc_conf, destination_prefix).await?;
    let kms_addrs = read_kms_addresses_from_local(destination_prefix, &cc_conf).await?;

    let mut bridge = MeshCoreClientBridge::new(gateway, internal_client, kms_addrs);

    let mut rng = aes_prng::AesRng::from_entropy();
    let req_id = RequestId::new_random(&mut rng);

    let ciphertexts = vec![TypedCiphertext {
        ciphertext,
        fhe_type: 0, // ebool
        external_handle: vec![23u8; 32],
        ciphertext_format: CiphertextFormat::SmallExpanded as i32,
    }];

    let (req, parsed, enc_pk, enc_sk) = build_user_decrypt_request(
        &mut bridge.internal_client,
        ciphertexts,
        &req_id,
        &key_id,
    )?;

    let plaintexts = bridge
        .do_user_decrypt(req, parsed, enc_pk, enc_sk)
        .await?;

    let first = plaintexts
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("user decrypt returned no plaintexts"))?;

    typed_plaintext_to_bool(first)
        .map_err(|e| e.into())
}

fn typed_plaintext_to_bool(pt: TypedPlaintext) -> Result<bool> {
    if pt.fhe_type != 0 {
        return Err(anyhow!(
            "expected decrypted ebool plaintext fhe_type=0, got {}",
            pt.fhe_type
        ));
    }

    match pt.bytes.as_slice() {
        [0] => Ok(false),
        [1] => Ok(true),
        other => Err(anyhow!(
            "expected decrypted bool bytes [0] or [1], got {:?}",
            other
        )),
    }
}

pub async fn fetch_keyset_artifacts_from_pylon(
    mut net_client: KentrNetClient,
    discovered_pylons: Vec<(PeerId, Multiaddr)>,
    key_id: [u8; 32],
) -> Result<Vec<(ArtifactKind, Vec<u8>)>> {
    let (pylon_peer, _addr) = discovered_pylons
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("no Pylon peers discovered from Mesh A"))?;

    let mut out = Vec::new();

    for kind in [
        ArtifactKind::PublicKey,
        ArtifactKind::PublicKeyMetadata,
        ArtifactKind::ServerKey,
    ] {
        let resp = net_client
            .jobs_request(
                pylon_peer,
                FaceARequest::GetArtifact {
                    id: key_id,
                    kind: kind.clone(),
                },
            )
            .await
            .map_err(|e| anyhow!("GetArtifact {:?} failed: {e}", kind))?;

        let bytes = match resp {
            FaceAResponse::Artifact {
                payload: Payload::Inline(bytes),
                ..
            } => bytes,

            FaceAResponse::Artifact {
                payload: Payload::BlobRef(blob_id),
                ..
            } => {
                net_client
                    .get_blob(pylon_peer, blob_id)
                    .await
                    .map_err(|e| anyhow!("get_blob for {:?} failed: {e}", kind))?
            }

            FaceAResponse::Error { message } => {
                return Err(anyhow!(
                    "artifact fetch failed for {:?}: {message}",
                    kind
                ));
            }

            other => {
                return Err(anyhow!(
                    "unexpected GetArtifact response for {:?}: {other:?}",
                    kind
                ));
            }
        };

        out.push((kind, bytes));
    }

    Ok(out)
}