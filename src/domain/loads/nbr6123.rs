//! ABNT NBR 6123 — Wind forces on rectangular buildings:
//! Tables 1–3 (`S1`, `S2(z)`, `S3`), Table 4 external pressure/shape coefficients
//! (`Cpe` windward/leeward/side walls and net drag `Ca = Cpe,barlavento − Cpe,sotavento`),
//! storey-by-storey wind forces, base shear, overturning moment and `ELU-EQU`
//! rigid-body stability screening in four orthogonal directions (`+X`, `−X`, `+Y`, `−Y`).

use crate::domain::{
    geometry::Point,
    model::Project,
    structure::StructureReport,
    wind::{OPEN_CPI, WindSettings},
};

use super::nbr8681::{GAMMA_G_EQU_FAV, GAMMA_W};

/// Principal orthogonal wind incidence direction in plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindDirection {
    PosX,
    NegX,
    PosY,
    NegY,
}

impl WindDirection {
    pub const ALL: [Self; 4] = [Self::PosX, Self::NegX, Self::PosY, Self::NegY];

    pub fn label(self) -> &'static str {
        match self {
            Self::PosX => "+X (0°)",
            Self::NegX => "−X (180°)",
            Self::PosY => "+Y (90°)",
            Self::NegY => "−Y (270°)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::PosX => "+X",
            Self::NegX => "−X",
            Self::PosY => "+Y",
            Self::NegY => "−Y",
        }
    }

    pub fn along_x(self) -> bool {
        matches!(self, Self::PosX | Self::NegX)
    }
}

/// External and internal wall pressure coefficients per ABNT NBR 6123 Table 4 & §6.2.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallPressureCoeffs {
    /// Windward face external coefficient (`Cpe,barlavento > 0`).
    pub cpe_windward: f64,
    /// Leeward face external coefficient (`Cpe,sotavento < 0`).
    pub cpe_leeward: f64,
    /// Side-wall edge zone peak suction (`A1/B1` or `C1/D1`, `< 0`).
    pub cpe_side_edge: f64,
    /// Side-wall interior zone suction (`A2/B2` or `C2/D2`, `< 0`).
    pub cpe_side_mid: f64,
    /// Net drag coefficient across windward + leeward faces: `Ca = Cpe,barlavento − Cpe,sotavento`.
    pub ca_drag: f64,
    /// Governing positive internal pressure coefficient (`+0.2` closed, `+0.8` dominant opening).
    pub cpi_pos: f64,
    /// Governing negative internal pressure coefficient (`−0.3` closed, `−0.6` dominant opening).
    pub cpi_neg: f64,
    /// Aspect ratio `h / b` (height over smaller plan side).
    pub h_over_b: f64,
    /// Aspect ratio `a / b` (larger plan side over smaller plan side).
    pub a_over_b: f64,
    /// True when `h/b ≤ 6` and `a/b ≤ 4` (directly inside NBR 6123 Table 4).
    pub within_table4: bool,
}

/// Exact ABNT NBR 6123 Table 4 external pressure/shape coefficients (`Ce`) for
/// walls of rectangular-plan buildings (`a ≥ b`, height `h`).
///
/// - `wind_on_long_face = true` corresponds to NBR 6123 `α = 0°` (wind perpendicular to `a`).
/// - `wind_on_long_face = false` corresponds to NBR 6123 `α = 90°` (wind perpendicular to `b`).
/// - For `3/2 < a/b < 2`, linearly interpolates between the `1 ≤ a/b ≤ 3/2` and
///   `2 ≤ a/b ≤ 4` columns per NBR 6123 Table 4 Note 1.
pub fn nbr6123_wall_coeffs(
    h: f64,
    a_side: f64,
    b_side: f64,
    wind_on_long_face: bool,
    enclosed: bool,
) -> WallPressureCoeffs {
    let a = a_side.max(b_side).max(0.1);
    let b = a_side.min(b_side).max(0.1);
    let h_b = (h / b).max(0.0);
    let a_b = (a / b).max(1.0);
    let within_table4 = h_b <= 6.0 + 1e-9 && a_b <= 4.0 + 1e-9;

    // Band lookup for (windward, leeward, side_edge, side_mid) at `a/b ≤ 1.5` and `a/b ≥ 2.0`.
    let (low_ab, high_ab) = if h_b <= 0.5 + 1e-9 {
        // h/b ≤ 1/2
        let col1 = (0.7, -0.4, -0.8, -0.4);
        let col2 = if wind_on_long_face {
            (0.7, -0.5, -0.9, -0.5)
        } else {
            (0.7, -0.3, -0.8, -0.4)
        };
        (col1, col2)
    } else if h_b <= 1.5 + 1e-9 {
        // 1/2 < h/b ≤ 3/2
        let col1 = (0.7, -0.5, -0.9, -0.5);
        let col2 = if wind_on_long_face {
            (0.7, -0.6, -1.0, -0.6)
        } else {
            (0.7, -0.3, -0.9, -0.5)
        };
        (col1, col2)
    } else {
        // 3/2 < h/b ≤ 6 (conservatively held for h/b > 6 with within_table4 = false)
        let col1 = (0.8, -0.6, -1.0, -0.6);
        let col2 = if wind_on_long_face {
            (0.8, -0.6, -1.0, -0.6)
        } else {
            (0.8, -0.4, -1.0, -0.6)
        };
        (col1, col2)
    };

    let t = ((a_b - 1.5) / 0.5).clamp(0.0, 1.0);
    let lerp = |u: f64, v: f64| u + t * (v - u);
    let cpe_windward = lerp(low_ab.0, high_ab.0);
    let cpe_leeward = lerp(low_ab.1, high_ab.1);
    let cpe_side_edge = lerp(low_ab.2, high_ab.2);
    let cpe_side_mid = lerp(low_ab.3, high_ab.3);
    let ca_drag = cpe_windward - cpe_leeward;

    // NBR 6123 §6.2.5: closed buildings with uniform permeability use +0.2 / -0.3;
    // open-sided buildings (dominant opening) use +0.8 / -0.6.
    let (cpi_pos, cpi_neg) = if enclosed {
        (0.2, -0.3)
    } else {
        (OPEN_CPI, -0.6)
    };

    WallPressureCoeffs {
        cpe_windward,
        cpe_leeward,
        cpe_side_edge,
        cpe_side_mid,
        ca_drag,
        cpi_pos,
        cpi_neg,
        h_over_b: h_b,
        a_over_b: a_b,
        within_table4,
    }
}

/// Wind force on one storey band of the building.
#[derive(Clone, Debug, PartialEq)]
pub struct StoreyWindLoad {
    pub storey: usize,
    pub label: String,
    /// Top elevation `z` of the storey band above ground, m.
    pub z_top_m: f64,
    /// Centroid elevation `z_mid` of the band (lever arm for overturning), m.
    pub z_mid_m: f64,
    /// Height of this storey band, m.
    pub height_m: f64,
    /// NBR 6123 Table 2 factor `S2(z_top)`.
    pub s2: f64,
    /// Characteristic wind speed `Vk(z_top)`, m/s.
    pub vk_ms: f64,
    /// Dynamic pressure `q(z_top) = 0.613 · Vk² / 1000`, kPa.
    pub q_kpa: f64,
    /// Exposed facade area `A_e = facade_width · height`, m².
    pub facade_area_m2: f64,
    /// Characteristic horizontal drag force `F_w = Ca · q · A_e`, kN.
    pub force_kn: f64,
}

/// Complete NBR 6123 building wind result for one orthogonal direction.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingWindDirectionResult {
    pub direction: WindDirection,
    /// Width perpendicular to wind (`l1`), m.
    pub facade_width_m: f64,
    /// Depth parallel to wind (`l2`), m.
    pub depth_m: f64,
    /// Total height above ground `H`, m.
    pub total_height_m: f64,
    pub coeffs: WallPressureCoeffs,
    pub storeys: Vec<StoreyWindLoad>,
    /// Peak characteristic net wall pressure `(Cpe,barlavento − Cpi,neg) · q_top`, kPa.
    pub max_wall_pressure_kpa: f64,
    /// Peak characteristic net wall suction `|Cpe,lateral − Cpi,pos| · q_top`, kPa.
    pub max_wall_suction_kpa: f64,
    /// Characteristic total horizontal base shear `V_w = Σ F_w,k`, kN.
    pub base_shear_kn: f64,
    /// Characteristic diaphragm wind shear acting at the ground-floor roof/ceiling
    /// beam level (for 2D portal-frame distribution), kN.
    pub diaphragm_shear_kn: f64,
    /// Characteristic base overturning moment `M_w = Σ F_w,k · z_mid,k`, kN·m.
    pub overturning_moment_knm: f64,
    /// Design stabilizing moment under NBR 8681 `ELU-EQU`: `0.90 · G_total · depth / 2`, kN·m.
    pub stabilizing_moment_knm: f64,
    /// `M_estab / (1.40 · M_w)`: values `≥ 1.0` satisfy rigid-body overturning screening.
    pub overturning_safety_ratio: f64,
    /// Characteristic base shear over total permanent gravity `V_w / G_total`.
    pub wind_to_gravity_ratio: f64,
}

impl BuildingWindDirectionResult {
    pub fn overturning_ok(&self) -> bool {
        self.overturning_safety_ratio >= 1.0 - 1e-9
    }
}

/// Whole-building NBR 6123 wind report across all four orthogonal directions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildingWindReport {
    pub settings: WindSettings,
    pub plan_width_x_m: f64,
    pub plan_depth_y_m: f64,
    pub total_height_m: f64,
    pub building_class: &'static str,
    pub enclosed: bool,
    pub directions: Vec<BuildingWindDirectionResult>,
}

impl BuildingWindReport {
    /// Characteristic diaphragm wind shear along the global X axis (`+X`), kN.
    pub fn diaphragm_shear_x_kn(&self) -> f64 {
        self.directions
            .iter()
            .find(|d| d.direction == WindDirection::PosX)
            .map_or(0.0, |d| d.diaphragm_shear_kn)
    }

    /// Characteristic diaphragm wind shear along the global Y axis (`+Y`), kN.
    pub fn diaphragm_shear_y_kn(&self) -> f64 {
        self.directions
            .iter()
            .find(|d| d.direction == WindDirection::PosY)
            .map_or(0.0, |d| d.diaphragm_shear_kn)
    }

    /// Characteristic total building base shear along the global X axis, kN.
    pub fn base_shear_x_kn(&self) -> f64 {
        self.directions
            .iter()
            .find(|d| d.direction == WindDirection::PosX)
            .map_or(0.0, |d| d.base_shear_kn)
    }

    /// Characteristic total building base shear along the global Y axis, kN.
    pub fn base_shear_y_kn(&self) -> f64 {
        self.directions
            .iter()
            .find(|d| d.direction == WindDirection::PosY)
            .map_or(0.0, |d| d.base_shear_kn)
    }
}

/// Collects storey heights from bottom to top (real floor levels when present,
/// otherwise `settings.floors` copies of `project.wall_height`).
fn building_storey_bands(project: &Project) -> Vec<(String, f64)> {
    let active_id = project.floors.active_floor_id;
    let has_real_levels = project.floors.levels.len() > 1
        && project.floors.levels.iter().any(|l| {
            Some(l.id) != active_id
                && (!l.data.walls.is_empty()
                    || !l.data.beams.is_empty()
                    || !l.data.slabs.is_empty())
        });
    if has_real_levels {
        let mut levels: Vec<_> = project.floors.levels.iter().collect();
        levels.sort_by(|a, b| a.elevation_m.total_cmp(&b.elevation_m));
        let mut bands = Vec::with_capacity(levels.len());
        for level in levels {
            let h = if Some(level.id) == active_id {
                project.wall_height.max(2.0)
            } else {
                level.ceiling_height_m.max(2.0)
            };
            bands.push((level.name.clone(), h));
        }
        return bands;
    }

    let n = project.structure.settings.floors.max(1) as usize;
    let base_h = project.wall_height.max(2.0);
    if n == 1 {
        let tall_h = project
            .walls
            .iter()
            .map(|w| {
                let (h0, h1) = w.heights(base_h);
                h0.max(h1)
            })
            .chain(project.structure.roofs.iter().map(|r| r.high_m))
            .fold(base_h, f64::max);
        vec![("Térreo".to_owned(), tall_h)]
    } else {
        (0..n)
            .map(|i| {
                let name = if i == 0 {
                    "Térreo".to_owned()
                } else {
                    format!("{}º Pavimento", i + 1)
                };
                (name, base_h)
            })
            .collect()
    }
}

/// Computes NBR 6123 wind loads on the whole building across `+X`, `−X`, `+Y`, `−Y`.
pub fn analyze_building_wind(
    project: &Project,
    report: &StructureReport,
    total_g_kn: f64,
) -> BuildingWindReport {
    let wind = project.structure.wind;
    let mut points: Vec<Point> = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .collect();
    for roof in &project.structure.roofs {
        points.extend([roof.min, roof.max]);
    }
    for col in project.structural_columns() {
        points.push(col.center);
    }
    let Some(first) = points.first().copied() else {
        return BuildingWindReport {
            settings: wind,
            building_class: "A",
            enclosed: true,
            ..BuildingWindReport::default()
        };
    };
    let (lo, hi) = points.into_iter().fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    });
    let plan_x = (hi.x - lo.x).max(0.0);
    let plan_y = (hi.y - lo.y).max(0.0);
    if plan_x < 0.20 && plan_y < 0.20 {
        return BuildingWindReport {
            settings: wind,
            building_class: "A",
            enclosed: true,
            ..BuildingWindReport::default()
        };
    }

    let bands = building_storey_bands(project);
    let total_height_m: f64 = bands.iter().map(|(_, h)| *h).sum();
    let max_dim = plan_x.max(plan_y).max(total_height_m);
    let building_class = WindSettings::building_class(max_dim);
    // An open-sided roof (bearing on timber posts / open edge) marks the building as non-enclosed.
    let enclosed = !report.wind.roofs.iter().any(|r| r.open);
    let a = plan_x.max(plan_y).max(0.1);
    let b = plan_x.min(plan_y).max(0.1);

    let mut directions = Vec::with_capacity(4);
    for direction in WindDirection::ALL {
        let (facade_width_m, depth_m) = if direction.along_x() {
            (plan_y.max(0.5), plan_x.max(0.5))
        } else {
            (plan_x.max(0.5), plan_y.max(0.5))
        };
        // Wind hits the long face (`α = 0°`) when facade_width >= depth.
        let wind_on_long_face = facade_width_m >= depth_m - 1e-6;
        let coeffs = nbr6123_wall_coeffs(total_height_m, a, b, wind_on_long_face, enclosed);

        let mut storeys = Vec::with_capacity(bands.len());
        let mut z_cursor = 0.0;
        let mut base_shear_kn = 0.0;
        let mut overturning_moment_knm = 0.0;
        let mut q_top = 0.0_f64;

        for (idx, (label, h)) in bands.iter().enumerate() {
            let z_top = z_cursor + *h;
            let z_mid = z_cursor + 0.5 * *h;
            let s2 = wind.s2(z_top, max_dim);
            let vk_ms = wind.vk_ms(z_top, max_dim);
            let q_kpa = wind.pressure_kpa(z_top, max_dim);
            let facade_area_m2 = facade_width_m * *h;
            let force_kn = coeffs.ca_drag * q_kpa * facade_area_m2;
            base_shear_kn += force_kn;
            overturning_moment_knm += force_kn * z_mid;
            q_top = q_top.max(q_kpa);
            storeys.push(StoreyWindLoad {
                storey: idx + 1,
                label: label.clone(),
                z_top_m: z_top,
                z_mid_m: z_mid,
                height_m: *h,
                s2,
                vk_ms,
                q_kpa,
                facade_area_m2,
                force_kn,
            });
            z_cursor = z_top;
        }

        // Horizontal shear transferred to the ground-floor roof/ceiling beam diaphragm:
        // half of the ground-floor band goes directly to the foundation slab, while the
        // upper half plus all upper-storey bands pass through the ground-floor frame.
        let diaphragm_shear_kn = if let Some(first_band) = storeys.first() {
            (base_shear_kn - 0.5 * first_band.force_kn).max(0.0)
        } else {
            0.0
        };

        let max_wall_pressure_kpa = (coeffs.cpe_windward - coeffs.cpi_neg) * q_top;
        let max_wall_suction_kpa = (coeffs.cpe_side_edge - coeffs.cpi_pos).abs() * q_top;

        // Rigid-body ELU-EQU overturning check (0.90·G·depth/2 vs 1.40·M_w).
        let stabilizing_moment_knm = GAMMA_G_EQU_FAV * total_g_kn * (depth_m * 0.5);
        let design_overturning_knm = GAMMA_W * overturning_moment_knm;
        let overturning_safety_ratio = if design_overturning_knm > 1e-9 {
            stabilizing_moment_knm / design_overturning_knm
        } else {
            f64::INFINITY
        };
        let wind_to_gravity_ratio = if total_g_kn > 1e-9 {
            base_shear_kn / total_g_kn
        } else {
            0.0
        };

        directions.push(BuildingWindDirectionResult {
            direction,
            facade_width_m,
            depth_m,
            total_height_m,
            coeffs,
            storeys,
            max_wall_pressure_kpa,
            max_wall_suction_kpa,
            base_shear_kn,
            diaphragm_shear_kn,
            overturning_moment_knm,
            stabilizing_moment_knm,
            overturning_safety_ratio,
            wind_to_gravity_ratio,
        });
    }

    BuildingWindReport {
        settings: wind,
        plan_width_x_m: plan_x,
        plan_depth_y_m: plan_y,
        total_height_m,
        building_class,
        enclosed,
        directions,
    }
}
