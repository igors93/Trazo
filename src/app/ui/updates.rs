use super::super::{FloorPlanApp, theme::*};
use super::dialogs::{backdrop, dialog_frame, dialog_title};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::io::{crash_log, updates};
use eframe::egui::{self, Margin, RichText, ScrollArea, Stroke};

impl FloorPlanApp {
    pub(super) fn updates_dialog(&mut self, context: &egui::Context) {
        if !self.show_updates_dialog {
            return;
        }
        let mut close = false;
        let mut trigger_check = false;

        egui::Modal::new(egui::Id::unique("updates_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(context, |ui| {
                ui.set_width(480.0);
                dialog_title(
                    ui,
                    Icon::Update,
                    "Atualizações do Trazo",
                    &format!("Versão instalada: v{}", crash_log::app_version()),
                );

                match &self.update_status {
                    updates::UpdateStatus::Idle => {
                        ui.label(
                            RichText::new(
                                "Clique em \"Verificar agora\" para consultar a versão mais \
                                 recente publicada no GitHub.",
                            )
                            .color(TEXT_2),
                        );
                    }
                    updates::UpdateStatus::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Consultando novas versões no GitHub...")
                                    .color(TEXT_2),
                            );
                        });
                    }
                    updates::UpdateStatus::UpToDate => {
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Você já está usando a versão mais recente.")
                                        .font(strong(13.0))
                                        .color(SUCCESS),
                                );
                            });
                    }
                    updates::UpdateStatus::Available(release) => {
                        let release_url = release.url.clone();
                        egui::Frame::new()
                            .fill(ACCENT_SOFTER)
                            .stroke(Stroke::new(1.0, ACCENT))
                            .corner_radius(8)
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Nova versão disponível: {}",
                                        release.tag
                                    ))
                                    .font(strong(14.0))
                                    .color(ACCENT_TEXT),
                                );
                                if !release.notes.is_empty() {
                                    ui.add_space(6.0);
                                    ScrollArea::vertical()
                                        .max_height(150.0)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&release.notes)
                                                    .font(regular(12.0))
                                                    .color(TEXT_2),
                                            );
                                        });
                                }
                                ui.add_space(8.0);
                                if primary_button(ui, "Baixar nova versão").clicked() {
                                    context.open_url(egui::OpenUrl::new_tab(release_url));
                                }
                            });
                    }
                    updates::UpdateStatus::Failed(error) => {
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.label(RichText::new(error).color(WARNING));
                            });
                    }
                }

                ui.add_space(12.0);
                ui.checkbox(
                    &mut self.check_updates_on_start,
                    "Verificar atualizações automaticamente ao iniciar",
                );

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Abrir página de versões").clicked() {
                        context.open_url(egui::OpenUrl::new_tab(updates::RELEASES_PAGE_URL));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let checking =
                            matches!(self.update_status, updates::UpdateStatus::Checking);
                        if ui
                            .add_enabled_ui(!checking, |ui| primary_button(ui, "Verificar agora"))
                            .inner
                            .clicked()
                        {
                            trigger_check = true;
                        }
                        if secondary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });

                if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });

        if trigger_check {
            self.start_update_check(context);
        }
        if close {
            self.show_updates_dialog = false;
        }
    }
}
