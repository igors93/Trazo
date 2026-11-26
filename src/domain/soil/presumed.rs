//! Presumed soil profile (*perfil geotécnico provável*): what the ground is
//! expected to be, from a qualitative geotechnical expectation such as the
//! regional geology or a neighbour's survey, when there is no SPT yet.
//!
//! A presumed profile is never evidence. It only ever makes Trazo *more*
//! careful: the allowable pressure it gives is capped at the conservative
//! value Trazo uses without a survey (100 kPa), and it brings in what the
//! soil type implies for the family — porous red tropical soils that collapse
//! when soaked, how slowly sewage seeps into clays, how hard the earth pushes
//! on a pool wall. A soil survey (SPT) always takes precedence.
//!
//! Indicative values are the lower bounds of the usual ranges in Brazilian
//! practice: blow counts per soil type (Teixeira & Godoy), `σ_adm ≈ 20·N` kPa
//! for shallow footings, effective friction angles for earth pressure at
//! rest (`K0 = 1 − sin φ'`), and the soil absorption rates of sewage used for
//! *sumidouros* (NBR 7229 / NBR 13969 practice: about 40 L/m²·dia in sandy
//! or silty clays, 20 L/m²·dia in compact clays and weathered rock).

use serde::{Deserialize, Serialize};

use crate::domain::protection::hypotheses::CONSERVATIVE_SOIL_KPA;

use super::decimal;

/// A soil type someone can recognise or read in a regional description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresumedSoilType {
    /// Porous red sandy clay (tropical lateritic soil over sandstone or basalt).
    RedSandyClay,
    /// Porous red clayey sand (Caiuá/Bauru sandstone cover).
    RedClayeySand,
    /// Silty sandy clay.
    SiltySandyClay,
    /// Silty clay.
    SiltyClay,
    /// Soil with lateritic concretions (*piçarra*, ironstone nodules).
    LateriticConcretions,
    /// Weathered sandstone or basalt (saprolite), usually at depth.
    WeatheredRock,
    /// Soft dark or organic clay (valley floors, near streams).
    SoftClay,
    /// Clean fine-to-medium sand.
    Sand,
}

/// Indicative properties of a soil type (lower bounds of usual ranges).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoilTypeProperties {
    /// Usual SPT blow count range near the surface (low, high).
    pub n_range: (u32, u32),
    /// Natural unit weight, kN/m³.
    pub unit_weight_kn_m3: f64,
    /// Effective friction angle, degrees.
    pub friction_deg: f64,
    /// Sewage absorption rate for a *sumidouro*, L/m²·dia.
    pub absorption_l_m2_day: f64,
    /// Porous structure that can collapse when soaked (*solo colapsível*).
    pub collapsible: bool,
}

impl PresumedSoilType {
    pub const ALL: [Self; 8] = [
        Self::RedSandyClay,
        Self::RedClayeySand,
        Self::SiltySandyClay,
        Self::SiltyClay,
        Self::LateriticConcretions,
        Self::WeatheredRock,
        Self::SoftClay,
        Self::Sand,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::RedSandyClay => "argila_arenosa_vermelha",
            Self::RedClayeySand => "areia_argilosa_vermelha",
            Self::SiltySandyClay => "argila_silto_arenosa",
            Self::SiltyClay => "argila_siltosa",
            Self::LateriticConcretions => "solo_concrecoes_lateriticas",
            Self::WeatheredRock => "alteracao_arenito_basalto",
            Self::SoftClay => "argila_mole",
            Self::Sand => "areia",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RedSandyClay => "Argila arenosa vermelha",
            Self::RedClayeySand => "Areia argilosa vermelha",
            Self::SiltySandyClay => "Argila silto-arenosa",
            Self::SiltyClay => "Argila siltosa",
            Self::LateriticConcretions => "Solo com concreções lateríticas",
            Self::WeatheredRock => "Solo de alteração de arenito ou basalto",
            Self::SoftClay => "Argila mole ou orgânica",
            Self::Sand => "Areia fina a média",
        }
    }

    /// Reads the key or the label, with or without accents.
    pub fn parse(raw: &str) -> Option<Self> {
        let key = normalise(raw);
        Self::ALL
            .into_iter()
            .find(|soil| soil.key() == key || normalise(soil.label()) == key)
    }

    pub fn properties(self) -> SoilTypeProperties {
        let p = |n_range, unit_weight_kn_m3, friction_deg, absorption_l_m2_day, collapsible| {
            SoilTypeProperties {
                n_range,
                unit_weight_kn_m3,
                friction_deg,
                absorption_l_m2_day,
                collapsible,
            }
        };
        match self {
            Self::RedSandyClay => p((3, 6), 16.0, 26.0, 40.0, true),
            Self::RedClayeySand => p((3, 8), 17.0, 28.0, 40.0, true),
            Self::SiltySandyClay => p((4, 8), 17.0, 24.0, 40.0, false),
            Self::SiltyClay => p((3, 6), 16.5, 22.0, 20.0, false),
            Self::LateriticConcretions => p((6, 15), 18.0, 30.0, 40.0, false),
            Self::WeatheredRock => p((15, 40), 19.0, 32.0, 20.0, false),
            Self::SoftClay => p((1, 3), 15.0, 18.0, 20.0, false),
            Self::Sand => p((4, 10), 18.0, 30.0, 60.0, false),
        }
    }
}

/// How likely a layer is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Likelihood {
    #[default]
    Probable,
    Possible,
    /// Possible, but only at depth (below the footings and the pool).
    PossibleAtDepth,
}

impl Likelihood {
    pub const ALL: [Self; 3] = [Self::Probable, Self::Possible, Self::PossibleAtDepth];

    pub fn key(self) -> &'static str {
        match self {
            Self::Probable => "provavel",
            Self::Possible => "possivel",
            Self::PossibleAtDepth => "possivel_em_profundidade",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Probable => "Provável",
            Self::Possible => "Possível",
            Self::PossibleAtDepth => "Possível em profundidade",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let key = normalise(raw);
        Self::ALL.into_iter().find(|l| l.key() == key)
    }

    fn shallow(self) -> bool {
        self != Self::PossibleAtDepth
    }
}

/// One layer of the presumed profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresumedLayer {
    pub soil: PresumedSoilType,
    #[serde(default)]
    pub likelihood: Likelihood,
}

/// The presumed profile, as the person described it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PresumedSoil {
    pub layers: Vec<PresumedLayer>,
    /// Where the description comes from (regional geology, a neighbour's survey...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// What a presumed profile means for the calculations.
#[derive(Clone, Debug, PartialEq)]
pub struct PresumedSoilAnalysis {
    /// The lowest blow count expected near the surface.
    pub governing_n: u32,
    /// Allowable pressure for shallow footings, kPa (never above 100 kPa).
    pub allowable_kpa: f64,
    /// The soil that governs the allowable pressure.
    pub governing_soil: PresumedSoilType,
    /// A porous soil that collapses when soaked is probable or possible near the surface.
    pub collapsible: bool,
    /// … and probable, not merely possible.
    pub collapsible_probable: bool,
    /// Unit weight for earth pressure (the heaviest shallow soil), kN/m³.
    pub unit_weight_kn_m3: f64,
    /// Friction angle for earth pressure (the weakest shallow soil), degrees.
    pub friction_deg: f64,
    /// Sewage absorption rate for the probable soils, L/m²·dia.
    pub absorption_l_m2_day: f64,
    /// … and for the worst soil the profile admits.
    pub worst_absorption_l_m2_day: f64,
    /// A competent layer (N ≥ 10) is expected at depth, for pile tips.
    pub firm_layer_at_depth: bool,
    /// Hand-dug piles may hit hard nodules.
    pub hard_digging: bool,
    /// What the profile means, in plain words.
    pub notes: Vec<String>,
}

impl PresumedSoil {
    /// Checks the profile: at least one layer, at least one near the surface.
    pub fn new(layers: Vec<PresumedLayer>, source: Option<String>) -> Result<Self, String> {
        if layers.is_empty() {
            return Err("Informe pelo menos uma camada do solo provável.".to_owned());
        }
        if !layers.iter().any(|l| l.likelihood.shallow()) {
            return Err(
                "Informe pelo menos uma camada provável ou possível perto da superfície."
                    .to_owned(),
            );
        }
        Ok(Self {
            layers,
            source: source.filter(|s| !s.trim().is_empty()),
        })
    }

    fn shallow(&self) -> impl Iterator<Item = &PresumedLayer> {
        self.layers.iter().filter(|l| l.likelihood.shallow())
    }

    pub fn analysis(&self) -> PresumedSoilAnalysis {
        let shallow: Vec<&PresumedLayer> = self.shallow().collect();
        let probable: Vec<&PresumedLayer> = shallow
            .iter()
            .copied()
            .filter(|l| l.likelihood == Likelihood::Probable)
            .collect();
        let design_set = if probable.is_empty() {
            &shallow
        } else {
            &probable
        };
        let (governing_soil, governing_n) = shallow
            .iter()
            .map(|l| (l.soil, l.soil.properties().n_range.0))
            .min_by_key(|(_, n)| *n)
            .unwrap_or((PresumedSoilType::RedSandyClay, 3));
        let by_teixeira = 20.0 * f64::from(governing_n.max(1));
        let allowable_kpa = ((by_teixeira.min(CONSERVATIVE_SOIL_KPA) / 10.0).floor() * 10.0)
            .clamp(20.0, CONSERVATIVE_SOIL_KPA);
        let collapsible = shallow.iter().any(|l| l.soil.properties().collapsible);
        let collapsible_probable = probable.iter().any(|l| l.soil.properties().collapsible);
        let unit_weight_kn_m3 = shallow
            .iter()
            .map(|l| l.soil.properties().unit_weight_kn_m3)
            .fold(16.0, f64::max);
        let friction_deg = shallow
            .iter()
            .map(|l| l.soil.properties().friction_deg)
            .fold(45.0, f64::min);
        let absorption_l_m2_day = design_set
            .iter()
            .map(|l| l.soil.properties().absorption_l_m2_day)
            .fold(f64::INFINITY, f64::min);
        let worst_absorption_l_m2_day = shallow
            .iter()
            .map(|l| l.soil.properties().absorption_l_m2_day)
            .fold(f64::INFINITY, f64::min);
        let firm_layer_at_depth = self.layers.iter().any(|l| {
            l.likelihood == Likelihood::PossibleAtDepth && l.soil.properties().n_range.0 >= 10
        });
        let hard_digging = self.layers.iter().any(|l| {
            matches!(
                l.soil,
                PresumedSoilType::LateriticConcretions | PresumedSoilType::WeatheredRock
            )
        });

        let mut notes = vec![format!(
            "Sem sondagem, o Trazo usa o perfil provável só para ficar mais cuidadoso: {} (N provável a partir de {governing_n}) dá cerca de {} kPa para sapatas rasas (σ ≈ 20·N), nunca mais que os {:.0} kPa seguros sem sondagem.",
            governing_soil.label().to_lowercase(),
            decimal(allowable_kpa, 0),
            CONSERVATIVE_SOIL_KPA
        )];
        if collapsible {
            notes.push(
                "Solo vermelho poroso pode ser colapsível: quando encharca (vazamento de piscina, fossa, sumidouro, cano furado, água de telhado), ele afunda de repente sob as fundações e a casa racha. Mantenha a água longe das fundações e confirme com sondagem e ensaio de colapso."
                    .to_owned(),
            );
        }
        if governing_n < super::survey::MIN_N_FOOTINGS as u32 {
            notes.push(format!(
                "Com N provável abaixo de {:.0}, sapatas rasas não são aconselháveis sem sondagem: prefira brocas ou estacas que atravessem a camada fraca.",
                super::survey::MIN_N_FOOTINGS
            ));
        }
        if firm_layer_at_depth {
            notes.push(
                "Uma camada mais firme (alteração de rocha) pode aparecer em profundidade: só a sondagem diz a que profundidade, e é nela que brocas e estacas devem apoiar a ponta."
                    .to_owned(),
            );
        }
        if hard_digging {
            notes.push(
                "Concreções lateríticas ou rocha alterada podem travar o trado manual da broca: tenha um plano B (estaca escavada mecanizada) antes de começar."
                    .to_owned(),
            );
        }
        notes.push(format!(
            "Infiltração do esgoto (sumidouro): cerca de {} L/m²·dia nos solos prováveis{}; o teste de infiltração no terreno (NBR 13969) é que vale.",
            decimal(absorption_l_m2_day, 0),
            if worst_absorption_l_m2_day + 1e-9 < absorption_l_m2_day {
                format!(
                    ", podendo cair a {} L/m²·dia se aparecer a camada possível menos permeável",
                    decimal(worst_absorption_l_m2_day, 0)
                )
            } else {
                String::new()
            }
        ));
        PresumedSoilAnalysis {
            governing_n,
            allowable_kpa,
            governing_soil,
            collapsible,
            collapsible_probable,
            unit_weight_kn_m3,
            friction_deg,
            absorption_l_m2_day,
            worst_absorption_l_m2_day,
            firm_layer_at_depth,
            hard_digging,
            notes,
        }
    }
}

impl crate::domain::model::Project {
    /// Keeps the presumed profile (or removes it with `None`). With no soil
    /// survey and no evidence behind the current allowable pressure, a
    /// presumed profile that allows less brings the pressure down to it: a
    /// presumed profile only ever makes the calculations safer. Returns the
    /// pressure adopted when it changed.
    pub fn set_presumed_soil(&mut self, soil: Option<PresumedSoil>) -> Option<f64> {
        self.structure.presumed_soil = soil;
        let presumed = self
            .structure
            .presumed_soil
            .as_ref()?
            .analysis()
            .allowable_kpa;
        if self.structure.soil_survey.is_some() {
            return None;
        }
        let backed = self
            .protection
            .hypotheses
            .record(crate::domain::protection::hypotheses::HypothesisKey::SoilPressure)
            .evidence
            .is_some();
        let current = self.structure.settings.soil_kpa;
        (current > presumed + 1e-9 && !backed).then(|| {
            self.structure.settings.soil_kpa = presumed;
            presumed
        })
    }
}

/// "Argila Arenosa  Vermelha" → "argila_arenosa_vermelha"; accents dropped.
fn normalise(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            ' ' | '-' => '_',
            other => other,
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(soil: PresumedSoilType, likelihood: Likelihood) -> PresumedLayer {
        PresumedLayer { soil, likelihood }
    }

    /// The profile of a red tropical soil over sandstone or basalt.
    fn red_profile() -> PresumedSoil {
        PresumedSoil::new(
            vec![
                layer(PresumedSoilType::RedSandyClay, Likelihood::Probable),
                layer(PresumedSoilType::RedClayeySand, Likelihood::Probable),
                layer(PresumedSoilType::SiltySandyClay, Likelihood::Possible),
                layer(PresumedSoilType::SiltyClay, Likelihood::Possible),
                layer(PresumedSoilType::LateriticConcretions, Likelihood::Possible),
                layer(PresumedSoilType::WeatheredRock, Likelihood::PossibleAtDepth),
            ],
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_red_porous_profile_is_weaker_than_the_survey_free_default_and_collapsible() {
        let analysis = red_profile().analysis();
        assert_eq!(analysis.governing_n, 3);
        // 20 · 3 = 60 kPa, below the 100 kPa Trazo assumes without a survey.
        assert!((analysis.allowable_kpa - 60.0).abs() < 1e-9);
        assert!(analysis.collapsible && analysis.collapsible_probable);
        assert!(analysis.firm_layer_at_depth);
        assert!(analysis.hard_digging);
        // Design absorption follows the probable soils; the worst follows the silty clay.
        assert!((analysis.absorption_l_m2_day - 40.0).abs() < 1e-9);
        assert!((analysis.worst_absorption_l_m2_day - 20.0).abs() < 1e-9);
        // Earth pressure uses the weakest friction and the heaviest soil.
        assert!((analysis.friction_deg - 22.0).abs() < 1e-9);
        assert!((analysis.unit_weight_kn_m3 - 18.0).abs() < 1e-9);
    }

    #[test]
    fn applying_a_presumed_profile_only_lowers_a_pressure_nothing_backs() {
        let mut project = crate::domain::model::Project::default();
        project.structure.settings.soil_kpa = 200.0;
        assert_eq!(project.set_presumed_soil(Some(red_profile())), Some(60.0));
        assert!((project.structure.settings.soil_kpa - 60.0).abs() < 1e-9);
        // It never raises a lower value.
        project.structure.settings.soil_kpa = 50.0;
        assert_eq!(project.set_presumed_soil(Some(red_profile())), None);
        assert!((project.structure.settings.soil_kpa - 50.0).abs() < 1e-9);
        // Evidence behind the current value wins over a presumption.
        project.structure.settings.soil_kpa = 150.0;
        crate::domain::protection::hypotheses::give_evidence(
            &mut project,
            crate::domain::protection::hypotheses::HypothesisKey::SoilPressure,
            "laudo SPT do vizinho",
        )
        .unwrap();
        assert_eq!(project.set_presumed_soil(Some(red_profile())), None);
    }

    #[test]
    fn a_presumed_profile_never_allows_more_than_the_safe_default() {
        let rock = PresumedSoil::new(
            vec![layer(PresumedSoilType::WeatheredRock, Likelihood::Probable)],
            None,
        )
        .unwrap();
        assert!((rock.analysis().allowable_kpa - CONSERVATIVE_SOIL_KPA).abs() < 1e-9);
    }

    #[test]
    fn a_profile_needs_a_layer_near_the_surface() {
        assert!(PresumedSoil::new(Vec::new(), None).is_err());
        assert!(
            PresumedSoil::new(
                vec![layer(
                    PresumedSoilType::WeatheredRock,
                    Likelihood::PossibleAtDepth
                )],
                None
            )
            .is_err()
        );
    }

    #[test]
    fn names_are_read_with_or_without_accents_and_spaces() {
        assert_eq!(
            PresumedSoilType::parse("Argila arenosa vermelha"),
            Some(PresumedSoilType::RedSandyClay)
        );
        assert_eq!(
            PresumedSoilType::parse("Solo com concreções lateríticas"),
            Some(PresumedSoilType::LateriticConcretions)
        );
        assert_eq!(
            PresumedSoilType::parse("Solo de alteração de arenito ou basalto"),
            Some(PresumedSoilType::WeatheredRock)
        );
        assert_eq!(
            PresumedSoilType::parse("solo_concrecoes_lateriticas"),
            Some(PresumedSoilType::LateriticConcretions)
        );
        assert_eq!(
            Likelihood::parse("Possível em profundidade"),
            Some(Likelihood::PossibleAtDepth)
        );
    }
}
