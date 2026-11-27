//! The "Quantitativos e planejamento de obra" window: tabbed hub uniting
//! masonry/finishes takeoff (#56) and Section H construction planning
//! (materials catalogue #135, stage shopping list #136, cost estimate & SINAPI
//! #87, schedule #137, site safety & permits #138 and site progress log #139).

use eframe::egui::{self, RichText};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::quantities::{takeoff_csv_for, takeoff_sheet_svg, takeoff_tables_for};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ConstructionHubTab {
    #[default]
    Takeoff,
    Materials,
}

impl FloorPlanApp {
    pub(super) fn quantities_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Quantities {
            return;
        }
        let mut open = true;
        let mut export_csv = false;
        let mut export_sheet: Option<bool> = None; // Some(true) = PDF, Some(false) = SVG
        let mut settings_changed = false;

        let tab_id = egui::Id::new("trazo_construction_hub_tab");
        let mut active_tab: ConstructionHubTab =
            ctx.data(|data| data.get_temp(tab_id)).unwrap_or_default();

        let scope_id = egui::Id::unique("trazo_quantities_all_floors");
        let multi_floor = self.project.floors.levels.len() > 1;
        let mut all_floors: bool = ctx
            .data(|data| data.get_temp(scope_id))
            .unwrap_or(multi_floor);
        if !multi_floor {
            all_floors = false;
        }

        egui::Window::new("Quantitativos e planejamento de obra")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(920.0)
            .default_height(640.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut active_tab,
                        ConstructionHubTab::Takeoff,
                        "Quantitativos",
                    );
                    ui.selectable_value(
                        &mut active_tab,
                        ConstructionHubTab::Materials,
                        "Catálogo de materiais",
                    );
                });
                ui.separator();

                match active_tab {
                    ConstructionHubTab::Takeoff => {
                        let takeoff = self
                            .project
                            .construction_takeoff_with_options(all_floors, None);
                        let tables = takeoff_tables_for(&takeoff);
                        ui.label(
                            RichText::new(
                                "Alvenaria (blocos com perda, meios-blocos e canaletas), vergas/contravergas por abertura, argamassa de assentamento, reboco/estuco, graute e acabamentos por cômodo.",
                            )
                            .size(12.0)
                            .color(TEXT_2),
                        );
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Perda blocos:");
                            settings_changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.project.schedules.waste_percent)
                                        .range(0.0..=50.0)
                                        .speed(0.5)
                                        .max_decimals(0)
                                        .suffix("%"),
                                )
                                .changed();
                            if multi_floor {
                                ui.separator();
                                ui.selectable_value(&mut all_floors, false, "Pavimento ativo");
                                ui.selectable_value(
                                    &mut all_floors,
                                    true,
                                    format!("Todos ({} pav.)", self.project.floors.levels.len()),
                                );
                            }
                            ui.separator();
                            settings_changed |= ui
                                .checkbox(
                                    &mut self.project.schedules.show_quantities_schedule,
                                    "Incluir na prancha (SVG/PDF)",
                                )
                                .changed();
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            export_csv = secondary_button(ui, "Exportar CSV").clicked();
                            if secondary_button(ui, "Exportar PDF").clicked() {
                                export_sheet = Some(true);
                            }
                            if secondary_button(ui, "Exportar SVG").clicked() {
                                export_sheet = Some(false);
                            }
                        });
                        ui.separator();
                        egui::ScrollArea::both().show(ui, |ui| {
                            for (index, table) in tables.iter().enumerate() {
                                ui.add_space(6.0);
                                ui.label(RichText::new(&table.title).font(strong(13.0)));
                                if table.rows.is_empty() {
                                    ui.label(RichText::new("Nada a medir.").color(MUTED));
                                    continue;
                                }
                                egui::Grid::new(("takeoff_table", index))
                                    .striped(true)
                                    .spacing([12.0, 5.0])
                                    .show(ui, |ui| {
                                        for header in &table.headers {
                                            ui.label(
                                                RichText::new(header)
                                                    .font(strong(12.0))
                                                    .color(TEXT_2),
                                            );
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
                                    "Meios-blocos para a amarração nas pontas das paredes e nos vãos; canaletas na cinta de amarração, vergas e contravergas (30 cm de transpasse de cada lado). Argamassa, reboco/estuco e graute com 10 % de perda; piso e paredes internas com +10 % de perda. Método em docs/engineering.md.",
                                )
                                .size(11.0)
                                .color(MUTED),
                            );
                        });
                    }
                    ConstructionHubTab::Materials => {
                        self.materials_catalogue_tab(ui);
                    }
                }
            });

        ctx.data_mut(|data| {
            data.insert_temp(tab_id, active_tab);
            data.insert_temp(scope_id, all_floors);
        });

        if settings_changed {
            self.mark_edited_outside_history();
        }
        if export_csv
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Planilha CSV", &["csv"])
                .set_file_name("quantitativos.csv")
                .save_file()
        {
            let refreshed = self
                .project
                .construction_takeoff_with_options(all_floors, None);
            let csv = takeoff_csv_for(&refreshed);
            self.message = match std::fs::write(&path, format!("\u{FEFF}{csv}")) {
                Ok(()) => format!("Quantitativos exportados em {}", path.display()),
                Err(error) => format!("Erro ao exportar os quantitativos: {error}"),
            };
        }
        if let Some(pdf) = export_sheet {
            let (label, ext) = if pdf {
                ("Documento PDF", "pdf")
            } else {
                ("Desenho SVG", "svg")
            };
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(label, &[ext])
                .set_file_name(format!("quantitativos-obra.{ext}"))
                .save_file()
            {
                let res = if pdf {
                    crate::io::quantities::export_pdf(&self.project, all_floors, &path)
                } else {
                    let markup = takeoff_sheet_svg(&self.project, all_floors);
                    crate::domain::views::export_view_svg(&markup, &path)
                };
                self.message = match res {
                    Ok(()) => format!("Planilha de quantitativos exportada em {}", path.display()),
                    Err(error) => format!("Erro ao exportar a planilha de quantitativos: {error}"),
                };
            }
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
