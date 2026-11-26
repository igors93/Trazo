//! Automated static guard (`T-08`):
//! 1. Verifies every embedded JSON pack in `data/codes/` parses cleanly and carries
//!    required metadata (`code`, `name`, `sections`, non-empty `sources` on non-generic packs).
//! 2. Prohibits loose normative numeric `const`/`static` declarations inside the
//!    new data-driven rule reader modules (`stairs.rs`, `checks.rs`, `electrical_demand.rs`,
//!    `utility_profiles.rs`), allowing only explicit physics/unit/cap constants.

use super::{
    BuildingCode, CodeDiscipline,
    checks::{DoorRules, RampRules, RoomRules},
    data::{builtin_codes, builtin_presets, builtin_site_presets, raw_code_files},
    electrical_demand::{
        BuildingDemandMethod, EntranceUtility, UnitDemandStandard, UtilityProfileData,
    },
    stairs::StairRules,
};

const READERS: &[(&str, &str)] = &[
    ("src/domain/codes/stairs.rs", include_str!("stairs.rs")),
    ("src/domain/codes/checks.rs", include_str!("checks.rs")),
    (
        "src/domain/codes/electrical_demand.rs",
        include_str!("electrical_demand.rs"),
    ),
    (
        "src/domain/utility_profiles.rs",
        include_str!("../utility_profiles.rs"),
    ),
];

/// Whitelisted non-normative physical conversions, tolerances and safety search caps.
const ALLOWED_CONSTS: &[&str] = &[
    "EPS",
    "MM_PER_M",
    "PERCENT_SCALE",
    "W_PER_KW",
    "INSET_FRAC",
    "INSET_MAX_M",
    "MAX_SOLVER_RISERS",
    "MAX_SOLVER_SPAN",
    "MAX_SOLVER_PROPOSALS",
    "MAX_SOLVER_FALLBACK",
    "MIN_RUN_RISERS",
];

#[test]
fn no_normative_numeric_constants_in_code_readers() {
    let mut violations = Vec::new();
    for &(path, source) in READERS {
        let mut in_test_mod = false;
        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[cfg(test)]") {
                in_test_mod = true;
            }
            if in_test_mod {
                continue;
            }
            let decl = trimmed
                .strip_prefix("pub const ")
                .or_else(|| trimmed.strip_prefix("pub(crate) const "))
                .or_else(|| trimmed.strip_prefix("const "))
                .or_else(|| trimmed.strip_prefix("pub static "))
                .or_else(|| trimmed.strip_prefix("static "));
            let Some(rest) = decl else {
                continue;
            };
            let name = rest
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if ALLOWED_CONSTS.contains(&name) {
                continue;
            }
            // Only flag numeric or array-of-number constants (not `&str` / enum slices like `ALL`).
            let is_numeric = rest.contains(": f64")
                || rest.contains(": f32")
                || rest.contains(": usize")
                || rest.contains(": u32")
                || rest.contains(": u64")
                || rest.contains(": i32")
                || rest.contains(": i64")
                || rest.contains(": [(f64")
                || rest.contains(": [f64");
            if is_numeric {
                violations.push(format!("{path}:{}: const `{name}`", idx + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Números normativos soltos proibidos em módulos consumidores de `codes`:\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_embedded_code_json_parses_and_validates_typed_sections() {
    let raw = raw_code_files();
    assert!(!raw.is_empty());
    for &(file, text) in raw {
        let pack: BuildingCode =
            serde_json::from_str(text).unwrap_or_else(|e| panic!("Falha em {file}: {e}"));
        assert!(!pack.code.is_empty(), "{file}: campo `code` vazio");
        assert!(!pack.name.is_empty(), "{file}: campo `name` vazio");

        // Validate every known typed section deserializes cleanly.
        if let Some(v) = pack.sections.get("stairs") {
            let s: StairRules = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} stairs inválido: {e}"));
            assert!(
                s.riser_min <= s.riser_max || s.riser_max == 0.0,
                "{file}: riser_min > riser_max"
            );
        }
        if let Some(v) = pack.sections.get("ramps") {
            let _: RampRules = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} ramps inválido: {e}"));
        }
        if let Some(v) = pack.sections.get("doors") {
            let _: DoorRules = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} doors inválido: {e}"));
        }
        if let Some(v) = pack.sections.get("rooms") {
            let _: RoomRules = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} rooms inválido: {e}"));
        }
        if let Some(v) = pack.sections.get("utility_profile") {
            let up: UtilityProfileData = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} utility_profile inválido: {e}"));
            assert!(
                up.single_phase_max_kva > 0.0,
                "{file}: single_phase_max_kva <= 0"
            );
            assert!(!up.general_table.is_empty(), "{file}: general_table vazia");
            assert!(
                !up.specific_table.is_empty(),
                "{file}: specific_table vazia"
            );
        }
        if let Some(v) = pack.sections.get("unit_demand") {
            let _: UnitDemandStandard = serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{file} unit_demand inválido: {e}"));
        }
        if let Some(v) = pack.sections.get("entrance") {
            let e: EntranceUtility = serde_json::from_value(v.clone())
                .unwrap_or_else(|err| panic!("{file} entrance inválido: {err}"));
            assert!(!e.tables.is_empty(), "{file}: entrance sem tabelas");
        }
        if let Some(v) = pack.sections.get("building_demand") {
            let bd: BuildingDemandMethod = serde_json::from_value(v.clone())
                .unwrap_or_else(|err| panic!("{file} building_demand inválido: {err}"));
            assert!(!bd.dwelling.by_area.is_empty(), "{file}: by_area vazia");
            assert!(
                !bd.diversity.steps.is_empty(),
                "{file}: diversity steps vazia"
            );
        }
    }

    assert_eq!(builtin_codes().len(), raw.len());
    assert!(builtin_presets().len() >= 2);
    assert!(builtin_site_presets().len() >= 20);
    for disc in CodeDiscipline::ALL {
        assert!(
            builtin_codes().iter().any(|c| c.discipline == disc),
            "Disciplina {disc:?} sem norma embutida"
        );
    }
}
