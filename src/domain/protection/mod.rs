//! Protection for self-builders (roadmap milestone M0): what Trazo adds on
//! top of the engineering checks so that a family without engineering
//! training, or an AI assistant working for it, cannot reach a reassuring
//! result through a wrong hypothesis. See `docs/proposta.md`.

pub mod hypotheses;

use serde::{Deserialize, Serialize};

/// Everything the protection layer stores in the project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Protection {
    /// Who set each calculation hypothesis, and on what evidence (#168).
    #[serde(skip_serializing_if = "hypotheses::Hypotheses::is_empty")]
    pub hypotheses: hypotheses::Hypotheses,
}

impl Protection {
    pub fn is_empty(&self) -> bool {
        self.hypotheses.is_empty()
    }
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}
