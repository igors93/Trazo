//! Translates architectural building-code findings (`T-10`) into Safety traffic
//! light verdicts (`Discipline::Architecture`).

use std::collections::BTreeMap;

use crate::domain::{
    codes::{RuleKind, check_project_codes},
    model::Project,
};

use super::{Check, Discipline, Light, Verdict};

/// Convert all active architectural building-code findings into traffic-light verdicts.
pub fn code_verdicts(project: &Project) -> Vec<Verdict> {
    let report = check_project_codes(project);
    if report.findings.is_empty() {
        return Vec::new();
    }

    let mut grouped: BTreeMap<(String, u64, String), Vec<Check>> = BTreeMap::new();
    for f in report.findings {
        let light = match f.kind {
            RuleKind::Requirement => Light::Red,
            _ => Light::Yellow,
        };
        let utilisation = if f.limit > 1e-9 && f.value.is_finite() {
            // Ratio >= 1.0 when failing so worst-first sorting stays consistent.
            if f.value > f.limit {
                f.value / f.limit
            } else if f.value > 1e-9 {
                f.limit / f.value
            } else {
                f64::INFINITY
            }
        } else {
            f64::INFINITY
        };
        let check_name = rule_label(&f.section, &f.rule).to_owned();
        let simple = format!("{} {}", f.message, f.suggestion);
        let source_str = f
            .source
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Norma ativa do projeto");
        let detail = format!(
            "Regra {}.{}: {} ({}). Valor: {:.2} {} · Limite: {:.2} {}.",
            f.section,
            f.rule,
            source_str,
            f.kind.label(),
            f.value,
            f.unit,
            f.limit,
            f.unit
        )
        .replace('.', ",");

        grouped
            .entry((f.element_kind, f.element_id, f.element_name))
            .or_default()
            .push(Check {
                name: check_name,
                utilisation,
                light,
                simple,
                detail,
            });
    }

    grouped
        .into_iter()
        .map(|((kind, _id, name), checks)| {
            let prefix = match kind.as_str() {
                "escada" => "Escada",
                "rampa" => "Rampa",
                "porta" => "Porta",
                "comodo" => "Cômodo",
                _ => "Elemento",
            };
            let element = if name
                .to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
            {
                name
            } else {
                format!("{prefix} {name}")
            };
            Verdict {
                discipline: Discipline::Architecture,
                element,
                checks,
            }
        })
        .collect()
}

fn rule_label(section: &str, rule: &str) -> &'static str {
    match (section, rule) {
        ("stairs", "riser_min") | ("stairs", "riser_max") => "Espelho da escada",
        ("stairs", "going_min") | ("stairs", "going_max") => "Piso do degrau",
        ("stairs", "step_min") | ("stairs", "step_max") => "Fórmula de Blondel (2e + p)",
        ("stairs", "width_min") => "Largura da escada",
        ("stairs", "headroom") => "Pé-direito livre (headroom)",
        ("stairs", "flight_rise_max") => "Desnível máximo por lance",
        ("stairs", "flight_risers_max") => "Degraus consecutivos por lance",
        ("stairs", "landing_length") => "Comprimento do patamar",
        ("stairs", "winders")
        | ("stairs", "winder_going_min")
        | ("stairs", "winder_narrow_min") => "Degraus em leque",
        ("stairs", "nosing_min") | ("stairs", "nosing_max") => "Bocel do degrau",
        ("stairs", "open_risers") => "Espelho vazado",
        ("stairs", "uniformity") => "Uniformidade dos espelhos",
        ("stairs", "handrail_min") | ("stairs", "handrail_max") => "Altura do corrimão",
        ("stairs", "guard_min") => "Altura do guarda-corpo",
        ("ramps", "width_min") => "Largura da rampa",
        ("ramps", "slope_by_rise") | ("ramps", "slope_max") => "Inclinação da rampa",
        ("ramps", "run_rise_max") => "Desnível por segmento de rampa",
        ("ramps", "landing_length_min") => "Patamar da rampa",
        ("doors", "clear_width_min") => "Vão livre da porta",
        ("doors", "clear_height_min") => "Altura livre da porta",
        ("doors", "leaf_width_max") => "Largura máxima da folha",
        ("rooms", "area_min") => "Área mínima do cômodo",
        ("rooms", "side_min") => "Menor dimensão inscrita",
        ("rooms", "height_min") => "Pé-direito mínimo",
        ("rooms", "light_min") => "Iluminação natural",
        ("rooms", "vent_min") => "Ventilação natural",
        _ => "Norma construtiva",
    }
}
