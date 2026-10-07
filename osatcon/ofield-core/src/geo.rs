use std::f64::consts::PI;

pub fn clamp_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0_f64;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlmb = (lon2 - lon1).to_radians();

    let a = (dphi / 2.0).sin().powi(2)
        + p1.cos() * p2.cos() * (dlmb / 2.0).sin().powi(2);

    2.0 * r * a.sqrt().asin()
}

pub fn lonlat_to_world_xy(lon_deg: f64, lat_deg: f64, map_w: f64, map_h: f64) -> (f64, f64) {
    let x = (lon_deg + 180.0) / 360.0 * map_w;
    let y = (90.0 - lat_deg) / 180.0 * map_h;
    (x, y)
}

pub fn bearing_rad(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dl = clamp_lon(lon2 - lon1).to_radians();

    let y = dl.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * dl.cos();
    y.atan2(x)
}

pub fn bearing_to_screen_angle(bearing: f64) -> f64 {
     // convert:
    // north-based bearing → east-based screen angle
    std::f64::consts::FRAC_PI_2 - bearing
}

#[allow(dead_code)]
pub fn wrap_pi(x: f64) -> f64 {
    (x + PI).rem_euclid(2.0 * PI) - PI
}