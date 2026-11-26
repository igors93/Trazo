//! Compile-time embedded normative JSON packs, regional presets and Brazilian
//! city climate presets.

use std::sync::OnceLock;

use super::{BuildingCode, CodePreset, site::SitePreset};

const PRESETS_JSON: &str = include_str!("../../../data/codes/presets.json");
const SITE_PRESETS_BR_JSON: &str = include_str!("../../../data/codes/site_presets_br.json");

const CODE_FILES: &[(&str, &str)] = &[
    // Architectural
    (
        "architectural/generic.json",
        include_str!("../../../data/codes/architectural/generic.json"),
    ),
    (
        "architectural/nbr-9050.json",
        include_str!("../../../data/codes/architectural/nbr-9050.json"),
    ),
    (
        "architectural/nbr-9077.json",
        include_str!("../../../data/codes/architectural/nbr-9077.json"),
    ),
    (
        "architectural/codigo-obras-br.json",
        include_str!("../../../data/codes/architectural/codigo-obras-br.json"),
    ),
    // Structural
    (
        "structural/generic.json",
        include_str!("../../../data/codes/structural/generic.json"),
    ),
    (
        "structural/nbr-6118.json",
        include_str!("../../../data/codes/structural/nbr-6118.json"),
    ),
    // Electrical (base + 9 Brazilian utilities)
    (
        "electrical/generic.json",
        include_str!("../../../data/codes/electrical/generic.json"),
    ),
    (
        "electrical/nbr-5410.json",
        include_str!("../../../data/codes/electrical/nbr-5410.json"),
    ),
    (
        "electrical/cpfl.json",
        include_str!("../../../data/codes/electrical/cpfl.json"),
    ),
    (
        "electrical/enel.json",
        include_str!("../../../data/codes/electrical/enel.json"),
    ),
    (
        "electrical/celesc.json",
        include_str!("../../../data/codes/electrical/celesc.json"),
    ),
    (
        "electrical/cemig.json",
        include_str!("../../../data/codes/electrical/cemig.json"),
    ),
    (
        "electrical/copel.json",
        include_str!("../../../data/codes/electrical/copel.json"),
    ),
    (
        "electrical/energisa.json",
        include_str!("../../../data/codes/electrical/energisa.json"),
    ),
    (
        "electrical/equatorial.json",
        include_str!("../../../data/codes/electrical/equatorial.json"),
    ),
    (
        "electrical/fecoergs.json",
        include_str!("../../../data/codes/electrical/fecoergs.json"),
    ),
    (
        "electrical/hidropan.json",
        include_str!("../../../data/codes/electrical/hidropan.json"),
    ),
    // Plumbing
    (
        "plumbing/generic.json",
        include_str!("../../../data/codes/plumbing/generic.json"),
    ),
    (
        "plumbing/nbr.json",
        include_str!("../../../data/codes/plumbing/nbr.json"),
    ),
    (
        "plumbing/cbpmesp-it22.json",
        include_str!("../../../data/codes/plumbing/cbpmesp-it22.json"),
    ),
    // Mechanical / Comfort
    (
        "mechanical/generic.json",
        include_str!("../../../data/codes/mechanical/generic.json"),
    ),
];

/// Every built-in code pack bundled with Trazo.
pub fn builtin_codes() -> &'static [BuildingCode] {
    static CACHE: OnceLock<Vec<BuildingCode>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut out = Vec::with_capacity(CODE_FILES.len());
        for (_, raw) in CODE_FILES {
            if let Ok(code) = serde_json::from_str::<BuildingCode>(raw) {
                out.push(code);
            }
        }
        out
    })
}

/// Raw embedded JSON sources for validation tests.
#[cfg(test)]
pub(crate) fn raw_code_files() -> &'static [(&'static str, &'static str)] {
    CODE_FILES
}

/// Built-in regional code presets (`data/codes/presets.json`).
pub fn builtin_presets() -> &'static [CodePreset] {
    static CACHE: OnceLock<Vec<CodePreset>> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(PRESETS_JSON).unwrap_or_default())
}

/// Built-in Brazilian city site & climate presets (`data/codes/site_presets_br.json`).
pub fn builtin_site_presets() -> &'static [SitePreset] {
    static CACHE: OnceLock<Vec<SitePreset>> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(SITE_PRESETS_BR_JSON).unwrap_or_default())
}
