//! Questions about what Trazo cannot see on the site (#156). Slopes,
//! streams, landfill, water in the ground and cracked neighbouring houses
//! cause the worst collapses, and nothing in the drawing shows them, so
//! Trazo asks. A risky answer stops the foundations: they turn red with
//! "stop and get help" instead of being sized.
//!
//! An answer that removes a risk counts only when the person gives it: an
//! assistant may report a risk, but its "no" waits for the person (#171).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::hypotheses::HypothesisSource;

/// One question about the site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteRisk {
    Slope,
    NearWater,
    Landfill,
    Groundwater,
    NeighbourCracks,
    DiggingNearNeighbour,
}

impl SiteRisk {
    pub const ALL: [Self; 6] = [
        Self::Slope,
        Self::NearWater,
        Self::Landfill,
        Self::Groundwater,
        Self::NeighbourCracks,
        Self::DiggingNearNeighbour,
    ];

    /// The question, as the person reads it.
    pub fn question(self) -> &'static str {
        match self {
            Self::Slope => "O terreno fica em morro ou encosta?",
            Self::NearWater => "Fica perto de córrego, rio, canal ou de um lugar que alaga?",
            Self::Landfill => "O chão é aterro (terra trazida de fora, entulho, lixão antigo)?",
            Self::Groundwater => "Brota água do chão ou ele fica encharcado mesmo sem chuva?",
            Self::NeighbourCracks => "As casas vizinhas têm rachaduras ou trincas grandes?",
            Self::DiggingNearNeighbour => "Vai escavar perto do muro ou da fundação de um vizinho?",
        }
    }

    /// What a "yes" means, in plain words.
    pub fn danger(self) -> &'static str {
        match self {
            Self::Slope => {
                "Em encosta, o terreno pode deslizar e a casa precisa de contenção e fundação projetadas por um profissional."
            }
            Self::NearWater => {
                "Perto de água, o solo pode ser fraco ou ser levado por enchentes; a fundação precisa de estudo."
            }
            Self::Landfill => {
                "Aterro afunda com o tempo e não aguenta o peso que a conta supõe; é preciso sondagem e fundação profunda."
            }
            Self::Groundwater => {
                "Água no solo enfraquece o chão e impede brocas; a fundação precisa de estudo."
            }
            Self::NeighbourCracks => {
                "Rachaduras nos vizinhos mostram que o solo ou as fundações da região já estão cedendo."
            }
            Self::DiggingNearNeighbour => {
                "Escavar junto à fundação do vizinho pode derrubar a casa dele; a escavação precisa de escoramento projetado."
            }
        }
    }

    /// The MCP argument name.
    pub fn key(self) -> &'static str {
        match self {
            Self::Slope => "encosta",
            Self::NearWater => "perto_de_agua",
            Self::Landfill => "aterro",
            Self::Groundwater => "agua_no_solo",
            Self::NeighbourCracks => "trincas_vizinhos",
            Self::DiggingNearNeighbour => "escavacao_vizinho",
        }
    }
}

/// One answer and who gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAnswer {
    pub yes: bool,
    #[serde(default)]
    pub by: HypothesisSource,
}

/// The answers given so far.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteRisks {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    answers: BTreeMap<SiteRisk, RiskAnswer>,
}

impl SiteRisks {
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }

    pub fn answer(&self, risk: SiteRisk) -> Option<RiskAnswer> {
        self.answers.get(&risk).copied()
    }

    /// Records an answer; `None` clears it ("não sei").
    pub fn set(&mut self, risk: SiteRisk, yes: Option<bool>, by: HypothesisSource) {
        match yes {
            Some(yes) => {
                self.answers.insert(risk, RiskAnswer { yes, by });
            }
            None => {
                self.answers.remove(&risk);
            }
        }
    }

    /// What Trazo takes as true: a "yes" from anyone, a "no" only from the
    /// person; `None` while nobody (or only an assistant) said "no".
    pub fn effective(&self, risk: SiteRisk) -> Option<bool> {
        let answer = self.answer(risk)?;
        if answer.yes {
            Some(true)
        } else {
            (answer.by == HypothesisSource::Person).then_some(false)
        }
    }

    /// The risks answered "yes".
    pub fn risky(&self) -> Vec<SiteRisk> {
        SiteRisk::ALL
            .into_iter()
            .filter(|&risk| self.effective(risk) == Some(true))
            .collect()
    }

    /// The questions still without an answer Trazo can take.
    pub fn unanswered(&self) -> Vec<SiteRisk> {
        SiteRisk::ALL
            .into_iter()
            .filter(|&risk| self.effective(risk).is_none())
            .collect()
    }
}

/// Verdict elements that are foundations, which a risky site stops.
pub fn is_foundation_element(element: &str) -> bool {
    [
        "Sapata ",
        "Bloco sobre broca ",
        "Viga baldrame ",
        "Viga alavanca ",
    ]
    .iter()
    .any(|prefix| element.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_no_counts_only_from_the_person_and_a_yes_from_anyone() {
        let mut risks = SiteRisks::default();
        assert_eq!(risks.unanswered().len(), SiteRisk::ALL.len());
        risks.set(SiteRisk::Slope, Some(false), HypothesisSource::Assistant);
        assert_eq!(risks.effective(SiteRisk::Slope), None);
        risks.set(SiteRisk::Slope, Some(false), HypothesisSource::Person);
        assert_eq!(risks.effective(SiteRisk::Slope), Some(false));
        risks.set(SiteRisk::Landfill, Some(true), HypothesisSource::Assistant);
        assert_eq!(risks.risky(), vec![SiteRisk::Landfill]);
        risks.set(SiteRisk::Landfill, None, HypothesisSource::Person);
        assert!(risks.risky().is_empty());
    }

    #[test]
    fn foundations_are_told_apart_from_other_elements() {
        assert!(is_foundation_element("Sapata S1"));
        assert!(is_foundation_element("Sapata corrida SC2"));
        assert!(is_foundation_element("Viga baldrame VB1"));
        assert!(!is_foundation_element("Pilar P1"));
    }
}
