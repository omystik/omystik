use chrono::{DateTime, Duration, Utc};

use crate::geo::{
    encode_distance_threshold_sq, squared_distance, FixedPoint2D, GeoReference,
};

/// TFHE-ready satellite candidate representation.
///
/// This is the structure the encrypted computation path should conceptually use:
/// satellite identity + fixed-point planar coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExposureSat {
    pub sat_id: i32,
    pub pos: FixedPoint2D,
}

/// TFHE-ready per-satellite distance candidate.
///
/// `d2` is the squared planar distance in fixed-point units.
/// It is intentionally integer-only and avoids sqrt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExposureHit {
    pub d2: i128,
    pub sat: ExposureSat,
}

/// TFHE-ready exposure result.
///
/// Notes:
/// - `min_d2` is squared distance, not kilometers
/// - `nearest_sat_id` avoids carrying extra floating-point state
/// - `sorted_sats` is retained only for cleartext/simulation compatibility;
///   do not require this in the encrypted SmartProgram output unless explicitly needed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureResult {
    pub nearest_sat_id: Option<i32>,
    pub min_d2: Option<i128>,
    pub exposed: bool,
    pub sorted_sats: Vec<ExposureHit>,
}

/// Integer-only exposure engine.
///
/// `radius_sq` is stored in squared fixed-point units and is directly comparable
/// with `squared_distance(...)`.
#[derive(Debug, Clone)]
pub struct ExposureEngine {
    pub radius_sq: i128,
    pub horizon_min: i64,
    pub step_s: i64,
    pub max_sats_scan: usize,
}

impl ExposureEngine {
    /// Construct from a cleartext radius in meters.
    ///
    /// This is the preferred constructor because edge-side/application config
    /// is naturally specified in metric units.
    pub fn new(radius_m: f64, horizon_min: i64, step_s: i64, max_sats_scan: usize) -> Self {
        Self {
            radius_sq: encode_distance_threshold_sq(radius_m),
            horizon_min,
            step_s,
            max_sats_scan,
        }
    }

    /// Construct directly from an already-encoded squared threshold.
    pub fn new_from_radius_sq(
        radius_sq: i128,
        horizon_min: i64,
        step_s: i64,
        max_sats_scan: usize,
    ) -> Self {
        Self {
            radius_sq,
            horizon_min,
            step_s,
            max_sats_scan,
        }
    }

    /// TFHE-compatible core proximity kernel over integer planar coordinates.
    ///
    /// This function:
    /// - uses only integer arithmetic
    /// - avoids trig / sqrt / float math
    /// - computes the minimum squared distance and exposure flag
    ///
    /// The sort is present only for cleartext compatibility/debugging.
    /// The encrypted SmartProgram should avoid materializing a full sorted list
    /// unless strictly necessary.
    pub fn exposure_now(&self, convoy_pos: FixedPoint2D, sats: &[ExposureSat]) -> ExposureResult {
        let mut dists: Vec<ExposureHit> = sats
            .iter()
            .map(|sp| ExposureHit {
                d2: squared_distance(convoy_pos, sp.pos),
                sat: *sp,
            })
            .collect();

        dists.sort_by(|a, b| a.d2.cmp(&b.d2));

        if dists.is_empty() {
            return ExposureResult {
                nearest_sat_id: None,
                min_d2: None,
                exposed: false,
                sorted_sats: Vec::new(),
            };
        }

        let min_d2 = dists[0].d2;
        let nearest_sat_id = Some(dists[0].sat.sat_id);

        ExposureResult {
            nearest_sat_id,
            min_d2: Some(min_d2),
            exposed: min_d2 <= self.radius_sq,
            sorted_sats: dists,
        }
    }

    /// Cleartext orchestration helper for future exposure scanning.
    ///
    /// Important:
    /// - trajectory propagation remains cleartext
    /// - lon/lat -> local planar encoding is done cleartext
    /// - the actual exposure predicate uses the same integer math contract
    ///   as the encrypted kernel (`squared_distance <= radius_sq`)
    ///
    /// This keeps simulation semantics aligned with the TFHE SmartProgram.
    pub fn next_exposure_in_seconds<FPos, FSat, S>(
        &self,
        geo_ref: GeoReference,
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
            let (clat, clon) = convoy_pos_at(t_s).ok()?;
            let (cx, cy) = geo_ref.lonlat_to_local_xy_fixed(clon, clat);
            let convoy_pos = FixedPoint2D::new(cx, cy);

            for s in scan_sats {
                let Ok((slat, slon, _alt)) = sat_llh_at_fn(s, t) else {
                    continue;
                };

                let (sx, sy) = geo_ref.lonlat_to_local_xy_fixed(slon, slat);
                let sat_pos = FixedPoint2D::new(sx, sy);

                if squared_distance(convoy_pos, sat_pos) <= self.radius_sq {
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