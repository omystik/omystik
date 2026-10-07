use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use node_autonomos::{
    AutonomosConfig,
    AutonomosClusterConfig,
    KeyType,
    KomvosManager,
    PeerId,
    YpoloConfig,
    YpoloClusterConfig,
    load_local_key,
};
use kryptografia::{
    FheEncryptor,
    TypedPlaintext,
    FHE_UINT64,
};
use ofield_core::geo::clamp_lon;
use ofield_core::satellites::{SatPosition, SatelliteCatalog};
use ofield_core::sync_scenario::{OsatconScenarioV1, session_id_from_code};
use ofield_core::projection::{encode_lat_projected_u64, encode_lon_projected_u64};

use smart_program_alert::{
    build_encrypted_satellite_payload_v1,
    push_satellite_payload_v1,
    EncryptedSatelliteBatchV1,
    EncryptedSatellitePointV1,
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, RwLock},
};
use tokio::time::{sleep, Duration};

mod sat_app;

#[derive(Debug, Parser)]
#[command(name = "edge-satellite-operator")]
#[command(about = "Osatcon satellite edge endpoint using Kryptografia for encryption and Mesh C for FHE compute")]
pub struct Args {
    /// Path to the Autonomos mesh config TOML
    #[arg(long)]
    pub mesh_a_cluster: PathBuf,

    /// Autonomos node id
    #[arg(long)]
    pub autonomos_id: u32,

    /// Path to the Ypolo mesh config TOML
    #[arg(long)]
    pub ypolo_cluster: PathBuf,

    /// Ypolo node id
    #[arg(long)]
    pub ypolo_id: u32,

    /// Shared session code used to derive the stable 32-byte session id.
    #[arg(long)]
    pub session_code: String,
}

#[derive(Debug, Clone)]
pub struct EdgeSatelliteUiState {
    pub sequence: u64,
    pub observed_unix_ms: u64,
    pub observed_at: Option<DateTime<Utc>>,
    pub sats: Vec<EdgeSatelliteUiPoint>,

    pub encrypt_ok: bool,
    pub build_ok: bool,
    pub last_push_ok: bool,
    pub last_error: Option<String>,
}

impl Default for EdgeSatelliteUiState {
    fn default() -> Self {
        Self {
            sequence: 0,
            observed_unix_ms: 0,
            observed_at: None,
            sats: Vec::new(),
            encrypt_ok: false,
            build_ok: false,
            last_push_ok: false,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgeSatelliteUiPoint {
    pub sat_id: String,
    pub lat: f64,
    pub lon: f64,
    pub alt_km: f64,
    pub x: u64,
    pub y: u64,
}

#[derive(Debug, Clone)]
pub struct SatelliteSampleBatch {
    pub observed_at: DateTime<Utc>,
    pub positions: Vec<SatPosition>,
}

fn load_autonomos_config(path: &Path, autonomos_id: u32) -> Result<AutonomosConfig> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;

    let cfg: AutonomosClusterConfig = toml::from_str(&data)
        .with_context(|| format!("parse TOML {}", path.display()))?;

    cfg.autonomos
        .into_iter()
        .find(|p| p.autonomos_id == autonomos_id)
        .with_context(|| format!("autonomos_id {} not found", autonomos_id))
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

// agnostic batch
// fn current_satellite_batch(
//     satcat: &SatelliteCatalog,
//     scenario: &OsatconScenarioV1,
// ) -> Result<SatelliteSampleBatch> {
//     let now = Utc::now();
//     let positions = satcat.positions_at(now, scenario.max_sats_scan);

//     Ok(SatelliteSampleBatch {
//         observed_at: now,
//         positions,
//     })
// }

// hormuz batch
const HORMUZ_PASSING_SAT_IDS: [i32; 20] = [
    28054, 59051, 41891, 57402, 43010,
    44387, 41887, 55974, 41888, 41884,
    35951, 58703, 58701, 56232, 58663,
    58648, 55976, 41885, 41886, 41890,
];

fn current_satellite_batch(
    satcat: &SatelliteCatalog,
    scenario: &OsatconScenarioV1,
) -> Result<SatelliteSampleBatch> {
    let now = Utc::now();

    // Ask for more than the UI/FHE scan count so filtering does not drop candidates.
    // weather.txt has 70 entries, so 256 is safely above the catalog size here.
    let all_positions = satcat.positions_at(now, 256);

    let mut positions = all_positions
        .into_iter()
        .filter(|sp| HORMUZ_PASSING_SAT_IDS.contains(&sp.sat_id))
        .collect::<Vec<_>>();

    positions.sort_by_key(|sp| {
        HORMUZ_PASSING_SAT_IDS
            .iter()
            .position(|id| *id == sp.sat_id)
            .unwrap_or(usize::MAX)
    });

    positions.truncate(scenario.max_sats_scan);

    if positions.is_empty() {
        return Err(anyhow!(
            "no Hormuz-passing satellites found in current TLE catalog"
        ));
    }

    Ok(SatelliteSampleBatch {
        observed_at: now,
        positions,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    let scenario_json = std::fs::read_to_string("data/scenario/scenario.json")
        .with_context(|| "read scenario file data/scenario/scenario.json")?;

    let scenario = OsatconScenarioV1::from_json_str(&scenario_json)
        .context("parse data/scenario/scenario.json")?;

    let ui_state = Arc::new(RwLock::new(EdgeSatelliteUiState::default()));

    let operator_ui_state = ui_state.clone();
    let operator_scenario = scenario.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for satellite operator");

        if let Err(e) = runtime.block_on(run_satellite_operator(
            args,
            operator_scenario,
            operator_ui_state,
        )) {
            eprintln!("satellite operator failed: {e:?}");
        }
    });

    sat_app::run(ui_state, scenario)?;

    Ok(())
}

async fn run_satellite_operator(
    args: Args,
    scenario: OsatconScenarioV1,
    ui_state: Arc<RwLock<EdgeSatelliteUiState>>,
) -> Result<()> {
    let session_id = session_id_from_code(&args.session_code);

    let autonomos_cfg =
        load_autonomos_config(&args.mesh_a_cluster, args.autonomos_id)?;

    let ypolo_cfg =
        load_ypolo_config(&args.ypolo_cluster, args.ypolo_id)?;

    let ypolo_peer = PeerId::from_str(&ypolo_cfg.peer_id)
        .context("invalid Ypolo peer_id")?;

    let namespace = autonomos_cfg
        .namespace
        .clone()
        .context("Autonomos namespace is missing")?;

    let namespace_static: &'static str = Box::leak(namespace.into_boxed_str());

    println!("Launching edge-satellite-operator as Autonomos endpoint");
    println!("Autonomos id={}", args.autonomos_id);
    println!("Ypolo id={} peer={}", args.ypolo_id, ypolo_peer);
    println!("Session id={}", hex::encode(session_id));

    let mut autonomos_node = KomvosManager::new(
        autonomos_cfg.secret_seed,
        None,
        None,
        None,
        None,
    )
    .await
    .map_err(boxed_err)
    .context("create Autonomos KomvosManager")?;

    sleep(Duration::from_secs(5)).await;

    autonomos_node
        .launch_inbound_request_task()
        .await
        .map_err(boxed_err)
        .context("launch Autonomos inbound task")?;

    let bootstrap_addr = autonomos_cfg
        .syndesmos_addr
        .context("missing Autonomos syndesmos_addr")?;

    let bootstrap_peer = autonomos_cfg
        .syndesmos_peer
        .context("missing Autonomos syndesmos_peer")?;

    autonomos_node
        .register_rendezvous(bootstrap_addr, bootstrap_peer, namespace_static)
        .await
        .map_err(boxed_err)
        .context("register Autonomos endpoint to rendezvous")?;

    sleep(Duration::from_secs(3)).await;

    autonomos_node
        .ask_key(None)
        .await
        .map_err(|e| anyhow!(e.to_string()))
        .context("ask PublicKey for Autonomos endpoint")?;

    autonomos_node
        .client
        .dial(ypolo_peer, ypolo_cfg.listen_address.clone())
        .await
        .context("dial Ypolo")?;

    let satcat = SatelliteCatalog::default().load()?;

    println!("Pi#2 Satellite Monitor");

    let public_key_path = load_local_key(Some(KeyType::PublicKey))
        .await
        .context("load local PublicKey for encrypting satellites coord")?;

    let encryptor = FheEncryptor::from_public_key_path(public_key_path)
        .await
        .context("load Autonomos local PublicKey")?;

    println!("encryptor ready");

    loop {
        let sequence = scenario.tick_index_now();

        let batch = current_satellite_batch(&satcat, &scenario)?;
        let observed_unix_ms = now_unix_ms();

        let ui_sats = satellite_batch_to_ui_points(&batch, &scenario)?;

        let encrypted_batch =
            match encrypt_satellite_batch(&encryptor, &batch, &scenario).await {
                Ok(v) => {
                    update_satellite_ui_state(
                        &ui_state,
                        sequence,
                        observed_unix_ms,
                        batch.observed_at,
                        ui_sats.clone(),
                        true,
                        false,
                        false,
                        None,
                    );

                    v
                }
                Err(e) => {
                    update_satellite_ui_state(
                        &ui_state,
                        sequence,
                        observed_unix_ms,
                        batch.observed_at,
                        ui_sats,
                        false,
                        false,
                        false,
                        Some(e.to_string()),
                    );

                    sleep(Duration::from_millis(scenario.tick_ms)).await;
                    continue;
                }
            };

        let satellite_payload_ct =
            match build_encrypted_satellite_payload_v1(encrypted_batch) {
                Ok(v) => {
                    update_satellite_ui_state(
                        &ui_state,
                        sequence,
                        observed_unix_ms,
                        batch.observed_at,
                        ui_sats.clone(),
                        true,
                        true,
                        false,
                        None,
                    );

                    v
                }
                Err(e) => {
                    update_satellite_ui_state(
                        &ui_state,
                        sequence,
                        observed_unix_ms,
                        batch.observed_at,
                        ui_sats,
                        true,
                        false,
                        false,
                        Some(e.to_string()),
                    );

                    sleep(Duration::from_millis(scenario.tick_ms)).await;
                    continue;
                }
            };

        match push_satellite_payload_v1(
            &mut autonomos_node.client,
            ypolo_peer,
            session_id,
            satellite_payload_ct,
            observed_unix_ms,
            sequence,
        )
        .await
        {
            Ok(_) => {
                update_satellite_ui_state(
                    &ui_state,
                    sequence,
                    observed_unix_ms,
                    batch.observed_at,
                    ui_sats,
                    true,
                    true,
                    true,
                    None,
                );
            }
            Err(e) => {
                update_satellite_ui_state(
                    &ui_state,
                    sequence,
                    observed_unix_ms,
                    batch.observed_at,
                    ui_sats,
                    true,
                    true,
                    false,
                    Some(e.to_string()),
                );
            }
        }

        sleep(Duration::from_millis(scenario.tick_ms)).await;
    }
}

async fn encrypt_satellite_batch(
    encryptor: &FheEncryptor,
    batch: &SatelliteSampleBatch,
    scenario: &OsatconScenarioV1,
) -> Result<EncryptedSatelliteBatchV1> {
    let mut sats = Vec::with_capacity(batch.positions.len());

    for sp in &batch.positions {
        let lon = clamp_lon(sp.lon);
        let lat = sp.lat;

        let x = encode_lon_projected_u64(lon, &scenario)?;
        let y = encode_lat_projected_u64(lat, &scenario)?;

        let x_ct = encrypt_u64(encryptor, x)
            .with_context(|| format!("encrypt satellite {} x", sp.sat_id))?;

        let y_ct = encrypt_u64(encryptor, y)
            .with_context(|| format!("encrypt satellite {} y", sp.sat_id))?;

        sats.push(EncryptedSatellitePointV1 {
            sat_id: sp.sat_id,
            x_ct,
            y_ct,
        });
    }

    Ok(EncryptedSatelliteBatchV1 { sats })
}

fn encrypt_u64(encryptor: &FheEncryptor, value: u64) -> Result<Vec<u8>> {
    let ct = encryptor.encrypt_typed_plaintext(TypedPlaintext {
        bytes: value.to_le_bytes().to_vec(),
        fhe_type: FHE_UINT64,
    })?;

    Ok(ct.ciphertext)
}

fn boxed_err(e: Box<dyn std::error::Error + Send + Sync>) -> anyhow::Error {
    anyhow!(e.to_string())
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn satellite_batch_to_ui_points(
    batch: &SatelliteSampleBatch,
    scenario: &OsatconScenarioV1,
) -> Result<Vec<EdgeSatelliteUiPoint>> {
    let mut sats = Vec::with_capacity(batch.positions.len());

    for sp in &batch.positions {
        let lon = clamp_lon(sp.lon);
        let lat = sp.lat;

        let x = encode_lon_projected_u64(lon, scenario)?;
        let y = encode_lat_projected_u64(lat, scenario)?;

        sats.push(EdgeSatelliteUiPoint {
            sat_id: sp.sat_id.to_string(),
            lat,
            lon,
            alt_km: sp.alt,
            x,
            y,
        });
    }

    Ok(sats)
}

fn update_satellite_ui_state(
    ui_state: &Arc<RwLock<EdgeSatelliteUiState>>,
    sequence: u64,
    observed_unix_ms: u64,
    observed_at: DateTime<Utc>,
    sats: Vec<EdgeSatelliteUiPoint>,
    encrypt_ok: bool,
    build_ok: bool,
    last_push_ok: bool,
    last_error: Option<String>,
) {
    if let Ok(mut ui) = ui_state.write() {
        ui.sequence = sequence;
        ui.observed_unix_ms = observed_unix_ms;
        ui.observed_at = Some(observed_at);
        ui.sats = sats;
        ui.encrypt_ok = encrypt_ok;
        ui.build_ok = build_ok;
        ui.last_push_ok = last_push_ok;
        ui.last_error = last_error;
    }
}