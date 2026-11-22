//! Applicability limits per module (#102). Each method in
//! `docs/engineering.md` holds only within a range of buildings; outside it
//! Trazo refuses to size, says why and asks for a professional, instead of
//! producing numbers that look trustworthy and are not.
//!
//! Project-wide limits (storeys, storey height, live load) refuse the whole
//! structural sizing; element limits (a beam span, a slab span, a footing on
//! soft soil) refuse that element alone.

use super::electrical::LoadSchedule;
use super::model::Project;
use super::structure::StructureReport;

/// Storeys the structural pre-sizing covers (houses and small buildings).
pub const MAX_STOREYS: u32 = 3;
/// Longest beam span the beam rules (L/10, L/12) and the analysis cover, m.
pub const MAX_BEAM_SPAN_M: f64 = 7.0;
/// Longest smaller slab span the thickness tables cover, m.
pub const MAX_SLAB_SPAN_M: f64 = 6.5;
/// Tallest storey: the columns are sized without a buckling check, m.
pub const MAX_STOREY_HEIGHT_M: f64 = 4.0;
/// Highest live load: beyond it the use is not residential or office, kN/m².
pub const MAX_LIVE_LOAD_KN_M2: f64 = 5.0;
/// Weakest soil for spread footings without a geotechnical study, kPa.
pub const MIN_SOIL_KPA: f64 = 100.0;
/// Largest demand a low-voltage connection takes; beyond it the utility
/// requires its own transformer, kVA.
pub const MAX_DEMAND_KVA: f64 = 75.0;

/// What a limit applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The whole structural pre-sizing is refused.
    Structure,
    /// One element is refused.
    Element,
    /// The service entrance is refused.
    Entrance,
}

/// A limit the project or one of its elements goes beyond.
#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    pub scope: Scope,
    /// The element's name as the verdicts use it ("Viga V3 · vão 1"), or
    /// the module for project-wide limits.
    pub element: String,
    pub value: f64,
    pub limit: f64,
    /// Why Trazo refuses, and what to do.
    pub message: String,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

const PROFESSIONAL: &str =
    "Procure um engenheiro estrutural: o Trazo não dimensiona fora desses limites.";

/// The storeys the structure carries: the real floors with structure, or the
/// *floors* setting when only one is drawn.
pub fn storeys(project: &Project) -> u32 {
    let real = project
        .floors
        .levels
        .iter()
        .filter(|level| {
            !level.data.beams.is_empty()
                || level
                    .data
                    .fixtures
                    .iter()
                    .any(|f| f.kind == super::fixtures::FixtureKind::Column)
        })
        .count() as u32;
    // The active floor's data lives in the project itself, not in its level.
    let active_has_structure =
        !project.structure.beams.is_empty() || project.structural_columns().next().is_some();
    let drawn = real + u32::from(active_has_structure);
    drawn.max(project.structure.settings.floors.max(1))
}

/// Project-wide structural limits.
pub fn structure_violations(project: &Project) -> Vec<Violation> {
    let mut out = Vec::new();
    let s = project.structure.settings;
    let storeys = storeys(project);
    if storeys > MAX_STOREYS {
        out.push(Violation {
            scope: Scope::Structure,
            element: "Estrutura".to_owned(),
            value: f64::from(storeys),
            limit: f64::from(MAX_STOREYS),
            message: format!(
                "{storeys} pavimentos: o pré-dimensionamento vale até {MAX_STOREYS}. {PROFESSIONAL}"
            ),
        });
    }
    let mut heights: Vec<f64> = vec![project.wall_height];
    let active_id = project.floors.active_floor_id;
    for level in &project.floors.levels {
        if Some(level.id) != active_id {
            heights.push(level.ceiling_height_m);
            heights.extend(level.data.walls.iter().map(|w| {
                let (start, end) = w.heights(level.ceiling_height_m);
                start.max(end)
            }));
        }
    }
    // Walls with their own, taller top (#145) count too.
    heights.extend(project.walls.iter().map(|w| {
        let (start, end) = w.heights(project.wall_height);
        start.max(end)
    }));
    let tallest = heights.into_iter().fold(0.0, f64::max);
    if tallest > MAX_STOREY_HEIGHT_M + 1e-9 {
        out.push(Violation {
            scope: Scope::Structure,
            element: "Estrutura".to_owned(),
            value: tallest,
            limit: MAX_STOREY_HEIGHT_M,
            message: format!(
                "Pé-direito ou parede de {} m: pilares acima de {} m precisam de verificação de flambagem. {PROFESSIONAL}",
                decimal(tallest, 2),
                decimal(MAX_STOREY_HEIGHT_M, 1)
            ),
        });
    }
    if s.live_kn_m2 > MAX_LIVE_LOAD_KN_M2 + 1e-9 {
        out.push(Violation {
            scope: Scope::Structure,
            element: "Estrutura".to_owned(),
            value: s.live_kn_m2,
            limit: MAX_LIVE_LOAD_KN_M2,
            message: format!(
                "Sobrecarga de {} kN/m²: acima de {} kN/m² o uso não é residencial nem de escritório. {PROFESSIONAL}",
                decimal(s.live_kn_m2, 1),
                decimal(MAX_LIVE_LOAD_KN_M2, 1)
            ),
        });
    }
    out
}

/// Element limits, from the pre-sizing report.
pub fn element_violations(report: &StructureReport) -> Vec<Violation> {
    let mut out = Vec::new();
    for line in &report.beams {
        for (i, span) in line.spans.iter().enumerate() {
            if span.length_m > MAX_BEAM_SPAN_M + 1e-9 {
                let element = if line.spans.len() > 1 {
                    format!("Viga {} · vão {}", line.label, i + 1)
                } else {
                    format!("Viga {}", line.label)
                };
                out.push(Violation {
                    scope: Scope::Element,
                    element,
                    value: span.length_m,
                    limit: MAX_BEAM_SPAN_M,
                    message: format!(
                        "Vão de {} m: as regras de pré-dimensionamento valem até {} m. Coloque um pilar no meio ou {}",
                        decimal(span.length_m, 2),
                        decimal(MAX_BEAM_SPAN_M, 1),
                        PROFESSIONAL.to_lowercase()
                    ),
                });
            }
        }
    }
    for slab in &report.slabs {
        if slab.lx_m > MAX_SLAB_SPAN_M + 1e-9 {
            out.push(Violation {
                scope: Scope::Element,
                element: format!("Laje {} ({})", slab.label, slab.room),
                value: slab.lx_m,
                limit: MAX_SLAB_SPAN_M,
                message: format!(
                    "Menor vão de {} m: as tabelas de espessura valem até {} m. Divida a laje com uma viga ou {}",
                    decimal(slab.lx_m, 2),
                    decimal(MAX_SLAB_SPAN_M, 1),
                    PROFESSIONAL.to_lowercase()
                ),
            });
        }
    }
    for footing in &report.footings {
        if footing.soil_kpa < MIN_SOIL_KPA - 1e-9 {
            out.push(Violation {
                scope: Scope::Element,
                element: format!("Sapata {}", footing.label),
                value: footing.soil_kpa,
                limit: MIN_SOIL_KPA,
                message: format!(
                    "Solo de {:.0} kPa: em solo mole, sapata exige sondagem e estudo geotécnico (fundação profunda, radier). Procure um engenheiro geotécnico.",
                    footing.soil_kpa
                ),
            });
        }
    }
    for wf in &report.wall_foundations {
        if wf.kind == super::wall_foundations::WallFoundationKind::StripFooting
            && wf.soil_kpa < MIN_SOIL_KPA - 1e-9
        {
            out.push(Violation {
                scope: Scope::Element,
                element: wf.label.clone(),
                value: wf.soil_kpa,
                limit: MIN_SOIL_KPA,
                message: format!(
                    "Solo de {:.0} kPa: em solo mole, sapata corrida exige sondagem e estudo geotécnico. Procure um engenheiro geotécnico.",
                    wf.soil_kpa
                ),
            });
        }
    }
    out
}

/// The electrical limit: a demand beyond low voltage.
pub fn electrical_violations(schedule: &LoadSchedule) -> Vec<Violation> {
    let kva = schedule.demand.demand_va / 1000.0;
    if kva > MAX_DEMAND_KVA + 1e-9 {
        vec![Violation {
            scope: Scope::Entrance,
            element: "Entrada de energia".to_owned(),
            value: kva,
            limit: MAX_DEMAND_KVA,
            message: format!(
                "Demanda de {} kVA: acima de {:.0} kVA a ligação em baixa tensão não vale e a distribuidora exige transformador próprio. Procure um engenheiro eletricista.",
                decimal(kva, 1),
                MAX_DEMAND_KVA
            ),
        }]
    } else {
        Vec::new()
    }
}

/// Why the structural sizing is refused for the whole project, if it is.
pub fn structural_refusal(project: &Project) -> Option<String> {
    let violations = structure_violations(project);
    (!violations.is_empty()).then(|| {
        violations
            .iter()
            .map(|v| v.message.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::geometry::Point;
    use crate::domain::verdict::{Light, safety_summary};

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        project
    }

    #[test]
    fn a_house_within_the_limits_is_not_refused() {
        let project = house();
        assert!(structure_violations(&project).is_empty());
        assert!(element_violations(&project.structure_report()).is_empty());
        assert!(structural_refusal(&project).is_none());
        assert_eq!(storeys(&project), 1);
    }

    #[test]
    fn a_nine_metre_span_is_refused_with_an_explanation() {
        let mut project = house();
        let a = project
            .add_fixture(FixtureKind::Column, Point::new(10.0, 10.0), 0.30, 0.14, 0)
            .unwrap();
        let b = project
            .add_fixture(FixtureKind::Column, Point::new(19.0, 10.0), 0.30, 0.14, 0)
            .unwrap();
        project.add_beam(a, b, 0.14, 0.90).unwrap();
        let violations = element_violations(&project.structure_report());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].message.contains("9,00 m"));
        assert!(violations[0].message.contains("engenheiro"));
        // The beam is refused: red, with no sizing numbers.
        let summary = safety_summary(&project);
        let verdict = summary
            .verdicts
            .iter()
            .find(|v| v.element == violations[0].element)
            .unwrap();
        assert_eq!(verdict.light(), Light::Red);
        assert_eq!(verdict.checks.len(), 1);
        assert_eq!(verdict.checks[0].name, "Fora dos limites");
    }

    #[test]
    fn too_many_storeys_or_too_much_load_refuse_the_whole_structure() {
        let mut project = house();
        project.structure.settings.floors = 4;
        let refusal = structural_refusal(&project).unwrap();
        assert!(refusal.contains("4 pavimentos"), "{refusal}");
        let summary = safety_summary(&project);
        assert_eq!(summary.verdicts[0].element, "Estrutura");
        assert_eq!(summary.verdicts[0].light(), Light::Red);
        // No structural element is sized while the project is refused.
        assert!(summary.verdicts.iter().all(|v| v.element == "Estrutura"
            || v.discipline != crate::domain::verdict::Discipline::Structure));
        project.structure.settings.floors = 1;
        project.structure.settings.live_kn_m2 = 6.0;
        assert!(structural_refusal(&project).unwrap().contains("6,0 kN/m²"));
        project.structure.settings.live_kn_m2 = 1.5;
        project.wall_height = 4.5;
        assert!(structural_refusal(&project).unwrap().contains("flambagem"));
    }

    #[test]
    fn footings_on_soft_soil_are_refused() {
        let mut project = house();
        project.structure.settings.soil_kpa = 80.0;
        let violations = element_violations(&project.structure_report());
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.element.starts_with("Sapata")));
        assert!(violations[0].message.contains("geotécnico"));
    }
}
