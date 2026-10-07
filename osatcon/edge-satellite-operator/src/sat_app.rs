use anyhow::Result;
use eframe::{egui, App, Frame, NativeOptions};
use egui::{pos2, vec2, Color32, Rect, TextureHandle, ViewportBuilder};

use ofield_core::geo::clamp_lon;
use ofield_core::sync_scenario::OsatconScenarioV1;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use crate::{EdgeSatelliteUiPoint, EdgeSatelliteUiState};

const EARTH_BG_PATH: &str = "data/ui/earth.tiff";
const MAX_TRAIL_POINTS: usize = 30;

pub fn run(
    ui_state: Arc<RwLock<EdgeSatelliteUiState>>,
    scenario: OsatconScenarioV1,
) -> Result<()> {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_fullscreen(true)
            .with_decorations(false)
            .with_title("ØMYSTIK - Edge Satellite Operator"),
        ..Default::default()
    };

    eframe::run_native(
        "ØMYSTIK - Edge Satellite Operator",
        options,
        Box::new(|cc| {
            Ok(Box::new(EdgeSatelliteApp::new(
                cc,
                ui_state.clone(),
                scenario.clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

struct LoadedState {
    background_texture: TextureHandle,
}

pub struct EdgeSatelliteApp {
    loaded: Option<LoadedState>,
    startup_error: Option<String>,
    ui_state: Arc<RwLock<EdgeSatelliteUiState>>,
    scenario: OsatconScenarioV1,
    trails: HashMap<String, VecDeque<SatelliteDraw>>,
}

#[derive(Debug, Clone, Copy)]
struct SatelliteDraw {
    lat: f64,
    lon: f64,
}

impl EdgeSatelliteApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        ui_state: Arc<RwLock<EdgeSatelliteUiState>>,
        scenario: OsatconScenarioV1,
    ) -> Self {
        Self {
            loaded: None,
            startup_error: None,
            ui_state,
            scenario,
            trails: HashMap::new(),
        }
    }

    fn try_load(&mut self, ctx: &egui::Context) -> Result<()> {
        let bg_img = image::open(EARTH_BG_PATH)
            .map_err(|e| anyhow::anyhow!("failed to load {EARTH_BG_PATH}: {e}"))?
            .to_rgba8();

        let bg_size = [bg_img.width() as usize, bg_img.height() as usize];

        let bg_color_image =
            egui::ColorImage::from_rgba_unmultiplied(bg_size, bg_img.as_raw());

        let background_texture =
            ctx.load_texture("edge-satellite-earth-bg", bg_color_image, Default::default());

        self.loaded = Some(LoadedState {
            background_texture,
        });

        Ok(())
    }

    fn ingest_trails(&mut self, sats: &[EdgeSatelliteUiPoint]) {
        for sat in sats {
            let trail = self
                .trails
                .entry(sat.sat_id.clone())
                .or_insert_with(VecDeque::new);

            let next = SatelliteDraw {
                lat: sat.lat,
                lon: clamp_lon(sat.lon),
            };

            let should_push = trail
                .back()
                .map(|p| {
                    (p.lat - next.lat).abs() > 1e-7 ||
                    (p.lon - next.lon).abs() > 1e-7
                })
                .unwrap_or(true);

            if should_push {
                trail.push_back(next);
            }

            while trail.len() > MAX_TRAIL_POINTS {
                trail.pop_front();
            }
        }

        let live_ids: std::collections::HashSet<&str> =
            sats.iter().map(|s| s.sat_id.as_str()).collect();

        self.trails.retain(|sat_id, _| live_ids.contains(sat_id.as_str()));
    }

    fn earth_rect(container: Rect) -> Rect {
        let margin = 18.0;
        let available = Rect::from_min_max(
            container.min + vec2(margin, margin),
            container.max - vec2(margin, margin),
        );

        // Your earth.tiff is 16200x8100, so 2:1.
        let map_aspect = 2.0_f32;
        let available_aspect = available.width() / available.height().max(1.0);

        if available_aspect > map_aspect {
            // Screen is wider than the map. Fit by height, center horizontally.
            let height = available.height();
            let width = height * map_aspect;
            let x0 = available.center().x - width * 0.5;

            Rect::from_min_size(
                pos2(x0, available.top()),
                vec2(width, height),
            )
        } else {
            // Screen is taller/narrower than the map. Fit by width, center vertically.
            let width = available.width();
            let height = width / map_aspect;
            let y0 = available.center().y - height * 0.5;

            Rect::from_min_size(
                pos2(available.left(), y0),
                vec2(width, height),
            )
        }
    }

    fn lat_lon_to_screen(lat: f64, lon: f64, rect: Rect) -> egui::Pos2 {
        let lon = clamp_lon(lon);

        let x = ((lon + 180.0) / 360.0) as f32;
        let y = ((90.0 - lat) / 180.0) as f32;

        pos2(
            rect.left() + x.clamp(0.0, 1.0) * rect.width(),
            rect.top() + y.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn draw_text(
        painter: &egui::Painter,
        text: &str,
        pos: egui::Pos2,
        color: Color32,
        bg: Option<Color32>,
    ) {
        let font = egui::FontId::monospace(13.0);

        if let Some(bg) = bg {
            let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
            let rect = Rect::from_min_size(pos, galley.size())
                .expand2(vec2(4.0, 2.0));

            painter.rect_filled(rect, 3.0, bg);
            painter.galley(pos, galley, color);
        } else {
            painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                text,
                font,
                color,
            );
        }
    }

    fn draw_satellite_marker(
        painter: &egui::Painter,
        pos: egui::Pos2,
        color: Color32,
    ) {
        painter.circle_filled(pos, 4.5, color);

        painter.circle_stroke(
            pos,
            8.0,
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 220, 255, 120)),
        );

        painter.line_segment(
            [pos + vec2(-7.0, 0.0), pos + vec2(-12.0, 0.0)],
            egui::Stroke::new(1.0, color),
        );

        painter.line_segment(
            [pos + vec2(7.0, 0.0), pos + vec2(12.0, 0.0)],
            egui::Stroke::new(1.0, color),
        );
    }

    fn draw_trail(
        painter: &egui::Painter,
        trail: &VecDeque<SatelliteDraw>,
        earth_rect: Rect,
    ) {
        if trail.len() < 2 {
            return;
        }

        let points: Vec<egui::Pos2> = trail
            .iter()
            .map(|p| Self::lat_lon_to_screen(p.lat, p.lon, earth_rect))
            .collect();

        for pair in points.windows(2) {
            painter.line_segment(
                [pair[0], pair[1]],
                egui::Stroke::new(
                    1.5,
                    Color32::from_rgba_unmultiplied(90, 190, 255, 120),
                ),
            );
        }
    }
}

impl App for EdgeSatelliteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if self.loaded.is_none() && self.startup_error.is_none() {
            if let Err(e) = self.try_load(ctx) {
                self.startup_error = Some(e.to_string());
            }
        }

        if let Some(err) = &self.startup_error {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Edge satellite UI startup failed");
                ui.label(err);
            });
            return;
        }

        let snapshot = self
            .ui_state
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();

        self.ingest_trails(&snapshot.sats);

        let Some(state) = self.loaded.as_ref() else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Loading edge satellite UI...");
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        };

        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let painter = ui.painter_at(available);

            painter.rect_filled(available, 0.0, Color32::from_rgb(5, 7, 12));

            let earth_rect = Self::earth_rect(available);

            painter.image(
                state.background_texture.id(),
                earth_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            painter.rect_filled(
                earth_rect,
                10.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            );

            painter.rect_stroke(
                earth_rect,
                10.0,
                egui::Stroke::new(1.0, Color32::from_rgb(45, 50, 56)),
                egui::StrokeKind::Inside,
            );

            for trail in self.trails.values() {
                Self::draw_trail(&painter, trail, earth_rect);
            }

            for sat in &snapshot.sats {
                let pos = Self::lat_lon_to_screen(sat.lat, sat.lon, earth_rect);

                Self::draw_satellite_marker(
                    &painter,
                    pos,
                    Color32::from_rgb(88, 220, 255),
                );

                Self::draw_text(
                    &painter,
                    &sat.sat_id,
                    pos + vec2(10.0, -10.0),
                    Color32::WHITE,
                    Some(Color32::from_rgba_unmultiplied(0, 0, 0, 160)),
                );
            }

            Self::draw_text(
                &painter,
                "EDGE SATELLITE OPERATOR / LIVE SCANNED BATCH",
                pos2(earth_rect.left() + 10.0, earth_rect.top() + 10.0),
                Color32::WHITE,
                Some(Color32::BLACK),
            );

            if let Some(observed_at) = snapshot.observed_at {
                Self::draw_text(
                    &painter,
                    &format!(
                        "seq={} observed={} sats={}/{} tick={}ms",
                        snapshot.sequence,
                        observed_at.to_rfc3339(),
                        snapshot.sats.len(),
                        self.scenario.max_sats_scan,
                        self.scenario.tick_ms,
                    ),
                    pos2(earth_rect.left() + 10.0, earth_rect.top() + 34.0),
                    Color32::WHITE,
                    Some(Color32::BLACK),
                );

                Self::draw_text(
                    &painter,
                    &format!("observed_unix_ms={}", snapshot.observed_unix_ms),
                    pos2(earth_rect.left() + 10.0, earth_rect.top() + 56.0),
                    Color32::LIGHT_GRAY,
                    Some(Color32::BLACK),
                );
            } else {
                Self::draw_text(
                    &painter,
                    "Waiting for first local satellite batch...",
                    pos2(earth_rect.left() + 10.0, earth_rect.top() + 34.0),
                    Color32::GRAY,
                    Some(Color32::BLACK),
                );
            }

            let status_text = format!(
                "encrypt={} build={} push={}",
                if snapshot.encrypt_ok { "ok" } else { "wait/error" },
                if snapshot.build_ok { "ok" } else { "wait/error" },
                if snapshot.last_push_ok { "ok" } else { "wait/error" },
            );

            let status_color =
                if snapshot.encrypt_ok && snapshot.build_ok && snapshot.last_push_ok {
                    Color32::from_rgb(68, 215, 168)
                } else {
                    Color32::ORANGE
                };

            Self::draw_text(
                &painter,
                &status_text,
                pos2(earth_rect.left() + 10.0, earth_rect.top() + 78.0),
                status_color,
                Some(Color32::BLACK),
            );

            if let Some(err) = &snapshot.last_error {
                let short_err = if err.len() > 96 {
                    format!("{}...", &err[..96])
                } else {
                    err.clone()
                };

                Self::draw_text(
                    &painter,
                    &format!("error={short_err}"),
                    pos2(earth_rect.left() + 10.0, earth_rect.top() + 100.0),
                    Color32::RED,
                    Some(Color32::BLACK),
                );
            }

            let mut y = earth_rect.bottom() - 28.0;

            for sat in snapshot.sats.iter().take(6).rev() {
                Self::draw_text(
                    &painter,
                    &format!(
                        "{} lat={:+.3} lon={:+.3} alt={:.0}km x={} y={}",
                        sat.sat_id,
                        sat.lat,
                        sat.lon,
                        sat.alt_km,
                        sat.x,
                        sat.y,
                    ),
                    pos2(earth_rect.left() + 10.0, y),
                    Color32::LIGHT_GRAY,
                    Some(Color32::BLACK),
                );

                y -= 22.0;
            }
        });
    }
}