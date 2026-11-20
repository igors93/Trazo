//! Dialog for inserting and managing texts, technical notes, leader callouts
//! and architectural symbols (north, level mark, slope arrow, entry arrow).

use eframe::egui::{self, Margin, RichText, Stroke, vec2};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{danger_button, primary_button, secondary_button};
use crate::domain::{
    geometry::Point,
    notes::{NoteKind, TechnicalSymbol},
};

impl FloorPlanApp {
    pub(super) fn notes_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Notes {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("notes_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(620.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Info,
                    "Textos, notas, chamadas e símbolos",
                    "Adicione textos técnicos, notas em caixa, linhas de chamada e símbolos de planta baixa.",
                );

                let anchor = self
                    .cursor_world
                    .or_else(|| self.project.bounds().map(|(a, b)| a.midpoint(b)))
                    .unwrap_or(Point::new(2.0, 2.0));

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Texto:").size(12.0).color(TEXT_2));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_note_text)
                            .desired_width(240.0)
                            .hint_text("Ex.: Piso porcelanato, Ralo linear..."),
                    );
                    if primary_button(ui, "Texto").clicked() {
                        let prev = self.project.clone();
                        let text = self.new_note_text.clone();
                        if self.project.add_note(anchor, &text, NoteKind::Text).is_ok() {
                            self.record_change(prev);
                            self.new_note_text.clear();
                        }
                    }
                    if secondary_button(ui, "Nota").clicked() {
                        let prev = self.project.clone();
                        let text = self.new_note_text.clone();
                        if self.project.add_note(anchor, &text, NoteKind::Note).is_ok() {
                            self.record_change(prev);
                            self.new_note_text.clear();
                        }
                    }
                    if secondary_button(ui, "Chamada").clicked() {
                        let prev = self.project.clone();
                        let text = self.new_note_text.clone();
                        let target = anchor.translated(Point::new(-0.8, 0.5));
                        if self
                            .project
                            .add_note(anchor, &text, NoteKind::Callout { target })
                            .is_ok()
                        {
                            self.record_change(prev);
                            self.new_note_text.clear();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Símbolos:").size(12.0).color(TEXT_2));
                    for (label, sym) in [
                        ("Nível ±0,00", TechnicalSymbol::LevelMark { elevation_m: 0.0 }),
                        ("Nível +0,15", TechnicalSymbol::LevelMark { elevation_m: 0.15 }),
                        ("Caimento 1%", TechnicalSymbol::SlopeArrow { percent: 1.0 }),
                        ("Norte", TechnicalSymbol::NorthArrow),
                        ("Acesso", TechnicalSymbol::EntryArrow),
                    ] {
                        if secondary_button(ui, label).clicked() {
                            let prev = self.project.clone();
                            if self
                                .project
                                .add_note(anchor, "", NoteKind::Symbol(sym))
                                .is_ok()
                            {
                                self.record_change(prev);
                                self.message = format!("Símbolo \"{label}\" adicionado.");
                            }
                        }
                    }
                });

                ui.add_space(10.0);
                if self.project.notes.is_empty() {
                    ui.label(
                        RichText::new("Nenhuma anotação no projeto.")
                            .size(12.0)
                            .color(MUTED),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            let ids: Vec<u64> = self.project.notes.iter().map(|n| n.id).collect();
                            for id in ids {
                                let Some(note) =
                                    self.project.notes.iter().find(|n| n.id == id).cloned()
                                else {
                                    continue;
                                };
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.add_sized(
                                                vec2(110.0, 20.0),
                                                egui::Label::new(
                                                    RichText::new(note.kind.label())
                                                        .font(strong(12.0))
                                                        .color(TEXT),
                                                )
                                                .truncate(),
                                            );
                                            let mut text = note.text.clone();
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut text)
                                                        .desired_width(250.0),
                                                )
                                                .changed()
                                            {
                                                let _ = self.project.update_note_text(id, &text);
                                                self.mark_edited_outside_history();
                                            }
                                            if secondary_button(ui, "→").on_hover_text("Deslocar +0,50 m em X").clicked() {
                                                let prev = self.project.clone();
                                                if self.project.move_note(id, Point::new(0.5, 0.0)) {
                                                    self.record_change(prev);
                                                }
                                            }
                                            if danger_button(ui, "Excluir").clicked() {
                                                let prev = self.project.clone();
                                                if self.project.delete_note(id) {
                                                    self.record_change(prev);
                                                }
                                            }
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                }

                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, "Fechar").clicked() {
                        close = true;
                    }
                });
            });
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
