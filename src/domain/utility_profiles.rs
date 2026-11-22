//! Regional Brazilian electrical utility profiles, published standard citations
//! and per-utility residential demand tables (B21).
//!
//! Each built-in [`UtilityProfile`] replaces arbitrary scaling multipliers with
//! explicit residential demand tables and service-entrance thresholds traced to
//! the utility's secondary-distribution standard. Because utilities issue
//! periodic technical bulletins (*comunicados técnicos*), every profile also
//! carries an explicit [`UtilityStandardCitation`] shown in the UI, single-line
//! diagram and MCP output so the designer can confirm the active local revision.

use serde::{Deserialize, Serialize};

/// Traceability metadata for a built-in utility profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtilityStandardCitation {
    pub standard: &'static str,
    pub edition: &'static str,
    pub tables: &'static str,
    pub nominal_system: &'static str,
    pub verification_note: &'static str,
}

/// Standard residential lighting + general-use outlet (TUG) demand table
/// (`kVA` upper bound → demand factor), common across NBR 5410 practice,
/// Enel SP LIG-BT, CPFL GED-13 Tabela 3 and COPEL NTC 901100 Tabela 5.
pub const STANDARD_GENERAL_DEMAND_TABLE: [(f64, f64); 10] = [
    (1.0, 0.86),
    (2.0, 0.75),
    (3.0, 0.66),
    (4.0, 0.59),
    (5.0, 0.52),
    (6.0, 0.45),
    (7.0, 0.40),
    (8.0, 0.35),
    (9.0, 0.31),
    (10.0, 0.27),
];
pub const STANDARD_GENERAL_DEMAND_OVER_10: f64 = 0.24;

/// CEMIG ND 5.1 residential lighting + general-use outlet demand table
/// (more conservative curve for Minas Gerais residential units).
pub const CEMIG_GENERAL_DEMAND_TABLE: [(f64, f64); 10] = [
    (1.0, 0.86),
    (2.0, 0.81),
    (3.0, 0.76),
    (4.0, 0.72),
    (5.0, 0.68),
    (6.0, 0.64),
    (7.0, 0.60),
    (8.0, 0.57),
    (9.0, 0.54),
    (10.0, 0.52),
];
pub const CEMIG_GENERAL_DEMAND_OVER_10: f64 = 0.45;

/// Standard residential specific-use appliance (TUE) demand table by circuit count.
pub const STANDARD_SPECIFIC_DEMAND_TABLE: [f64; 25] = [
    1.00, 1.00, 0.84, 0.76, 0.70, 0.65, 0.60, 0.57, 0.54, 0.52, 0.49, 0.48, 0.46, 0.45, 0.44, 0.43,
    0.42, 0.41, 0.40, 0.40, 0.39, 0.39, 0.39, 0.38, 0.38,
];

/// Enel SP LIG-BT residential heating/TUE simultaneity table by equipment count.
pub const ENEL_SP_SPECIFIC_DEMAND_TABLE: [f64; 12] = [
    1.00, 0.92, 0.84, 0.76, 0.70, 0.65, 0.60, 0.57, 0.54, 0.52, 0.49, 0.48,
];

/// COPEL NTC 901100 residential TUE simultaneity table by equipment count.
pub const COPEL_SPECIFIC_DEMAND_TABLE: [f64; 12] = [
    1.00, 0.75, 0.65, 0.60, 0.55, 0.52, 0.49, 0.47, 0.45, 0.44, 0.43, 0.42,
];

/// Regional Brazilian utility profile controlling residential demand tables
/// and service-entrance thresholds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilityProfile {
    /// Generic Brazilian dwelling baseline (ABNT NBR 5410).
    #[default]
    Generic,
    /// Enel SP — LIG-BT (12ª ed., 2014): 127/220 V, mono ≤ 12 kVA, bi ≤ 25 kVA.
    EnelSp,
    /// CPFL — GED-13: 127/220 V, mono ≤ 12 kVA, bi ≤ 25 kVA.
    Cpfl,
    /// CEMIG — ND 5.1: 127/220 V, mono (Tipo A) ≤ 8 kVA, bi (Tipo B) ≤ 16 kVA.
    Cemig,
    /// COPEL — NTC 901100: 127/220 V (PR), mono ≤ 9 kVA, bi ≤ 15 kVA.
    Copel,
}

impl UtilityProfile {
    pub const ALL: [UtilityProfile; 5] = [
        UtilityProfile::Generic,
        UtilityProfile::EnelSp,
        UtilityProfile::Cpfl,
        UtilityProfile::Cemig,
        UtilityProfile::Copel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Padrão genérico (NBR 5410)",
            Self::EnelSp => "Enel SP (LIG-BT)",
            Self::Cpfl => "CPFL (GED-13)",
            Self::Cemig => "CEMIG (ND 5.1)",
            Self::Copel => "COPEL (NTC 901100)",
        }
    }

    pub fn citation(self) -> UtilityStandardCitation {
        match self {
            Self::Generic => UtilityStandardCitation {
                standard: "ABNT NBR 5410:2004",
                edition: "Referência genérica residencial",
                tables: "Faixas usuais de iluminação/TUG (0,86–0,24) e TUE (1,00–0,38)",
                nominal_system: "127/220 V",
                verification_note: "Confirme os limites e tabelas na norma da distribuidora local.",
            },
            Self::EnelSp => UtilityStandardCitation {
                standard: "Enel SP LIG-BT",
                edition: "12ª edição (2014)",
                tables: "Tabelas de categorias BT (mono ≤ 12 kVA, bi ≤ 25 kVA) e demanda residencial",
                nominal_system: "127/220 V",
                verification_note: "Verifique comunicados técnicos vigentes (CT / ET 1569) da Enel SP antes da execução.",
            },
            Self::Cpfl => UtilityStandardCitation {
                standard: "CPFL GED-13",
                edition: "Fornecimento em Tensão Secundária",
                tables: "Tabelas 1A–1C (categorias mono ≤ 12 kVA, bi ≤ 25 kVA), Tabela 3 (luz/TUG) e Tabela 4 (TUE)",
                nominal_system: "127/220 V",
                verification_note: "Confirme a revisão vigente da GED-13 para a distribuidora do grupo CPFL.",
            },
            Self::Cemig => UtilityStandardCitation {
                standard: "CEMIG ND 5.1",
                edition: "Edificações Individuais — Rede Aérea",
                tables: "Padrões Tipo A (≤ 8 kVA), Tipo B (≤ 16 kVA), Tipo C (≤ 75 kVA) e tabelas de demanda residencial",
                nominal_system: "127/220 V",
                verification_note: "Aparelhos em 220 V exigem no mínimo padrão Tipo B (bifásico) na ND 5.1; confirme a revisão vigente.",
            },
            Self::Copel => UtilityStandardCitation {
                standard: "COPEL NTC 901100",
                edition: "Fornecimento em Tensão Secundária",
                tables: "Tabela 2 (categorias: mono ≤ 9 kVA, bi ≤ 15 kVA, tri ≤ 75 kVA) e Tabela 5 (fatores de demanda g1/TUE)",
                nominal_system: "127/220 V",
                verification_note: "Confirme a revisão vigente da NTC 901100 na COPEL antes do pedido de ligação.",
            },
        }
    }

    pub fn rules(self) -> UtilityRules {
        match self {
            Self::Generic => UtilityRules {
                single_phase_max_kva: 8.0,
                two_phase_max_kva: 15.0,
                max_bt_kva: 75.0,
                min_breaker_a: 32.0,
                min_wire_mm2: 6.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
                general_table: &STANDARD_GENERAL_DEMAND_TABLE,
                general_over_max: STANDARD_GENERAL_DEMAND_OVER_10,
                specific_table: &STANDARD_SPECIFIC_DEMAND_TABLE,
                citation: self.citation(),
            },
            Self::EnelSp => UtilityRules {
                single_phase_max_kva: 12.0,
                two_phase_max_kva: 25.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
                general_table: &STANDARD_GENERAL_DEMAND_TABLE,
                general_over_max: STANDARD_GENERAL_DEMAND_OVER_10,
                specific_table: &ENEL_SP_SPECIFIC_DEMAND_TABLE,
                citation: self.citation(),
            },
            Self::Cpfl => UtilityRules {
                single_phase_max_kva: 12.0,
                two_phase_max_kva: 25.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
                general_table: &STANDARD_GENERAL_DEMAND_TABLE,
                general_over_max: STANDARD_GENERAL_DEMAND_OVER_10,
                specific_table: &STANDARD_SPECIFIC_DEMAND_TABLE,
                citation: self.citation(),
            },
            Self::Cemig => UtilityRules {
                single_phase_max_kva: 8.0,
                two_phase_max_kva: 16.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
                general_table: &CEMIG_GENERAL_DEMAND_TABLE,
                general_over_max: CEMIG_GENERAL_DEMAND_OVER_10,
                specific_table: &STANDARD_SPECIFIC_DEMAND_TABLE,
                citation: self.citation(),
            },
            Self::Copel => UtilityRules {
                single_phase_max_kva: 9.0,
                two_phase_max_kva: 15.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
                general_table: &STANDARD_GENERAL_DEMAND_TABLE,
                general_over_max: STANDARD_GENERAL_DEMAND_OVER_10,
                specific_table: &COPEL_SPECIFIC_DEMAND_TABLE,
                citation: self.citation(),
            },
        }
    }
}

/// Service-entrance limits, explicit residential demand tables and standard
/// citation for a utility profile.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub general_table: &'static [(f64, f64)],
    pub general_over_max: f64,
    pub specific_table: &'static [f64],
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
            if let Some(v) = o.single_phase_max_kva {
                self.single_phase_max_kva = v.max(1.0);
            }
            if let Some(v) = o.two_phase_max_kva {
                self.two_phase_max_kva = v.max(self.single_phase_max_kva);
            }
            if let Some(v) = o.max_bt_kva {
                self.max_bt_kva = v.max(10.0);
            }
            if let Some(v) = o.min_breaker_a {
                self.min_breaker_a = v.max(10.0);
            }
            if let Some(v) = o.min_wire_mm2 {
                self.min_wire_mm2 = v.max(2.5);
            }
            if let Some(v) = o.general_demand_scale {
                self.general_demand_scale = v.clamp(0.1, 2.0);
            }
            if let Some(v) = o.specific_demand_scale {
                self.specific_demand_scale = v.clamp(0.1, 2.0);
            }
        }
        self
    }

    pub fn general_demand_factor(&self, kva: f64) -> f64 {
        let base = self
            .general_table
            .iter()
            .find(|(up_to, _)| kva <= *up_to + 1e-9)
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
        for profile in UtilityProfile::ALL {
            let rules = profile.rules();
            assert!(!rules.citation.standard.is_empty());
            assert!(!rules.citation.edition.is_empty());
            assert!(!rules.citation.tables.is_empty());
            assert!(!rules.citation.verification_note.is_empty());
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
    }
}
