//! Calculation hypotheses and their origin (#168).
//!
//! Every structural result rests on a few values nobody can see in the
//! drawing: the soil's allowable pressure, the concrete strength, the loads,
//! the wind. Each one records who set it (Trazo's default, the person or the
//! AI assistant) and the evidence behind it, so that Trazo can contest a
//! favourable value that nothing backs up.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::decimal;
use crate::domain::model::Project;
use crate::domain::wind::WindSettings;

/// One calculation hypothesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisKey {
    SoilPressure,
    ConcreteStrength,
    LiveLoad,
    FinishLoad,
    MasonryWeight,
    Occupancy,
    WindSpeed,
    WindCategory,
    WindTopography,
    WindStatistical,
}

impl HypothesisKey {
    pub const ALL: [Self; 10] = [
        Self::SoilPressure,
        Self::ConcreteStrength,
        Self::LiveLoad,
        Self::FinishLoad,
        Self::MasonryWeight,
        Self::Occupancy,
        Self::WindSpeed,
        Self::WindCategory,
        Self::WindTopography,
        Self::WindStatistical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SoilPressure => "Tensão admissível do solo",
            Self::ConcreteStrength => "Resistência do concreto (fck)",
            Self::LiveLoad => "Sobrecarga de uso",
            Self::FinishLoad => "Peso do revestimento",
            Self::MasonryWeight => "Peso da alvenaria",
            Self::Occupancy => "Uso da edificação",
            Self::WindSpeed => "Velocidade básica do vento (V0)",
            Self::WindCategory => "Categoria do terreno (vento)",
            Self::WindTopography => "Fator topográfico do vento (S1)",
            Self::WindStatistical => "Fator estatístico do vento (S3)",
        }
    }

    /// What the hypothesis means, for someone who never studied engineering.
    pub fn plain(self) -> &'static str {
        match self {
            Self::SoilPressure => {
                "Quanto peso cada pedaço do chão aguenta. Só uma sondagem do terreno diz isso com segurança."
            }
            Self::ConcreteStrength => {
                "Quão forte é o concreto. Concreto feito na obra costuma ser mais fraco que o usinado."
            }
            Self::LiveLoad => "O peso de pessoas, móveis e objetos sobre a laje.",
            Self::FinishLoad => "O peso do contrapiso e do piso sobre a laje.",
            Self::MasonryWeight => "O peso das paredes, que depende do tijolo ou bloco usado.",
            Self::Occupancy => "Para que o imóvel será usado: casa, escritório, garagem ou loja.",
            Self::WindSpeed => "A força do vento na região, tirada do mapa da norma para a cidade.",
            Self::WindCategory => {
                "O quanto o entorno protege a casa do vento: campo aberto venta mais que a cidade."
            }
            Self::WindTopography => {
                "Morros e encostas aumentam o vento; vales protegidos diminuem."
            }
            Self::WindStatistical => {
                "A importância do imóvel para o cálculo do vento (casas usam 1,0)."
            }
        }
    }

    /// The argument name assistants use for it through MCP.
    pub fn key(self) -> &'static str {
        match self {
            Self::SoilPressure => "solo_kpa",
            Self::ConcreteStrength => "fck_mpa",
            Self::LiveLoad => "sobrecarga_kn_m2",
            Self::FinishLoad => "revestimento_kn_m2",
            Self::MasonryWeight => "peso_alvenaria_kn_m3",
            Self::Occupancy => "ocupacao",
            Self::WindSpeed => "v0_ms",
            Self::WindCategory => "categoria_terreno",
            Self::WindTopography => "s1",
            Self::WindStatistical => "s3",
        }
    }

    /// The current value, as a number (the occupancy by its base live load).
    pub fn value(self, project: &Project) -> f64 {
        let settings = &project.structure.settings;
        let wind = &project.structure.wind;
        match self {
            Self::SoilPressure => settings.soil_kpa,
            Self::ConcreteStrength => settings.fck_mpa,
            Self::LiveLoad => settings.live_kn_m2,
            Self::FinishLoad => settings.finish_kn_m2,
            Self::MasonryWeight => settings.masonry_kn_m3,
            Self::Occupancy => settings.occupancy.live_kn_m2(),
            Self::WindSpeed => wind.v0_ms,
            Self::WindCategory => f64::from(wind.category),
            Self::WindTopography => wind.s1,
            Self::WindStatistical => wind.s3,
        }
    }

    /// The current value as people read it.
    pub fn display(self, project: &Project) -> String {
        self.format(project, self.value(project))
    }

    /// Any value of this hypothesis as people read it.
    pub fn format(self, project: &Project, value: f64) -> String {
        match self {
            Self::SoilPressure => format!("{value:.0} kPa"),
            Self::ConcreteStrength => format!("{value:.0} MPa"),
            Self::LiveLoad | Self::FinishLoad => format!("{} kN/m²", decimal(value, 1)),
            Self::MasonryWeight => format!("{value:.0} kN/m³"),
            Self::Occupancy => project.structure.settings.occupancy.label().to_owned(),
            Self::WindSpeed => format!("{value:.0} m/s"),
            Self::WindCategory => WindSettings::category_label(value.round() as u8).to_owned(),
            Self::WindTopography | Self::WindStatistical => decimal(value, 2),
        }
    }

    /// Whether `a` and `b` are the same setting.
    fn same(self, a: &Project, b: &Project) -> bool {
        match self {
            Self::Occupancy => a.structure.settings.occupancy == b.structure.settings.occupancy,
            _ => (self.value(a) - self.value(b)).abs() <= 1e-9,
        }
    }
}

/// Who set a hypothesis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisSource {
    /// Trazo's own default value.
    #[default]
    Default,
    /// The person, in the window.
    Person,
    /// An AI assistant, through MCP.
    Assistant,
}

impl HypothesisSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "valor padrão do Trazo",
            Self::Person => "definido pela pessoa",
            Self::Assistant => "definido pela IA",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Default => "padrao",
            Self::Person => "pessoa",
            Self::Assistant => "ia",
        }
    }
}

/// The origin of one hypothesis.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HypothesisRecord {
    #[serde(default)]
    pub source: HypothesisSource,
    /// What backs the value up: a soil survey, an invoice, a test, the city.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// The origin of every hypothesis that is not simply Trazo's default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hypotheses {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    records: BTreeMap<HypothesisKey, HypothesisRecord>,
}

impl Hypotheses {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The origin of `key` (Trazo's default when nobody changed it).
    pub fn record(&self, key: HypothesisKey) -> HypothesisRecord {
        self.records.get(&key).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, key: HypothesisKey, record: HypothesisRecord) {
        if record == HypothesisRecord::default() {
            self.records.remove(&key);
        } else {
            self.records.insert(key, record);
        }
    }
}

/// The evidence the project itself holds for the current value of `key`:
/// a soil survey that allows at least that pressure, or the city's wind
/// speed from the NBR 6123 map.
pub fn project_evidence(key: HypothesisKey, project: &Project) -> Option<String> {
    let value = key.value(project);
    match key {
        HypothesisKey::SoilPressure => {
            let kpa = survey_soil_kpa(project)?;
            (value <= kpa + 0.5).then(|| format!("sondagem SPT ({kpa:.0} kPa)"))
        }
        HypothesisKey::WindSpeed => {
            let site = &project.codes.site;
            let v0 = site.wind_v0_ms?;
            (value + 0.01 >= v0).then(|| {
                let place = site
                    .location_label()
                    .unwrap_or_else(|| "a cidade do terreno".to_owned());
                format!("mapa de isopletas da NBR 6123 para {place} ({v0:.0} m/s)")
            })
        }
        _ => None,
    }
}

/// The allowable pressure the soil survey gives for 1 m footings, rounded
/// down to 10 kPa as `Project::apply_survey_to_soil` adopts it.
pub fn survey_soil_kpa(project: &Project) -> Option<f64> {
    let kpa = project.structure.soil_survey.as_ref()?.footing_kpa(1.0)?;
    Some(((kpa / 10.0).floor() * 10.0).clamp(10.0, 600.0))
}

/// The evidence for `key`: what the person or the assistant gave, or what
/// the project itself holds.
pub fn evidence(key: HypothesisKey, project: &Project) -> Option<String> {
    project
        .protection
        .hypotheses
        .record(key)
        .evidence
        .or_else(|| project_evidence(key, project))
}

/// Records who changed each hypothesis in the edit from `previous` to
/// `current`. A hypothesis whose origin the edit already set explicitly
/// keeps it; any other changed hypothesis is credited to `source`, without
/// evidence until someone gives it.
pub fn track_changes(previous: &Project, current: &mut Project, source: HypothesisSource) {
    for key in HypothesisKey::ALL {
        if key.same(previous, current) {
            continue;
        }
        let before = previous.protection.hypotheses.record(key);
        if current.protection.hypotheses.record(key) != before {
            continue;
        }
        current.protection.hypotheses.set(
            key,
            HypothesisRecord {
                source,
                evidence: None,
            },
        );
    }
}

/// One line per hypothesis for the assumptions list: value, origin and
/// evidence.
pub fn origin_lines(project: &Project) -> Vec<String> {
    HypothesisKey::ALL
        .into_iter()
        .map(|key| {
            let record = project.protection.hypotheses.record(key);
            let evidence = evidence(key, project)
                .map_or_else(|| "sem prova".to_owned(), |e| format!("prova: {e}"));
            format!(
                "{}: {} · {} · {evidence}.",
                key.label(),
                key.display(project),
                record.source.label()
            )
        })
        .collect()
}

/// Allowable soil pressure Trazo can assume without a soil survey: the
/// traditional conservative 1 kgf/cm² (#160).
pub const CONSERVATIVE_SOIL_KPA: f64 = 100.0;
/// Concrete strength Trazo can assume without a specification: the lowest
/// class NBR 6118 allows for reinforced concrete (C20).
pub const CONSERVATIVE_FCK_MPA: f64 = 20.0;
/// Usual floor finish and screed, kN/m².
pub const USUAL_FINISH_KN_M2: f64 = 1.0;
/// Usual weight of plastered hollow ceramic block masonry, kN/m³.
pub const USUAL_MASONRY_KN_M3: f64 = 13.0;
/// Basic wind speed Trazo can assume while the city is unknown: covers the
/// NBR 6123 isopleth map almost everywhere outside the far south.
pub const CONSERVATIVE_V0_MS: f64 = 45.0;
/// Terrain category assumed for a house among others (III).
pub const REFERENCE_WIND_CATEGORY: f64 = 3.0;

/// The value Trazo can stand behind without evidence. A hypothesis more
/// favourable than this needs a reason; `None` when no value is safer than
/// another (the occupancy is a fact about the building, not a margin).
pub fn reference(key: HypothesisKey, project: &Project) -> Option<f64> {
    let settings = &project.structure.settings;
    match key {
        HypothesisKey::SoilPressure => {
            Some(survey_soil_kpa(project).unwrap_or(CONSERVATIVE_SOIL_KPA))
        }
        HypothesisKey::ConcreteStrength => Some(CONSERVATIVE_FCK_MPA),
        HypothesisKey::LiveLoad => Some(settings.occupancy.live_kn_m2()),
        HypothesisKey::FinishLoad => Some(USUAL_FINISH_KN_M2),
        HypothesisKey::MasonryWeight => Some(USUAL_MASONRY_KN_M3),
        HypothesisKey::Occupancy => None,
        HypothesisKey::WindSpeed => {
            Some(project.codes.site.wind_v0_ms.unwrap_or(CONSERVATIVE_V0_MS))
        }
        HypothesisKey::WindCategory => Some(REFERENCE_WIND_CATEGORY),
        HypothesisKey::WindTopography | HypothesisKey::WindStatistical => Some(1.0),
    }
}

impl HypothesisKey {
    /// Whether a larger value makes the structure look safer.
    pub fn favourable_when_higher(self) -> bool {
        matches!(
            self,
            Self::SoilPressure | Self::ConcreteStrength | Self::WindCategory
        )
    }

    /// Whether `a` is more favourable (makes the structure look safer) than `b`.
    pub fn more_favourable(self, a: f64, b: f64) -> bool {
        let tolerance = 1e-6 * b.abs().max(1.0);
        if self.favourable_when_higher() {
            a > b + tolerance
        } else {
            a < b - tolerance
        }
    }
}

/// A hypothesis Trazo does not accept as it stands (#169).
#[derive(Clone, Debug, PartialEq)]
pub struct Contest {
    pub key: HypothesisKey,
    /// Why, in plain words.
    pub reason: String,
    /// What to do, in plain words.
    pub fix: String,
    /// The value breaks a standard or contradicts the project's own data,
    /// so no evidence can make it acceptable.
    pub normative: bool,
}

/// Every hypothesis Trazo contests: a value more favourable than the one it
/// can stand behind with no evidence for it, or one beyond what the
/// standards and the project's own data allow (#169).
pub fn contests(project: &Project) -> Vec<Contest> {
    HypothesisKey::ALL
        .into_iter()
        .filter_map(|key| contest(key, project))
        .collect()
}

fn contest(key: HypothesisKey, project: &Project) -> Option<Contest> {
    let reference = reference(key, project)?;
    let value = key.value(project);
    if !key.more_favourable(value, reference) {
        return None;
    }
    let shown = key.display(project);
    let safe = key.format(project, reference);
    let origin = project.protection.hypotheses.record(key).source.label();
    let normative = match key {
        HypothesisKey::LiveLoad => true,
        HypothesisKey::WindSpeed => project.codes.site.wind_v0_ms.is_some(),
        HypothesisKey::SoilPressure => survey_soil_kpa(project).is_some(),
        _ => false,
    };
    if !normative && evidence(key, project).is_some() {
        return None;
    }
    let (reason, fix) = match key {
        HypothesisKey::SoilPressure if normative => (
            format!("O solo foi considerado com {shown}, mais do que a sondagem permite ({safe})."),
            format!("Use o valor da sondagem: {safe}."),
        ),
        HypothesisKey::SoilPressure => (
            format!("O solo foi considerado com {shown} sem sondagem ou outra prova ({origin})."),
            format!(
                "Faça uma sondagem e use o valor dela, ou volte para {safe}, o valor seguro sem sondagem."
            ),
        ),
        HypothesisKey::ConcreteStrength => (
            format!("O concreto foi considerado com fck {shown} sem prova ({origin})."),
            format!(
                "Use {safe}, ou informe a prova: concreto usinado com nota fiscal ou ensaio de resistência."
            ),
        ),
        HypothesisKey::LiveLoad => (
            format!(
                "A sobrecarga de {shown} é menor que o mínimo da NBR 6120 para o uso {} ({safe}).",
                project.structure.settings.occupancy.label()
            ),
            format!("Volte para pelo menos {safe}: a norma não permite menos."),
        ),
        HypothesisKey::FinishLoad => (
            format!(
                "O revestimento foi considerado com {shown}, mais leve que o usual ({safe}), sem prova ({origin})."
            ),
            format!("Use {safe}, ou informe o contrapiso e o piso reais."),
        ),
        HypothesisKey::MasonryWeight => (
            format!(
                "As paredes foram consideradas com {shown}, mais leves que blocos cerâmicos comuns ({safe}), sem prova ({origin})."
            ),
            format!(
                "Use {safe} ou mais (tijolo maciço e bloco de concreto pesam mais), ou informe o bloco real."
            ),
        ),
        HypothesisKey::WindSpeed if normative => (
            format!(
                "O vento foi considerado com V0 = {shown}, menor que o da cidade ({safe}, NBR 6123)."
            ),
            format!("Use o V0 da cidade: {safe}."),
        ),
        HypothesisKey::WindSpeed => (
            format!("O vento foi considerado com V0 = {shown} sem a cidade definida ({origin})."),
            format!("Defina a cidade do terreno em Normas › Local e clima, ou use {safe}."),
        ),
        HypothesisKey::WindCategory => (
            format!(
                "O terreno foi considerado mais protegido do vento ({shown}) sem prova ({origin})."
            ),
            format!("Use {safe}, ou informe por que o entorno protege a casa."),
        ),
        HypothesisKey::WindTopography => (
            format!(
                "O fator topográfico {shown} reduz o vento (vale protegido) sem prova ({origin})."
            ),
            "Use 1,00. Em morro ou encosta o vento pode ser maior: procure um profissional."
                .to_owned(),
        ),
        HypothesisKey::WindStatistical => (
            format!("O fator estatístico {shown} reduz o vento sem prova ({origin})."),
            "Use 1,00, o valor da norma para residências.".to_owned(),
        ),
        HypothesisKey::Occupancy => return None,
    };
    Some(Contest {
        key,
        reason,
        fix,
        normative,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_credits_the_changed_hypotheses_to_who_made_it() {
        let before = Project::default();
        let mut after = before.clone();
        after.structure.settings.soil_kpa = 300.0;
        after.structure.wind.v0_ms = 40.0;
        track_changes(&before, &mut after, HypothesisSource::Assistant);
        let hypotheses = &after.protection.hypotheses;
        assert_eq!(
            hypotheses.record(HypothesisKey::SoilPressure).source,
            HypothesisSource::Assistant
        );
        assert_eq!(
            hypotheses.record(HypothesisKey::WindSpeed).source,
            HypothesisSource::Assistant
        );
        assert_eq!(
            hypotheses.record(HypothesisKey::ConcreteStrength).source,
            HypothesisSource::Default
        );
    }

    #[test]
    fn an_origin_set_by_the_edit_itself_is_kept() {
        let before = Project::default();
        let mut after = before.clone();
        after.structure.settings.fck_mpa = 30.0;
        after.protection.hypotheses.set(
            HypothesisKey::ConcreteStrength,
            HypothesisRecord {
                source: HypothesisSource::Person,
                evidence: Some("nota fiscal do concreto usinado C30".to_owned()),
            },
        );
        track_changes(&before, &mut after, HypothesisSource::Assistant);
        let record = after
            .protection
            .hypotheses
            .record(HypothesisKey::ConcreteStrength);
        assert_eq!(record.source, HypothesisSource::Person);
        assert!(record.evidence.is_some());
    }

    #[test]
    fn a_favourable_soil_without_evidence_is_contested_and_evidence_settles_it() {
        let mut project = Project::default();
        project.structure.settings.soil_kpa = 300.0;
        assert!(
            contests(&project)
                .iter()
                .any(|c| c.key == HypothesisKey::SoilPressure && !c.normative)
        );
        project.protection.hypotheses.set(
            HypothesisKey::SoilPressure,
            HypothesisRecord {
                source: HypothesisSource::Person,
                evidence: Some("laudo de sondagem SP-01".to_owned()),
            },
        );
        assert!(
            contests(&project)
                .iter()
                .all(|c| c.key != HypothesisKey::SoilPressure)
        );
    }

    #[test]
    fn a_live_load_below_the_standard_is_contested_whatever_the_evidence() {
        let mut project = Project::default();
        project.structure.settings.live_kn_m2 = 1.0;
        project.protection.hypotheses.set(
            HypothesisKey::LiveLoad,
            HypothesisRecord {
                source: HypothesisSource::Assistant,
                evidence: Some("a família disse que a casa é leve".to_owned()),
            },
        );
        assert!(
            contests(&project)
                .iter()
                .any(|c| c.key == HypothesisKey::LiveLoad && c.normative)
        );
    }

    #[test]
    fn conservative_values_are_never_contested() {
        let mut project = Project::default();
        project.structure.settings.soil_kpa = CONSERVATIVE_SOIL_KPA;
        project.structure.settings.fck_mpa = CONSERVATIVE_FCK_MPA;
        project.structure.wind.v0_ms = 50.0;
        assert!(contests(&project).is_empty());
    }

    #[test]
    fn origin_lines_name_the_source_and_the_evidence() {
        let mut project = Project::default();
        project.protection.hypotheses.set(
            HypothesisKey::SoilPressure,
            HypothesisRecord {
                source: HypothesisSource::Assistant,
                evidence: None,
            },
        );
        let lines = origin_lines(&project);
        assert_eq!(lines.len(), HypothesisKey::ALL.len());
        assert!(lines[0].contains("definido pela IA"));
        assert!(lines[0].contains("sem prova"));
    }
}
