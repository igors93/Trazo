//! Regional Brazilian electrical utility profiles, published standard citations
//! and data-driven residential demand tables (`T-11` / B21 / `#140`).
//!
//! Every built-in [`UtilityProfile`] reads its thresholds, residential demand
//! tables and standard citations directly from the versioned JSON packs under
//! `data/codes/electrical/*.json` via [`crate::domain::codes`]. Project-owned
//! custom packs installed in [`BuildingCodes::own`] override built-in rules
//! immediately without recompiling.

use serde::{Deserialize, Serialize};

use super::codes::{
    BuildingCodes, CodeDiscipline,
    data::builtin_codes,
    electrical_demand::{
        BuildingDemandMethod, EntranceUtility, UnitDemandStandard, UtilityProfileData,
    },
};

const EPS: f64 = 1e-9;

/// Traceability metadata for a utility profile (sourced from JSON packs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UtilityStandardCitation {
    pub standard: String,
    pub edition: String,
    pub tables: String,
    pub nominal_system: String,
    pub verification_note: String,
}

/// Regional Brazilian utility profile controlling residential demand tables,
/// service-entrance categories and collective multifamily demand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilityProfile {
    /// Generic Brazilian dwelling baseline (ABNT NBR 5410).
    #[default]
    Generic,
    /// Enel (RJ & SP) — GRI-0017 / LIG-BT.
    #[serde(alias = "enel")]
    EnelSp,
    /// CPFL (Paulista, Piratininga, Santa Cruz e RGE) — GED-13 v46.0 (`D = a..i`).
    Cpfl,
    /// Celesc Distribuição (SC) — N-321.0001 / N-321.0003 (individual + coletiva).
    Celesc,
    /// CEMIG (MG) — ND-5.1 / ND-5.2 (individual + coletiva).
    Cemig,
    /// COPEL Distribuição (PR) — NTC 901100 (urbano 127/220 V + rural 127/254 V).
    Copel,
    /// Energisa Group — NDU-001 (9 distribuidoras).
    Energisa,
    /// Equatorial Group — NT.00001.EQTL.
    Equatorial,
    /// Fecoergs (RS) — REGD 035.01.06 (15 cooperativas + demanda coletiva).
    Fecoergs,
    /// Hidropan (Panambi, RS) — RIC 2014 (individual + demanda coletiva).
    Hidropan,
}

impl UtilityProfile {
    /// All 10 built-in entries: Generic NBR 5410 + 9 Brazilian utilities.
    pub const ALL: [UtilityProfile; 10] = [
        UtilityProfile::Generic,
        UtilityProfile::Cpfl,
        UtilityProfile::EnelSp,
        UtilityProfile::Celesc,
        UtilityProfile::Cemig,
        UtilityProfile::Copel,
        UtilityProfile::Energisa,
        UtilityProfile::Equatorial,
        UtilityProfile::Fecoergs,
        UtilityProfile::Hidropan,
    ];

    /// Stable key matching `data/codes/electrical/<code>.json`.
    pub fn code_key(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::EnelSp => "enel",
            Self::Cpfl => "cpfl",
            Self::Celesc => "celesc",
            Self::Cemig => "cemig",
            Self::Copel => "copel",
            Self::Energisa => "energisa",
            Self::Equatorial => "equatorial",
            Self::Fecoergs => "fecoergs",
            Self::Hidropan => "hidropan",
        }
    }

    /// Key used by legacy MCP / JSON APIs.
    pub fn api_key(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::EnelSp => "enel_sp",
            Self::Cpfl => "cpfl",
            Self::Celesc => "celesc",
            Self::Cemig => "cemig",
            Self::Copel => "copel",
            Self::Energisa => "energisa",
            Self::Equatorial => "equatorial",
            Self::Fecoergs => "fecoergs",
            Self::Hidropan => "hidropan",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "generic" | "generico" | "genérico" | "nbr5410" | "nbr-5410" => Some(Self::Generic),
            "enel" | "enel_sp" | "enel-sp" | "enel_rj" => Some(Self::EnelSp),
            "cpfl" | "rge" => Some(Self::Cpfl),
            "celesc" => Some(Self::Celesc),
            "cemig" => Some(Self::Cemig),
            "copel" => Some(Self::Copel),
            "energisa" => Some(Self::Energisa),
            "equatorial" => Some(Self::Equatorial),
            "fecoergs" => Some(Self::Fecoergs),
            "hidropan" => Some(Self::Hidropan),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Padrão genérico (NBR 5410)",
            Self::EnelSp => "Enel (GRI-0017 / LIG-BT)",
            Self::Cpfl => "CPFL (GED-13)",
            Self::Celesc => "Celesc (N-321.0001 / .0003)",
            Self::Cemig => "CEMIG (ND-5.1 / ND-5.2)",
            Self::Copel => "COPEL (NTC 901100)",
            Self::Energisa => "Energisa (NDU-001 · 9 dist.)",
            Self::Equatorial => "Equatorial (NT.00001)",
            Self::Fecoergs => "Fecoergs (REGD · 15 coop.)",
            Self::Hidropan => "Hidropan (RIC 2014)",
        }
    }

    /// Resolve `UtilityProfileData` from the embedded JSON pack (or project-owned override).
    fn load_profile_data(self, codes: Option<&BuildingCodes>) -> UtilityProfileData {
        let key = self.code_key();
        // 1. If a project-owned or active amendment provides `utility_profile` for this code, prefer it.
        if let Some(c) = codes {
            if let Some(pack) = c.find_code(CodeDiscipline::Electrical, key)
                && let Some(val) = pack.sections.get("utility_profile")
                && let Ok(parsed) = serde_json::from_value::<UtilityProfileData>(val.clone())
            {
                return parsed;
            }
            // Also check active electrical layered section if user selected a custom amendment directly.
            if self == Self::Generic
                && let Some(parsed) = c.resolve_typed::<UtilityProfileData>(
                    CodeDiscipline::Electrical,
                    "utility_profile",
                )
            {
                return parsed;
            }
        }
        // 2. Built-in JSON pack
        if let Some(pack) = builtin_codes()
            .iter()
            .find(|c| c.discipline == CodeDiscipline::Electrical && c.code == key)
            && let Some(val) = pack.sections.get("utility_profile")
            && let Ok(parsed) = serde_json::from_value::<UtilityProfileData>(val.clone())
        {
            return parsed;
        }
        UtilityProfileData::default()
    }

    #[allow(dead_code)]
    pub fn citation(self) -> UtilityStandardCitation {
        self.rules().citation
    }

    pub fn rules(self) -> UtilityRules {
        self.rules_from_codes(None)
    }

    pub fn rules_from_codes(self, codes: Option<&BuildingCodes>) -> UtilityRules {
        let data = self.load_profile_data(codes);
        UtilityRules {
            single_phase_max_kva: data.single_phase_max_kva,
            two_phase_max_kva: data.two_phase_max_kva,
            max_bt_kva: data.max_bt_kva,
            min_breaker_a: data.min_breaker_a,
            min_wire_mm2: data.min_wire_mm2,
            general_demand_scale: 1.0,
            specific_demand_scale: 1.0,
            general_table: data.general_table,
            general_over_max: data.general_over_max,
            specific_table: data.specific_table,
            citation: UtilityStandardCitation {
                standard: data.citation.standard,
                edition: data.citation.edition,
                tables: data.citation.tables,
                nominal_system: data.citation.nominal_system,
                verification_note: data.citation.verification_note,
            },
        }
    }

    /// Load the utility's full `unit_demand` method (CPFL `D=a..i`, Enel annexes, Celesc table).
    pub fn unit_demand_standard(self, codes: Option<&BuildingCodes>) -> Option<UnitDemandStandard> {
        let key = self.code_key();
        let pack = codes
            .and_then(|c| c.find_code(CodeDiscipline::Electrical, key))
            .or_else(|| {
                builtin_codes()
                    .iter()
                    .find(|c| c.discipline == CodeDiscipline::Electrical && c.code == key)
            })?;
        let val = pack.sections.get("unit_demand")?;
        serde_json::from_value(val.clone()).ok()
    }

    /// Load the utility's full data-driven `entrance` table (COPEL, Energisa, Equatorial, Fecoergs, Hidropan).
    pub fn entrance_utility(self, codes: Option<&BuildingCodes>) -> Option<EntranceUtility> {
        let key = self.code_key();
        let pack = codes
            .and_then(|c| c.find_code(CodeDiscipline::Electrical, key))
            .or_else(|| {
                builtin_codes()
                    .iter()
                    .find(|c| c.discipline == CodeDiscipline::Electrical && c.code == key)
            })?;
        let val = pack.sections.get("entrance")?;
        serde_json::from_value(val.clone()).ok()
    }

    /// Load the utility's collective residential demand method (`building_demand`: CEMIG, Celesc, Fecoergs, Hidropan).
    pub fn building_demand_method(
        self,
        codes: Option<&BuildingCodes>,
    ) -> Option<BuildingDemandMethod> {
        let key = self.code_key();
        let pack = codes
            .and_then(|c| c.find_code(CodeDiscipline::Electrical, key))
            .or_else(|| {
                builtin_codes()
                    .iter()
                    .find(|c| c.discipline == CodeDiscipline::Electrical && c.code == key)
            })?;
        let val = pack.sections.get("building_demand")?;
        serde_json::from_value(val.clone()).ok()
    }

    /// Member distributors / cooperatives declared in the utility's JSON pack (e.g. Energisa 9, Fecoergs 15).
    pub fn members(self) -> Vec<(String, String)> {
        self.entrance_utility(None)
            .map(|e| e.members.into_iter().map(|m| (m.id, m.seat)).collect())
            .unwrap_or_default()
    }

    /// Resolve the official utility service-entrance category (e.g. CPFL `B2`, Enel `B2`,
    /// COPEL `14`, Energisa/Equatorial/Fecoergs/Hidropan) from the active JSON pack.
    pub fn entrance_category_summary(
        self,
        codes: Option<&BuildingCodes>,
        installed_kw: f64,
        demand_kva: f64,
        voltage_v: f64,
        min_phases: u8,
        member: Option<&str>,
    ) -> Option<String> {
        let system = if voltage_v < 170.0 {
            "127/220"
        } else {
            "220/380"
        };
        if let Some(std) = self.unit_demand_standard(codes) {
            let res = super::codes::electrical_demand::calculate_unit_demand(
                &std,
                &super::codes::electrical_demand::UnitDemandInput {
                    system: system.to_owned(),
                    installed_kw_override: Some(installed_kw),
                    min_phases: Some(min_phases),
                    ..Default::default()
                },
            );
            if let Some(cat) = res.category {
                let brk = res
                    .breaker_a
                    .map(|b| format!(" · disjuntor {b:.0} A"))
                    .unwrap_or_default();
                return Some(format!("Categoria {cat} ({system} V{brk})"));
            }
        }
        if let Some(util) = self.entrance_utility(codes)
            && let Some(pick) = super::codes::electrical_demand::pick_entrance_category(
                &util,
                &super::codes::electrical_demand::EntrancePickInput {
                    system: system.to_owned(),
                    use_kind: "individual".to_owned(),
                    member: member.map(ToOwned::to_owned),
                    installed_kw: Some(installed_kw),
                    demand_kva: Some(demand_kva),
                    min_phases: Some(min_phases),
                },
            )
            && let Some(cat) = pick.category
        {
            let brk = pick
                .breaker_a
                .first()
                .map(|b| format!(" · disjuntor {b:.0} A"))
                .unwrap_or_default();
            return Some(format!(
                "Categoria {cat} ({} V{brk} · {})",
                pick.system, pick.table_source
            ));
        }
        None
    }
}

/// Service-entrance limits, explicit residential demand tables and standard
/// citation resolved from the utility's versioned JSON pack.
#[derive(Clone, Debug, PartialEq)]
pub struct UtilityRules {
    pub single_phase_max_kva: f64,
    pub two_phase_max_kva: f64,
    pub max_bt_kva: f64,
    pub min_breaker_a: f64,
    pub min_wire_mm2: f64,
    /// Optional user override multiplier (always `1.0` for built-in standards).
    pub general_demand_scale: f64,
    /// Optional user override multiplier (always `1.0` for built-in standards).
    pub specific_demand_scale: f64,
    pub general_table: Vec<(f64, f64)>,
    pub general_over_max: f64,
    pub specific_table: Vec<f64>,
    pub citation: UtilityStandardCitation,
}

/// Optional per-project overrides for regional utility tables not covered by a
/// built-in [`UtilityProfile`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UtilityOverrides {
    pub single_phase_max_kva: Option<f64>,
    pub two_phase_max_kva: Option<f64>,
    pub max_bt_kva: Option<f64>,
    pub min_breaker_a: Option<f64>,
    pub min_wire_mm2: Option<f64>,
    pub general_demand_scale: Option<f64>,
    pub specific_demand_scale: Option<f64>,
}

impl UtilityRules {
    pub fn with_overrides(mut self, overrides: Option<UtilityOverrides>) -> Self {
        if let Some(o) = overrides {
            if let Some(v) = o.single_phase_max_kva
                && v.is_finite()
            {
                self.single_phase_max_kva = v.max(1.0);
                self.two_phase_max_kva = self.two_phase_max_kva.max(self.single_phase_max_kva);
            }
            if let Some(v) = o.two_phase_max_kva
                && v.is_finite()
            {
                self.two_phase_max_kva = v.max(self.single_phase_max_kva);
            }
            if let Some(v) = o.max_bt_kva
                && v.is_finite()
            {
                self.max_bt_kva = v.max(10.0);
            }
            if let Some(v) = o.min_breaker_a
                && v.is_finite()
            {
                self.min_breaker_a = v.max(10.0);
            }
            if let Some(v) = o.min_wire_mm2
                && v.is_finite()
            {
                self.min_wire_mm2 = v.max(2.5);
            }
            if let Some(v) = o.general_demand_scale
                && v.is_finite()
            {
                self.general_demand_scale = v.clamp(0.1, 2.0);
            }
            if let Some(v) = o.specific_demand_scale
                && v.is_finite()
            {
                self.specific_demand_scale = v.clamp(0.1, 2.0);
            }
        }
        self
    }

    pub fn general_demand_factor(&self, kva: f64) -> f64 {
        let base = self
            .general_table
            .iter()
            .find(|(up_to, _)| kva <= *up_to + EPS)
            .map_or(self.general_over_max, |(_, factor)| *factor);
        (base * self.general_demand_scale).clamp(0.10, 1.0)
    }

    pub fn specific_demand_factor(&self, count: usize) -> f64 {
        if count == 0 {
            return 1.0;
        }
        let idx = (count - 1).min(self.specific_table.len().saturating_sub(1));
        let base = self.specific_table.get(idx).copied().unwrap_or(1.0);
        (base * self.specific_demand_scale).clamp(0.10, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_profile_cites_a_standard_and_uses_no_hidden_multipliers() {
        assert_eq!(UtilityProfile::ALL.len(), 10);
        for profile in UtilityProfile::ALL {
            let rules = profile.rules();
            assert!(!rules.citation.standard.is_empty(), "{profile:?}");
            assert!(!rules.citation.edition.is_empty(), "{profile:?}");
            assert!(!rules.citation.tables.is_empty(), "{profile:?}");
            assert!(!rules.citation.verification_note.is_empty(), "{profile:?}");
            assert!((rules.general_demand_scale - 1.0).abs() < 1e-12);
            assert!((rules.specific_demand_scale - 1.0).abs() < 1e-12);
            assert!(!rules.general_table.is_empty());
            assert!(!rules.specific_table.is_empty());
        }
    }

    #[test]
    fn utility_tables_reflect_published_regional_rules() {
        // CPFL GED-13 Tabela 3: 4.2 kW falls in (4, 5] → 0.52; mono ≤ 12, bi ≤ 25.
        let cpfl = UtilityProfile::Cpfl.rules();
        assert!((cpfl.general_demand_factor(4.2) - 0.52).abs() < 1e-9);
        assert!((cpfl.single_phase_max_kva - 12.0).abs() < 1e-9);
        assert!((cpfl.two_phase_max_kva - 25.0).abs() < 1e-9);

        // CEMIG ND 5.1: Tipo A ≤ 8 kVA, Tipo B ≤ 16 kVA; conservative general curve.
        let cemig = UtilityProfile::Cemig.rules();
        assert!((cemig.single_phase_max_kva - 8.0).abs() < 1e-9);
        assert!((cemig.two_phase_max_kva - 16.0).abs() < 1e-9);
        assert!(cemig.general_demand_factor(5.0) > cpfl.general_demand_factor(5.0));

        // COPEL NTC 901100: mono ≤ 9 kVA, bi ≤ 15 kVA; distinct TUE curve (2 eq. → 0.75).
        let copel = UtilityProfile::Copel.rules();
        assert!((copel.single_phase_max_kva - 9.0).abs() < 1e-9);
        assert!((copel.two_phase_max_kva - 15.0).abs() < 1e-9);
        assert!((copel.specific_demand_factor(2) - 0.75).abs() < 1e-9);

        // New utilities: Energisa (9 members), Fecoergs (15 cooperatives), Celesc/Hidropan collective demand.
        assert_eq!(UtilityProfile::Energisa.members().len(), 9);
        assert_eq!(UtilityProfile::Fecoergs.members().len(), 15);
        assert!(
            UtilityProfile::Celesc
                .building_demand_method(None)
                .is_some()
        );
        assert!(UtilityProfile::Cemig.building_demand_method(None).is_some());
        assert!(
            UtilityProfile::Fecoergs
                .building_demand_method(None)
                .is_some()
        );
        assert!(
            UtilityProfile::Hidropan
                .building_demand_method(None)
                .is_some()
        );
    }
}
