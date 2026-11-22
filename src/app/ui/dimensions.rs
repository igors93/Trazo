//! Associative dimensions manager dialog: generate internal room dimensions,
//! add aligned wall dimensions, override labels, adjust offsets and remove.

use eframe::egui::{self, Margin, RichText, Stroke, vec2};

use super::super::{DocDialog, FloorPlanApp, Selection, theme::*};
use super::icons::Icon;
use super::widgets::{danger_button, primary_button, secondary_button, toggle_pill};

impl FloorPlanApp {
    pub(super) fn dimensions_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Dimensions {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("dimensions_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(600.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Ruler,
                    "Cotas associativas",
                    "Cotas internas, externas, alinhadas e angulares que acompanham a geometria e aceitam texto editável.",
                );

                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, "Cotas internas").clicked() {
                        let prev = self.project.clone();
                        let added = self.project.generate_room_internal_dimensions();
                        if added > 0 {
                            self.record_change(prev);
                            self.message =
                                format!("{added} cota(s) interna(s) associativa(s) criada(s).");
                        } else {
                            self.message =
                                "Todos os cômodos já possuem cotas internas.".to_owned();
                        }
                    }
                    if primary_button(ui, "Cadeias de aberturas")
                        .on_hover_text(
                            "Gera cadeias de cotas associativas localizando cada porta, janela, portão e vão nas paredes.",
                        )
                        .clicked()
                    {
                        let prev = self.project.clone();
                        if let Ok(summary) = self.project.generate_opening_dimension_chains(None, 0.45)
                        {
                            if summary.dimensions_added > 0 {
                                self.record_change(prev);
                                self.message = format!(
                                    "{} cota(s) de abertura gerada(s) em {} parede(s).",
                                    summary.dimensions_added, summary.walls_processed
                                );
                            } else if summary.walls_processed == 0 {
                                self.message =
                                    "Nenhuma parede possui aberturas para cotar.".to_owned();
                            } else {
                                self.message =
                                    "Todas as paredes com aberturas já possuem cadeias de cotas."
                                        .to_owned();
                            }
                        }
                    }
                    if secondary_button(ui, "Cotas completas")
                        .on_hover_text(
                            "Gera em um clique as cotas internas de todos os cômodos e as cadeias de aberturas.",
                        )
                        .clicked()
                    {
                        let prev = self.project.clone();
                        let summary = self.project.generate_site_dimensions(0.45);
                        if summary.total_added() > 0 {
                            self.record_change(prev);
                            self.message = format!(
                                "{} cota(s) gerada(s) ({} interna(s), {} de abertura em {} parede(s)).",
                                summary.total_added(),
                                summary.internal_added,
                                summary.opening_chains.dimensions_added,
                                summary.opening_chains.walls_processed,
                            );
                        } else {
                            self.message =
                                "O projeto já possui todas as cotas internas e cadeias de aberturas."
                                    .to_owned();
                        }
                    }
                    if let Some(Selection::Wall(wall_id)) = self.selection {
                        if secondary_button(ui, "Cota da parede selecionada").clicked() {
                            let prev = self.project.clone();
                            if self
                                .project
                                .add_wall_aligned_dimension(wall_id, -0.35)
                                .is_ok()
                            {
                                self.record_change(prev);
                                self.message = "Cota alinhada adicionada à parede.".to_owned();
                            }
                        }
                        if self.project.openings.iter().any(|o| o.wall_id == wall_id)
                            && secondary_button(ui, "Cadeia da parede selecionada").clicked()
                        {
                            let prev = self.project.clone();
                            if let Ok(summary) = self
                                .project
                                .generate_opening_dimension_chains(Some(wall_id), 0.45)
                            {
                                if summary.dimensions_added > 0 {
                                    self.record_change(prev);
                                    self.message = format!(
                                        "{} cota(s) da cadeia adicionada(s) na parede.",
                                        summary.dimensions_added
                                    );
                                } else {
                                    self.message =
                                        "Esta parede já possui cadeia de aberturas.".to_owned();
                                }
                            }
                        }
                    }
                });
                ui.add_space(10.0);

                if self.project.dimensions.is_empty() {
                    ui.label(
                        RichText::new(
                            "Nenhuma cota associativa criada ainda. Gere as cotas internas dos cômodos ou selecione uma parede para cotar.",
                        )
                        .size(12.0)
                        .color(MUTED),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let ids: Vec<u64> =
                                self.project.dimensions.iter().map(|d| d.id).collect();
                            for id in ids {
                                let Some(dim) =
                                    self.project.dimensions.iter().find(|d| d.id == id).cloned()
                                else {
                                    continue;
                                };
                                let resolved = self.project.resolve_dimension(&dim);
                                let display = resolved
                                    .as_ref()
                                    .map(|r| r.label().to_owned())
                                    .unwrap_or_else(|| "—".to_owned());
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.add_sized(
                                                vec2(72.0, 20.0),
                                                egui::Label::new(
                                                    RichText::new(dim.kind.label())
                                                        .font(strong(12.0))
                                                        .color(TEXT),
                                                ),
                                            );
                                            ui.add_sized(
                                                vec2(96.0, 20.0),
                                                egui::Label::new(
                                                    RichText::new(&display)
                                                        .size(12.0)
                                                        .color(ACCENT_TEXT),
                                                )
                                                .truncate(),
                                            );
                                            if toggle_pill(
                                                ui,
                                                if dim.visible { "Visível" } else { "Oculta" },
                                                dim.visible,
                                                "Mostrar ou ocultar esta cota",
                                            )
                                            .clicked()
                                            {
                                                let prev = self.project.clone();
                                                if let Some(d) = self
                                                    .project
                                                    .dimensions
                                                    .iter_mut()
                                                    .find(|d| d.id == id)
                                                {
                                                    d.visible = !d.visible;
                                                }
                                                self.record_change(prev);
                                            }
                                            let mut text =
                                                dim.override_text.clone().unwrap_or_default();
                                            let edit = ui.add(
                                                egui::TextEdit::singleline(&mut text)
                                                    .desired_width(140.0)
                                                    .hint_text("Texto personalizado"),
                                            );
                                            if edit.changed() {
                                                let _ = self
                                                    .project
                                                    .update_dimension_override(id, Some(text));
                                                self.mark_edited_outside_history();
                                            }
                                            if secondary_button(ui, "±").on_hover_text("Deslocar linha de cota (+0,20 m)").clicked() {
                                                let next = match dim.kind {
                                                    crate::domain::dimensions::DimensionKind::Internal { offset, .. }
                                                    | crate::domain::dimensions::DimensionKind::External { offset, .. }
                                                    | crate::domain::dimensions::DimensionKind::Aligned { offset, .. } => offset + 0.20,
                                                    crate::domain::dimensions::DimensionKind::Angular { radius, .. } => radius + 0.20,
                                                };
                                                let prev = self.project.clone();
                                                if self.project.update_dimension_offset(id, next).is_ok() {
                                                    self.record_change(prev);
                                                }
                                            }
                                            if danger_button(ui, "Excluir").clicked() {
                                                let prev = self.project.clone();
                                                if self.project.delete_dimension(id) {
                                                    self.record_change(prev);
                                                }
                                            }
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                }

                ui.add_space(10.0);
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
