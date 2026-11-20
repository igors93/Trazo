use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{secondary_button, toggle_pill};
use crate::domain::{
    ergonomics::{IssueSeverity, analyze_ergonomics},
    geometry::Point,
};

impl FloorPlanApp {
    pub(super) fn ergonomics_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Ergonomics {
            return;
        }
        let mut open = true;
        let mut focus_point: Option<Point> = None;

        egui::Window::new("Verificação de Circulação, Interferências e Áreas de Uso")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .default_height(500.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Analisa colisões entre móveis e paredes, obstrução do raio de portas, áreas de uso frontal e largura livre de circulação.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(
                        &mut self.show_ergonomics,
                        "Destacar áreas de uso e interferências na planta",
                    );
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Circulação mínima:")
                            .size(12.0)
                            .color(TEXT_2),
                    );
                    for (label, value, tip_text) in [
                        ("0,60 m (Compacto)", 0.60, "Passagem residencial mínima"),
                        ("0,70 m (Residencial)", 0.70, "Conforto residencial padrão"),
                        ("0,90 m (NBR 9050)", 0.90, "Largura livre acessível ABNT NBR 9050"),
                    ] {
                        let active = (self.ergonomics_min_passage - value).abs() < 0.01;
                        if toggle_pill(ui, label, active, tip_text).clicked() {
                            self.ergonomics_min_passage = value;
                        }
                    }
                });

                ui.add_space(8.0);
                let report = analyze_ergonomics(&self.project, self.ergonomics_min_passage);
                let errors = report.error_count();
                let warnings = report.warning_count();

                let (banner_text, banner_color) = if report.issues.is_empty() {
                    (
                        format!(
                            "Tudo certo! {} área(s) de uso verificada(s) sem interferências.",
                            report.clearance_zones.len()
                        ),
                        SUCCESS,
                    )
                } else {
                    (
                        format!(
                            "Encontrados {errors} erro(s) crítico(s) e {warnings} alerta(s) de circulação/uso."
                        ),
                        if errors > 0 { DANGER } else { WARNING },
                    )
                };
                ui.label(
                    RichText::new(banner_text)
                        .font(strong(12.5))
                        .color(banner_color),
                );
                ui.add_space(6.0);
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(310.0)
                    .show(ui, |ui| {
                        for issue in &report.issues {
                            let badge_color = match issue.severity {
                                IssueSeverity::Error => DANGER,
                                IssueSeverity::Warning => WARNING,
                            };
                            egui::Frame::new()
                                .fill(PANEL_ALT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(6)
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("[{}]", issue.severity.label()))
                                                .font(strong(11.5))
                                                .color(badge_color),
                                        );
                                        ui.label(
                                            RichText::new(&issue.message)
                                                .size(12.0)
                                                .color(TEXT),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if secondary_button(ui, "Ver na planta").clicked() {
                                                    focus_point = Some(issue.location);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

        if let Some(pt) = focus_point {
            self.show_ergonomics = true;
            self.zoom_target = Some((
                Point::new(pt.x - 1.5, pt.y - 1.5),
                Point::new(pt.x + 1.5, pt.y + 1.5),
            ));
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
