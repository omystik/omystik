use anyhow::{anyhow, Context, Result};
use content_hashing::get_keys_dir;
use futures::io::empty;
use libp2p::{Multiaddr, PeerId};
use libp2p_common::{
    key_ops,
    manager::KomvosManager,
    node_primary::KeyType,
    node_services::spread_into_network,
};
use observability::conf::Settings;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    fs,
};
use tokio::sync::Mutex;

use crate::{
    key_artifacts::parse_hex32,
    mesh_cmd::fetch_keyset_artifacts_from_pylon,
};

pub async fn rehydrate_keyset_from_mesh_local_key(
    kentr_node: Arc<Mutex<KomvosManager>>,
    keys_folder: &Path,
    core_client_config_path: &Path,
    discovered_pylons: Vec<(PeerId, Multiaddr)>,
) -> Result<()> {
    if discovered_pylons.is_empty() {
        return Err(anyhow!("cannot rehydrate keyset: no Pylon peers discovered"));
    }

    let cc_conf: kms_core_client::CoreClientConfig = Settings::builder()
        .path(
            core_client_config_path
                .to_str()
                .ok_or_else(|| {
                    anyhow!(
                        "config path is not valid UTF-8: {}",
                        core_client_config_path.display()
                    )
                })?,
        )
        .env_prefix("CORE_CLIENT")
        .build()
        .init_conf()
        .with_context(|| {
            format!(
                "failed to load core-client config {}",
                core_client_config_path.display()
            )
        })?;

    let party_count = match cc_conf.kms_type {
        kms_core_client::KmsType::Centralized => 1,
        kms_core_client::KmsType::Threshold => cc_conf.cores.len(),
    };

    // let key_id_hex = infer_key_id_hex_from_mesh_local_public_key()?;
    // let key_id = parse_hex32(&key_id_hex)?;

    let key_id_hex = key_ops::unique_public_key_id_from_pub_folders(keys_folder)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot infer key id from {}/PUB-p*/PublicKey",
                    keys_folder.display()
                )
            })?;

    let key_id = parse_hex32(&key_id_hex)?;

    let net_client = { kentr_node.lock().await.client.clone() };

    let artifacts = fetch_keyset_artifacts_from_pylon(
        net_client,
        discovered_pylons,
        key_id,
    )
    .await
    .context("fetch keyset artifacts from Pylon")?;

    for (kind, bytes) in artifacts {
        let key_type = match kind {
            mesh_gateway_wire::ArtifactKind::PublicKey => KeyType::PublicKey,
            mesh_gateway_wire::ArtifactKind::PublicKeyMetadata => KeyType::PublicKeyMetadata,
            mesh_gateway_wire::ArtifactKind::ServerKey => KeyType::ServerKey,
            other => {
                return Err(anyhow!(
                    "unexpected keyset artifact kind from Pylon: {:?}",
                    other
                ));
            }
        };

        key_ops::save_kentr_pub_artifact_to_all_parties(
            keys_folder,
            key_type,
            &key_id_hex,
            &bytes,
            party_count,
        )
        .await
        .with_context(|| {
            format!(
                "save {:?} artifact {} to local KMS-style cache {}",
                key_type,
                key_id_hex,
                keys_folder.display()
            )
        })?;
    }

    check_keys_kentr(kentr_node, keys_folder).await
}

/// Post-check for Kentr after key materialization.
///
/// If key material is present and coherent under `keys_folder`,
/// this mirrors the canonical key artifacts to the mesh-local key cache:
///
///   data/core/keys/<KeyType>/<key_id>
///
/// and advertises each KeyType into the DHT.
///
/// This is what allows NonAdmin nodes to later call:
///
///   ask_key(Some(KeyType::PublicKey))
///   ask_key(Some(KeyType::ServerKey))
///
/// without querying Pylon directly.
pub async fn check_keys_kentr(
    kentr: Arc<Mutex<KomvosManager>>,
    keys_folder: &Path,
) -> Result<()> {
    let (ok, key_map) = key_ops::check_all_pub_folders(keys_folder)?;

    if !ok {
        return Ok(());
    }

    let mut guard = kentr.lock().await;

    let shared_keys_dir = PathBuf::from(get_keys_dir());

    empty_dir_contents(&shared_keys_dir)
        .with_context(|| format!("empty keys directory {}", shared_keys_dir.display()))?;

    for (key_type, key_name) in key_map {
        let source_path = keys_folder
            .join("PUB-p1")
            .join(key_type.as_str())
            .join(&key_name);

        let key_bytes = tokio::fs::read(&source_path)
            .await
            .with_context(|| format!("read {}", source_path.display()))?;

        let output_path = shared_keys_dir
            .join(key_type.as_str())
            .join(&key_name);

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output parent {}", parent.display()))?;
        }

        key_ops::save_key_at_path(&output_path, &key_bytes)
            .await
            .with_context(|| format!("save {}", output_path.display()))?;

        spread_into_network(&mut guard.client, key_type.to_string())
            .await
            .map_err(|e| anyhow!(e.to_string()))
            .with_context(|| format!("spread {} into network", key_type))?;

        println!("{} ({}) spread on DHT successfully", key_type, key_name);
    }

    Ok(())
}

fn empty_dir_contents(dir: &Path) -> Result<()> {
    if !dir.exists() {
        fs::create_dir_all(dir)
            .with_context(|| format!("create directory {}", dir.display()))?;
        return Ok(());
    }

    if !dir.is_dir() {
        anyhow::bail!("{} exists but is not a directory", dir.display());
    }

    for entry in fs::read_dir(dir)
        .with_context(|| format!("read directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove directory {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("remove file {}", path.display()))?;
        }
    }

    Ok(())
}
