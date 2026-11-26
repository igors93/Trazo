//! Questions about what Trazo cannot see on the site (#156). Slopes,
//! streams, landfill, water in the ground and cracked neighbouring houses
//! cause the worst collapses, and nothing in the drawing shows them, so
//! Trazo asks. A risky answer stops the foundations: they turn red with
//! "stop and get help" instead of being sized.
//!
//! An answer that removes a risk counts only when the person gives it: an
//! assistant may report a risk, but its "no" waits for the person (#171).
//!
//! Two questions are about building on top of an existing house (#157): a
//! new floor over a ground floor whose foundation and columns nobody knows
//! is refused until someone finds out.

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
    /// The new work goes on top of a house that already exists (#157).
    BuildingOnTop,
    /// How the existing house's foundation and columns were made is known.
    ExistingBaseKnown,
}

impl SiteRisk {
    pub const ALL: [Self; 8] = [
        Self::Slope,
        Self::NearWater,
        Self::Landfill,
        Self::Groundwater,
        Self::NeighbourCracks,
        Self::DiggingNearNeighbour,
        Self::BuildingOnTop,
        Self::ExistingBaseKnown,
    ];

    /// The answer that makes the site look safer, which only the person can
    /// give: "no" to a risk, "yes" to knowing the existing base.
    pub fn favourable(self) -> bool {
        self == Self::ExistingBaseKnown
    }

    /// The question, as the person reads it.
    pub fn question(self) -> &'static str {
        match self {
            Self::Slope => "O terreno fica em morro ou encosta?",
            Self::NearWater => "Fica perto de córrego, rio, canal ou de um lugar que alaga?",
            Self::Landfill => "O chão é aterro (terra trazida de fora, entulho, lixão antigo)?",
            Self::Groundwater => "Brota água do chão ou ele fica encharcado mesmo sem chuva?",
            Self::NeighbourCracks => "As casas vizinhas têm rachaduras ou trincas grandes?",
            Self::DiggingNearNeighbour => "Vai escavar perto do muro ou da fundação de um vizinho?",
            Self::BuildingOnTop => "Vai construir em cima de uma casa que já existe?",
            Self::ExistingBaseKnown => {
                "Você sabe como foram feitos a fundação e os pilares da casa que já existe (tipo, tamanho e ferro)?"
            }
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
            Self::BuildingOnTop => {
                "Um andar novo pesa sobre a fundação e os pilares da casa de baixo, que podem ter sido feitos só para o térreo."
            }
            Self::ExistingBaseKnown => EXISTING_BASE_GUIDANCE,
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
            Self::BuildingOnTop => "sobre_casa_existente",
            Self::ExistingBaseKnown => "base_existente_conhecida",
        }
    }
}

/// What to find out before building on top of an existing house (#157).
pub const EXISTING_BASE_GUIDANCE: &str = "Antes de subir um andar, descubra: (1) o tipo e o tamanho da fundação, cavando ao lado de um pilar até ver a sapata ou o baldrame, com cuidado e sem tirar terra de baixo dela; (2) o tamanho e o ferro dos pilares e das vigas, pela planta antiga, pelo pedreiro que fez ou abrindo o reboco num ponto; (3) se há trincas, ferrugem ou laje com barriga. Com isso, um profissional, inclusive o da assistência técnica gratuita, avalia se a casa aguenta mais um andar.";

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

    /// What Trazo takes as true: the unfavourable answer from anyone, the
    /// favourable one only from the person; `None` while nobody (or only an
    /// assistant) gave the favourable answer.
    pub fn effective(&self, risk: SiteRisk) -> Option<bool> {
        let answer = self.answer(risk)?;
        if answer.yes != risk.favourable() || answer.by == HypothesisSource::Person {
            Some(answer.yes)
        } else {
            None
        }
    }

    /// Whether an answer was given by an assistant and still waits for the
    /// person.
    pub fn waits_for_person(&self, risk: SiteRisk) -> bool {
        self.answer(risk).is_some() && self.effective(risk).is_none()
    }

    /// The site risks answered "yes" (the existing-house questions apart).
    pub fn risky(&self) -> Vec<SiteRisk> {
        SiteRisk::ALL
            .into_iter()
            .filter(|&risk| {
                !matches!(risk, SiteRisk::BuildingOnTop | SiteRisk::ExistingBaseKnown)
                    && self.effective(risk) == Some(true)
            })
            .collect()
    }

    /// Building on top of an existing house whose base nobody knows (#157).
    pub fn unknown_existing_base(&self) -> bool {
        self.effective(SiteRisk::BuildingOnTop) == Some(true)
            && self.effective(SiteRisk::ExistingBaseKnown) != Some(true)
    }

    /// The questions still without an answer Trazo can take. The question
    /// about the existing base only matters when building on top.
    pub fn unanswered(&self) -> Vec<SiteRisk> {
        SiteRisk::ALL
            .into_iter()
            .filter(|&risk| {
                (risk != SiteRisk::ExistingBaseKnown
                    || self.effective(SiteRisk::BuildingOnTop) == Some(true))
                    && self.effective(risk).is_none()
            })
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
        // The existing-base question only matters when building on top.
        assert_eq!(risks.unanswered().len(), SiteRisk::ALL.len() - 1);
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
    fn building_on_an_unknown_base_needs_the_person_to_know_it() {
        let mut risks = SiteRisks::default();
        assert!(!risks.unanswered().contains(&SiteRisk::ExistingBaseKnown));
        risks.set(
            SiteRisk::BuildingOnTop,
            Some(true),
            HypothesisSource::Assistant,
        );
        assert!(risks.unknown_existing_base());
        assert!(risks.unanswered().contains(&SiteRisk::ExistingBaseKnown));
        risks.set(
            SiteRisk::ExistingBaseKnown,
            Some(true),
            HypothesisSource::Assistant,
        );
        assert!(risks.unknown_existing_base());
        assert!(risks.waits_for_person(SiteRisk::ExistingBaseKnown));
        risks.set(
            SiteRisk::ExistingBaseKnown,
            Some(true),
            HypothesisSource::Person,
        );
        assert!(!risks.unknown_existing_base());
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
