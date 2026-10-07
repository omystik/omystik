use chrono::{DateTime, Duration, Utc};

use crate::geo::{clamp_lon, haversine_km};

#[derive(Debug, Clone)]
pub struct ExposureSat {
    pub sat_id: i32,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

#[derive(Debug, Clone)]
pub struct ExposureHit {
    pub d_km: f64,
    pub sat: ExposureSat,
}

#[derive(Debug, Clone)]
pub struct ExposureResult {
    pub nearest: Option<ExposureSat>,
    pub min_d_km: Option<f64>,
    pub exposed: bool,
    pub sorted_sats: Vec<ExposureHit>,
}

#[derive(Debug, Clone)]
pub struct ExposureEngine {
    pub radius_km: f64,
    pub horizon_min: i64,
    pub step_s: i64,
    pub max_sats_scan: usize,
}

impl ExposureEngine {
    pub fn new(radius_km: f64, horizon_min: i64, step_s: i64, max_sats_scan: usize) -> Self {
        Self {
            radius_km,
            horizon_min,
            step_s,
            max_sats_scan,
        }
    }

    pub fn exposure_now(&self, convoy_lat: f64, convoy_lon: f64, sats: &[ExposureSat]) -> ExposureResult {
        let mut dists: Vec<ExposureHit> = sats.iter().map(|sp| {
            let d = haversine_km(convoy_lat, convoy_lon, sp.lat, sp.lon);
            ExposureHit { d_km: d, sat: sp.clone() }
        }).collect();

        dists.sort_by(|a, b| a.d_km.partial_cmp(&b.d_km).unwrap());

        if dists.is_empty() {
            return ExposureResult {
                nearest: None,
                min_d_km: None,
                exposed: false,
                sorted_sats: Vec::new(),
            };
        }

        let min_d = dists[0].d_km;
        let nearest = Some(dists[0].sat.clone());

        ExposureResult {
            nearest,
            min_d_km: Some(min_d),
            exposed: min_d <= self.radius_km,
            sorted_sats: dists,
        }
    }

    pub fn next_exposure_in_seconds<FPos, FSat, S>(
        &self,
        convoy_pos_at: FPos,
        convoy_start_time: DateTime<Utc>,
        now: DateTime<Utc>,
        sats: &[S],
        sat_llh_at_fn: FSat,
    ) -> Option<i64>
    where
        FPos: Fn(f64) -> anyhow::Result<(f64, f64)>,
        FSat: Fn(&S, DateTime<Utc>) -> anyhow::Result<(f64, f64, f64)>,
    {
        let end = now + Duration::minutes(self.horizon_min);
        let mut t = now;

        let scan_len = sats.len().min(self.max_sats_scan);
        let scan_sats = &sats[..scan_len];

        while t <= end {
            let t_s = (t - convoy_start_time).num_milliseconds() as f64 / 1000.0;
            let (clat, clon0) = convoy_pos_at(t_s).ok()?;
            let clon = clamp_lon(clon0);

            for s in scan_sats {
                let Ok((slat, slon0, _)) = sat_llh_at_fn(s, t) else { continue };
                let slon = clamp_lon(slon0);

                if haversine_km(clat, clon, slat, slon) <= self.radius_km {
                    return Some((t - now).num_seconds());
                }
            }

            t += Duration::seconds(self.step_s);
        }

        None
    }
}

pub fn fmt_countdown(sec: Option<i64>) -> String {
    match sec {
        None => "—".to_string(),
        Some(s) => {
            let s = s.max(0);
            let h = s / 3600;
            let m = (s % 3600) / 60;
            let ss = s % 60;

            if h > 0 {
                format!("{h}h {m:02}m {ss:02}s")
            } else {
                format!("{m}m {ss:02}s")
            }
        }
    }
}