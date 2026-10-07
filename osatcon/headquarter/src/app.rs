use anyhow::Result;
use eframe::{egui, App, Frame, NativeOptions};
use egui::{pos2, vec2, Color32, Rect, RichText, TextureHandle, ViewportBuilder};

use ofield_core::geo::{bearing_rad, bearing_to_screen_angle, clamp_lon, lonlat_to_world_xy};
use ofield_core::paths::assets_dir;

use ofield_core::sync_scenario::OsatconScenarioV1;

use crate::ui::{
    draw_text, draw_world_zoom_inset, red_to_blue_gradient, SatDraw, WorldInsetParams,
};

use std::sync::{ Arc, RwLock };

const MAX_SATS_RENDER: usize = 10;
const DESIRE_CLOSER_SAT: usize = 3;

const CAM_ZOOM_W_FRAC: f32 = 0.03;
const CAM_ZOOM_H_FRAC: f32 = 0.03;

#[derive(Debug, Clone, Default)]
pub struct MeshAlertUiState {
    pub latest_sequence: u64,
    /// Mesh returned an encrypted AlertMathOutputV1.
    pub encrypted_result_available: bool,
    /// Result after Kentr user-decrypt of exposed_ct.
    ///
    /// None means:
    /// - no encrypted result yet, or
    /// - decrypt still pending, or
    /// - decrypt failed and last_error is set.
    pub fhe_exposed: Option<bool>,
    pub fhe_context: Option<MeshFheAlertContext>,

    pub last_metadata: Vec<(String, String)>,

    pub compute_error: Option<String>,
    pub decrypt_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MeshFheAlertContext {
    pub convoy_lat: f64,
    pub convoy_lon: f64,
    pub satellites: Vec<MeshFheSatelliteContext>,
}

#[derive(Debug, Clone)]
pub struct MeshFheSatelliteContext {
    pub sat_id: i32,
    pub lat: f64,
    pub lon: f64,

    /// Distance computed from the same projected fixed-point coordinate
    /// system used by smart-program-alert, not haversine.
    pub d_km: f64,
}

pub fn run(
    mesh_alert: Arc<RwLock<MeshAlertUiState>>,
    scenario: OsatconScenarioV1,
) -> Result<()> {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1920.0, 1020.0])
            .with_title("ØMYSTIK - Convoy Exposure to Satellites"),
        ..Default::default()
    };

    eframe::run_native(
        "ØMYSTIK - Convoy Exposure to Satellites",
        options,
        Box::new(|cc| {
            Ok(Box::new(OfieldSimApp::new(
                cc,
                mesh_alert.clone(),
                scenario.clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

struct LoadedState {
    // main map
    world_texture: TextureHandle,
    world_display_size: [usize; 2],

    // original TIFF kept on CPU for sharp zoom crops
    world_full_rgba: image::RgbaImage,
    world_full_size: [usize; 2],

    // zoom inset texture updated from full-res crop
    zoom_texture: TextureHandle,

    logo_texture: TextureHandle,
}

pub struct OfieldSimApp {
    loaded: Option<LoadedState>,
    startup_error: Option<String>,
    mesh_alert: Arc<RwLock<MeshAlertUiState>>,
    scenario: OsatconScenarioV1,

    last_mesh_convoy_pos: Option<(f64, f64)>,
    mesh_heading: f32,
}

impl OfieldSimApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        mesh_alert: Arc<RwLock<MeshAlertUiState>>,
        scenario: OsatconScenarioV1,
    ) -> Self {
        Self {
            loaded: None,
            startup_error: None,
            mesh_alert,
            scenario,
            last_mesh_convoy_pos: None,
            mesh_heading: 0.0,
        }
    }

    fn try_load(&mut self, ctx: &egui::Context) -> Result<()> {

        let world_path = assets_dir().join("GRAY_LR_SR_OB_DR.tif");
        let full_img = image::open(&world_path)?.to_rgba8();
        let full_w = full_img.width();
        let full_h = full_img.height();

        // main map display texture: downscaled for GPU safety
        let max_w: u32 = 2048;
        let (disp_w, disp_h) = if full_w > max_w {
            let scale = max_w as f32 / full_w as f32;
            let h = (full_h as f32 * scale).round().max(1.0) as u32;
            (max_w, h)
        } else {
            (full_w, full_h)
        };

        let disp_img = image::DynamicImage::ImageRgba8(full_img.clone())
            .resize_exact(disp_w, disp_h, image::imageops::FilterType::Triangle)
            .to_rgba8();

        let display_color_image = egui::ColorImage::from_rgba_unmultiplied(
            [disp_w as usize, disp_h as usize],
            disp_img.as_raw(),
        );
        let world_texture = ctx.load_texture("world-map", display_color_image, Default::default());

        // initialize zoom texture with a tiny placeholder; will be replaced on first frame
        let zoom_placeholder = egui::ColorImage::from_rgba_unmultiplied([2, 2], &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
        let zoom_texture = ctx.load_texture("world-zoom", zoom_placeholder, Default::default());

        let logo_path = assets_dir().join("ømystik_xmini.png");
        let logo_img = image::open(&logo_path)?.to_rgba8();
        let logo_size = [logo_img.width() as usize, logo_img.height() as usize];

        let logo_color_image =
            egui::ColorImage::from_rgba_unmultiplied(logo_size, logo_img.as_raw());

        let logo_texture = ctx.load_texture("logo", logo_color_image, Default::default());

       self.loaded = Some(LoadedState {
            world_texture,
            world_display_size: [disp_w as usize, disp_h as usize],
            world_full_rgba: full_img,
            world_full_size: [full_w as usize, full_h as usize],
            zoom_texture,
            logo_texture,
        });

        Ok(())
    }

    fn fit_rect_preserve_aspect(container: Rect, image_w: f32, image_h: f32) -> Rect {
        let img_aspect = image_w / image_h.max(1.0);
        let box_aspect = container.width() / container.height().max(1.0);

        let size = if box_aspect > img_aspect {
            let h = container.height();
            let w = h * img_aspect;
            vec2(w, h)
        } else {
            let w = container.width();
            let h = w / img_aspect;
            vec2(w, h)
        };

        Rect::from_center_size(container.center(), size)
    }

    fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
        v.max(lo).min(hi)
    }

fn render_state_from_mesh_context(
    ctx: &MeshFheAlertContext,
) -> (f64, f64, Vec<SatDraw>) {
    let convoy_lat = ctx.convoy_lat;
    let convoy_lon = clamp_lon(ctx.convoy_lon);

    let mut sats = ctx
        .satellites
        .iter()
        .map(|sat| SatDraw {
            sat_id: sat.sat_id,
            lat: sat.lat,
            lon: clamp_lon(sat.lon),
            alt: 0.0,
            d_km: sat.d_km,
        })
        .collect::<Vec<_>>();

    sats.sort_by(|a, b| {
        a.d_km
            .partial_cmp(&b.d_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (convoy_lat, convoy_lon, sats)
}

fn update_zoom_texture(
    state: &mut LoadedState,
    center_lat: f64,
    center_lon: f64,
    zoom_w_frac: f32,
    zoom_h_frac: f32,
    inset_pixel_w: usize,
    inset_pixel_h: usize,
) {
    let full_w = state.world_full_size[0] as f32;
    let full_h = state.world_full_size[1] as f32;

    let inset_aspect = inset_pixel_w as f32 / (inset_pixel_h as f32).max(1.0);

    let mut base_w = (full_w * zoom_w_frac).max(64.0);
    let mut base_h = (full_h * zoom_h_frac).max(64.0);

    if (base_w / base_h) > inset_aspect {
        base_w = base_h * inset_aspect;
    } else {
        base_h = base_w / inset_aspect;
    }

    let (wx, wy) = lonlat_to_world_xy(clamp_lon(center_lon), center_lat, full_w as f64, full_h as f64);

    let mut left = wx as f32 - base_w * 0.5;
    let mut top = wy as f32 - base_h * 0.5;

    left = Self::clamp_f32(left, 0.0, (full_w - base_w).max(0.0));
    top = Self::clamp_f32(top, 0.0, (full_h - base_h).max(0.0));

    let crop_x = left.round() as u32;
    let crop_y = top.round() as u32;
    let crop_w = base_w.round().max(1.0) as u32;
    let crop_h = base_h.round().max(1.0) as u32;

    let sub = image::imageops::crop_imm(
        &state.world_full_rgba,
        crop_x,
        crop_y,
        crop_w.min(state.world_full_rgba.width().saturating_sub(crop_x)),
        crop_h.min(state.world_full_rgba.height().saturating_sub(crop_y)),
    )
    .to_image();

    let resized = image::DynamicImage::ImageRgba8(sub)
        .resize_exact(
            inset_pixel_w.max(1) as u32,
            inset_pixel_h.max(1) as u32,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgba8();

    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [inset_pixel_w.max(1), inset_pixel_h.max(1)],
        resized.as_raw(),
    );

    state.zoom_texture.set(color_image, Default::default());
}


}


impl App for OfieldSimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if self.loaded.is_none() && self.startup_error.is_none() {
            if let Err(e) = self.try_load(ctx) {
                self.startup_error = Some(e.to_string());
            }
        }

        if let Some(err) = &self.startup_error {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Startup failed");
                ui.label(err);
            });
            return;
        }

        let alert_radius_km = self.scenario.alert_radius_km;

        let Some(state) = self.loaded.as_mut() else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Loading...");
            });
            ctx.request_repaint();
            return;
        };

        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        let mesh_snapshot = self
            .mesh_alert
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();

        let mesh_context_ready = mesh_snapshot.fhe_context.is_some();

        let (display_convoy_lat, display_convoy_lon, sorted_sats) =
            if let Some(mesh_context) = &mesh_snapshot.fhe_context {
                Self::render_state_from_mesh_context(mesh_context)
            } else {
                (0.0, 0.0, Vec::new())
            };

        let display_source = if mesh_context_ready {
            "Decrypted context"
        } else {
            "Waiting for decrypted context"
        };

        let gradient_colors = red_to_blue_gradient(MAX_SATS_RENDER);

        if mesh_context_ready {
            let current_pos = (display_convoy_lat, display_convoy_lon);

            if let Some((prev_lat, prev_lon)) = self.last_mesh_convoy_pos {
                let d_lat = (display_convoy_lat - prev_lat).abs();
                let d_lon = (display_convoy_lon - prev_lon).abs();

                if d_lat > 1e-9 || d_lon > 1e-9 {
                    let b = bearing_rad(
                        prev_lat,
                        prev_lon,
                        display_convoy_lat,
                        display_convoy_lon,
                    );

                    self.mesh_heading = -(bearing_to_screen_angle(b) as f32);
                }
            }

            self.last_mesh_convoy_pos = Some(current_pos);
        }

        let mesh_heading = self.mesh_heading;

        egui::SidePanel::right("side_panel")
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.add_space(2.0);

                ui.label(
                    RichText::new("CONVOY")
                        .strong()
                        .size(16.0),
                );

                ui.add_space(8.0);

                ui.small(format!("Display source: {display_source}"));

                if mesh_context_ready {
                    ui.label(format!(
                        "Lat/Lon: {:+.4}, {:+.4}",
                        display_convoy_lat,
                        display_convoy_lon
                    ));
                } else {
                    ui.colored_label(
                        Color32::ORANGE,
                        "Waiting for decrypted context...",
                    );
                    ui.label("Lat/Lon: —");
                }

                ui.add_space(12.0);
                ui.label("convoy zoom");
                ui.add_space(4.0);

                let zoom_size = egui::vec2(ui.available_width(), 250.0);
                let (zoom_rect, _) = ui.allocate_exact_size(zoom_size, egui::Sense::hover());
                let zoom_painter = ui.painter_at(zoom_rect);

                if mesh_context_ready {
                    Self::update_zoom_texture(
                        state,
                        display_convoy_lat,
                        display_convoy_lon,
                        CAM_ZOOM_W_FRAC,
                        CAM_ZOOM_H_FRAC,
                        zoom_rect.width().max(1.0) as usize,
                        zoom_rect.height().max(1.0) as usize,
                    );

                    draw_world_zoom_inset(
                        &zoom_painter,
                        state.zoom_texture.id(),
                        WorldInsetParams {
                            inset_rect: zoom_rect,
                            center_lat: display_convoy_lat,
                            center_lon: display_convoy_lon,
                            map_w: state.world_full_size[0] as f32,
                            map_h: state.world_full_size[1] as f32,
                            zoom_w_frac: CAM_ZOOM_W_FRAC,
                            zoom_h_frac: CAM_ZOOM_H_FRAC,
                            convoy_track: &[],
                            convoy_lat: display_convoy_lat,
                            convoy_lon: display_convoy_lon,
                            heading_rad: mesh_heading,
                            sats: &sorted_sats,
                            sat_colors: &gradient_colors,
                            max_sats: DESIRE_CLOSER_SAT,
                            radius_km: Some(alert_radius_km),
                        },
                        mesh_snapshot.fhe_exposed.unwrap_or(false),
                    );
                } else {
                    zoom_painter.rect_filled(
                        zoom_rect,
                        6.0,
                        Color32::from_rgb(18, 18, 18),
                    );

                    zoom_painter.text(
                        zoom_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Waiting for decrypted context...",
                        egui::FontId::proportional(14.0),
                        Color32::GRAY,
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(
                    RichText::new("EXPOSURE RESULT")
                        .strong()
                        .size(16.0),
                );

                ui.add_space(6.0);

                if let Some(err) = &mesh_snapshot.compute_error {
                    ui.colored_label(Color32::RED, "Mesh compute error");
                    ui.colored_label(Color32::RED, err);
                } else if mesh_snapshot.encrypted_result_available {
                    ui.colored_label(
                        Color32::from_rgb(68, 215, 168),
                        "Encrypted FHE alert result available",
                    );

                    ui.small(format!("Result sequence: {}", mesh_snapshot.latest_sequence));

                    for (k, v) in mesh_snapshot.last_metadata.iter().take(6) {
                        ui.small(format!("{k}: {v}"));
                    }

                    if let Some(err) = &mesh_snapshot.decrypt_error {
                        ui.colored_label(
                            Color32::ORANGE,
                            format!("Threshold decrypt pending/error: {err}"),
                        );
                    }
                } else {
                    ui.colored_label(
                        Color32::ORANGE,
                        "Waiting for encrypted FHE result...",
                    );

                    ui.small(format!(
                        "Encrypted result available: {}",
                        mesh_snapshot.encrypted_result_available
                    ));

                    ui.small("Compute error: —");
                    ui.small("Threshold decrypt pending/error: —");
                }

                ui.add_space(8.0);

                if mesh_context_ready {
                    ui.colored_label(
                        Color32::from_rgb(68, 215, 168),
                        "Decrypted context: available",
                    );
                } else {
                    ui.colored_label(
                        Color32::ORANGE,
                        "Decrypted context: waiting",
                    );
                }

                ui.add_space(8.0);

                match mesh_snapshot.fhe_exposed {
                    Some(true) => {
                        ui.colored_label(
                            Color32::from_rgb(255, 165, 0),
                            "Authoritative FHE status: EXPOSED",
                        );
                    }

                    Some(false) => {
                        ui.colored_label(
                            Color32::from_rgb(0, 128, 255),
                            "Authoritative FHE status: SAFE",
                        );
                    }

                    None => {
                        ui.colored_label(
                            Color32::ORANGE,
                            "Authoritative FHE status: unavailable",
                        );
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(
                    RichText::new("NEAREST SATELLITES")
                        .strong()
                        .size(16.0),
                );

                ui.add_space(4.0);

                if mesh_context_ready {
                    if let Some(nearest) = sorted_sats.first() {
                        let nearest_color = gradient_colors
                            .first()
                            .copied()
                            .unwrap_or(Color32::WHITE);

                        ui.colored_label(
                            nearest_color,
                            format!("Nearest sat. id: {}", nearest.sat_id),
                        );

                        ui.label(format!("Distance: {:.0} km", nearest.d_km));
                        ui.label(format!("Radius: {:.0} km", alert_radius_km));
                    } else {
                        ui.label("Nearest: —");
                    }

                    ui.add_space(6.0);

                    for (idx, sp) in sorted_sats.iter().take(DESIRE_CLOSER_SAT).enumerate() {
                        let col = gradient_colors
                            .get(idx)
                            .copied()
                            .unwrap_or(Color32::LIGHT_GRAY);

                        ui.colored_label(
                            col,
                            format!("{}  d={:.0}km", sp.sat_id, sp.d_km),
                        );
                    }

                    if let Some(first) = sorted_sats.first() {
                        ui.small(format!(
                            "Nearest pos: {:+.3}, {:+.3}",
                            first.lat,
                            first.lon
                        ));
                    }
                } else {
                    ui.colored_label(
                        Color32::ORANGE,
                        "Waiting for decrypted satellite context...",
                    );
                }

                ui.add_space(12.0);
                ui.small("Privacy story:
                Raw edge streams are encrypted before spread into ømystik.
                FHE computation are held into the mesh between encrypted Convoy and Satellites data. 
                Only exposure result and localization fields data are threshold-decrypted for the UI.");
            });


        egui::CentralPanel::default().show(ctx, |ui| {
            let availa = ui.available_rect_before_wrap();
            let painter = ui.painter_at(availa);


            let map_rect = Self::fit_rect_preserve_aspect(
                availa,
                state.world_display_size[0] as f32,
                state.world_display_size[1] as f32,
            );

            painter.rect_filled(availa, 0.0, Color32::BLACK);

            painter.image(
                state.world_texture.id(),
                map_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            let logo_h = 62.0;
            let logo_w = 62.0 * 2.65;

            let logo_rect = Rect::from_min_size(
                pos2(map_rect.left() + 25.0, map_rect.top() + 20.0),
                vec2(logo_w, logo_h),
            );

            painter.image(
                state.logo_texture.id(),
                logo_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );

            let map_w = map_rect.width() as f64;
            let map_h = map_rect.height() as f64;


            if mesh_context_ready {
                let (cx, cy) = lonlat_to_world_xy(display_convoy_lon, display_convoy_lat, map_w, map_h);
                let convoy_pos = pos2(map_rect.left() + cx as f32, map_rect.top() + cy as f32);

                crate::ui::draw_arrow(
                    &painter,
                    convoy_pos,
                    7.0,
                    Color32::from_rgb(239, 206, 94),
                    mesh_heading,
                );

                draw_text(
                    &painter,
                    "Decrypted CONVOY",
                    convoy_pos + vec2(12.0, -10.0),
                    Color32::WHITE,
                    Some(Color32::BLACK),
                );
            } else {
                draw_text(
                    &painter,
                    "Waiting for decrypted context...",
                    pos2(map_rect.left() + 25.0, map_rect.top() + 95.0),
                    Color32::GRAY,
                    Some(Color32::BLACK),
                );
            }

            if mesh_context_ready {
                for (idx, sp) in sorted_sats.iter().take(MAX_SATS_RENDER).enumerate() {
                    let (sx, sy) = lonlat_to_world_xy(clamp_lon(sp.lon), sp.lat, map_w, map_h);
                    let p = pos2(map_rect.left() + sx as f32, map_rect.top() + sy as f32);

                    let col = if idx < DESIRE_CLOSER_SAT {
                        gradient_colors[idx]
                    } else {
                        Color32::from_rgb(68, 215, 168)
                    };
                    let r = if idx < DESIRE_CLOSER_SAT { 5.0 } else { 4.0 };

                    painter.circle_filled(p, r, col);
                }
            }
        });
    }

}