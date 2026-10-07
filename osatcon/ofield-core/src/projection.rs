use anyhow::{anyhow, Result};

use crate::geo::clamp_lon;
use crate::sync_scenario::OsatconScenarioV1;

const KM_PER_DEG_LAT: f64 = 111.32;
const EARTH_HALF_SPAN_KM: f64 = 20_100.0;

pub fn encode_lat_projected_u64(
    lat_deg: f64,
    scenario: &OsatconScenarioV1,
) -> Result<u64> {
    if !(-90.0..=90.0).contains(&lat_deg) {
        return Err(anyhow!("latitude out of range: {lat_deg}"));
    }

    let y_km = lat_deg * KM_PER_DEG_LAT;
    encode_projected_km_u64(y_km, scenario)
}

pub fn encode_lon_projected_u64(
    lon_deg: f64,
    scenario: &OsatconScenarioV1,
) -> Result<u64> {
    let lon_deg = clamp_lon(lon_deg);
    let ref_lat_rad = scenario.projection_ref_lat_deg.to_radians();

    let x_km = lon_deg * KM_PER_DEG_LAT * ref_lat_rad.cos();

    encode_projected_km_u64(x_km, scenario)
}

pub fn encode_distance_threshold_sq_u64(
    radius_km: f64,
    scenario: &OsatconScenarioV1,
) -> Result<u64> {
    if radius_km <= 0.0 {
        return Err(anyhow!("radius_km must be > 0"));
    }

    let r = radius_km * scenario.coord_scale_per_km;
    let sq = r * r;

    if sq < 0.0 || sq > u64::MAX as f64 {
        return Err(anyhow!("encoded radius_sq out of u64 range: {sq}"));
    }

    Ok(sq.round() as u64)
}

fn encode_projected_km_u64(
    value_km: f64,
    scenario: &OsatconScenarioV1,
) -> Result<u64> {
    let shifted = value_km + EARTH_HALF_SPAN_KM;

    if shifted < 0.0 {
        return Err(anyhow!(
            "projected coordinate out of range: value_km={value_km}, shifted={shifted}"
        ));
    }

    let encoded = shifted * scenario.coord_scale_per_km;

    if encoded > u64::MAX as f64 {
        return Err(anyhow!("encoded coordinate exceeds u64 range: {encoded}"));
    }

    Ok(encoded.round() as u64)
}

pub fn decode_lat_projected_u64(
    encoded: u64,
    scenario: &OsatconScenarioV1,
) -> Result<f64> {
    let y_km = decode_projected_km(encoded, scenario);
    let lat_deg = y_km / KM_PER_DEG_LAT;

    if !(-90.0..=90.0).contains(&lat_deg) {
        return Err(anyhow!("decoded latitude out of range: {lat_deg}"));
    }

    Ok(lat_deg)
}

pub fn decode_lon_projected_u64(
    encoded: u64,
    scenario: &OsatconScenarioV1,
) -> Result<f64> {
    let x_km = decode_projected_km(encoded, scenario);
    let ref_lat_rad = scenario.projection_ref_lat_deg.to_radians();
    let denom = KM_PER_DEG_LAT * ref_lat_rad.cos();

    if denom.abs() < 1e-9 {
        return Err(anyhow!(
            "cannot decode longitude with projection_ref_lat_deg={} because cos(ref_lat) is too close to zero",
            scenario.projection_ref_lat_deg
        ));
    }

    Ok(clamp_lon(x_km / denom))
}

fn decode_projected_km(
    encoded: u64,
    scenario: &OsatconScenarioV1,
) -> f64 {
    (encoded as f64 / scenario.coord_scale_per_km) - EARTH_HALF_SPAN_KM
}