//! Structural pre-sizing of reinforced-concrete stairs, balconies and
//! cantilevers (#115), following ABNT NBR 6118:2023 (§13.2.4 Table 13.2 `γ_n`
//! additional safety factor for cantilever slabs, Table 13.3 `L/125` cantilever
//! deflection limit, §17.3.2 creep) and NBR 6120:2019 (Table 10 stair & balcony
//! live loads and guardrail thrust).

use serde::{Deserialize, Serialize};

use super::{
    circulation::{CirculationElement, CirculationKind, StairShape},
    geometry::{nearest_point_on_segment, polygon_area},
    model::Project,
    structure::{CONCRETE_KN_M3, OccupancyClass, SlabKind, round_up},
    verdict::Light,
};

/// Optional explicit cantilever balcony or cantilever beam declared on the project
/// (in addition to automatic detection of stairs and balcony/1-edge-supported slabs).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CantileverSpec {
    /// Explicit cantilever balcony slab (*sacada / marquise em balanço*).
    Slab {
        id: u64,
        label: String,
        /// Clear cantilever projection `L_bal`, metres (0.40 to 3.00).
        span_m: f64,
        /// Width along the support edge, metres.
        width_m: f64,
        /// Solid slab thickness, metres (≥ 0.10 m).
        thickness_m: f64,
        /// Adjacent back-span for overturning balance, metres.
        #[serde(default)]
        back_span_m: f64,
        /// Parapet/guardrail tip vertical line load, kN/m (default 1.50 kN/m).
        #[serde(default = "default_parapet_kn_m")]
        tip_load_kn_m: f64,
    },
    /// Explicit cantilever beam (*viga em balanço*).
    Beam {
        id: u64,
        label: String,
        /// Cantilever projection `L_bal`, metres (0.50 to 3.50).
        span_m: f64,
        /// Beam width `b`, metres.
        width_m: f64,
        /// Beam depth `h`, metres.
        height_m: f64,
        /// Additional characteristic distributed load (excluding beam self-weight), kN/m.
        distributed_kn_m: f64,
        /// Characteristic point load at free tip, kN.
        #[serde(default)]
        tip_point_load_kn: f64,
        /// Adjacent back-span length for anchorage/equilibrium, metres.
        #[serde(default)]
        back_span_m: f64,
    },
}

fn default_parapet_kn_m() -> f64 {
    1.50
}

impl CantileverSpec {
    pub fn id(&self) -> u64 {
        match self {
            Self::Slab { id, .. } | Self::Beam { id, .. } => *id,
        }
    }

    pub fn set_id(&mut self, new_id: u64) {
        match self {
            Self::Slab { id, .. } | Self::Beam { id, .. } => *id = new_id,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Slab {
                span_m,
                width_m,
                thickness_m,
                back_span_m,
                tip_load_kn_m,
                ..
            } => {
                if !span_m.is_finite() || !(0.30..=4.0).contains(span_m) {
                    return Err("O balanço da laje deve ficar entre 0,30 e 4,00 m.".to_owned());
                }
                if !width_m.is_finite() || !(0.50..=30.0).contains(width_m) {
                    return Err(
                        "A largura da laje em balanço deve ficar entre 0,50 e 30 m.".to_owned()
                    );
                }
                if !thickness_m.is_finite() || !(0.08..=0.40).contains(thickness_m) {
                    return Err(
                        "A espessura da laje em balanço deve ficar entre 8 e 40 cm.".to_owned()
                    );
                }
                if !back_span_m.is_finite()
                    || *back_span_m < 0.0
                    || !tip_load_kn_m.is_finite()
                    || *tip_load_kn_m < 0.0
                {
                    return Err("Valores de contravão ou carga na ponta inválidos.".to_owned());
                }
                Ok(())
            }
            Self::Beam {
                span_m,
                width_m,
                height_m,
                distributed_kn_m,
                tip_point_load_kn,
                back_span_m,
                ..
            } => {
                if !span_m.is_finite() || !(0.30..=5.0).contains(span_m) {
                    return Err("O balanço da viga deve ficar entre 0,30 e 5,00 m.".to_owned());
                }
                if !width_m.is_finite() || !(0.12..=0.80).contains(width_m) {
                    return Err(
                        "A largura da viga em balanço deve ficar entre 12 e 80 cm.".to_owned()
                    );
                }
                if !height_m.is_finite() || !(0.20..=1.50).contains(height_m) {
                    return Err(
                        "A altura da viga em balanço deve ficar entre 20 e 150 cm.".to_owned()
                    );
                }
                if !distributed_kn_m.is_finite()
                    || *distributed_kn_m < 0.0
                    || !tip_point_load_kn.is_finite()
                    || *tip_point_load_kn < 0.0
                    || !back_span_m.is_finite()
                    || *back_span_m < 0.0
                {
                    return Err("Cargas ou contravão da viga em balanço inválidos.".to_owned());
                }
                Ok(())
            }
        }
    }
}

/// NBR 6118:2023 §13.2.4.1 Table 13.2 additional factor `γ_n` for thin cantilever slabs:
/// `γ_n = 1.95 − 0.05 · h_cm` for `10 cm ≤ h < 19 cm`, `1.0` for `h ≥ 19 cm`.
pub fn nbr6118_cantilever_gamma_n(thickness_m: f64) -> f64 {
    let h_cm = (thickness_m * 100.0).max(10.0);
    if h_cm >= 19.0 {
        1.0
    } else {
        (1.95 - 0.05 * h_cm).max(1.0)
    }
}

/// Concrete secant modulus `E_cs` (kPa), NBR 6118:2023 §8.2.8.
fn concrete_ecs_kpa(fck_mpa: f64) -> f64 {
    let fck = fck_mpa.max(20.0);
    let alpha_i = (0.8 + 0.2 * fck / 80.0).min(1.0);
    alpha_i * 5600.0 * fck.sqrt() * 1000.0
}

/// Singly reinforced rectangular section CA-50 steel area `As` (cm²) for
/// design moment `Md` (kN·m), width `b_m` (m), effective depth `d_m` (m),
/// `fck` (MPa) and minimum ratio `rho_min`.
fn required_steel_cm2(md_knm: f64, b_m: f64, d_m: f64, fck_mpa: f64, rho_min: f64) -> (f64, bool) {
    let fcd_kpa = (fck_mpa.max(20.0) / 1.4) * 1000.0;
    let fyd_kpa = (500.0 / 1.15) * 1000.0;
    let b = b_m.max(0.10);
    let d = d_m.max(0.05);
    let kmd = md_knm / (b * d * d * 0.85 * fcd_kpa);
    let disc = 1.0 - 2.0 * kmd;
    let ductile_ok = disc >= 0.0 && {
        let kx = (1.0 - disc.sqrt()) / 0.8;
        kx <= 0.45
    };
    let z = if disc >= 0.0 {
        let kx = ((1.0 - disc.sqrt()) / 0.8).min(0.45);
        d * (1.0 - 0.4 * kx)
    } else {
        0.82 * d
    };
    let as_calc_cm2 = (md_knm / (z * fyd_kpa).max(1e-6)) * 10_000.0;
    let as_min_cm2 = rho_min * b * (d + 0.03) * 10_000.0;
    (as_calc_cm2.max(as_min_cm2), ductile_ok)
}

#[derive(Clone, Debug, PartialEq)]
pub struct StairStructureResult {
    pub id: u64,
    pub label: String,
    pub name: String,
    pub shape: StairShape,
    pub width_m: f64,
    pub flight_span_m: f64,
    pub total_rise_m: f64,
    pub slope_deg: f64,
    pub waist_m: f64,
    pub waist_min_m: f64,
    pub dead_kn_m2: f64,
    pub live_kn_m2: f64,
    pub moment_design_knm_m: f64,
    pub reaction_kn_m: f64,
    pub reaction_total_kn: f64,
    pub as_bottom_cm2_m: f64,
    pub as_top_cm2_m: f64,
    pub deflection_mm: f64,
    pub limit_mm: f64,
    pub utilisation: f64,
    pub light: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BalconyCantileverResult {
    pub id: u64,
    pub label: String,
    pub source: String,
    pub span_m: f64,
    pub width_m: f64,
    pub thickness_m: f64,
    pub thickness_min_m: f64,
    pub is_ribbed_warning: bool,
    pub gamma_n: f64,
    pub dead_kn_m2: f64,
    pub live_kn_m2: f64,
    pub tip_load_kn_m: f64,
    pub guardrail_moment_knm_m: f64,
    pub moment_k_neg_knm_m: f64,
    pub moment_d_neg_knm_m: f64,
    pub shear_k_kn_m: f64,
    pub as_top_cm2_m: f64,
    pub required_back_span_m: f64,
    pub back_span_m: f64,
    pub back_span_ok: bool,
    pub deflection_mm: f64,
    pub limit_mm: f64,
    pub ductile_ok: bool,
    pub utilisation: f64,
    pub light: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CantileverBeamResult {
    pub id: u64,
    pub label: String,
    pub span_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    pub height_min_m: f64,
    pub distributed_total_kn_m: f64,
    pub tip_point_load_kn: f64,
    pub moment_k_neg_knm: f64,
    pub moment_d_neg_knm: f64,
    pub shear_k_kn: f64,
    pub as_top_cm2: f64,
    pub required_back_span_m: f64,
    pub back_span_m: f64,
    pub back_span_ok: bool,
    pub deflection_mm: f64,
    pub limit_mm: f64,
    pub ductile_ok: bool,
    pub utilisation: f64,
    pub light: Light,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CantileverReport {
    pub stairs: Vec<StairStructureResult>,
    pub balconies: Vec<BalconyCantileverResult>,
    pub beams: Vec<CantileverBeamResult>,
}

impl CantileverReport {
    pub fn is_empty(&self) -> bool {
        self.stairs.is_empty() && self.balconies.is_empty() && self.beams.is_empty()
    }
}

fn is_balcony_room_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("sacada")
        || lower.contains("varanda")
        || lower.contains("balcão")
        || lower.contains("balcao")
        || lower.contains("balanço")
        || lower.contains("balanco")
        || lower.contains("marquise")
}

fn analyse_stair(
    idx: usize,
    circ: &CirculationElement,
    fck_mpa: f64,
    finish_kn_m2: f64,
    occupancy: OccupancyClass,
) -> Option<StairStructureResult> {
    let CirculationKind::Stair {
        shape,
        width_m,
        total_rise_m,
        tread_m,
        landing_depth_m,
    } = circ.kind
    else {
        return None;
    };
    let risers = (total_rise_m / 0.175).round().max(2.0);
    let riser_h = total_rise_m / risers;
    let tread = tread_m.clamp(0.24, 0.40);
    let total_treads = (risers - 1.0).max(1.0);
    let flight_span_m = match shape {
        StairShape::Straight => total_treads * tread,
        StairShape::LShaped | StairShape::UShaped => {
            (total_treads * 0.5).ceil() * tread + landing_depth_m.max(width_m)
        }
    }
    .max(1.2);
    let alpha = (riser_h / tread).atan();
    let cos_a = alpha.cos().max(0.5);

    let waist_min_m = round_up((flight_span_m / 25.0).max(0.12), 0.01);
    let ecs = concrete_ecs_kpa(fck_mpa);
    let limit_mm = flight_span_m * 1000.0 / 250.0;
    // NBR 6120:2019 Table 10: 2.5 kN/m² private dwelling stair, 3.0 kN/m² public/office/retail.
    let live_kn_m2 = match occupancy {
        OccupancyClass::Residential => 2.5,
        OccupancyClass::Office | OccupancyClass::Retail | OccupancyClass::Garage => 3.0,
    };

    // Select the thinnest practical waist (from L/25 up to 24 cm) that satisfies L/250 deflection.
    let mut waist_m = waist_min_m;
    let mut dead_kn_m2 = 0.0;
    let mut deflection_mm = 0.0;
    let fctm_kpa = 0.3 * fck_mpa.max(20.0).cbrt().powi(2) * 1000.0;
    for step in 0..=12 {
        let candidate = waist_min_m + (step as f64) * 0.01;
        let h_eq = candidate / cos_a + riser_h * 0.5;
        let g = CONCRETE_KN_M3 * h_eq + finish_kn_m2;
        let q_qp = g + 0.3 * live_kn_m2;
        let m_qp = q_qp * flight_span_m * flight_span_m / 8.0;
        let ic = 1.0 * candidate.powi(3) / 12.0;
        let mr = 1.5 * fctm_kpa * (1.0 * candidate * candidate / 6.0);
        // Branson Stage I/II stiffness interpolation (NBR 6118 §17.3.2.1.1).
        let i_eq = if m_qp <= mr {
            ic
        } else {
            let ratio = (mr / m_qp).powi(3);
            (ratio * ic + (1.0 - ratio) * (0.35 * ic)).min(ic)
        };
        let w_imm_m = 5.0 * q_qp * flight_span_m.powi(4) / (384.0 * ecs * i_eq);
        let w_tot = w_imm_m * 2.32 * 1000.0;
        waist_m = candidate;
        dead_kn_m2 = g;
        deflection_mm = w_tot;
        if w_tot <= limit_mm || candidate >= 0.24 {
            break;
        }
    }

    let q_total = dead_kn_m2 + live_kn_m2;
    let mk = q_total * flight_span_m * flight_span_m / 8.0;
    let md = 1.4 * mk;
    let reaction_kn_m = q_total * flight_span_m * 0.5;
    let reaction_total_kn = reaction_kn_m * width_m;

    let d_m = (waist_m - 0.025).max(0.06);
    let (as_bottom, ductile_ok) = required_steel_cm2(md, 1.0, d_m, fck_mpa, 0.0015);
    let as_top = (0.25 * as_bottom).max(0.90);

    let u_span: f64 = if flight_span_m > 5.5 { 1.5 } else { 0.0 };
    let u_defl = deflection_mm / limit_mm.max(1e-6);
    let u_duct: f64 = if ductile_ok { 0.0 } else { 1.25 };
    let util = u_span.max(u_defl).max(u_duct);

    Some(StairStructureResult {
        id: circ.id,
        label: format!("ESC{}", idx + 1),
        name: if circ.name.trim().is_empty() {
            format!("Escada {}", idx + 1)
        } else {
            circ.name.clone()
        },
        shape,
        width_m,
        flight_span_m,
        total_rise_m,
        slope_deg: alpha.to_degrees(),
        waist_m,
        waist_min_m,
        dead_kn_m2,
        live_kn_m2,
        moment_design_knm_m: md,
        reaction_kn_m,
        reaction_total_kn,
        as_bottom_cm2_m: as_bottom,
        as_top_cm2_m: as_top,
        deflection_mm,
        limit_mm,
        utilisation: util,
        light: Light::from_utilisation(util),
    })
}

#[allow(clippy::too_many_arguments)]
fn analyse_balcony_slab(
    id: u64,
    label: String,
    source: String,
    span_m: f64,
    width_m: f64,
    thickness_m: f64,
    is_ribbed_warning: bool,
    back_span_m: f64,
    tip_load_kn_m: f64,
    fck_mpa: f64,
    finish_kn_m2: f64,
    base_live_kn_m2: f64,
) -> BalconyCantileverResult {
    let thickness_min_m = round_up((span_m / 10.0).max(0.10), 0.01);
    let gamma_n = nbr6118_cantilever_gamma_n(thickness_m);
    let dead_kn_m2 = CONCRETE_KN_M3 * thickness_m + finish_kn_m2;
    // NBR 6120:2019 Table 10: balconies/varandas carry at least 2.5 kN/m².
    let live_kn_m2 = base_live_kn_m2.max(2.5);
    // NBR 6120 guardrail horizontal thrust (0.80 kN/m at 1.10 m height -> 0.88 kN·m/m).
    let guardrail_moment_knm_m = if tip_load_kn_m > 0.0 {
        0.80 * 1.10
    } else {
        0.0
    };

    let q_total = dead_kn_m2 + live_kn_m2;
    let moment_k_neg =
        q_total * span_m * span_m * 0.5 + tip_load_kn_m * span_m + guardrail_moment_knm_m;
    let moment_d_neg = 1.4 * gamma_n * moment_k_neg;
    let shear_k = q_total * span_m + tip_load_kn_m;

    let d_m = (thickness_m - 0.03).max(0.05);
    let (as_top_cm2_m, ductile_ok) = required_steel_cm2(moment_d_neg, 1.0, d_m, fck_mpa, 0.0015);

    // NBR 6118 Table 13.3 cantilever deflection limit = 2 * L_bal / 250 = L_bal / 125.
    let q_qp = dead_kn_m2 + 0.3 * live_kn_m2;
    let ecs = concrete_ecs_kpa(fck_mpa);
    let i_eff = 0.45 * (1.0 * thickness_m.powi(3) / 12.0);
    let w_imm_m = q_qp * span_m.powi(4) / (8.0 * ecs * i_eff)
        + tip_load_kn_m * span_m.powi(3) / (3.0 * ecs * i_eff)
        + (0.3 * guardrail_moment_knm_m) * span_m.powi(2) / (2.0 * ecs * i_eff);
    let deflection_mm = w_imm_m * 2.32 * 1000.0;
    let limit_mm = span_m * 1000.0 / 125.0;

    let required_back_span_m = 1.5 * span_m;
    let back_span_ok = back_span_m + 1e-6 >= required_back_span_m;

    let u_thick = thickness_min_m / thickness_m.max(1e-6);
    let u_defl = deflection_mm / limit_mm.max(1e-6);
    let u_back = if back_span_m > 1e-4 {
        required_back_span_m / back_span_m
    } else {
        1.25
    };
    let u_lim = if span_m > 2.50 { 1.5 } else { 0.0 };
    let u_duct = if ductile_ok && !is_ribbed_warning {
        0.0
    } else {
        1.25
    };
    let util = u_thick.max(u_defl).max(u_back).max(u_lim).max(u_duct);

    BalconyCantileverResult {
        id,
        label,
        source,
        span_m,
        width_m,
        thickness_m,
        thickness_min_m,
        is_ribbed_warning,
        gamma_n,
        dead_kn_m2,
        live_kn_m2,
        tip_load_kn_m,
        guardrail_moment_knm_m,
        moment_k_neg_knm_m: moment_k_neg,
        moment_d_neg_knm_m: moment_d_neg,
        shear_k_kn_m: shear_k,
        as_top_cm2_m,
        required_back_span_m,
        back_span_m,
        back_span_ok,
        deflection_mm,
        limit_mm,
        ductile_ok,
        utilisation: util,
        light: Light::from_utilisation(util),
    }
}

#[allow(clippy::too_many_arguments)]
fn analyse_cantilever_beam(
    id: u64,
    label: String,
    span_m: f64,
    width_m: f64,
    height_m: f64,
    distributed_kn_m: f64,
    tip_point_load_kn: f64,
    back_span_m: f64,
    fck_mpa: f64,
) -> CantileverBeamResult {
    let height_min_m = round_up((span_m / 5.0).max(0.30), 0.05);
    let own_wt = CONCRETE_KN_M3 * width_m * height_m;
    let q_total = own_wt + distributed_kn_m;

    let mk = q_total * span_m * span_m * 0.5 + tip_point_load_kn * span_m;
    let md = 1.4 * mk;
    let vk = q_total * span_m + tip_point_load_kn;

    let d_m = (height_m - 0.04).max(0.10);
    let (as_top_cm2, ductile_ok) = required_steel_cm2(md, width_m, d_m, fck_mpa, 0.0015);

    // Quasi-permanent cantilever beam deflection vs L/125 (NBR 6118 Table 13.3).
    let q_qp = own_wt + 0.75 * distributed_kn_m;
    let p_qp = 0.75 * tip_point_load_kn;
    let ecs = concrete_ecs_kpa(fck_mpa);
    let i_eff = 0.45 * (width_m * height_m.powi(3) / 12.0);
    let w_imm_m =
        q_qp * span_m.powi(4) / (8.0 * ecs * i_eff) + p_qp * span_m.powi(3) / (3.0 * ecs * i_eff);
    let deflection_mm = w_imm_m * 2.32 * 1000.0;
    let limit_mm = span_m * 1000.0 / 125.0;

    let required_back_span_m = 1.5 * span_m;
    let back_span_ok = back_span_m + 1e-6 >= required_back_span_m;

    let u_h = height_min_m / height_m.max(1e-6);
    let u_defl = deflection_mm / limit_mm.max(1e-6);
    let u_back = if back_span_m > 1e-4 {
        required_back_span_m / back_span_m
    } else {
        1.25
    };
    let u_lim = if span_m > 3.00 { 1.5 } else { 0.0 };
    let u_duct = if ductile_ok { 0.0 } else { 1.25 };
    let util = u_h.max(u_defl).max(u_back).max(u_lim).max(u_duct);

    CantileverBeamResult {
        id,
        label,
        span_m,
        width_m,
        height_m,
        height_min_m,
        distributed_total_kn_m: q_total,
        tip_point_load_kn,
        moment_k_neg_knm: mk,
        moment_d_neg_knm: md,
        shear_k_kn: vk,
        as_top_cm2,
        required_back_span_m,
        back_span_m,
        back_span_ok,
        deflection_mm,
        limit_mm,
        ductile_ok,
        utilisation: util,
        light: Light::from_utilisation(util),
    }
}

impl Project {
    /// Adds an explicit cantilever balcony slab or cantilever beam (#115).
    pub fn add_cantilever(&mut self, mut spec: CantileverSpec) -> Result<u64, String> {
        spec.validate()?;
        let id = self.id();
        spec.set_id(id);
        self.structure.cantilevers.push(spec);
        Ok(id)
    }

    pub fn remove_cantilever(&mut self, id: u64) -> bool {
        let before = self.structure.cantilevers.len();
        self.structure.cantilevers.retain(|c| c.id() != id);
        self.structure.cantilevers.len() != before
    }

    /// Analyses all concrete stairs, balcony/cantilever slabs and cantilever beams (#115).
    pub fn analyse_cantilevers(&self) -> CantileverReport {
        let s = self.structure.settings;
        let mut report = CantileverReport::default();

        // 1. Concrete stairs from drawn `CirculationKind::Stair`.
        let mut stair_idx = 0usize;
        for circ in &self.circulations {
            if let Some(res) =
                analyse_stair(stair_idx, circ, s.fck_mpa, s.finish_kn_m2, s.occupancy)
            {
                report.stairs.push(res);
                stair_idx += 1;
            }
        }

        // 2. Automatically detected balconies / 1-edge-supported cantilever slabs.
        let mut bal_idx = 0usize;
        let beams_pts: Vec<_> = self
            .valid_beams()
            .filter_map(|b| self.beam_endpoints(b))
            .collect();
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|r| r.id == slab.room_id) else {
                continue;
            };
            let Some(poly) = self.room_polygon(room) else {
                continue;
            };
            if poly.len() < 3 || polygon_area(&poly) < 0.25 {
                continue;
            }
            let mut supported_edges = 0usize;
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                let mid = a.lerp(b, 0.5);
                let has_beam = beams_pts.iter().any(|(ba, bb)| {
                    let (n, _) = nearest_point_on_segment(mid, *ba, *bb);
                    n.distance(mid) <= 0.35
                });
                if has_beam {
                    supported_edges += 1;
                }
            }
            let named_balcony = is_balcony_room_name(room.display_name());
            // A slab supported on 3 or 4 sides is a regular supported slab, not a cantilever.
            let is_cantilever = supported_edges == 1 || (named_balcony && supported_edges <= 2);
            if !is_cantilever {
                continue;
            }
            let (min_x, max_x, min_y, max_y) = poly.iter().fold(
                (
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(x0, x1, y0, y1), p| (x0.min(p.x), x1.max(p.x), y0.min(p.y), y1.max(p.y)),
            );
            let dx = max_x - min_x;
            let dy = max_y - min_y;
            let span_m = dx.min(dy);
            let width_m = dx.max(dy);
            // Estimate adjacent interior room span along the cantilever direction.
            let back_span_m = self
                .rooms
                .iter()
                .filter(|r| r.id != room.id)
                .filter_map(|r| self.room_polygon(r))
                .map(|p| {
                    let (x0, x1, y0, y1) = p.iter().fold(
                        (
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                        ),
                        |(x0, x1, y0, y1), pt| {
                            (x0.min(pt.x), x1.max(pt.x), y0.min(pt.y), y1.max(pt.y))
                        },
                    );
                    (x1 - x0).min(y1 - y0)
                })
                .fold(0.0_f64, f64::max);
            bal_idx += 1;
            report.balconies.push(analyse_balcony_slab(
                slab.id,
                format!("BAL{bal_idx}"),
                room.display_name().to_owned(),
                span_m,
                width_m,
                slab.thickness_m,
                slab.kind == SlabKind::Ribbed,
                back_span_m,
                default_parapet_kn_m(),
                s.fck_mpa,
                s.finish_kn_m2,
                s.live_kn_m2,
            ));
        }

        // 3. Explicit user/MCP cantilever specs (slabs & beams).
        let mut beam_idx = 0usize;
        for spec in &self.structure.cantilevers {
            match spec {
                CantileverSpec::Slab {
                    id,
                    label,
                    span_m,
                    width_m,
                    thickness_m,
                    back_span_m,
                    tip_load_kn_m,
                } => {
                    bal_idx += 1;
                    let tag = if label.trim().is_empty() {
                        format!("BAL{bal_idx}")
                    } else {
                        label.clone()
                    };
                    report.balconies.push(analyse_balcony_slab(
                        *id,
                        tag,
                        "Balanço declarado".to_owned(),
                        *span_m,
                        *width_m,
                        *thickness_m,
                        false,
                        *back_span_m,
                        *tip_load_kn_m,
                        s.fck_mpa,
                        s.finish_kn_m2,
                        s.live_kn_m2,
                    ));
                }
                CantileverSpec::Beam {
                    id,
                    label,
                    span_m,
                    width_m,
                    height_m,
                    distributed_kn_m,
                    tip_point_load_kn,
                    back_span_m,
                } => {
                    beam_idx += 1;
                    let tag = if label.trim().is_empty() {
                        format!("VB{beam_idx}")
                    } else {
                        label.clone()
                    };
                    report.beams.push(analyse_cantilever_beam(
                        *id,
                        tag,
                        *span_m,
                        *width_m,
                        *height_m,
                        *distributed_kn_m,
                        *tip_point_load_kn,
                        *back_span_m,
                        s.fck_mpa,
                    ));
                }
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn cantilever_balcony_matches_nbr6118_table13_2_gamma_n_and_l125_deflection() {
        // Worked hand example: 1.50 m solid balcony slab, h = 15 cm, back_span = 3.50 m,
        // fck = 25 MPa, finish = 1.0 kN/m², live = 2.5 kN/m², parapet = 1.50 kN/m.
        // NBR 6118 §13.2.4.1 Table 13.2: γ_n = 1.95 - 0.05 * 15 = 1.20.
        // g = 25 * 0.15 + 1.0 = 4.75 kN/m²; q = 2.5 kN/m²; q_tot = 7.25 kN/m².
        // M_guard = 0.80 * 1.10 = 0.88 kN·m/m.
        // Mk_neg = 7.25 * 1.5^2 / 2 + 1.50 * 1.50 + 0.88 = 8.15625 + 2.25 + 0.88 = 11.28625 kN·m/m.
        // Md_neg = 1.4 * 1.20 * 11.28625 = 18.9609 kN·m/m.
        // Deflection limit = L / 125 = 1500 / 125 = 12.0 mm.
        assert!((nbr6118_cantilever_gamma_n(0.15) - 1.20).abs() < 1e-9);
        assert!((nbr6118_cantilever_gamma_n(0.12) - 1.35).abs() < 1e-9);
        assert!((nbr6118_cantilever_gamma_n(0.20) - 1.00).abs() < 1e-9);

        let mut project = Project::default();
        project.structure.settings.fck_mpa = 25.0;
        project.structure.settings.finish_kn_m2 = 1.0;
        project.structure.settings.live_kn_m2 = 1.5; // balcony raises to 2.5 min per NBR 6120
        project
            .add_cantilever(CantileverSpec::Slab {
                id: 0,
                label: "Sacada Frontal".to_owned(),
                span_m: 1.50,
                width_m: 3.00,
                thickness_m: 0.15,
                back_span_m: 3.50,
                tip_load_kn_m: 1.50,
            })
            .unwrap();

        let rep = project.analyse_cantilevers();
        assert_eq!(rep.balconies.len(), 1);
        let b = &rep.balconies[0];
        assert!((b.gamma_n - 1.20).abs() < 1e-9);
        assert!((b.moment_k_neg_knm_m - 11.28625).abs() < 1e-4);
        assert!((b.moment_d_neg_knm_m - 18.9609).abs() < 1e-3);
        assert!((b.limit_mm - 12.0).abs() < 1e-6);
        assert!(b.back_span_ok);
        assert!(b.as_top_cm2_m > 3.0);
        assert!(b.light != Light::Red);

        // Short back-span (< 1.5 * L_bal) must flag overturning red!
        project
            .add_cantilever(CantileverSpec::Slab {
                id: 0,
                label: "Sacada Curta".to_owned(),
                span_m: 1.80,
                width_m: 2.50,
                thickness_m: 0.18,
                back_span_m: 1.50,
                tip_load_kn_m: 1.50,
            })
            .unwrap();
        let rep2 = project.analyse_cantilevers();
        assert!(!rep2.balconies[1].back_span_ok);
        assert_eq!(rep2.balconies[1].light, Light::Red);
    }

    #[test]
    fn stair_and_cantilever_beam_match_worked_hand_calculations() {
        let mut project = Project::default();
        project.structure.settings.fck_mpa = 25.0;
        project.structure.settings.finish_kn_m2 = 1.0;

        // Straight concrete stair: total rise 2.80 m (16 risers of 17.5 cm, 15 treads of 28 cm -> L = 4.20 m).
        project
            .add_stair(Point::new(1.0, 1.0), StairShape::Straight, 1.20, 2.80, 0.28)
            .unwrap();

        // Cantilever concrete beam: L = 1.80 m, 15×40 cm, q_ext = 10 kN/m, P_tip = 8 kN, back_span = 3.5 m.
        // Own weight = 25 * 0.15 * 0.40 = 1.50 kN/m -> q_tot = 11.50 kN/m.
        // Mk = 11.50 * 1.8^2 / 2 + 8.0 * 1.8 = 18.63 + 14.40 = 33.03 kN·m.
        // Md = 1.4 * 33.03 = 46.242 kN·m.
        // Vk = 11.50 * 1.8 + 8.0 = 28.70 kN.
        project
            .add_cantilever(CantileverSpec::Beam {
                id: 0,
                label: "VB1".to_owned(),
                span_m: 1.80,
                width_m: 0.15,
                height_m: 0.40,
                distributed_kn_m: 10.0,
                tip_point_load_kn: 8.0,
                back_span_m: 3.50,
            })
            .unwrap();

        let rep = project.analyse_cantilevers();
        assert_eq!(rep.stairs.len(), 1);
        let st = &rep.stairs[0];
        assert!((st.flight_span_m - 4.20).abs() < 1e-6);
        assert!(st.moment_design_knm_m > 20.0);
        assert!(st.as_bottom_cm2_m > 1.5);
        assert!(st.deflection_mm < st.limit_mm);
        assert_ne!(st.light, Light::Red);

        assert_eq!(rep.beams.len(), 1);
        let bm = &rep.beams[0];
        assert!((bm.moment_k_neg_knm - 33.03).abs() < 1e-4);
        assert!((bm.moment_d_neg_knm - 46.242).abs() < 1e-3);
        assert!((bm.shear_k_kn - 28.70).abs() < 1e-4);
        assert!((bm.limit_mm - 14.4).abs() < 1e-6); // 1800 / 125 = 14.4 mm
        assert!(bm.back_span_ok);
        assert_ne!(bm.light, Light::Red);
    }
}
