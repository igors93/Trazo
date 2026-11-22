//! Flexural and shear reinforcement sizing helpers (NBR 6118:2023 Model I).

use super::{
    BarSet, LONGITUDINAL_DIAMETERS_MM, MESH_DIAMETERS_MM, MeshBars, STANDARD_SPACINGS_CM,
    STEEL_FYK_MPA, STIRRUP_DIAMETERS_MM, TransverseBars, bar_area_cm2, bar_mass_kg_m,
};

/// Minimum flexural steel ratio ρ_min (NBR 6118:2023 Table 17.3, CA-50).
pub(super) fn rho_min(fck_mpa: f64) -> f64 {
    if fck_mpa <= 30.0 {
        0.00150
    } else if fck_mpa <= 35.0 {
        0.00164
    } else if fck_mpa <= 40.0 {
        0.00179
    } else if fck_mpa <= 45.0 {
        0.00194
    } else {
        0.00208
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FlexuralDemand {
    pub(super) required_cm2: f64,
    pub(super) domain_ok: bool,
}

/// Singly reinforced rectangular section at ULS. `moment_k_knm` is the
/// characteristic moment; the same 1.4 load factor and CA-50 design strength
/// used by the existing beam-stiffness calculation are applied here.
pub(super) fn flexural_demand(
    width_m: f64,
    height_m: f64,
    cover_m: f64,
    moment_k_knm: f64,
    fck_mpa: f64,
) -> FlexuralDemand {
    let b = width_m.max(0.10);
    let h = height_m.max(0.08);
    let d = (h - cover_m).max(0.60 * h);
    let fck = fck_mpa.max(20.0);
    let fcd_kn_m2 = (fck / 1.4) * 1000.0;
    let fyd_kn_m2 = (STEEL_FYK_MPA / 1.15) * 1000.0;
    let md = 1.4 * moment_k_knm.abs();
    let raw_mu = md / (b * d * d * 0.85 * fcd_kn_m2).max(1e-9);
    let mu = raw_mu.min(0.295);
    let xi = ((1.0 - (1.0 - 2.0 * mu).max(0.0).sqrt()) / 0.8).clamp(0.0, 0.45);
    let z = d * (1.0 - 0.4 * xi);
    let minimum = rho_min(fck) * b * h;
    let required_m2 = (md / (z * fyd_kn_m2).max(1e-9)).max(minimum);
    FlexuralDemand {
        required_cm2: required_m2 * 10_000.0,
        domain_ok: raw_mu <= 0.295 + 1e-9,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BarChoice {
    count: u32,
    pub(super) diameter_mm: f64,
    pub(super) provided_cm2: f64,
    fits: bool,
}

pub(super) fn choose_beam_bars(
    required_cm2: f64,
    width_m: f64,
    cover_m: f64,
    stirrup_mm: f64,
) -> BarChoice {
    let stirrup_m = stirrup_mm.max(STIRRUP_DIAMETERS_MM[0]) / 1000.0;
    let available = (width_m - 2.0 * (cover_m + stirrup_m)).max(0.0);
    // NBR 6118:2023 §18.3.2.2: horizontal clear spacing >= max(20 mm, Ø, 1.2 * d_max,agg),
    // with standard Brazilian coarse aggregate brita 1 (d_max = 19 mm -> 22.8 mm).
    let agg_clear_m = 1.2 * 0.019;
    let mut best: Option<(f64, BarChoice)> = None;
    let mut fallback: Option<(f64, BarChoice)> = None;
    for diameter in LONGITUDINAL_DIAMETERS_MM {
        let area = bar_area_cm2(diameter);
        let count = ((required_cm2 / area).ceil() as u32).max(2);
        if count > 12 {
            continue;
        }
        let diameter_m = diameter / 1000.0;
        let occupied = f64::from(count) * diameter_m;
        let clear = if count > 1 {
            (available - occupied) / f64::from(count - 1)
        } else {
            available - occupied
        };
        let fits = clear + 1e-9 >= 0.020_f64.max(diameter_m).max(agg_clear_m);
        let provided = f64::from(count) * area;
        let choice = BarChoice {
            count,
            diameter_mm: diameter,
            provided_cm2: provided,
            fits,
        };
        let score = provided - required_cm2 + f64::from(count) * 0.002;
        if fallback.as_ref().is_none_or(|(old, _)| score < *old) {
            fallback = Some((score, choice));
        }
        if fits && best.as_ref().is_none_or(|(old, _)| score < *old) {
            best = Some((score, choice));
        }
    }
    best.or(fallback)
        .map(|(_, value)| value)
        .unwrap_or(BarChoice {
            count: 12,
            diameter_mm: 25.0,
            provided_cm2: 12.0 * bar_area_cm2(25.0),
            fits: false,
        })
}

pub(super) fn choose_column_bars(required_cm2: f64) -> BarChoice {
    let mut best: Option<(f64, BarChoice)> = None;
    for diameter in LONGITUDINAL_DIAMETERS_MM {
        let area = bar_area_cm2(diameter);
        let raw = ((required_cm2 / area).ceil() as u32).max(4);
        let count = if raw.is_multiple_of(2) { raw } else { raw + 1 };
        if count > 16 {
            continue;
        }
        let provided = f64::from(count) * area;
        let score = provided - required_cm2 + f64::from(count) * 0.002;
        let choice = BarChoice {
            count,
            diameter_mm: diameter,
            provided_cm2: provided,
            fits: true,
        };
        if best.as_ref().is_none_or(|(old, _)| score < *old) {
            best = Some((score, choice));
        }
    }
    best.map(|(_, value)| value).unwrap_or(BarChoice {
        count: 16,
        diameter_mm: 25.0,
        provided_cm2: 16.0 * bar_area_cm2(25.0),
        fits: false,
    })
}

pub(super) fn bar_set(choice: BarChoice, required_cm2: f64, length_each_m: f64) -> BarSet {
    let total_length_m = f64::from(choice.count) * length_each_m.max(0.0);
    BarSet {
        count: choice.count,
        diameter_mm: choice.diameter_mm,
        required_cm2,
        provided_cm2: choice.provided_cm2,
        length_each_m,
        total_length_m,
        mass_kg: total_length_m * bar_mass_kg_m(choice.diameter_mm),
        fits_section: choice.fits,
    }
}

pub(super) fn beam_stirrups(
    length_m: f64,
    width_m: f64,
    height_m: f64,
    shear_k_n: f64,
    fck_mpa: f64,
) -> TransverseBars {
    let b = width_m.max(0.10);
    let h = height_m.max(0.15);
    let d = (h - 0.04).max(0.60 * h);
    let fck = fck_mpa.max(20.0);
    let fctm_mpa = 0.3 * fck.powf(2.0 / 3.0);
    let fctd_kn_m2 = 0.7 * fctm_mpa / 1.4 * 1000.0;
    let fyd_kn_m2 = STEEL_FYK_MPA / 1.15 * 1000.0;
    let vsd = 1.4 * shear_k_n.abs();
    let vc = 0.60 * fctd_kn_m2 * b * d;
    let minimum_m2_m = 0.20 * fctm_mpa / STEEL_FYK_MPA * b;
    let needed_m2_m = ((vsd - vc).max(0.0) / (0.9 * d * fyd_kn_m2)).max(minimum_m2_m);
    let fcd_kn_m2 = fck / 1.4 * 1000.0;
    let alpha_v2 = (1.0 - fck / 250.0).max(0.1);
    let vrd2 = 0.27 * alpha_v2 * fcd_kn_m2 * b * d;
    let shear_capacity_ok = vsd <= vrd2 + 1e-6;
    let maximum_spacing_m = if vsd > 0.67 * vrd2 {
        (0.30 * d).min(0.20)
    } else {
        (0.60 * d).min(0.30)
    };

    let mut selected = (10.0, 0.05);
    let mut found = false;
    'diameters: for diameter in STIRRUP_DIAMETERS_MM {
        let two_legs_m2 = 2.0 * bar_area_cm2(diameter) / 10_000.0;
        let theoretical = (two_legs_m2 / needed_m2_m.max(1e-12)).min(maximum_spacing_m);
        for spacing_cm in STANDARD_SPACINGS_CM {
            let spacing_m = spacing_cm / 100.0;
            if spacing_m <= theoretical + 1e-9 {
                selected = (diameter, spacing_m);
                found = true;
                break 'diameters;
            }
        }
    }
    let shear_capacity_ok = shear_capacity_ok && found;
    let (diameter_mm, spacing_m) = selected;
    let count = (length_m / spacing_m).ceil() as u32 + 1;
    let cover = 0.03;
    let core_b = (b - 2.0 * cover).max(0.04);
    let core_h = (h - 2.0 * cover).max(0.04);
    let length_each_m = 2.0 * (core_b + core_h) + 2.0 * 10.0 * diameter_mm / 1000.0;
    let total_length_m = f64::from(count) * length_each_m;
    TransverseBars {
        diameter_mm,
        spacing_cm: spacing_m * 100.0,
        count,
        length_each_m,
        total_length_m,
        mass_kg: total_length_m * bar_mass_kg_m(diameter_mm),
        shear_capacity_ok,
    }
}

pub(super) fn choose_mesh(
    required_cm2_m: f64,
    transverse_span_m: f64,
    bar_length_m: f64,
    max_spacing_cm: f64,
) -> MeshBars {
    let s_cap = max_spacing_cm.clamp(5.0, 20.0);
    let mut best: Option<(f64, f64, f64)> = None;
    for diameter in MESH_DIAMETERS_MM {
        let area = bar_area_cm2(diameter);
        for spacing_cm in STANDARD_SPACINGS_CM {
            if spacing_cm > s_cap + 1e-9 {
                continue;
            }
            let provided = area / (spacing_cm / 100.0);
            if provided + 1e-9 < required_cm2_m {
                continue;
            }
            let score = provided - required_cm2_m + (20.0 - spacing_cm) * 0.0001;
            if best.as_ref().is_none_or(|(old, _, _)| score < *old) {
                best = Some((score, diameter, spacing_cm));
            }
        }
    }
    let (_, diameter_mm, spacing_cm) = best.unwrap_or((0.0, 12.5, 5.0));
    let count = (transverse_span_m / (spacing_cm / 100.0)).ceil() as u32 + 1;
    let total_length_m = f64::from(count) * bar_length_m.max(0.0);
    MeshBars {
        diameter_mm,
        spacing_cm,
        required_cm2_m,
        provided_cm2_m: bar_area_cm2(diameter_mm) / (spacing_cm / 100.0),
        count,
        length_each_m: bar_length_m,
        total_length_m,
        mass_kg: total_length_m * bar_mass_kg_m(diameter_mm),
    }
}
