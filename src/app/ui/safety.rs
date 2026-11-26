//! The "Segurança" window (#101): every engineering check of the project as
//! a traffic light, worst first, with its margin, what to change in plain
//! words and, on request, the technical detail and the assumptions.

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::verdict::{Light, SafetySummary, safety_summary};

pub(crate) fn light_color(light: Light) -> Color32 {
    match light {
        Light::Green => SUCCESS,
        Light::Yellow => WARNING,
        Light::Red => DANGER,
    }
}

/// A round light with its colour.
pub(crate) fn light_dot(ui: &mut Ui, light: Light) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 5.5, light_color(light));
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

impl FloorPlanApp {
    pub(super) fn safety_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Safety {
            return;
        }
        let mut open = true;
        let detail_id = egui::Id::unique("trazo_safety_detail");
        let filter_id = egui::Id::unique("trazo_safety_filter");
        let mut detail: bool = ctx.data(|d| d.get_temp(detail_id)).unwrap_or(false);
        let mut filter: u8 = ctx.data(|d| d.get_temp(filter_id)).unwrap_or(0);
        let summary: SafetySummary = safety_summary(&self.project);
        let mut export = false;
        let mut hypothesis_action = None;
        let mut help_contacts = None;
        let urgent = summary.red > 0;
        egui::Window::new("Segurança · semáforo dos resultados")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(760.0)
            .default_height(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Cada verificação de engenharia em verde (folga confortável), amarelo (passa com pouca folga) ou vermelho (não passa). Pré-dimensionamento: confirme sempre com um profissional habilitado.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                if let Some(worst) = summary.worst() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        light_dot(ui, worst);
                        ui.label(
                            RichText::new(format!("Situação geral: {}", worst.meaning()))
                                .font(strong(13.5))
                                .color(light_color(worst)),
                        );
                    });
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (light, count) in [
                        (Light::Red, summary.red),
                        (Light::Yellow, summary.yellow),
                        (Light::Green, summary.green),
                    ] {
                        light_dot(ui, light);
                        ui.label(
                            RichText::new(format!("{count} {}", light.label().to_lowercase()))
                                .font(strong(13.0))
                                .color(light_color(light)),
                        );
                        ui.add_space(8.0);
                    }
                    ui.separator();
                    for (index, title) in ["Todos", "Só vermelhos e amarelos"].iter().enumerate() {
                        if ui.selectable_label(filter == index as u8, *title).clicked() {
                            filter = index as u8;
                        }
                    }
                    ui.separator();
                    ui.checkbox(&mut detail, "Detalhes técnicos");
                    ui.separator();
                    export = secondary_button(ui, "Exportar memorial (PDF)").clicked();
                });
                if summary.verdicts.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Nada para verificar ainda: gere a estrutura (aba Estrutura) ou distribua os circuitos (aba Elétrica).",
                        )
                        .color(MUTED),
                    );
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Who set each hypothesis, what Trazo contests and what the
                    // assistant proposed (#168, #169, #171).
                    hypothesis_action = super::hypotheses::show(ui, &self.project);
                    // Where to find help, open whenever something is red (#159).
                    help_contacts = super::help_panel::show(ui, &self.project, urgent);
                    ui.add_space(6.0);
                    for (index, verdict) in summary.verdicts.iter().enumerate() {
                        let light = verdict.light();
                        if filter == 1 && light == Light::Green {
                            continue;
                        }
                        let governing = verdict.governing();
                        let header = format!(
                            "{} · {}{}",
                            verdict.discipline.label(),
                            verdict.element,
                            governing.map_or(String::new(), |g| format!(
                                " · {} {}",
                                g.name.to_lowercase(),
                                if g.utilisation.is_finite() {
                                    format!("({} % de folga)", decimal(g.margin_percent(), 0))
                                } else {
                                    "(falha)".to_owned()
                                }
                            ))
                        );
                        egui::CollapsingHeader::new(
                            RichText::new(header).color(if light == Light::Green {
                                TEXT
                            } else {
                                light_color(light)
                            }),
                        )
                        .id_salt(("safety_verdict", index))
                        .default_open(light == Light::Red)
                        .show(ui, |ui| {
                            for check in &verdict.checks {
                                ui.horizontal_wrapped(|ui| {
                                    light_dot(ui, check.light);
                                    ui.label(RichText::new(&check.name).font(strong(12.5)));
                                    if check.utilisation.is_finite() {
                                        ui.label(
                                            RichText::new(format!(
                                                "uso {} % · folga {} %",
                                                decimal(check.utilisation * 100.0, 0),
                                                decimal(check.margin_percent(), 0)
                                            ))
                                            .color(MUTED),
                                        );
                                    }
                                    ui.label(&check.simple);
                                });
                                if detail {
                                    ui.label(
                                        RichText::new(&check.detail)
                                            .size(11.0)
                                            .color(Color32::from_rgb(80, 96, 112)),
                                    );
                                }
                            }
                        });
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new("Premissas").font(strong(13.0)));
                    for assumption in &summary.assumptions {
                        ui.label(RichText::new(format!("• {assumption}")).size(11.5).color(MUTED));
                    }
                });
            });
        ctx.data_mut(|d| {
            d.insert_temp(detail_id, detail);
            d.insert_temp(filter_id, filter);
        });
        if export {
            self.export_presizing_report();
        }
        if let Some(action) = hypothesis_action {
            self.apply_hypothesis_action(action);
        }
        if let Some(contacts) = help_contacts {
            self.save_help_contacts(contacts);
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }

    /// Saves the pre-sizing report (#103) for a professional to review.
    pub(crate) fn export_presizing_report(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name("memorial-pre-dimensionamento.pdf")
            .save_file()
        else {
            return;
        };
        let date = crate::io::report::today();
        self.message = match crate::io::report::export_presizing_report(&self.project, &date, &path)
        {
            Ok(pages) => format!(
                "Memorial de pré-dimensionamento ({pages} páginas) salvo em {}",
                path.display()
            ),
            Err(error) => format!("Erro ao exportar o memorial: {error}"),
        };
    }
}
