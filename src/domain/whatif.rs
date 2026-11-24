//! Interactive What-if simulation mode (#107): non-destructive sliders for
//! span, section, load and material that run Trazo's real structural and
//! traffic-light solvers in under 100 ms and highlight the green/yellow/red
//! safe range for every parameter.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{
    fixtures::FixtureKind,
    geometry::Point,
    limits,
    model::Project,
    reinforcement::build_reinforcement_report,
    structure::{OccupancyClass, StructureReport},
    verdict::{Light, structure_verdicts},
};

/// Slider parameters for a non-destructive What-if simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WhatIfParams {
    /// Optional focused beam id (`None` adjusts all beams together).
    pub target_beam_id: Option<u64>,
    /// Geometric span scale factor (1.0 = current plan geometry; e.g. 0.7..=1.6).
    pub span_scale: f64,
    /// Beam section width (cm), 12..=40.
    pub beam_width_cm: f64,
    /// Beam section height (cm), 20..=80.
    pub beam_height_cm: f64,
    /// Minimum column side (cm), 14..=60.
    pub column_side_cm: f64,
    /// Slab thickness (cm), 8..=25.
    pub slab_thickness_cm: f64,
    /// Live load q (kN/m²), 0.5..=6.0.
    pub live_load_kn_m2: f64,
    /// Concrete characteristic compressive strength fck (MPa), 20..=50.
    pub fck_mpa: f64,
    /// Allowable soil bearing pressure (kPa), 60..=500.
    pub soil_kpa: f64,
    /// Basic wind speed V0 (m/s, NBR 6123), 30..=50.
    pub wind_v0_ms: f64,
    /// Building occupancy class (NBR 6120 / NBR 8681).
    pub occupancy: OccupancyClass,
}

impl WhatIfParams {
    /// Initializes sliders from the current project's governing elements and settings.
    pub fn from_project(project: &Project) -> Self {
        let report = project.cached_structure_report();
        // Pick the governing (longest or highest-utilisation) beam as default reference.
        let governing_span = report
            .beams
            .iter()
            .flat_map(|line| line.spans.iter())
            .max_by(|a, b| {
                let ua = (a.height_min_m / a.height_m.max(1e-6))
                    .max(a.deflection_mm / a.limit_mm.max(1e-6));
                let ub = (b.height_min_m / b.height_m.max(1e-6))
                    .max(b.deflection_mm / b.limit_mm.max(1e-6));
                ua.total_cmp(&ub)
            });

        let (beam_width_cm, beam_height_cm) = governing_span
            .map(|s| (s.width_m * 100.0, s.height_m * 100.0))
            .or_else(|| {
                project
                    .structure
                    .beams
                    .first()
                    .map(|b| (b.width_m * 100.0, b.height_m * 100.0))
            })
            .unwrap_or((14.0, 35.0));

        let column_side_cm = project
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Column)
            .map(|f| f.width.min(f.depth) * 100.0)
            .fold(f64::INFINITY, f64::min);
        let column_side_cm = if column_side_cm.is_finite() {
            column_side_cm.clamp(14.0, 60.0)
        } else {
            20.0
        };

        let slab_thickness_cm = project
            .structure
            .slabs
            .first()
            .map(|s| (s.thickness_m * 100.0).clamp(8.0, 25.0))
            .unwrap_or(12.0);

        Self {
            target_beam_id: None,
            span_scale: 1.0,
            beam_width_cm: beam_width_cm.clamp(12.0, 40.0),
            beam_height_cm: beam_height_cm.clamp(20.0, 80.0),
            column_side_cm,
            slab_thickness_cm,
            live_load_kn_m2: project.structure.settings.live_kn_m2.clamp(0.5, 6.0),
            fck_mpa: project.structure.settings.fck_mpa.clamp(20.0, 50.0),
            soil_kpa: project.structure.settings.soil_kpa.clamp(60.0, 500.0),
            wind_v0_ms: project.structure.wind.v0_ms.clamp(30.0, 50.0),
            occupancy: project.structure.settings.occupancy,
        }
    }
}

/// Colour-coded safe range along one slider axis (`min..=max`), showing which
/// sub-intervals keep the governing check green (≤ 85 %) or yellow (85–100 %).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafeRangeBand {
    pub parameter: String,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub current: f64,
    /// Interval `[low, high]` where utilisation ≤ 85 % (green), if any.
    pub green: Option<(f64, f64)>,
    /// Interval `[low, high]` where utilisation ≤ 100 % (green + yellow), if any.
    pub passing: Option<(f64, f64)>,
}

/// Compact engineering snapshot used to compare baseline vs. simulated states.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WhatIfSnapshot {
    pub overall_light: String,
    pub refused_reason: Option<String>,
    pub max_utilisation_pct: f64,
    pub governing_element: String,
    pub max_span_m: f64,
    pub max_beam_moment_knm: f64,
    pub max_beam_deflection_mm: f64,
    pub max_beam_deflection_ratio: f64,
    pub max_column_utilisation_pct: f64,
    pub max_footing_utilisation_pct: f64,
    pub steel_mass_kg: f64,
    pub concrete_volume_m3: f64,
    pub green_count: usize,
    pub yellow_count: usize,
    pub red_count: usize,
}

/// Full result of one What-if evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub params: WhatIfParams,
    pub baseline: WhatIfSnapshot,
    pub simulated: WhatIfSnapshot,
    pub delta_utilisation_pct: f64,
    pub delta_steel_kg: f64,
    pub delta_concrete_m3: f64,
    pub safe_ranges: Vec<SafeRangeBand>,
    pub advice: String,
    pub elapsed_ms: f64,
}

fn project_centroid(project: &Project) -> Point {
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut n = 0.0;
    for w in &project.walls {
        sx += w.start.x + w.end.x;
        sy += w.start.y + w.end.y;
        n += 2.0;
    }
    if n < 1.0 {
        return Point::new(0.0, 0.0);
    }
    Point::new(sx / n, sy / n)
}

fn scale_point(p: Point, origin: Point, factor: f64) -> Point {
    Point::new(
        origin.x + (p.x - origin.x) * factor,
        origin.y + (p.y - origin.y) * factor,
    )
}

/// Produces a cloned project reflecting `params` without mutating the source.
pub fn build_whatif_project(project: &Project, params: &WhatIfParams) -> Project {
    let mut sim = project.clone();
    let baseline_live = project.structure.settings.live_kn_m2;

    // 1. Scale geometry if span_scale != 1.0 (keeps all beams/columns/slabs/walls connected).
    let s = params.span_scale.clamp(0.5, 2.0);
    if (s - 1.0).abs() > 1e-4 {
        let origin = project_centroid(&sim);
        for w in &mut sim.walls {
            w.start = scale_point(w.start, origin, s);
            w.end = scale_point(w.end, origin, s);
        }
        for f in &mut sim.fixtures {
            f.center = scale_point(f.center, origin, s);
        }
        for r in &mut sim.structure.roofs {
            r.min = scale_point(r.min, origin, s);
            r.max = scale_point(r.max, origin, s);
        }
        if let Some(t) = &mut sim.terrain {
            t.min = scale_point(t.min, origin, s);
            t.max = scale_point(t.max, origin, s);
            for pt in &mut t.vertices {
                *pt = scale_point(*pt, origin, s);
            }
        }
    }

    // 2. Apply section parameters.
    let bw = (params.beam_width_cm / 100.0).clamp(0.10, 0.60);
    let bh = (params.beam_height_cm / 100.0).clamp(0.15, 1.20);
    for beam in &mut sim.structure.beams {
        if params.target_beam_id.is_none() || params.target_beam_id == Some(beam.id) {
            beam.width_m = bw;
            beam.height_m = bh;
        }
    }
    let col_side = (params.column_side_cm / 100.0).clamp(0.14, 0.80);
    for fix in &mut sim.fixtures {
        if fix.kind == FixtureKind::Column {
            if fix.width <= fix.depth {
                fix.width = col_side;
                fix.depth = fix.depth.max(col_side);
            } else {
                fix.depth = col_side;
                fix.width = fix.width.max(col_side);
            }
        }
    }
    let slab_h = (params.slab_thickness_cm / 100.0).clamp(0.08, 0.30);
    for slab in &mut sim.structure.slabs {
        slab.thickness_m = slab_h;
    }

    // 3. Apply loads & material settings.
    sim.structure.settings.fck_mpa = params.fck_mpa.clamp(20.0, 50.0);
    sim.structure.settings.soil_kpa = params.soil_kpa.clamp(50.0, 600.0);
    sim.structure.settings.occupancy = params.occupancy;
    sim.structure.settings.live_kn_m2 = params.live_load_kn_m2.clamp(0.5, 10.0);
    if (params.live_load_kn_m2 - baseline_live).abs() > 1e-3 {
        sim.structure.settings.per_room_live_loads = false;
    }
    sim.structure.wind.v0_ms = params.wind_v0_ms.clamp(25.0, 55.0);

    sim
}

/// Applies the section, load and material parameters from `params` onto a real
/// project (span scaling is left non-destructive so room coordinates are not
/// unexpectedly shifted unless edited explicitly).
pub fn apply_whatif_to_project(project: &mut Project, params: &WhatIfParams) {
    let bw = (params.beam_width_cm / 100.0).clamp(0.10, 0.60);
    let bh = (params.beam_height_cm / 100.0).clamp(0.15, 1.20);
    for beam in &mut project.structure.beams {
        if params.target_beam_id.is_none() || params.target_beam_id == Some(beam.id) {
            beam.width_m = bw;
            beam.height_m = bh;
        }
    }
    let col_side = (params.column_side_cm / 100.0).clamp(0.14, 0.80);
    for fix in &mut project.fixtures {
        if fix.kind == FixtureKind::Column {
            if fix.width <= fix.depth {
                fix.width = col_side;
                fix.depth = fix.depth.max(col_side);
            } else {
                fix.depth = col_side;
                fix.width = fix.width.max(col_side);
            }
        }
    }
    let slab_h = (params.slab_thickness_cm / 100.0).clamp(0.08, 0.30);
    for slab in &mut project.structure.slabs {
        slab.thickness_m = slab_h;
    }
    let old_live = project.structure.settings.live_kn_m2;
    project.structure.settings.fck_mpa = params.fck_mpa.clamp(20.0, 50.0);
    project.structure.settings.soil_kpa = params.soil_kpa.clamp(50.0, 600.0);
    project.structure.settings.occupancy = params.occupancy;
    project.structure.settings.live_kn_m2 = params.live_load_kn_m2.clamp(0.5, 10.0);
    if (params.live_load_kn_m2 - old_live).abs() > 1e-3 {
        project.structure.settings.per_room_live_loads = false;
    }
    project.structure.wind.v0_ms = params.wind_v0_ms.clamp(25.0, 55.0);
}

fn estimate_concrete_m3(project: &Project, report: &StructureReport) -> f64 {
    let mut vol = 0.0;
    for line in &report.beams {
        for span in &line.spans {
            vol += span.width_m * span.height_m * span.length_m;
        }
    }
    for col in &report.columns {
        vol += col.width_m * col.depth_m * col.height_m;
    }
    for ft in &report.footings {
        vol += ft.width_m * ft.depth_m * ft.height_m;
    }
    for slab in &project.structure.slabs {
        if let Some(room) = project.rooms.iter().find(|r| r.id == slab.room_id)
            && let Some(poly) = project.room_polygon(room)
        {
            let area = super::geometry::polygon_area(&poly);
            let factor = match slab.kind {
                super::structure::SlabKind::Solid => 1.0,
                super::structure::SlabKind::Ribbed => 0.55,
            };
            vol += area * slab.thickness_m * factor;
        }
    }
    vol
}

fn snapshot_project(project: &Project, report: &StructureReport) -> WhatIfSnapshot {
    let refused_reason = limits::structural_refusal(project);
    let verdicts = structure_verdicts(project, report);
    let reinf = build_reinforcement_report(project, report);

    let mut overall = Light::Green;
    let mut max_util = 0.0_f64;
    let mut governing = "—".to_owned();
    let mut green_count = 0usize;
    let mut yellow_count = 0usize;
    let mut red_count = 0usize;

    for v in &verdicts {
        let light = v.light();
        overall = overall.max(light);
        match light {
            Light::Green => green_count += 1,
            Light::Yellow => yellow_count += 1,
            Light::Red => red_count += 1,
        }
        let util = v
            .governing()
            .map(|c| {
                if c.utilisation.is_finite() {
                    c.utilisation
                } else {
                    1.5
                }
            })
            .unwrap_or(0.0);
        if util > max_util {
            max_util = util;
            governing = v.element.clone();
        }
    }
    if refused_reason.is_some() {
        overall = Light::Red;
        max_util = max_util.max(1.5);
    }

    let mut max_span_m = 0.0_f64;
    let mut max_beam_moment_knm = 0.0_f64;
    let mut max_beam_deflection_mm = 0.0_f64;
    let mut max_beam_deflection_ratio = 0.0_f64;
    for line in &report.beams {
        for span in &line.spans {
            max_span_m = max_span_m.max(span.length_m);
            let m = span
                .moment_left
                .abs()
                .max(span.moment_right.abs())
                .max(span.moment_span.abs());
            max_beam_moment_knm = max_beam_moment_knm.max(m);
            max_beam_deflection_mm = max_beam_deflection_mm.max(span.deflection_mm);
            if span.limit_mm > 1e-6 {
                max_beam_deflection_ratio =
                    max_beam_deflection_ratio.max(span.deflection_mm / span.limit_mm);
            }
        }
    }

    let max_col = report
        .columns
        .iter()
        .map(|c| {
            let ua = c.area_needed_cm2 / c.area_cm2.max(1e-6);
            let us = c.slenderness / 90.0;
            ua.max(us)
        })
        .fold(0.0_f64, f64::max);

    let max_ft = report
        .footings
        .iter()
        .map(|f| {
            let us = f.max_pressure_kpa / f.soil_kpa.max(1e-6);
            us.max(f.punching_ratio)
        })
        .fold(0.0_f64, f64::max);

    WhatIfSnapshot {
        overall_light: overall.key().to_owned(),
        refused_reason,
        max_utilisation_pct: (max_util * 100.0).min(999.0),
        governing_element: governing,
        max_span_m,
        max_beam_moment_knm,
        max_beam_deflection_mm,
        max_beam_deflection_ratio,
        max_column_utilisation_pct: max_col * 100.0,
        max_footing_utilisation_pct: max_ft * 100.0,
        steel_mass_kg: reinf.total_mass_kg(),
        concrete_volume_m3: estimate_concrete_m3(project, report),
        green_count,
        yellow_count,
        red_count,
    }
}

/// Computes fast physical safe-range bands (`green ≤ 85%`, `passing ≤ 100%`)
/// for the primary What-if sliders using the governing element's equations and
/// Trazo's applicability limits (`#102`), keeping total evaluation well under 100 ms.
fn compute_safe_ranges(
    sim_report: &StructureReport,
    params: &WhatIfParams,
    base_max_span: f64,
) -> Vec<SafeRangeBand> {
    let mut bands = Vec::with_capacity(5);

    // Governing beam span & load.
    let gov_span = sim_report
        .beams
        .iter()
        .flat_map(|l| l.spans.iter())
        .max_by(|a, b| {
            let ua =
                (a.height_min_m / a.height_m.max(1e-6)).max(a.deflection_mm / a.limit_mm.max(1e-6));
            let ub =
                (b.height_min_m / b.height_m.max(1e-6)).max(b.deflection_mm / b.limit_mm.max(1e-6));
            ua.total_cmp(&ub)
        });

    let cur_h_m = (params.beam_height_cm / 100.0).max(0.15);
    let cur_l_m = gov_span
        .map(|s| s.length_m)
        .unwrap_or(base_max_span.max(3.0));
    let cur_defl_ratio = gov_span
        .map(|s| s.deflection_mm / s.limit_mm.max(1e-6))
        .unwrap_or(0.3)
        .max(0.05);
    let h_ratio_divisor = gov_span
        .map(|s| {
            if s.height_min_m > 1e-6 {
                s.length_m / s.height_min_m
            } else {
                10.0
            }
        })
        .unwrap_or(10.0)
        .clamp(10.0, 12.0);

    // 1. Span scale (shown as governing span in m, 1.5..=7.5 m; limit 7.0 m).
    let span_min = (base_max_span * 0.6).clamp(1.5, 4.0);
    let span_max = (base_max_span * 1.6).clamp(6.5, 8.0);
    // Max span from h_min = L/divisor <= u * h and deflection ~ (L/L_cur)^3 * cur_defl_ratio <= u.
    let max_l_for = |u: f64| -> f64 {
        let from_h = u * cur_h_m * h_ratio_divisor;
        let from_defl = cur_l_m * (u / cur_defl_ratio).cbrt();
        from_h.min(from_defl).min(7.0)
    };
    let l_green = max_l_for(0.85).clamp(span_min, span_max);
    let l_pass = max_l_for(1.00).clamp(span_min, span_max);
    bands.push(SafeRangeBand {
        parameter: "Vão governante".to_owned(),
        unit: "m".to_owned(),
        min: span_min,
        max: span_max,
        current: cur_l_m.clamp(span_min, span_max),
        green: (l_green > span_min + 0.05).then_some((span_min, l_green)),
        passing: (l_pass > span_min + 0.05).then_some((span_min, l_pass)),
    });

    // 2. Beam height h (20..=80 cm): h_min / u and deflection ~ (h_cur / h)^3.
    let h_min_for = |u: f64| -> f64 {
        let from_ratio = (cur_l_m / h_ratio_divisor) / u * 100.0;
        let from_defl = params.beam_height_cm * (cur_defl_ratio / u).cbrt();
        from_ratio.max(from_defl).clamp(20.0, 80.0)
    };
    let h_green = h_min_for(0.85);
    let h_pass = h_min_for(1.00);
    bands.push(SafeRangeBand {
        parameter: "Altura da viga".to_owned(),
        unit: "cm".to_owned(),
        min: 20.0,
        max: 80.0,
        current: params.beam_height_cm,
        green: (h_green < 80.0).then_some((h_green, 80.0)),
        passing: (h_pass < 80.0).then_some((h_pass, 80.0)),
    });

    // 3. Live load q (0.5..=6.0 kN/m²; applicability limit 5.0 kN/m²).
    let gov_u = gov_span
        .map(|s| s.deflection_mm / s.limit_mm.max(1e-6))
        .unwrap_or(0.3);
    let q_cur = params.live_load_kn_m2;
    let q_max_for = |u: f64| -> f64 {
        // Approximate linear response of deflection/footing pressure to live load fraction (~30% of total load).
        let headroom = if gov_u > 1e-4 { u / gov_u } else { 3.0 };
        let q_limit = q_cur * (1.0 + (headroom - 1.0) / 0.35).max(0.1);
        q_limit.min(5.0).clamp(0.5, 6.0)
    };
    let q_green = q_max_for(0.85);
    let q_pass = q_max_for(1.00);
    bands.push(SafeRangeBand {
        parameter: "Sobrecarga".to_owned(),
        unit: "kN/m²".to_owned(),
        min: 0.5,
        max: 6.0,
        current: q_cur.clamp(0.5, 6.0),
        green: (q_green > 0.55).then_some((0.5, q_green)),
        passing: (q_pass > 0.55).then_some((0.5, q_pass)),
    });

    // 4. Concrete fck (20..=50 MPa): column area ~ 1/fck, Ec ~ sqrt(fck).
    let col_max_u = sim_report
        .columns
        .iter()
        .map(|c| c.area_needed_cm2 / c.area_cm2.max(1e-6))
        .fold(0.0_f64, f64::max);
    let fck_min_for = |u: f64| -> f64 {
        let from_col = params.fck_mpa * (col_max_u / u);
        let from_defl = params.fck_mpa * (cur_defl_ratio / u).powi(2);
        from_col.max(from_defl).clamp(20.0, 50.0)
    };
    let fck_green = fck_min_for(0.85);
    let fck_pass = fck_min_for(1.00);
    bands.push(SafeRangeBand {
        parameter: "Concreto fck".to_owned(),
        unit: "MPa".to_owned(),
        min: 20.0,
        max: 50.0,
        current: params.fck_mpa,
        green: (fck_green <= 49.5).then_some((fck_green, 50.0)),
        passing: (fck_pass <= 49.5).then_some((fck_pass, 50.0)),
    });

    // 5. Soil allowable bearing pressure (60..=500 kPa; applicability limit ≥ 100 kPa).
    let max_pressure = sim_report
        .footings
        .iter()
        .map(|f| f.max_pressure_kpa)
        .fold(0.0_f64, f64::max);
    let soil_min_for = |u: f64| -> f64 { (max_pressure / u).max(100.0).clamp(60.0, 500.0) };
    let soil_green = soil_min_for(0.85);
    let soil_pass = soil_min_for(1.00);
    bands.push(SafeRangeBand {
        parameter: "Solo admissível".to_owned(),
        unit: "kPa".to_owned(),
        min: 60.0,
        max: 500.0,
        current: params.soil_kpa.clamp(60.0, 500.0),
        green: (soil_green < 495.0).then_some((soil_green, 500.0)),
        passing: (soil_pass < 495.0).then_some((soil_pass, 500.0)),
    });

    bands
}

/// Evaluates a What-if scenario against the current project, returning baseline
/// vs. simulated snapshots, deltas, safe ranges and plain-language advice.
pub fn evaluate_whatif(project: &Project, params: &WhatIfParams) -> WhatIfResult {
    let start = Instant::now();
    let base_report = project.cached_structure_report();
    let baseline = snapshot_project(project, &base_report);

    let sim_project = build_whatif_project(project, params);
    let sim_report = sim_project.structure_report();
    let simulated = snapshot_project(&sim_project, &sim_report);

    let safe_ranges = compute_safe_ranges(&sim_report, params, baseline.max_span_m.max(3.0));

    let advice = if let Some(reason) = &simulated.refused_reason {
        format!(
            "Fora dos limites do Trazo: {reason} Reduza o vão ou a carga para voltar à faixa segura."
        )
    } else if simulated.overall_light == "vermelho" {
        let h_rec = safe_ranges
            .iter()
            .find(|b| b.parameter == "Altura da viga")
            .and_then(|b| b.green)
            .map(|(lo, _)| lo.ceil())
            .unwrap_or((params.beam_height_cm + 10.0).min(80.0));
        format!(
            "Elemento crítico {} em {:.0} % (vermelho). Tente elevar a altura da viga para ~{h_rec:.0} cm ou reduzir o vão.",
            simulated.governing_element, simulated.max_utilisation_pct
        )
    } else if simulated.overall_light == "amarelo" {
        format!(
            "Passa com margem reduzida ({:.0} % em {}). Pequeno ganho de seção ou fck leva o cenário ao verde (≤ 85 %).",
            simulated.max_utilisation_pct, simulated.governing_element
        )
    } else {
        format!(
            "Cenário verde com folga ({:.0} % máx. em {}). Δ aço: {:+.1} kg.",
            simulated.max_utilisation_pct,
            simulated.governing_element,
            simulated.steel_mass_kg - baseline.steel_mass_kg
        )
    };

    WhatIfResult {
        params: params.clone(),
        delta_utilisation_pct: simulated.max_utilisation_pct - baseline.max_utilisation_pct,
        delta_steel_kg: simulated.steel_mass_kg - baseline.steel_mass_kg,
        delta_concrete_m3: simulated.concrete_volume_m3 - baseline.concrete_volume_m3,
        baseline,
        simulated,
        safe_ranges,
        advice,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatif_sliders_update_sample_house_under_100ms_and_compute_safe_ranges() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.5), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.5, 0.0), Point::new(8.0, 3.5), "Quarto")
            .unwrap();
        project
            .add_room(Point::new(0.0, 3.5), Point::new(4.5, 6.5), "Cozinha")
            .unwrap();
        project.generate_structure();
        // Warm baseline cache just like in interactive UI usage.
        let _ = project.cached_structure_report();

        let base_params = WhatIfParams::from_project(&project);
        let base_res = evaluate_whatif(&project, &base_params);
        assert!(
            base_res.elapsed_ms < 100.0,
            "what-if update must stay under 100 ms (got {:.2} ms)",
            base_res.elapsed_ms
        );
        assert_eq!(base_res.safe_ranges.len(), 5);

        // Shallow beam (20 cm) raises utilisation; deep beam (55 cm) lowers it.
        let mut shallow = base_params.clone();
        shallow.beam_height_cm = 20.0;
        let shallow_res = evaluate_whatif(&project, &shallow);
        let mut deep = base_params.clone();
        deep.beam_height_cm = 55.0;
        let deep_res = evaluate_whatif(&project, &deep);
        assert!(
            shallow_res.simulated.max_utilisation_pct > deep_res.simulated.max_utilisation_pct,
            "shallower beam should have higher utilisation than deeper beam"
        );
        assert!(
            shallow_res.elapsed_ms < 100.0 && deep_res.elapsed_ms < 100.0,
            "slider updates must finish in under 100 ms"
        );

        // Excess live load (> 5.0 kN/m²) triggers Trazo's #102 refusal cleanly.
        let mut heavy = base_params.clone();
        heavy.live_load_kn_m2 = 5.5;
        let heavy_res = evaluate_whatif(&project, &heavy);
        assert_eq!(heavy_res.simulated.overall_light, "vermelho");
        assert!(heavy_res.simulated.refused_reason.is_some());

        // Applying What-if params mutates the project cleanly.
        apply_whatif_to_project(&mut project, &deep);
        assert!((project.structure.beams[0].height_m - 0.55).abs() < 1e-6);
    }
}
