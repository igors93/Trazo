//! Builds and solves a 2D plane frame (`Frame2D`) for each continuous beam
//! line in a `Project`, incorporating real column axial & flexural stiffness,
//! rigid or pinned beam-column joints, fixed or pinned column bases, separate
//! NBR 6120 load cases (`G1`, `G2`, `Q`), NBR 6123 building wind (`W+`, `W−`),
//! and exact NBR 8681 / NBR 6118 ultimate (ELU) & service (ELS) combinations
//! and envelopes (#94, #105).

use std::collections::HashMap;

use crate::domain::{
    fixtures::FixtureKind,
    loads::{
        CombinationCategory, CombinationFactors, LoadCaseKind, PSI1_WIND,
        build_nbr8681_combinations,
    },
    model::Project,
    structure::{BeamSupport, SpanPointLoad, StructureReport},
};

use super::{
    model::{EndRelease, Frame2D, MemberKind, SupportKind},
    solver::{FrameSolution, MemberSample, solve_frame},
};

/// Ultimate force envelope of one physical beam span from the 2D frame analysis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameBeamEnvelope {
    /// Already factored design forces, enveloped over all ELU combinations.
    pub max_positive_moment_knm: f64,
    pub positive_combination: String,
    pub min_negative_moment_knm: f64,
    pub negative_combination: String,
    pub max_shear_kn: f64,
    pub shear_combination: String,
}

/// Internal service forces and ultimate envelope of one physical beam span.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameBeamForces {
    pub beam_id: u64,
    pub label: String,
    pub length_m: f64,
    pub load_kn_m: f64,
    /// Concentrated loads actually applied to this frame member.
    pub point_loads: Vec<SpanPointLoad>,
    /// Bending moment at left end, max sagging along span, and right end (kN·m).
    pub moment_left_knm: f64,
    pub moment_span_knm: f64,
    pub moment_right_knm: f64,
    pub max_shear_kn: f64,
    /// Axial force in beam (+ tension, − compression), kN.
    pub axial_kn: f64,
    /// Elastic local deflection of the span from the 2D frame solution, mm.
    pub elastic_deflection_mm: f64,
    pub ultimate: FrameBeamEnvelope,
}

/// Internal forces and base reactions in one column of a 2D plane frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameColumnForces {
    pub column_id: u64,
    pub label: String,
    pub height_m: f64,
    pub width_m: f64,
    pub depth_m: f64,
    /// Axial force (− compression, + tension), kN.
    pub axial_kn: f64,
    /// Column shear force, kN.
    pub shear_kn: f64,
    /// Bending moment at column base and column top, kN·m.
    pub moment_base_knm: f64,
    pub moment_top_knm: f64,
    pub max_moment_knm: f64,
    /// Base support reactions in global frame coordinates (`Rx, Ry, Mz`).
    pub reaction_rx_kn: f64,
    pub reaction_ry_kn: f64,
    pub reaction_mz_knm: f64,
}

/// Summary of one fundamental load case (`G1`, `G2`, `Q`, `W+`, `W−`) on a 2D frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLoadCaseResult {
    pub kind: LoadCaseKind,
    pub total_vertical_kn: f64,
    pub total_horizontal_kn: f64,
    pub max_beam_moment_knm: f64,
    pub max_column_moment_knm: f64,
    pub max_lateral_drift_mm: f64,
}

/// Summary of one NBR 8681 / NBR 6118 combination evaluated by exact superposition
/// on a 2D plane frame (#105).
#[derive(Clone, Debug, PartialEq)]
pub struct FrameCombinationResult {
    pub code: &'static str,
    pub name: &'static str,
    pub category: CombinationCategory,
    pub formula: String,
    /// Largest sagging (positive) beam moment, kN·m.
    pub max_beam_pos_knm: f64,
    /// Largest hogging (negative, reported as signed ≤ 0) beam moment, kN·m.
    pub min_beam_neg_knm: f64,
    /// Largest absolute beam shear, kN.
    pub max_beam_shear_kn: f64,
    /// Largest absolute column bending moment, kN·m.
    pub max_column_moment_knm: f64,
    /// Largest column compression magnitude (`max(-N, 0)`), kN.
    pub max_column_compression_kn: f64,
    /// Minimum column axial force (signed: negative = compression, positive = net tension/uplift), kN.
    pub max_column_tension_kn: f64,
    /// Largest local beam deflection along any span, mm.
    pub max_beam_deflection_mm: f64,
    /// Largest horizontal displacement magnitude at beam level, mm.
    pub max_lateral_drift_mm: f64,
}

/// Governing ELU and ELS envelopes across all NBR 8681 combinations for one frame (#105).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameEnvelope {
    pub elu_max_beam_pos_knm: f64,
    pub elu_max_beam_pos_combo: String,
    pub elu_min_beam_neg_knm: f64,
    pub elu_min_beam_neg_combo: String,
    pub elu_max_beam_shear_kn: f64,
    pub elu_max_beam_shear_combo: String,
    pub elu_max_column_moment_knm: f64,
    pub elu_max_column_moment_combo: String,
    pub elu_max_column_compression_kn: f64,
    pub elu_max_column_compression_combo: String,
    /// Largest signed column axial force (`> 0` indicates net tension / uplift under wind).
    pub elu_min_column_axial_kn: f64,
    pub elu_min_column_axial_combo: String,
    pub has_column_uplift: bool,
    pub els_qp_max_beam_deflection_mm: f64,
    pub els_f_max_drift_mm: f64,
    pub els_f_drift_combo: String,
}

/// Sampled deformed geometry & bending moment diagram for one member,
/// stored in the frame's local 2D plane `(x, y)` for SVG/UI rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct DeformedMember {
    pub label: String,
    pub kind: MemberKind,
    pub samples: Vec<MemberSample>,
}

/// Support reaction badge in the frame's 2D plane `(x, y)` for diagrams.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameReactionPoint {
    pub label: String,
    pub x_m: f64,
    pub y_m: f64,
    pub support: SupportKind,
    pub rx_kn: f64,
    pub ry_kn: f64,
    pub mz_knm: f64,
}

/// Complete 2D plane-frame analysis result for one continuous beam line.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLineResult {
    pub label: String,
    pub rigid_joints: bool,
    pub fixed_bases: bool,
    /// True only when a verified physical bracing system is included in the model.
    /// The current builder does not model such a system and leaves this false.
    pub braced_sway: bool,
    pub total_length_m: f64,
    pub story_height_m: f64,
    /// Frequent service horizontal wind force (`ψ1·Wk`) at beam level (NBR 6123 / NBR 6118), kN.
    pub lateral_wind_kn: f64,
    /// Full characteristic horizontal wind force (`Wk`) assigned to this frame from NBR 6123, kN.
    pub characteristic_wind_kn: f64,
    pub beams: Vec<FrameBeamForces>,
    pub columns: Vec<FrameColumnForces>,
    /// Largest horizontal displacement magnitude at beam level under ELS-F (`ψ1·W`), mm.
    pub max_lateral_drift_mm: f64,
    /// NBR 6118 Table 13.3 inter-storey service drift limit (`H / 850`), mm.
    pub drift_limit_mm: f64,
    /// Global equilibrium check residuals (`max(|ΣFx|, |ΣFy|)` and `|ΣM|`).
    pub equilibrium_residual_kn: f64,
    pub equilibrium_residual_knm: f64,
    pub reactions: Vec<FrameReactionPoint>,
    pub members: Vec<DeformedMember>,
    /// Fundamental NBR 6120 / NBR 6123 load cases (`G1`, `G2`, `Q`, `W+`, `W−`).
    pub load_cases: Vec<FrameLoadCaseResult>,
    /// All NBR 8681 / NBR 6118 ELU and ELS combinations solved by exact superposition.
    pub combinations: Vec<FrameCombinationResult>,
    /// Governing ELU and ELS envelopes across all combinations.
    pub envelope: FrameEnvelope,
}

/// A portal-frame line that was eligible for analysis but could not be
/// solved. Keeping the original solver reason is part of the safety contract:
/// an absent result must never look like an absent model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameAnalysisFailure {
    pub label: String,
    pub reason: String,
}

/// Complete outcome of project frame analysis, including partial success.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectFrameAnalysis {
    pub frames: Vec<FrameLineResult>,
    pub failures: Vec<FrameAnalysisFailure>,
}

impl FrameLineResult {
    pub fn drift_ok(&self) -> bool {
        self.max_lateral_drift_mm <= self.drift_limit_mm + 1e-6
    }

    pub fn max_column_moment_knm(&self) -> f64 {
        self.columns
            .iter()
            .map(|c| c.max_moment_knm)
            .fold(0.0_f64, f64::max)
    }

    pub fn max_beam_moment_knm(&self) -> f64 {
        self.beams
            .iter()
            .map(|b| {
                b.moment_left_knm
                    .abs()
                    .max(b.moment_right_knm.abs())
                    .max(b.moment_span_knm.abs())
            })
            .fold(0.0_f64, f64::max)
    }

    pub fn total_point_load_kn(&self) -> f64 {
        self.beams
            .iter()
            .flat_map(|beam| &beam.point_loads)
            .map(|load| load.load_kn)
            .sum()
    }
}

/// Exact linear combination of four solved `FrameSolution` instances (`G1, G2, Q, W+`).
fn combine_solutions(
    g1: &FrameSolution,
    g2: &FrameSolution,
    q: &FrameSolution,
    w: &FrameSolution,
    factors: &CombinationFactors,
    frame: &Frame2D,
) -> FrameSolution {
    let c = |a: f64, b: f64, d: f64, e: f64| factors.combine(a, b, d, e);
    let displacements = (0..g1.displacements.len())
        .map(|i| super::solver::NodalDisplacement {
            u_m: c(
                g1.displacements[i].u_m,
                g2.displacements[i].u_m,
                q.displacements[i].u_m,
                w.displacements[i].u_m,
            ),
            v_m: c(
                g1.displacements[i].v_m,
                g2.displacements[i].v_m,
                q.displacements[i].v_m,
                w.displacements[i].v_m,
            ),
            theta_rad: c(
                g1.displacements[i].theta_rad,
                g2.displacements[i].theta_rad,
                q.displacements[i].theta_rad,
                w.displacements[i].theta_rad,
            ),
        })
        .collect();

    let reactions = (0..g1.reactions.len())
        .map(|i| super::solver::SupportReaction {
            node: g1.reactions[i].node,
            rx_kn: c(
                g1.reactions[i].rx_kn,
                g2.reactions[i].rx_kn,
                q.reactions[i].rx_kn,
                w.reactions[i].rx_kn,
            ),
            ry_kn: c(
                g1.reactions[i].ry_kn,
                g2.reactions[i].ry_kn,
                q.reactions[i].ry_kn,
                w.reactions[i].ry_kn,
            ),
            mz_knm: c(
                g1.reactions[i].mz_knm,
                g2.reactions[i].mz_knm,
                q.reactions[i].mz_knm,
                w.reactions[i].mz_knm,
            ),
        })
        .collect();

    let members = (0..g1.members.len())
        .map(|mi| {
            let m1 = &g1.members[mi];
            let m2 = &g2.members[mi];
            let mq = &q.members[mi];
            let mw = &w.members[mi];
            let local_end_forces = std::array::from_fn(|k| {
                c(
                    m1.local_end_forces[k],
                    m2.local_end_forces[k],
                    mq.local_end_forces[k],
                    mw.local_end_forces[k],
                )
            });
            let fm = &frame.members[mi];
            let n_start = &frame.nodes[fm.start];
            let n_end = &frame.nodes[fm.end];
            let dx = n_end.x - n_start.x;
            let dy = n_end.y - n_start.y;
            let len = dx.hypot(dy).max(1e-12);
            let cos_a = dx / len;
            let sin_a = dy / len;

            let n_samples = m1.samples.len();
            let mut samples = Vec::with_capacity(n_samples);
            let mut max_sagging = f64::NEG_INFINITY;
            let mut max_abs_moment = 0.0_f64;
            let mut max_abs_shear = 0.0_f64;

            for si in 0..n_samples {
                let s1 = m1.samples[si];
                let s2 = m2.samples[si];
                let sq = mq.samples[si];
                let sw = mw.samples[si];
                let dx_m = c(s1.dx_m, s2.dx_m, sq.dx_m, sw.dx_m);
                let dy_m = c(s1.dy_m, s2.dy_m, sq.dy_m, sw.dy_m);
                let axial_kn = c(s1.axial_kn, s2.axial_kn, sq.axial_kn, sw.axial_kn);
                let shear_kn = c(s1.shear_kn, s2.shear_kn, sq.shear_kn, sw.shear_kn);
                let moment_knm = c(s1.moment_knm, s2.moment_knm, sq.moment_knm, sw.moment_knm);
                max_sagging = max_sagging.max(moment_knm);
                max_abs_moment = max_abs_moment.max(moment_knm.abs());
                max_abs_shear = max_abs_shear.max(shear_kn.abs());
                samples.push(MemberSample {
                    s_m: s1.s_m,
                    x0: s1.x0,
                    y0: s1.y0,
                    dx_m,
                    dy_m,
                    axial_kn,
                    shear_kn,
                    moment_knm,
                });
            }

            // Compute exact local transverse deflection relative to member chord.
            let mut max_local_def_m = 0.0_f64;
            if let (Some(first), Some(last)) = (samples.first(), samples.last()) {
                let v0 = -sin_a * first.dx_m + cos_a * first.dy_m;
                let v1 = -sin_a * last.dx_m + cos_a * last.dy_m;
                for s in &samples {
                    let xi = (s.s_m / len).clamp(0.0, 1.0);
                    let v_loc = -sin_a * s.dx_m + cos_a * s.dy_m;
                    let v_chord = (1.0 - xi) * v0 + xi * v1;
                    max_local_def_m = max_local_def_m.max((v_loc - v_chord).abs());
                }
            }

            super::solver::MemberResult {
                member: m1.member,
                label: m1.label.clone(),
                kind: m1.kind,
                length_m: m1.length_m,
                local_end_forces,
                moment_start_knm: -local_end_forces[2],
                moment_end_knm: local_end_forces[5],
                max_sagging_knm: max_sagging.max(0.0),
                max_abs_moment_knm: max_abs_moment,
                max_abs_shear_kn: max_abs_shear,
                axial_kn: -local_end_forces[0],
                max_local_deflection_mm: max_local_def_m * 1000.0,
                samples,
            }
        })
        .collect();

    let equilibrium = super::solver::EquilibriumResidual {
        sum_fx_kn: c(
            g1.equilibrium.sum_fx_kn,
            g2.equilibrium.sum_fx_kn,
            q.equilibrium.sum_fx_kn,
            w.equilibrium.sum_fx_kn,
        ),
        sum_fy_kn: c(
            g1.equilibrium.sum_fy_kn,
            g2.equilibrium.sum_fy_kn,
            q.equilibrium.sum_fy_kn,
            w.equilibrium.sum_fy_kn,
        ),
        sum_mz_knm: c(
            g1.equilibrium.sum_mz_knm,
            g2.equilibrium.sum_mz_knm,
            q.equilibrium.sum_mz_knm,
            w.equilibrium.sum_mz_knm,
        ),
    };

    FrameSolution {
        displacements,
        reactions,
        members,
        equilibrium,
    }
}

fn summarize_load_case(
    kind: LoadCaseKind,
    sol: &FrameSolution,
    top_nodes: &[usize],
    sign: f64,
) -> FrameLoadCaseResult {
    let total_vertical_kn: f64 = sol.reactions.iter().map(|r| sign * r.ry_kn).sum();
    let total_horizontal_kn: f64 = sol.reactions.iter().map(|r| -sign * r.rx_kn).sum();
    let max_beam_moment_knm = sol
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Beam)
        .map(|m| m.max_abs_moment_knm)
        .fold(0.0_f64, f64::max);
    let max_column_moment_knm = sol
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Column)
        .map(|m| m.max_abs_moment_knm)
        .fold(0.0_f64, f64::max);
    let max_lateral_drift_mm = top_nodes
        .iter()
        .map(|&nid| sol.displacements[nid].u_m.abs() * 1000.0)
        .fold(0.0_f64, f64::max);

    FrameLoadCaseResult {
        kind,
        total_vertical_kn,
        total_horizontal_kn,
        max_beam_moment_knm,
        max_column_moment_knm,
        max_lateral_drift_mm,
    }
}

fn summarize_combination(
    factors: &CombinationFactors,
    sol: &FrameSolution,
    top_nodes: &[usize],
) -> FrameCombinationResult {
    let mut max_beam_pos_knm = 0.0_f64;
    let mut min_beam_neg_knm = 0.0_f64;
    let mut max_beam_shear_kn = 0.0_f64;
    let mut max_beam_deflection_mm = 0.0_f64;
    let mut max_column_moment_knm = 0.0_f64;
    let mut max_column_compression_kn = 0.0_f64;
    let mut max_column_tension_kn = f64::NEG_INFINITY;

    for m in &sol.members {
        match m.kind {
            MemberKind::Beam => {
                max_beam_pos_knm = max_beam_pos_knm.max(m.max_sagging_knm);
                for s in &m.samples {
                    min_beam_neg_knm = min_beam_neg_knm.min(s.moment_knm);
                }
                max_beam_shear_kn = max_beam_shear_kn.max(m.max_abs_shear_kn);
                max_beam_deflection_mm = max_beam_deflection_mm.max(m.max_local_deflection_mm);
            }
            MemberKind::Column => {
                max_column_moment_knm = max_column_moment_knm.max(m.max_abs_moment_knm);
                max_column_compression_kn = max_column_compression_kn.max((-m.axial_kn).max(0.0));
                max_column_tension_kn = max_column_tension_kn.max(m.axial_kn);
            }
        }
    }
    if !max_column_tension_kn.is_finite() {
        max_column_tension_kn = 0.0;
    }

    let max_lateral_drift_mm = top_nodes
        .iter()
        .map(|&nid| sol.displacements[nid].u_m.abs() * 1000.0)
        .fold(0.0_f64, f64::max);

    FrameCombinationResult {
        code: factors.code,
        name: factors.name,
        category: factors.category,
        formula: factors.formula.clone(),
        max_beam_pos_knm,
        min_beam_neg_knm,
        max_beam_shear_kn,
        max_column_moment_knm,
        max_column_compression_kn,
        max_column_tension_kn,
        max_beam_deflection_mm,
        max_lateral_drift_mm,
    }
}

/// Analyzes every continuous beam line of `project` as a coupled 2D portal
/// frame (beams + columns together) using the Direct Stiffness Method,
/// solving separate NBR 6120 / NBR 6123 load cases and combining them into
/// the full NBR 8681 / NBR 6118 ELU and ELS envelopes.
pub fn analyze_project_frames(project: &Project, report: &StructureReport) -> ProjectFrameAnalysis {
    let settings = project.structure.settings;
    let e_c = settings.modulus_kn_m2();
    let lines = project.beam_lines();
    if lines.is_empty() || report.beams.is_empty() {
        return ProjectFrameAnalysis::default();
    }

    let col_results: HashMap<u64, &crate::domain::structure::ColumnResult> =
        report.columns.iter().map(|c| (c.id, c)).collect();
    let col_fixtures: HashMap<u64, (f64, f64)> = project
        .fixtures
        .iter()
        .filter(|f| f.kind == FixtureKind::Column)
        .map(|f| {
            let rotated = f.quarter_turns % 2 != 0;
            let sx = if rotated { f.depth } else { f.width };
            let sy = if rotated { f.width } else { f.depth };
            (f.id, (sx.max(0.10), sy.max(0.10)))
        })
        .collect();
    let span_results: HashMap<u64, &crate::domain::structure::SpanResult> = report
        .beams
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|s| (s.beam, s))
        .collect();
    let labels = project.structure_labels();

    // Count eligible column-supported frames along X and along Y so building-level
    // NBR 6123 diaphragm shear distributes conservatively across parallel frames.
    let mut frames_along_x = 0usize;
    let mut frames_along_y = 0usize;
    let mut line_along_x = Vec::with_capacity(lines.len());
    for line in &lines {
        let mut sum_dx = 0.0;
        let mut sum_dy = 0.0;
        for &beam_id in &line.spans {
            if let Some(beam) = project.structure.beams.iter().find(|b| b.id == beam_id)
                && let Some((a, b)) = project.beam_endpoints(beam)
            {
                sum_dx += (b.x - a.x).abs();
                sum_dy += (b.y - a.y).abs();
            }
        }
        let is_x = sum_dx >= sum_dy;
        line_along_x.push(is_x);
        let eligible = !line.spans.is_empty()
            && line.supports.len() == line.spans.len() + 1
            && line
                .supports
                .iter()
                .any(|s| matches!(s, BeamSupport::Column { .. }));
        if eligible {
            if is_x {
                frames_along_x += 1;
            } else {
                frames_along_y += 1;
            }
        }
    }

    let bwind = &report.loads.building_wind;
    let combo_specs = build_nbr8681_combinations(settings.occupancy);
    let rigid_joints = settings.frame_rigid_joints;
    let fixed_bases = settings.frame_fixed_bases;
    let mut analysis = ProjectFrameAnalysis::default();

    for (line_idx, line) in lines.iter().enumerate() {
        let frame_label = format!("Pórtico {}", line.label);
        if line.spans.is_empty() {
            continue;
        }
        if line.supports.len() != line.spans.len() + 1 {
            analysis.failures.push(FrameAnalysisFailure {
                label: frame_label,
                reason: format!(
                    "topologia inconsistente: {} vão(s) e {} apoio(s)",
                    line.spans.len(),
                    line.supports.len()
                ),
            });
            continue;
        }
        // Only build a 2D portal frame when at least one column supports the line.
        let has_column = line
            .supports
            .iter()
            .any(|s| matches!(s, BeamSupport::Column { .. }));
        if !has_column {
            continue;
        }

        // This topology has a lateral mechanism regardless of the applied load.
        // Reject it explicitly: roundoff in a singular stiffness matrix must not
        // make an unmodelled bracing system appear to exist.
        if !rigid_joints && !fixed_bases {
            analysis.failures.push(FrameAnalysisFailure {
                label: frame_label,
                reason: "pórtico hipostático: bases e ligações articuladas, sem contraventamento físico modelado"
                    .to_owned(),
            });
            continue;
        }

        let along_x = line_along_x[line_idx];

        // Gather span lengths & stations.
        let mut span_lengths = Vec::with_capacity(line.spans.len());
        let mut stations = Vec::with_capacity(line.supports.len());
        stations.push(0.0_f64);
        let mut cursor = 0.0_f64;
        for &beam_id in &line.spans {
            let len = span_results
                .get(&beam_id)
                .map(|s| s.length_m)
                .or_else(|| {
                    project
                        .structure
                        .beams
                        .iter()
                        .find(|b| b.id == beam_id)
                        .and_then(|b| project.beam_endpoints(b))
                        .map(|(a, b)| a.distance(b))
                })
                .unwrap_or(0.0);
            if len < 0.05 {
                cursor = -1.0;
                break;
            }
            cursor += len;
            span_lengths.push(len);
            stations.push(cursor);
        }
        if cursor <= 0.0 {
            analysis.failures.push(FrameAnalysisFailure {
                label: frame_label,
                reason: "contém vão nulo ou menor que 5 cm".to_owned(),
            });
            continue;
        }

        // Story height from the columns in this line (fallback to project.wall_height).
        let story_height = line
            .supports
            .iter()
            .filter_map(|sup| match sup {
                BeamSupport::Column { id } => col_results.get(id).map(|c| c.height_m),
                BeamSupport::Beam { .. } => None,
            })
            .fold(0.0_f64, f64::max)
            .max(project.wall_height.max(2.0));

        // Assemble shared frame geometry once.
        let mut geom = Frame2D::default();
        let mut top_nodes = Vec::with_capacity(line.supports.len());
        let mut column_meta: Vec<(usize, u64, String, usize, f64, f64, f64)> = Vec::new();

        for (idx, sup) in line.supports.iter().enumerate() {
            let x = stations[idx];
            match *sup {
                BeamSupport::Column { id } => {
                    let col_label = labels
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| format!("P{}", idx + 1));
                    // Roof/masonry checks do not supply a physical frame restraint.
                    // Keep sway free so an unbraced mechanism reaches the solver.
                    let top_id =
                        geom.add_node(format!("{col_label}t"), x, story_height, SupportKind::Free);
                    let base_support = if fixed_bases {
                        SupportKind::Fixed
                    } else {
                        SupportKind::Pinned
                    };
                    let base_id = geom.add_node(col_label.clone(), x, 0.0, base_support);

                    let (sx, sy) = col_fixtures
                        .get(&id)
                        .copied()
                        .or_else(|| col_results.get(&id).map(|c| (c.width_m, c.depth_m)))
                        .unwrap_or((0.14, 0.20));
                    let (h_inplane, b_out) = if along_x { (sx, sy) } else { (sy, sx) };
                    let area = b_out * h_inplane;
                    let inertia = b_out * h_inplane.powi(3) / 12.0;
                    let col_release = if rigid_joints {
                        EndRelease::Rigid
                    } else {
                        EndRelease::PinnedEnd
                    };
                    let member_id = geom.add_member(
                        col_label.clone(),
                        MemberKind::Column,
                        base_id,
                        top_id,
                        area,
                        inertia,
                        e_c,
                        col_release,
                    );
                    top_nodes.push(top_id);
                    column_meta.push((member_id, id, col_label, base_id, story_height, sx, sy));
                }
                BeamSupport::Beam { .. } => {
                    let top_id = geom.add_node(
                        format!("A{}", idx + 1),
                        x,
                        story_height,
                        SupportKind::RollerY,
                    );
                    top_nodes.push(top_id);
                }
            }
        }

        // Create beam members on the base geometry, then clone into 4 load cases: G1, G2, Q, W+.
        let mut beam_meta: Vec<(usize, u64, String, f64, f64, Vec<SpanPointLoad>)> = Vec::new();
        let mut beam_loads_split: Vec<(usize, f64, f64, f64, Vec<SpanPointLoad>)> = Vec::new();

        for (i, &beam_id) in line.spans.iter().enumerate() {
            let span = span_results.get(&beam_id);
            let b_w = span.map(|s| s.width_m).unwrap_or(0.14).max(0.10);
            let h_b = span.map(|s| s.height_m).unwrap_or(0.30).max(0.15);
            let g1 = span.map(|s| s.g1_dead_kn_m).unwrap_or(0.0).max(0.0);
            let g2 = span.map(|s| s.g2_dead_kn_m).unwrap_or(0.0).max(0.0);
            let q_live = span.map(|s| s.live_kn_m).unwrap_or(0.0).max(0.0);
            let load_kn_m = (g1 + g2 + q_live).max(0.0);
            let area = b_w * h_b;
            let inertia = b_w * h_b.powi(3) / 12.0;
            let span_label = if line.spans.len() == 1 {
                line.label.clone()
            } else {
                format!("{}.{}", line.label, i + 1)
            };
            let member_id = geom.add_member(
                span_label.clone(),
                MemberKind::Beam,
                top_nodes[i],
                top_nodes[i + 1],
                area,
                inertia,
                e_c,
                EndRelease::Rigid,
            );
            let point_loads = span.map(|s| s.point_loads.clone()).unwrap_or_default();
            beam_loads_split.push((member_id, g1, g2, q_live, point_loads.clone()));
            beam_meta.push((
                member_id,
                beam_id,
                span_label,
                span_lengths[i],
                load_kn_m,
                point_loads,
            ));
        }

        let mut frame_g1 = geom.clone();
        let mut frame_g2 = geom.clone();
        let mut frame_q = geom.clone();
        let mut frame_w = geom.clone();

        for (member_id, g1, g2, q_live, point_loads) in &beam_loads_split {
            frame_g1.add_uniform_load(*member_id, -*g1);
            frame_g2.add_uniform_load(*member_id, -*g2);
            frame_q.add_uniform_load(*member_id, -*q_live);
            for point in point_loads {
                let p_dead = point.load_kn * point.quasi_factor.clamp(0.0, 1.0);
                let p_live = point.load_kn - p_dead;
                frame_g2.add_point_load(*member_id, point.position_m, -p_dead);
                frame_q.add_point_load(*member_id, point.position_m, -p_live);
            }
        }

        // Characteristic NBR 6123 horizontal wind force assigned to this frame at beam level.
        let characteristic_wind_kn = if !bwind.directions.is_empty() {
            if along_x {
                bwind.diaphragm_shear_x_kn() / frames_along_x.max(1) as f64
            } else {
                bwind.diaphragm_shear_y_kn() / frames_along_y.max(1) as f64
            }
        } else {
            let q_wind_kpa = project
                .structure
                .wind
                .pressure_kpa(story_height, cursor.max(story_height));
            q_wind_kpa * (story_height * 0.5) * 2.0 * 1.2
        };
        if characteristic_wind_kn > 1e-6 {
            frame_w.add_nodal_load(top_nodes[0], characteristic_wind_kn, 0.0, 0.0);
        }
        let lateral_wind_kn = PSI1_WIND * characteristic_wind_kn;

        // Solve the four fundamental load cases.
        let (sol_g1, sol_g2, sol_q, sol_w) = match (
            solve_frame(&frame_g1),
            solve_frame(&frame_g2),
            solve_frame(&frame_q),
            solve_frame(&frame_w),
        ) {
            (Ok(s1), Ok(s2), Ok(sq), Ok(sw)) => (s1, s2, sq, sw),
            (Err(reason), _, _, _)
            | (_, Err(reason), _, _)
            | (_, _, Err(reason), _)
            | (_, _, _, Err(reason)) => {
                analysis.failures.push(FrameAnalysisFailure {
                    label: frame_label,
                    reason,
                });
                continue;
            }
        };

        let load_cases = vec![
            summarize_load_case(LoadCaseKind::G1, &sol_g1, &top_nodes, 1.0),
            summarize_load_case(LoadCaseKind::G2, &sol_g2, &top_nodes, 1.0),
            summarize_load_case(LoadCaseKind::Q, &sol_q, &top_nodes, 1.0),
            summarize_load_case(LoadCaseKind::WindPos, &sol_w, &top_nodes, 1.0),
            summarize_load_case(LoadCaseKind::WindNeg, &sol_w, &top_nodes, -1.0),
        ];

        // Evaluate every NBR 8681 combination via exact linear superposition.
        let mut combinations = Vec::with_capacity(combo_specs.len());
        let mut beam_envelopes = vec![FrameBeamEnvelope::default(); beam_meta.len()];
        let mut primary_service_sol: Option<FrameSolution> = None;
        let mut envelope = FrameEnvelope {
            elu_min_column_axial_kn: f64::NEG_INFINITY,
            ..FrameEnvelope::default()
        };

        for factors in &combo_specs {
            let combined = combine_solutions(&sol_g1, &sol_g2, &sol_q, &sol_w, factors, &geom);
            let summary = summarize_combination(factors, &combined, &top_nodes);

            if factors.category.is_ultimate() {
                for (beam_envelope, (mid, ..)) in beam_envelopes.iter_mut().zip(&beam_meta) {
                    let member = &combined.members[*mid];
                    if member.max_sagging_knm >= beam_envelope.max_positive_moment_knm {
                        beam_envelope.max_positive_moment_knm = member.max_sagging_knm;
                        beam_envelope.positive_combination = factors.code.to_owned();
                    }
                    let negative = member
                        .samples
                        .iter()
                        .map(|s| s.moment_knm)
                        .fold(0.0, f64::min);
                    if negative <= beam_envelope.min_negative_moment_knm {
                        beam_envelope.min_negative_moment_knm = negative;
                        beam_envelope.negative_combination = factors.code.to_owned();
                    }
                    if member.max_abs_shear_kn >= beam_envelope.max_shear_kn {
                        beam_envelope.max_shear_kn = member.max_abs_shear_kn;
                        beam_envelope.shear_combination = factors.code.to_owned();
                    }
                }
                if summary.max_beam_pos_knm >= envelope.elu_max_beam_pos_knm {
                    envelope.elu_max_beam_pos_knm = summary.max_beam_pos_knm;
                    envelope.elu_max_beam_pos_combo = factors.code.to_owned();
                }
                if summary.min_beam_neg_knm <= envelope.elu_min_beam_neg_knm {
                    envelope.elu_min_beam_neg_knm = summary.min_beam_neg_knm;
                    envelope.elu_min_beam_neg_combo = factors.code.to_owned();
                }
                if summary.max_beam_shear_kn >= envelope.elu_max_beam_shear_kn {
                    envelope.elu_max_beam_shear_kn = summary.max_beam_shear_kn;
                    envelope.elu_max_beam_shear_combo = factors.code.to_owned();
                }
                if summary.max_column_moment_knm >= envelope.elu_max_column_moment_knm {
                    envelope.elu_max_column_moment_knm = summary.max_column_moment_knm;
                    envelope.elu_max_column_moment_combo = factors.code.to_owned();
                }
                if summary.max_column_compression_kn >= envelope.elu_max_column_compression_kn {
                    envelope.elu_max_column_compression_kn = summary.max_column_compression_kn;
                    envelope.elu_max_column_compression_combo = factors.code.to_owned();
                }
                if summary.max_column_tension_kn > envelope.elu_min_column_axial_kn {
                    envelope.elu_min_column_axial_kn = summary.max_column_tension_kn;
                    envelope.elu_min_column_axial_combo = factors.code.to_owned();
                }
            } else if factors.category == CombinationCategory::ElsQuasiPermanent {
                envelope.els_qp_max_beam_deflection_mm = summary.max_beam_deflection_mm;
            } else if factors.category == CombinationCategory::ElsFrequent
                && summary.max_lateral_drift_mm >= envelope.els_f_max_drift_mm
            {
                envelope.els_f_max_drift_mm = summary.max_lateral_drift_mm;
                envelope.els_f_drift_combo = factors.code.to_owned();
            }

            if factors.code == "ELS-R1" {
                primary_service_sol = Some(combined);
            }
            combinations.push(summary);
        }
        if !envelope.elu_min_column_axial_kn.is_finite() {
            envelope.elu_min_column_axial_kn = 0.0;
        }
        envelope.has_column_uplift = envelope.elu_min_column_axial_kn > 1e-3;

        // Primary displayed service solution (`ELS-R1`: G1 + G2 + Q + ψ1·W+).
        let sol = primary_service_sol.unwrap_or_else(|| {
            combine_solutions(&sol_g1, &sol_g2, &sol_q, &sol_w, &combo_specs[0], &geom)
        });

        let reaction_map: HashMap<usize, (f64, f64, f64)> = sol
            .reactions
            .iter()
            .map(|r| (r.node, (r.rx_kn, r.ry_kn, r.mz_knm)))
            .collect();

        let beams: Vec<FrameBeamForces> = beam_meta
            .iter()
            .zip(beam_envelopes)
            .map(|((mid, beam_id, label, len, q, point_loads), ultimate)| {
                let m = &sol.members[*mid];
                FrameBeamForces {
                    beam_id: *beam_id,
                    label: label.clone(),
                    length_m: *len,
                    load_kn_m: *q,
                    point_loads: point_loads.clone(),
                    moment_left_knm: m.moment_start_knm,
                    moment_span_knm: m.max_sagging_knm,
                    moment_right_knm: m.moment_end_knm,
                    max_shear_kn: m.max_abs_shear_kn,
                    axial_kn: m.axial_kn,
                    elastic_deflection_mm: m.max_local_deflection_mm,
                    ultimate,
                }
            })
            .collect();

        let columns: Vec<FrameColumnForces> = column_meta
            .iter()
            .map(|(mid, col_id, label, base_node, h, sx, sy)| {
                let m = &sol.members[*mid];
                let (rx, ry, mz) = reaction_map
                    .get(base_node)
                    .copied()
                    .unwrap_or((0.0, 0.0, 0.0));
                FrameColumnForces {
                    column_id: *col_id,
                    label: label.clone(),
                    height_m: *h,
                    width_m: *sx,
                    depth_m: *sy,
                    axial_kn: m.axial_kn,
                    shear_kn: m.max_abs_shear_kn,
                    moment_base_knm: m.moment_start_knm,
                    moment_top_knm: m.moment_end_knm,
                    max_moment_knm: m.max_abs_moment_knm,
                    reaction_rx_kn: rx,
                    reaction_ry_kn: ry,
                    reaction_mz_knm: mz,
                }
            })
            .collect();

        let max_lateral_drift_mm = envelope.els_f_max_drift_mm;
        let drift_limit_mm = story_height * 1000.0 / 850.0;

        let reactions: Vec<FrameReactionPoint> = sol
            .reactions
            .iter()
            .map(|r| {
                let node = &geom.nodes[r.node];
                FrameReactionPoint {
                    label: node.label.clone(),
                    x_m: node.x,
                    y_m: node.y,
                    support: node.support,
                    rx_kn: r.rx_kn,
                    ry_kn: r.ry_kn,
                    mz_knm: r.mz_knm,
                }
            })
            .collect();

        let members: Vec<DeformedMember> = sol
            .members
            .into_iter()
            .map(|m| DeformedMember {
                label: m.label,
                kind: m.kind,
                samples: m.samples,
            })
            .collect();

        analysis.frames.push(FrameLineResult {
            label: frame_label,
            rigid_joints,
            fixed_bases,
            braced_sway: false,
            total_length_m: cursor,
            story_height_m: story_height,
            lateral_wind_kn,
            characteristic_wind_kn,
            beams,
            columns,
            max_lateral_drift_mm,
            drift_limit_mm,
            equilibrium_residual_kn: sol
                .equilibrium
                .sum_fx_kn
                .abs()
                .max(sol.equilibrium.sum_fy_kn.abs()),
            equilibrium_residual_knm: sol.equilibrium.sum_mz_knm.abs(),
            reactions,
            members,
            load_cases,
            combinations,
            envelope,
        });
    }

    analysis
}
