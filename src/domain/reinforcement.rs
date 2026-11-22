//! Preliminary reinforcement for the concrete structure (#110).
//!
//! The module turns the forces already computed by `structure` into a reviewable
//! reinforcement proposal and a steel schedule. It deliberately stays inside
//! the same applicability limits as structural pre-sizing. The result is not a
//! fabrication drawing: anchorage, laps, openings, construction joints and the
//! 2026 revision of NBR 6118 still require a licensed engineer's review.

use std::f64::consts::PI;

use super::{
    fixtures::FixtureKind,
    limits,
    model::Project,
    structure::{FootingResult, SlabKind, StructureReport, effective_beam_stiffness},
};

pub const STEEL_DENSITY_KG_M3: f64 = 7_850.0;
pub const STEEL_FYK_MPA: f64 = 500.0;
pub const COMMERCIAL_BAR_LENGTH_M: f64 = 12.0;
pub const STEEL_WASTE_FACTOR: f64 = 1.10;

const LONGITUDINAL_DIAMETERS_MM: [f64; 5] = [10.0, 12.5, 16.0, 20.0, 25.0];
const MESH_DIAMETERS_MM: [f64; 4] = [6.3, 8.0, 10.0, 12.5];
const STIRRUP_DIAMETERS_MM: [f64; 3] = [6.3, 8.0, 10.0];
const STANDARD_SPACINGS_CM: [f64; 6] = [20.0, 15.0, 12.5, 10.0, 7.5, 5.0];

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
/// in metres (NBR 6118:2023 §9.4.2.5), where `α1 = 0.7` for hooked bars and
/// `1.0` for straight bars.
pub fn required_anchorage_m(
    diameter_mm: f64,
    fck_mpa: f64,
    zone: BondZone,
    hooked: bool,
    as_calc_cm2: f64,
    as_ef_cm2: f64,
) -> f64 {
    let phi_m = (diameter_mm / 1000.0).max(0.005);
    let lb = basic_anchorage_m(diameter_mm, fck_mpa, zone);
    let alpha1 = if hooked { 0.7 } else { 1.0 };
    let ratio = if as_ef_cm2 > 1e-9 {
        (as_calc_cm2 / as_ef_cm2).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let lb_min = (0.3 * lb).max(10.0 * phi_m).max(0.10);
    (alpha1 * lb * ratio).max(lb_min)
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

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn diameter_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        decimal(value, 1)
    }
}

fn rho_min(fck_mpa: f64) -> f64 {
    if fck_mpa <= 30.0 {
        0.00150
    } else if fck_mpa <= 35.0 {
        0.00173
    } else if fck_mpa <= 40.0 {
        0.00208
    } else if fck_mpa <= 45.0 {
        0.00245
    } else {
        0.00284
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlexuralDemand {
    required_cm2: f64,
    domain_ok: bool,
}

/// Singly reinforced rectangular section at ULS. `moment_k_knm` is the
/// characteristic moment; the same 1.4 load factor and CA-50 design strength
/// used by the existing beam-stiffness calculation are applied here.
fn flexural_demand(
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

#[derive(Clone, Debug, PartialEq)]
pub struct BarSet {
    pub count: u32,
    pub diameter_mm: f64,
    pub required_cm2: f64,
    pub provided_cm2: f64,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
    pub fits_section: bool,
}

impl BarSet {
    pub fn notation(&self) -> String {
        format!("{} Ø{}", self.count, diameter_text(self.diameter_mm))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BarChoice {
    count: u32,
    diameter_mm: f64,
    provided_cm2: f64,
    fits: bool,
}

fn choose_beam_bars(required_cm2: f64, width_m: f64, cover_m: f64) -> BarChoice {
    let stirrup_m = STIRRUP_DIAMETERS_MM[0] / 1000.0;
    let available = (width_m - 2.0 * (cover_m + stirrup_m)).max(0.0);
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
        let fits = clear + 1e-9 >= 0.020_f64.max(diameter_m);
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

fn choose_column_bars(required_cm2: f64) -> BarChoice {
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

fn bar_set(choice: BarChoice, required_cm2: f64, length_each_m: f64) -> BarSet {
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

#[derive(Clone, Debug, PartialEq)]
pub struct TransverseBars {
    pub diameter_mm: f64,
    pub spacing_cm: f64,
    pub count: u32,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
    pub shear_capacity_ok: bool,
}

impl TransverseBars {
    pub fn notation(&self) -> String {
        format!(
            "Ø{} c/{} cm",
            diameter_text(self.diameter_mm),
            decimal(
                self.spacing_cm,
                if self.spacing_cm.fract() == 0.0 { 0 } else { 1 }
            )
        )
    }
}

fn beam_stirrups(
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
    'diameters: for diameter in STIRRUP_DIAMETERS_MM {
        let two_legs_m2 = 2.0 * bar_area_cm2(diameter) / 10_000.0;
        let theoretical = (two_legs_m2 / needed_m2_m.max(1e-12)).min(maximum_spacing_m);
        for spacing_cm in STANDARD_SPACINGS_CM {
            let spacing_m = spacing_cm / 100.0;
            if spacing_m <= theoretical + 1e-9 {
                selected = (diameter, spacing_m);
                break 'diameters;
            }
        }
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct MeshBars {
    pub diameter_mm: f64,
    pub spacing_cm: f64,
    pub required_cm2_m: f64,
    pub provided_cm2_m: f64,
    pub count: u32,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
}

impl MeshBars {
    pub fn notation(&self) -> String {
        format!(
            "Ø{} c/{} cm",
            diameter_text(self.diameter_mm),
            decimal(
                self.spacing_cm,
                if self.spacing_cm.fract() == 0.0 { 0 } else { 1 }
            )
        )
    }
}

fn choose_mesh(required_cm2_m: f64, transverse_span_m: f64, bar_length_m: f64) -> MeshBars {
    let mut best: Option<(f64, f64, f64)> = None;
    for diameter in MESH_DIAMETERS_MM {
        let area = bar_area_cm2(diameter);
        for spacing_cm in STANDARD_SPACINGS_CM {
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

#[derive(Clone, Debug, PartialEq)]
pub struct BeamReinforcement {
    pub beam_id: u64,
    pub label: String,
    pub span_index: usize,
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    pub bottom: BarSet,
    pub top_left: BarSet,
    pub top_right: BarSet,
    pub stirrups: TransverseBars,
    /// Required hooked anchorage length (metres) for bottom (good bond) and
    /// top (poor bond when h ≥ 30 cm) longitudinal bars (NBR 6118 §9.4.2.5).
    pub bottom_anchorage_m: f64,
    pub top_anchorage_m: f64,
    pub flexural_domain_ok: bool,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnReinforcement {
    pub column_id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    pub longitudinal: BarSet,
    pub ties: TransverseBars,
    /// Preliminary lap/starter splice length `l_0` in metres (NBR 6118 §9.5.2).
    pub lap_length_m: f64,
    pub steel_ratio: f64,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlabReinforcement {
    pub slab_id: u64,
    pub label: String,
    pub room: String,
    pub kind: SlabKind,
    pub main: MeshBars,
    pub distribution: MeshBars,
    pub domain_ok: bool,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootingReinforcement {
    pub footing_id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    pub bars_x: MeshBars,
    pub bars_y: MeshBars,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SteelScheduleRow {
    pub mark: String,
    pub element: String,
    pub position: String,
    pub shape: String,
    pub diameter_mm: f64,
    pub quantity: u32,
    pub unit_length_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
}

/// Consolidated bar-schedule summary grouped by commercial CA-50 diameter.
#[derive(Clone, Debug, PartialEq)]
pub struct SteelDiameterSummary {
    pub diameter_mm: f64,
    pub total_length_m: f64,
    pub bars_12m: u32,
    pub mass_kg: f64,
    pub mass_with_waste_kg: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReinforcementReport {
    pub beams: Vec<BeamReinforcement>,
    pub columns: Vec<ColumnReinforcement>,
    pub slabs: Vec<SlabReinforcement>,
    pub footings: Vec<FootingReinforcement>,
    pub schedule: Vec<SteelScheduleRow>,
    pub by_diameter: Vec<SteelDiameterSummary>,
    pub warnings: Vec<String>,
}

impl ReinforcementReport {
    pub fn total_mass_kg(&self) -> f64 {
        self.schedule.iter().map(|row| row.mass_kg).sum()
    }

    pub fn total_mass_with_waste_kg(&self) -> f64 {
        self.total_mass_kg() * STEEL_WASTE_FACTOR
    }

    pub fn total_bars_12m(&self) -> u32 {
        self.by_diameter.iter().map(|row| row.bars_12m).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.beams.is_empty()
            && self.columns.is_empty()
            && self.slabs.is_empty()
            && self.footings.is_empty()
    }
}

fn schedule_bar_set(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &BarSet,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn schedule_transverse(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &TransverseBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn schedule_mesh(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &MeshBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn summarize_by_diameter(schedule: &[SteelScheduleRow]) -> Vec<SteelDiameterSummary> {
    let mut totals: Vec<(i32, f64, f64, f64)> = Vec::new();
    for row in schedule {
        let key = (row.diameter_mm * 10.0).round() as i32;
        if let Some(entry) = totals.iter_mut().find(|(k, _, _, _)| *k == key) {
            entry.2 += row.total_length_m;
            entry.3 += row.mass_kg;
        } else {
            totals.push((key, row.diameter_mm, row.total_length_m, row.mass_kg));
        }
    }
    totals.sort_by_key(|(key, _, _, _)| *key);
    totals
        .into_iter()
        .map(
            |(_, diameter_mm, total_length_m, mass_kg)| SteelDiameterSummary {
                diameter_mm,
                total_length_m,
                bars_12m: ((total_length_m * STEEL_WASTE_FACTOR) / COMMERCIAL_BAR_LENGTH_M).ceil()
                    as u32,
                mass_kg,
                mass_with_waste_kg: mass_kg * STEEL_WASTE_FACTOR,
            },
        )
        .collect()
}

fn footing_column_dimensions(project: &Project, footing: &FootingResult) -> Option<(f64, f64)> {
    let footing_fixture = project
        .fixtures
        .iter()
        .find(|fixture| fixture.id == footing.id && fixture.kind == FixtureKind::Footing)?;
    let column = project
        .structural_columns()
        .find(|column| footing_fixture.contains(column.center))?;
    if column.quarter_turns % 2 == footing_fixture.quarter_turns % 2 {
        Some((column.width, column.depth))
    } else {
        Some((column.depth, column.width))
    }
}

/// Builds the reinforcement proposal from the structural analysis results.
pub fn build_reinforcement_report(
    project: &Project,
    structure: &StructureReport,
) -> ReinforcementReport {
    let mut out = ReinforcementReport::default();
    if let Some(refusal) = limits::structural_refusal(project) {
        out.warnings.push(refusal);
        return out;
    }
    let fck = project.structure.settings.fck_mpa;
    let modulus = project.structure.settings.modulus_kn_m2();

    for line in &structure.beams {
        for (index, span) in line.spans.iter().enumerate() {
            if span.length_m > limits::MAX_BEAM_SPAN_M + 1e-9 {
                out.warnings.push(format!(
                    "{} vão {}: armadura recusada para vão acima de {:.1} m.",
                    line.label,
                    index + 1,
                    limits::MAX_BEAM_SPAN_M
                ));
                continue;
            }
            // Effective depth uses d' = 4 cm (c_nom + stirrup + bar/2), while
            // bar-fit uses nominal cover c_nom = 3 cm (CAA II, NBR 6118 Table 7.2),
            // matching `beam_stirrups`.
            let d_prime = 0.04;
            let nominal_cover = 0.03;
            let bottom_demand =
                flexural_demand(span.width_m, span.height_m, d_prime, span.moment_span, fck);
            let left_demand = flexural_demand(
                span.width_m,
                span.height_m,
                d_prime,
                span.moment_left.abs(),
                fck,
            );
            let right_demand = flexural_demand(
                span.width_m,
                span.height_m,
                d_prime,
                span.moment_right.abs(),
                fck,
            );
            // Keep the positive steel aligned with the steel already used for
            // Branson stiffness; both paths must describe the same section.
            let stiffness = effective_beam_stiffness(
                span.width_m,
                span.height_m,
                fck,
                modulus,
                span.moment_span,
                span.moment_span,
            );
            let bottom_required = bottom_demand.required_cm2.max(stiffness.as_cm2);
            let bottom_choice = choose_beam_bars(bottom_required, span.width_m, nominal_cover);
            let left_choice =
                choose_beam_bars(left_demand.required_cm2, span.width_m, nominal_cover);
            let right_choice =
                choose_beam_bars(right_demand.required_cm2, span.width_m, nominal_cover);
            let bottom_anchorage_m = required_anchorage_m(
                bottom_choice.diameter_mm,
                fck,
                BondZone::Good,
                true,
                bottom_required,
                bottom_choice.provided_cm2,
            );
            let top_zone = if span.height_m >= 0.30 {
                BondZone::Poor
            } else {
                BondZone::Good
            };
            let left_anchorage_m = required_anchorage_m(
                left_choice.diameter_mm,
                fck,
                top_zone,
                true,
                left_demand.required_cm2,
                left_choice.provided_cm2,
            );
            let right_anchorage_m = required_anchorage_m(
                right_choice.diameter_mm,
                fck,
                top_zone,
                true,
                right_demand.required_cm2,
                right_choice.provided_cm2,
            );
            let top_anchorage_m = left_anchorage_m.max(right_anchorage_m);
            let bottom = bar_set(
                bottom_choice,
                bottom_required,
                span.length_m + 2.0 * bottom_anchorage_m,
            );
            let support_length = (0.30 * span.length_m).max(0.60);
            let top_left = bar_set(
                left_choice,
                left_demand.required_cm2,
                support_length + left_anchorage_m,
            );
            let top_right = bar_set(
                right_choice,
                right_demand.required_cm2,
                support_length + right_anchorage_m,
            );
            let stirrups =
                beam_stirrups(span.length_m, span.width_m, span.height_m, span.shear, fck);
            let label = if line.spans.len() > 1 {
                format!("{} v{}", line.label, index + 1)
            } else {
                line.label.clone()
            };
            let flexural_domain_ok =
                bottom_demand.domain_ok && left_demand.domain_ok && right_demand.domain_ok;
            if !flexural_domain_ok {
                out.warnings.push(format!(
                    "{label}: momento acima do domínio de armadura simples; aumente a seção e revise com engenheiro."
                ));
            }
            if !bottom.fits_section || !top_left.fits_section || !top_right.fits_section {
                out.warnings.push(format!(
                    "{label}: as barras não cabem em uma camada com cobrimento nominal de 3 cm (CAA II); aumente a largura ou detalhe em camadas."
                ));
            }
            if !stirrups.shear_capacity_ok {
                out.warnings.push(format!(
                    "{label}: compressão diagonal excedida; estribos sozinhos não resolvem, aumente a seção."
                ));
            }
            let mass_kg = bottom.mass_kg + top_left.mass_kg + top_right.mass_kg + stirrups.mass_kg;
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-INF"),
                &label,
                "longitudinal inferior",
                "reta + 2 ganchos 90°",
                &bottom,
            );
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-SUP-E"),
                &label,
                "superior apoio esquerdo",
                "reta + gancho 90°",
                &top_left,
            );
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-SUP-D"),
                &label,
                "superior apoio direito",
                "reta + gancho 90°",
                &top_right,
            );
            schedule_transverse(
                &mut out.schedule,
                format!("{label}-E"),
                &label,
                "estribos",
                "estribo fechado",
                &stirrups,
            );
            out.beams.push(BeamReinforcement {
                beam_id: span.beam,
                label,
                span_index: index + 1,
                width_m: span.width_m,
                height_m: span.height_m,
                length_m: span.length_m,
                bottom,
                top_left,
                top_right,
                bottom_anchorage_m,
                top_anchorage_m,
                stirrups,
                flexural_domain_ok,
                mass_kg,
            });
        }
    }

    for column in &structure.columns {
        let ac_cm2 = column.area_cm2;
        let fcd_kn_cm2 = fck / 1.4 / 10.0;
        let fyd_kn_cm2 = STEEL_FYK_MPA / 1.15 / 10.0;
        let nsd = 1.4 * column.position.factor() * column.load_kn;
        let steel_by_load = ((nsd - 0.85 * fcd_kn_cm2 * ac_cm2) / fyd_kn_cm2).max(0.0);
        let required = steel_by_load.max(0.004 * ac_cm2);
        let choice = choose_column_bars(required);
        let lap_length_m = lap_splice_m(
            choice.diameter_mm,
            fck,
            BondZone::Good,
            1.4,
            required,
            choice.provided_cm2,
        )
        .max(0.40);
        // Bars as long as the column (#145: the tallest wall it stands in).
        let longitudinal = bar_set(choice, required, column.height_m + lap_length_m);
        let tie_spacing_m = 0.20_f64
            .min(12.0 * choice.diameter_mm / 1000.0)
            .min(column.width_m.min(column.depth_m))
            .max(0.05);
        let spacing_cm = (tie_spacing_m * 100.0 / 5.0).floor().max(1.0) * 5.0;
        let tie_count = (column.height_m / (spacing_cm / 100.0)).ceil() as u32 + 1;
        let tie_diameter = STIRRUP_DIAMETERS_MM[0];
        let core_b = (column.width_m - 0.06).max(0.04);
        let core_h = (column.depth_m - 0.06).max(0.04);
        let tie_length = 2.0 * (core_b + core_h) + 2.0 * 10.0 * tie_diameter / 1000.0;
        let tie_total = f64::from(tie_count) * tie_length;
        let ties = TransverseBars {
            diameter_mm: tie_diameter,
            spacing_cm,
            count: tie_count,
            length_each_m: tie_length,
            total_length_m: tie_total,
            mass_kg: tie_total * bar_mass_kg_m(tie_diameter),
            shear_capacity_ok: true,
        };
        let steel_ratio = longitudinal.provided_cm2 / ac_cm2.max(1e-9);
        if steel_ratio > 0.04 + 1e-9 {
            out.warnings.push(format!(
                "{}: taxa longitudinal de {:.1}% excede o limite de pré-detalhamento; aumente a seção.",
                column.label,
                steel_ratio * 100.0
            ));
        }
        let mass_kg = longitudinal.mass_kg + ties.mass_kg;
        schedule_bar_set(
            &mut out.schedule,
            format!("{}-L", column.label),
            &column.label,
            "longitudinal",
            "reta + traspasse",
            &longitudinal,
        );
        schedule_transverse(
            &mut out.schedule,
            format!("{}-T", column.label),
            &column.label,
            "estribos",
            "estribo fechado",
            &ties,
        );
        out.columns.push(ColumnReinforcement {
            column_id: column.id,
            label: column.label.clone(),
            width_m: column.width_m,
            depth_m: column.depth_m,
            height_m: column.height_m,
            longitudinal,
            lap_length_m,
            ties,
            steel_ratio,
            mass_kg,
        });
    }

    for slab in &structure.slabs {
        if slab.lx_m > limits::MAX_SLAB_SPAN_M + 1e-9 || !slab.supported {
            out.warnings.push(format!(
                "{} ({}): armadura não emitida porque a laje está fora dos limites ou sem apoio.",
                slab.label, slab.room
            ));
            continue;
        }
        let characteristic_moment = if slab.kind == SlabKind::Solid {
            slab.moment_x_knm_m
        } else {
            slab.load_kn_m2 * slab.lx_m.powi(2) / 8.0 + slab.point_moment_knm_m
        };
        let demand_x = flexural_demand(1.0, slab.thickness_m, 0.025, characteristic_moment, fck);
        let main_length = slab.lx_m + 0.30;
        let main = choose_mesh(demand_x.required_cm2, slab.ly_m, main_length);
        let demand_y = flexural_demand(1.0, slab.thickness_m, 0.025, slab.moment_y_knm_m, fck);
        let distribution_required = if slab.kind == SlabKind::Solid {
            demand_y.required_cm2.max(0.90)
        } else {
            (0.20 * main.required_cm2_m).max(0.90)
        };
        let distribution = choose_mesh(distribution_required, slab.lx_m, slab.ly_m + 0.30);
        if slab.kind == SlabKind::Ribbed {
            out.warnings.push(format!(
                "{} ({}): aço principal é uma estimativa equivalente por metro; confirme a vigota/treliça com o fabricante.",
                slab.label, slab.room
            ));
        }
        let domain_ok = demand_x.domain_ok && (slab.kind != SlabKind::Solid || demand_y.domain_ok);
        if !domain_ok {
            out.warnings.push(format!(
                "{} ({}): seção insuficiente para armadura simples; aumente a espessura.",
                slab.label, slab.room
            ));
        }
        let mass_kg = main.mass_kg + distribution.mass_kg;
        schedule_mesh(
            &mut out.schedule,
            format!("{}-P", slab.label),
            &slab.label,
            "direção x (menor vão)",
            if slab.kind == SlabKind::Solid {
                "malha reta"
            } else {
                "equivalente vigota"
            },
            &main,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-D", slab.label),
            &slab.label,
            if slab.kind == SlabKind::Solid {
                "direção y (maior vão)"
            } else {
                "distribuição"
            },
            "malha reta",
            &distribution,
        );
        out.slabs.push(SlabReinforcement {
            slab_id: slab.id,
            label: slab.label.clone(),
            room: slab.room.clone(),
            kind: slab.kind,
            main,
            distribution,
            domain_ok,
            mass_kg,
        });
    }

    for footing in &structure.footings {
        if footing.soil_kpa < limits::MIN_SOIL_KPA - 1e-9 || footing.column.is_none() {
            out.warnings.push(format!(
                "{}: armadura não emitida sem solo aplicável e pilar associado.",
                footing.label
            ));
            continue;
        }
        let Some((column_width, column_depth)) = footing_column_dimensions(project, footing) else {
            out.warnings.push(format!(
                "{}: não foi possível identificar a seção do pilar.",
                footing.label
            ));
            continue;
        };
        let cantilever_x = ((footing.width_m - column_width) * 0.5).max(0.0);
        let cantilever_y = ((footing.depth_m - column_depth) * 0.5).max(0.0);
        let q = footing.max_pressure_kpa;
        let moment_x = q * cantilever_x.powi(2) / 2.0;
        let moment_y = q * cantilever_y.powi(2) / 2.0;
        let demand_x = flexural_demand(1.0, footing.height_m, 0.07, moment_x, fck);
        let demand_y = flexural_demand(1.0, footing.height_m, 0.07, moment_y, fck);
        let hook_return = (footing.height_m - 0.14).clamp(0.10, 0.25);
        let bars_x = choose_mesh(
            demand_x.required_cm2,
            (footing.depth_m - 0.14).max(0.20),
            (footing.width_m - 0.14).max(0.30) + 2.0 * hook_return,
        );
        let bars_y = choose_mesh(
            demand_y.required_cm2,
            (footing.width_m - 0.14).max(0.20),
            (footing.depth_m - 0.14).max(0.30) + 2.0 * hook_return,
        );
        let mass_kg = bars_x.mass_kg + bars_y.mass_kg;
        schedule_mesh(
            &mut out.schedule,
            format!("{}-X", footing.label),
            &footing.label,
            "malha inferior X",
            "malha c/ dobras 90°",
            &bars_x,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-Y", footing.label),
            &footing.label,
            "malha inferior Y",
            "malha c/ dobras 90°",
            &bars_y,
        );
        out.footings.push(FootingReinforcement {
            footing_id: footing.id,
            label: footing.label.clone(),
            width_m: footing.width_m,
            depth_m: footing.depth_m,
            height_m: footing.height_m,
            bars_x,
            bars_y,
            mass_kg,
        });
    }

    out.by_diameter = summarize_by_diameter(&out.schedule);
    out
}

pub fn steel_schedule_csv(report: &ReinforcementReport) -> String {
    let mut csv = String::from(
        "Marca;Elemento;Posição;Forma;Diâmetro (mm);Quantidade;Comprimento unitário (m);Comprimento total (m);Massa (kg)\n",
    );
    for row in &report.schedule {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{};{}\n",
            row.mark,
            row.element,
            row.position,
            row.shape,
            diameter_text(row.diameter_mm),
            row.quantity,
            decimal(row.unit_length_m, 2),
            decimal(row.total_length_m, 2),
            decimal(row.mass_kg, 1)
        ));
    }
    csv.push_str(&format!(
        ";;;;;;;Massa total teórica (kg);{}\n",
        decimal(report.total_mass_kg(), 1)
    ));
    csv.push_str(&format!(
        ";;;;;;;Massa total +10% perda (kg);{}\n",
        decimal(report.total_mass_with_waste_kg(), 1)
    ));
    csv.push_str(&format!(
        ";;;;;;;Total barras comerciais 12 m;{}\n",
        report.total_bars_12m()
    ));
    if !report.by_diameter.is_empty() {
        csv.push_str("\nResumo por bitola (CA-50)\n");
        csv.push_str(
            "Bitola (mm);Comprimento total (m);Barras de 12 m (+10%);Massa teórica (kg);Massa +10% perda (kg)\n",
        );
        for row in &report.by_diameter {
            csv.push_str(&format!(
                "{};{};{};{};{}\n",
                diameter_text(row.diameter_mm),
                decimal(row.total_length_m, 2),
                row.bars_12m,
                decimal(row.mass_kg, 1),
                decimal(row.mass_with_waste_kg, 1)
            ));
        }
    }
    csv.push_str(
        "\nCaráter;Estimativa preliminar NBR 6118:2023 (B12 Branson, ancoragem §9.4 e traspasse §9.5) - auditoria da edição 2026 e revisão por engenheiro habilitado obrigatórias\n",
    );
    csv
}

fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Renders an A3 landscape schematic reinforcement detailing sheet (SVG)
/// illustrating the governing beam, column, footing, slab summary, and
/// CA-50 steel summary by diameter.
pub fn reinforcement_detail_svg(
    project: &Project,
    structure: &StructureReport,
    report: &ReinforcementReport,
) -> String {
    use std::fmt::Write as _;

    let fck = project.structure.settings.fck_mpa;
    let project_name = if project.name.trim().is_empty() {
        "Projeto sem título"
    } else {
        project.name.trim()
    };

    let mut svg = String::with_capacity(16_384);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="297mm" viewBox="0 0 420 297">"#
    );
    let _ = writeln!(
        svg,
        r#"  <style>
    .bg {{ fill: #ffffff; }}
    .frame {{ fill: none; stroke: #1f2937; stroke-width: 0.45; }}
    .panel {{ fill: #fafbfc; stroke: #cbd5e1; stroke-width: 0.35; rx: 1.5; }}
    .panel-hdr {{ fill: #1e293b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 4.1px; font-weight: 700; }}
    .title {{ fill: #0f172a; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 6.2px; font-weight: 700; }}
    .sub {{ fill: #475569; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.4px; }}
    .lbl {{ fill: #1e293b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.1px; }}
    .lbl-b {{ fill: #0f172a; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.2px; font-weight: 700; }}
    .dim {{ fill: #334155; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 2.8px; }}
    .muted {{ fill: #64748b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 2.7px; }}
    .conc {{ fill: #e2e8f0; stroke: #334155; stroke-width: 0.4; }}
    .stirrup {{ fill: none; stroke: #0284c7; stroke-width: 0.45; }}
    .bar-pos {{ fill: none; stroke: #dc2626; stroke-width: 0.7; stroke-linecap: round; stroke-linejoin: round; }}
    .bar-neg {{ fill: none; stroke: #b45309; stroke-width: 0.65; stroke-linecap: round; stroke-linejoin: round; }}
    .bar-dot {{ fill: #dc2626; stroke: #7f1d1d; stroke-width: 0.2; }}
    .bar-dot-top {{ fill: #b45309; stroke: #78350f; stroke-width: 0.2; }}
    .dim-line {{ stroke: #64748b; stroke-width: 0.25; }}
    .tbl-hdr {{ fill: #e2e8f0; }}
    .tbl-row {{ fill: #ffffff; }}
    .tbl-alt {{ fill: #f8fafc; }}
    .tbl-tot {{ fill: #e0f2fe; }}
    .tbl-grid {{ stroke: #cbd5e1; stroke-width: 0.22; }}
  </style>"#
    );

    let _ = writeln!(
        svg,
        r#"  <rect class="bg" x="0" y="0" width="420" height="297"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <rect class="frame" x="8" y="8" width="404" height="281"/>"#
    );

    // Header band
    let _ = writeln!(
        svg,
        r#"  <text class="title" x="14" y="18.5">DETALHAMENTO ESQUEMÁTICO DE ARMADURAS — PRÉ-DIMENSIONAMENTO</text>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="sub" x="14" y="24.5">Projeto: {}  ·  Concreto C{:.0} (fck = {:.0} MPa)  ·  Aço CA-50  ·  ABNT NBR 6118:2023</text>"#,
        escape_xml(project_name),
        fck,
        fck
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="406" y="18.5" text-anchor="end">Aço teórico: {} kg  |  Com +10%: {} kg ({} barras de 12 m)</text>"#,
        decimal(report.total_mass_kg(), 1),
        decimal(report.total_mass_with_waste_kg(), 1),
        report.total_bars_12m()
    );
    let _ = writeln!(
        svg,
        r#"  <text class="sub" x="406" y="24.5" text-anchor="end">{} vigas · {} pilares · {} lajes · {} sapatas · {} posições na tabela</text>"#,
        report.beams.len(),
        report.columns.len(),
        report.slabs.len(),
        report.footings.len(),
        report.schedule.len()
    );
    let _ = writeln!(
        svg,
        r##"  <line x1="12" y1="28.5" x2="408" y2="28.5" stroke="#94a3b8" stroke-width="0.35"/>"##
    );

    // -------------------------------------------------------------------------
    // Panel 1: Governing beam (x: 12..252, y: 33..159)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="12" y="33" width="240" height="126"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="17" y="40.5">1. VIGA DE REFERÊNCIA — SEÇÃO TRANSVERSAL E ELEVAÇÃO ESQUEMÁTICA</text>"#
    );
    if let Some(beam) = report
        .beams
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let span_match = structure.beams.iter().find_map(|line| {
            line.spans
                .iter()
                .enumerate()
                .find(|(idx, sp)| sp.beam == beam.beam_id && idx + 1 == beam.span_index)
                .map(|(_, sp)| sp)
        });
        let b_cm = beam.width_m * 100.0;
        let h_cm = beam.height_m * 100.0;
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="17" y="47">{} — seção {:.0} × {:.0} cm · vão L = {} m · massa = {} kg</text>"#,
            escape_xml(&beam.label),
            b_cm,
            h_cm,
            decimal(beam.length_m, 2),
            decimal(beam.mass_kg, 1)
        );

        // Cross section box (x: 24..72, y: 55..118)
        let max_w = 40.0_f64;
        let max_h = 56.0_f64;
        let scale = (max_w / beam.width_m.max(0.10)).min(max_h / beam.height_m.max(0.20));
        let sw = beam.width_m * scale;
        let sh = beam.height_m * scale;
        let sx = 46.0 - sw * 0.5;
        let sy = 56.0 + (max_h - sh) * 0.5;
        let cov = (0.03 * scale).clamp(2.0, sw * 0.25);

        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{sx:.1}" y="{sy:.1}" width="{sw:.1}" height="{sh:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="stirrup" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="1.2"/>"#,
            sx + cov,
            sy + cov,
            (sw - 2.0 * cov).max(2.0),
            (sh - 2.0 * cov).max(2.0)
        );
        // Draw bottom bar dots
        let draw_bar_row = |svg: &mut String, count: u32, y_pos: f64, cls: &str| {
            let n = count.clamp(2, 6);
            let inner_l = sx + cov + 1.5;
            let inner_r = sx + sw - cov - 1.5;
            for i in 0..n {
                let t = if n <= 1 {
                    0.5
                } else {
                    f64::from(i) / f64::from(n - 1)
                };
                let cx = inner_l + t * (inner_r - inner_l).max(0.0);
                let _ = writeln!(
                    svg,
                    r#"  <circle class="{cls}" cx="{cx:.1}" cy="{y_pos:.1}" r="1.25"/>"#
                );
            }
        };
        draw_bar_row(&mut svg, beam.bottom.count, sy + sh - cov - 1.4, "bar-dot");
        let top_count = beam.top_left.count.max(beam.top_right.count);
        draw_bar_row(&mut svg, top_count, sy + cov + 1.4, "bar-dot-top");

        // Dimensions for cross section
        let _ = writeln!(
            svg,
            r#"  <line class="dim-line" x1="{sx:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            sy + sh + 4.0,
            sx + sw,
            sy + sh + 4.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">b = {:.0} cm</text>"#,
            sx + sw * 0.5,
            sy + sh + 8.0,
            b_cm
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="end">h = {:.0} cm</text>"#,
            sx - 2.0,
            sy + sh * 0.5 + 1.0,
            h_cm
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="46" y="126" text-anchor="middle">c = 3 cm · estribos {}</text>"#,
            escape_xml(&beam.stirrups.notation())
        );

        // Longitudinal elevation (x: 84..242, y: 58..112)
        let ex = 94.0;
        let ew = 136.0;
        let ey = 64.0;
        let eh = 34.0;
        // Support pillars
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="10" height="{:.1}"/>"#,
            ex - 10.0,
            ey,
            eh + 12.0
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="10" height="{:.1}"/>"#,
            ex + ew,
            ey,
            eh + 12.0
        );
        // Beam elevation outline
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{ex:.1}" y="{ey:.1}" width="{ew:.1}" height="{eh:.1}"/>"#
        );
        // Stirrup vertical ticks
        for i in 1..17 {
            let tx = ex + ew * (f64::from(i) / 17.0);
            let _ = writeln!(
                svg,
                r#"  <line class="stirrup" x1="{tx:.1}" y1="{:.1}" x2="{tx:.1}" y2="{:.1}" stroke-opacity="0.55"/>"#,
                ey + 3.0,
                ey + eh - 3.0
            );
        }
        // Bottom positive bar with 90-degree hooks into supports
        let by = ey + eh - 4.0;
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-pos" points="{:.1},{:.1} {:.1},{by:.1} {:.1},{by:.1} {:.1},{:.1}"/>"#,
            ex - 6.5,
            by - 6.0,
            ex - 6.5,
            ex + ew + 6.5,
            ex + ew + 6.5,
            by - 6.0
        );
        // Top negative bars (left & right supports, 0.30*L + 90-degree hooks)
        let ty = ey + 4.0;
        let neg_len = ew * 0.30;
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-neg" points="{:.1},{:.1} {:.1},{ty:.1} {:.1},{ty:.1}"/>"#,
            ex - 6.5,
            ty + 6.0,
            ex - 6.5,
            ex + neg_len
        );
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-neg" points="{:.1},{ty:.1} {:.1},{ty:.1} {:.1},{:.1}"/>"#,
            ex + ew - neg_len,
            ex + ew + 6.5,
            ex + ew + 6.5,
            ty + 6.0
        );

        // Callouts on elevation
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">Superior esq.: {} · dir.: {} (lb,nec = {:.0} cm, zona {})</text>"#,
            ex + ew * 0.5,
            ey - 4.0,
            escape_xml(&beam.top_left.notation()),
            escape_xml(&beam.top_right.notation()),
            beam.top_anchorage_m * 100.0,
            if beam.height_m >= 0.30 { "má" } else { "boa" }
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">Inferior: {} · lb,nec = {:.0} cm (gancho 90°, boa aderência)</text>"#,
            ex + ew * 0.5,
            ey + eh + 8.5,
            escape_xml(&beam.bottom.notation()),
            beam.bottom_anchorage_m * 100.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">Vão L = {} m  ·  Negativo = 0,30·L ({} m) + ancoragem</text>"#,
            ex + ew * 0.5,
            ey + eh + 14.5,
            decimal(beam.length_m, 2),
            decimal((0.30 * beam.length_m).max(0.60), 2)
        );

        // Branson B12 cracked-section box
        let branson_line = if let Some(sp) = span_match {
            let stage = if sp.cracked {
                "Estádio II (fissurada)"
            } else {
                "Estádio I (não fissurada)"
            };
            format!(
                "Branson B12: {} · Mr = {} kN·m · I_eq/Ic = {:.0}% · As = {} cm² · flecha = {} mm (L/{:.0})",
                stage,
                decimal(sp.cracking_moment_knm, 1),
                sp.i_eq_ratio * 100.0,
                decimal(sp.branson_as_cm2, 2),
                decimal(sp.deflection_mm, 1),
                if sp.deflection_mm > 1e-6 {
                    sp.length_m * 1000.0 / sp.deflection_mm
                } else {
                    9999.0
                }
            )
        } else {
            format!(
                "Armadura inferior efetiva: {} cm² (req. {} cm²) · domínio {}",
                decimal(beam.bottom.provided_cm2, 2),
                decimal(beam.bottom.required_cm2, 2),
                if beam.flexural_domain_ok {
                    "OK"
                } else {
                    "EXCEDIDO"
                }
            )
        };
        let _ = writeln!(
            svg,
            r##"  <rect x="17" y="138" width="230" height="15" fill="#f1f5f9" stroke="#cbd5e1" stroke-width="0.25" rx="1"/>"##
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="21" y="144.2">{}</text>"#,
            escape_xml(&branson_line)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="21" y="149.8">Estribos: {} ({} un., compr. unit. {} m) · Verificação de biela comprimida: {}</text>"#,
            escape_xml(&beam.stirrups.notation()),
            beam.stirrups.count,
            decimal(beam.stirrups.length_each_m, 2),
            if beam.stirrups.shear_capacity_ok {
                "OK"
            } else {
                "EXCEDIDA"
            }
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="54">Nenhuma viga armada neste projeto.</text>"#
        );
    }

    // -------------------------------------------------------------------------
    // Panel 2: Governing column (x: 258..408, y: 33..159)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="258" y="33" width="150" height="126"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="263" y="40.5">2. PILAR DE REFERÊNCIA — SEÇÃO E TRASPASSE</text>"#
    );
    if let Some(col) = report
        .columns
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="263" y="47">{} — {:.0} × {:.0} cm · H = {} m · ρ = {}%</text>"#,
            escape_xml(&col.label),
            col.width_m * 100.0,
            col.depth_m * 100.0,
            decimal(col.height_m, 2),
            decimal(col.steel_ratio * 100.0, 2)
        );
        // Column cross-section (centered around x=292, y=82)
        let scale = (36.0 / col.width_m.max(0.14)).min(36.0 / col.depth_m.max(0.14));
        let cw = col.width_m * scale;
        let ch = col.depth_m * scale;
        let cx = 292.0 - cw * 0.5;
        let cy = 80.0 - ch * 0.5;
        let cov = (0.03 * scale).clamp(2.2, cw.min(ch) * 0.25);
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{cx:.1}" y="{cy:.1}" width="{cw:.1}" height="{ch:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="stirrup" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="1.1"/>"#,
            cx + cov,
            cy + cov,
            (cw - 2.0 * cov).max(2.0),
            (ch - 2.0 * cov).max(2.0)
        );
        // Place corner + side bars
        let x0 = cx + cov + 1.3;
        let x1 = cx + cw - cov - 1.3;
        let y0 = cy + cov + 1.3;
        let y1 = cy + ch - cov - 1.3;
        let mut pts = vec![(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
        if col.longitudinal.count >= 6 {
            pts.push(((x0 + x1) * 0.5, y0));
            pts.push(((x0 + x1) * 0.5, y1));
        }
        if col.longitudinal.count >= 8 {
            pts.push((x0, (y0 + y1) * 0.5));
            pts.push((x1, (y0 + y1) * 0.5));
        }
        for (px, py) in pts {
            let _ = writeln!(
                svg,
                r#"  <circle class="bar-dot" cx="{px:.1}" cy="{py:.1}" r="1.3"/>"#
            );
        }
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">{:.0} × {:.0} cm</text>"#,
            cx + cw * 0.5,
            cy + ch + 6.0,
            col.width_m * 100.0,
            col.depth_m * 100.0
        );

        // Column elevation with lap splice (x: 342..366, y: 58..126)
        let elx = 346.0;
        let elw = 18.0;
        let ely = 62.0;
        let elh = 62.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{elx:.1}" y="{ely:.1}" width="{elw:.1}" height="{elh:.1}"/>"#
        );
        // Ties along elevation
        for i in 1..10 {
            let ty = ely + elh * (f64::from(i) / 10.0);
            let _ = writeln!(
                svg,
                r#"  <line class="stirrup" x1="{:.1}" y1="{ty:.1}" x2="{:.1}" y2="{ty:.1}"/>"#,
                elx + 2.0,
                elx + elw - 2.0
            );
        }
        // Longitudinal bars + starter lap zone extending above
        let lap_h = 13.0;
        let _ = writeln!(
            svg,
            r#"  <line class="bar-pos" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            elx + 3.2,
            ely - lap_h,
            elx + 3.2,
            ely + elh - 2.0
        );
        let _ = writeln!(
            svg,
            r#"  <line class="bar-pos" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            elx + elw - 3.2,
            ely - lap_h,
            elx + elw - 3.2,
            ely + elh - 2.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}">l0 = {:.0} cm</text>"#,
            elx + elw + 3.5,
            ely - 4.0,
            col.lap_length_m * 100.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}">H = {} m</text>"#,
            elx + elw + 3.5,
            ely + elh * 0.5,
            decimal(col.height_m, 2)
        );
        let _ = writeln!(
            svg,
            r##"  <rect x="263" y="138" width="140" height="15" fill="#f1f5f9" stroke="#cbd5e1" stroke-width="0.25" rx="1"/>"##
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="267" y="144.2">Longitudinal: {} (As = {} cm²) · l0 = {:.0} cm</text>"#,
            escape_xml(&col.longitudinal.notation()),
            decimal(col.longitudinal.provided_cm2, 2),
            col.lap_length_m * 100.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="267" y="149.8">Estribos: {} ({} un.) · massa total = {} kg</text>"#,
            escape_xml(&col.ties.notation()),
            col.ties.count,
            decimal(col.mass_kg, 1)
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="263" y="54">Nenhum pilar armado neste projeto.</text>"#
        );
    }

    // -------------------------------------------------------------------------
    // Panel 3: Footing + slabs summary (x: 12..204, y: 164..274)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="12" y="164" width="192" height="110"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="17" y="171.5">3. SAPATA DE REFERÊNCIA E RESUMO DE LAJES</text>"#
    );
    if let Some(ft) = report
        .footings
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="17" y="178">{} — {:.0} × {:.0} × {:.0} cm · cobrimento c = 7 cm · massa = {} kg</text>"#,
            escape_xml(&ft.label),
            ft.width_m * 100.0,
            ft.depth_m * 100.0,
            ft.height_m * 100.0,
            decimal(ft.mass_kg, 1)
        );
        // Footing plan view (left)
        let fx = 24.0;
        let fy = 185.0;
        let fw = 44.0;
        let fh = 38.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{fx:.1}" y="{fy:.1}" width="{fw:.1}" height="{fh:.1}"/>"#
        );
        // Column footprint in center
        let _ = writeln!(
            svg,
            r##"  <rect x="{:.1}" y="{:.1}" width="10" height="10" fill="#94a3b8" stroke="#334155" stroke-width="0.3"/>"##,
            fx + fw * 0.5 - 5.0,
            fy + fh * 0.5 - 5.0
        );
        // Bottom mesh grid lines
        for i in 1..6 {
            let x = fx + fw * (f64::from(i) / 6.0);
            let y = fy + fh * (f64::from(i) / 6.0);
            let _ = writeln!(
                svg,
                r#"  <line class="bar-pos" x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{:.1}" stroke-width="0.35"/>"#,
                fy + 3.0,
                fy + fh - 3.0
            );
            let _ = writeln!(
                svg,
                r#"  <line class="bar-neg" x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke-width="0.35"/>"#,
                fx + 3.0,
                fx + fw - 3.0
            );
        }
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">Planta ({:.0}×{:.0} cm)</text>"#,
            fx + fw * 0.5,
            fy + fh + 5.5,
            ft.width_m * 100.0,
            ft.depth_m * 100.0
        );

        // Footing cross-section (right)
        let sx = 92.0;
        let sy = 197.0;
        let sw = 94.0;
        let sh = 24.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{sx:.1}" y="{sy:.1}" width="{sw:.1}" height="{sh:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="14" height="12"/>"#,
            sx + sw * 0.5 - 7.0,
            sy - 12.0
        );
        // Bottom mesh bar with upturned 90-degree hooks
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-pos" points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}"/>"#,
            sx + 4.0,
            sy + 6.0,
            sx + 4.0,
            sy + sh - 4.0,
            sx + sw - 4.0,
            sy + sh - 4.0,
            sx + sw - 4.0,
            sy + 6.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">Malha X: {} · Malha Y: {}</text>"#,
            sx + sw * 0.5,
            sy + sh + 6.5,
            escape_xml(&ft.bars_x.notation()),
            escape_xml(&ft.bars_y.notation())
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="185">Nenhuma sapata armada neste projeto.</text>"#
        );
    }

    // Slabs summary strip at bottom of panel 3
    let _ = writeln!(
        svg,
        r##"  <line x1="17" y1="236" x2="199" y2="236" stroke="#cbd5e1" stroke-width="0.25"/>"##
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="17" y="242.5">Lajes armadas ({})</text>"#,
        report.slabs.len()
    );
    if report.slabs.is_empty() {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="249">Nenhuma laje armada neste projeto.</text>"#
        );
    } else {
        for (idx, slab) in report.slabs.iter().take(4).enumerate() {
            let y = 249.0 + (idx as f64) * 5.6;
            let kind = match slab.kind {
                SlabKind::Solid => "maciça",
                SlabKind::Ribbed => "treliçada (equiv.)",
            };
            let _ = writeln!(
                svg,
                r#"  <text class="lbl" x="17" y="{y:.1}">{} ({}) — {}: X {} · Y {} ({} kg)</text>"#,
                escape_xml(&slab.label),
                escape_xml(&slab.room),
                kind,
                escape_xml(&slab.main.notation()),
                escape_xml(&slab.distribution.notation()),
                decimal(slab.mass_kg, 1)
            );
        }
        if report.slabs.len() > 4 {
            let _ = writeln!(
                svg,
                r#"  <text class="muted" x="17" y="271.5">... e mais {} laje(s) na tabela completa de aço.</text>"#,
                report.slabs.len() - 4
            );
        }
    }

    // -------------------------------------------------------------------------
    // Panel 4: CA-50 Diameter Summary Table (x: 210..408, y: 164..274)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="210" y="164" width="198" height="110"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="215" y="171.5">4. RESUMO DE AÇO POR BITOLA — CA-50 (BARRAS DE 12 m)</text>"#
    );
    let tx = 215.0;
    let ty = 176.0;
    let tw = 188.0;
    let row_h = 7.2;
    let cols = [0.0, 28.0, 68.0, 108.0, 148.0, 188.0];

    // Table header
    let _ = writeln!(
        svg,
        r#"  <rect class="tbl-hdr" x="{tx:.1}" y="{ty:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
    );
    let headers = [
        ("Bitola", cols[0] + 3.0, "start"),
        ("Compr. (m)", cols[2] - 3.0, "end"),
        ("Barras 12 m", cols[3] - 3.0, "end"),
        ("Teórica (kg)", cols[4] - 3.0, "end"),
        ("+10% (kg)", cols[5] - 3.0, "end"),
    ];
    for (label, offset, anchor) in headers {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="{anchor}">{label}</text>"#,
            tx + offset,
            ty + 4.9
        );
    }

    for (idx, row) in report.by_diameter.iter().enumerate() {
        let ry = ty + row_h * ((idx + 1) as f64);
        let bg_cls = if idx % 2 == 0 { "tbl-row" } else { "tbl-alt" };
        let _ = writeln!(
            svg,
            r#"  <rect class="{bg_cls}" x="{tx:.1}" y="{ry:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}">Ø{} mm</text>"#,
            tx + cols[0] + 3.0,
            ry + 4.9,
            diameter_text(row.diameter_mm)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[2] - 3.0,
            ry + 4.9,
            decimal(row.total_length_m, 1)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[3] - 3.0,
            ry + 4.9,
            row.bars_12m
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[4] - 3.0,
            ry + 4.9,
            decimal(row.mass_kg, 1)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[5] - 3.0,
            ry + 4.9,
            decimal(row.mass_with_waste_kg, 1)
        );
    }

    let tot_y = ty + row_h * ((report.by_diameter.len() + 1) as f64);
    let total_len: f64 = report.by_diameter.iter().map(|r| r.total_length_m).sum();
    let _ = writeln!(
        svg,
        r#"  <rect class="tbl-tot" x="{tx:.1}" y="{tot_y:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}">TOTAL</text>"#,
        tx + cols[0] + 3.0,
        tot_y + 4.9
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[2] - 3.0,
        tot_y + 4.9,
        decimal(total_len, 1)
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[3] - 3.0,
        tot_y + 4.9,
        report.total_bars_12m()
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[4] - 3.0,
        tot_y + 4.9,
        decimal(report.total_mass_kg(), 1)
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[5] - 3.0,
        tot_y + 4.9,
        decimal(report.total_mass_with_waste_kg(), 1)
    );

    // Table outer border
    let tbl_h = row_h * ((report.by_diameter.len() + 2) as f64);
    let _ = writeln!(
        svg,
        r#"  <rect fill="none" class="tbl-grid" x="{tx:.1}" y="{ty:.1}" width="{tw:.1}" height="{tbl_h:.1}"/>"#
    );

    // Warnings / notes inside Panel 4 below table
    let note_y = (tot_y + row_h + 7.0).min(258.0);
    if report.warnings.is_empty() {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="215" y="{note_y:.1}">Sem alertas de domínio, biela comprimida ou espaçamento nesta estimativa.</text>"#
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="215" y="{note_y:.1}">Alertas ({}):</text>"#,
            report.warnings.len()
        );
        for (idx, w) in report.warnings.iter().take(2).enumerate() {
            let trimmed: String = w.chars().take(78).collect();
            let suffix = if w.chars().count() > 78 { "..." } else { "" };
            let _ = writeln!(
                svg,
                r#"  <text class="muted" x="215" y="{:.1}">• {}{suffix}</text>"#,
                note_y + 5.0 + (idx as f64) * 4.8,
                escape_xml(&trimmed)
            );
        }
    }

    // Footer disclaimer
    let _ = writeln!(
        svg,
        r#"  <text class="muted" x="14" y="284.5">CARÁTER PRELIMINAR: Estimativa pela ABNT NBR 6118:2023 (flexão/cortante, Branson B12, ancoragem §9.4, traspasse §9.5, +10% perda). Não substitui projeto executivo nem ART/RRT de engenheiro habilitado.</text>"#
    );
    let _ = writeln!(svg, "</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn bar_geometry_and_mass_match_the_physical_section() {
        assert!((bar_area_cm2(10.0) - PI / 4.0).abs() < 1e-5);
        assert!((bar_mass_kg_m(10.0) - 0.6165).abs() < 0.002);
        assert!((bar_mass_kg_m(12.5) - 0.963).abs() < 0.003);
    }

    #[test]
    fn flexural_steel_matches_the_existing_branson_input() {
        let fck: f64 = 25.0;
        let modulus = 0.85 * 5600.0 * fck.sqrt() * 1000.0;
        let old = effective_beam_stiffness(0.15, 0.40, fck, modulus, 45.0, 30.0);
        let demand = flexural_demand(0.15, 0.40, 0.04, 45.0, fck);
        assert!(demand.domain_ok);
        assert!((demand.required_cm2 - old.as_cm2).abs() / old.as_cm2 < 0.01);
    }

    #[test]
    fn generated_house_has_reinforcement_and_a_balanced_schedule() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.5), "Sala")
            .unwrap();
        project.generate_structure();
        let structure = project.structure_report();
        let report = build_reinforcement_report(&project, &structure);
        assert!(!report.beams.is_empty());
        assert!(!report.columns.is_empty());
        assert!(!report.slabs.is_empty());
        assert!(!report.footings.is_empty());
        assert!(!report.schedule.is_empty());
        assert!(report.total_mass_kg() > 0.0);
        assert!(
            report
                .beams
                .iter()
                .all(|beam| beam.bottom.provided_cm2 + 1e-9 >= beam.bottom.required_cm2)
        );
        assert!(
            report
                .columns
                .iter()
                .all(|column| column.longitudinal.provided_cm2 + 1e-9
                    >= column.longitudinal.required_cm2)
        );
        let sum: f64 = report.schedule.iter().map(|row| row.mass_kg).sum();
        assert!((sum - report.total_mass_kg()).abs() < 1e-9);
        let csv = steel_schedule_csv(&report);
        assert!(csv.contains("Forma"));
        assert!(csv.contains("Diâmetro (mm)"));
        assert!(csv.contains("Massa total teórica"));
        assert!(csv.contains("Resumo por bitola (CA-50)"));
    }

    #[test]
    fn nbr_6118_bond_anchorage_and_lap_lengths_match_hand_calculation() {
        // C25, CA-50 Ø10 mm:
        // fctm = 0.3 * 25^(2/3) = 2.56496 MPa
        // fctd = 0.7 * fctm / 1.4 = 1.28248 MPa
        // fbd (good, η1=2.25, η2=1.0) = 2.88559 MPa
        // fbd (poor, η1=2.25, η2=0.7) = 2.01991 MPa
        // fyd = 500 / 1.15 = 434.7826 MPa
        // lb (good) = (0.010 / 4) * (434.7826 / 2.88559) = 0.37668 m
        // lb (poor) = 0.37668 / 0.7 = 0.53812 m
        let fbd_good = bond_strength_mpa(25.0, BondZone::Good);
        let fbd_poor = bond_strength_mpa(25.0, BondZone::Poor);
        assert!((fbd_good - 2.8856).abs() < 0.002);
        assert!((fbd_poor - 2.0199).abs() < 0.002);
        assert!((fbd_poor / fbd_good - 0.7).abs() < 1e-9);

        let lb_good = basic_anchorage_m(10.0, 25.0, BondZone::Good);
        let lb_poor = basic_anchorage_m(10.0, 25.0, BondZone::Poor);
        assert!((lb_good - 0.3767).abs() < 0.002);
        assert!((lb_poor - 0.5381).abs() < 0.002);

        // Hooked anchorage with As,calc == As,ef uses α1 = 0.7
        let lb_nec_hook = required_anchorage_m(10.0, 25.0, BondZone::Good, true, 2.0, 2.0);
        assert!((lb_nec_hook - 0.7 * lb_good).abs() < 1e-6);

        // Straight lap splice with α0t = 1.4 and As,calc == As,ef
        let as_col = 4.0 * bar_area_cm2(10.0);
        let l0 = lap_splice_m(10.0, 25.0, BondZone::Good, 1.4, as_col, as_col);
        assert!((l0 - 1.4 * lb_good).abs() < 1e-6);
    }

    #[test]
    fn diameter_summary_reconciles_with_schedule_and_commercial_bars() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.8), "Sala")
            .unwrap();
        project.generate_structure();
        let structure = project.structure_report();
        let report = build_reinforcement_report(&project, &structure);

        assert!(!report.by_diameter.is_empty());
        let sum_mass: f64 = report.by_diameter.iter().map(|r| r.mass_kg).sum();
        assert!((sum_mass - report.total_mass_kg()).abs() < 1e-9);
        let sum_waste: f64 = report
            .by_diameter
            .iter()
            .map(|r| r.mass_with_waste_kg)
            .sum();
        assert!((sum_waste - report.total_mass_with_waste_kg()).abs() < 1e-9);
        for row in &report.by_diameter {
            let expected_bars =
                ((row.total_length_m * STEEL_WASTE_FACTOR) / COMMERCIAL_BAR_LENGTH_M).ceil() as u32;
            assert_eq!(row.bars_12m, expected_bars);
        }
        assert!(report.total_bars_12m() > 0);
        assert!(report.beams.iter().all(|b| b.bottom_anchorage_m >= 0.10));
        assert!(report.beams.iter().all(|b| b.top_anchorage_m >= 0.10));
        assert!(report.columns.iter().all(|c| c.lap_length_m >= 0.40));
    }

    #[test]
    fn reinforcement_detail_svg_is_valid_svg_with_all_sections() {
        let mut project = Project::default();
        project.name = "Casa Teste #110".to_owned();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.8), "Sala")
            .unwrap();
        project.generate_structure();
        let structure = project.structure_report();
        let report = build_reinforcement_report(&project, &structure);
        let svg = reinforcement_detail_svg(&project, &structure, &report);

        assert!(svg.contains("DETALHAMENTO ESQUEMÁTICO DE ARMADURAS"));
        assert!(svg.contains("VIGA DE REFERÊNCIA"));
        assert!(svg.contains("PILAR DE REFERÊNCIA"));
        assert!(svg.contains("SAPATA DE REFERÊNCIA"));
        assert!(svg.contains("RESUMO DE AÇO POR BITOLA"));
        assert!(svg.contains("Branson B12:"));
        assert!(
            svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok(),
            "reinforcement_detail_svg must produce valid SVG"
        );
    }

    #[test]
    fn reinforcement_is_refused_outside_the_structural_limits() {
        let mut project = Project::default();
        project.structure.settings.floors = limits::MAX_STOREYS + 1;
        let report = build_reinforcement_report(&project, &StructureReport::default());
        assert!(report.is_empty());
        assert!(report.warnings[0].contains("pavimentos"));
    }
}
