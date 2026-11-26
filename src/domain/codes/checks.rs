//! Data-driven architectural building-code checks (`T-10`):
//! - Ramps (NBR 9050 Tabela 4 `slope_by_rise`, width, run rise, landings)
//! - Doors (clear width after frame deduction, clear height, leaf max, exemptions)
//! - Rooms (minimum floor area, minimum inscribed side, minimum ceiling height,
//!   natural lighting ratio `ΣA_windows / A_floor`, and natural ventilation ratio
//!   `Σ(A_windows * vent_factor) / A_floor` over exterior room walls).
//!
//! Zero hardcoded normative numbers live in this module.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CodeDiscipline, ResolvedSection, RuleKind, stairs::stair_findings};
use crate::domain::{
    circulation::{CirculationElement, CirculationKind},
    geometry::Point,
    model::{Opening, OpeningKind, Project, Room},
    openings::WindowType,
};

const EPS: f64 = 1e-6;
const MM_PER_M: f64 = 1000.0;
const PERCENT_SCALE: f64 = 100.0;

/// Ramp rules loaded from `architectural / ramps`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RampRules {
    pub slope: f64,
    pub width: f64,
    pub slope_max: f64,
    /// Ascending tiers `[rise_max_mm, slope_max_percent]` (e.g. NBR 9050 Tabela 4).
    pub slope_by_rise: Vec<[f64; 2]>,
    pub run_rise_max: f64,
    pub width_min: f64,
    pub landing_length_min: f64,
}

/// Door rules loaded from `architectural / doors`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DoorRules {
    pub clear_width_min: f64,
    pub clear_height_min: f64,
    pub leaf_width_max: f64,
    /// Frame + leaf thickness deduction in millimetres to obtain clear opening (*vão livre*).
    pub frame_allowance: f64,
    /// Case-insensitive room/label substrings exempted from door checks.
    pub exempt: Vec<String>,
}

/// Per-room-type habitability requirements loaded from `architectural / rooms`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomRuleKind {
    pub kind: String,
    pub names: Vec<String>,
    /// Minimum floor area in m².
    pub area_min: f64,
    /// Minimum inscribed width / narrower side in mm.
    pub side_min: f64,
    /// Minimum ceiling height in mm.
    pub height_min: f64,
    /// Minimum natural lighting ratio `ΣA_windows / A_floor` (0..1, e.g. `0.125` = 1/8).
    pub light_min: f64,
    /// Minimum effective natural ventilation ratio `Σ(A_windows * factor) / A_floor` (0..1, e.g. `0.0625` = 1/16).
    pub vent_min: f64,
}

/// Room habitability rules loaded from `architectural / rooms`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomRules {
    pub kinds: Vec<RoomRuleKind>,
    /// Effective openable ventilation fraction per [`WindowType`] key (`sliding`, `casement`, `fixed`, ...).
    pub window_vent_factors: BTreeMap<String, f64>,
}

/// Unified finding emitted by real-time architectural code checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeFinding {
    pub element_kind: String,
    pub element_id: u64,
    pub element_name: String,
    pub section: String,
    pub rule: String,
    pub value: f64,
    pub limit: f64,
    pub unit: String,
    pub kind: RuleKind,
    pub source: Option<String>,
    pub message: String,
    pub suggestion: String,
}

/// Complete real-time building-code compliance report for a project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeCheckReport {
    pub active_code: String,
    pub active_amendments: Vec<String>,
    pub findings: Vec<CodeFinding>,
}

impl CodeCheckReport {
    pub fn requirement_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.kind == RuleKind::Requirement)
            .count()
    }

    pub fn advisory_count(&self) -> usize {
        self.findings.len().saturating_sub(self.requirement_count())
    }
}

pub fn resolve_ramp_rules(project: &Project) -> (RampRules, ResolvedSection) {
    let resolved = project
        .codes
        .resolve_section(CodeDiscipline::Architectural, "ramps");
    let rules = serde_json::from_value::<RampRules>(resolved.value.clone()).unwrap_or_default();
    (rules, resolved)
}

pub fn resolve_door_rules(project: &Project) -> (DoorRules, ResolvedSection) {
    let resolved = project
        .codes
        .resolve_section(CodeDiscipline::Architectural, "doors");
    let rules = serde_json::from_value::<DoorRules>(resolved.value.clone()).unwrap_or_default();
    (rules, resolved)
}

pub fn resolve_room_rules(project: &Project) -> (RoomRules, ResolvedSection) {
    let resolved = project
        .codes
        .resolve_section(CodeDiscipline::Architectural, "rooms");
    let rules = serde_json::from_value::<RoomRules>(resolved.value.clone()).unwrap_or_default();
    (rules, resolved)
}

/// Check a single ramp against the active `architectural/ramps` rules.
pub fn ramp_findings(project: &Project, element: &CirculationElement) -> Vec<CodeFinding> {
    let CirculationKind::Ramp {
        width_m,
        length_m,
        total_rise_m,
        landing_depth_m,
    } = element.kind
    else {
        return Vec::new();
    };
    let (rules, resolved) = resolve_ramp_rules(project);
    let mut out = Vec::new();

    let width_mm = width_m * MM_PER_M;
    let rise_mm = total_rise_m * MM_PER_M;
    let landing_mm = landing_depth_m * MM_PER_M;
    let slope_pct = if length_m > EPS {
        (total_rise_m / length_m) * PERCENT_SCALE
    } else {
        PERCENT_SCALE
    };

    let meta = |key: &str| -> (RuleKind, Option<String>) {
        resolved
            .rules
            .get(key)
            .map(|r| (r.kind, r.source.clone()))
            .unwrap_or((resolved.section_kind, resolved.section_source.clone()))
    };

    if rules.width_min > EPS && width_mm + EPS < rules.width_min {
        let (kind, source) = meta("width_min");
        out.push(CodeFinding {
            element_kind: "rampa".to_owned(),
            element_id: element.id,
            element_name: element.name.clone(),
            section: "ramps".to_owned(),
            rule: "width_min".to_owned(),
            value: width_mm,
            limit: rules.width_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Largura da rampa ({:.0} mm) abaixo do mínimo de {:.0} mm.",
                width_mm, rules.width_min
            ),
            suggestion: format!("Alargue a rampa para pelo menos {:.0} mm.", rules.width_min),
        });
    }

    // Tiered slope_by_rise (e.g. NBR 9050 Tabela 4) or global slope_max
    let mut tier_limit = None;
    for &[tier_rise, tier_slope] in &rules.slope_by_rise {
        if rise_mm <= tier_rise + EPS {
            tier_limit = Some((tier_rise, tier_slope));
            break;
        }
    }
    if let Some((tier_rise, max_slope)) = tier_limit {
        if slope_pct > max_slope + 0.01 {
            let (kind, source) = meta("slope_by_rise");
            let req_len = total_rise_m / (max_slope / PERCENT_SCALE);
            out.push(CodeFinding {
                element_kind: "rampa".to_owned(),
                element_id: element.id,
                element_name: element.name.clone(),
                section: "ramps".to_owned(),
                rule: "slope_by_rise".to_owned(),
                value: slope_pct,
                limit: max_slope,
                unit: "%".to_owned(),
                kind,
                source,
                message: format!(
                    "Inclinação {:.2}% excede o limite de {:.2}% para desnível até {:.0} mm.",
                    slope_pct, max_slope, tier_rise
                ),
                suggestion: format!(
                    "Aumente o comprimento da rampa para pelo menos {:.2} m ou divida em segmentos com patamar.",
                    req_len
                )
                .replace('.', ","),
            });
        }
    } else if rules.slope_max > EPS && slope_pct > rules.slope_max + 0.01 {
        let (kind, source) = meta("slope_max");
        let req_len = total_rise_m / (rules.slope_max / PERCENT_SCALE);
        out.push(CodeFinding {
            element_kind: "rampa".to_owned(),
            element_id: element.id,
            element_name: element.name.clone(),
            section: "ramps".to_owned(),
            rule: "slope_max".to_owned(),
            value: slope_pct,
            limit: rules.slope_max,
            unit: "%".to_owned(),
            kind,
            source,
            message: format!(
                "Inclinação {:.2}% excede o máximo permitido de {:.2}%.",
                slope_pct, rules.slope_max
            ),
            suggestion: format!("Aumente o comprimento para pelo menos {:.2} m.", req_len)
                .replace('.', ","),
        });
    }

    if rules.run_rise_max > EPS && rise_mm > rules.run_rise_max + EPS {
        let (kind, source) = meta("run_rise_max");
        out.push(CodeFinding {
            element_kind: "rampa".to_owned(),
            element_id: element.id,
            element_name: element.name.clone(),
            section: "ramps".to_owned(),
            rule: "run_rise_max".to_owned(),
            value: rise_mm,
            limit: rules.run_rise_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Desnível por segmento ({:.0} mm) excede o máximo de {:.0} mm.",
                rise_mm, rules.run_rise_max
            ),
            suggestion: "Divida a rampa em múltiplos segmentos com patamares intermediários."
                .to_owned(),
        });
    }

    if rules.landing_length_min > EPS && landing_mm + EPS < rules.landing_length_min {
        let (kind, source) = meta("landing_length_min");
        out.push(CodeFinding {
            element_kind: "rampa".to_owned(),
            element_id: element.id,
            element_name: element.name.clone(),
            section: "ramps".to_owned(),
            rule: "landing_length_min".to_owned(),
            value: landing_mm,
            limit: rules.landing_length_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Patamar de rampa ({:.0} mm) abaixo do mínimo de {:.0} mm.",
                landing_mm, rules.landing_length_min
            ),
            suggestion: format!(
                "Aumente o comprimento do patamar para pelo menos {:.0} mm.",
                rules.landing_length_min
            ),
        });
    }

    out
}

/// Check a single door opening against the active `architectural/doors` rules
/// (calculating clear opening width by subtracting `frame_allowance`).
pub fn door_findings(project: &Project, opening: &Opening) -> Vec<CodeFinding> {
    if !matches!(opening.kind, OpeningKind::Door | OpeningKind::SaloonDoor) {
        return Vec::new();
    }
    let (rules, resolved) = resolve_door_rules(project);
    let mut out = Vec::new();

    // Check exemptions against adjacent room names
    if !rules.exempt.is_empty() {
        let exempt_match = project
            .rooms
            .iter()
            .filter(|r| r.wall_ids.contains(&opening.wall_id))
            .any(|r| {
                let lower = r.display_name().to_ascii_lowercase();
                rules
                    .exempt
                    .iter()
                    .any(|ex| !ex.is_empty() && lower.contains(&ex.to_ascii_lowercase()))
            });
        if exempt_match {
            return out;
        }
    }

    let meta = |key: &str| -> (RuleKind, Option<String>) {
        resolved
            .rules
            .get(key)
            .map(|r| (r.kind, r.source.clone()))
            .unwrap_or((resolved.section_kind, resolved.section_source.clone()))
    };

    let nominal_w_mm = opening.width() * MM_PER_M;
    let clear_w_mm = (nominal_w_mm - rules.frame_allowance).max(0.0);
    let clear_h_mm = opening.height().unwrap_or(project.wall_height) * MM_PER_M;
    let label = format!("Porta #{}", opening.id);

    if rules.clear_width_min > EPS && clear_w_mm + EPS < rules.clear_width_min {
        let (kind, source) = meta("clear_width_min");
        let needed_nominal = rules.clear_width_min + rules.frame_allowance;
        out.push(CodeFinding {
            element_kind: "porta".to_owned(),
            element_id: opening.id,
            element_name: label.clone(),
            section: "doors".to_owned(),
            rule: "clear_width_min".to_owned(),
            value: clear_w_mm,
            limit: rules.clear_width_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Vão livre de {:.0} mm (folha {:.0} mm − batente {:.0} mm) abaixo do mínimo {:.0} mm.",
                clear_w_mm, nominal_w_mm, rules.frame_allowance, rules.clear_width_min
            ),
            suggestion: format!(
                "Aumente a largura nominal da porta para pelo menos {:.0} mm.",
                needed_nominal
            ),
        });
    }

    if rules.clear_height_min > EPS && clear_h_mm + EPS < rules.clear_height_min {
        let (kind, source) = meta("clear_height_min");
        out.push(CodeFinding {
            element_kind: "porta".to_owned(),
            element_id: opening.id,
            element_name: label.clone(),
            section: "doors".to_owned(),
            rule: "clear_height_min".to_owned(),
            value: clear_h_mm,
            limit: rules.clear_height_min,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Altura livre de {:.0} mm abaixo do mínimo de {:.0} mm.",
                clear_h_mm, rules.clear_height_min
            ),
            suggestion: format!(
                "Aumente a altura da porta para pelo menos {:.0} mm.",
                rules.clear_height_min
            ),
        });
    }

    if rules.leaf_width_max > EPS && nominal_w_mm > rules.leaf_width_max + EPS {
        let (kind, source) = meta("leaf_width_max");
        out.push(CodeFinding {
            element_kind: "porta".to_owned(),
            element_id: opening.id,
            element_name: label,
            section: "doors".to_owned(),
            rule: "leaf_width_max".to_owned(),
            value: nominal_w_mm,
            limit: rules.leaf_width_max,
            unit: "mm".to_owned(),
            kind,
            source,
            message: format!(
                "Largura de folha {:.0} mm acima do máximo de {:.0} mm.",
                nominal_w_mm, rules.leaf_width_max
            ),
            suggestion: format!(
                "Reduza a folha ou utilize porta de duas folhas (máx. {:.0} mm).",
                rules.leaf_width_max
            ),
        });
    }

    out
}

/// Check a single room against `architectural/rooms` rules: minimum area,
/// minimum inscribed side, minimum ceiling height, natural lighting ratio
/// (`ΣA_windows / A_floor`), and natural ventilation ratio (`Σ(A_windows * factor) / A_floor`).
pub fn room_findings(project: &Project, room: &Room) -> Vec<CodeFinding> {
    let (rules, resolved) = resolve_room_rules(project);
    if rules.kinds.is_empty() {
        return Vec::new();
    }

    let name_lower = room.display_name().to_ascii_lowercase();
    let Some(rule_kind) = rules.kinds.iter().find(|rk| {
        rk.names
            .iter()
            .any(|kw| !kw.is_empty() && name_lower.contains(&kw.to_ascii_lowercase()))
    }) else {
        return Vec::new();
    };

    let (kind, source) = resolved
        .rules
        .get("kinds")
        .map(|r| (r.kind, r.source.clone()))
        .unwrap_or((resolved.section_kind, resolved.section_source.clone()));

    let mut out = Vec::new();
    let label = room.display_name().to_owned();

    let area_m2 = project
        .room_usable_area(room)
        .or_else(|| project.room_area(room))
        .unwrap_or(0.0);
    let poly = project
        .room_usable_polygon(room)
        .or_else(|| project.room_polygon(room));

    // 1. Minimum floor area
    if rule_kind.area_min > EPS && area_m2 + 1e-4 < rule_kind.area_min {
        out.push(CodeFinding {
            element_kind: "comodo".to_owned(),
            element_id: room.id,
            element_name: label.clone(),
            section: "rooms".to_owned(),
            rule: "area_min".to_owned(),
            value: area_m2,
            limit: rule_kind.area_min,
            unit: "m²".to_owned(),
            kind,
            source: source.clone(),
            message: format!(
                "Área útil {:.2} m² abaixo do mínimo de {:.2} m² para '{}'.",
                area_m2, rule_kind.area_min, rule_kind.kind
            )
            .replace('.', ","),
            suggestion: format!(
                "Amplie o cômodo para atingir pelo menos {:.2} m².",
                rule_kind.area_min
            )
            .replace('.', ","),
        });
    }

    // 2. Minimum inscribed side (narrower span projected onto longest edge & perpendicular)
    if rule_kind.side_min > EPS
        && let Some(ring) = &poly
    {
        let narrow_mm = narrower_side(ring) * MM_PER_M;
        if narrow_mm + 1.0 < rule_kind.side_min {
            out.push(CodeFinding {
                element_kind: "comodo".to_owned(),
                element_id: room.id,
                element_name: label.clone(),
                section: "rooms".to_owned(),
                rule: "side_min".to_owned(),
                value: narrow_mm,
                limit: rule_kind.side_min,
                unit: "mm".to_owned(),
                kind,
                source: source.clone(),
                message: format!(
                    "Menor dimensão inscrita ({:.0} mm) abaixo do mínimo {:.0} mm para '{}'.",
                    narrow_mm, rule_kind.side_min, rule_kind.kind
                ),
                suggestion: format!(
                    "Aumente a menor largura do cômodo para pelo menos {:.2} m.",
                    rule_kind.side_min / MM_PER_M
                )
                .replace('.', ","),
            });
        }
    }

    // 3. Minimum ceiling height
    let ceiling_m = room_ceiling_height_m(project, room);
    let ceiling_mm = ceiling_m * MM_PER_M;
    if rule_kind.height_min > EPS && ceiling_mm + 1.0 < rule_kind.height_min {
        out.push(CodeFinding {
            element_kind: "comodo".to_owned(),
            element_id: room.id,
            element_name: label.clone(),
            section: "rooms".to_owned(),
            rule: "height_min".to_owned(),
            value: ceiling_mm,
            limit: rule_kind.height_min,
            unit: "mm".to_owned(),
            kind,
            source: source.clone(),
            message: format!(
                "Pé-direito {:.2} m abaixo do mínimo {:.2} m para '{}'.",
                ceiling_m,
                rule_kind.height_min / MM_PER_M,
                rule_kind.kind
            )
            .replace('.', ","),
            suggestion: format!(
                "Eleve o pé-direito do pavimento para pelo menos {:.2} m.",
                rule_kind.height_min / MM_PER_M
            )
            .replace('.', ","),
        });
    }

    // 4 & 5. Natural lighting and ventilation ratios over exterior room walls
    if (rule_kind.light_min > EPS || rule_kind.vent_min > EPS) && area_m2 > EPS {
        let (window_area_m2, vent_area_m2) = room_window_areas(project, room, &rules);
        let light_ratio = window_area_m2 / area_m2;
        let vent_ratio = vent_area_m2 / area_m2;

        if rule_kind.light_min > EPS && light_ratio + 1e-4 < rule_kind.light_min {
            let needed_m2 = rule_kind.light_min * area_m2;
            out.push(CodeFinding {
                element_kind: "comodo".to_owned(),
                element_id: room.id,
                element_name: label.clone(),
                section: "rooms".to_owned(),
                rule: "light_min".to_owned(),
                value: light_ratio * PERCENT_SCALE,
                limit: rule_kind.light_min * PERCENT_SCALE,
                unit: "%".to_owned(),
                kind,
                source: source.clone(),
                message: format!(
                    "Iluminação natural {:.1}% ({:.2} m² / {:.2} m²) abaixo do mínimo de {:.1}%.",
                    light_ratio * PERCENT_SCALE,
                    window_area_m2,
                    area_m2,
                    rule_kind.light_min * PERCENT_SCALE
                )
                .replace('.', ","),
                suggestion: format!(
                    "Adicione ou amplie janelas em parede externa até atingir {:.2} m² de vão.",
                    needed_m2
                )
                .replace('.', ","),
            });
        }

        if rule_kind.vent_min > EPS && vent_ratio + 1e-4 < rule_kind.vent_min {
            let needed_m2 = rule_kind.vent_min * area_m2;
            out.push(CodeFinding {
                element_kind: "comodo".to_owned(),
                element_id: room.id,
                element_name: label,
                section: "rooms".to_owned(),
                rule: "vent_min".to_owned(),
                value: vent_ratio * PERCENT_SCALE,
                limit: rule_kind.vent_min * PERCENT_SCALE,
                unit: "%".to_owned(),
                kind,
                source,
                message: format!(
                    "Ventilação natural efetiva {:.1}% ({:.2} m² úteis / {:.2} m²) abaixo do mínimo de {:.1}%.",
                    vent_ratio * PERCENT_SCALE,
                    vent_area_m2,
                    area_m2,
                    rule_kind.vent_min * PERCENT_SCALE
                )
                .replace('.', ","),
                suggestion: format!(
                    "Amplie a área de abertura útil das janelas (ou troque folha fixa/correr por maxim-ar/abrir) até {:.2} m².",
                    needed_m2
                )
                .replace('.', ","),
            });
        }
    }

    out
}

/// Run every real-time architectural building-code check on the project.
pub fn check_project_codes(project: &Project) -> CodeCheckReport {
    let active_code = project.codes.active_base_key(CodeDiscipline::Architectural);
    let active_amendments = project.codes.architectural.amendments.clone();
    let mut findings = Vec::new();

    for el in &project.circulations {
        match el.kind {
            CirculationKind::Stair { .. } => {
                for sf in stair_findings(project, el) {
                    findings.push(CodeFinding {
                        element_kind: "escada".to_owned(),
                        element_id: el.id,
                        element_name: el.name.clone(),
                        section: "stairs".to_owned(),
                        rule: sf.rule,
                        value: sf.value,
                        limit: sf.limit,
                        unit: sf.unit,
                        kind: sf.kind,
                        source: sf.source,
                        message: sf.message,
                        suggestion: sf.suggestion,
                    });
                }
            }
            CirculationKind::Ramp { .. } => {
                findings.extend(ramp_findings(project, el));
            }
            _ => {}
        }
    }

    for opening in &project.openings {
        findings.extend(door_findings(project, opening));
    }

    for room in &project.rooms {
        findings.extend(room_findings(project, room));
    }

    CodeCheckReport {
        active_code,
        active_amendments,
        findings,
    }
}

fn room_ceiling_height_m(project: &Project, room: &Room) -> f64 {
    let default_h = project
        .active_floor()
        .map(|f| f.ceiling_height_m)
        .unwrap_or(project.wall_height);
    let mut sum = 0.0;
    let mut count = 0usize;
    for &wid in &room.wall_ids {
        if let Some(wall) = project.walls.iter().find(|w| w.id == wid) {
            sum += wall.mean_height(default_h);
            count += 1;
        }
    }
    if count > 0 {
        sum / count as f64
    } else {
        default_h
    }
}

fn room_window_areas(project: &Project, room: &Room, rules: &RoomRules) -> (f64, f64) {
    let mut total_light = 0.0;
    let mut total_vent = 0.0;
    for opening in &project.openings {
        if opening.kind != OpeningKind::Window || !room.wall_ids.contains(&opening.wall_id) {
            continue;
        }
        // Only exterior walls (referenced by at most 1 room) count toward natural light & ventilation.
        let room_refs = project
            .rooms
            .iter()
            .filter(|r| r.wall_ids.contains(&opening.wall_id))
            .count();
        if room_refs > 1 {
            continue;
        }
        let height = opening.height().unwrap_or(project.wall_height);
        let area = opening.width() * height;
        total_light += area;
        let factor_key = window_type_key(opening.effective_window_type());
        let factor = rules
            .window_vent_factors
            .get(factor_key)
            .copied()
            .unwrap_or(1.0);
        total_vent += area * factor;
    }
    (total_light, total_vent)
}

fn window_type_key(wt: WindowType) -> &'static str {
    match wt {
        WindowType::Sliding => "sliding",
        WindowType::Basculante => "basculante",
        WindowType::MaximAr => "maxim_ar",
        WindowType::Casement => "casement",
        WindowType::TiltTurn => "tilt_turn",
        WindowType::Fixed => "fixed",
        WindowType::BayWindow => "bay_window",
    }
}

/// Narrower span of a polygon projected onto its longest edge and perpendicular.
fn narrower_side(ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut best_dx = 1.0;
    let mut best_dy = 0.0;
    let mut best_len_sq = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq > best_len_sq {
            best_len_sq = len_sq;
            best_dx = dx;
            best_dy = dy;
        }
    }
    let len = best_len_sq.sqrt();
    if len <= EPS {
        return 0.0;
    }
    let ux = best_dx / len;
    let uy = best_dy / len;
    let vx = -uy;
    let vy = ux;

    let mut min_u = f64::INFINITY;
    let mut max_u = f64::NEG_INFINITY;
    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    for &p in ring {
        let u = p.x * ux + p.y * uy;
        let v = p.x * vx + p.y * vy;
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    (max_u - min_u).min(max_v - min_v).max(0.0)
}
