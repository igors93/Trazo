use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button};
use crate::domain::{
    geometry::Point,
    sheet::DrawingScale,
    views::{
        ElevationSide, detail_svg, elevation_svg, export_view_pdf, export_view_svg, section_svg,
    },
};

enum ViewExportTarget {
    Section(u64),
    Elevation(u64),
    Detail(u64),
}

impl FloorPlanApp {
    pub(super) fn views_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Views {
            return;
        }
        let mut open = true;
        let mut export_request: Option<(ViewExportTarget, bool)> = None;
        let mut changed = false;

        egui::Window::new("Cortes, Fachadas, Detalhes e Referências")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(660.0)
            .default_height(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Defina linhas de corte, fachadas e chamadas de detalhe com referência cruzada entre pranchas.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                egui::ScrollArea::vertical()
                    .max_height(430.0)
                    .show(ui, |ui| {
                        // Sections
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Cortes").font(strong(13.0)).color(TEXT),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if secondary_button(ui, "+ Corte transversal").clicked()
                                            {
                                                let prev = self.project.clone();
                                                if self.project.add_default_section(false).is_ok() {
                                                    self.record_change(prev);
                                                }
                                            }
                                            if primary_button(ui, "+ Corte longitudinal").clicked()
                                            {
                                                let prev = self.project.clone();
                                                if self.project.add_default_section(true).is_ok() {
                                                    self.record_change(prev);
                                                }
                                            }
                                        },
                                    );
                                });
                                ui.add_space(6.0);
                                if self.project.views.sections.is_empty() {
                                    ui.label(
                                        RichText::new("Nenhum corte criado ainda.")
                                            .size(11.5)
                                            .color(MUTED),
                                    );
                                }
                                let mut remove_id = None;
                                for sec in &mut self.project.views.sections {
                                    let mut ref_text =
                                        sec.target_sheet.clone().unwrap_or_default();
                                    let id = sec.id;
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("Corte {}", sec.label))
                                                .font(strong(12.0))
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new("Ref. prancha:")
                                                .size(11.5)
                                                .color(TEXT_2),
                                        );
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut ref_text)
                                                    .desired_width(68.0)
                                                    .hint_text("PR-02"),
                                            )
                                            .changed()
                                        {
                                            let trimmed = ref_text.trim();
                                            sec.target_sheet = (!trimmed.is_empty())
                                                .then(|| trimmed.to_owned());
                                            changed = true;
                                        }
                                        if ui
                                            .small_button(if sec.look_left {
                                                "Olhar ↑/←"
                                            } else {
                                                "Olhar ↓/→"
                                            })
                                            .clicked()
                                        {
                                            sec.look_left = !sec.look_left;
                                            changed = true;
                                        }
                                        if ui.small_button("SVG").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Section(id), false));
                                        }
                                        if ui.small_button("PDF").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Section(id), true));
                                        }
                                        if ui.small_button("Excluir").clicked() {
                                            remove_id = Some(id);
                                        }
                                    });
                                }
                                if let Some(id) = remove_id {
                                    let prev = self.project.clone();
                                    self.project.remove_section(id);
                                    self.record_change(prev);
                                }
                            });

                        ui.add_space(8.0);

                        // Elevations
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new("Fachadas").font(strong(13.0)).color(TEXT),
                                    );
                                    for side in ElevationSide::ALL {
                                        if secondary_button(ui, &format!("+ {}", side.short()))
                                            .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.add_elevation_view(side, None);
                                            self.record_change(prev);
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                if self.project.views.elevations.is_empty() {
                                    ui.label(
                                        RichText::new("Nenhuma fachada criada ainda.")
                                            .size(11.5)
                                            .color(MUTED),
                                    );
                                }
                                let mut remove_id = None;
                                for elev in &mut self.project.views.elevations {
                                    let mut ref_text =
                                        elev.target_sheet.clone().unwrap_or_default();
                                    let id = elev.id;
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&elev.label)
                                                .font(strong(12.0))
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new("Ref. prancha:")
                                                .size(11.5)
                                                .color(TEXT_2),
                                        );
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut ref_text)
                                                    .desired_width(68.0)
                                                    .hint_text("PR-03"),
                                            )
                                            .changed()
                                        {
                                            let trimmed = ref_text.trim();
                                            elev.target_sheet = (!trimmed.is_empty())
                                                .then(|| trimmed.to_owned());
                                            changed = true;
                                        }
                                        if ui.small_button("SVG").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Elevation(id), false));
                                        }
                                        if ui.small_button("PDF").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Elevation(id), true));
                                        }
                                        if ui.small_button("Excluir").clicked() {
                                            remove_id = Some(id);
                                        }
                                    });
                                }
                                if let Some(id) = remove_id {
                                    let prev = self.project.clone();
                                    self.project.remove_elevation(id);
                                    self.record_change(prev);
                                }
                            });

                        ui.add_space(8.0);

                        // Detail callouts
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Chamadas de Detalhe")
                                            .font(strong(13.0))
                                            .color(TEXT),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if primary_button(ui, "+ Novo detalhe (1:25)").clicked()
                                            {
                                                let center = self
                                                    .project
                                                    .content_bounds()
                                                    .map(|(a, b)| a.midpoint(b))
                                                    .unwrap_or(Point::new(2.0, 2.0));
                                                let prev = self.project.clone();
                                                if self
                                                    .project
                                                    .add_detail_callout(
                                                        "",
                                                        center,
                                                        1.2,
                                                        DrawingScale::S1_25,
                                                        None,
                                                    )
                                                    .is_ok()
                                                {
                                                    self.record_change(prev);
                                                }
                                            }
                                        },
                                    );
                                });
                                ui.add_space(6.0);
                                if self.project.views.details.is_empty() {
                                    ui.label(
                                        RichText::new("Nenhuma chamada de detalhe criada ainda.")
                                            .size(11.5)
                                            .color(MUTED),
                                    );
                                }
                                let mut remove_id = None;
                                for det in &mut self.project.views.details {
                                    let mut ref_text =
                                        det.target_sheet.clone().unwrap_or_default();
                                    let id = det.id;
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&det.label)
                                                .font(strong(12.0))
                                                .color(TEXT),
                                        );
                                        ui.label("R:");
                                        changed |= ui
                                            .add(
                                                egui::DragValue::new(&mut det.radius)
                                                    .range(0.3..=6.0)
                                                    .speed(0.1)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                        ui.label(RichText::new("Ref.:").size(11.5).color(TEXT_2));
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut ref_text)
                                                    .desired_width(64.0)
                                                    .hint_text("PR-04"),
                                            )
                                            .changed()
                                        {
                                            let trimmed = ref_text.trim();
                                            det.target_sheet = (!trimmed.is_empty())
                                                .then(|| trimmed.to_owned());
                                            changed = true;
                                        }
                                        if ui.small_button("SVG").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Detail(id), false));
                                        }
                                        if ui.small_button("PDF").clicked() {
                                            export_request =
                                                Some((ViewExportTarget::Detail(id), true));
                                        }
                                        if ui.small_button("Excluir").clicked() {
                                            remove_id = Some(id);
                                        }
                                    });
                                }
                                if let Some(id) = remove_id {
                                    let prev = self.project.clone();
                                    self.project.remove_detail_callout(id);
                                    self.record_change(prev);
                                }
                            });
                    });
            });

        if changed {
            self.mark_edited_outside_history();
        }
        if let Some((target, is_pdf)) = export_request {
            let rendered = match target {
                ViewExportTarget::Section(id) => self
                    .project
                    .views
                    .sections
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| {
                        (
                            format!("corte-{}", s.label.to_lowercase()),
                            section_svg(&self.project, s),
                        )
                    }),
                ViewExportTarget::Elevation(id) => self
                    .project
                    .views
                    .elevations
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| {
                        (
                            format!("fachada-{}", e.side.short().to_lowercase()),
                            elevation_svg(&self.project, e),
                        )
                    }),
                ViewExportTarget::Detail(id) => self
                    .project
                    .views
                    .details
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| (d.label.to_lowercase(), detail_svg(&self.project, d))),
            };
            if let Some((stem, markup)) = rendered {
                let (label, ext) = if is_pdf {
                    ("Documento PDF", "pdf")
                } else {
                    ("Desenho SVG", "svg")
                };
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(label, &[ext])
                    .set_file_name(format!("{stem}.{ext}"))
                    .save_file()
                {
                    let res = if is_pdf {
                        export_view_pdf(&markup, &path)
                    } else {
                        export_view_svg(&markup, &path)
                    };
                    match res {
                        Ok(()) => self.message = format!("Vista exportada em {}", path.display()),
                        Err(e) => self.message = format!("Erro ao exportar vista: {e}"),
                    }
                }
            }
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
