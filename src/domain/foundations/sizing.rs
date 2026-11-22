//! Rectangular concrete ULS sizing shared by balance beams (#144), ground
//! beams (#143) and tall-wall wind columns (#147).

use crate::domain::reinforcement::bar_area_cm2;

/// Steel limit of the section: `x/d ≤ 0.45` with the rectangular stress block.
const MU_LIMIT: f64 = 0.295;

/// The smallest height (in 5 cm steps, 40 to 120 cm) that carries `md`
/// (kN·m) and `vd` (kN) with width `b`, and the top steel it needs.
pub fn size_balance_beam(b: f64, md: f64, vd: f64, fck_mpa: f64) -> (Option<f64>, f64, f64, f64) {
    size_rectangular_beam(b, md, vd, fck_mpa, 0.40, 1.20)
}

/// The smallest height from `h_min` to `h_max` (5 cm steps) of a rectangular
/// beam `b` wide that carries `md` (kN·m) and `vd` (kN), returning
/// `(height_m, mu/mu_lim, tau_wd/tau_rd2, tension_steel_cm2)`.
pub fn size_rectangular_beam(
    b: f64,
    md: f64,
    vd: f64,
    fck_mpa: f64,
    h_min: f64,
    h_max: f64,
) -> (Option<f64>, f64, f64, f64) {
    let fcd = fck_mpa.max(10.0) / 1.4 * 1000.0; // kN/m²
    let fyd = 500.0 / 1.15 * 1000.0;
    let tau_rd2 = 0.27 * (1.0 - fck_mpa / 250.0) * fcd;
    let mut last = (0.0, 0.0, 0.0);
    let first = (h_min / 0.05).round() as u32;
    let final_step = (h_max / 0.05).round() as u32;
    for step in first..=final_step {
        let h = f64::from(step) * 0.05;
        let d = h - 0.05;
        let mu = md / (0.85 * fcd * b * d * d);
        let tau = vd / (b * d);
        let xi = ((1.0 - (1.0 - 2.0 * mu.min(MU_LIMIT)).max(0.0).sqrt()) / 0.8).min(0.45);
        let z = d * (1.0 - 0.4 * xi);
        // NBR 6118:2023 Table 17.3 minimum longitudinal steel ratio ρ_min (CA-50).
        let rho_min = if fck_mpa <= 30.0 {
            0.00150
        } else if fck_mpa <= 35.0 {
            0.00164
        } else if fck_mpa <= 40.0 {
            0.00179
        } else if fck_mpa <= 45.0 {
            0.00194
        } else {
            0.00208
        };
        let steel = (md / (z * fyd) * 10_000.0).max(rho_min * b * h * 10_000.0);
        last = (mu / MU_LIMIT, tau / tau_rd2, steel);
        if mu <= MU_LIMIT && tau <= tau_rd2 {
            return (Some(h), last.0, last.1, last.2);
        }
    }
    (None, last.0, last.1, last.2)
}

/// Top longitudinal bars for `steel_cm2` (2 to 6 bars of 10 to 20 mm).
pub(crate) fn choose_bars(steel_cm2: f64) -> String {
    for diameter in [10.0, 12.5, 16.0, 20.0] {
        let count = ((steel_cm2 / bar_area_cm2(diameter)).ceil() as u32).max(2);
        if count <= 6 {
            let d = if diameter.fract() == 0.0 {
                format!("{diameter:.0}")
            } else {
                format!("{diameter:.1}").replace('.', ",")
            };
            return format!("{count} Ø{d} (superior)");
        }
    }
    "mais de 6 Ø20: aumente a seção".to_owned()
}

/// Preliminary closed stirrups (CA-50, 2 legs) for a balance beam section
/// `b × h` carrying design shear `vd` (kN), per NBR 6118 §17.4.
pub(crate) fn choose_stirrups(b: f64, h: f64, vd: f64, fck_mpa: f64) -> String {
    let fck = fck_mpa.max(20.0);
    let d = (h - 0.05).max(0.60 * h);
    let fctm = 0.3 * fck.powf(2.0 / 3.0);
    let fctd_kn_m2 = 0.7 * fctm / 1.4 * 1000.0;
    let fyd_kn_m2 = 500.0 / 1.15 * 1000.0;
    let vc = 0.60 * fctd_kn_m2 * b * d;
    let min_asw = 0.20 * fctm / 500.0 * b;
    let needed = ((vd.abs() - vc).max(0.0) / (0.9 * d * fyd_kn_m2)).max(min_asw);
    let fcd_kn_m2 = fck / 1.4 * 1000.0;
    let vrd2 = 0.27 * (1.0 - fck / 250.0) * fcd_kn_m2 * b * d;
    // NBR 6118 §18.3.3.2: tighten max stirrup spacing when V_Sd > 0.67 V_Rd2.
    let s_max = if vd.abs() > 0.67 * vrd2 {
        (0.30 * d).min(0.20)
    } else {
        (0.60 * d).min(0.20)
    };
    for dia in [6.3_f64, 8.0, 10.0] {
        let two_legs_m2 = 2.0 * bar_area_cm2(dia) / 10_000.0;
        let s_req = (two_legs_m2 / needed.max(1e-12)).min(s_max);
        for spacing_cm in [20.0_f64, 15.0, 12.5, 10.0, 7.5, 5.0] {
            if spacing_cm / 100.0 <= s_req + 1e-9 {
                let d_txt = if dia.fract() == 0.0 {
                    format!("{dia:.0}")
                } else {
                    format!("{dia:.1}").replace('.', ",")
                };
                let s_txt = if spacing_cm.fract() == 0.0 {
                    format!("{spacing_cm:.0}")
                } else {
                    format!("{spacing_cm:.1}").replace('.', ",")
                };
                return format!("Ø{d_txt} c/{s_txt} cm");
            }
        }
    }
    "Ø10 c/5 cm".to_owned()
}
