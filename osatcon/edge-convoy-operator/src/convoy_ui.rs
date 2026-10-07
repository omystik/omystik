use egui::{vec2, Color32, Rect};

#[derive(Debug, Clone)]
pub struct TrackDraw {
    pub lat: f64,
    pub lon: f64,
}

pub fn draw_arrow(
    painter: &egui::Painter,
    center: egui::Pos2,
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

        rot.push(egui::pos2(
            center.x + rx,
            center.y + ry,
        ));
    }

    painter.add(egui::Shape::convex_polygon(
        rot,
        color,
        egui::Stroke::NONE,
    ));
}

pub fn draw_text(
    painter: &egui::Painter,
    text: &str,
    pos: egui::Pos2,
    color: Color32,
    bg: Option<Color32>,
) {
    if let Some(bg) = bg {
        let approx_w = text.len() as f32 * 7.5;
        let rect = Rect::from_min_size(pos + vec2(-4.0, -2.0), vec2(approx_w + 8.0, 18.0));
        painter.rect_filled(rect, 3.0, bg);
    }

    painter.text(
        pos,
        egui::Align2::LEFT_TOP,
        text,
        egui::FontId::proportional(13.0),
        color,
    );
}