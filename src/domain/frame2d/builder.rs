//! Builds and solves a 2D plane frame (`Frame2D`) for each continuous beam
//! line in a `Project`, incorporating real column axial & flexural stiffness,
//! rigid or pinned beam-column joints, fixed or pinned column bases, beam
//! gravity loads, concentrated loads, and NBR 6123 service lateral wind.

use std::collections::HashMap;

use crate::domain::{
    fixtures::FixtureKind,
    model::Project,
    structure::{BeamSupport, SpanPointLoad, StructureReport},
};

use super::{
    model::{EndRelease, Frame2D, MemberKind, SupportKind},
    solver::{MemberSample, solve_frame},
};

/// Internal forces in one beam span from the 2D plane-frame analysis.
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
    /// True when both joints and bases are pinned, requiring lateral bracing
    /// (e.g. masonry shear walls) to stabilize horizontal sway.
    pub braced_sway: bool,
    pub total_length_m: f64,
    pub story_height_m: f64,
    /// Service horizontal wind force applied at beam level (NBR 6123), kN.
    pub lateral_wind_kn: f64,
    pub beams: Vec<FrameBeamForces>,
    pub columns: Vec<FrameColumnForces>,
    /// Largest horizontal displacement magnitude at beam level, mm.
    pub max_lateral_drift_mm: f64,
    /// NBR 6118 Table 13.3 inter-storey service drift limit (`H / 850`), mm.
    pub drift_limit_mm: f64,
    /// Global equilibrium check residuals (`max(|ΣFx|, |ΣFy|)` and `|ΣM|`).
    pub equilibrium_residual_kn: f64,
    pub equilibrium_residual_knm: f64,
    pub reactions: Vec<FrameReactionPoint>,
    pub members: Vec<DeformedMember>,
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

/// Analyzes every continuous beam line of `project` as a coupled 2D portal
/// frame (beams + columns together) using the Direct Stiffness Method.
pub fn analyze_project_frames(project: &Project, report: &StructureReport) -> Vec<FrameLineResult> {
    let settings = project.structure.settings;
    let e_c = settings.modulus_kn_m2();
    let lines = project.beam_lines();
    if lines.is_empty() || report.beams.is_empty() {
        return Vec::new();
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

    // Estimate average perpendicular spacing between frames for service wind tributary width.
    let wind_tributary_m = if !project.rooms.is_empty() {
        let mut sum = 0.0;
        let mut count = 0usize;
        for room in &project.rooms {
            if let Some((min, max)) = project.room_bounds(room) {
                sum += ((max.x - min.x).abs() + (max.y - min.y).abs()) * 0.5;
                count += 1;
            }
        }
        if count > 0 {
            (sum / count as f64 * 0.5).clamp(1.2, 3.5)
        } else {
            2.0
        }
    } else {
        2.0
    };

    let rigid_joints = settings.frame_rigid_joints;
    let fixed_bases = settings.frame_fixed_bases;
    let mut results = Vec::new();

    for line in &lines {
        if line.spans.is_empty() || line.supports.len() != line.spans.len() + 1 {
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

        // Determine frame orientation in plan (mostly X vs mostly Y) for column in-plane inertia.
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
        let along_x = sum_dx >= sum_dy;

        // Gather span lengths & heights.
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

        let mut frame = Frame2D::default();
        let mut top_nodes = Vec::with_capacity(line.supports.len());
        let mut column_meta: Vec<(usize, u64, String, usize, f64, f64, f64)> = Vec::new();
        let braced_sway = !rigid_joints && !fixed_bases;
        let mut sway_brace_placed = false;

        // Create beam-level nodes and columns beneath each column support.
        for (idx, sup) in line.supports.iter().enumerate() {
            let x = stations[idx];
            match *sup {
                BeamSupport::Column { id } => {
                    let col_label = labels
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| format!("P{}", idx + 1));
                    let top_support = if braced_sway && !sway_brace_placed {
                        sway_brace_placed = true;
                        SupportKind::RollerX
                    } else {
                        SupportKind::Free
                    };
                    let top_id =
                        frame.add_node(format!("{col_label}t"), x, story_height, top_support);
                    let base_support = if fixed_bases {
                        SupportKind::Fixed
                    } else {
                        SupportKind::Pinned
                    };
                    let base_id = frame.add_node(col_label.clone(), x, 0.0, base_support);

                    let (sx, sy) = col_fixtures
                        .get(&id)
                        .copied()
                        .or_else(|| col_results.get(&id).map(|c| (c.width_m, c.depth_m)))
                        .unwrap_or((0.14, 0.20));
                    // In-plane dimension bends in the frame plane; out-of-plane is width b.
                    let (h_inplane, b_out) = if along_x { (sx, sy) } else { (sy, sx) };
                    let area = b_out * h_inplane;
                    let inertia = b_out * h_inplane.powi(3) / 12.0;
                    // When joints are pinned, release rotation at the top of the column
                    // so the beam stays continuous across interior supports while column top M = 0.
                    let col_release = if rigid_joints {
                        EndRelease::Rigid
                    } else {
                        EndRelease::PinnedEnd
                    };
                    let member_id = frame.add_member(
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
                    let top_id = frame.add_node(
                        format!("A{}", idx + 1),
                        x,
                        story_height,
                        SupportKind::RollerY,
                    );
                    top_nodes.push(top_id);
                }
            }
        }

        // Create beam members and apply gravity uniform loads (dead + live).
        let mut beam_meta: Vec<(usize, u64, String, f64, f64, Vec<SpanPointLoad>)> = Vec::new();
        for (i, &beam_id) in line.spans.iter().enumerate() {
            let span = span_results.get(&beam_id);
            let b_w = span.map(|s| s.width_m).unwrap_or(0.14).max(0.10);
            let h_b = span.map(|s| s.height_m).unwrap_or(0.30).max(0.15);
            let load_kn_m = span
                .map(|s| s.dead_kn_m + s.live_kn_m)
                .unwrap_or(0.0)
                .max(0.0);
            let area = b_w * h_b;
            let inertia = b_w * h_b.powi(3) / 12.0;
            let span_label = if line.spans.len() == 1 {
                line.label.clone()
            } else {
                format!("{}.{}", line.label, i + 1)
            };
            let member_id = frame.add_member(
                span_label.clone(),
                MemberKind::Beam,
                top_nodes[i],
                top_nodes[i + 1],
                area,
                inertia,
                e_c,
                EndRelease::Rigid,
            );
            frame.add_uniform_load(member_id, -load_kn_m);
            let point_loads = span.map(|s| s.point_loads.clone()).unwrap_or_default();
            for point in &point_loads {
                frame.add_point_load(member_id, point.position_m, -point.load_kn);
            }
            beam_meta.push((
                member_id,
                beam_id,
                span_label,
                span_lengths[i],
                load_kn_m,
                point_loads,
            ));
        }

        // Service horizontal wind force at beam level (NBR 6123 dynamic pressure
        // with NBR 6118 Table 13.3 frequent combination factor ψ1 = 0.30 for
        // inter-storey lateral drift H/850).
        let q_wind_kpa = project
            .structure
            .wind
            .pressure_kpa(story_height, cursor.max(story_height));
        let drag_ca = 1.2;
        let psi1_wind = 0.30;
        let lateral_wind_kn =
            psi1_wind * q_wind_kpa * (story_height * 0.5) * wind_tributary_m * drag_ca;
        if lateral_wind_kn > 1e-6 {
            frame.add_nodal_load(top_nodes[0], lateral_wind_kn, 0.0, 0.0);
        }

        let Ok(sol) = solve_frame(&frame) else {
            continue;
        };

        let reaction_map: HashMap<usize, (f64, f64, f64)> = sol
            .reactions
            .iter()
            .map(|r| (r.node, (r.rx_kn, r.ry_kn, r.mz_knm)))
            .collect();

        let beams: Vec<FrameBeamForces> = beam_meta
            .iter()
            .map(|(mid, beam_id, label, len, q, point_loads)| {
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

        let max_lateral_drift_mm = top_nodes
            .iter()
            .map(|&nid| sol.displacements[nid].u_m.abs() * 1000.0)
            .fold(0.0_f64, f64::max);
        let drift_limit_mm = story_height * 1000.0 / 850.0;

        let reactions: Vec<FrameReactionPoint> = sol
            .reactions
            .iter()
            .map(|r| {
                let node = &frame.nodes[r.node];
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

        results.push(FrameLineResult {
            label: format!("Pórtico {}", line.label),
            rigid_joints,
            fixed_bases,
            braced_sway,
            total_length_m: cursor,
            story_height_m: story_height,
            lateral_wind_kn,
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
        });
    }

    results
}
