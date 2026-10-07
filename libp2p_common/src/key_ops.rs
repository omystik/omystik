use crate::node_primary::KeyType;
use std::{io, path::{Path, PathBuf}, collections::HashMap};
use tokio::fs;

// checking solo key
pub fn key_received_as_expected(
    path: impl AsRef<Path>,
    expected_hex_name: &str,
) -> io::Result<bool> {
    let path = path.as_ref();

    if !path.exists() || !path.is_file() {
        return Ok(false);
    }

    let actual_name = match path.file_name().and_then(|s| s.to_str()) {
        Some(name) => name,
        None => return Ok(false),
    };

    if actual_name != expected_hex_name {
        return Ok(false);
    }

    Ok(std::fs::metadata(path)?.len() > 256)
}

pub fn unique_public_key_id_from_pub_folders(
    root: impl AsRef<Path>,
) -> io::Result<Option<String>> {
    let root = root.as_ref();

    let mut reference: Option<String> = None;
    let mut found_pub = false;

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;

        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        if !name.starts_with("PUB-p") {
            continue;
        }

        found_pub = true;

        let public_key_dir = root.join(&name).join(KeyType::PublicKey.as_str());
        let Some(key_id) = single_filename(public_key_dir)? else {
            return Ok(None);
        };

        match &reference {
            None => reference = Some(key_id),
            Some(expected) if expected == &key_id => {}
            Some(_) => return Ok(None),
        }
    }

    if found_pub {
        Ok(reference)
    } else {
        Ok(None)
    }
}

// checking existence of keys for kentr nodes -- <> --
pub fn check_all_pub_folders(root: impl AsRef<Path>
    ) -> io::Result<(bool, HashMap<KeyType, String>)> {
        let root = root.as_ref();
        let mut found_pub = false;
        let mut reference: HashMap<KeyType, String> = HashMap::new();

        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("PUB-p") {
                continue;
            }

            found_pub = true;

        let folder_map = match key_names_for_pub_folder(root, &name)? {
                Some(map) => map,
                None => return Ok((false, HashMap::new())),
        };

        if reference.is_empty() {
            reference = folder_map;
        } else {
            for key_type in [
                KeyType::PublicKey,
                KeyType::PublicKeyMetadata,
                KeyType::ServerKey,
            ] {
                let expected = reference.get(&key_type);
                let actual = folder_map.get(&key_type);

                if expected != actual {
                    return Ok((false, HashMap::new()));
                }
            }
        }
    }

    Ok((found_pub, reference))
}

fn key_names_for_pub_folder(
        root: &Path,
    pub_folder: &str,
) -> io::Result<Option<HashMap<KeyType, String>>> {
    let base = root.join(pub_folder);

    let pk = single_filename(base.join("PublicKey"))?;
    let pkm = single_filename(base.join("PublicKeyMetadata"))?;
    let sk = single_filename(base.join("ServerKey"))?;

    match (pk, pkm, sk) {
        (Some(pk), Some(pkm), Some(sk)) => {
            let mut map = HashMap::new();
            map.insert(KeyType::PublicKey, pk);
            map.insert(KeyType::PublicKeyMetadata, pkm);
            map.insert(KeyType::ServerKey, sk);
            Ok(Some(map))
        }
        _ => Ok(None),
    }
}

pub fn single_filename(dir: impl AsRef<Path>) -> io::Result<Option<String>> {
    let dir = dir.as_ref();

    if !dir.exists() || !dir.is_dir() {
        return Ok(None);
    }

    let files: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|ft| ft.is_file())
                .map(|_| entry.file_name().to_string_lossy().to_string())
        })
        .collect();

    if files.len() == 1 {
        Ok(Some(files[0].clone()))
    } else {
        Ok(None)
    }
}

pub async fn save_key_at_path(path: &Path, key_bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::write(path, key_bytes).await?;
    Ok(())
}

pub async fn save_kentr_pub_artifact_to_all_parties(
        keys_root: impl AsRef<Path>,
        key_type: KeyType,
        key_id_hex: &str,
        bytes: &[u8],
        party_count: usize,
    ) -> anyhow::Result<()> {
        let keys_root = keys_root.as_ref();

        for party in 1..=party_count {
            let path = keys_root
                .join(format!("PUB-p{party}"))
                .join(key_type.as_str())
                .join(key_id_hex);

            save_key_at_path(&path, bytes).await?;
    }

    Ok(())
}

pub async fn load_local_key(key_type: Option<KeyType>) -> anyhow::Result<PathBuf> {
    let key_type = key_type.unwrap_or_default();

    let keys_folder = content_hashing::get_keys_dir();

    let public_key_dir = Path::new(&keys_folder).join(key_type.to_string());

    let key_name = single_filename(&public_key_dir)?
        .ok_or_else(|| anyhow::anyhow!("expected exactly one PublicKey in {}", public_key_dir.display()))?;

    let key_path = public_key_dir.join(key_name);

    Ok(key_path)
}
