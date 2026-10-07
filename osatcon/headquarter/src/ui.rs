use egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
};

use ofield_core::geo::{clamp_lon, lonlat_to_world_xy};

#[derive(Debug, Clone, Copy)]
pub struct SatDraw {
    pub sat_id: i32,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub d_km: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TrackDraw {
    pub lat: f64,
    pub lon: f64,
}

pub fn draw_arrow(
    painter: &Painter,
    center: Pos2,
    size: f32,
    color: Color32,
    heading_rad: f32,
) {
    let pts = [
        vec2(size, 0.0),
        vec2(-size * 0.9, -size * 0.95),
        vec2(-size * 0.5, 0.0),
        vec2(-size * 0.9, size * 0.95),
    ];

    let c = heading_rad.cos();
    let s = heading_rad.sin();

    let mut rot = Vec::with_capacity(pts.len());
    for p in pts {
        let rx = p.x * c - p.y * s;
        let ry = p.x * s + p.y * c;
        rot.push(pos2(center.x + rx, center.y + ry));
    }

    painter.add(Shape::convex_polygon(rot, color, Stroke::NONE));
}

pub fn draw_text(
    painter: &Painter,
    text: impl Into<String>,
    pos: Pos2,
    fg: Color32,
    bg: Option<Color32>,
) {
    let text = text.into();
    if let Some(bg) = bg {
        let galley = painter.layout_no_wrap(text.clone(), FontId::monospace(12.0), fg);
        let rect = Rect::from_min_size(pos, galley.size() + vec2(4.0, 4.0));
        painter.rect_filled(
            rect,
            2.0,
            Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 180),
        );
        painter.galley(pos + vec2(2.0, 2.0), galley, fg);
    } else {
        painter.text(pos, Align2::LEFT_TOP, text, FontId::monospace(12.0), fg);
    }
}

pub fn red_to_blue_gradient(n: usize) -> Vec<Color32> {
    if n <= 1 {
        return vec![Color32::from_rgb(255, 0, 0)];
    }

    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            let r = (255.0 * (1.0 - t)) as u8;
            let b = (255.0 * t) as u8;
            Color32::from_rgb(r, 0, b)
        })
        .collect()
}

fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

fn compute_cam_rect_centered(
    convoy_lon: f64,
    convoy_lat: f64,
    map_w: f32,
    map_h: f32,
    zoom_w_px: f32,
    zoom_h_px: f32,
) -> Rect {
    let (wx, wy) =
        lonlat_to_world_xy(clamp_lon(convoy_lon), convoy_lat, map_w as f64, map_h as f64);
    let center = pos2(wx as f32, wy as f32);
    Rect::from_center_size(center, vec2(zoom_w_px, zoom_h_px))
}

fn clamp_cam_rect(cam: Rect, map_w: f32, map_h: f32) -> Rect {
    let x = clamp_f32(cam.left(), 0.0, (map_w - cam.width()).max(0.0));
    let y = clamp_f32(cam.top(), 0.0, (map_h - cam.height()).max(0.0));
    Rect::from_min_size(pos2(x, y), cam.size())
}

fn lon_relative_to_center(lon: f64, center_lon: f64) -> f64 {
    center_lon + clamp_lon(lon - center_lon)
}

fn add_split_polyline(
    painter: &Painter,
    pts: impl IntoIterator<Item = Pos2>,
    stroke: Stroke,
    jump_x: f32,
    jump_y: f32,
) {
    let mut seg: Vec<Pos2> = Vec::new();
    let mut prev: Option<Pos2> = None;

    for p in pts {
        let jump = prev
            .map(|pp| (p.x - pp.x).abs() > jump_x || (p.y - pp.y).abs() > jump_y)
            .unwrap_or(false);

        if jump {
            if seg.len() >= 2 {
                painter.add(Shape::line(std::mem::take(&mut seg), stroke));
            } else {
                seg.clear();
            }
        }

        seg.push(p);
        prev = Some(p);
    }

    if seg.len() >= 2 {
        painter.add(Shape::line(seg, stroke));
    }
}

fn draw_geo_radius_ring(
    painter: &Painter,
    center_lat: f64,
    center_lon: f64,
    radius_km: f64,
    project: impl Fn(f64, f64) -> Pos2,
    stroke: Stroke,
    jump_x: f32,
    jump_y: f32,
) {
    let earth_r_km = 6371.0_f64;
    let ang = radius_km / earth_r_km;

    let lat1 = center_lat.to_radians();
    let lon1 = center_lon.to_radians();

    let pts = (0..=72).map(|i| {
        let brg = (i as f64) * std::f64::consts::TAU / 72.0;

        let lat2 = (lat1.sin() * ang.cos() + lat1.cos() * ang.sin() * brg.cos()).asin();

        let lon2 = lon1
            + (brg.sin() * ang.sin() * lat1.cos())
                .atan2(ang.cos() - lat1.sin() * lat2.sin());

        project(lat2.to_degrees(), clamp_lon(lon2.to_degrees()))
    });

    add_split_polyline(painter, pts, stroke, jump_x, jump_y);
}

pub struct WorldInsetParams<'a> {
    pub inset_rect: Rect,
    pub center_lat: f64,
    pub center_lon: f64,
    pub map_w: f32,
    pub map_h: f32,
    pub zoom_w_frac: f32,
    pub zoom_h_frac: f32,
    pub convoy_track: &'a [TrackDraw],
    pub convoy_lat: f64,
    pub convoy_lon: f64,
    pub heading_rad: f32,
    pub sats: &'a [SatDraw],
    pub sat_colors: &'a [Color32],
    pub max_sats: usize,
    pub radius_km: Option<f64>,
}

pub fn draw_world_zoom_inset(
    painter: &Painter,
    texture: egui::TextureId,
    params: WorldInsetParams<'_>,
    exposed: bool,
) {
    let inset_rect = params.inset_rect;
    painter.rect_filled(inset_rect, 0.0, Color32::from_rgb(10, 10, 10));
    painter.rect_stroke(
        inset_rect,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(90, 90, 90)),
        StrokeKind::Outside,
    );

    let inset_aspect = inset_rect.width() / inset_rect.height().max(1.0);
    let mut base_w = (params.map_w * params.zoom_w_frac).max(64.0);
    let mut base_h = (params.map_h * params.zoom_h_frac).max(64.0);

    if (base_w / base_h) > inset_aspect {
        base_w = base_h * inset_aspect;
    } else {
        base_h = base_w / inset_aspect;
    }

    let cam_rect = clamp_cam_rect(
        compute_cam_rect_centered(
            params.center_lon,
            params.center_lat,
            params.map_w,
            params.map_h,
            base_w,
            base_h,
        ),
        params.map_w,
        params.map_h,
    );

    painter.image(
        texture,
        inset_rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    let w2i = |wx: f64, wy: f64| -> Pos2 {
        let sx = inset_rect.left()
            + ((wx as f32 - cam_rect.left()) * inset_rect.width() / cam_rect.width());
        let sy = inset_rect.top()
            + ((wy as f32 - cam_rect.top()) * inset_rect.height() / cam_rect.height());
        pos2(sx, sy)
    };

    let clip = painter.with_clip_rect(inset_rect);

    if !params.convoy_track.is_empty() {
        let pts = params.convoy_track.iter().map(|p| {
            let lon = clamp_lon(p.lon);
            let (wx, wy) = lonlat_to_world_xy(lon, p.lat, params.map_w as f64, params.map_h as f64);
            w2i(wx, wy)
        });

        add_split_polyline(
            &clip,
            pts,
            Stroke::new(2.0, Color32::from_rgb(180, 180, 255)),
            inset_rect.width() * 0.5,
            inset_rect.height() * 0.5,
        );
    }

    let color_exposed = if exposed {
        Color32::from_rgb(255, 165, 0)
    } else {
        Color32::from_rgb(0, 128, 255)
    };

    if let Some(radius_km) = params.radius_km {
        draw_geo_radius_ring(
            &clip,
            params.convoy_lat,
            params.convoy_lon,
            radius_km,
            |lat, lon| {
                let (wx, wy) = lonlat_to_world_xy(
                    clamp_lon(lon),
                    lat,
                    params.map_w as f64,
                    params.map_h as f64,
                );
                w2i(wx, wy)
            },
            Stroke::new(1.0, color_exposed),
            inset_rect.width() * 0.5,
            inset_rect.height() * 0.5,
        );
    }

    for (i, sp) in params.sats.iter().take(params.max_sats).enumerate() {
        let col = params.sat_colors.get(i).copied().unwrap_or(Color32::LIGHT_GRAY);
        let (wx, wy) =
            lonlat_to_world_xy(clamp_lon(sp.lon), sp.lat, params.map_w as f64, params.map_h as f64);
        let p = w2i(wx, wy);
        if inset_rect.contains(p) {
            clip.circle_filled(p, 4.0, col);
        }
    }

    let (wxc, wyc) = lonlat_to_world_xy(
        clamp_lon(params.convoy_lon),
        params.convoy_lat,
        params.map_w as f64,
        params.map_h as f64,
    );
    let c = w2i(wxc, wyc);
    draw_arrow(&clip, c, 9.0, Color32::from_rgb(239, 206, 94), params.heading_rad);
}