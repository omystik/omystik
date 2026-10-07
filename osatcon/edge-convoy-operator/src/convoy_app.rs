use anyhow::Result;
use eframe::{egui, App, Frame, NativeOptions};
use egui::{pos2, vec2, Color32, Rect, TextureHandle, ViewportBuilder};

use ofield_core::convoy::{ConvoyRoute, TrackPoint};
use ofield_core::geo::clamp_lon;

use ofield_core::sync_scenario::OsatconScenarioV1;

use std::sync::{Arc, RwLock};

use crate::convoy_ui::{draw_arrow, draw_text, TrackDraw};
use crate::EdgeConvoyUiState;

const ROUTE_BG_PATH: &str = "data/ui/topography.jpg";

pub fn run(
    ui_state: Arc<RwLock<EdgeConvoyUiState>>,
    scenario: OsatconScenarioV1,
    convoy: ConvoyRoute,
) -> Result<()> {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([800.0, 480.0])
            .with_fullscreen(true)
            .with_decorations(false)
            .with_title("ØMYSTIK - Edge Convoy Operator"),
        ..Default::default()
    };

    eframe::run_native(
        "ØMYSTIK - Edge Convoy Operator",
        options,
        Box::new(|cc| {
            Ok(Box::new(EdgeConvoyApp::new(
                cc,
                ui_state.clone(),
                scenario.clone(),
                convoy.clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

struct LoadedState {
    convoy: ConvoyRoute,
    route_bounds: RouteBounds,
    background_texture: TextureHandle,
}

#[derive(Debug, Clone, Copy)]
struct RouteBounds {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
}

pub struct EdgeConvoyApp {
    loaded: Option<LoadedState>,
    startup_error: Option<String>,
    ui_state: Arc<RwLock<EdgeConvoyUiState>>,
    scenario: OsatconScenarioV1,
    convoy: ConvoyRoute,
}

impl EdgeConvoyApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        ui_state: Arc<RwLock<EdgeConvoyUiState>>,
        scenario: OsatconScenarioV1,
        convoy: ConvoyRoute,
    ) -> Self {
        Self {
            loaded: None,
            startup_error: None,
            ui_state,
            scenario,
            convoy,
        }
    }

    fn try_load(&mut self, ctx: &egui::Context) -> Result<()> {
        let route_bounds = Self::route_bounds(&self.convoy.track);

        let bg_img = image::open(ROUTE_BG_PATH)?.to_rgba8();
        let bg_size = [bg_img.width() as usize, bg_img.height() as usize];

        let bg_color_image =
            egui::ColorImage::from_rgba_unmultiplied(bg_size, bg_img.as_raw());

        let background_texture =
            ctx.load_texture("edge-convoy-route-bg", bg_color_image, Default::default());

        self.loaded = Some(LoadedState {
            convoy: self.convoy.clone(),
            route_bounds,
            background_texture,
        });

        Ok(())
    }

    fn track_draws(track: &[TrackPoint]) -> Vec<TrackDraw> {
        track
            .iter()
            .map(|p| TrackDraw {
                lat: p.lat,
                lon: p.lon,
            })
            .collect()
    }

    fn route_bounds(track: &[TrackPoint]) -> RouteBounds {
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for p in track {
            min_lat = min_lat.min(p.lat);
            max_lat = max_lat.max(p.lat);

            let lon = clamp_lon(p.lon);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
        }

        if !min_lat.is_finite() || !max_lat.is_finite() || !min_lon.is_finite() || !max_lon.is_finite() {
            return RouteBounds {
                min_lat: -1.0,
                max_lat: 1.0,
                min_lon: -1.0,
                max_lon: 1.0,
            };
        }

        let lat_pad = ((max_lat - min_lat) * 0.12).max(0.002);
        let lon_pad = ((max_lon - min_lon) * 0.12).max(0.002);

        RouteBounds {
            min_lat: min_lat - lat_pad,
            max_lat: max_lat + lat_pad,
            min_lon: min_lon - lon_pad,
            max_lon: max_lon + lon_pad,
        }
    }

    fn route_pos_to_screen(bounds: RouteBounds, lat: f64, lon: f64, rect: Rect) -> egui::Pos2 {
        let lon = clamp_lon(lon);

        let lon_span = (bounds.max_lon - bounds.min_lon).abs().max(1e-9);
        let lat_span = (bounds.max_lat - bounds.min_lat).abs().max(1e-9);

        let x = ((lon - bounds.min_lon) / lon_span) as f32;
        let y = (1.0 - ((lat - bounds.min_lat) / lat_span) as f32).clamp(0.0, 1.0);

        pos2(
            rect.left() + x.clamp(0.0, 1.0) * rect.width(),
            rect.top() + y * rect.height(),
        )
    }

    fn convoy_heading_on_route_screen(
        convoy: &ConvoyRoute,
        t_s: f64,
        bounds: RouteBounds,
        route_rect: Rect,
    ) -> f32 {
        let (lat0, lon0) = convoy.pos_at(t_s).unwrap_or((0.0, 0.0));
        let (lat1, lon1) = convoy.pos_at(t_s + 1.0).unwrap_or((lat0, lon0));

        let p0 = Self::route_pos_to_screen(bounds, lat0, lon0, route_rect);
        let p1 = Self::route_pos_to_screen(bounds, lat1, lon1, route_rect);

        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;

        if dx.abs() < 0.001 && dy.abs() < 0.001 {
            return 0.0;
        }

        // Matches convoy_ui::draw_arrow(), where:
        // angle=0 points upward, and the tip vector is (sin(angle), -cos(angle)).
        dy.atan2(dx)
    }

    fn local_route_rect(container: Rect) -> Rect {
        let margin = 28.0;
        Rect::from_min_max(
            container.min + vec2(margin, margin),
            container.max - vec2(margin, margin),
        )
    }
}

impl App for EdgeConvoyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if self.loaded.is_none() && self.startup_error.is_none() {
            if let Err(e) = self.try_load(ctx) {
                self.startup_error = Some(e.to_string());
            }
        }

        if let Some(err) = &self.startup_error {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Edge convoy UI startup failed");
                ui.label(err);
            });
            return;
        }

        let Some(state) = self.loaded.as_ref() else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Loading edge convoy UI...");
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        };

        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        let snapshot = self
            .ui_state
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();

        let _convoy_track = Self::track_draws(&state.convoy.track);

        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let painter = ui.painter_at(available);

            painter.rect_filled(available, 0.0, Color32::from_rgb(8, 10, 12));

            let route_rect = Self::local_route_rect(available);

            let heading = Self::convoy_heading_on_route_screen(
                &state.convoy,
                snapshot.t_s,
                state.route_bounds,
                route_rect,
            );

            painter.image(
                state.background_texture.id(),
                route_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            // Slight dark overlay so the yellow route and text stay readable
            painter.rect_filled(
                route_rect,
                10.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            );

            painter.rect_stroke(
                route_rect,
                10.0,
                egui::Stroke::new(1.0, Color32::from_rgb(45, 50, 56)),
                egui::StrokeKind::Inside,
            );

            let mut route_points = Vec::with_capacity(state.convoy.track.len());

            for p in &state.convoy.track {
                route_points.push(Self::route_pos_to_screen(
                    state.route_bounds,
                    p.lat,
                    p.lon,
                    route_rect,
                ));
            }

            if route_points.len() >= 2 {
                painter.add(egui::Shape::line(
                    route_points,
                    egui::Stroke::new(4.0, Color32::from_rgb(92, 142, 255)),
                ));
            }

            if let (Some(lat), Some(lon)) = (snapshot.lat, snapshot.lon) {
                let convoy_pos = Self::route_pos_to_screen(
                    state.route_bounds,
                    lat,
                    lon,
                    route_rect,
                );

                draw_arrow(
                    &painter,
                    convoy_pos,
                    10.0,
                    Color32::from_rgb(239, 206, 94),
                    heading,
                );

                draw_text(
                    &painter,
                    "CONVOY",
                    convoy_pos + vec2(18.0, -12.0),
                    Color32::WHITE,
                    Some(Color32::BLACK),
                );
            }

            let corridor_name = state
                .convoy
                .corridor_meta
                .as_ref()
                .and_then(|m| m.raw.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("—");

            draw_text(
                &painter,
                 &format!("EDGE CONVOY ROUTE / {corridor_name}"),
                pos2(route_rect.left() + 10.0, route_rect.top() + 10.0),
                Color32::WHITE,
                Some(Color32::BLACK),
            );

            if let (Some(lat), Some(lon), Some(x), Some(y)) =
                (snapshot.lat, snapshot.lon, snapshot.x, snapshot.y)
            {
                draw_text(
                    &painter,
                    &format!(
                        "seq={} t={:8.1}s lat={:+.5} lon={:+.5}",
                        snapshot.sequence,
                        snapshot.t_s,
                        lat,
                        lon,
                    ),
                    pos2(route_rect.left() + 10.0, route_rect.top() + 34.0),
                    Color32::WHITE,
                    Some(Color32::BLACK),
                );

                draw_text(
                    &painter,
                    &format!("x={} y={}", x, y),
                    pos2(route_rect.left() + 10.0, route_rect.top() + 56.0),
                    Color32::LIGHT_GRAY,
                    Some(Color32::BLACK),
                );
            } else {
                draw_text(
                    &painter,
                    "Waiting for first local convoy position...",
                    pos2(route_rect.left() + 10.0, route_rect.top() + 34.0),
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

            draw_text(
                &painter,
                &status_text,
                pos2(route_rect.left() + 10.0, route_rect.top() + 78.0),
                status_color,
                Some(Color32::BLACK),
            );

            if let Some(err) = &snapshot.last_error {
                let short_err = if err.len() > 96 {
                    format!("{}...", &err[..96])
                } else {
                    err.clone()
                };

                draw_text(
                    &painter,
                    &format!("error={short_err}"),
                    pos2(route_rect.left() + 10.0, route_rect.top() + 100.0),
                    Color32::RED,
                    Some(Color32::BLACK),
                );
            }

            draw_text(
                &painter,
                &format!(
                    "speed={:.0}km/h route={:.1}km eta={:.0}min",
                    self.scenario.convoy_speed_kmh,
                    state.convoy.route_km,
                    state.convoy.eta_min,
                ),
                pos2(route_rect.left() + 10.0, route_rect.bottom() - 28.0),
                Color32::LIGHT_GRAY,
                Some(Color32::BLACK),
            );
        });
    }
}