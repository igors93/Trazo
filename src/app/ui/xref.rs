//! External references (XREF) manager dialog and live canvas overlay (#44).

use eframe::egui::{self, Color32, Margin, RichText, Shape, Stroke, vec2};

use super::super::{DocDialog, FloorPlanApp, canvas::ViewTransform, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::domain::{element::Element, geometry::Point, transform::Transform};

impl FloorPlanApp {
    /// Polls attached external references for live on-disk modifications and
    /// schedules gentle periodic repaints when XREFs are attached.
    pub(in crate::app) fn poll_xrefs(&mut self, ctx: &egui::Context) {
        if self.project.xrefs.is_empty() {
            if self.xref_manager.has_entries() {
                self.xref_manager.sync_live(&[], false);
            }
            return;
        }
        let updated = self.xref_manager.sync_live(&self.project.xrefs, false);
        if !updated.is_empty() {
            self.message = format!(
                "Referência externa atualizada ao vivo: {}.",
                updated.join(", ")
            );
            ctx.request_repaint();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    pub(in crate::app) fn attach_xref_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Anexar referência externa (XREF)")
            .add_filter("Projetos Trazo e DXF", &["trazo", "json", "dxf"])
            .add_filter("Projeto Trazo", &["trazo", "json"])
            .add_filter("AutoCAD DXF", &["dxf"])
            .pick_file()
        else {
            return;
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Referência")
            .trim_end_matches(".trazo")
            .to_owned();
        let prev = self.project.clone();
        let id = self.project.add_xref(
            name.clone(),
            path.to_string_lossy().into_owned(),
            Point::new(0.0, 0.0),
        );
        if let Some(xref) = self.project.xrefs.iter().find(|x| x.id == id) {
            self.xref_manager.reload_one(xref);
        }
        self.record_change(prev);
        self.message =
            format!("Referência externa '{name}' anexada com atualização automática ao vivo.");
    }

    /// Paints all visible attached XREFs onto the canvas in a tinted,
    /// translucent style so external drawings coordinate cleanly with the active plan.
    pub(in crate::app) fn paint_xrefs_overlay(&self, painter: &egui::Painter, view: ViewTransform) {
        for xref in &self.project.xrefs {
            if !xref.visible {
                continue;
            }
            let Some(loaded) = self.xref_manager.get(xref.id) else {
                continue;
            };
            if loaded.error.is_some() {
                continue;
            }
            let alpha = (xref.opacity.clamp(0.1, 1.0) * 255.0).round() as u8;
            let [r, g, b] = xref.tint;
            let ink = Color32::from_rgba_unmultiplied(r, g, b, alpha);
            let soft_fill = Color32::from_rgba_unmultiplied(r, g, b, alpha / 8);
            let shift = |p: Point| view.to_screen(p.translated(xref.offset));

            // Faint room polygons.
            for room in &loaded.project.rooms {
                if let Some(poly) = loaded.project.room_polygon(room) {
                    for tri in crate::domain::geometry::triangulate_polygon(&poly) {
                        painter.add(Shape::convex_polygon(
                            vec![shift(tri[0]), shift(tri[1]), shift(tri[2])],
                            soft_fill,
                            Stroke::NONE,
                        ));
                    }
                }
            }

            // Double-line / outlined walls of the external drawing.
            let border = Stroke::new(1.1, ink);
            for wall in &loaded.project.walls {
                let a = shift(wall.start);
                let b = shift(wall.end);
                let len = a.distance(b);
                if len < 0.5 {
                    continue;
                }
                let dir = (b - a) / len;
                let n =
                    vec2(-dir.y, dir.x) * ((view.scale * wall.thickness() as f32).max(1.5) * 0.5);
                painter.line_segment([a + n, b + n], border);
                painter.line_segment([a - n, b - n], border);
            }

            // Fixtures outlines.
            let detail = Stroke::new(0.9, ink);
            for fixture in &loaded.project.fixtures {
                for shape in fixture.symbol() {
                    match shape {
                        crate::domain::fixtures::SymbolShape::Polyline {
                            points, closed, ..
                        } => {
                            let pts: Vec<_> =
                                points.iter().map(|p| shift(fixture.to_world(*p))).collect();
                            if closed {
                                painter.add(Shape::closed_line(pts, detail));
                            } else {
                                painter.add(Shape::line(pts, detail));
                            }
                        }
                        crate::domain::fixtures::SymbolShape::Circle { center, radius, .. } => {
                            painter.circle_stroke(
                                shift(fixture.to_world(center)),
                                view.pixels(radius),
                                detail,
                            );
                        }
                    }
                }
            }

            // 2D drawings.
            for drawing in &loaded.project.drawings {
                let pts: Vec<_> = drawing.shape.outline().into_iter().map(&shift).collect();
                if pts.len() >= 2 {
                    painter.add(Shape::line(pts, detail));
                }
            }
        }
    }

    pub(super) fn xref_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Xref {
            return;
        }
        let mut close = false;
        let mut attach_clicked = false;
        let mut reload_id: Option<u64> = None;
        let mut bind_id: Option<u64> = None;
        let mut remove_id: Option<u64> = None;

        let response = egui::Modal::new(egui::Id::unique("xref_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(660.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Layers,
                    "Referências externas (XREF)",
                    "Vincule outros projetos (.trazo / .json) ou desenhos AutoCAD (.dxf) sobre a planta com atualização automática ao vivo.",
                );

                ui.horizontal(|ui| {
                    if primary_button(ui, "Anexar desenho (.trazo / .dxf)...").clicked() {
                        attach_clicked = true;
                    }
                    if !self.project.xrefs.is_empty()
                        && secondary_button(ui, "Recarregar todas").clicked()
                    {
                        self.xref_manager.sync_live(&self.project.xrefs, true);
                        self.message = "Referências externas recarregadas.".to_owned();
                    }
                });

                ui.add_space(10.0);
                if self.project.xrefs.is_empty() {
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Nenhuma referência externa anexada neste projeto.")
                                    .size(12.5)
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(
                                    "Quando o arquivo referenciado for salvo em outra aba ou programa, o Trazo atualiza a sobreposição automaticamente.",
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
                            for xref in &mut self.project.xrefs {
                                let status_line = match self.xref_manager.get(xref.id) {
                                    Some(loaded) if loaded.error.is_none() => (
                                        format!(
                                            "Ativo ao vivo · {} paredes, {} cômodos, {} objetos",
                                            loaded.project.walls.len(),
                                            loaded.project.rooms.len(),
                                            loaded.project.fixtures.len(),
                                        ),
                                        SUCCESS,
                                    ),
                                    Some(loaded) => (
                                        loaded
                                            .error
                                            .clone()
                                            .unwrap_or_else(|| "Erro ao carregar".to_owned()),
                                        DANGER,
                                    ),
                                    None => ("Carregando...".to_owned(), MUTED),
                                };
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(8)
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&xref.name)
                                                    .font(strong(13.0))
                                                    .color(TEXT),
                                            );
                                            ui.label(
                                                RichText::new(status_line.0)
                                                    .size(11.5)
                                                    .color(status_line.1),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.small_button("Desanexar").clicked() {
                                                        remove_id = Some(xref.id);
                                                    }
                                                    if ui
                                                        .small_button("Vincular (Bind)")
                                                        .on_hover_text(
                                                            "Incorpora a geometria da referência como elementos editáveis neste projeto",
                                                        )
                                                        .clicked()
                                                    {
                                                        bind_id = Some(xref.id);
                                                    }
                                                    if ui.small_button("Recarregar").clicked() {
                                                        reload_id = Some(xref.id);
                                                    }
                                                    if ui
                                                        .checkbox(&mut xref.visible, "Visível")
                                                        .changed()
                                                    {
                                                        edited = true;
                                                    }
                                                },
                                            );
                                        });
                                        ui.label(
                                            RichText::new(&xref.path).size(10.5).color(MUTED),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("Opacidade:")
                                                    .size(11.5)
                                                    .color(TEXT_2),
                                            );
                                            let mut pct = (xref.opacity * 100.0).round() as i32;
                                            if ui
                                                .add(
                                                    egui::Slider::new(&mut pct, 10..=100)
                                                        .suffix("%"),
                                                )
                                                .changed()
                                            {
                                                xref.opacity = (pct as f32) / 100.0;
                                                edited = true;
                                            }
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new("Deslocamento X:")
                                                    .size(11.5)
                                                    .color(MUTED),
                                            );
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut xref.offset.x)
                                                        .speed(0.25)
                                                        .suffix(" m"),
                                                )
                                                .changed()
                                            {
                                                edited = true;
                                            }
                                            ui.label(RichText::new("Y:").size(11.5).color(MUTED));
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut xref.offset.y)
                                                        .speed(0.25)
                                                        .suffix(" m"),
                                                )
                                                .changed()
                                            {
                                                edited = true;
                                            }
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

        if attach_clicked {
            self.attach_xref_dialog();
        }
        if let Some(id) = reload_id
            && let Some(xref) = self.project.xrefs.iter().find(|x| x.id == id)
        {
            self.xref_manager.reload_one(xref);
            self.message = format!("Referência '{}' recarregada.", xref.name);
        }
        if let Some(id) = bind_id
            && let Some(xref) = self.project.xrefs.iter().find(|x| x.id == id).cloned()
            && let Some(loaded) = self.xref_manager.get(id)
            && loaded.error.is_none()
        {
            let elements: Vec<Element> = loaded
                .project
                .walls
                .iter()
                .map(|w| Element::Wall(w.id))
                .chain(
                    loaded
                        .project
                        .fixtures
                        .iter()
                        .map(|f| Element::Fixture(f.id)),
                )
                .chain(
                    loaded
                        .project
                        .drawings
                        .iter()
                        .map(|d| Element::Drawing(d.id)),
                )
                .collect();
            let frag = loaded.project.fragment(&elements);
            if let Ok(moved) = frag.transformed(&Transform::Translate(xref.offset)) {
                let prev = self.project.clone();
                match self.project.insert_fragment(&moved) {
                    Ok(created) => {
                        self.project.remove_xref(id);
                        self.record_change(prev);
                        self.message = format!(
                            "Referência '{}' vinculada ao projeto ({} elemento(s) incorporados).",
                            xref.name,
                            created.len()
                        );
                    }
                    Err(err) => self.message = err,
                }
            }
        }
        if let Some(id) = remove_id {
            let prev = self.project.clone();
            if self.project.remove_xref(id) {
                self.record_change(prev);
                self.message = "Referência externa desanexada.".to_owned();
            }
        }
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
