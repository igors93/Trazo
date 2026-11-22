//! Mechanical checks for spread footings: eccentric soil pressure, overturning
//! stability (NBR 6122) and punching shear (NBR 6118 §19.5 & §22.6).

/// Mechanical checks for a spread footing per NBR 6122 (eccentric bearing
/// pressure & overturning) and NBR 6118 19.5 (punching shear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootingCheck {
    pub max_pressure_kpa: f64,
    pub outside_middle_third: bool,
    pub overturning_fs: f64,
    pub punching_ratio: f64,
}

/// Evaluates eccentric soil pressure, overturning safety factor and punching
/// shear utilisation for a rectangular footing (`bx` × `by`, height `h`)
/// carrying a column (`cw` × `cd`) with characteristic axial load `load_kn`
/// offset by `(ex, ey)` from the footing centre.
#[allow(clippy::too_many_arguments)]
pub fn check_footing(
    load_kn: f64,
    bx: f64,
    by: f64,
    h: f64,
    cw: f64,
    cd: f64,
    ex: f64,
    ey: f64,
    fck_mpa: f64,
) -> FootingCheck {
    let bx = bx.max(0.1);
    let by = by.max(0.1);
    let ex = ex.abs();
    let ey = ey.abs();
    let n_soil = 1.05 * load_kn.max(0.0);

    // Eccentricity & overturning (NBR 6122).
    let kern_ratio = 6.0 * ex / bx + 6.0 * ey / by;
    let outside_middle_third = kern_ratio > 1.0 + 1e-6;
    let overturning_fs = if ex <= 1e-6 && ey <= 1e-6 {
        f64::INFINITY
    } else {
        1.0 / (2.0 * (ex / bx + ey / by)).max(1e-9)
    };
    let max_pressure_kpa = if !outside_middle_third {
        (n_soil / (bx * by)) * (1.0 + kern_ratio)
    } else {
        // Linear triangular edge pressure with zero tension (NBR 6122):
        // Uniaxial σ_max = 2N / (3·By·(Bx/2 - ex)) = (4/3)·N / ((Bx - 2ex)·By),
        // which joins the kern boundary continuously at 2.0·N / (Bx·By)
        // instead of dropping by 25% to Meyerhof's average N / (Bx_eff·By_eff).
        let bx_eff = (bx - 2.0 * ex).max(0.01);
        let by_eff = (by - 2.0 * ey).max(0.01);
        let linear_peak = (4.0 / 3.0) * n_soil / (bx_eff * by_eff);
        linear_peak.max(2.0 * n_soil / (bx * by))
    };

    // Punching shear (NBR 6118 §19.5.2.1 & §22.6.2):
    // 1. Diagonal compression at the column face perimeter u0 (τ_Sd0 ≤ τ_Rd2).
    // 2. Concrete shear along the rounded 2d critical perimeter C' for flexible
    //    footings extending beyond 2d from the column face (rigid footings whose
    //    base lies inside 2d have zero shear outside C', governed by τ_Rd2).
    let fck = fck_mpa.max(10.0);
    let d = (h - 0.05).max(0.10);
    let f_sd = 1.4 * load_kn.max(0.0);
    let fcd_kn_m2 = (fck / 1.4) * 1000.0;
    let alpha_v2 = (1.0 - fck / 250.0).max(0.1);
    let tau_rd2 = 0.27 * alpha_v2 * fcd_kn_m2;
    let cw_eff = cw.max(0.10);
    let cd_eff = cd.max(0.10);
    let u0 = (2.0 * (cw_eff + cd_eff)).max(0.40);
    let tau_sd0 = f_sd / (u0 * d);

    let ax = ((bx - cw_eff) * 0.5).max(0.0);
    let ay = ((by - cd_eff) * 0.5).max(0.0);
    let tau_sd1_ratio = if 2.0 * d >= ax.max(ay) {
        0.0
    } else {
        let rx = (2.0 * d).min(ax);
        let ry = (2.0 * d).min(ay);
        let area_inside = (cw_eff * cd_eff
            + 2.0 * cw_eff * ry
            + 2.0 * cd_eff * rx
            + std::f64::consts::PI * rx * ry)
            .min(bx * by);
        let v_sd = (f_sd * (1.0 - area_inside / (bx * by))).max(0.0);
        let u1 = 2.0 * (cw_eff + cd_eff) + std::f64::consts::PI * (rx + ry);
        let tau_sd1 = v_sd / (u1 * d);
        let k_size = (1.0 + (0.20 / d).sqrt()).min(2.0);
        let rho_flex = 0.002;
        let tau_rd1 = 0.13 * k_size * (100.0 * rho_flex * fck).cbrt() * 1000.0;
        tau_sd1 / tau_rd1
    };
    let punching_ratio = (tau_sd0 / tau_rd2).max(tau_sd1_ratio);

    FootingCheck {
        max_pressure_kpa,
        outside_middle_third,
        overturning_fs,
        punching_ratio,
    }
}
