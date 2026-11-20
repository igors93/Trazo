use eframe::egui::{self, RichText};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use crate::domain::schedules::{active_schedules, schedules_csv};

impl FloorPlanApp {
    pub(super) fn schedules_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Schedules {
            return;
        }
        let mut open = true;
        let mut changed = false;

        egui::Window::new("Quadros de Áreas, Esquadrias, Ambientes e Materiais")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .default_height(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Configure quais quadros técnicos serão exibidos e incluídos na prancha exportada.",
                    )
                    .font(regular(12.0))
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_area_schedule,
                            "Quadro de áreas",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_rooms_schedule,
                            "Quadro de ambientes",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_openings_schedule,
                            "Quadro de esquadrias",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_materials_schedule,
                            "Quadro de materiais",
                        )
                        .changed();
                });

                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_perimeter,
                            "Coluna perímetro",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(&mut self.project.schedules.show_sill, "Coluna peitoril")
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.show_occupancy,
                            "Taxa de ocupação",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.project.schedules.include_on_sheet,
                            "Incluir na prancha (SVG/PDF)",
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Perda estimada de blocos (%):");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut self.project.schedules.waste_percent)
                                .range(0.0..=50.0)
                                .speed(0.5)
                                .suffix("%"),
                        )
                        .changed();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Exportar quadros (CSV)...").clicked() {
                            self.export_configured_schedules_csv();
                        }
                    });
                });

                ui.separator();

                let tables = active_schedules(&self.project, &self.project.schedules);
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .show(ui, |ui| {
                        if tables.is_empty() {
                            ui.label(
                                RichText::new("Nenhum quadro ativo para o projeto atual.")
                                    .color(MUTED),
                            );
                        }
                        for (idx, table) in tables.iter().enumerate() {
                            ui.label(RichText::new(&table.title).font(strong(13.0)).color(TEXT));
                            egui::Grid::new(format!("schedule_table_{idx}"))
                                .striped(true)
                                .min_col_width(90.0)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    for h in &table.headers {
                                        ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                                    }
                                    ui.end_row();
                                    for row in &table.rows {
                                        for cell in row {
                                            ui.label(RichText::new(cell).font(regular(11.5)));
                                        }
                                        ui.end_row();
                                    }
                                    if let Some(footer) = &table.footer {
                                        for cell in footer {
                                            ui.label(RichText::new(cell).font(strong(11.5)));
                                        }
                                        ui.end_row();
                                    }
                                });
                            ui.add_space(10.0);
                        }
                    });
            });

        if changed {
            self.mark_edited_outside_history();
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }

    fn export_configured_schedules_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planilha CSV", &["csv"])
            .set_file_name("quadros.csv")
            .save_file()
        else {
            return;
        };
        let csv = schedules_csv(&self.project, &self.project.schedules);
        match std::fs::write(&path, format!("\u{FEFF}{csv}")) {
            Ok(()) => self.message = format!("Quadros exportados em {}", path.display()),
            Err(error) => self.message = format!("Erro ao exportar quadros: {error}"),
        }
    }
}
