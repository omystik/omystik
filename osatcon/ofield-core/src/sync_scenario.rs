use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsatconScenarioV1 {
    pub scenario_id: String,
    pub start_unix_ms: i64,
    pub tick_ms: u64,
    pub alert_radius_km: f64,
    pub max_sats_scan: usize,

    /// Reference latitude used for local equirectangular longitude scaling.
    pub projection_ref_lat_deg: f64,

    /// Integer scale applied to projected kilometers.
    /// 1000.0 means 1 unit = 1 meter.
    pub coord_scale_per_km: f64,

    /// Convoy speed used by all Osatcon engines for this scenario.
    pub convoy_speed_kmh: f64,

    /// Convoy interpolation/sample step in seconds.
    pub convoy_dt_s: u32,

    /// How often the UI recomputes the plaintext future exposure forecast.
    pub recompute_every_s: i64,

    /// Plaintext forecast horizon in minutes.
    pub exposure_horizon_min: i64,

    /// Plaintext forecast scan step in seconds.
    pub exposure_step_s: i64,
}

impl OsatconScenarioV1 {
    pub fn generate_now() -> Self {
        let now = Utc::now();

        let seed = format!("osatcon-alert-session-v1|{}", now.timestamp_millis());
        let scenario_hash = stable_hash_hex(&seed);

        Self {
            scenario_id: format!("osatcon-alert-session-v1-{scenario_hash}"),
            start_unix_ms: now.timestamp_millis(),
            tick_ms: 3000,
            alert_radius_km: 500.0,
            max_sats_scan: 12,
            projection_ref_lat_deg: 0.0,
            coord_scale_per_km: 1_000.0,
            convoy_speed_kmh: 130.0,
            convoy_dt_s: 5,
            recompute_every_s: 10,
            exposure_horizon_min: 180,
            exposure_step_s: 10,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.scenario_id.trim().is_empty() {
            return Err(anyhow!("scenario_id must not be empty"));
        }

        if self.tick_ms == 0 {
            return Err(anyhow!("tick_ms must be > 0"));
        }

        if self.alert_radius_km <= 0.0 {
            return Err(anyhow!("alert_radius_km must be > 0"));
        }

        if self.max_sats_scan == 0 {
            return Err(anyhow!("max_sats_scan must be > 0"));
        }

        if !(-90.0..=90.0).contains(&self.projection_ref_lat_deg) {
            return Err(anyhow!(
                "projection_ref_lat_deg out of range: {}",
                self.projection_ref_lat_deg
            ));
        }

        if self.coord_scale_per_km <= 0.0 {
            return Err(anyhow!("coord_scale_per_km must be > 0"));
        }

        if self.convoy_speed_kmh <= 0.0 {
            return Err(anyhow!("convoy_speed_kmh must be > 0"));
        }

        if self.convoy_dt_s == 0 {
            return Err(anyhow!("convoy_dt_s must be > 0"));
        }

        if self.recompute_every_s <= 0 {
            return Err(anyhow!("recompute_every_s must be > 0"));
        }

        if self.exposure_horizon_min <= 0 {
            return Err(anyhow!("exposure_horizon_min must be > 0"));
        }

        if self.exposure_step_s <= 0 {
            return Err(anyhow!("exposure_step_s must be > 0"));
        }

        Ok(())
    }

    pub fn start_time(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.start_unix_ms)
            .single()
            .unwrap_or_else(Utc::now)
    }

    /// Raw elapsed convoy seconds from the scenario start.
    ///
    /// This is useful when a caller already has a chosen semantic time.
    pub fn convoy_t_s_at(&self, at: DateTime<Utc>) -> f64 {
        (at.timestamp_millis() - self.start_unix_ms) as f64 / 1000.0
    }

    /// Current scenario tick index based on wall-clock time.
    ///
    /// This is not a mesh/node clock protocol. It is only a deterministic
    /// data-engine convention: every process with the same scenario computes
    /// the same tick index for the same wall-clock instant.
    pub fn current_sequence(&self) -> u64 {
        self.tick_index_now()
    }

    /// Current scenario tick index.
    pub fn tick_index_now(&self) -> u64 {
        self.tick_index_at(Utc::now())
    }

    /// Scenario tick index for an explicit timestamp.
    pub fn tick_index_at(&self, at: DateTime<Utc>) -> u64 {
        let elapsed = at.timestamp_millis().saturating_sub(self.start_unix_ms);

        if elapsed <= 0 {
            0
        } else {
            (elapsed as u64) / self.tick_ms
        }
    }

    /// Unix timestamp in milliseconds for a scenario tick.
    pub fn tick_unix_ms(&self, tick_index: u64) -> i64 {
        self.start_unix_ms + tick_index.saturating_mul(self.tick_ms) as i64
    }

    /// UTC time for a scenario tick.
    pub fn tick_time(&self, tick_index: u64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.tick_unix_ms(tick_index))
            .single()
            .unwrap_or_else(Utc::now)
    }

    /// Current semantic tick time.
    ///
    /// Convoy, satellite, and simulation engines should use this when they
    /// need synchronized scenario data for the current tick.
    pub fn current_tick_time(&self) -> DateTime<Utc> {
        self.tick_time(self.tick_index_now())
    }

    /// Convoy elapsed seconds for a scenario tick.
    pub fn convoy_t_s_for_tick(&self, tick_index: u64) -> f64 {
        (self.tick_unix_ms(tick_index) - self.start_unix_ms) as f64 / 1000.0
    }

    /// Convenience: current convoy elapsed seconds at the current scenario tick.
    pub fn current_convoy_t_s(&self) -> f64 {
        self.convoy_t_s_for_tick(self.tick_index_now())
    }

    pub fn encode_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }

    pub fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(bytes)?;
        value.validate()?;
        Ok(value)
    }

    pub fn to_json_string(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json_str(s: &str) -> Result<Self> {
        let value: Self = serde_json::from_str(s)?;
        value.validate()?;
        Ok(value)
    }
    
    pub fn with_projection_ref_lat(mut self, lat_deg: f64) -> Result<Self> {
        if !(-90.0..=90.0).contains(&lat_deg) {
            return Err(anyhow!("projection reference latitude out of range: {lat_deg}"));
        }

        self.projection_ref_lat_deg = lat_deg;
        self.validate()?;
        Ok(self)
    }
}

fn stable_hash_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let out = h.finalize();
    hex::encode(&out[..8])
}

pub fn session_id_from_scenario_file(path: &Path) -> Result<[u8; 32]> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("invalid scenario file name: {}", path.display()))?;

    let Some(hex_id) = file_name.strip_suffix(".scenario.json") else {
        return Err(anyhow!(
            "scenario file name must end with .scenario.json: {}",
            file_name
        ));
    };

    parse_hex32(hex_id)
}

pub fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s)
        .with_context(|| format!("invalid hex32: {s}"))?;

    if bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "expected 32 bytes, got {} for {s}",
            bytes.len()
        ));
    }

    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn session_id_from_code(secret_code: &str) -> [u8; 32] {
    *blake3::hash(secret_code.trim().as_bytes()).as_bytes()
}