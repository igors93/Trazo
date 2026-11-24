use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::domain::{samples::SampleProject, templates::ProjectTemplate};

impl FloorPlanApp {
    pub(super) fn templates_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Templates {
            return;
        }
        let mut close = false;
        let mut chosen: Option<ProjectTemplate> = None;
        let mut chosen_sample: Option<SampleProject> = None;
        let mut chosen_user_template: Option<std::path::PathBuf> = None;
        let user_tpls: Vec<std::path::PathBuf> = self.user_templates.shown().to_vec();

        let response = egui::Modal::new(egui::Id::unique("templates_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(580.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Plan,
                    "Modelos e projetos de exemplo",
                    "Escolha um modelo técnico ou abra um dos projetos prontos da pasta examples/.",
                );

                egui::ScrollArea::vertical()
                    .max_height(430.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if !user_tpls.is_empty() {
                            ui.label(
                                RichText::new("MEUS MODELOS (.TRAZOT)")
                                    .font(strong(11.0))
                                    .color(MUTED),
                            );
                            ui.add_space(6.0);
                            for tpl_path in &user_tpls {
                                let title = tpl_path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "Modelo".to_owned());
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(8)
                                    .inner_margin(Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&title)
                                                        .font(strong(13.0))
                                                        .color(TEXT),
                                                );
                                                ui.label(
                                                    RichText::new(tpl_path.display().to_string())
                                                        .size(11.0)
                                                        .color(MUTED),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if primary_button(ui, "Novo deste modelo")
                                                        .clicked()
                                                    {
                                                        chosen_user_template =
                                                            Some(tpl_path.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(8.0);
                            }
                            ui.add_space(6.0);
                        }

                        ui.label(
                            RichText::new("PROJETOS DE EXEMPLO")
                                .font(strong(11.0))
                                .color(MUTED),
                        );
                        ui.add_space(6.0);
                        for sample in SampleProject::ALL {
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
                                                    RichText::new(sample.title())
                                                        .font(strong(13.0))
                                                        .color(TEXT),
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "· examples/{}",
                                                        sample.file_name()
                                                    ))
                                                    .size(11.5)
                                                    .color(ACCENT),
                                                );
                                            });
                                            ui.add_space(2.0);
                                            ui.label(
                                                RichText::new(sample.summary())
                                                    .size(11.5)
                                                    .color(TEXT_2),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if primary_button(ui, "Abrir exemplo").clicked() {
                                                    chosen_sample = Some(sample);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(8.0);
                        }

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("MODELOS RÁPIDOS")
                                .font(strong(11.0))
                                .color(MUTED),
                        );
                        ui.add_space(6.0);
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
                                                    RichText::new(format!(
                                                        "· {}",
                                                        template.category()
                                                    ))
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
                    });

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

        if let Some(tpl_path) = chosen_user_template {
            let _ = self.open_in_tab(&tpl_path);
            self.show_start_page = false;
            self.active_doc_dialog = DocDialog::None;
        } else if let Some(sample) = chosen_sample {
            let prev = self.project.clone();
            self.project = sample.load_embedded();
            self.clear_selection();
            self.file_path = None;
            self.record_change(prev);
            self.fit_view_requested();
            self.show_start_page = false;
            self.message = format!("Projeto de exemplo carregado: {}", sample.title());
            self.active_doc_dialog = DocDialog::None;
        } else if let Some(template) = chosen {
            let prev = self.project.clone();
            self.project = template.build();
            self.clear_selection();
            self.file_path = None;
            self.record_change(prev);
            self.fit_view_requested();
            self.show_start_page = false;
            self.message = format!("Modelo carregado: {}", template.title());
            self.active_doc_dialog = DocDialog::None;
        } else if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
