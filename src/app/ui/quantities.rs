//! The "Quantitativos da obra" window (#56): the consolidated takeoff of the
//! masonry, lintels and finishes, with a CSV export for the shopping list.

use eframe::egui::{self, RichText};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::quantities::{takeoff_csv, takeoff_tables};

impl FloorPlanApp {
    pub(super) fn quantities_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Quantities {
            return;
        }
        let mut open = true;
        let mut export = false;
        let tables = takeoff_tables(&self.project);
        egui::Window::new("Quantitativos da obra")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Alvenaria, vergas e acabamentos do pavimento ativo, medidos pelo eixo das paredes, com {:.0} % de perda nos blocos (ajuste em Quadros). Confira as quantidades antes de comprar.",
                        self.project.schedules.waste_percent
                    ))
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(4.0);
                export = secondary_button(ui, "Exportar CSV").clicked();
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, table) in tables.iter().enumerate() {
                        ui.add_space(6.0);
                        ui.label(RichText::new(&table.title).font(strong(13.0)));
                        if table.rows.is_empty() {
                            ui.label(RichText::new("Nada a medir.").color(MUTED));
                            continue;
                        }
                        egui::Grid::new(("takeoff_table", index))
                            .striped(true)
                            .spacing([14.0, 5.0])
                            .show(ui, |ui| {
                                for header in &table.headers {
                                    ui.label(RichText::new(header).font(strong(12.0)).color(TEXT_2));
                                }
                                ui.end_row();
                                for row in &table.rows {
                                    for cell in row {
                                        ui.label(cell);
                                    }
                                    ui.end_row();
                                }
                                if let Some(footer) = &table.footer {
                                    for cell in footer {
                                        ui.label(RichText::new(cell).font(strong(12.0)));
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Meios-blocos para a amarração nas pontas das paredes e nos vãos; canaletas na fiada da cinta de amarração e nas vergas e contravergas (30 cm de transpasse de cada lado). Argamassa e reboco com 10 % de perda. Método em docs/engineering.md.",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
        if export
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Planilha CSV", &["csv"])
                .set_file_name("quantitativos.csv")
                .save_file()
        {
            self.message = match std::fs::write(&path, takeoff_csv(&self.project)) {
                Ok(()) => format!("Quantitativos exportados em {}", path.display()),
                Err(error) => format!("Erro ao exportar os quantitativos: {error}"),
            };
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
