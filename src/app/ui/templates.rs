use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::domain::templates::ProjectTemplate;

impl FloorPlanApp {
    pub(super) fn templates_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Templates {
            return;
        }
        let mut close = false;
        let mut chosen: Option<ProjectTemplate> = None;

        let response = egui::Modal::new(egui::Id::unique("templates_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(580.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Plan,
                    "Modelos de projeto",
                    "Escolha um ponto de partida pronto para planta baixa, implantação ou detalhamento.",
                );

                for template in ProjectTemplate::ALL {
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(template.title())
                                                .font(strong(13.0))
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new(format!("· {}", template.category()))
                                                .size(11.5)
                                                .color(ACCENT),
                                        );
                                    });
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new(template.summary())
                                            .size(11.5)
                                            .color(TEXT_2),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if primary_button(ui, "Usar modelo").clicked() {
                                            chosen = Some(template);
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(8.0);
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(
                            "O projeto atual será substituído, podendo ser recuperado com Desfazer (Ctrl+Z).",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });
            });

        if let Some(template) = chosen {
            let prev = self.project.clone();
            self.project = template.build();
            self.clear_selection();
            self.file_path = None;
            self.record_change(prev);
            self.fit_view_requested();
            self.message = format!("Modelo carregado: {}", template.title());
            self.active_doc_dialog = DocDialog::None;
        } else if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
