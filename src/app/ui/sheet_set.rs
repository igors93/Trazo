use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::discipline::Discipline;
use crate::{
    domain::sheet_set::{SheetContent, SheetStatus, red_checks},
    io::export::sheet::{export_sheet_item_pdf, export_sheet_item_svg},
};

impl FloorPlanApp {
    pub(super) fn sheet_set_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::SheetSet {
            return;
        }
        let mut open = true;
        let mut export_item: Option<(u64, bool)> = None;
        let mut changed = false;
        let current_red_checks = red_checks(&self.project);
        let mut status_cmd: Option<(u64, SheetStatus)> = None;

        egui::Window::new("Conjunto de Pranchas e Controle de Revisões")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Organize pranchas com numeração automática (01/N), histórico de revisões (R00, R01...) e lista mestra.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, "+ Planta Baixa").clicked() {
                        let prev = self.project.clone();
                        let base = self.project.sheet.clone();
                        let id = self
                            .project
                            .sheet_set
                            .add_sheet("Planta Baixa", SheetContent::FloorPlan, &base);
                        if let Some(sheet) =
                            self.project.sheet_set.sheets.iter_mut().find(|s| s.id == id)
                        {
                            sheet.discipline = Discipline::Architecture;
                        }
                        self.record_change(prev);
                    }
                    // A floor plan sheet for each other discipline.
                    for discipline in [
                        Discipline::Electrical,
                        Discipline::Plumbing,
                        Discipline::Structure,
                    ] {
                        if secondary_button(ui, &format!("+ {}", discipline.sheet_title()))
                            .clicked()
                        {
                            let prev = self.project.clone();
                            let base = self.project.sheet.clone();
                            let id = self.project.sheet_set.add_sheet(
                                discipline.sheet_title(),
                                SheetContent::FloorPlan,
                                &base,
                            );
                            if let Some(sheet) =
                                self.project.sheet_set.sheets.iter_mut().find(|s| s.id == id)
                            {
                                sheet.discipline = discipline;
                            }
                            self.record_change(prev);
                        }
                    }
                    let sections: Vec<_> = self
                        .project
                        .views
                        .sections
                        .iter()
                        .map(|s| (s.id, s.label.clone()))
                        .collect();
                    for (id, label) in sections {
                        if secondary_button(ui, &format!("+ Corte {label}")).clicked() {
                            let prev = self.project.clone();
                            let base = self.project.sheet.clone();
                            self.project.sheet_set.add_sheet(
                                &format!("Corte {label}"),
                                SheetContent::Section { id },
                                &base,
                            );
                            self.record_change(prev);
                        }
                    }
                    let elevs: Vec<_> = self
                        .project
                        .views
                        .elevations
                        .iter()
                        .map(|e| (e.id, e.label.clone()))
                        .collect();
                    for (id, label) in elevs {
                        if secondary_button(ui, &format!("+ {label}")).clicked() {
                            let prev = self.project.clone();
                            let base = self.project.sheet.clone();
                            self.project.sheet_set.add_sheet(
                                &label,
                                SheetContent::Elevation { id },
                                &base,
                            );
                            self.record_change(prev);
                        }
                    }
                    let details: Vec<_> = self
                        .project
                        .views
                        .details
                        .iter()
                        .map(|d| (d.id, d.label.clone()))
                        .collect();
                    for (id, label) in details {
                        if secondary_button(ui, &format!("+ {label}")).clicked() {
                            let prev = self.project.clone();
                            let base = self.project.sheet.clone();
                            self.project.sheet_set.add_sheet(
                                &format!("Detalhe {label}"),
                                SheetContent::Detail { id },
                                &base,
                            );
                            self.record_change(prev);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !self.project.sheet_set.is_empty()
                            && secondary_button(ui, "Lista Mestra CSV...").clicked()
                        {
                            self.export_master_sheet_list_csv();
                        }
                    });
                });

                ui.add_space(8.0);

                if self.project.sheet_set.is_empty() {
                    ui.label(
                        RichText::new(
                            "Nenhuma prancha no conjunto. Clique em '+ Planta Baixa' para iniciar o caderno de pranchas.",
                        )
                        .size(12.0)
                        .color(MUTED),
                    );
                    return;
                }

                // Ensure an active sheet is selected.
                if self.project.sheet_set.active_sheet_id.is_none() {
                    self.project.sheet_set.active_sheet_id =
                        self.project.sheet_set.sheets.first().map(|s| s.id);
                }

                let mut move_cmd: Option<(u64, isize)> = None;
                let mut remove_id: Option<u64> = None;

                egui::ScrollArea::vertical()
                    .id_salt("sheet_set_list")
                    .max_height(190.0)
                    .show(ui, |ui| {
                        for sheet in &self.project.sheet_set.sheets {
                            let selected = self.project.sheet_set.active_sheet_id == Some(sheet.id);
                            egui::Frame::new()
                                .fill(if selected { ACCENT_SOFTER } else { PANEL_ALT })
                                .stroke(Stroke::new(
                                    1.0,
                                    if selected { ACCENT } else { BORDER },
                                ))
                                .corner_radius(6)
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .selectable_label(
                                                selected,
                                                RichText::new(format!(
                                                    "{} ({}) · {} · {}",
                                                    sheet.code,
                                                    sheet.number,
                                                    sheet.title,
                                                    sheet.latest_revision_code()
                                                ))
                                                .font(strong(12.0)),
                                            )
                                            .clicked()
                                        {
                                            self.project.sheet_set.active_sheet_id = Some(sheet.id);
                                        }
                                        ui.label(
                                            RichText::new(sheet.effective_status(&current_red_checks).label())
                                                .size(11.0)
                                                .color(MUTED),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Excluir").clicked() {
                                                    remove_id = Some(sheet.id);
                                                }
                                                if ui.small_button("PDF").clicked() {
                                                    export_item = Some((sheet.id, true));
                                                }
                                                if ui.small_button("SVG").clicked() {
                                                    export_item = Some((sheet.id, false));
                                                }
                                                if ui.small_button("↓").clicked() {
                                                    move_cmd = Some((sheet.id, 1));
                                                }
                                                if ui.small_button("↑").clicked() {
                                                    move_cmd = Some((sheet.id, -1));
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });

                if let Some((id, delta)) = move_cmd {
                    let prev = self.project.clone();
                    if self.project.sheet_set.move_sheet(id, delta) {
                        self.record_change(prev);
                    }
                }
                if let Some(id) = remove_id {
                    let prev = self.project.clone();
                    if self.project.sheet_set.remove_sheet(id) {
                        self.record_change(prev);
                    }
                }

                ui.add_space(6.0);
                ui.separator();

                // Active sheet details & revision history.
                if let Some(active_id) = self.project.sheet_set.active_sheet_id
                    && let Some(sheet) = self
                        .project
                        .sheet_set
                        .sheets
                        .iter_mut()
                        .find(|s| s.id == active_id)
                {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Título da prancha:").size(12.0).color(TEXT_2));
                        if ui
                            .add(egui::TextEdit::singleline(&mut sheet.title).desired_width(220.0))
                            .changed()
                        {
                            sheet.layout.title_block.drawing_title = sheet.title.clone();
                            changed = true;
                        }
                        ui.label(RichText::new("Disciplina:").size(12.0).color(TEXT_2));
                        egui::ComboBox::from_id_salt(("sheet_discipline", sheet.id))
                            .selected_text(sheet.discipline.label())
                            .show_ui(ui, |ui| {
                                for option in Discipline::ALL {
                                    if ui
                                        .selectable_label(sheet.discipline == option, option.label())
                                        .clicked()
                                    {
                                        sheet.discipline = option;
                                        changed = true;
                                    }
                                }
                            });
                        ui.label(RichText::new("Situação:").size(12.0).color(TEXT_2));
                        for st in SheetStatus::ALL {
                            if toggle_pill(ui, st.label(), sheet.effective_status(&current_red_checks) == st, st.label()).clicked()
                            {
                                status_cmd = Some((sheet.id, st));
                            }
                        }
                    });

                    if !current_red_checks.is_empty() {
                        ui.add_space(5.0);
                        ui.label(RichText::new(format!(
                            "{} verificações vermelhas: a liberação exige revisão profissional explícita. Se os resultados ou a revisão mudarem, a prancha volta a 'Para aprovação'.",
                            current_red_checks.len()
                        )).size(11.5).color(DANGER));
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Revisor:");
                            ui.add(egui::TextEdit::singleline(&mut self.release_reviewer).desired_width(150.0));
                            ui.label("CREA/CAU:");
                            ui.add(egui::TextEdit::singleline(&mut self.release_registration).desired_width(130.0));
                            ui.label("Data:");
                            ui.add(egui::TextEdit::singleline(&mut self.release_date).desired_width(100.0));
                        });
                        if let Some(review) = &sheet.release_review {
                            ui.label(RichText::new(format!(
                                "Revisão registrada: {} · {} · {} · {}",
                                review.reviewer, review.registration, review.date, review.revision
                            )).size(11.0).color(MUTED));
                        }
                    }

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Histórico de revisões")
                            .font(strong(12.5))
                            .color(TEXT),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("sheet_revisions")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            egui::Grid::new("revisions_grid")
                                .striped(true)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Rev.").font(strong(11.5)));
                                    ui.label(RichText::new("Data").font(strong(11.5)));
                                    ui.label(RichText::new("Responsável").font(strong(11.5)));
                                    ui.label(RichText::new("Descrição").font(strong(11.5)));
                                    ui.end_row();
                                    for rev in &sheet.revisions {
                                        ui.label(RichText::new(&rev.code).font(strong(11.5)));
                                        ui.label(RichText::new(&rev.date).size(11.5));
                                        ui.label(RichText::new(&rev.author).size(11.5));
                                        ui.label(RichText::new(&rev.description).size(11.5));
                                        ui.end_row();
                                    }
                                });
                        });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Nova revisão:").size(12.0).color(TEXT_2));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_revision_text)
                                .desired_width(340.0)
                                .hint_text("Ex.: Compatibilização estrutural e revisão de cotas"),
                        );
                        if primary_button(ui, "+ Emitir revisão").clicked() {
                            let desc = self.new_revision_text.clone();
                            let date = self.project.sheet.title_block.date.clone();
                            let author = self.project.sheet.title_block.author.clone();
                            let prev = self.project.clone();
                            match self
                                .project
                                .sheet_set
                                .add_revision(active_id, &date, &author, &desc)
                            {
                                Ok(code) => {
                                    self.new_revision_text.clear();
                                    self.record_change(prev);
                                    self.message = format!("Revisão {code} registrada.");
                                }
                                Err(err) => self.message = err,
                            }
                        }
                    });
                }
                if let Some((id, status)) = status_cmd {
                    let prev = self.project.clone();
                    match self.project.set_sheet_status(id, status, Some((
                        &self.release_reviewer,
                        &self.release_registration,
                        &self.release_date,
                    ))) {
                        Ok(()) => {
                            self.record_change(prev);
                            self.message = format!("Situação da prancha: {}.", status.label());
                        }
                        Err(error) => self.message = error,
                    }
                }
            });

        if changed {
            self.mark_edited_outside_history();
        }
        if let Some((sheet_id, is_pdf)) = export_item
            && let Some(item) = self
                .project
                .sheet_set
                .sheets
                .iter()
                .find(|s| s.id == sheet_id)
                .cloned()
        {
            let (label, ext) = if is_pdf {
                ("Documento PDF", "pdf")
            } else {
                ("Desenho SVG", "svg")
            };
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(label, &[ext])
                .set_file_name(format!(
                    "{}-{}.{ext}",
                    item.code.to_lowercase(),
                    item.latest_revision_code().to_lowercase()
                ))
                .save_file()
            {
                let res = if is_pdf {
                    export_sheet_item_pdf(&self.project, &item, &path)
                } else {
                    export_sheet_item_svg(&self.project, &item, &path)
                };
                match res {
                    Ok(()) => {
                        self.message =
                            format!("Prancha {} exportada em {}", item.code, path.display())
                    }
                    Err(e) => self.message = format!("Erro ao exportar prancha: {e}"),
                }
            }
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }

    fn export_master_sheet_list_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planilha CSV", &["csv"])
            .set_file_name("lista-mestra-pranchas.csv")
            .save_file()
        else {
            return;
        };
        let csv = self.project.sheet_set.transmittal_csv(&self.project);
        match std::fs::write(&path, format!("\u{FEFF}{csv}")) {
            Ok(()) => self.message = format!("Lista mestra exportada em {}", path.display()),
            Err(e) => self.message = format!("Erro ao exportar lista mestra: {e}"),
        }
    }
}
