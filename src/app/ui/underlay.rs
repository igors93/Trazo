//! External image and PDF underlay manager and interactive two-point scale
//! calibration UI (#40).

use std::{collections::HashMap, path::Path};

use eframe::egui::{
    self, Color32, ColorImage, Margin, Pos2, RichText, Shape, Stroke, TextureHandle, TextureId,
    TextureOptions, vec2,
};

use super::super::{DocDialog, FloorPlanApp, canvas::ViewTransform, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::{
    domain::geometry::{Point, format_meters},
    io::underlay as underlay_io,
};

#[derive(Clone, Debug)]
pub(in crate::app) struct UnderlayCalibration {
    pub underlay_id: u64,
    pub p1: Option<Point>,
    pub p2: Option<Point>,
    pub distance_text: String,
    pub error: Option<String>,
}

#[derive(Default)]
pub(in crate::app) struct UnderlayUiState {
    pub textures: HashMap<u64, TextureHandle>,
    pub failed_ids: Vec<u64>,
    pub pdf_page_input: u32,
    pub calibration: Option<UnderlayCalibration>,
    pub status: Option<(bool, String)>,
}

impl UnderlayUiState {
    pub fn texture_pairs(&self) -> Vec<(u64, TextureId)> {
        self.textures
            .iter()
            .map(|(&id, handle)| (id, handle.id()))
            .collect()
    }
}

impl FloorPlanApp {
    /// Ensures every visible underlay in the active project has a GPU texture
    /// loaded (from its embedded PNG or source path).
    pub(in crate::app) fn sync_underlay_textures(&mut self, ctx: &egui::Context) {
        let active_ids: Vec<u64> = self.project.underlays.iter().map(|u| u.id).collect();
        self.underlay_state
            .textures
            .retain(|id, _| active_ids.contains(id));
        self.underlay_state
            .failed_ids
            .retain(|id| active_ids.contains(id));

        for underlay in &self.project.underlays {
            if !underlay.visible
                || self.underlay_state.textures.contains_key(&underlay.id)
                || self.underlay_state.failed_ids.contains(&underlay.id)
            {
                continue;
            }
            let decoded = if let Some(ref b64) = underlay.embedded_png_base64 {
                underlay_io::decode_embedded_png(b64)
            } else if !underlay.source_path.is_empty() {
                underlay_io::load_underlay_file(Path::new(&underlay.source_path), underlay.page)
                    .map(|d| (d.width, d.height, d.rgba))
            } else {
                Err("Sem origem de imagem".to_owned())
            };
            match decoded {
                Ok((w, h, rgba)) => {
                    let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    let handle = ctx.load_texture(
                        format!("trazo_underlay_{}", underlay.id),
                        image,
                        TextureOptions::LINEAR,
                    );
                    self.underlay_state.textures.insert(underlay.id, handle);
                }
                Err(_) => {
                    self.underlay_state.failed_ids.push(underlay.id);
                }
            }
        }
    }

    /// Prompts for an external PNG/JPG/WebP/BMP/SVG/PDF file and inserts it as
    /// a semi-transparent tracing underlay behind the plan.
    pub(in crate::app) fn import_underlay_file(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importar imagem ou PDF de fundo")
            .add_filter(
                "Imagens e PDF",
                &["png", "jpg", "jpeg", "webp", "bmp", "svg", "pdf"],
            )
            .add_filter("PDF", &["pdf"])
            .add_filter("Imagens", &["png", "jpg", "jpeg", "webp", "bmp", "svg"])
            .pick_file()
        else {
            return;
        };
        let page = self.underlay_state.pdf_page_input.max(1);
        match underlay_io::load_underlay_file(&path, page) {
            Ok(decoded) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Imagem de fundo")
                    .to_owned();
                let origin = self
                    .project
                    .bounds()
                    .map(|(min, _)| min)
                    .unwrap_or(Point::new(0.0, 0.0));
                let default_width_m = self
                    .project
                    .bounds()
                    .map(|(min, max)| (max.x - min.x).clamp(6.0, 40.0))
                    .unwrap_or(15.0);
                let prev = self.project.clone();
                let id = self.project.add_underlay(
                    name.clone(),
                    path.to_string_lossy().into_owned(),
                    page,
                    decoded.width,
                    decoded.height,
                    origin,
                    default_width_m,
                    Some(decoded.png_base64),
                );
                let image = ColorImage::from_rgba_unmultiplied(
                    [decoded.width as usize, decoded.height as usize],
                    &decoded.rgba,
                );
                let handle = ctx.load_texture(
                    format!("trazo_underlay_{id}"),
                    image,
                    TextureOptions::LINEAR,
                );
                self.underlay_state.textures.insert(id, handle);
                self.record_change(prev);
                self.fit_pending = true;
                let msg = format!(
                    "Fundo \"{name}\" importado ({} × {} px). Use Calibrar 2 pontos para ajustar à escala real.",
                    decoded.width, decoded.height
                );
                self.message = msg.clone();
                self.underlay_state.status = Some((true, msg));
            }
            Err(error) => {
                self.message = error.clone();
                self.underlay_state.status = Some((false, error));
            }
        }
    }

    pub(in crate::app) fn start_underlay_calibration(&mut self, underlay_id: u64) {
        if let Some(u) = self.project.underlays.iter().find(|u| u.id == underlay_id)
            && u.locked
        {
            self.underlay_state.status = Some((
                false,
                "Desbloqueie a imagem antes de calibrar a escala.".to_owned(),
            ));
            return;
        }
        self.active_doc_dialog = DocDialog::None;
        self.underlay_state.calibration = Some(UnderlayCalibration {
            underlay_id,
            p1: None,
            p2: None,
            distance_text: "3,00".to_owned(),
            error: None,
        });
        self.message =
            "Calibração de escala: clique no 1º ponto de referência sobre a imagem (Esc cancela)."
                .to_owned();
    }

    /// Handles a canvas click while two-point underlay calibration is active.
    pub(in crate::app) fn handle_calibration_click(&mut self, world_point: Point) {
        let Some(cal) = &mut self.underlay_state.calibration else {
            return;
        };
        if let Some(p1) = cal.p1 {
            if cal.p2.is_none() {
                let current = p1.distance(world_point);
                cal.p2 = Some(world_point);
                cal.distance_text = format!("{current:.2}").replace('.', ",");
                cal.error = None;
                self.message = "Informe a distância real entre os dois pontos clicados.".to_owned();
            }
        } else {
            cal.p1 = Some(world_point);
            self.message =
                "Calibração de escala: clique no 2º ponto de referência (Esc cancela).".to_owned();
        }
    }

    /// Paints the live calibration crosshair, points and dashed ruler over the canvas.
    pub(in crate::app) fn paint_underlay_calibration_overlay(
        &self,
        painter: &egui::Painter,
        view: ViewTransform,
        cursor: Option<Point>,
    ) {
        let Some(cal) = &self.underlay_state.calibration else {
            return;
        };
        let accent = ACCENT;
        let stroke = Stroke::new(1.8, accent);
        let paint_mark = |pt: Point, label: &str| {
            let s = view.to_screen(pt);
            painter.circle(s, 5.5, PANEL, stroke);
            painter.circle_filled(s, 2.2, accent);
            let galley = painter.layout_no_wrap(label.to_owned(), strong(11.0), accent);
            let box_rect = egui::Rect::from_min_size(
                s + vec2(8.0, -galley.size().y * 0.5 - 3.0),
                galley.size() + vec2(8.0, 5.0),
            );
            painter.rect(
                box_rect,
                4.0,
                PANEL,
                Stroke::new(1.0, ACCENT_RING),
                egui::StrokeKind::Inside,
            );
            painter.galley(box_rect.min + vec2(4.0, 2.5), galley, accent);
        };

        if let Some(p1) = cal.p1 {
            paint_mark(p1, "1º ponto");
            if let Some(p2) = cal.p2.or(cursor) {
                let s1 = view.to_screen(p1);
                let s2 = view.to_screen(p2);
                painter.extend(Shape::dashed_line(&[s1, s2], stroke, 7.0, 4.0));
                if cal.p2.is_some() {
                    paint_mark(p2, "2º ponto");
                }
                let dist = p1.distance(p2);
                let mid = s1.lerp(s2, 0.5) + vec2(0.0, -14.0);
                let text = format!("Medido: {}", format_meters(dist));
                let galley = painter.layout_no_wrap(text, strong(11.0), ACCENT_TEXT);
                let pill = egui::Rect::from_center_size(mid, galley.size() + vec2(10.0, 5.0));
                painter.rect(
                    pill,
                    4.0,
                    PANEL,
                    Stroke::new(1.0, ACCENT_RING),
                    egui::StrokeKind::Inside,
                );
                painter.galley(pill.min + vec2(5.0, 2.5), galley, ACCENT_TEXT);
            }
        } else if let Some(cur) = cursor {
            let s = view.to_screen(cur);
            painter.line_segment([s + vec2(-10.0, 0.0), s + vec2(10.0, 0.0)], stroke);
            painter.line_segment([s + vec2(0.0, -10.0), s + vec2(0.0, 10.0)], stroke);
        }
    }

    pub(super) fn underlay_dialog(&mut self, ctx: &egui::Context) {
        // 1) Compact calibration distance modal once both points have been picked.
        if let Some(cal) = self.underlay_state.calibration.clone()
            && let (Some(p1), Some(p2)) = (cal.p1, cal.p2)
        {
            let mut cancel = false;
            let mut apply = false;
            let mut distance_text = cal.distance_text.clone();
            let error_msg = cal.error.clone();
            let measured = p1.distance(p2);

            let resp = egui::Modal::new(egui::Id::unique("underlay_calibrate_modal"))
                .frame(super::dialogs::dialog_frame())
                .backdrop_color(super::dialogs::backdrop())
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    super::dialogs::dialog_title(
                        ui,
                        Icon::Ruler,
                        "Calibrar escala por 2 pontos",
                        "Informe a distância real correspondente ao trecho clicado na imagem.",
                    );
                    ui.label(
                        RichText::new(format!(
                            "Distância atual na tela: {}",
                            format_meters(measured)
                        ))
                        .size(12.0)
                        .color(MUTED),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Distância real (m):").size(12.5).color(TEXT));
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut distance_text)
                                .desired_width(110.0)
                                .hint_text("ex: 4,50"),
                        );
                        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            apply = true;
                        }
                    });
                    if let Some(err) = &error_msg {
                        ui.add_space(6.0);
                        ui.label(RichText::new(err).size(12.0).color(DANGER));
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, "Aplicar escala").clicked() {
                            apply = true;
                        }
                        if secondary_button(ui, "Cancelar").clicked() {
                            cancel = true;
                        }
                    });
                });

            if let Some(cal_mut) = &mut self.underlay_state.calibration {
                cal_mut.distance_text = distance_text.clone();
            }
            if cancel || resp.should_close() {
                self.underlay_state.calibration = None;
                self.message = "Calibração de escala cancelada.".to_owned();
            } else if apply {
                let parsed = distance_text.trim().replace(',', ".").parse::<f64>();
                match parsed {
                    Ok(real_m) => {
                        let prev = self.project.clone();
                        match self
                            .project
                            .calibrate_underlay(cal.underlay_id, p1, p2, real_m)
                        {
                            Ok(factor) => {
                                self.record_change(prev);
                                self.underlay_state.calibration = None;
                                let msg = format!(
                                    "Escala calibrada (fator {:.2}× · trecho = {}).",
                                    factor,
                                    format_meters(real_m)
                                )
                                .replace('.', ",");
                                self.message = msg.clone();
                                self.underlay_state.status = Some((true, msg));
                                self.active_doc_dialog = DocDialog::Underlay;
                            }
                            Err(err) => {
                                if let Some(cal_mut) = &mut self.underlay_state.calibration {
                                    cal_mut.error = Some(err);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        if let Some(cal_mut) = &mut self.underlay_state.calibration {
                            cal_mut.error =
                                Some("Digite um número válido em metros (ex.: 4,50).".to_owned());
                        }
                    }
                }
            }
            return;
        }

        // 2) Main Underlay Manager modal.
        if self.active_doc_dialog != DocDialog::Underlay {
            return;
        }
        if self.underlay_state.pdf_page_input == 0 {
            self.underlay_state.pdf_page_input = 1;
        }

        let mut close = false;
        let mut import_clicked = false;
        let mut calibrate_id: Option<u64> = None;
        let mut remove_id: Option<u64> = None;

        let response = egui::Modal::new(egui::Id::unique("underlay_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(640.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::ExportImage,
                    "Imagem e PDF de fundo (Underlay)",
                    "Importe plantas escaneadas, fotos ou páginas PDF para traçar por cima com calibração de escala em 2 pontos, opacidade e bloqueio.",
                );

                ui.horizontal(|ui| {
                    if primary_button(ui, "Importar imagem / PDF...").clicked() {
                        import_clicked = true;
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new("Página do PDF:").size(12.0).color(TEXT_2));
                    ui.add(
                        egui::DragValue::new(&mut self.underlay_state.pdf_page_input)
                            .range(1..=200),
                    );
                });

                if let Some((ok, ref msg)) = self.underlay_state.status {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(msg)
                            .size(12.0)
                            .color(if ok { ACCENT_TEXT } else { DANGER }),
                    );
                }

                ui.add_space(10.0);
                if self.project.underlays.is_empty() {
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Nenhuma imagem ou PDF de fundo importado ainda.")
                                    .size(12.5)
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(
                                    "Dica: importe uma planta em PNG, JPG ou PDF e clique em \"Calibrar 2 pontos\" para alinhar com uma medida conhecida (ex.: uma parede de 4,00 m).",
                                )
                                .size(11.5)
                                .color(MUTED),
                            );
                        });
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            let mut edited = false;
                            for underlay in &mut self.project.underlays {
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(8)
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&underlay.name)
                                                    .font(strong(13.0))
                                                    .color(TEXT),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} × {} px · {:.2} × {:.2} m",
                                                    underlay.pixel_width,
                                                    underlay.pixel_height,
                                                    underlay.width_m,
                                                    underlay.height_m()
                                                ).replace('.', ","))
                                                .size(11.5)
                                                .color(MUTED),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add_enabled_ui(!underlay.locked, |ui| {
                                                            secondary_button(ui, "Remover")
                                                        })
                                                        .inner
                                                        .clicked()
                                                    {
                                                        remove_id = Some(underlay.id);
                                                    }
                                                    if ui
                                                        .checkbox(&mut underlay.locked, "Bloquear")
                                                        .changed()
                                                    {
                                                        edited = true;
                                                    }
                                                    if ui
                                                        .checkbox(&mut underlay.visible, "Visível")
                                                        .changed()
                                                    {
                                                        edited = true;
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(6.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("Opacidade:")
                                                    .size(12.0)
                                                    .color(TEXT_2),
                                            );
                                            let mut pct = (underlay.opacity * 100.0).round() as i32;
                                            if ui
                                                .add(
                                                    egui::Slider::new(&mut pct, 5..=100)
                                                        .suffix("%"),
                                                )
                                                .changed()
                                            {
                                                underlay.opacity = (pct as f32) / 100.0;
                                                edited = true;
                                            }
                                            ui.add_space(8.0);
                                            if ui
                                                .add_enabled_ui(!underlay.locked, |ui| {
                                                    secondary_button(ui, "Calibrar 2 pontos")
                                                })
                                                .inner
                                                .clicked()
                                            {
                                                calibrate_id = Some(underlay.id);
                                            }
                                        });
                                        ui.add_space(4.0);
                                        ui.add_enabled_ui(!underlay.locked, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new("X (m):").size(11.5).color(MUTED));
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut underlay.origin.x)
                                                            .speed(0.1),
                                                    )
                                                    .changed()
                                                {
                                                    edited = true;
                                                }
                                                ui.label(RichText::new("Y (m):").size(11.5).color(MUTED));
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut underlay.origin.y)
                                                            .speed(0.1),
                                                    )
                                                    .changed()
                                                {
                                                    edited = true;
                                                }
                                                ui.label(
                                                    RichText::new("Largura (m):")
                                                        .size(11.5)
                                                        .color(MUTED),
                                                );
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut underlay.width_m)
                                                            .range(0.2..=5000.0)
                                                            .speed(0.1),
                                                    )
                                                    .changed()
                                                {
                                                    edited = true;
                                                }
                                                ui.label(
                                                    RichText::new("Rotação (°):")
                                                        .size(11.5)
                                                        .color(MUTED),
                                                );
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(
                                                            &mut underlay.rotation_deg,
                                                        )
                                                        .range(-180.0..=180.0)
                                                        .speed(1.0),
                                                    )
                                                    .changed()
                                                {
                                                    edited = true;
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                            if edited {
                                self.mark_edited_outside_history();
                            }
                        });
                }

                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if secondary_button(ui, "Fechar").clicked() {
                        close = true;
                    }
                });
            });

        if import_clicked {
            self.import_underlay_file(ctx);
        }
        if let Some(id) = remove_id {
            let prev = self.project.clone();
            if self.project.remove_underlay(id).is_ok() {
                self.underlay_state.textures.remove(&id);
                self.record_change(prev);
            }
        }
        if let Some(id) = calibrate_id {
            self.start_underlay_calibration(id);
        } else if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}

/// Paints all visible external image / PDF underlays behind the floor plan.
pub(in crate::app) fn paint_image_underlays(
    painter: &egui::Painter,
    view: ViewTransform,
    project: &crate::domain::model::Project,
    textures: &[(u64, TextureId)],
) {
    for underlay in &project.underlays {
        if !underlay.visible {
            continue;
        }
        let Some(&(_, tex_id)) = textures.iter().find(|&&(id, _)| id == underlay.id) else {
            continue;
        };
        let h = underlay.height_m();
        let raw_corners = [
            underlay.origin,
            Point::new(underlay.origin.x + underlay.width_m, underlay.origin.y),
            Point::new(underlay.origin.x + underlay.width_m, underlay.origin.y + h),
            Point::new(underlay.origin.x, underlay.origin.y + h),
        ];
        let world_corners = if underlay.rotation_deg.abs() > 1e-6 {
            let center = Point::new(
                underlay.origin.x + underlay.width_m * 0.5,
                underlay.origin.y + h * 0.5,
            );
            let (sin, cos) = underlay.rotation_deg.to_radians().sin_cos();
            raw_corners.map(|c| {
                let dx = c.x - center.x;
                let dy = c.y - center.y;
                Point::new(
                    center.x + dx * cos - dy * sin,
                    center.y + dx * sin + dy * cos,
                )
            })
        } else {
            raw_corners
        };
        let s: [Pos2; 4] = world_corners.map(|p| view.to_screen(p));
        let alpha = (underlay.opacity.clamp(0.05, 1.0) * 255.0).round() as u8;
        let tint = Color32::from_white_alpha(alpha);
        let mut mesh = egui::Mesh::with_texture(tex_id);
        let uvs = [
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
        ];
        for i in 0..4 {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: s[i],
                uv: uvs[i],
                color: tint,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(Shape::mesh(mesh));

        // Subtle dashed frame around unlocked underlays so users can spot bounds.
        if !underlay.locked {
            painter.extend(Shape::dashed_line(
                &[s[0], s[1], s[2], s[3], s[0]],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(30, 111, 217, 90)),
                6.0,
                4.0,
            ));
        }
    }
}
