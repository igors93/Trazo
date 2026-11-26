//! The calculation hypotheses inside the "Segurança" window (#168, #169,
//! #171): each value with its origin and evidence, the ones Trazo contests,
//! and the assistant's proposals waiting for the person to confirm them.

use eframe::egui::{self, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::safety::light_dot;
use super::widgets::{primary_button, secondary_button};
use crate::domain::model::Project;
use crate::domain::protection::hypotheses::{self as hyp, HypothesisKey};
use crate::domain::verdict::Light;

/// What the person asked for in the hypotheses panel.
pub(super) enum HypothesisAction {
    /// Adopt the assistant's proposal, with evidence.
    Confirm(HypothesisKey, String),
    /// Turn the assistant's proposal down.
    Reject(HypothesisKey),
    /// Back up the current value with evidence.
    Evidence(HypothesisKey, String),
    /// Go back to the value Trazo can stand behind.
    Safe(HypothesisKey),
}

/// Draws the hypotheses and returns what the person asked for.
pub(super) fn show(ui: &mut Ui, project: &Project) -> Option<HypothesisAction> {
    let mut action = None;
    let contests = hyp::contests(project);
    let proposals = project.protection.hypotheses.proposals();
    let pending = !contests.is_empty() || !proposals.is_empty();
    let title = if pending {
        format!(
            "Hipóteses de cálculo · {} contestada(s) · {} proposta(s) da IA",
            contests.len(),
            proposals.len()
        )
    } else {
        "Hipóteses de cálculo".to_owned()
    };
    egui::CollapsingHeader::new(
        RichText::new(title)
            .font(strong(13.0))
            .color(if pending { DANGER } else { TEXT }),
    )
    .id_salt("safety_hypotheses")
    .default_open(pending)
    .show(ui, |ui| {
        ui.label(
            RichText::new(
                "Todo resultado depende destes valores. Um valor mais otimista que o seguro só vale com prova; sem ela, os resultados ficam contestados e nunca verdes.",
            )
            .size(11.5)
            .color(TEXT_2),
        );
        for proposal in proposals {
            let key = proposal.key;
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                light_dot(ui, Light::Yellow);
                ui.label(
                    RichText::new(format!(
                        "A IA propôs {}: {} (ainda não aplicado)",
                        key.label().to_lowercase(),
                        key.format(project, proposal.value)
                    ))
                    .font(strong(12.5)),
                );
                if let Some(cited) = &proposal.evidence {
                    ui.label(RichText::new(format!("prova citada pela IA: {cited}")).color(MUTED));
                }
            });
            if let Some(chosen) = proof_row(
                ui,
                ("hyp_proposal", key),
                proposal.evidence.clone().unwrap_or_default(),
                "Confirmar",
            ) {
                action = Some(HypothesisAction::Confirm(key, chosen));
            }
            if secondary_button(ui, "Recusar a proposta").clicked() {
                action = Some(HypothesisAction::Reject(key));
            }
        }
        ui.add_space(6.0);
        for key in HypothesisKey::ALL {
            let contested = contests.iter().find(|c| c.key == key);
            let record = project.protection.hypotheses.record(key);
            let proof = hyp::evidence(key, project);
            ui.horizontal_wrapped(|ui| {
                light_dot(
                    ui,
                    if contested.is_some() {
                        Light::Red
                    } else {
                        Light::Green
                    },
                );
                ui.label(RichText::new(key.label()).font(strong(12.0)))
                    .on_hover_text(key.plain());
                ui.label(key.display(project));
                ui.label(
                    RichText::new(format!(
                        "· {} · {}",
                        record.source.label(),
                        proof.map_or_else(|| "sem prova".to_owned(), |e| format!("prova: {e}"))
                    ))
                    .color(MUTED),
                );
            });
            let Some(contested) = contested else {
                continue;
            };
            ui.label(
                RichText::new(format!("{} {}", contested.reason, contested.fix))
                    .size(11.5)
                    .color(DANGER),
            );
            ui.horizontal_wrapped(|ui| {
                if hyp::reference(key, project).is_some()
                    && secondary_button(ui, "Usar o valor seguro").clicked()
                {
                    action = Some(HypothesisAction::Safe(key));
                }
            });
            if !contested.normative
                && let Some(chosen) = proof_row(ui, ("hyp_proof", key), String::new(), "Informar prova")
            {
                action = Some(HypothesisAction::Evidence(key, chosen));
            }
        }
    });
    action
}

/// A text field for the evidence and its button; returns the text when the
/// button is clicked.
fn proof_row(
    ui: &mut Ui,
    salt: (&str, HypothesisKey),
    initial: String,
    button: &str,
) -> Option<String> {
    let id = egui::Id::unique(salt);
    let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or(initial);
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.label("Prova:");
        ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("laudo da sondagem, nota fiscal, ensaio, fonte do valor")
                .desired_width(280.0),
        );
        if primary_button(ui, button).clicked() {
            chosen = Some(text.clone());
        }
    });
    ui.data_mut(|d| d.insert_temp(id, text));
    chosen
}

impl FloorPlanApp {
    /// Applies what the person asked for in the hypotheses panel, as one
    /// undo step credited to the person.
    pub(super) fn apply_hypothesis_action(&mut self, action: HypothesisAction) {
        let previous = self.project.clone();
        let result = match action {
            HypothesisAction::Confirm(key, proof) => {
                hyp::confirm_proposal(&mut self.project, key, &proof).map(|()| {
                    format!(
                        "{} confirmada por você, com a prova informada.",
                        key.label()
                    )
                })
            }
            HypothesisAction::Reject(key) => {
                hyp::reject_proposal(&mut self.project, key);
                Ok(format!(
                    "Proposta da IA para {} recusada.",
                    key.label().to_lowercase()
                ))
            }
            HypothesisAction::Evidence(key, proof) => {
                hyp::give_evidence(&mut self.project, key, &proof)
                    .map(|()| format!("Prova registrada para {}.", key.label().to_lowercase()))
            }
            HypothesisAction::Safe(key) => match hyp::reference(key, &self.project) {
                Some(value) => hyp::set_value(key, &mut self.project, value).map(|()| {
                    format!(
                        "{} voltou ao valor seguro: {}.",
                        key.label(),
                        key.display(&self.project)
                    )
                }),
                None => Err("Essa hipótese não tem um valor seguro único.".to_owned()),
            },
        };
        match result {
            Ok(message) => {
                if self.record_change(previous) {
                    self.message = message;
                }
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
            }
        }
    }
}
