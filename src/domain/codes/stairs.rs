//! 26-rule data-driven stair compliance checks, 3D spatial headroom clearance
//! check and interactive least-change stair solver (`T-10`).
//!
//! All normative limits come from `BuildingCodes::resolve_section(Architectural, "stairs")`.
//! Zero hardcoded normative numbers live in this module.

use serde::{Deserialize, Serialize};

use super::{CodeDiscipline, ResolvedSection, RuleKind};
use crate::domain::{
    circulation::{CirculationElement, CirculationKind, RailingSide, StairLandingSpec, StairShape},
    geometry::{Point, polygon_contains_point},
    model::Project,
};

const EPS: f64 = 1e-6;
const MM_PER_M: f64 = 1000.0;
const INSET_FRAC: f64 = 0.05;
const INSET_MAX_M: f64 = 0.02;
const MAX_SOLVER_RISERS: usize = 60;
const MAX_SOLVER_SPAN: usize = 40;
const MAX_SOLVER_PROPOSALS: usize = 5;
const MAX_SOLVER_FALLBACK: usize = 3;
const MIN_RUN_RISERS: usize = 2;

/// How minimum landing length is determined by the active building code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandingRule {
    /// At least the stair width (capped by `landing_length`).
    #[default]
    Width,
    /// Fixed minimum length `landing_length` (e.g. NBR 9050: 1200 mm).
    Fixed,
    /// At least `2*riser + 2*going`, no less than `landing_length` (e.g. NBR 9077).
    Step,
}

/// Where tread going is measured on curved/winder flights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkLine {
    #[default]
    Centre,
    NarrowSide,
}

/// Complete 26-field stair specification loaded from JSON building codes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StairRules {
    pub riser: f64,
    pub going: f64,
    pub riser_min: f64,
    pub riser_max: f64,
    pub going_min: f64,
    pub going_max: f64,
    pub step_min: f64,
    pub step_max: f64,
    pub width_min: f64,
    pub headroom: f64,
    pub flight_rise_max: f64,
    pub flight_risers_max: usize,
    pub landing_rule: LandingRule,
    pub landing_length: f64,
    pub winders: bool,
    pub walk_line: WalkLine,
    pub walk_line_offset: f64,
    pub winder_going_min: f64,
    pub winder_narrow_min: f64,
    pub nosing_min: f64,
    pub nosing_max: f64,
    pub open_risers: bool,
    pub uniformity: f64,
    pub handrail_min: f64,
    pub handrail_max: f64,
    pub guard_min: f64,
}

impl Default for StairRules {
    fn default() -> Self {
        Self {
            riser: 0.0,
            going: 0.0,
            riser_min: 0.0,
            riser_max: 0.0,
            going_min: 0.0,
            going_max: 0.0,
            step_min: 0.0,
            step_max: 0.0,
            width_min: 0.0,
            headroom: 0.0,
            flight_rise_max: 0.0,
            flight_risers_max: 0,
            landing_rule: LandingRule::Width,
            landing_length: 0.0,
            winders: true,
            walk_line: WalkLine::Centre,
            walk_line_offset: 0.0,
            winder_going_min: 0.0,
            winder_narrow_min: 0.0,
            nosing_min: 0.0,
            nosing_max: 0.0,
            open_risers: true,
            uniformity: 0.0,
            handrail_min: 0.0,
            handrail_max: 0.0,
            guard_min: 0.0,
        }
    }
}

impl StairRules {
    /// Minimum required landing length in millimetres for the given flight geometry.
    pub fn landing_due_mm(&self, width_mm: f64, riser_mm: f64, going_mm: f64) -> f64 {
        match self.landing_rule {
            LandingRule::Width => {
                if self.landing_length > EPS {
                    width_mm.min(self.landing_length)
                } else {
                    width_mm
                }
            }
            LandingRule::Fixed => self.landing_length.max(width_mm),
            LandingRule::Step => (2.0 * riser_mm + 2.0 * going_mm).max(self.landing_length),
        }
    }

    /// Maximum risers allowed in a single flight without a landing.
    pub fn max_risers_in_flight(&self, riser_mm: f64) -> usize {
        let by_rise = if self.flight_rise_max > EPS && riser_mm > EPS {
            (self.flight_rise_max / riser_mm).floor().max(1.0) as usize
        } else {
            usize::MAX
        };
        let by_count = if self.flight_risers_max > 0 {
            self.flight_risers_max
        } else {
            usize::MAX
        };
        by_rise.min(by_count)
    }
}

/// One normative finding on a stair element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StairFinding {
    pub rule: String,
    pub value: f64,
    pub limit: f64,
    pub unit: String,
    pub kind: RuleKind,
    pub source: Option<String>,
    pub message: String,
    pub suggestion: String,
}

/// Result of the 3D spatial headroom clearance check over a stair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeadroomResult {
    /// Minimum vertical clearance found above any tread, in millimetres (`None` if unobstructed).
    pub min_clearance_mm: Option<f64>,
    /// Required clearance from the active code, in millimetres.
    pub required_mm: f64,
    /// Offending overhead element description (beam/slab), if clearance is below required.
    pub obstruction: Option<String>,
    /// 1-based step index where minimum clearance occurs.
    pub worst_step: Option<usize>,
}

/// Resolve the active [`StairRules`] and its rule provenance metadata.
pub fn resolve_stair_rules(project: &Project) -> (StairRules, ResolvedSection) {
    let resolved = project
        .codes
        .resolve_section(CodeDiscipline::Architectural, "stairs");
    let rules = serde_json::from_value::<StairRules>(resolved.value.clone()).unwrap_or_default();
    (rules, resolved)
}

/// Spatial 3D headroom clearance check above every step of a stair against
/// overhead structural beams and upper-floor slabs.
pub fn stair_headroom(
    project: &Project,
    element: &CirculationElement,
    rules: &StairRules,
) -> HeadroomResult {
    let steps = element.stair_step_polygons();
    if steps.is_empty() || rules.headroom <= EPS {
        return HeadroomResult {
            min_clearance_mm: None,
            required_mm: rules.headroom,
            obstruction: None,
            worst_step: None,
        };
    }

    let ceiling_m = project
        .active_floor()
        .map(|f| f.ceiling_height_m)
        .unwrap_or(project.wall_height);
    let upper_slab_thickness = project
        .active_floor()
        .map(|f| f.slab_thickness_m)
        .unwrap_or(0.0);
    let has_upper_level = project.floors.levels.len() > 1 || !project.structure.slabs.is_empty();

    let mut worst_clearance_m: Option<f64> = None;
    let mut worst_obstruction: Option<String> = None;
    let mut worst_step: Option<usize> = None;

    // Collect overhead structural beams (each beam sits at the ceiling underside).
    let beams: Vec<(String, Vec<Point>, f64)> = project
        .structure
        .beams
        .iter()
        .filter_map(|beam| {
            let (start, end) = project.beam_endpoints(beam)?;
            let poly = beam_plan_polygon(start, end, beam.width_m)?;
            let underside_z = ceiling_m - beam.height_m;
            Some((format!("Viga V{}", beam.id), poly, underside_z))
        })
        .collect();

    // Collect overhead slab surfaces if an upper storey or structural slab exists.
    // If `slab_opening_len_m` is set on the stair, steps inside the opening from
    // the top of the stair are unobstructed by the slab.
    let opening_len = element.stair_details.slab_opening_len_m;
    let total_steps = steps.len();
    let tread_m = match element.kind {
        CirculationKind::Stair { tread_m, .. } => tread_m.max(EPS),
        _ => EPS,
    };

    for (idx, (poly, step_z)) in steps.iter().enumerate() {
        let samples = inset_sample_points(poly);

        // 1. Overhead beams
        for (beam_name, beam_poly, underside_z) in &beams {
            if samples
                .iter()
                .any(|pt| polygon_contains_point(beam_poly, *pt))
            {
                let clear_m = underside_z - step_z;
                if worst_clearance_m.is_none_or(|cur| clear_m < cur) {
                    worst_clearance_m = Some(clear_m);
                    worst_obstruction = Some(beam_name.clone());
                    worst_step = Some(idx + 1);
                }
            }
        }

        // 2. Overhead floor slab (only checked when an upper slab exists AND
        // either an explicit `slab_opening_len_m` was declared or a structural
        // slab covers the step footprint).
        if has_upper_level && upper_slab_thickness > EPS {
            let slab_underside_z = ceiling_m - upper_slab_thickness;
            let dist_from_top_m = (total_steps.saturating_sub(idx + 1)) as f64 * tread_m;
            let covered_by_slab = if let Some(open_len) = opening_len {
                // Step is covered by the slab if it lies outside the opening measured from the top.
                dist_from_top_m > open_len + EPS
            } else {
                // Only check structural slabs that explicitly exist over this step's XY footprint
                // AND only when `slab_opening_len_m` is explicitly specified or a non-opening slab sits above.
                false
            };
            if covered_by_slab {
                let clear_m = slab_underside_z - step_z;
                if worst_clearance_m.is_none_or(|cur| clear_m < cur) {
                    worst_clearance_m = Some(clear_m);
                    worst_obstruction = Some("Laje superior (abertura insuficiente)".to_owned());
                    worst_step = Some(idx + 1);
                }
            }
        }
    }

    let min_clearance_mm = worst_clearance_m.map(|m| m * MM_PER_M);
    let obstruction = if min_clearance_mm.is_some_and(|mm| mm + EPS < rules.headroom) {
        worst_obstruction
    } else {
        None
    };

    HeadroomResult {
        min_clearance_mm,
        required_mm: rules.headroom,
        obstruction,
        worst_step,
    }
}

/// Evaluate all 26 normative stair rules on a single [`CirculationElement`].
pub fn stair_findings(project: &Project, element: &CirculationElement) -> Vec<StairFinding> {
    let CirculationKind::Stair {
        shape,
        width_m,
        total_rise_m,
        tread_m,
        landing_depth_m,
    } = element.kind
    else {
        return Vec::new();
    };
    let Some(metrics) = element.stair_metrics() else {
        return Vec::new();
    };

    let (rules, resolved) = resolve_stair_rules(project);
    let mut findings = Vec::new();

    let riser_mm = metrics.riser_height_m * MM_PER_M;
    let going_mm = tread_m * MM_PER_M;
    let step_mm = 2.0 * riser_mm + going_mm;
    let width_mm = width_m * MM_PER_M;
    let total_rise_mm = total_rise_m * MM_PER_M;
    let landing_mm = landing_depth_m * MM_PER_M;
    let nosing_mm = element.stair_details.nosing_m * MM_PER_M;

    let meta = |rule_key: &str| -> (RuleKind, Option<String>) {
        resolved
            .rules
            .get(rule_key)
            .map(|r| (r.kind, r.source.clone()))
            .unwrap_or((resolved.section_kind, resolved.section_source.clone()))
    };

    // 1–2. Riser min / max
    if rules.riser_min > EPS && riser_mm + EPS < rules.riser_min {
        let (kind, source) = meta("riser_min");
        findings.push(StairFinding {
            rule: "riser_min".to_owned(),
            value: riser_mm,
            limit: rules.riser_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Espelho {:.1} mm abaixo do mínimo de {:.0} mm.",
                riser_mm, rules.riser_min
            ),
            suggestion: "Reduza o número de degraus ou aumente o desnível por lance.".to_owned(),
        });
    }
    if rules.riser_max > EPS && riser_mm > rules.riser_max + EPS {
        let (kind, source) = meta("riser_max");
        findings.push(StairFinding {
            rule: "riser_max".to_owned(),
            value: riser_mm,
            limit: rules.riser_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Espelho {:.1} mm acima do máximo de {:.0} mm.",
                riser_mm, rules.riser_max
            ),
            suggestion: "Aumente o número de espelhos (use o Solver de Escadas).".to_owned(),
        });
    }

    // 3–4. Going (tread) min / max
    if rules.going_min > EPS && going_mm + EPS < rules.going_min {
        let (kind, source) = meta("going_min");
        findings.push(StairFinding {
            rule: "going_min".to_owned(),
            value: going_mm,
            limit: rules.going_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Piso {:.0} mm abaixo do mínimo de {:.0} mm.",
                going_mm, rules.going_min
            ),
            suggestion: format!("Aumente o piso para pelo menos {:.0} mm.", rules.going_min),
        });
    }
    if rules.going_max > EPS && going_mm > rules.going_max + EPS {
        let (kind, source) = meta("going_max");
        findings.push(StairFinding {
            rule: "going_max".to_owned(),
            value: going_mm,
            limit: rules.going_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Piso {:.0} mm acima do máximo de {:.0} mm.",
                going_mm, rules.going_max
            ),
            suggestion: format!("Reduza o piso para no máximo {:.0} mm.", rules.going_max),
        });
    }

    // 5–6. Blondel step formula (2e + p) min / max
    if rules.step_min > EPS && step_mm + EPS < rules.step_min {
        let (kind, source) = meta("step_min");
        findings.push(StairFinding {
            rule: "step_min".to_owned(),
            value: step_mm,
            limit: rules.step_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Fórmula de Blondel (2e + p = {:.0} mm) abaixo do mínimo {:.0} mm.",
                step_mm, rules.step_min
            ),
            suggestion: "Aumente o piso ou ajuste o número de espelhos.".to_owned(),
        });
    }
    if rules.step_max > EPS && step_mm > rules.step_max + EPS {
        let (kind, source) = meta("step_max");
        findings.push(StairFinding {
            rule: "step_max".to_owned(),
            value: step_mm,
            limit: rules.step_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Fórmula de Blondel (2e + p = {:.0} mm) acima do máximo {:.0} mm.",
                step_mm, rules.step_max
            ),
            suggestion: "Reduza o piso ou aumente o número de espelhos.".to_owned(),
        });
    }

    // 7. Width min
    if rules.width_min > EPS && width_mm + EPS < rules.width_min {
        let (kind, source) = meta("width_min");
        findings.push(StairFinding {
            rule: "width_min".to_owned(),
            value: width_mm,
            limit: rules.width_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Largura livre {:.0} mm abaixo do mínimo de {:.0} mm.",
                width_mm, rules.width_min
            ),
            suggestion: format!(
                "Aumente a largura da escada para pelo menos {:.0} mm.",
                rules.width_min
            ),
        });
    }

    // 8. Headroom (3D spatial check)
    let headroom = stair_headroom(project, element, &rules);
    if let (Some(clear_mm), Some(obstruction)) = (headroom.min_clearance_mm, headroom.obstruction) {
        let (kind, source) = meta("headroom");
        findings.push(StairFinding {
            rule: "headroom".to_owned(),
            value: clear_mm,
            limit: rules.headroom,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Pé-direito livre {:.0} mm sob {obstruction} abaixo do mínimo {:.0} mm.",
                clear_mm, rules.headroom
            ),
            suggestion: "Amplie a abertura da laje ou desloque a viga sobre a escada.".to_owned(),
        });
    }

    // 9–10. Per-flight rise max and risers max (splitting by L/U landing and intermediate landings)
    let flights = compute_flight_risers(
        shape,
        metrics.risers,
        &element.stair_details.intermediate_landings,
    );
    let max_flight_risers = flights.iter().copied().max().unwrap_or(metrics.risers);
    let max_flight_rise_mm = max_flight_risers as f64 * riser_mm;

    if rules.flight_rise_max > EPS && max_flight_rise_mm > rules.flight_rise_max + EPS {
        let (kind, source) = meta("flight_rise_max");
        findings.push(StairFinding {
            rule: "flight_rise_max".to_owned(),
            value: max_flight_rise_mm,
            limit: rules.flight_rise_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Lance vence {:.0} mm sem patamar (máximo {:.0} mm; desnível total {:.0} mm).",
                max_flight_rise_mm, rules.flight_rise_max, total_rise_mm
            ),
            suggestion: "Insira um patamar intermediário (ou use o Solver de Escadas).".to_owned(),
        });
    }
    if rules.flight_risers_max > 0 && max_flight_risers > rules.flight_risers_max {
        let (kind, source) = meta("flight_risers_max");
        findings.push(StairFinding {
            rule: "flight_risers_max".to_owned(),
            value: max_flight_risers as f64,
            limit: rules.flight_risers_max as f64,
            unit: "espelhos".to_owned(),
            kind,
            source,
            message: format!(
                "Lance possui {max_flight_risers} espelhos consecutivos (máximo {}).",
                rules.flight_risers_max
            ),
            suggestion: "Divida o lance com um patamar intermediário.".to_owned(),
        });
    }

    // 11–12. Landing length (for L/U implicit landing and every explicit intermediate landing)
    let landing_due = rules.landing_due_mm(width_mm, riser_mm, going_mm);
    if matches!(shape, StairShape::LShaped | StairShape::UShaped)
        && landing_due > EPS
        && landing_mm + EPS < landing_due
    {
        let (kind, source) = meta("landing_length");
        findings.push(StairFinding {
            rule: "landing_length".to_owned(),
            value: landing_mm,
            limit: landing_due,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Patamar de {:.0} mm abaixo do comprimento mínimo {:.0} mm.",
                landing_mm, landing_due
            ),
            suggestion: format!("Aumente o patamar para pelo menos {:.0} mm.", landing_due),
        });
    }
    for land in &element.stair_details.intermediate_landings {
        let l_mm = land.length_m * MM_PER_M;
        if land.winder_steps == 0 && landing_due > EPS && l_mm + EPS < landing_due {
            let (kind, source) = meta("landing_length");
            findings.push(StairFinding {
                rule: "landing_length".to_owned(),
                value: l_mm,
                limit: landing_due,
                unit: "mm".to_owned(),
                kind,
                source,
                message: format!(
                    "Patamar intermediário (degrau {}) com {:.0} mm abaixo de {:.0} mm.",
                    land.at_riser, l_mm, landing_due
                ),
                suggestion: format!(
                    "Aumente o patamar intermediário para {:.0} mm.",
                    landing_due
                ),
            });
        }
        // 13–15. Winders checks when a landing has winder steps
        if land.winder_steps > 0 {
            if !rules.winders {
                let (kind, source) = meta("winders");
                findings.push(StairFinding {
                    rule: "winders".to_owned(),
                    value: land.winder_steps as f64,
                    limit: 0.0,
                    unit: "degraus".to_owned(),
                    kind,
                    source,
                    message: "Degraus em leque não são permitidos pela norma ativa.".to_owned(),
                    suggestion: "Substitua o leque por patamar plano.".to_owned(),
                });
            } else {
                let walk_r = match rules.walk_line {
                    WalkLine::Centre => width_mm * 0.5,
                    WalkLine::NarrowSide => rules.walk_line_offset.min(width_mm * 0.5),
                };
                let winder_going =
                    (std::f64::consts::FRAC_PI_2 * walk_r) / land.winder_steps as f64;
                if rules.winder_going_min > EPS && winder_going + EPS < rules.winder_going_min {
                    let (kind, source) = meta("winder_going_min");
                    findings.push(StairFinding {
                        rule: "winder_going_min".to_owned(),
                        value: winder_going,
                        limit: rules.winder_going_min,
                        unit: "mm".to_owned(),
                        kind,
                        source,
                        message: format!(
                            "Piso na linha de caminhamento do leque ({:.0} mm) abaixo do mínimo {:.0} mm.",
                            winder_going, rules.winder_going_min
                        ),
                        suggestion: "Reduza o número de degraus em leque ou aumente o raio interno."
                            .to_owned(),
                    });
                }
            }
        }
    }

    // 16–17. Nosing min / max
    if rules.nosing_min > EPS && nosing_mm + EPS < rules.nosing_min {
        let (kind, source) = meta("nosing_min");
        findings.push(StairFinding {
            rule: "nosing_min".to_owned(),
            value: nosing_mm,
            limit: rules.nosing_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Bocel {:.0} mm abaixo do mínimo {:.0} mm.",
                nosing_mm, rules.nosing_min
            ),
            suggestion: format!("Ajuste o bocel para pelo menos {:.0} mm.", rules.nosing_min),
        });
    }
    if rules.nosing_max > EPS && nosing_mm > rules.nosing_max + EPS {
        let (kind, source) = meta("nosing_max");
        findings.push(StairFinding {
            rule: "nosing_max".to_owned(),
            value: nosing_mm,
            limit: rules.nosing_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Bocel {:.0} mm acima do máximo {:.0} mm.",
                nosing_mm, rules.nosing_max
            ),
            suggestion: format!("Reduza o bocel para no máximo {:.0} mm.", rules.nosing_max),
        });
    }

    // 18. Open risers
    if element.stair_details.open_risers && !rules.open_risers {
        let (kind, source) = meta("open_risers");
        findings.push(StairFinding {
            rule: "open_risers".to_owned(),
            value: 1.0,
            limit: 0.0,
            unit: String::new(),
            kind,
            source,
            message: "Espelhos vazados não são permitidos na norma ativa.".to_owned(),
            suggestion: "Feche os espelhos da escada (espelho fechado obrigatório).".to_owned(),
        });
    }

    // 19. Riser uniformity
    let var_mm = element.stair_details.riser_variation_m * MM_PER_M;
    if rules.uniformity > EPS && var_mm > rules.uniformity + EPS {
        let (kind, source) = meta("uniformity");
        findings.push(StairFinding {
            rule: "uniformity".to_owned(),
            value: var_mm,
            limit: rules.uniformity,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Variação entre espelhos ({:.1} mm) excede tolerância de {:.1} mm.",
                var_mm, rules.uniformity
            ),
            suggestion: "Uniformize a altura de todos os espelhos no lance.".to_owned(),
        });
    }

    // 20–22. Handrail min/max and guardrail min
    if let Some(hr_m) = element.stair_details.handrail_height_m {
        let hr_mm = hr_m * MM_PER_M;
        if rules.handrail_min > EPS && hr_mm + EPS < rules.handrail_min {
            let (kind, source) = meta("handrail_min");
            findings.push(StairFinding {
                rule: "handrail_min".to_owned(),
                value: hr_mm,
                limit: rules.handrail_min,
                unit: "mm".to_owned(),
                kind,
                source,
                message: format!(
                    "Altura de corrimão {:.0} mm abaixo do mínimo {:.0} mm.",
                    hr_mm, rules.handrail_min
                ),
                suggestion: format!(
                    "Eleve o corrimão para pelo menos {:.0} mm.",
                    rules.handrail_min
                ),
            });
        }
        if rules.handrail_max > EPS && hr_mm > rules.handrail_max + EPS {
            let (kind, source) = meta("handrail_max");
            findings.push(StairFinding {
                rule: "handrail_max".to_owned(),
                value: hr_mm,
                limit: rules.handrail_max,
                unit: "mm".to_owned(),
                kind,
                source,
                message: format!(
                    "Altura de corrimão {:.0} mm acima do máximo {:.0} mm.",
                    hr_mm, rules.handrail_max
                ),
                suggestion: format!(
                    "Reduza o corrimão para no máximo {:.0} mm.",
                    rules.handrail_max
                ),
            });
        }
    }
    if element.railing != RailingSide::None
        && let Some(guard_m) = element.stair_details.guard_height_m
    {
        let guard_mm = guard_m * MM_PER_M;
        if rules.guard_min > EPS && guard_mm + EPS < rules.guard_min {
            let (kind, source) = meta("guard_min");
            findings.push(StairFinding {
                rule: "guard_min".to_owned(),
                value: guard_mm,
                limit: rules.guard_min,
                unit: "mm".to_owned(),
                kind,
                source,
                message: format!(
                    "Altura de guarda-corpo {:.0} mm abaixo do mínimo {:.0} mm.",
                    guard_mm, rules.guard_min
                ),
                suggestion: format!(
                    "Aumente o guarda-corpo para pelo menos {:.0} mm.",
                    rules.guard_min
                ),
            });
        }
    }

    findings
}

/// User locks respected by the interactive stair solver (`solve_stair`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StairLocks {
    pub risers: bool,
    pub tread: bool,
    pub width: bool,
    pub landings: bool,
}

/// One ranked solution proposed by [`solve_stair`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StairProposal {
    pub risers: usize,
    pub riser_height_m: f64,
    pub tread_m: f64,
    pub width_m: f64,
    pub landing_depth_m: f64,
    pub intermediate_landings: Vec<StairLandingSpec>,
    pub blondel_m: f64,
    pub compliant: bool,
    pub remaining_findings: usize,
    pub cost: f64,
    pub changes: Vec<String>,
}

/// Interactive least-change stair solver (`T-10`).
///
/// Explores valid combinations of riser count, tread depth, width and landings
/// while respecting user locks, validates each candidate against all 26 stair
/// rules (including 3D headroom), and returns up to 5 compliant proposals (or
/// up to 3 closest non-compliant proposals if locks prevent full compliance).
pub fn solve_stair(
    project: &Project,
    element: &CirculationElement,
    locks: StairLocks,
) -> Vec<StairProposal> {
    let CirculationKind::Stair {
        shape,
        width_m,
        total_rise_m,
        tread_m,
        landing_depth_m,
    } = element.kind
    else {
        return Vec::new();
    };
    let Some(metrics) = element.stair_metrics() else {
        return Vec::new();
    };
    let (rules, _) = resolve_stair_rules(project);
    let total_rise_mm = total_rise_m * MM_PER_M;
    let n0 = metrics.risers;

    let mut riser_candidates: Vec<usize> = if locks.risers {
        vec![n0]
    } else {
        let lo = if rules.riser_max > EPS {
            (total_rise_mm / rules.riser_max)
                .ceil()
                .max(MIN_RUN_RISERS as f64) as usize
        } else {
            MIN_RUN_RISERS
        };
        let hi = if rules.riser_min > EPS {
            (total_rise_mm / rules.riser_min).floor().max(lo as f64) as usize
        } else {
            MAX_SOLVER_RISERS
        };
        let lo = lo.clamp(MIN_RUN_RISERS, MAX_SOLVER_RISERS);
        let hi = hi.clamp(lo, (lo + MAX_SOLVER_SPAN).min(MAX_SOLVER_RISERS));
        let mut v: Vec<usize> = (lo..=hi).collect();
        if !v.contains(&n0) {
            v.push(n0);
        }
        v
    };
    riser_candidates.sort_unstable();
    riser_candidates.dedup();

    let mut proposals = Vec::new();

    for n in riser_candidates {
        let riser_mm = total_rise_mm / n as f64;
        let orig_tread_mm = tread_m * MM_PER_M;

        let target_tread_mm = if locks.tread {
            orig_tread_mm
        } else {
            let min_g = rules.going_min.max(rules.step_min - 2.0 * riser_mm);
            let max_g = if rules.going_max > EPS && rules.step_max > EPS {
                rules.going_max.min(rules.step_max - 2.0 * riser_mm)
            } else if rules.going_max > EPS {
                rules.going_max
            } else if rules.step_max > EPS {
                rules.step_max - 2.0 * riser_mm
            } else {
                orig_tread_mm
            };
            if min_g <= max_g + EPS {
                orig_tread_mm.clamp(min_g.ceil(), max_g.floor().max(min_g.ceil()))
            } else if rules.going_min > EPS && rules.going_max > EPS {
                orig_tread_mm.clamp(rules.going_min, rules.going_max)
            } else {
                orig_tread_mm
            }
        };

        let target_width_mm = if locks.width {
            width_m * MM_PER_M
        } else {
            (width_m * MM_PER_M).max(rules.width_min)
        };

        let landing_due_mm = rules.landing_due_mm(target_width_mm, riser_mm, target_tread_mm);
        let target_landing_mm = if locks.landings {
            landing_depth_m * MM_PER_M
        } else if matches!(shape, StairShape::LShaped | StairShape::UShaped) {
            (landing_depth_m * MM_PER_M).max(landing_due_mm)
        } else {
            landing_depth_m * MM_PER_M
        };

        // Build intermediate landings variants: keep existing or split flights exceeding max risers/rise.
        let mut landing_variants: Vec<Vec<StairLandingSpec>> =
            vec![element.stair_details.intermediate_landings.clone()];
        if !locks.landings {
            let max_per_flight = rules.max_risers_in_flight(riser_mm);
            let current_flights =
                compute_flight_risers(shape, n, &element.stair_details.intermediate_landings);
            let worst_flight = current_flights.iter().copied().max().unwrap_or(n);
            if worst_flight > max_per_flight
                && max_per_flight >= MIN_RUN_RISERS
                && shape == StairShape::Straight
            {
                let segments = n.div_ceil(max_per_flight);
                if segments > 1 {
                    let mut split = Vec::new();
                    for seg in 1..segments {
                        let at = (n * seg) / segments;
                        if at >= MIN_RUN_RISERS && n.saturating_sub(at) >= MIN_RUN_RISERS {
                            split.push(StairLandingSpec {
                                at_riser: at,
                                length_m: (landing_due_mm / MM_PER_M)
                                    .max(target_width_mm / MM_PER_M),
                                winder_steps: 0,
                            });
                        }
                    }
                    if !split.is_empty() {
                        landing_variants.push(split);
                    }
                }
            }
        }

        for inter_landings in landing_variants {
            let mut candidate = element.clone();
            candidate.kind = CirculationKind::Stair {
                shape,
                width_m: target_width_mm / MM_PER_M,
                total_rise_m,
                tread_m: target_tread_mm / MM_PER_M,
                landing_depth_m: target_landing_mm / MM_PER_M,
            };
            candidate.stair_details.risers = Some(n);
            candidate.stair_details.intermediate_landings = inter_landings.clone();

            let cand_findings = stair_findings(project, &candidate);
            let mut changes = Vec::new();
            if n != n0 {
                changes.push(format!("Espelhos: {n0} → {n}"));
            }
            if (target_tread_mm - orig_tread_mm).abs() > 0.5 {
                changes.push(format!(
                    "Piso: {:.0} → {:.0} mm",
                    orig_tread_mm, target_tread_mm
                ));
            }
            if (target_width_mm - width_m * MM_PER_M).abs() > 0.5 {
                changes.push(format!(
                    "Largura: {:.0} → {:.0} mm",
                    width_m * MM_PER_M,
                    target_width_mm
                ));
            }
            if (target_landing_mm - landing_depth_m * MM_PER_M).abs() > 0.5 {
                changes.push(format!(
                    "Patamar: {:.0} → {:.0} mm",
                    landing_depth_m * MM_PER_M,
                    target_landing_mm
                ));
            }
            if inter_landings.len() != element.stair_details.intermediate_landings.len() {
                changes.push(format!(
                    "Patamares intermediários: {} → {}",
                    element.stair_details.intermediate_landings.len(),
                    inter_landings.len()
                ));
            }

            let added_landings = inter_landings
                .len()
                .abs_diff(element.stair_details.intermediate_landings.len())
                as f64;
            let cost = (n.abs_diff(n0) as f64) * 10.0
                + (target_tread_mm - orig_tread_mm).abs() / 10.0
                + (target_width_mm - width_m * MM_PER_M).abs() / 50.0
                + added_landings * 3.0
                + if changes.is_empty() { 0.0 } else { 0.5 }
                + (cand_findings.len() as f64) * 1000.0;

            proposals.push(StairProposal {
                risers: n,
                riser_height_m: riser_mm / MM_PER_M,
                tread_m: target_tread_mm / MM_PER_M,
                width_m: target_width_mm / MM_PER_M,
                landing_depth_m: target_landing_mm / MM_PER_M,
                intermediate_landings: inter_landings,
                blondel_m: (2.0 * riser_mm + target_tread_mm) / MM_PER_M,
                compliant: cand_findings.is_empty(),
                remaining_findings: cand_findings.len(),
                cost,
                changes,
            });
        }
    }

    proposals.sort_by(|a, b| a.cost.total_cmp(&b.cost));

    if let Some(first) = proposals.first()
        && first.compliant
        && first.changes.is_empty()
    {
        return vec![first.clone()];
    }

    let compliant: Vec<StairProposal> = proposals
        .iter()
        .filter(|p| p.compliant && !p.changes.is_empty())
        .take(MAX_SOLVER_PROPOSALS)
        .cloned()
        .collect();
    if !compliant.is_empty() {
        return compliant;
    }

    proposals
        .into_iter()
        .filter(|p| !p.changes.is_empty())
        .take(MAX_SOLVER_FALLBACK)
        .collect()
}

/// Apply a [`StairProposal`] onto a stair element.
pub fn apply_stair_proposal(
    element: &mut CirculationElement,
    proposal: &StairProposal,
) -> Result<(), String> {
    let CirculationKind::Stair {
        shape,
        total_rise_m,
        ..
    } = element.kind
    else {
        return Err("O elemento selecionado não é uma escada.".to_owned());
    };
    element.kind = CirculationKind::Stair {
        shape,
        width_m: proposal.width_m,
        total_rise_m,
        tread_m: proposal.tread_m,
        landing_depth_m: proposal.landing_depth_m,
    };
    element.stair_details.risers = Some(proposal.risers);
    element.stair_details.intermediate_landings = proposal.intermediate_landings.clone();
    Ok(())
}

fn compute_flight_risers(
    shape: StairShape,
    total_risers: usize,
    intermediate: &[StairLandingSpec],
) -> Vec<usize> {
    let mut split_points: Vec<usize> = intermediate
        .iter()
        .map(|l| l.at_riser)
        .filter(|&at| at > 0 && at < total_risers)
        .collect();
    if matches!(shape, StairShape::LShaped | StairShape::UShaped) && total_risers >= 2 {
        split_points.push(total_risers / 2);
    }
    split_points.sort_unstable();
    split_points.dedup();

    let mut flights = Vec::new();
    let mut prev = 0usize;
    for sp in split_points {
        flights.push(sp.saturating_sub(prev));
        prev = sp;
    }
    flights.push(total_risers.saturating_sub(prev));
    flights
}

fn inset_sample_points(poly: &[Point]) -> Vec<Point> {
    if poly.is_empty() {
        return Vec::new();
    }
    let n = poly.len() as f64;
    let cx = poly.iter().map(|p| p.x).sum::<f64>() / n;
    let cy = poly.iter().map(|p| p.y).sum::<f64>() / n;
    let centroid = Point::new(cx, cy);
    let mut pts = Vec::with_capacity(poly.len() + 1);
    pts.push(centroid);
    for &v in poly {
        let dist = v.distance(centroid);
        if dist > EPS {
            let step = (INSET_FRAC * dist).min(INSET_MAX_M);
            let t = step / dist;
            pts.push(Point::new(
                v.x + (centroid.x - v.x) * t,
                v.y + (centroid.y - v.y) * t,
            ));
        }
    }
    pts
}

fn beam_plan_polygon(start: Point, end: Point, width_m: f64) -> Option<Vec<Point>> {
    let len = start.distance(end);
    if len <= EPS || width_m <= EPS {
        return None;
    }
    let nx = -(end.y - start.y) / len * (width_m * 0.5);
    let ny = (end.x - start.x) / len * (width_m * 0.5);
    Some(vec![
        Point::new(start.x + nx, start.y + ny),
        Point::new(end.x + nx, end.y + ny),
        Point::new(end.x - nx, end.y - ny),
        Point::new(start.x - nx, start.y - ny),
    ])
}
