use anyhow::{anyhow, Result};
use alloy_primitives::Address;
use kms_api::kms::v1::{
    CiphertextFormat, PublicDecryptionResponse, TypedCiphertext, TypedPlaintext,
};
use kms_api::{KeyId, RequestId};
use kms_core_client::{CoreClientConfig, EncryptionResult};
use kms_lib::client::client_wasm::Client;
use libp2p::{Multiaddr, PeerId};
use libp2p_common::komvos::Client as KomvosClient;
use observability::conf::Settings;
use rand::SeedableRng;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use threshold_fhe::execution::runtime::party::Role;

use crate::{
    build_public_decrypt_request, build_user_decrypt_request,
    ArtifactCache, MeshCoreClientBridge, MeshGatewayClient,
};

use kms_lib::consts::{DEFAULT_PARAM, SIGNING_KEY_ID, TEST_PARAM};
use kms_lib::vault::storage::{file::FileStorage, StorageType};

pub struct ThresholdFheClient {
    pub bridge: MeshCoreClientBridge,
    pub cache_root: PathBuf,
    pub party_ids: Vec<usize>,
    pub artifact_replica_party_id: usize,
}

impl ThresholdFheClient {
    pub async fn from_mesh(
        core_client_config_path: impl AsRef<Path>,
        cache_root: impl Into<PathBuf>,
        net_client: KomvosClient,
        discovered_pylons: Vec<(PeerId, Multiaddr)>,
    ) -> Result<Self> {
        let cache_root = cache_root.into();

        let cc_conf: CoreClientConfig = Settings::builder()
            .path(
                core_client_config_path
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| {
                        anyhow!(
                            "core client config path is not valid UTF-8: {}",
                            core_client_config_path.as_ref().display()
                        )
                    })?,
            )
            .env_prefix("CORE_CLIENT")
            .build()
            .init_conf()?;

        bootstrap_verification_material_from_config(&cc_conf, &cache_root).await?;

        let party_ids: Vec<usize> = cc_conf.cores.iter().map(|c| c.party_id).collect();

        let artifact_replica_party_id = *party_ids
            .first()
            .ok_or_else(|| anyhow!("no cores configured"))?;

        if discovered_pylons.is_empty() {
            return Err(anyhow!("no Pylon peers discovered from Mesh A"));
        }

        let gateway = MeshGatewayClient::new(
            net_client,
            ArtifactCache::new(cache_root.clone()),
            party_ids.clone(),
        );

        for (peer, addr) in discovered_pylons {
            gateway.upsert_pylon(peer, vec![addr]).await;
        }

        let internal_client = build_internal_client(&cc_conf, &cache_root).await?;
        let kms_addrs = read_kms_addresses_from_local(&cache_root, &cc_conf).await?;

        let bridge = MeshCoreClientBridge::new(gateway, internal_client, kms_addrs);

        Ok(Self {
            bridge,
            cache_root,
            party_ids,
            artifact_replica_party_id,
        })
    }

    pub async fn user_decrypt_bool(
        &mut self,
        key_id: &KeyId,
        ciphertext: Vec<u8>,
        ciphertext_format: CiphertextFormat,
    ) -> Result<bool> {
        
        let req_id = RequestId::new_random(&mut aes_prng::AesRng::from_entropy());

        let ciphertexts = vec![TypedCiphertext {
            ciphertext,
            fhe_type: 0, // ebool
            external_handle: vec![23u8; 32],
            ciphertext_format: ciphertext_format.into(),
        }];

        let (req, parsed, enc_pk, enc_sk) = build_user_decrypt_request(
            &mut self.bridge.internal_client,
            ciphertexts,
            &req_id,
            key_id,
        )?;

        let plaintexts = self
            .bridge
            .do_user_decrypt(req, parsed, enc_pk, enc_sk)
            .await?;

        let first = plaintexts
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("user decrypt returned no plaintexts"))?;

        typed_plaintext_to_bool(first)
    }

    pub async fn user_decrypt(
        &mut self,
        key_id: &KeyId,
        ciphertexts: Vec<TypedCiphertext>,
    ) -> Result<Vec<TypedPlaintext>> {
        let req_id = RequestId::new_random(&mut aes_prng::AesRng::from_entropy());

        let (req, parsed, enc_pk, enc_sk) = build_user_decrypt_request(
            &mut self.bridge.internal_client,
            ciphertexts,
            &req_id,
            key_id,
        )?;

        self.bridge
            .do_user_decrypt(req, parsed, enc_pk, enc_sk)
            .await
    }

    pub async fn encrypt_with_cached_material(
        &mut self,
        params: kms_core_client::CipherParameters,
    ) -> Result<EncryptionResult, Box<dyn std::error::Error + 'static>> {
        crate::encrypt::encrypt_with_gateway(
            &mut self.bridge.gateway,
            &self.cache_root,
            self.artifact_replica_party_id,
            params,
        )
        .await
    }
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

async fn build_internal_client(
    cc_conf: &CoreClientConfig,
    destination_prefix: &Path,
) -> Result<Client> {
    let num_parties = cc_conf.cores.len();
    let mut pub_storage: HashMap<u32, FileStorage> = HashMap::with_capacity(num_parties);

    let client_storage: FileStorage =
        FileStorage::new(Some(destination_prefix), StorageType::CLIENT, None).unwrap();

    match cc_conf.kms_type {
        kms_core_client::KmsType::Centralized => {
            pub_storage.insert(
                1,
                FileStorage::new(Some(destination_prefix), StorageType::PUB, None).unwrap(),
            );
        }
        kms_core_client::KmsType::Threshold => {
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

    let param = cc_conf.fhe_params.unwrap_or(kms_api::kms::v1::FheParameter::Default);
    let client_param = match param {
        kms_api::kms::v1::FheParameter::Test => TEST_PARAM,
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
    core: &kms_core_client::CoreConf,
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