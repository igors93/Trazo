//! Continuous-beam slope-deflection solver, Branson cracked-section equivalent
//! stiffness (NBR 6118 §17.3.2.1.1) and numerical curvature deflection.

/// Solves `a · x = b` by Gaussian elimination with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let (above, below) = a.split_at_mut(row);
            for (target, source) in below[0][col..].iter_mut().zip(&above[col][col..]) {
                *target -= factor * source;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

/// End moments (sagging positive, kN·m) of a continuous beam on pinned
/// supports, by the slope-deflection (stiffness) method: `spans` gives each
/// span's length, uniform load and EI.
#[cfg(test)]
pub fn continuous_beam(spans: &[(f64, f64, f64)]) -> Vec<(f64, f64)> {
    let empty: Vec<Vec<(f64, f64)>> = vec![Vec::new(); spans.len()];
    continuous_beam_with_point_loads(spans, &empty)
}

/// Same as [`continuous_beam`], with optional concentrated point loads
/// `(distance_from_left_m, load_kn)` per span (e.g. reactions of secondary
/// beams supported on this beam).
pub fn continuous_beam_with_point_loads(
    spans: &[(f64, f64, f64)],
    point_loads: &[Vec<(f64, f64)>],
) -> Vec<(f64, f64)> {
    continuous_beam_with_end_moments(spans, point_loads, (0.0, 0.0))
}

/// End moments with prescribed bending moments at the first and last
/// supports (sagging positive). A loaded overhang supplies a hogging end
/// moment to its adjacent span; its vertical load is added to the support
/// reaction by the caller.
pub fn continuous_beam_with_end_moments(
    spans: &[(f64, f64, f64)],
    point_loads: &[Vec<(f64, f64)>],
    end_moments: (f64, f64),
) -> Vec<(f64, f64)> {
    if spans.is_empty() {
        return Vec::new();
    }
    let n = spans.len() + 1;
    let mut k = vec![vec![0.0; n]; n];
    let mut f = vec![0.0; n];
    let fems: Vec<(f64, f64)> = spans
        .iter()
        .enumerate()
        .map(|(i, (length, load, _))| {
            let l = length.max(1e-6);
            // Fixed-end moments (clockwise positive): -wL²/12 at left, +wL²/12 at right.
            let mut fem_ab = -load * l * l / 12.0;
            let mut fem_ba = load * l * l / 12.0;
            if let Some(loads) = point_loads.get(i) {
                for &(a, p) in loads {
                    let a = a.clamp(0.0, l);
                    let b = l - a;
                    fem_ab -= p * a * b * b / (l * l);
                    fem_ba += p * a * a * b / (l * l);
                }
            }
            (fem_ab, fem_ba)
        })
        .collect();
    for (i, (length, _, ei)) in spans.iter().enumerate() {
        let s = 2.0 * ei / length.max(1e-6);
        k[i][i] += 2.0 * s;
        k[i][i + 1] += s;
        k[i + 1][i] += s;
        k[i + 1][i + 1] += 2.0 * s;
        f[i] -= fems[i].0;
        f[i + 1] -= fems[i].1;
    }
    // Joint equilibrium with the overhangs: the right end's clockwise
    // member moment is the negative of the reported sagging moment.
    f[0] += end_moments.0;
    f[n - 1] -= end_moments.1;
    let theta = solve(k, f).unwrap_or_else(|| vec![0.0; n]);
    spans
        .iter()
        .enumerate()
        .map(|(i, (length, _, ei))| {
            let s = 2.0 * ei / length.max(1e-6);
            let m_ab = s * (2.0 * theta[i] + theta[i + 1]) + fems[i].0;
            let m_ba = s * (theta[i] + 2.0 * theta[i + 1]) + fems[i].1;
            (m_ab, -m_ba)
        })
        .collect()
}

/// Cracked-section stiffness summary for a rectangular RC beam span per
/// ABNT NBR 6118:2023 (17.3.2.1.1 — Branson's equivalent inertia).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamStiffness {
    /// Gross concrete inertia I_c, m⁴.
    pub ic_m4: f64,
    /// Stage II cracked transformed inertia I_II, m⁴.
    pub i_cracked_m4: f64,
    /// Branson equivalent inertia I_eq, m⁴.
    pub i_eq_m4: f64,
    /// Cracking moment M_r, kN·m.
    pub m_r_knm: f64,
    /// Pre-sized longitudinal tension steel area A_s (CA-50), cm².
    pub as_cm2: f64,
}

/// Computes Branson's equivalent moment of inertia (NBR 6118 17.3.2.1.1) for a
/// rectangular beam (`b` × `h`, metres) of concrete `fck_mpa` under design
/// characteristic moment `m_k_design` (for tension steel pre-sizing) and
/// quasi-permanent service moment `m_a_qp` (kN·m).
pub fn effective_beam_stiffness(
    b: f64,
    h: f64,
    fck_mpa: f64,
    modulus_kn_m2: f64,
    m_k_design: f64,
    m_a_qp: f64,
) -> BeamStiffness {
    let fck = fck_mpa.max(10.0);
    let ic = (b * h.powi(3) / 12.0).max(1e-9);
    // NBR 6118 8.2.5 & 17.3.1: fct,m = 0.3·fck^(2/3) (MPa), α = 1.5 for rectangular beams.
    let fct_m_kn_m2 = 0.3 * fck.powf(2.0 / 3.0) * 1000.0;
    let m_r = 1.5 * fct_m_kn_m2 * ic / (h * 0.5).max(1e-6);

    // Pre-size tension reinforcement (CA-50, fyd = 500/1.15 MPa) at ULS (γf = 1.4),
    // never below NBR 6118 Table 17.3 minimum ratio ρ_min.
    let d = (h - 0.04).max(0.7 * h).max(0.05);
    let fcd_kn_m2 = (fck / 1.4) * 1000.0;
    let fyd_kn_m2 = (500.0 / 1.15) * 1000.0;
    let m_d = 1.4 * m_k_design.abs();
    let mu = (m_d / (b * d * d * 0.85 * fcd_kn_m2).max(1e-9)).min(0.295);
    let xi = ((1.0 - (1.0 - 2.0 * mu).max(0.0).sqrt()) / 0.8).clamp(0.0, 0.45);
    let z = d * (1.0 - 0.4 * xi);
    // NBR 6118:2023 Table 17.3 minimum flexural steel ratio ρ_min (CA-50).
    let rho_min = if fck <= 30.0 {
        0.00150
    } else if fck <= 35.0 {
        0.00164
    } else if fck <= 40.0 {
        0.00179
    } else if fck <= 45.0 {
        0.00194
    } else {
        0.00208
    };
    let as_m2 = (m_d / (z * fyd_kn_m2).max(1e-9)).max(rho_min * b * h);

    // Stage II cracked transformed section inertia I_II.
    const ES_KN_M2: f64 = 210_000_000.0;
    let alpha_e = ES_KN_M2 / modulus_kn_m2.max(1.0);
    let as_tr = alpha_e * as_m2;
    let x_ii = (-as_tr + (as_tr * as_tr + 2.0 * b * as_tr * d).sqrt()) / b.max(1e-6);
    let i_ii = (b * x_ii.powi(3) / 3.0 + as_tr * (d - x_ii).powi(2)).min(ic);

    // Branson interpolation (NBR 6118 17.3.2.1.1).
    let m_a = m_a_qp.abs();
    let i_eq = if m_a <= m_r || m_a <= 1e-9 {
        ic
    } else {
        let r3 = (m_r / m_a).powi(3);
        (r3 * ic + (1.0 - r3) * i_ii).min(ic)
    };
    BeamStiffness {
        ic_m4: ic,
        i_cracked_m4: i_ii,
        i_eq_m4: i_eq,
        m_r_knm: m_r,
        as_cm2: as_m2 * 10_000.0,
    }
}

/// Largest deflection of a span with end moments and a uniform load, by
/// integrating the curvature twice; same units as `ei`.
#[cfg(test)]
pub fn span_deflection(length: f64, load: f64, left: f64, right: f64, ei: f64) -> f64 {
    span_deflection_with_point_loads(length, load, &[], left, right, ei)
}

/// Same as [`span_deflection`], including concentrated point loads `(a_m, p_kn)`.
pub fn span_deflection_with_point_loads(
    length: f64,
    load: f64,
    point_loads: &[(f64, f64)],
    left: f64,
    right: f64,
    ei: f64,
) -> f64 {
    let steps = 100;
    let l = length.max(1e-6);
    let dx = l / steps as f64;
    let point_v_left: f64 = point_loads
        .iter()
        .map(|&(a, p)| p * (l - a.clamp(0.0, l)) / l)
        .sum();
    let v_left = load * l / 2.0 + point_v_left + (right - left) / l;
    let moment = |x: f64| {
        let mut m = left + v_left * x - load * x * x / 2.0;
        for &(a, p) in point_loads {
            let a = a.clamp(0.0, l);
            if x > a {
                m -= p * (x - a);
            }
        }
        m
    };
    let mut slope = 0.0;
    let mut deflection = vec![0.0; steps + 1];
    for i in 0..steps {
        let (x0, x1) = (i as f64 * dx, (i + 1) as f64 * dx);
        let next = slope + (moment(x0) + moment(x1)) / 2.0 / ei * dx;
        deflection[i + 1] = deflection[i] + (slope + next) / 2.0 * dx;
        slope = next;
    }
    let end = deflection[steps];
    (0..=steps)
        .map(|i| (deflection[i] - end * i as f64 / steps as f64).abs())
        .fold(0.0, f64::max)
}
