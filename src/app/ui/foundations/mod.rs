//! Foundation commands and UI panels: balance beams for boundary footings
//! (#144), wall foundations (#143), multi-borehole SPT survey and comparative
//! foundation study (#112).

mod study_panel;
mod survey_panel;

use eframe::egui::Ui;

use super::super::FloorPlanApp;
use crate::domain::model::Project;
use crate::domain::soil::SoilSurvey;

impl FloorPlanApp {
    /// Gives every ground-floor wall a ground beam or a strip footing.
    pub(crate) fn generate_wall_foundations(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (beams, strips) = self.project.generate_wall_foundations();
        if beams + strips == 0 {
            self.message =
                "Todas as paredes do térreo já têm fundação (ou você está num pavimento superior)."
                    .to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "Fundação das paredes: {beams} viga(s) baldrame e {strips} sapata(s) corrida(s). Use Ajustar sapatas para incluir o peso das paredes."
            );
        }
    }

    /// Lays a balance beam from every off-centre boundary footing to the
    /// nearest interior column.
    pub(crate) fn generate_balance_beams(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (added, alone) = self.project.generate_balance_beams();
        if added == 0 {
            self.message = if alone > 0 {
                format!(
                    "{alone} pilar(es) de divisa sem pilar interno a até 10 m para ancorar uma viga alavanca."
                )
            } else {
                "Nenhuma sapata de divisa fora do centro do pilar (ou todas já têm viga alavanca)."
                    .to_owned()
            };
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "{added} viga(s) alavanca lançada(s): as sapatas de divisa ficam centradas.{}",
                if alone > 0 {
                    format!(" {alone} pilar(es) de divisa ficaram sem pilar interno para ancorar.")
                } else {
                    String::new()
                }
            );
        }
    }
}

/// What the soil tab asks for (#112).
pub(super) enum SoilAction {
    /// Replace or remove the survey.
    Survey(Option<SoilSurvey>),
    /// Take the survey's governing footing pressure as the soil pressure.
    ApplyToSoil,
}

pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// The "Solo e fundações" tab of the structure window: multi-borehole SPT
/// survey and comparative foundation study.
pub(super) fn soil_tab(ui: &mut Ui, project: &Project) -> Option<SoilAction> {
    let action = survey_panel::show(ui, project);
    study_panel::show(ui, project);
    action
}
