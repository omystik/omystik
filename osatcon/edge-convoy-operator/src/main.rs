mod convoy_app;
mod convoy_ui;

use anyhow::{anyhow, Context, Result};
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
use ofield_core::convoy::ConvoyRoute;
use ofield_core::geo::clamp_lon;
use ofield_core::sync_scenario::{OsatconScenarioV1, session_id_from_code};
use ofield_core::projection::{encode_lat_projected_u64, encode_lon_projected_u64};

use smart_program_alert::{
    build_encrypted_convoy_payload_v1,
    push_convoy_payload_v1,
    EncryptedConvoyPointV1,
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use std::sync::{Arc, RwLock};

use tokio::time::{sleep, Duration};

#[derive(Debug, Parser)]
#[command(name = "edge-convoy-operator")]
#[command(about = "Osatcon convoy edge endpoint using Kryptografia for encryption and Mesh C for FHE compute")]
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

#[derive(Debug, Clone, Default)]
pub struct EdgeConvoyUiState {
    pub sequence: u64,
    pub t_s: f64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub x: Option<u64>,
    pub y: Option<u64>,

    pub encrypt_ok: bool,
    pub build_ok: bool,
    pub last_push_ok: bool,

    pub last_error: Option<String>,
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

fn main() -> Result<()> {
    let args = Args::parse();

    let scenario_json = std::fs::read_to_string("data/scenario/scenario.json")
        .with_context(|| format!("read scenario file scenario"))?;
    
    let scenario = OsatconScenarioV1::from_json_str(&scenario_json)
        .context("parse --scenario-file")?;

    let session_id = session_id_from_code(&args.session_code);

    let convoy = ConvoyRoute::default().build_with_scenario(&scenario, &session_id)?;

    let ui_state = Arc::new(RwLock::new(EdgeConvoyUiState::default()));

    {
        let worker_args = args;
        let worker_scenario = scenario.clone();
        let worker_convoy = convoy.clone();
        let worker_ui_state = ui_state.clone();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    if let Ok(mut s) = worker_ui_state.write() {
                        s.last_error = Some(format!("create Tokio runtime: {e}"));
                    }
                    return;
                }
            };

            if let Err(e) = runtime.block_on(run_edge_convoy_operator(
                worker_args,
                worker_scenario,
                worker_convoy,
                worker_ui_state.clone(),
                session_id,
            )) {
                if let Ok(mut s) = worker_ui_state.write() {
                    s.last_error = Some(format!("{e:#}"));
                    s.last_push_ok = false;
                }
            }
        });
    }

    // Important: on macOS, eframe/winit must run on the main thread.
    convoy_app::run(ui_state, scenario, convoy)
}


async fn run_edge_convoy_operator(
    args: Args,
    scenario: OsatconScenarioV1,
    convoy: ConvoyRoute,
    ui_state: Arc<RwLock<EdgeConvoyUiState>>,
    session_id: [u8; 32],
) -> Result<()>  {
    
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

    println!("Launching edge-convoy-operator as Autonomos endpoint");
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


    println!("Pi#1 Convoy Operator");

    let corridor_name = convoy
        .corridor_meta
        .as_ref()
        .and_then(|m| m.raw.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");

    println!("Corridor: {}", corridor_name);

    let public_key_path = load_local_key(Some(KeyType::PublicKey))
        .await
        .context("load local PublicKey for encrypting convoy coord")?;

    let encryptor = FheEncryptor::from_public_key_path(public_key_path)
        .await
        .context("load Autonomos local PublicKey")?;

    println!("encryptor ready");


    loop {
        let sequence = scenario.tick_index_now();
        let observed_unix_ms = now_unix_ms();

        let t_s = convoy_route_t_s_now(&convoy);

        let (lat, lon0) = convoy.pos_at(t_s)?;
        let lon = clamp_lon(lon0);

        let x = encode_lon_projected_u64(lon, &scenario)?;
        let y = encode_lat_projected_u64(lat, &scenario)?;

        if let Ok(mut s) = ui_state.write() {
            s.sequence = sequence;
            s.t_s = t_s;
            s.lat = Some(lat);
            s.lon = Some(lon);
            s.x = Some(x);
            s.y = Some(y);
            s.encrypt_ok = false;
            s.build_ok = false;
            s.last_push_ok = false;
            s.last_error = None;
        }

        let x_ct = match encrypt_u64(&encryptor, x).context("encrypt convoy x") {
            Ok(ct) => ct,
            Err(e) => {
                let err = format!("{e:#}");
                eprintln!("convoy encryption failed: {err}");

                if let Ok(mut s) = ui_state.write() {
                    s.encrypt_ok = false;
                    s.build_ok = false;
                    s.last_push_ok = false;
                    s.last_error = Some(err);
                }

                sleep(Duration::from_millis(scenario.tick_ms)).await;
                continue;
            }
        };

        let y_ct = match encrypt_u64(&encryptor, y).context("encrypt convoy y") {
            Ok(ct) => ct,
            Err(e) => {
                let err = format!("{e:#}");
                eprintln!("convoy encryption failed: {err}");

                if let Ok(mut s) = ui_state.write() {
                    s.encrypt_ok = false;
                    s.build_ok = false;
                    s.last_push_ok = false;
                    s.last_error = Some(err);
                }

                sleep(Duration::from_millis(scenario.tick_ms)).await;
                continue;
            }
        };

        if let Ok(mut s) = ui_state.write() {
            s.encrypt_ok = true;
        }

        let encrypted_convoy = EncryptedConvoyPointV1 {
            x_ct,
            y_ct,
        };

        let convoy_payload_ct =
            match build_encrypted_convoy_payload_v1(encrypted_convoy)
                .context("build encrypted convoy payload")
            {
                Ok(payload) => payload,
                Err(e) => {
                    let err = format!("{e:#}");
                    eprintln!("convoy payload build failed: {err}");

                    if let Ok(mut s) = ui_state.write() {
                        s.build_ok = false;
                        s.last_push_ok = false;
                        s.last_error = Some(err);
                    }

                    sleep(Duration::from_millis(scenario.tick_ms)).await;
                    continue;
                }
            };

        if let Ok(mut s) = ui_state.write() {
            s.build_ok = true;
        }

        match push_convoy_payload_v1(
            &mut autonomos_node.client,
            ypolo_peer,
            session_id,
            convoy_payload_ct,
            observed_unix_ms,
            sequence,
        )
        .await
        .context("push convoy payload to Ypolo session")
        {
            Ok(()) => {
                if let Ok(mut s) = ui_state.write() {
                    s.last_push_ok = true;
                    s.last_error = None;
                }
            }

            Err(e) => {
                let err = format!("{e:#}");
                eprintln!("push convoy payload failed: {err}");

                if let Ok(mut s) = ui_state.write() {
                    s.last_push_ok = false;
                    s.last_error = Some(err);
                }
            }
        }

        println!(
            "seq={} t={:8.1}s lat={:+.5} lon={:+.5} x={} y={}",
            sequence,
            t_s,
            lat,
            lon,
            x,
            y,
        );

        sleep(Duration::from_millis(scenario.tick_ms)).await;
    }
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

fn convoy_route_t_s_now(convoy: &ConvoyRoute) -> f64 {
    let route_end_t_s = convoy
        .track
        .last()
        .map(|p| p.t)
        .unwrap_or(0.0);

    if route_end_t_s <= 0.0 {
        return 0.0;
    }

    let now_s = now_unix_ms() as f64 / 1000.0;
    now_s % route_end_t_s
}