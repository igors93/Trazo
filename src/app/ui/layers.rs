//! Layer manager dialog: colour, pen weight, line type, visibility and lock.

use eframe::egui::{self, Color32, Margin, RichText, Stroke, vec2};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{danger_button, primary_button, secondary_button, toggle_pill};
use crate::domain::layers::LineType;

const COLOR_PALETTE: [(&str, [u8; 3]); 7] = [
    ("Grafite", [29, 40, 54]),
    ("Azul", [40, 120, 210]),
    ("Vermelho", [198, 56, 48]),
    ("Verde", [38, 138, 86]),
    ("Âmbar", [196, 124, 24]),
    ("Roxo", [126, 74, 178]),
    ("Cinza", [98, 116, 137]),
];

const PEN_WEIGHTS: [f64; 6] = [0.13, 0.18, 0.20, 0.25, 0.35, 0.50];

impl FloorPlanApp {
    pub(super) fn layers_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Layers {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("layers_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(640.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Plan,
                    "Camadas do projeto",
                    "Configure cor, espessura (mm), tipo de linha, visibilidade e bloqueio.",
                );

                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        let ids: Vec<u64> = self.project.layers.iter().map(|l| l.id).collect();
                        for id in ids {
                            let Some(layer) = self.project.layer(id).cloned() else {
                                continue;
                            };
                            egui::Frame::new()
                                .fill(PANEL_ALT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(6)
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let [r, g, b] = layer.color;
                                        let (swatch, swatch_resp) = ui.allocate_exact_size(
                                            vec2(18.0, 18.0),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            swatch,
                                            4.0,
                                            Color32::from_rgb(r, g, b),
                                        );
                                        if swatch_resp
                                            .on_hover_text("Clique para alternar a cor da camada")
                                            .clicked()
                                        {
                                            let next_idx = COLOR_PALETTE
                                                .iter()
                                                .position(|(_, c)| *c == layer.color)
                                                .map(|i| (i + 1) % COLOR_PALETTE.len())
                                                .unwrap_or(0);
                                            let prev = self.project.clone();
                                            if self
                                                .project
                                                .update_layer(
                                                    id,
                                                    &layer.name,
                                                    COLOR_PALETTE[next_idx].1,
                                                    layer.line_weight_mm,
                                                    layer.line_type,
                                                )
                                                .is_ok()
                                            {
                                                self.record_change(prev);
                                            }
                                        }

                                        ui.add_sized(
                                            vec2(126.0, 20.0),
                                            egui::Label::new(
                                                RichText::new(&layer.name)
                                                    .font(strong(12.5))
                                                    .color(TEXT),
                                            )
                                            .truncate(),
                                        );

                                        if toggle_pill(
                                            ui,
                                            if layer.visible { "Visível" } else { "Oculta" },
                                            layer.visible,
                                            "Mostrar ou ocultar elementos desta camada",
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.toggle_layer_visibility(id);
                                            self.record_change(prev);
                                        }

                                        if toggle_pill(
                                            ui,
                                            if layer.locked { "Bloqueada" } else { "Livre" },
                                            layer.locked,
                                            "Bloquear edição e exclusão de elementos nesta camada",
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.toggle_layer_lock(id);
                                            self.record_change(prev);
                                        }

                                        egui::ComboBox::from_id_salt(("layer_weight", id))
                                            .width(72.0)
                                            .selected_text(
                                                RichText::new(
                                                    format!("{:.2} mm", layer.line_weight_mm)
                                                        .replace('.', ","),
                                                )
                                                .size(11.5),
                                            )
                                            .show_ui(ui, |ui| {
                                                for w in PEN_WEIGHTS {
                                                    let label =
                                                        format!("{w:.2} mm").replace('.', ",");
                                                    if ui
                                                        .selectable_label(
                                                            (layer.line_weight_mm - w).abs() < 1e-3,
                                                            label,
                                                        )
                                                        .clicked()
                                                    {
                                                        let prev = self.project.clone();
                                                        if self
                                                            .project
                                                            .update_layer(
                                                                id,
                                                                &layer.name,
                                                                layer.color,
                                                                w,
                                                                layer.line_type,
                                                            )
                                                            .is_ok()
                                                        {
                                                            self.record_change(prev);
                                                        }
                                                    }
                                                }
                                            });

                                        egui::ComboBox::from_id_salt(("layer_line_type", id))
                                            .width(96.0)
                                            .selected_text(
                                                RichText::new(layer.line_type.label()).size(11.5),
                                            )
                                            .show_ui(ui, |ui| {
                                                for lt in LineType::ALL {
                                                    if ui
                                                        .selectable_label(
                                                            layer.line_type == lt,
                                                            lt.label(),
                                                        )
                                                        .clicked()
                                                    {
                                                        let prev = self.project.clone();
                                                        if self
                                                            .project
                                                            .update_layer(
                                                                id,
                                                                &layer.name,
                                                                layer.color,
                                                                layer.line_weight_mm,
                                                                lt,
                                                            )
                                                            .is_ok()
                                                        {
                                                            self.record_change(prev);
                                                        }
                                                    }
                                                }
                                            });

                                        if layer.category.is_none()
                                            && danger_button(ui, "Excluir").clicked()
                                        {
                                            let prev = self.project.clone();
                                            if self.project.delete_layer(id).is_ok() {
                                                self.record_change(prev);
                                            }
                                        }
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nova camada:").size(12.0).color(TEXT_2));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_layer_name)
                            .desired_width(190.0)
                            .hint_text("Ex.: Elétrica, Hidráulica..."),
                    );
                    if secondary_button(ui, "Adicionar camada").clicked() {
                        let name = self.new_layer_name.trim().to_owned();
                        let prev = self.project.clone();
                        match self.project.add_layer(
                            &name,
                            COLOR_PALETTE[1].1,
                            0.20,
                            LineType::Solid,
                        ) {
                            Ok(_) => {
                                self.record_change(prev);
                                self.new_layer_name.clear();
                                self.message = format!("Camada \"{name}\" criada.");
                            }
                            Err(err) => self.message = err,
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Fechar").clicked() {
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
