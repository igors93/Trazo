//! "Perguntas sobre o terreno" inside the "Segurança" window (#156): what
//! Trazo cannot see on the site, answered by the person.

use eframe::egui::{self, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use crate::domain::model::Project;
use crate::domain::protection::hypotheses::HypothesisSource;
use crate::domain::protection::site_risks::SiteRisk;

/// Draws the questions; returns the answer the person changed.
pub(super) fn show(ui: &mut Ui, project: &Project) -> Option<(SiteRisk, Option<bool>)> {
    let mut changed = None;
    let risks = &project.protection.site_risks;
    let open = !risks.unanswered().is_empty() || !risks.risky().is_empty();
    egui::CollapsingHeader::new(
        RichText::new(format!(
            "Perguntas sobre o terreno{}",
            if risks.risky().is_empty() {
                String::new()
            } else {
                " · há risco: pare e procure ajuda".to_owned()
            }
        ))
        .font(strong(13.0))
        .color(if risks.risky().is_empty() { TEXT } else { DANGER }),
    )
    .id_salt("safety_site_risks")
    .default_open(open)
    .show(ui, |ui| {
        ui.label(
            RichText::new(
                "O Trazo não vê o terreno. Responda com sinceridade: um \"sim\" para as fundações até um profissional avaliar.",
            )
            .size(11.5)
            .color(TEXT_2),
        );
        for risk in SiteRisk::ALL {
            let answer = risks.answer(risk);
            let current = answer.filter(|a| a.yes || a.by == HypothesisSource::Person).map(|a| a.yes);
            ui.horizontal_wrapped(|ui| {
                ui.label(risk.question()).on_hover_text(risk.danger());
                for (label, value) in [("Sim", Some(true)), ("Não", Some(false)), ("Não sei", None)] {
                    if ui.selectable_label(current == value, label).clicked() && current != value {
                        changed = Some((risk, value));
                    }
                }
                if answer.is_some_and(|a| !a.yes && a.by != HypothesisSource::Person) {
                    ui.label(RichText::new("a IA respondeu \"não\": confirme").color(WARNING));
                }
            });
        }
    });
    changed
}

impl FloorPlanApp {
    /// Records the person's answer, as one undo step.
    pub(super) fn answer_site_risk(&mut self, risk: SiteRisk, yes: Option<bool>) {
        let previous = self.project.clone();
        self.project
            .protection
            .site_risks
            .set(risk, yes, HypothesisSource::Person);
        if self.record_change(previous) {
            self.message = match yes {
                Some(true) => format!(
                    "{} Pare e procure ajuda antes de fazer as fundações.",
                    risk.danger()
                ),
                Some(false) => "Resposta registrada.".to_owned(),
                None => "Resposta apagada.".to_owned(),
            };
        }
    }
}
