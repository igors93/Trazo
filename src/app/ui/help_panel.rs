//! "Onde buscar ajuda" inside the "Segurança" window (#159): the free help
//! paths, open whenever something is red, and the project's local contacts.

use eframe::egui::{self, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::model::Project;
use crate::domain::protection::help::{HELP_PATHS, clean_contacts};

/// Draws the help paths; returns the contacts to save when the person
/// clicks the button.
pub(super) fn show(ui: &mut Ui, project: &Project, urgent: bool) -> Option<Vec<String>> {
    let mut save = None;
    egui::CollapsingHeader::new(
        RichText::new("Onde buscar ajuda")
            .font(strong(13.0))
            .color(if urgent { DANGER } else { TEXT }),
    )
    .id_salt("safety_help")
    .default_open(urgent)
    .show(ui, |ui| {
        if urgent {
            ui.label(
                RichText::new(
                    "Há resultados vermelhos ou recusados: não construa essas partes assim. Corrija o que o Trazo indica ou procure ajuda.",
                )
                .color(DANGER),
            );
        }
        for path in HELP_PATHS {
            ui.label(RichText::new(path.name).font(strong(12.5)));
            ui.label(RichText::new(format!("{} {}", path.what, path.how)).size(11.5).color(TEXT_2));
            ui.add_space(3.0);
        }
        ui.label(RichText::new("Contatos locais (um por linha)").font(strong(12.0)));
        let id = egui::Id::unique("safety_help_contacts");
        let mut text: String = ui
            .data(|d| d.get_temp(id))
            .unwrap_or_else(|| project.protection.help_contacts.join("\n"));
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .hint_text("Prefeitura: secretaria de habitação, telefone…\nEscritório modelo da universidade…")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        let cleaned = clean_contacts(&text);
        if cleaned != project.protection.help_contacts
            && secondary_button(ui, "Salvar contatos").clicked()
        {
            save = Some(cleaned);
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    });
    save
}

impl FloorPlanApp {
    /// Keeps the local help contacts in the project, as one undo step.
    pub(super) fn save_help_contacts(&mut self, contacts: Vec<String>) {
        let previous = self.project.clone();
        self.project.protection.help_contacts = contacts;
        if self.record_change(previous) {
            self.message = "Contatos de ajuda salvos no projeto.".to_owned();
        }
    }
}
