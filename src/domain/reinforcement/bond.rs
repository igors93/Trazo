//! NBR 6118:2023 §9.3–9.5 bond strength, anchorage lengths, lap splices and bar geometry.

use std::f64::consts::PI;

use super::{STEEL_DENSITY_KG_M3, STEEL_FYK_MPA};

/// Bond condition zone per ABNT NBR 6118:2023 §9.3.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BondZone {
    /// Bottom bars, slabs, footings and vertical column bars (`η2 = 1.0`).
    Good,
    /// Top bars of beams with `h ≥ 30 cm` (`η2 = 0.7`).
    Poor,
}

/// Design bond strength `f_bd` (MPa) for ribbed CA-50 bars (`Ø ≤ 32 mm`,
/// `η1 = 2.25`, `η3 = 1.0`, NBR 6118:2023 §9.3.2.1).
pub fn bond_strength_mpa(fck_mpa: f64, zone: BondZone) -> f64 {
    let fck = fck_mpa.max(20.0);
    let fctm = 0.3 * fck.powf(2.0 / 3.0);
    let fctd = 0.7 * fctm / 1.4;
    let eta1 = 2.25;
    let eta2 = match zone {
        BondZone::Good => 1.0,
        BondZone::Poor => 0.7,
    };
    eta1 * eta2 * fctd
}

/// Basic straight anchorage length `l_b = (Ø / 4) · (f_yd / f_bd) ≥ 25 Ø`
/// in metres (NBR 6118:2023 §9.4.2.4).
pub fn basic_anchorage_m(diameter_mm: f64, fck_mpa: f64, zone: BondZone) -> f64 {
    let phi_m = (diameter_mm / 1000.0).max(0.005);
    let fyd = STEEL_FYK_MPA / 1.15;
    let fbd = bond_strength_mpa(fck_mpa, zone).max(1e-6);
    ((phi_m / 4.0) * (fyd / fbd)).max(25.0 * phi_m)
}

/// Required anchorage length `l_b,nec = α1 · l_b · (As,calc / As,ef) ≥ l_b,min`
/// Preliminary required anchorage length `l_b,nec = α1 · l_b · (As,calc / As,ef) ≥ l_b,min`
/// in metres (NBR 6118:2023 §9.4.2.5), where `α1 = 0.7` for hooked bars when
/// perpendicular concrete cover is at least `3·Ø` and `1.0` otherwise.
pub fn required_anchorage_with_cover_m(
    diameter_mm: f64,
    fck_mpa: f64,
    zone: BondZone,
    hooked: bool,
    cover_m: f64,
    as_calc_cm2: f64,
    as_ef_cm2: f64,
) -> f64 {
    let phi_m = (diameter_mm / 1000.0).max(0.005);
    let lb = basic_anchorage_m(diameter_mm, fck_mpa, zone);
    // NBR 6118:2023 §9.4.2.5: α1 = 0.7 only when cover perpendicular to hook plane >= 3Ø.
    let alpha1 = if hooked && cover_m + 1e-9 >= 3.0 * phi_m {
        0.7
    } else {
        1.0
    };
    let ratio = if as_ef_cm2 > 1e-9 {
        (as_calc_cm2 / as_ef_cm2).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let lb_min = (0.3 * lb).max(10.0 * phi_m).max(0.10);
    (alpha1 * lb * ratio).max(lb_min)
}

/// Preliminary required anchorage length assuming sufficient hook cover (`≥ 3Ø`).
pub fn required_anchorage_m(
    diameter_mm: f64,
    fck_mpa: f64,
    zone: BondZone,
    hooked: bool,
    as_calc_cm2: f64,
    as_ef_cm2: f64,
) -> f64 {
    let generous_cover = (3.0 * diameter_mm / 1000.0).max(0.03);
    required_anchorage_with_cover_m(
        diameter_mm,
        fck_mpa,
        zone,
        hooked,
        generous_cover,
        as_calc_cm2,
        as_ef_cm2,
    )
}

/// Preliminary lap splice length `l_0 = α0t · l_b,nec ≥ l_0,min` in metres
/// (NBR 6118:2023 §9.5.2.2). Uses straight anchorage in the given bond zone.
pub fn lap_splice_m(
    diameter_mm: f64,
    fck_mpa: f64,
    zone: BondZone,
    alpha_0t: f64,
    as_calc_cm2: f64,
    as_ef_cm2: f64,
) -> f64 {
    let phi_m = (diameter_mm / 1000.0).max(0.005);
    let lb = basic_anchorage_m(diameter_mm, fck_mpa, zone);
    let lb_nec = required_anchorage_m(diameter_mm, fck_mpa, zone, false, as_calc_cm2, as_ef_cm2);
    let alpha = alpha_0t.max(1.0);
    let l0_min = (0.3 * alpha * lb).max(15.0 * phi_m).max(0.20);
    (alpha * lb_nec).max(l0_min)
}

pub fn bar_area_cm2(diameter_mm: f64) -> f64 {
    PI * diameter_mm * diameter_mm / 400.0
}

pub fn bar_mass_kg_m(diameter_mm: f64) -> f64 {
    let area_m2 = PI * (diameter_mm / 1000.0).powi(2) / 4.0;
    area_m2 * STEEL_DENSITY_KG_M3
}
