//! ABNT NBR 8681:2003 / NBR 6118:2023 §11.8 — Ultimate (ELU) and Service (ELS)
//! load combinations for building structures.

use crate::domain::structure::OccupancyClass;

/// Unfavorable partial factor for grouped permanent actions in buildings
/// (NBR 8681:2003 Table 1 / NBR 6118:2023 Table 11.1).
pub const GAMMA_G_UNFAV: f64 = 1.40;
/// Favorable partial factor for grouped permanent actions in normal strength ELU
/// combinations in buildings (NBR 8681:2003 Table 1 / NBR 6118:2023 Table 11.1).
pub const GAMMA_G_FAV: f64 = 1.00;
/// Favorable partial factor for permanent actions in rigid-body equilibrium
/// (`ELU-EQU`: overturning and roof uplift, NBR 8681:2003 Table 1).
pub const GAMMA_G_EQU_FAV: f64 = 0.90;
/// Partial factor for occupancy live loads in normal ELU combinations
/// (NBR 8681:2003 Table 2 / NBR 6118:2023 Table 11.1).
pub const GAMMA_Q: f64 = 1.40;
/// Partial factor for wind actions in normal ELU combinations
/// (NBR 8681:2003 Table 2 / NBR 6118:2023 Table 11.1).
pub const GAMMA_W: f64 = 1.40;

/// Wind combination factors (NBR 8681:2003 Table 6 / NBR 6118:2023 Table 11.2):
/// dynamic wind pressure on general structures.
pub const PSI0_WIND: f64 = 0.60;
pub const PSI1_WIND: f64 = 0.30;
pub const PSI2_WIND: f64 = 0.00;

/// Limit-state category of an NBR 8681 combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombinationCategory {
    /// Normal ultimate limit state (`ELU Normal`).
    EluNormal,
    /// Rigid-body equilibrium / overturning / uplift (`ELU Equilíbrio`).
    EluEquilibrium,
    /// Quasi-permanent serviceability (`ELS-QP`: long-term deflection & creep).
    ElsQuasiPermanent,
    /// Frequent serviceability (`ELS-F`: lateral drift `H/850`, crack width).
    ElsFrequent,
    /// Rare serviceability (`ELS-R`: crack formation, peak working reactions).
    ElsRare,
}

impl CombinationCategory {
    pub fn key(self) -> &'static str {
        match self {
            Self::EluNormal => "elu_normal",
            Self::EluEquilibrium => "elu_equilibrio",
            Self::ElsQuasiPermanent => "els_quase_permanente",
            Self::ElsFrequent => "els_frequente",
            Self::ElsRare => "els_rara",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::EluNormal => "ELU Normal",
            Self::EluEquilibrium => "ELU Equilíbrio",
            Self::ElsQuasiPermanent => "ELS Quase perm.",
            Self::ElsFrequent => "ELS Frequente",
            Self::ElsRare => "ELS Rara",
        }
    }

    pub fn is_ultimate(self) -> bool {
        matches!(self, Self::EluNormal | Self::EluEquilibrium)
    }
}

/// Factors defining one NBR 8681:2003 / NBR 6118:2023 §11.8 load combination.
#[derive(Clone, Debug, PartialEq)]
pub struct CombinationFactors {
    pub code: &'static str,
    pub name: &'static str,
    pub category: CombinationCategory,
    pub clause: &'static str,
    pub gamma_g1: f64,
    pub gamma_g2: f64,
    pub gamma_q: f64,
    /// Signed factor on characteristic wind `W` (`> 0` for `+dir`, `< 0` for `−dir`).
    pub gamma_w: f64,
    pub formula: String,
}

impl CombinationFactors {
    /// Linearly combines scalar values `(g1, g2, q, w_pos)` using this combination's factors.
    pub fn combine(&self, g1: f64, g2: f64, q: f64, w_pos: f64) -> f64 {
        self.gamma_g1 * g1 + self.gamma_g2 * g2 + self.gamma_q * q + self.gamma_w * w_pos
    }
}

/// Whole-building combined resultants for one NBR 8681 combination.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingCombinationResult {
    pub factors: CombinationFactors,
    /// Combined vertical gravity resultant `Fz = γg1·G1 + γg2·G2 + γq·Q`, kN.
    pub vertical_kn: f64,
    /// Combined horizontal resultant along X `Fx = γw · V_wx`, kN.
    pub horizontal_x_kn: f64,
    /// Combined horizontal resultant along Y `Fy = γw · V_wy`, kN.
    pub horizontal_y_kn: f64,
}

fn f2(value: f64) -> String {
    format!("{value:.2}").replace('.', ",")
}

fn make_formula(g1: f64, g2: f64, q: f64, w: f64) -> String {
    let mut parts = vec![format!("{}·G1", f2(g1)), format!("{}·G2", f2(g2))];
    if q.abs() > 1e-9 {
        parts.push(format!("{}·Q", f2(q)));
    }
    if w > 1e-9 {
        parts.push(format!("{}·W+", f2(w)));
    } else if w < -1e-9 {
        parts.push(format!("{}·W−", f2(-w)));
    }
    parts.join(" + ")
}

/// Generates the full standard ABNT NBR 8681:2003 / NBR 6118:2023 §11.8 suite
/// of ultimate (ELU) and serviceability (ELS) combinations for `occupancy`.
pub fn build_nbr8681_combinations(occupancy: OccupancyClass) -> Vec<CombinationFactors> {
    let psi0_q = occupancy.psi0();
    let psi1_q = occupancy.psi1();
    let psi2_q = occupancy.psi2();

    let elu_w_sec = GAMMA_W * PSI0_WIND; // 1.40 × 0.60 = 0.84
    let elu_q_sec = GAMMA_Q * psi0_q;

    let entry = |code: &'static str,
                 name: &'static str,
                 category: CombinationCategory,
                 clause: &'static str,
                 gamma_g1: f64,
                 gamma_g2: f64,
                 gamma_q: f64,
                 gamma_w: f64| CombinationFactors {
        code,
        name,
        category,
        clause,
        gamma_g1,
        gamma_g2,
        gamma_q,
        gamma_w,
        formula: make_formula(gamma_g1, gamma_g2, gamma_q, gamma_w),
    };

    vec![
        // --- Ultimate Limit State (ELU Normal) ---
        entry(
            "ELU-1",
            "Gravidade · Sobrecarga Q principal",
            CombinationCategory::EluNormal,
            "NBR 8681 §5.1.3.1 / NBR 6118 §11.8.2.2",
            GAMMA_G_UNFAV,
            GAMMA_G_UNFAV,
            GAMMA_Q,
            0.0,
        ),
        entry(
            "ELU-2",
            "Q principal + Vento W+ secundário",
            CombinationCategory::EluNormal,
            "NBR 8681 §5.1.3.1 / NBR 6118 §11.8.2.2",
            GAMMA_G_UNFAV,
            GAMMA_G_UNFAV,
            GAMMA_Q,
            elu_w_sec,
        ),
        entry(
            "ELU-3",
            "Q principal + Vento W− secundário",
            CombinationCategory::EluNormal,
            "NBR 8681 §5.1.3.1 / NBR 6118 §11.8.2.2",
            GAMMA_G_UNFAV,
            GAMMA_G_UNFAV,
            GAMMA_Q,
            -elu_w_sec,
        ),
        entry(
            "ELU-4",
            "Vento W+ principal + Q secundária (G desfav.)",
            CombinationCategory::EluNormal,
            "NBR 8681 §5.1.3.1 / NBR 6118 §11.8.2.2",
            GAMMA_G_UNFAV,
            GAMMA_G_UNFAV,
            elu_q_sec,
            GAMMA_W,
        ),
        entry(
            "ELU-5",
            "Vento W− principal + Q secundária (G desfav.)",
            CombinationCategory::EluNormal,
            "NBR 8681 §5.1.3.1 / NBR 6118 §11.8.2.2",
            GAMMA_G_UNFAV,
            GAMMA_G_UNFAV,
            elu_q_sec,
            -GAMMA_W,
        ),
        entry(
            "ELU-6",
            "Vento W+ principal · G favorável (inversão/tração)",
            CombinationCategory::EluNormal,
            "NBR 8681 Tabela 1 / NBR 6118 Tabela 11.1",
            GAMMA_G_FAV,
            GAMMA_G_FAV,
            0.0,
            GAMMA_W,
        ),
        entry(
            "ELU-7",
            "Vento W− principal · G favorável (inversão/tração)",
            CombinationCategory::EluNormal,
            "NBR 8681 Tabela 1 / NBR 6118 Tabela 11.1",
            GAMMA_G_FAV,
            GAMMA_G_FAV,
            0.0,
            -GAMMA_W,
        ),
        // --- Ultimate Limit State — Rigid-body Equilibrium (ELU-EQU) ---
        entry(
            "ELU-EQU+",
            "Equilíbrio / tombamento · Vento W+ (0,90·G)",
            CombinationCategory::EluEquilibrium,
            "NBR 8681 Tabela 1 (equilíbrio de corpo rígido)",
            GAMMA_G_EQU_FAV,
            GAMMA_G_EQU_FAV,
            0.0,
            GAMMA_W,
        ),
        entry(
            "ELU-EQU−",
            "Equilíbrio / tombamento · Vento W− (0,90·G)",
            CombinationCategory::EluEquilibrium,
            "NBR 8681 Tabela 1 (equilíbrio de corpo rígido)",
            GAMMA_G_EQU_FAV,
            GAMMA_G_EQU_FAV,
            0.0,
            -GAMMA_W,
        ),
        // --- Serviceability Limit State (ELS) ---
        entry(
            "ELS-QP",
            "Quase permanente (flecha diferida / fluência)",
            CombinationCategory::ElsQuasiPermanent,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            psi2_q,
            0.0,
        ),
        entry(
            "ELS-F1",
            "Frequente · Sobrecarga Q principal (fissuração)",
            CombinationCategory::ElsFrequent,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            psi1_q,
            0.0,
        ),
        entry(
            "ELS-F2",
            "Frequente · Vento W+ principal (desloc. H/850)",
            CombinationCategory::ElsFrequent,
            "NBR 8681 §5.1.5 / NBR 6118 Tabela 13.3",
            1.0,
            1.0,
            psi2_q,
            PSI1_WIND,
        ),
        entry(
            "ELS-F3",
            "Frequente · Vento W− principal (desloc. H/850)",
            CombinationCategory::ElsFrequent,
            "NBR 8681 §5.1.5 / NBR 6118 Tabela 13.3",
            1.0,
            1.0,
            psi2_q,
            -PSI1_WIND,
        ),
        entry(
            "ELS-R1",
            "Rara · Sobrecarga Q principal + Vento W+",
            CombinationCategory::ElsRare,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            1.0,
            PSI1_WIND,
        ),
        entry(
            "ELS-R2",
            "Rara · Sobrecarga Q principal + Vento W−",
            CombinationCategory::ElsRare,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            1.0,
            -PSI1_WIND,
        ),
        entry(
            "ELS-R3",
            "Rara · Vento W+ principal + Q secundária",
            CombinationCategory::ElsRare,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            psi1_q,
            1.0,
        ),
        entry(
            "ELS-R4",
            "Rara · Vento W− principal + Q secundária",
            CombinationCategory::ElsRare,
            "NBR 8681 §5.1.5 / NBR 6118 §11.8.3.2",
            1.0,
            1.0,
            psi1_q,
            -1.0,
        ),
    ]
}
