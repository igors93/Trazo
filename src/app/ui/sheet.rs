//! Technical sheet configuration dialog: paper size (A4–A0 and custom),
//! orientation and ABNT NBR 16752 margins.

use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::{
    doc_style::{DimensionTick, DimensionUnit, DocStyle, DocStylePreset, NBR_6492_DISCLAIMER},
    sheet::{DrawingScale, Orientation, PaperSize, SheetMargins},
};

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

                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Formato da folha")
                                        .font(strong(13.0))
                                        .color(TEXT),
                                );
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
                                    self.mark_edited_outside_history();
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

                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Escala exata de impressão")
                                .font(strong(13.0))
                                .color(TEXT),
                        );
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for scale in DrawingScale::STANDARD {
                                if toggle_pill(
                                    ui,
                                    &scale.label(),
                                    self.project.sheet.scale == scale,
                                    &format!(
                                        "1,00 m real = {:.1} mm no papel",
                                        scale.mm_per_meter()
                                    ),
                                )
                                .clicked()
                                {
                                    let prev = self.project.clone();
                                    self.project.sheet.scale = scale;
                                    self.record_change(prev);
                                }
                            }
                            if let Some((min, max)) = self.project.bounds() {
                                let (draw_w, draw_h) = self.project.sheet.drawing_area_mm();
                                let plan_w = (max.x - min.x).max(0.5);
                                let plan_h = (max.y - min.y).max(0.5);
                                if secondary_button(ui, "Melhor escala").clicked() {
                                    let best =
                                        DrawingScale::best_fit(plan_w, plan_h, draw_w, draw_h);
                                    let prev = self.project.clone();
                                    self.project.sheet.scale = best;
                                    self.record_change(prev);
                                }
                            }
                        });
                        if let Some((min, max)) = self.project.bounds() {
                            let (draw_w, draw_h) = self.project.sheet.drawing_area_mm();
                            let plan_w = (max.x - min.x).max(0.5);
                            let plan_h = (max.y - min.y).max(0.5);
                            let fits = self.project.sheet.scale.fits(plan_w, plan_h, draw_w, draw_h);
                            ui.add_space(4.0);
                            let (status, color) = if fits {
                                (
                                    format!(
                                        "A planta ({:.1} × {:.1} m) cabe na folha {} em escala {}.",
                                        plan_w,
                                        plan_h,
                                        self.project.sheet.size.short_name(),
                                        self.project.sheet.scale.label()
                                    )
                                    .replace('.', ","),
                                    SUCCESS,
                                )
                            } else {
                                (
                                    format!(
                                        "Atenção: a planta excede a folha {} em {}. Use uma folha maior ou clique em Melhor escala.",
                                        self.project.sheet.size.short_name(),
                                        self.project.sheet.scale.label()
                                    ),
                                    WARNING,
                                )
                            };
                            ui.label(RichText::new(status).size(12.0).color(color));
                        }
                    });

                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Carimbo, identificação, legenda e paginação")
                                .font(strong(13.0))
                                .color(TEXT),
                        );
                        ui.add_space(6.0);
                        let tb = &mut self.project.sheet.title_block;
                        let mut changed = false;
                        egui::Grid::new("title_block_grid")
                            .num_columns(4)
                            .spacing([8.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Desenho:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.drawing_title)
                                            .desired_width(170.0),
                                    )
                                    .changed();
                                ui.label(RichText::new("Prancha:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.sheet_number)
                                            .desired_width(80.0)
                                            .hint_text("01/03"),
                                    )
                                    .changed();
                                ui.end_row();

                                ui.label(RichText::new("Cliente:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.client)
                                            .desired_width(170.0),
                                    )
                                    .changed();
                                ui.label(RichText::new("Revisão:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.revision)
                                            .desired_width(80.0)
                                            .hint_text("R00"),
                                    )
                                    .changed();
                                ui.end_row();

                                ui.label(RichText::new("Responsável:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.author)
                                            .desired_width(170.0),
                                    )
                                    .changed();
                                ui.label(RichText::new("CAU/CREA:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.registration)
                                            .desired_width(80.0),
                                    )
                                    .changed();
                                ui.end_row();

                                ui.label(RichText::new("Endereço:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.address)
                                            .desired_width(170.0),
                                    )
                                    .changed();
                                ui.label(RichText::new("Data:").size(12.0).color(TEXT_2));
                                changed |= ui
                                    .add(
                                        egui::TextEdit::singleline(&mut tb.date)
                                            .desired_width(80.0)
                                            .hint_text("19/11/2026"),
                                    )
                                    .changed();
                                ui.end_row();
                            });
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            changed |= toggle_pill(
                                ui,
                                "Carimbo",
                                tb.show_title_block,
                                "Incluir carimbo ABNT no canto inferior direito",
                            )
                            .clicked()
                            .then(|| tb.show_title_block = !tb.show_title_block)
                            .is_some();
                            changed |= toggle_pill(
                                ui,
                                "Legenda",
                                tb.show_legend,
                                "Incluir legenda de símbolos técnicos",
                            )
                            .clicked()
                            .then(|| tb.show_legend = !tb.show_legend)
                            .is_some();
                            changed |= toggle_pill(
                                ui,
                                "Norte",
                                tb.show_north,
                                "Incluir indicação do Norte",
                            )
                            .clicked()
                            .then(|| tb.show_north = !tb.show_north)
                            .is_some();
                            changed |= toggle_pill(
                                ui,
                                "Escala gráfica",
                                tb.show_scale_bar,
                                "Incluir barra de escala gráfica proporcional",
                            )
                            .clicked()
                            .then(|| tb.show_scale_bar = !tb.show_scale_bar)
                            .is_some();
                        });
                        if changed {
                            self.mark_edited_outside_history();
                        }
                    });

                        ui.add_space(10.0);
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Estilo de documentação (Normas Brasileiras)")
                                        .font(strong(13.0))
                                        .color(TEXT),
                                );
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    for preset in DocStylePreset::ALL {
                                        if toggle_pill(
                                            ui,
                                            preset.label(),
                                            self.project.doc_style.preset == preset,
                                            preset.label(),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.doc_style = DocStyle::from_preset(preset);
                                            self.record_change(prev);
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Terminal de cota:").size(12.0).color(TEXT_2));
                                    for tick in DimensionTick::ALL {
                                        if toggle_pill(
                                            ui,
                                            tick.label(),
                                            self.project.doc_style.tick == tick,
                                            tick.label(),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.doc_style.tick = tick;
                                            self.record_change(prev);
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Unidade das cotas:").size(12.0).color(TEXT_2));
                                    for unit in [DimensionUnit::Centimetres, DimensionUnit::Metres] {
                                        if toggle_pill(
                                            ui,
                                            unit.label(),
                                            self.project.doc_style.unit == unit,
                                            unit.label(),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.doc_style.unit = unit;
                                            self.record_change(prev);
                                        }
                                    }
                                    if ui
                                        .checkbox(
                                            &mut self.project.doc_style.monochrome,
                                            "Monocromático (P&B)",
                                        )
                                        .changed()
                                    {
                                        self.mark_edited_outside_history();
                                    }
                                });
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(NBR_6492_DISCLAIMER)
                                        .size(11.0)
                                        .color(MUTED),
                                );
                            });
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Imprimir em escala...").clicked() {
                        self.open_print_dialog();
                    }
                    let multi_tab = self.project_tab_count(self.active_project_id) > 1;
                    let pdf_sheet_label = if multi_tab {
                        "Exportar PDF (todas as abas)"
                    } else {
                        "Exportar PDF (escala exata)"
                    };
                    if secondary_button(ui, pdf_sheet_label).clicked() {
                        self.export_sheet_pdf();
                    }
                    if multi_tab && secondary_button(ui, "PDF (aba atual)").clicked() {
                        self.export_active_tab_sheet_pdf();
                    }
                    if secondary_button(ui, "Exportar SVG").clicked() {
                        self.export_sheet_svg();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
