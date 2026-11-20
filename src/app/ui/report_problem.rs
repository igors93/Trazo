use super::super::{FloorPlanApp, theme::*};
use super::dialogs::{backdrop, dialog_frame, dialog_title};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::io::crash_log;
use eframe::egui::{self, Margin, RichText, ScrollArea, Stroke};

impl FloorPlanApp {
    pub(super) fn report_problem_dialog(&mut self, context: &egui::Context) {
        if !self.show_report_dialog {
            return;
        }
        let mut close = false;
        let recent_log = crash_log::read_recent_log(6_000);
        let has_log = recent_log.is_some();

        egui::Modal::new(egui::Id::unique("report_problem_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(context, |ui| {
                ui.set_width(520.0);
                dialog_title(
                    ui,
                    Icon::Info,
                    "Relatar problema",
                    "Envie um relato no GitHub já preenchido com versão, sistema e log de falha.",
                );

                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Trazo v{}", crash_log::app_version()))
                                    .font(strong(12.5))
                                    .color(TEXT),
                            );
                            ui.label(RichText::new("·").color(MUTED));
                            ui.label(
                                RichText::new(crash_log::os_summary())
                                    .font(regular(12.5))
                                    .color(TEXT_2),
                            );
                        });
                        ui.label(
                            RichText::new(format!(
                                "Log de falhas: {}",
                                crash_log::crash_log_path().display()
                            ))
                            .font(regular(11.5))
                            .color(MUTED),
                        );
                    });

                ui.add_space(12.0);
                ui.label(
                    RichText::new("Resumo do problema (opcional)")
                        .font(strong(12.5))
                        .color(TEXT_2),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.report_summary)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("Descreva brevemente o que aconteceu antes do erro..."),
                );

                ui.add_space(12.0);
                if let Some(excerpt) = &recent_log {
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.report_include_log,
                            "Incluir registro de falha no relatório",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if secondary_button(ui, "Limpar logs").clicked() {
                                match crash_log::clear_crash_logs() {
                                    Ok(()) => {
                                        self.message = "Logs de falha removidos.".to_owned();
                                    }
                                    Err(error) => {
                                        self.message =
                                            format!("Não foi possível limpar os logs: {error}");
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(6.0);
                    egui::Frame::new()
                        .fill(BAND)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ScrollArea::vertical()
                                .max_height(150.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(excerpt)
                                            .font(egui::FontId::monospace(11.5))
                                            .color(TEXT_2),
                                    );
                                });
                        });
                } else {
                    ui.label(
                        RichText::new(
                            "Nenhuma falha registrada ainda. Se o Trazo fechar inesperadamente, \
                             o diagnóstico aparecerá aqui automaticamente.",
                        )
                        .color(MUTED),
                    );
                }

                ui.add_space(16.0);
                let attached_log = if has_log && self.report_include_log {
                    recent_log.as_deref()
                } else {
                    None
                };

                ui.horizontal(|ui| {
                    if secondary_button(ui, "Abrir pasta de logs").clicked()
                        && let Err(error) = crash_log::open_log_folder()
                    {
                        self.message = error;
                    }
                    if secondary_button(ui, "Copiar diagnóstico").clicked() {
                        let bundle =
                            crash_log::format_diagnostic_bundle(&self.report_summary, attached_log);
                        context.copy_text(bundle);
                        self.message =
                            "Diagnóstico copiado para a área de transferência.".to_owned();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Abrir no GitHub").clicked() {
                            let url =
                                crash_log::build_bug_report_url(&self.report_summary, attached_log);
                            context.open_url(egui::OpenUrl::new_tab(url));
                            self.message = "Formulário de problema aberto no navegador.".to_owned();
                            close = true;
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

        if close {
            self.show_report_dialog = false;
        }
    }
}
