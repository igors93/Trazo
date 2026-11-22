//! Two-way solid-slab analysis (#116).
//!
//! Rectangular panels supported on their four edges are checked with the
//! simply-supported (case 1) Czerny and Marcus tables.  The largest positive
//! moment from the two tables governs each direction.  Czerny's tabulated
//! deflection coefficient supplies the serviceability check.  Ratios above
//! two are treated as one-way slabs.

/// Which table governed a slab-panel result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlabTableMethod {
    Czerny,
    Marcus,
    OneWay,
}

impl SlabTableMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Czerny => "Czerny",
            Self::Marcus => "Marcus",
            Self::OneWay => "unidirecional",
        }
    }
}

/// One rectangular panel cut from a slab outline.
#[derive(Clone, Debug, PartialEq)]
pub struct SlabPanelResult {
    pub index: usize,
    pub lx_m: f64,
    pub ly_m: f64,
    pub ratio: f64,
    pub moment_x_knm_m: f64,
    pub moment_y_knm_m: f64,
    pub method_x: SlabTableMethod,
    pub method_y: SlabTableMethod,
    pub deflection_mm: f64,
    pub limit_mm: f64,
}

// Case 1, four simply-supported edges, ratios ly/lx from 1.00 to 2.00.
// Values at 0.05 intervals; intermediate ratios are linearly interpolated.
// Czerny columns: ratio, mx, my, deflection coefficient w.
const CZERNY: [(f64, f64, f64, f64); 21] = [
    (1.00, 27.2, 27.2, 20.534),
    (1.05, 24.5, 27.5, 18.826),
    (1.10, 22.4, 27.9, 17.118),
    (1.15, 20.7, 28.4, 15.933),
    (1.20, 19.1, 29.1, 14.748),
    (1.25, 17.8, 29.9, 13.906),
    (1.30, 16.8, 30.9, 13.064),
    (1.35, 15.8, 31.8, 12.421),
    (1.40, 15.0, 32.8, 11.779),
    (1.45, 14.3, 33.8, 11.286),
    (1.50, 13.7, 34.7, 10.794),
    (1.55, 13.2, 35.4, 10.417),
    (1.60, 12.7, 36.1, 10.039),
    (1.65, 12.3, 36.7, 9.733),
    (1.70, 11.9, 37.2, 9.428),
    (1.75, 11.6, 37.9, 9.165),
    (1.80, 11.3, 38.5, 8.903),
    (1.85, 11.1, 39.1, 8.714),
    (1.90, 10.8, 39.6, 8.526),
    (1.95, 10.6, 40.0, 8.373),
    (2.00, 10.4, 40.3, 8.224),
];

// Marcus case 1: ratio, mx, my.  The source table is tabulated every 0.01;
// these exact 0.05 ordinates preserve its engineering precision here.
const MARCUS: [(f64, f64, f64); 21] = [
    (1.00, 27.43, 27.43),
    (1.05, 24.91, 27.47),
    (1.10, 22.79, 27.57),
    (1.15, 20.99, 27.76),
    (1.20, 19.45, 28.01),
    (1.25, 18.14, 28.34),
    (1.30, 17.01, 28.76),
    (1.35, 16.05, 29.25),
    (1.40, 15.21, 29.82),
    (1.45, 14.49, 30.47),
    (1.50, 13.87, 31.21),
    (1.55, 13.32, 32.01),
    (1.60, 12.85, 32.80),
    (1.65, 12.43, 33.85),
    (1.70, 12.06, 34.87),
    (1.75, 11.74, 35.95),
    (1.80, 11.45, 37.10),
    (1.85, 11.19, 38.31),
    (1.90, 10.96, 39.58),
    (1.95, 10.76, 40.91),
    (2.00, 10.57, 42.29),
];

fn interpolate<const N: usize>(ratio: f64, table: &[(f64, [f64; N])]) -> [f64; N] {
    let ratio = ratio.clamp(table[0].0, table[table.len() - 1].0);
    let upper = table
        .iter()
        .position(|row| row.0 >= ratio - 1e-12)
        .unwrap_or(table.len() - 1);
    if upper == 0 || (table[upper].0 - ratio).abs() < 1e-12 {
        return table[upper].1;
    }
    let (a, b) = (table[upper - 1], table[upper]);
    let t = (ratio - a.0) / (b.0 - a.0);
    let mut out = [0.0; N];
    for (i, value) in out.iter_mut().enumerate() {
        *value = a.1[i] + t * (b.1[i] - a.1[i]);
    }
    out
}

fn coefficients(ratio: f64) -> ([f64; 3], [f64; 2]) {
    let czerny: Vec<_> = CZERNY
        .iter()
        .map(|&(r, mx, my, w)| (r, [mx, my, w]))
        .collect();
    let marcus: Vec<_> = MARCUS.iter().map(|&(r, mx, my)| (r, [mx, my])).collect();
    (interpolate(ratio, &czerny), interpolate(ratio, &marcus))
}

/// Analyses one rectangular panel. Loads are characteristic `g` and `q` in
/// kN/m², `E` in kN/m² and thickness in metres.
pub fn analyse_panel(
    index: usize,
    lx_m: f64,
    ly_m: f64,
    dead_kn_m2: f64,
    live_kn_m2: f64,
    thickness_m: f64,
    modulus_kn_m2: f64,
) -> SlabPanelResult {
    let lx = lx_m.min(ly_m).max(0.01);
    let ly = lx_m.max(ly_m).max(lx);
    let ratio = ly / lx;
    let load = dead_kn_m2 + live_kn_m2;
    let service = dead_kn_m2 + 0.3 * live_kn_m2;
    let limit_mm = lx / 250.0 * 1000.0;
    if ratio > 2.0 + 1e-9 {
        let moment_x = load * lx * lx / 8.0;
        let inertia = thickness_m.powi(3) / 12.0;
        let immediate =
            5.0 * service * lx.powi(4) / (384.0 * modulus_kn_m2.max(1.0) * inertia.max(1e-12));
        return SlabPanelResult {
            index,
            lx_m: lx,
            ly_m: ly,
            ratio,
            moment_x_knm_m: moment_x,
            moment_y_knm_m: 0.0,
            method_x: SlabTableMethod::OneWay,
            method_y: SlabTableMethod::OneWay,
            deflection_mm: immediate * 2.32 * 1000.0,
            limit_mm,
        };
    }
    let (c, m) = coefficients(ratio);
    let cz_x = load * lx * lx / c[0];
    let cz_y = load * lx * lx / c[1];
    let ma_x = load * lx * lx / m[0];
    let ma_y = load * lx * lx / m[1];
    let (moment_x, method_x) = if cz_x >= ma_x {
        (cz_x, SlabTableMethod::Czerny)
    } else {
        (ma_x, SlabTableMethod::Marcus)
    };
    let (moment_y, method_y) = if cz_y >= ma_y {
        (cz_y, SlabTableMethod::Czerny)
    } else {
        (ma_y, SlabTableMethod::Marcus)
    };
    let immediate =
        service * lx.powi(4) / (c[2] * modulus_kn_m2.max(1.0) * thickness_m.powi(3).max(1e-12));
    SlabPanelResult {
        index,
        lx_m: lx,
        ly_m: ly,
        ratio,
        moment_x_knm_m: moment_x,
        moment_y_knm_m: moment_y,
        method_x,
        method_y,
        deflection_mm: immediate * 2.32 * 1000.0,
        limit_mm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_panel_reproduces_the_published_czerny_and_marcus_tables() {
        let load = 6.0;
        let result = analyse_panel(1, 4.0, 4.0, load, 0.0, 0.12, 30_000_000.0);
        // Czerny: M = q·l²/27.2 = 3.529; Marcus gives 3.500.
        assert!((result.moment_x_knm_m - load * 16.0 / 27.2).abs() < 1e-9);
        assert_eq!(result.method_x, SlabTableMethod::Czerny);
        assert!((result.moment_y_knm_m - load * 16.0 / 27.2).abs() < 1e-9);
    }

    #[test]
    fn rectangular_panel_envelopes_both_published_methods() {
        let load = 6.0;
        let result = analyse_panel(1, 3.5, 5.25, load, 0.0, 0.12, 30_000_000.0);
        // λ=1.50: Czerny governs x (13.7), Marcus governs y (31.21).
        assert!((result.moment_x_knm_m - load * 3.5_f64.powi(2) / 13.7).abs() < 1e-9);
        assert!((result.moment_y_knm_m - load * 3.5_f64.powi(2) / 31.21).abs() < 1e-9);
        assert_eq!(result.method_x, SlabTableMethod::Czerny);
        assert_eq!(result.method_y, SlabTableMethod::Marcus);
    }

    #[test]
    fn long_panel_switches_to_a_one_way_strip() {
        let result = analyse_panel(1, 2.0, 5.0, 5.0, 1.5, 0.10, 30_000_000.0);
        assert_eq!(result.method_x, SlabTableMethod::OneWay);
        assert_eq!(result.moment_y_knm_m, 0.0);
        assert!((result.moment_x_knm_m - 6.5 * 4.0 / 8.0).abs() < 1e-9);
    }
}
