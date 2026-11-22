//! Immediate elastic and differential settlement estimates (NBR 6122 / B11).
//!
//! Uses Boussinesq–Janbu elastic settlement for rigid footings and rafts:
//! `s = q · B · (1 − ν²) · I_s / E_s`
//! where the deformation modulus `E_s` is correlated to SPT `N̄` following
//! Brazilian practice (Teixeira & Godoy). For piles, sums elastic shaft
//! shortening with tip soil compression.

use super::SoilKind;
use super::piles::PileCapacity;

/// Typical total settlement limit for isolated footings on masonry buildings, mm.
pub const MAX_FOOTING_SETTLEMENT_MM: f64 = 25.0;
/// Typical total settlement limit for a stiff raft (*radier*), mm.
pub const MAX_RAFT_SETTLEMENT_MM: f64 = 40.0;
/// Typical differential settlement limit between adjacent supports, mm.
pub const MAX_DIFFERENTIAL_SETTLEMENT_MM: f64 = 15.0;
/// Typical working settlement limit for pile groups, mm.
pub const MAX_PILE_SETTLEMENT_MM: f64 = 15.0;

/// Empirical soil elastic modulus `E_s` in MPa from SPT `N̄`.
pub fn soil_modulus_mpa(soil: SoilKind, n_bar: f64) -> f64 {
    (soil.modulus_mpa_per_n() * n_bar.max(1.0)).max(2.0)
}

/// Rigid shape/influence factor `I_s` as a function of plan aspect ratio `L / B`.
fn rigid_shape_factor(aspect: f64) -> f64 {
    let r = aspect.clamp(1.0, 5.0);
    // Rigid square = 0.82, L/B = 2 ≈ 1.00, L/B = 5 ≈ 1.25 (Bowles / NBR 6122).
    if r <= 2.0 {
        0.82 + (r - 1.0) * (1.00 - 0.82)
    } else {
        1.00 + (r - 2.0) / 3.0 * (1.25 - 1.00)
    }
}

/// Immediate elastic settlement of a rectangular spread footing (`bx` × `by`)
/// under working soil pressure `pressure_kpa`, in millimetres.
pub fn estimate_footing_settlement_mm(
    pressure_kpa: f64,
    bx_m: f64,
    by_m: f64,
    soil: SoilKind,
    n_bar: f64,
) -> f64 {
    let b = bx_m.min(by_m).max(0.20);
    let l = bx_m.max(by_m).max(b);
    let aspect = l / b;
    let nu = soil.poisson_ratio();
    let es_kpa = soil_modulus_mpa(soil, n_bar) * 1000.0;
    let influence = rigid_shape_factor(aspect);
    let s_m = pressure_kpa.max(0.0) * b * (1.0 - nu * nu) * influence / es_kpa;
    s_m * 1000.0
}

/// Immediate elastic settlement of a stiff raft (*radier*) covering `area_m2`
/// under mean contact pressure `pressure_kpa`, in millimetres.
pub fn estimate_raft_settlement_mm(
    pressure_kpa: f64,
    area_m2: f64,
    soil: SoilKind,
    n_bar: f64,
) -> f64 {
    let b_eq = area_m2.max(1.0).sqrt();
    let nu = soil.poisson_ratio();
    let es_kpa = soil_modulus_mpa(soil, n_bar) * 1000.0;
    // Rigid mat rigidity reduction (0.80) × square influence (0.88).
    let s_m = pressure_kpa.max(0.0) * b_eq * (1.0 - nu * nu) * 0.88 * 0.80 / es_kpa;
    s_m * 1000.0
}

/// Estimated working settlement of one pile under `working_load_kn`:
/// elastic shaft shortening (`E_c = 25 GPa`) plus tip compression, in mm.
pub fn estimate_pile_settlement_mm(
    working_load_kn: f64,
    pile: &PileCapacity,
    tip_soil: SoilKind,
) -> f64 {
    let load = working_load_kn.max(0.0);
    let area = std::f64::consts::PI * pile.diameter_m * pile.diameter_m / 4.0;
    // Elastic shortening with average axial force factor 0.65 along the shaft.
    let ec_kpa = 25_000_000.0;
    let s_elastic_m = 0.65 * load * pile.length_m / (ec_kpa * area.max(1e-6));
    // Tip share (approx. 20% of load for friction-dominated bored/CFA piles).
    let q_tip_kpa = (0.20 * load) / area.max(1e-6);
    let nu = tip_soil.poisson_ratio();
    let es_kpa = soil_modulus_mpa(tip_soil, pile.tip_n.max(3.0)) * 1000.0;
    let s_tip_m = q_tip_kpa * pile.diameter_m * (1.0 - nu * nu) * 0.78 / es_kpa;
    (s_elastic_m + s_tip_m) * 1000.0
}

/// Preliminary differential settlement estimate between the most and least
/// loaded footings (75 % of the spread, usual Skempton–MacDonald practice), mm.
pub fn differential_settlement_mm(max_mm: f64, min_mm: f64) -> f64 {
    0.75 * (max_mm - min_mm).max(0.0)
}
