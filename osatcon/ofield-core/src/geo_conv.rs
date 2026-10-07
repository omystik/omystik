use std::f64::consts::PI;

/// Mean Earth radius in meters.
/// Cleartext-only constant used during edge-side coordinate projection.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Fixed-point scale for projected local coordinates.
///
/// 1 unit = 1 centimeter if source meters are multiplied by 100.
/// This gives good proximity precision while keeping values well within i64
/// for regional operating areas.
pub const XY_SCALE: i64 = 100;

/// Fixed-point scale for normalized world/screen coordinates.
pub const SCREEN_SCALE: i64 = 1_000_000;

/// Cleartext-only longitude normalization to [-180, 180).
pub fn clamp_lon_deg(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

/// Cleartext-only wrap to [-PI, PI).
#[allow(dead_code)]
pub fn wrap_pi(x: f64) -> f64 {
    (x + PI).rem_euclid(2.0 * PI) - PI
}

/// Reference frame for local tangent-plane projection.
///
/// This is intended to be chosen in cleartext and shared as configuration.
/// The encrypted domain should only see already-projected integer coordinates.
#[derive(Debug, Clone, Copy)]
pub struct GeoReference {
    pub lat0_deg: f64,
    pub lon0_deg: f64,
}

impl GeoReference {
    /// Project lon/lat to a local equirectangular tangent plane in meters.
    ///
    /// Cleartext-only:
    /// - uses floating point
    /// - uses cosine
    ///
    /// Output convention:
    /// - x: east-west meters
    /// - y: north-south meters
    pub fn lonlat_to_local_xy_m(&self, lon_deg: f64, lat_deg: f64) -> (f64, f64) {
        let lat0_rad = self.lat0_deg.to_radians();
        let dlon_rad = clamp_lon_deg(lon_deg - self.lon0_deg).to_radians();
        let dlat_rad = (lat_deg - self.lat0_deg).to_radians();

        let x_m = EARTH_RADIUS_M * dlon_rad * lat0_rad.cos();
        let y_m = EARTH_RADIUS_M * dlat_rad;

        (x_m, y_m)
    }

    /// Project lon/lat to fixed-point local coordinates.
    ///
    /// This is the primary edge-side encoding function to use before TFHE encryption.
    /// The result is integer-only and suitable for encrypted arithmetic.
    pub fn lonlat_to_local_xy_fixed(&self, lon_deg: f64, lat_deg: f64) -> (i64, i64) {
        let (x_m, y_m) = self.lonlat_to_local_xy_m(lon_deg, lat_deg);
        (
            round_f64_to_i64(x_m * XY_SCALE as f64),
            round_f64_to_i64(y_m * XY_SCALE as f64),
        )
    }
}

/// Integer planar point suitable for encrypted-domain computations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPoint2D {
    pub x: i64,
    pub y: i64,
}

impl FixedPoint2D {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

/// Integer-only delta between two planar points.
/// This is TFHE-friendly.
pub fn delta_xy(a: FixedPoint2D, b: FixedPoint2D) -> (i64, i64) {
    (b.x - a.x, b.y - a.y)
}

/// Integer-only squared Euclidean distance.
///
/// This is the preferred TFHE kernel primitive.
/// Use this instead of true distance to avoid sqrt.
///
/// Returns i128 to reduce overflow risk from the squares.
pub fn squared_distance(a: FixedPoint2D, b: FixedPoint2D) -> i128 {
    let dx = (b.x as i128) - (a.x as i128);
    let dy = (b.y as i128) - (a.y as i128);
    dx * dx + dy * dy
}

/// Encode a distance threshold expressed in meters into squared fixed-point units.
///
/// Example:
/// - threshold_m = 5_000.0
/// - output can be compared directly against `squared_distance(...)`
pub fn encode_distance_threshold_sq(threshold_m: f64) -> i128 {
    let scaled = round_f64_to_i64(threshold_m * XY_SCALE as f64) as i128;
    scaled * scaled
}

/// Cleartext-only helper for UI/simulation projection.
///
/// This should not be used inside the encrypted SmartProgram path.
/// It remains useful for visualization and downstream simulation.
pub fn lonlat_to_world_xy_fixed(
    lon_deg: f64,
    lat_deg: f64,
    map_w: i64,
    map_h: i64,
) -> FixedPoint2D {
    let x = ((lon_deg + 180.0) / 360.0) * map_w as f64;
    let y = ((90.0 - lat_deg) / 180.0) * map_h as f64;

    FixedPoint2D {
        x: round_f64_to_i64(x * SCREEN_SCALE as f64),
        y: round_f64_to_i64(y * SCREEN_SCALE as f64),
    }
}

/// Cleartext-only approximate great-circle distance for debugging, validation,
/// or simulation baselines.
///
/// DO NOT call from the encrypted computation path.
#[allow(dead_code)]
pub fn haversine_km_clear(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0_f64;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlmb = (lon2 - lon1).to_radians();

    let a = (dphi / 2.0).sin().powi(2)
        + p1.cos() * p2.cos() * (dlmb / 2.0).sin().powi(2);

    2.0 * r * a.sqrt().asin()
}

/// Cleartext-only bearing helper retained only for validation or simulation.
///
/// DO NOT use in TFHE execution.
#[allow(dead_code)]
pub fn bearing_rad_clear(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dl = clamp_lon_deg(lon2 - lon1).to_radians();

    let y = dl.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * dl.cos();
    y.atan2(x)
}

/// Cleartext-only conversion from north-based bearing to screen angle.
///
/// DO NOT use in TFHE execution.
#[allow(dead_code)]
pub fn bearing_to_screen_angle_clear(bearing: f64) -> f64 {
    std::f64::consts::FRAC_PI_2 - bearing
}

#[inline]
fn round_f64_to_i64(v: f64) -> i64 {
    // Saturating cast behavior for safety.
    if v.is_nan() {
        0
    } else if v >= i64::MAX as f64 {
        i64::MAX
    } else if v <= i64::MIN as f64 {
        i64::MIN
    } else {
        v.round() as i64
    }
}