//! Protection for self-builders (roadmap milestone M0): what Trazo adds on
//! top of the engineering checks so that a family without engineering
//! training, or an AI assistant working for it, cannot reach a reassuring
//! result through a wrong hypothesis. See `docs/proposta.md`.

pub mod action_report;
pub mod checklists;
pub mod cross_checks;
pub mod glossary;
pub mod help;
pub mod hypotheses;
pub mod site_risks;
pub mod stages;

use serde::{Deserialize, Serialize};

/// Everything the protection layer stores in the project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Protection {
    /// Who set each calculation hypothesis, and on what evidence (#168).
    #[serde(skip_serializing_if = "hypotheses::Hypotheses::is_empty")]
    pub hypotheses: hypotheses::Hypotheses,
    /// Local places to ask for help, typed by the person (#159).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub help_contacts: Vec<String>,
    /// What Trazo cannot see on the site, as the person answered (#156).
    #[serde(skip_serializing_if = "site_risks::SiteRisks::is_empty")]
    pub site_risks: site_risks::SiteRisks,
    /// The floors to come and the foundation already built (#158).
    #[serde(skip_serializing_if = "stages::StagePlan::is_empty")]
    pub stages: stages::StagePlan,
}

impl Protection {
    pub fn is_empty(&self) -> bool {
        self.hypotheses.is_empty()
            && self.help_contacts.is_empty()
            && self.site_risks.is_empty()
            && self.stages.is_empty()
    }
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}
