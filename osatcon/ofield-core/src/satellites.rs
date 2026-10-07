use anyhow::{bail, Context, Result};
use chrono::{DateTime, Datelike, Timelike, Utc};
use sgp4::{Constants, Elements};
use std::f64::consts::PI;

use crate::geo::clamp_lon;
use crate::paths::assets_dir;

const WGS84_A: f64 = 6378.137;
const WGS84_F: f64 = 1.0 / 298.257223563;
const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F);

#[derive(Debug, Clone)]
pub struct TleSat {
    pub sat_id: i32,
    pub elements: Elements,
    pub constants: Constants,
    pub l1: String,
    pub l2: String,
}

#[derive(Debug, Clone)]
pub struct SatPosition {
    pub sat_id: i32,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub sat: TleSat,
}

#[derive(Debug, Clone)]
pub struct SatelliteCatalog {
    pub tle_path: String,
    pub leo_alt_min_km: f64,
    pub leo_alt_max_km: f64,
    pub sats: Vec<TleSat>,
}

impl Default for SatelliteCatalog {
    fn default() -> Self {
        Self {
            tle_path: assets_dir().join("weather.txt").display().to_string(),
            leo_alt_min_km: 300.0,
            leo_alt_max_km: 1500.0,
            sats: Vec::new(),
        }
    }
}

impl SatelliteCatalog {
    pub fn load(mut self) -> Result<Self> {
        let content = std::fs::read_to_string(&self.tle_path)
            .with_context(|| format!("reading TLE {}", self.tle_path))?;

        let lines: Vec<&str> = content.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let mut sats = Vec::new();
        let mut i = 0;

        while i + 1 < lines.len() {
            let l1 = lines[i];
            let l2 = lines[i + 1];

            if l1.starts_with("1 ") && l2.starts_with("2 ") {
                if let Ok(elements) = Elements::from_tle(None, l1.as_bytes(), l2.as_bytes()) {
                    if let Ok(constants) = Constants::from_elements(&elements) {
                        let sat_id = l1[2..7].trim().parse::<i32>().unwrap_or(0);
                        sats.push(TleSat {
                            sat_id,
                            elements,
                            constants,
                            l1: l1.to_string(),
                            l2: l2.to_string(),
                        });
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }

        if sats.is_empty() {
            bail!("No valid TLE pairs found in {}", self.tle_path);
        }

        self.sats = sats;
        Ok(self)
    }

    pub fn positions_at(&self, now: DateTime<Utc>, max_keep: usize) -> Vec<SatPosition> {
        let mut out = Vec::new();

        for s in &self.sats {
            if let Ok((lat, lon, alt)) = sat_llh_at(s, now) {
                if alt >= self.leo_alt_min_km && alt <= self.leo_alt_max_km {
                    out.push(SatPosition {
                        sat_id: s.sat_id,
                        lat,
                        lon: clamp_lon(lon),
                        alt,
                        sat: s.clone(),
                    });
                }
            }
            if out.len() >= max_keep {
                break;
            }
        }

        out
    }
}

pub fn sat_llh_at(sat: &TleSat, t: DateTime<Utc>) -> Result<(f64, f64, f64)> {
    let tle_time: DateTime<Utc> = sat.elements.datetime.and_utc();
    let minutes_since_epoch = t.signed_duration_since(tle_time).num_milliseconds() as f64 / 60000.0;
    let prediction = sat.constants.propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))?;
    let position = prediction.position;

    let jd_ut1 = julian_date(t);
    let r_ecef = teme_to_ecef([position[0], position[1], position[2]], jd_ut1);
    Ok(ecef_to_llh(r_ecef))
}

fn julian_date(t: DateTime<Utc>) -> f64 {
    let y = t.year();
    let m = t.month() as i32;
    let d = t.day() as i32;
    let hr = t.hour() as f64;
    let min = t.minute() as f64;
    let sec = t.second() as f64 + t.nanosecond() as f64 * 1e-9;

    let (yy, mm) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let a = (yy as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();

    (365.25 * (yy as f64 + 4716.0)).floor()
        + (30.6001 * (mm as f64 + 1.0)).floor()
        + d as f64
        + b
        - 1524.5
        + (hr + min / 60.0 + sec / 3600.0) / 24.0
}

fn gstime(jd_ut1: f64) -> f64 {
    let tut1 = (jd_ut1 - 2451545.0) / 36525.0;
    let gmst_sec = 67310.54841
        + (876600.0 * 3600.0 + 8640184.812866) * tut1
        + 0.093104 * tut1 * tut1
        - 6.2e-6 * tut1 * tut1 * tut1;

    (gmst_sec * (PI / 43200.0)).rem_euclid(2.0 * PI)
}

fn teme_to_ecef(r_teme_km: [f64; 3], jd_ut1: f64) -> [f64; 3] {
    let theta = gstime(jd_ut1);
    let (c, s) = (theta.cos(), theta.sin());
    let [x, y, z] = r_teme_km;
    [c * x + s * y, -s * x + c * y, z]
}

fn ecef_to_llh(r_ecef_km: [f64; 3]) -> (f64, f64, f64) {
    let [x, y, z] = r_ecef_km;
    let lon = y.atan2(x);
    let p = x.hypot(y);
    let mut lat = z.atan2(p * (1.0 - WGS84_E2));
    let mut alt = 0.0;

    for _ in 0..5 {
        let sin_lat = lat.sin();
        let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
        alt = p / lat.cos().max(1e-12) - n;
        lat = z.atan2(p * (1.0 - WGS84_E2 * (n / (n + alt))));
    }

    (lat.to_degrees(), lon.to_degrees(), alt)
}