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

const LONGITUDINAL_DIAMETERS_MM: [f64; 5] = [10.0, 12.5, 16.0, 20.0, 25.0];
const MESH_DIAMETERS_MM: [f64; 4] = [6.3, 8.0, 10.0, 12.5];
const STIRRUP_DIAMETERS_MM: [f64; 3] = [6.3, 8.0, 10.0];
const STANDARD_SPACINGS_CM: [f64; 6] = [20.0, 15.0, 12.5, 10.0, 7.5, 5.0];

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
    pub bottom: BarSet,
    pub top_left: BarSet,
    pub top_right: BarSet,
    pub stirrups: TransverseBars,
    pub flexural_domain_ok: bool,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnReinforcement {
    pub column_id: u64,
    pub label: String,
    pub longitudinal: BarSet,
    pub ties: TransverseBars,
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
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootingReinforcement {
    pub footing_id: u64,
    pub label: String,
    pub bars_x: MeshBars,
    pub bars_y: MeshBars,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SteelScheduleRow {
    pub mark: String,
    pub element: String,
    pub position: String,
    pub diameter_mm: f64,
    pub quantity: u32,
    pub unit_length_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReinforcementReport {
    pub beams: Vec<BeamReinforcement>,
    pub columns: Vec<ColumnReinforcement>,
    pub slabs: Vec<SlabReinforcement>,
    pub footings: Vec<FootingReinforcement>,
    pub schedule: Vec<SteelScheduleRow>,
    pub warnings: Vec<String>,
}

impl ReinforcementReport {
    pub fn total_mass_kg(&self) -> f64 {
        self.schedule.iter().map(|row| row.mass_kg).sum()
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
    bars: &BarSet,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
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
    bars: &TransverseBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
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
    bars: &MeshBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
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
            let cover = 0.04;
            let bottom_demand =
                flexural_demand(span.width_m, span.height_m, cover, span.moment_span, fck);
            let left_demand = flexural_demand(
                span.width_m,
                span.height_m,
                cover,
                span.moment_left.abs(),
                fck,
            );
            let right_demand = flexural_demand(
                span.width_m,
                span.height_m,
                cover,
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
            let anchorage = |diameter_mm: f64| (40.0 * diameter_mm / 1000.0).max(0.30);
            let bottom_choice = choose_beam_bars(bottom_required, span.width_m, cover);
            let left_choice = choose_beam_bars(left_demand.required_cm2, span.width_m, cover);
            let right_choice = choose_beam_bars(right_demand.required_cm2, span.width_m, cover);
            let bottom = bar_set(
                bottom_choice,
                bottom_required,
                span.length_m + 2.0 * anchorage(bottom_choice.diameter_mm),
            );
            let support_length = (0.30 * span.length_m).max(0.60);
            let top_left = bar_set(
                left_choice,
                left_demand.required_cm2,
                support_length + anchorage(left_choice.diameter_mm),
            );
            let top_right = bar_set(
                right_choice,
                right_demand.required_cm2,
                support_length + anchorage(right_choice.diameter_mm),
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
                    "{label}: as barras não cabem em uma camada com cobrimento de 4 cm; aumente a largura ou detalhe em camadas."
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
                &bottom,
            );
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-SUP-E"),
                &label,
                "superior apoio esquerdo",
                &top_left,
            );
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-SUP-D"),
                &label,
                "superior apoio direito",
                &top_right,
            );
            schedule_transverse(
                &mut out.schedule,
                format!("{label}-E"),
                &label,
                "estribos",
                &stirrups,
            );
            out.beams.push(BeamReinforcement {
                beam_id: span.beam,
                label,
                span_index: index + 1,
                length_m: span.length_m,
                bottom,
                top_left,
                top_right,
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
        let lap = (40.0 * choice.diameter_mm / 1000.0).max(0.40);
        let longitudinal = bar_set(choice, required, project.wall_height + lap);
        let tie_spacing_m = 0.20_f64
            .min(12.0 * choice.diameter_mm / 1000.0)
            .min(column.width_m.min(column.depth_m))
            .max(0.05);
        let spacing_cm = (tie_spacing_m * 100.0 / 5.0).floor().max(1.0) * 5.0;
        let tie_count = (project.wall_height / (spacing_cm / 100.0)).ceil() as u32 + 1;
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
            &longitudinal,
        );
        schedule_transverse(
            &mut out.schedule,
            format!("{}-T", column.label),
            &column.label,
            "estribos",
            &ties,
        );
        out.columns.push(ColumnReinforcement {
            column_id: column.id,
            label: column.label.clone(),
            longitudinal,
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
        let characteristic_moment = slab.load_kn_m2 * slab.lx_m.powi(2) / 8.0;
        let demand = flexural_demand(1.0, slab.thickness_m, 0.025, characteristic_moment, fck);
        let main_length = slab.lx_m + 0.30;
        let main = choose_mesh(demand.required_cm2, slab.ly_m, main_length);
        let distribution_required = (0.20 * main.required_cm2_m).max(0.90);
        let distribution = choose_mesh(distribution_required, slab.lx_m, slab.ly_m + 0.30);
        if slab.kind == SlabKind::Ribbed {
            out.warnings.push(format!(
                "{} ({}): aço principal é uma estimativa equivalente por metro; confirme a vigota/treliça com o fabricante.",
                slab.label, slab.room
            ));
        }
        if !demand.domain_ok {
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
            "principal (menor vão)",
            &main,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-D", slab.label),
            &slab.label,
            "distribuição",
            &distribution,
        );
        out.slabs.push(SlabReinforcement {
            slab_id: slab.id,
            label: slab.label.clone(),
            room: slab.room.clone(),
            kind: slab.kind,
            main,
            distribution,
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
        let bars_x = choose_mesh(
            demand_x.required_cm2,
            footing.depth_m - 0.10,
            footing.width_m - 0.10,
        );
        let bars_y = choose_mesh(
            demand_y.required_cm2,
            footing.width_m - 0.10,
            footing.depth_m - 0.10,
        );
        let mass_kg = bars_x.mass_kg + bars_y.mass_kg;
        schedule_mesh(
            &mut out.schedule,
            format!("{}-X", footing.label),
            &footing.label,
            "malha inferior X",
            &bars_x,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-Y", footing.label),
            &footing.label,
            "malha inferior Y",
            &bars_y,
        );
        out.footings.push(FootingReinforcement {
            footing_id: footing.id,
            label: footing.label.clone(),
            bars_x,
            bars_y,
            mass_kg,
        });
    }

    out
}

pub fn steel_schedule_csv(report: &ReinforcementReport) -> String {
    let mut csv = String::from(
        "Marca;Elemento;Posição;Diâmetro (mm);Quantidade;Comprimento unitário (m);Comprimento total (m);Massa (kg)\n",
    );
    for row in &report.schedule {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{}\n",
            row.mark,
            row.element,
            row.position,
            diameter_text(row.diameter_mm),
            row.quantity,
            decimal(row.unit_length_m, 2),
            decimal(row.total_length_m, 2),
            decimal(row.mass_kg, 1)
        ));
    }
    csv.push_str(&format!(
        ";;;;;;Massa total;{}\n",
        decimal(report.total_mass_kg(), 1)
    ));
    csv.push_str(
        ";;;;;;Caráter;Estimativa preliminar NBR 6118:2023 - auditoria da edição 2026 e revisão por engenheiro obrigatórias\n",
    );
    csv
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
        assert!(csv.contains("Diâmetro (mm)"));
        assert!(csv.contains("Massa total"));
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
