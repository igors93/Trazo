//! Technical sheet configuration dialog: paper size (A4–A0 and custom),
//! orientation and ABNT NBR 16752 margins.

use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::sheet::{Orientation, PaperSize, SheetMargins};

impl FloorPlanApp {
    pub(super) fn sheet_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Sheet {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("sheet_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(600.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::ExportPdf,
                    "Prancha técnica",
                    "Configure o formato da folha (A4 a A0 ou personalizado), orientação e margens ABNT.",
                );

                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Formato da folha").font(strong(13.0)).color(TEXT));
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for standard in PaperSize::STANDARD {
                                let selected = self.project.sheet.size == standard;
                                if toggle_pill(ui, standard.short_name(), selected, &standard.label())
                                    .clicked()
                                {
                                    let prev = self.project.clone();
                                    self.project.sheet.size = standard;
                                    self.project.sheet.margins = SheetMargins::abnt_for(standard);
                                    self.record_change(prev);
                                }
                            }
                            let is_custom = matches!(self.project.sheet.size, PaperSize::Custom { .. });
                            if toggle_pill(
                                ui,
                                "Personalizado",
                                is_custom,
                                "Definir largura e altura em milímetros",
                            )
                            .clicked()
                                && !is_custom
                            {
                                let prev = self.project.clone();
                                let (w, h) = self.project.sheet.sheet_mm();
                                self.project.sheet.size = PaperSize::Custom {
                                    width_mm: w,
                                    height_mm: h,
                                };
                                self.record_change(prev);
                            }
                        });

                        if let PaperSize::Custom {
                            mut width_mm,
                            mut height_mm,
                        } = self.project.sheet.size
                        {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Largura (mm):").size(12.0).color(TEXT_2));
                                let w_resp = ui.add(
                                    egui::DragValue::new(&mut width_mm)
                                        .range(100.0..=5000.0)
                                        .speed(5.0),
                                );
                                ui.label(RichText::new("Altura (mm):").size(12.0).color(TEXT_2));
                                let h_resp = ui.add(
                                    egui::DragValue::new(&mut height_mm)
                                        .range(100.0..=5000.0)
                                        .speed(5.0),
                                );
                                if w_resp.changed() || h_resp.changed() {
                                    self.project.sheet.size = PaperSize::Custom {
                                        width_mm,
                                        height_mm,
                                    };
                                    self.dirty = true;
                                }
                            });
                        }

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Orientação:").size(12.0).color(TEXT_2));
                            for orient in [Orientation::Landscape, Orientation::Portrait] {
                                if toggle_pill(
                                    ui,
                                    orient.label(),
                                    self.project.sheet.orientation == orient,
                                    orient.label(),
                                )
                                .clicked()
                                {
                                    let prev = self.project.clone();
                                    self.project.sheet.orientation = orient;
                                    self.record_change(prev);
                                }
                            }
                            if secondary_button(ui, "Margens ABNT").clicked() {
                                let prev = self.project.clone();
                                self.project.sheet.margins =
                                    SheetMargins::abnt_for(self.project.sheet.size);
                                self.record_change(prev);
                            }
                        });

                        ui.add_space(8.0);
                        let (sw, sh) = self.project.sheet.sheet_mm();
                        let (_, _, pw, ph) = self.project.sheet.printable_rect_mm();
                        ui.label(
                            RichText::new(format!(
                                "Folha: {sw:.0} × {sh:.0} mm · Área útil de desenho: {pw:.0} × {ph:.0} mm (margem esquerda {:.0} mm)",
                                self.project.sheet.margins.left_mm
                            ))
                            .size(12.0)
                            .color(MUTED),
                        );
                    });

                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, "Fechar").clicked() {
                        close = true;
                    }
                });
            });
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
