//! The constraints of the selected wall or drawing in the properties
//! panel: what each one ties it to, the distance it keeps, and removing it.

use eframe::egui::{self, Key, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{hint, secondary_button, section};
use crate::domain::model::{Constraint, Entity};

impl FloorPlanApp {
    /// The "Restrições" section for `entity`, when it has any.
    pub(super) fn constraints_section(&mut self, ui: &mut Ui, entity: Entity) {
        let constraints: Vec<Constraint> = self.project.constraints_of(entity);
        if constraints.is_empty() {
            return;
        }
        let mut remove = None;
        let mut apply = None;
        section(ui, "selection_constraints", "Restrições", |ui| {
            for constraint in &constraints {
                let [a, b] = constraint.kind.entities();
                let other = if a == entity { b } else { a };
                let with = if other == entity {
                    String::new()
                } else {
                    format!(" · com {}", self.entity_name(other))
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("{}{with}", constraint.kind.label()))
                            .color(TEXT_2)
                            .size(12.5),
                    );
                });
                ui.horizontal(|ui| {
                    if constraint.kind.distance().is_some()
                        && let Some(text) = self.constraint_texts.get_mut(&constraint.id)
                    {
                        let field = ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(70.0)
                                .align(egui::Align2::RIGHT_CENTER),
                        );
                        ui.label(RichText::new("m").color(MUTED));
                        if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                            apply = Some(constraint.id);
                        }
                    }
                    if secondary_button(ui, "Remover").clicked() {
                        remove = Some(constraint.id);
                    }
                });
                ui.add_space(4.0);
            }
            hint(
                ui,
                Icon::Info,
                "Ao editar, o que você não mexeu se ajusta para manter as restrições; uma edição que as quebraria é recusada.",
                ACCENT,
            );
        });
        if let Some(id) = apply {
            self.apply_constraint_distance(id);
        }
        if let Some(id) = remove {
            self.remove_constraint(id);
        }
    }
}
