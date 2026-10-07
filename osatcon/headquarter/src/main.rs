mod app;
mod ui;

use anyhow::{anyhow, Context, Result};
use clap::Parser;

use kms_api::KeyId;

use kms_lib::consts::SIGNING_KEY_ID;
use kms_lib::util::key_setup::ensure_client_keys_exist;

use libp2p_common::key_ops;

use plegma_fhe_pelatis::ThresholdFheClient;

use std::collections::HashMap;

use kryptografia::{
    CiphertextFormat,
    FheEncryptor,
    TypedCiphertext,
    TypedPlaintext,
    FHE_UINT64,
};

use mesh_gateway_wire::encode;

use ofield_core::sync_scenario::{OsatconScenarioV1, session_id_from_code};
use ofield_core::projection::{
    decode_lat_projected_u64,
    decode_lon_projected_u64,
    encode_distance_threshold_sq_u64
};

use node_kentr::{
    JobMetadata,
    KentrClusterConfig,
    KentrConfig,
    KeyType,
    key_rehydration::rehydrate_keyset_from_mesh_local_key,
    KomvosManager,
    Multiaddr,
    PeerId,
    YpoloClusterConfig,
    YpoloConfig,
};

use smart_program_alert::{
    AlertMathOutputV1,
    AlertMathEncryptedConfigV1,
    EncryptedDistanceBatchV1,
    decode_encrypted_convoy_payload_v1,
    decode_encrypted_satellite_payload_v1,
    poll_alert_output_v1,
    push_alert_config_v1,
    start_alert_session_v1,
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, RwLock as StdRwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::Mutex,
    time::{sleep, Duration, Instant},
};

use crate::app::{
    MeshAlertUiState,
    MeshFheAlertContext,
    MeshFheSatelliteContext,
};

#[derive(Debug, Parser)]
#[command(name = "osatcon-simulation")]
#[command(about = "Osatcon simulation UI running through a Kentr transport node")]
pub struct Args {
    /// Path to the Kentr mesh config TOML
    #[arg(long)]
    pub kentr_cluster: PathBuf,

    /// Kentr node id used by the simulation endpoint
    #[arg(long)]
    pub kentr_id: u32,

    /// Path to the Ypolo mesh config TOML
    #[arg(long)]
    pub ypolo_cluster: PathBuf,

    /// Ypolo node id used for FHE compute
    #[arg(long)]
    pub ypolo_id: u32,

    #[arg(long)]
    ui_only: bool,

    /// Shared session code used to derive the stable 32-byte session id.
    /// Must be passed to simulation, edge-convoy, and edge-satellite.
    #[arg(long)]
    pub session_code: String,

    /// args passed to core-client, for example: -- -f config/client_threshold.toml
    #[arg(last = true)]
    pub core_client_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let session_id = session_id_from_code(&args.session_code);

    let scenario_json = std::fs::read_to_string("data/scenario/scenario.json")
        .context("read scenario file data/scenario/scenario.json")?;

    let scenario = OsatconScenarioV1::from_json_str(&scenario_json)
        .context("parse data/scenario/scenario.json")?;

    let mesh_alert = Arc::new(StdRwLock::new(MeshAlertUiState::default()));
        
    if args.ui_only {
        println!("Launching Osatcon simulation UI-only mode");
        println!("Kentr/Ypolo args are ignored in UI-only mode");
        println!("Session id={}", hex::encode(session_id));

        return app::run(mesh_alert, scenario);
    }

    let kentr_cfg = load_kentr_config(&args.kentr_cluster, args.kentr_id)?;
    let ypolo_cfg = load_ypolo_config(&args.ypolo_cluster, args.ypolo_id)?;

    let core_client_cfg_path = core_client_config_path(&args.core_client_args)?;

    let ypolo_peer = PeerId::from_str(&ypolo_cfg.peer_id)
        .context("invalid Ypolo peer_id")?;

    let namespace_static: &'static str =
        Box::leak(kentr_cfg.namespace.clone().into_boxed_str());

    println!("Launching Osatcon simulation through Kentr transport");
    println!("Kentr id={}", kentr_cfg.kentr_id);
    println!("Ypolo id={} peer={}", ypolo_cfg.ypolo_id, ypolo_peer);
    println!("Session id={}", hex::encode(session_id));

    let kentr_node = Arc::new(Mutex::new(
        KomvosManager::new(
            Some(kentr_cfg.secret_seed),
            Some(kentr_cfg.listen_address.clone()),
            None,
            kentr_cfg.node_type,
            kentr_cfg.node_role,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("create simulation Kentr KomvosManager")?,
    ));

    sleep(Duration::from_secs(3)).await;

    {
        let node = kentr_node.clone();
        tokio::spawn(async move {
            if let Err(e) = node.lock().await.launch_inbound_request_task().await {
                eprintln!("simulation Kentr inbound handler error: {e}");
            }
        });
    }

    let bootstrap_addr = Multiaddr::from_str(&kentr_cfg.syndesmos_address)
        .context("invalid Kentr syndesmos_address")?;

    let bootstrap_peer = PeerId::from_str(&kentr_cfg.syndesmos_peer_id_1)
        .context("invalid Kentr syndesmos_peer_id_1")?;

    kentr_node
        .lock()
        .await
        .register_rendezvous(
            bootstrap_addr,
            bootstrap_peer,
            namespace_static,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("register simulation Kentr endpoint to rendezvous")?;

    sleep(Duration::from_secs(3)).await;
 

    let discovered_pylons = {
        kentr_node.lock().await.pylon_peers().await
    };

    if discovered_pylons.is_empty() {
        eprintln!("warning: no Pylon Face-A peers discovered yet; threshold decrypt will fail until Pylon is discovered");
    }

    let keys_folder = PathBuf::from("keys");
   
    rehydrate_keyset_from_mesh_local_key(
        Arc::clone(&kentr_node),
        &keys_folder,
        &core_client_cfg_path,
        discovered_pylons.clone(),
    )
    .await
    .context("rehydrate Kentr keyset for simulation threshold decrypt")?;

    ensure_client_keys_exist(Some(&keys_folder), &SIGNING_KEY_ID, true).await;


    let key_id = current_kentr_key_id(&keys_folder)
        .context("resolve current Kentr key_id for threshold decrypt")?;

    let net_client = {
        kentr_node.lock().await.client.clone()
    };

    let threshold_client = ThresholdFheClient::from_mesh(
        &core_client_cfg_path,
        keys_folder.clone(),
        net_client,
        discovered_pylons,
    )
    .await
    .context("build ThresholdFheClient for simulation result decrypt")?;
    
    bootstrap_alert_session(
        kentr_node.clone(),
        ypolo_peer,
        ypolo_cfg.listen_address.clone(),
        session_id,
        scenario.clone(),
    )
    .await?;

    spawn_alert_result_poller(
        kentr_node.clone(),
        ypolo_peer,
        session_id,
        mesh_alert.clone(),
        threshold_client,
        key_id,
        scenario.clone(),
    );

    app::run(mesh_alert, scenario)
}

async fn bootstrap_alert_session(
    kentr_node: Arc<Mutex<KomvosManager>>,
    ypolo_peer: PeerId,
    ypolo_addr: Multiaddr,
    session_id: [u8; 32],
    scenario: OsatconScenarioV1,
) -> Result<()> {
    let mut guard = kentr_node.lock().await;

    guard
        .client
        .dial(ypolo_peer, ypolo_addr)
        .await
        .context("dial Ypolo from simulation Kentr endpoint")?;

    sleep(Duration::from_secs(1)).await;

    start_alert_session_v1(
        &mut guard.client,
        ypolo_peer,
        session_id,
        JobMetadata {
            created_unix_ms: now_unix_ms(),
            client_hint: Some("osatcon-simulation".into()),
        },
    )
    .await
    .context("start alert session on Ypolo")?;

    let public_key_path = libp2p_common::key_ops::load_local_key(Some(KeyType::PublicKey))
        .await
        .context("load local PublicKey for encrypting config_alert_zone")?;

    let encryptor = FheEncryptor::from_public_key_path(public_key_path)
        .await
        .context("load CompactPublicKey for encrypting config_alert_zone")?;


    let radius_sq = encode_distance_threshold_sq_u64(
        scenario.alert_radius_km,
        &scenario,
    )
    .context("encode radius_sq for FHE projected coordinate space")?;

    let max_sats_scan = scenario.max_sats_scan as u64;
    
    let config_alert_zone = AlertMathEncryptedConfigV1 {
        radius_sq: encrypt_u64(&encryptor, radius_sq)
            .context("encrypt radius_sq")?,
        max_sats_scan: encrypt_u64(&encryptor, max_sats_scan)
            .context("encrypt max_sats_scan")?,
    };

    push_alert_config_v1(
        &mut guard.client,
        ypolo_peer,
        session_id,
        config_alert_zone,
        now_unix_ms(),
        1,
    )
    .await
    .context("push config_alert_zone to Ypolo session")?;

    println!("Alert session bootstrapped through Kentr");

    Ok(())
}

fn load_kentr_config(path: &Path, kentr_id: u32) -> Result<KentrConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: KentrClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.kentr
        .into_iter()
        .find(|p| p.kentr_id == kentr_id)
        .with_context(|| format!("kentr_id {} not found", kentr_id))
}

fn load_ypolo_config(path: &Path, ypolo_id: u32) -> Result<YpoloConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: YpoloClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.ypolo
        .into_iter()
        .find(|p| p.ypolo_id == ypolo_id)
        .with_context(|| format!("ypolo_id {} not found", ypolo_id))
}

fn encrypt_u64(encryptor: &FheEncryptor, value: u64) -> Result<Vec<u8>> {
    let ct = encryptor
        .encrypt_typed_plaintext(TypedPlaintext {
            bytes: value.to_le_bytes().to_vec(),
            fhe_type: FHE_UINT64,
        })?;

    Ok(ct.ciphertext)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// retry decrypt
#[derive(Debug, Clone)]
struct DecryptRetryState {
    attempts: u32,
    next_retry_at: Instant,
    last_error: String,
}

const MAX_DECRYPT_RETRY_BACKOFF: Duration = Duration::from_secs(15);

fn decrypt_retry_backoff(attempts: u32) -> Duration {
    match attempts {
        0 | 1 => Duration::from_millis(500),
        2 => Duration::from_secs(10),
        3 => Duration::from_secs(20),
        4 => Duration::from_secs(30),
        5 => Duration::from_secs(60),
        _ => MAX_DECRYPT_RETRY_BACKOFF,
    }
}

fn is_retryable_decrypt_error(msg: &str) -> bool {
    let msg_lc = msg.to_ascii_lowercase();

    // These errors usually mean Headquarter asked too early, a pylon/kryphos
    // transport response was empty/truncated, or a peer/session has not caught up yet.
    msg_lc.contains("notfound")
        || msg_lc.contains("does not exist")
        || msg_lc.contains("could not retrieve userdecryption")
        || msg_lc.contains("timeout while waiting")
        || msg_lc.contains("pending")
        || msg_lc.contains("unknown job_id")
        || msg_lc.contains("decode facearesponse failed")
        || msg_lc.contains("decode facebresponse failed")
        || msg_lc.contains("unexpectedend")
        || msg_lc.contains("outbound failure")
        || msg_lc.contains("failed to dial")
        || msg_lc.contains("dispatch outcome unknown")
        || msg_lc.contains("status transport failure")
        || msg_lc.contains("insufficient successful faceb results")
}
// ---

fn spawn_alert_result_poller(
    kentr_node: Arc<Mutex<KomvosManager>>,
    ypolo_peer: PeerId,
    session_id: [u8; 32],
    mesh_alert: Arc<StdRwLock<MeshAlertUiState>>,
    mut threshold_client: ThresholdFheClient,
    key_id: KeyId,
    scenario: OsatconScenarioV1,
) {
    tokio::spawn(async move {
        let mut sequence: u64 = 0;

        let mut decrypted_seen: std::collections::HashSet<[u8; 32]> =
            std::collections::HashSet::new();

        // Only permanently bad outputs go here.
        // Timing/readiness failures must not be inserted here.
        let mut decrypted_failed_seen: std::collections::HashSet<[u8; 32]> =
            std::collections::HashSet::new();

        // Retryable decrypt failures are throttled here so Headquarter does not
        // hammer KMS/Pylon every 500ms while convoy/satellite/Ypolo state catches up.
        let mut decrypt_retry_state: HashMap<[u8; 32], DecryptRetryState> = HashMap::new();

        loop {
            sequence = sequence.saturating_add(1);

            let result = {
                let mut guard = kentr_node.lock().await;

                poll_alert_output_v1(
                    &mut guard.client,
                    ypolo_peer,
                    session_id,
                )
                .await
            };

            match result {
                Ok(Some(output)) => {
                    let output_hash = output_hash(&output);

                    if let Ok(mut state) = mesh_alert.write() {
                        state.latest_sequence = sequence;
                        state.encrypted_result_available = true;
                        state.last_metadata = output.metadata.clone();
                        state.compute_error = None;
                    }

                    if decrypted_seen.contains(&output_hash)
                        || decrypted_failed_seen.contains(&output_hash)
                    {
                        decrypt_retry_state.remove(&output_hash);
                        sleep(Duration::from_millis(500)).await;
                        continue;
                    }

                    
                    if let Some(retry) = decrypt_retry_state.get(&output_hash) {
                        if Instant::now() < retry.next_retry_at {
                            if let Ok(mut state) = mesh_alert.write() {
                                state.latest_sequence = sequence;
                                state.decrypt_error = Some(format!(
                                    "decrypt not ready yet; retrying after backoff; attempts={}; last_error={}",
                                    retry.attempts,
                                    retry.last_error,
                                ));
                            }

                            sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    }

                    let decrypt_result: Result<(bool, MeshFheAlertContext)> = async {
                        let exposed = threshold_decrypt_bool(
                            &mut threshold_client,
                            &key_id,
                            output.encrypted_outputs.exposed_ct.clone(),
                        )
                        .await
                        .context("decrypt exposed_ct")?;

                        let context = decrypt_context_of_exposition_batch(
                            &mut threshold_client,
                            &key_id,
                            output.encrypted_outputs.context_of_exposition.clone(),
                            output.encrypted_outputs.distance_context.clone(),
                            &scenario,
                        )
                        .await
                        .context("decrypt context_of_exposition")?;

                        Ok((exposed, context))
                    }
                    .await;

                    match decrypt_result {
                        Ok((exposed, context)) => {
                            decrypted_seen.insert(output_hash);
                            decrypt_retry_state.remove(&output_hash);

                            if let Ok(mut state) = mesh_alert.write() {
                                state.latest_sequence = sequence;
                                state.encrypted_result_available = true;
                                state.fhe_exposed = Some(exposed);
                                state.fhe_context = Some(context);
                                state.decrypt_error = None;
                            }
                        }

                        Err(e) => {
                            let msg = format!("{e:#}");

                            if is_retryable_decrypt_error(&msg) {
                                let attempts = decrypt_retry_state
                                    .get(&output_hash)
                                    .map(|s| s.attempts.saturating_add(1))
                                    .unwrap_or(1);

                                let backoff = decrypt_retry_backoff(attempts);

                                let now = Instant::now();

                                decrypt_retry_state.insert(
                                    output_hash,
                                    DecryptRetryState {
                                        attempts,
                                        next_retry_at: now + backoff,
                                        last_error: msg.clone(),
                                    },
                                );

                                if let Ok(mut state) = mesh_alert.write() {
                                    state.latest_sequence = sequence;
                                    state.decrypt_error = Some(format!(
                                        "threshold decrypt not ready; will retry in {:?}; attempts={}; error={}",
                                        backoff,
                                        attempts,
                                        msg,
                                    ));
                                }
                            } else {
                                decrypted_failed_seen.insert(output_hash);
                                decrypt_retry_state.remove(&output_hash);

                                if let Ok(mut state) = mesh_alert.write() {
                                    state.latest_sequence = sequence;
                                    state.decrypt_error = Some(format!(
                                        "permanent threshold decrypt failure: {msg}"
                                    ));
                                }
                            }
                        }
                    }
                }

                Ok(None) => {
                    if let Ok(mut state) = mesh_alert.write() {
                        state.latest_sequence = sequence;
                    }
                }

                Err(e) => {
                    if let Ok(mut state) = mesh_alert.write() {
                        state.latest_sequence = sequence;
                        state.compute_error = Some(e.to_string());
                    }
                }
            }

            if decrypted_seen.len() > 512 {
                decrypted_seen.clear();
            }

            if decrypted_failed_seen.len() > 512 {
                decrypted_failed_seen.clear();
            }

            if decrypt_retry_state.len() > 512 {
                decrypt_retry_state.clear();
            }

            sleep(Duration::from_millis(500)).await;
        }
    });
}

fn core_client_config_path(args: &[String]) -> Result<PathBuf> {
    for i in 0..args.len() {
        if args[i] == "-f" || args[i] == "--file-conf" {
            let Some(path) = args.get(i + 1) else {
                return Err(anyhow!("missing value after {}", args[i]));
            };
            return Ok(PathBuf::from(path));
        }
    }

    Err(anyhow!(
        "simulation threshold decrypt requires core-client config path: -- -f <config.toml>"
    ))
}

fn current_kentr_key_id(keys_folder: &Path) -> Result<KeyId> {
    let key_id_hex = key_ops::unique_public_key_id_from_pub_folders(keys_folder)?
        .ok_or_else(|| {
            anyhow!(
                "cannot infer key id from {}/PUB-p*/PublicKey",
                keys_folder.display()
            )
        })?;

        
    KeyId::from_str(&key_id_hex)
        .with_context(|| format!("failed to parse KeyId from {key_id_hex}"))
}

fn blake3_hash32(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn output_hash(output: &AlertMathOutputV1) -> [u8; 32] {
    match encode(&output.encrypted_outputs) {
        Ok(bytes) => blake3_hash32(&bytes),
        Err(_) => blake3_hash32(&output.encrypted_outputs.exposed_ct),
    }
}

async fn _threshold_decrypt_u64(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    ciphertext: Vec<u8>,
) -> Result<u64> {
    let plaintexts = threshold_client
        .user_decrypt(
            key_id,
            vec![TypedCiphertext {
                ciphertext,
                fhe_type: FHE_UINT64,
                external_handle: vec![23u8; 32],
                ciphertext_format: CiphertextFormat::SmallExpanded as i32,
            }],
        )
        .await
        .map_err(|e| anyhow!("threshold user-decrypt u64 failed: {e}"))?;

    let first = plaintexts
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("threshold user-decrypt u64 returned no plaintexts"))?;

    typed_plaintext_to_u64(first)
}

fn typed_plaintext_to_u64(pt: TypedPlaintext) -> Result<u64> {
    if pt.fhe_type != FHE_UINT64 {
        return Err(anyhow!(
            "expected decrypted euint64 plaintext fhe_type={}, got {}",
            FHE_UINT64,
            pt.fhe_type
        ));
    }

    if pt.bytes.len() < 8 {
        return Err(anyhow!(
            "decrypted u64 expected at least 8 bytes, got {}",
            pt.bytes.len()
        ));
    }

    let mut buf = [0u8; 8];
    buf.copy_from_slice(&pt.bytes[..8]);

    Ok(u64::from_le_bytes(buf))
}

async fn threshold_decrypt_bool(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    ciphertext: Vec<u8>,
) -> Result<bool> {
    threshold_client
        .user_decrypt_bool(key_id, ciphertext, CiphertextFormat::SmallExpanded)
        .await
        .map_err(|e| anyhow!("threshold user-decrypt bool failed: {e}"))
}

async fn threshold_decrypt_u64_many(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    items: Vec<(String, Vec<u8>)>,
) -> Result<HashMap<String, u64>> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }

    let ciphertexts = items
        .iter()
        .map(|(_label, ciphertext)| TypedCiphertext {
            ciphertext: ciphertext.clone(),
            fhe_type: FHE_UINT64,
            external_handle: vec![23u8; 32],
            ciphertext_format: CiphertextFormat::SmallExpanded as i32,
        })
        .collect::<Vec<_>>();

    let plaintexts = threshold_client
        .user_decrypt(key_id, ciphertexts)
        .await
        .map_err(|e| anyhow!("threshold batch user-decrypt u64 failed: {e}"))?;

    if plaintexts.len() != items.len() {
        return Err(anyhow!(
            "threshold batch user-decrypt u64 length mismatch: requested={}, got={}",
            items.len(),
            plaintexts.len()
        ));
    }

    let mut out = HashMap::with_capacity(items.len());

    for ((label, _), pt) in items.into_iter().zip(plaintexts.into_iter()) {
        out.insert(label, typed_plaintext_to_u64(pt)?);
    }

    Ok(out)
}

async fn threshold_decrypt_bool_many(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    items: Vec<(String, Vec<u8>)>,
) -> Result<HashMap<String, bool>> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }

    let ciphertexts = items
        .iter()
        .map(|(_label, ciphertext)| TypedCiphertext {
            ciphertext: ciphertext.clone(),
            fhe_type: 0, // ebool
            external_handle: vec![23u8; 32],
            ciphertext_format: CiphertextFormat::SmallExpanded as i32,
        })
        .collect::<Vec<_>>();

    let plaintexts = threshold_client
        .user_decrypt(key_id, ciphertexts)
        .await
        .map_err(|e| anyhow!("threshold batch user-decrypt bool failed: {e}"))?;

    if plaintexts.len() != items.len() {
        return Err(anyhow!(
            "threshold batch user-decrypt bool length mismatch: requested={}, got={}",
            items.len(),
            plaintexts.len()
        ));
    }

    let mut out = HashMap::with_capacity(items.len());

    for ((label, _), pt) in items.into_iter().zip(plaintexts.into_iter()) {
        out.insert(label, typed_plaintext_to_bool(pt)?);
    }

    Ok(out)
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

fn projected_d2_to_km(d2: u64, scenario: &OsatconScenarioV1) -> f64 {
    (d2 as f64).sqrt() / scenario.coord_scale_per_km
}

async fn _decrypt_context_of_exposition(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    context: smart_program_alert::AlertMathEncryptedFieldsV1,
    distance_context: EncryptedDistanceBatchV1,
    scenario: &OsatconScenarioV1,
) -> Result<MeshFheAlertContext> {
    let convoy = decode_encrypted_convoy_payload_v1(&context.convoy_payload_ct)
        .context("decode context convoy payload")?;

    let satellite_batch = decode_encrypted_satellite_payload_v1(&context.satellite_payload_ct)
        .context("decode context satellite payload")?;

    let convoy_x = _threshold_decrypt_u64(threshold_client, key_id, convoy.x_ct)
        .await
        .context("decrypt context convoy x")?;

    let convoy_y = _threshold_decrypt_u64(threshold_client, key_id, convoy.y_ct)
        .await
        .context("decrypt context convoy y")?;

    let convoy_lon = decode_lon_projected_u64(convoy_x, scenario)
        .context("decode context convoy longitude")?;

    let convoy_lat = decode_lat_projected_u64(convoy_y, scenario)
        .context("decode context convoy latitude")?;

    let mut distance_by_sat = std::collections::HashMap::new();

    for point in distance_context.sats {
        let d2 = _threshold_decrypt_u64(threshold_client, key_id, point.d2_ct)
            .await
            .with_context(|| format!("decrypt satellite {} d2", point.sat_id))?;

        let within_scan = threshold_decrypt_bool(threshold_client, key_id, point.within_scan_ct)
            .await
            .with_context(|| format!("decrypt satellite {} within_scan", point.sat_id))?;

        distance_by_sat.insert(point.sat_id, (d2, within_scan));
    }

    let mut satellites = Vec::with_capacity(satellite_batch.sats.len());

    for sat in satellite_batch.sats {
        let Some((d2, within_scan)) = distance_by_sat.get(&sat.sat_id).copied() else {
            return Err(anyhow!(
                "missing distance_context entry for satellite {}",
                sat.sat_id
            ));
        };

        if !within_scan {
            continue;
        }

        let x = _threshold_decrypt_u64(threshold_client, key_id, sat.x_ct)
            .await
            .with_context(|| format!("decrypt context satellite {} x", sat.sat_id))?;

        let y = _threshold_decrypt_u64(threshold_client, key_id, sat.y_ct)
            .await
            .with_context(|| format!("decrypt context satellite {} y", sat.sat_id))?;

        let lon = decode_lon_projected_u64(x, scenario)
            .with_context(|| format!("decode context satellite {} longitude", sat.sat_id))?;

        let lat = decode_lat_projected_u64(y, scenario)
            .with_context(|| format!("decode context satellite {} latitude", sat.sat_id))?;

        satellites.push(MeshFheSatelliteContext {
            sat_id: sat.sat_id,
            lat,
            lon,
            d_km: projected_d2_to_km(d2, scenario),
        });
    }

    satellites.sort_by(|a, b| {
        a.d_km
            .partial_cmp(&b.d_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(MeshFheAlertContext {
        convoy_lat,
        convoy_lon,
        satellites,
    })
}

async fn decrypt_context_of_exposition_batch(
    threshold_client: &mut ThresholdFheClient,
    key_id: &KeyId,
    context: smart_program_alert::AlertMathEncryptedFieldsV1,
    distance_context: smart_program_alert::EncryptedDistanceBatchV1,
    scenario: &OsatconScenarioV1,
) -> Result<MeshFheAlertContext> {
    let convoy = decode_encrypted_convoy_payload_v1(&context.convoy_payload_ct)
        .context("decode context convoy payload")?;

    let satellite_batch = decode_encrypted_satellite_payload_v1(&context.satellite_payload_ct)
        .context("decode context satellite payload")?;

    let mut u64_items: Vec<(String, Vec<u8>)> = Vec::new();
    let mut bool_items: Vec<(String, Vec<u8>)> = Vec::new();

    u64_items.push(("convoy.x".into(), convoy.x_ct));
    u64_items.push(("convoy.y".into(), convoy.y_ct));

    for sat in &satellite_batch.sats {
        u64_items.push((format!("sat.{}.x", sat.sat_id), sat.x_ct.clone()));
        u64_items.push((format!("sat.{}.y", sat.sat_id), sat.y_ct.clone()));
    }

    for point in &distance_context.sats {
        u64_items.push((format!("dist.{}.d2", point.sat_id), point.d2_ct.clone()));
        bool_items.push((
            format!("dist.{}.within_scan", point.sat_id),
            point.within_scan_ct.clone(),
        ));
    }

    let u64_values = threshold_decrypt_u64_many(
        threshold_client,
        key_id,
        u64_items,
    )
    .await
    .context("batch decrypt u64 context")?;

    let bool_values = threshold_decrypt_bool_many(
        threshold_client,
        key_id,
        bool_items,
    )
    .await
    .context("batch decrypt bool context")?;

    let convoy_x = *u64_values
        .get("convoy.x")
        .ok_or_else(|| anyhow!("missing decrypted convoy.x"))?;

    let convoy_y = *u64_values
        .get("convoy.y")
        .ok_or_else(|| anyhow!("missing decrypted convoy.y"))?;

    let convoy_lon = decode_lon_projected_u64(convoy_x, scenario)
        .context("decode context convoy longitude")?;

    let convoy_lat = decode_lat_projected_u64(convoy_y, scenario)
        .context("decode context convoy latitude")?;

    let mut satellites = Vec::with_capacity(satellite_batch.sats.len());

    for sat in satellite_batch.sats {
        let scan_key = format!("dist.{}.within_scan", sat.sat_id);
        let within_scan = *bool_values
            .get(&scan_key)
            .ok_or_else(|| anyhow!("missing decrypted {}", scan_key))?;

        if !within_scan {
            continue;
        }

        let x_key = format!("sat.{}.x", sat.sat_id);
        let y_key = format!("sat.{}.y", sat.sat_id);
        let d2_key = format!("dist.{}.d2", sat.sat_id);

        let x = *u64_values
            .get(&x_key)
            .ok_or_else(|| anyhow!("missing decrypted {}", x_key))?;

        let y = *u64_values
            .get(&y_key)
            .ok_or_else(|| anyhow!("missing decrypted {}", y_key))?;

        let d2 = *u64_values
            .get(&d2_key)
            .ok_or_else(|| anyhow!("missing decrypted {}", d2_key))?;

        let lon = decode_lon_projected_u64(x, scenario)
            .with_context(|| format!("decode context satellite {} longitude", sat.sat_id))?;

        let lat = decode_lat_projected_u64(y, scenario)
            .with_context(|| format!("decode context satellite {} latitude", sat.sat_id))?;

        satellites.push(MeshFheSatelliteContext {
            sat_id: sat.sat_id,
            lat,
            lon,
            d_km: projected_d2_to_km(d2, scenario),
        });
    }

    satellites.sort_by(|a, b| {
        a.d_km
            .partial_cmp(&b.d_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(MeshFheAlertContext {
        convoy_lat,
        convoy_lon,
        satellites,
    })
}

