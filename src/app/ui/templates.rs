use eframe::egui::{self, Margin, RichText, Stroke, Ui, vec2};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::domain::{samples::SampleProject, templates::ProjectTemplate};

/// Renders one card with a constrained left text column (title, optional badge,
/// wrapped description) and a right-aligned primary action button that never
/// overlaps the text.
fn template_card(
    ui: &mut Ui,
    title: &str,
    badge: Option<&str>,
    summary: &str,
    button_label: &str,
    button_reserve: f32,
) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            let full_w = ui.available_width();
            ui.set_width(full_w);
            let gap = 14.0;
            let text_w = (full_w - button_reserve - gap).max(180.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.set_max_width(text_w);
                    ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 2.0);
                        ui.label(RichText::new(title).font(strong(13.0)).color(TEXT));
                        if let Some(badge) = badge {
                            ui.label(
                                RichText::new(format!("· {badge}"))
                                    .size(11.5)
                                    .color(ACCENT_TEXT),
                            );
                        }
                    });
                    ui.add(
                        egui::Label::new(RichText::new(summary).size(11.5).color(TEXT_2)).wrap(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, button_label).clicked() {
                        clicked = true;
                    }
                });
            });
        });
    clicked
}

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

        let viewport = ctx.content_rect();
        let dialog_w = (viewport.width() - 64.0).clamp(480.0, 650.0);
        let scroll_max_h = (viewport.height() - 200.0).clamp(260.0, 480.0);

        let response = egui::Modal::new(egui::Id::unique("templates_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(dialog_w);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Plan,
                    "Modelos e projetos de exemplo",
                    "Escolha um modelo técnico ou abra um dos projetos prontos da pasta examples/.",
                );

                egui::ScrollArea::vertical()
                    .max_height(scroll_max_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
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
                                let path_str = tpl_path.display().to_string();
                                if template_card(
                                    ui,
                                    &title,
                                    None,
                                    &path_str,
                                    "Novo deste modelo",
                                    148.0,
                                ) {
                                    chosen_user_template = Some(tpl_path.clone());
                                }
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
                            let badge = format!("examples/{}", sample.file_name());
                            if template_card(
                                ui,
                                sample.title(),
                                Some(&badge),
                                sample.summary(),
                                "Abrir exemplo",
                                124.0,
                            ) {
                                chosen_sample = Some(sample);
                            }
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
                            if template_card(
                                ui,
                                template.title(),
                                Some(template.category()),
                                template.summary(),
                                "Usar modelo",
                                124.0,
                            ) {
                                chosen = Some(template);
                            }
                            ui.add_space(8.0);
                        }
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let hint_w = (ui.available_width() - 90.0).max(180.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(hint_w);
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "O projeto atual será substituído, podendo ser recuperado com Desfazer (Ctrl+Z).",
                                )
                                .size(11.0)
                                .color(MUTED),
                            )
                            .wrap(),
                        );
                    });
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
