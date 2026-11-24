//! Coupled slab + beam grillage mesh generator (#109, B06): builds a
//! conforming orthogonal grid covering rectangular, L-shaped and irregular
//! slabs together with their supporting concrete beams and columns.

use std::collections::HashMap;

use crate::domain::{
    fixtures::FixtureKind,
    geometry::{Point, nearest_point_on_segment, polygon_area, polygon_contains_point},
    model::Project,
    structure::{CONCRETE_KN_M3, SlabKind, room_live_load_kn_m2},
    verdict::Light,
};

use super::solver::{GridMember, GridMemberKind, GridNode, solve_grid};
use super::{GridAnalysisReport, GridBeamResult, GridCellHeat, GridReaction, GridSlabResult};

/// Concrete Poisson's ratio per NBR 6118:2023 §8.2.9.
pub const CONCRETE_NU: f64 = 0.20;

/// Secant modulus of elasticity of concrete `E_cs` (kPa) per NBR 6118:2023 §8.2.8:
/// `E_ci = 5600 * sqrt(fck)` (MPa), `α_i = min(0.8 + 0.2*fck/80, 1.0)`, `E_cs = α_i * E_ci`.
pub fn concrete_ecs_kpa(fck_mpa: f64) -> f64 {
    let fck = fck_mpa.max(20.0);
    let e_ci_mpa = 5600.0 * fck.sqrt();
    let alpha_i = (0.8 + 0.2 * fck / 80.0).min(1.0);
    alpha_i * e_ci_mpa * 1000.0
}

/// St. Venant torsional constant `J` (m⁴) of a rectangular section `b × h` (`b <= h`):
/// `J = β(h/b) · h · b³` with the classic Roark polynomial approximation.
pub fn rectangular_torsion_j(width_m: f64, height_m: f64) -> f64 {
    let b = width_m.min(height_m).max(1e-4);
    let h = width_m.max(height_m).max(1e-4);
    let r = b / h;
    let beta = (1.0 / 3.0) - 0.21 * r * (1.0 - r.powi(4) / 12.0);
    beta * h * b.powi(3)
}

/// Deduplicates and subdivides sorted coordinate lines so every interval is at
/// most `max_step` metres (while respecting a node budget for responsiveness).
fn build_grid_coords(raw: &[f64], target_step: f64, max_lines: usize) -> Vec<f64> {
    if raw.is_empty() {
        return Vec::new();
    }
    let mut base = raw.to_vec();
    base.sort_by(f64::total_cmp);
    let mut merged: Vec<f64> = Vec::new();
    for v in base {
        if merged
            .last()
            .map(|prev| (v - prev).abs() > 0.08)
            .unwrap_or(true)
        {
            merged.push(v);
        }
    }
    if merged.len() < 2 {
        return merged;
    }

    let (Some(&first), Some(&last)) = (merged.first(), merged.last()) else {
        return merged;
    };
    let span = last - first;
    let step = target_step.max(span / (max_lines.saturating_sub(1).max(2) as f64));
    let mut refined = Vec::new();
    for pair in merged.windows(2) {
        let a = pair[0];
        let b = pair[1];
        refined.push(a);
        let seg = b - a;
        if seg > step * 1.15 {
            let n = (seg / step).ceil() as usize;
            for k in 1..n {
                refined.push(a + seg * (k as f64) / (n as f64));
            }
        }
    }
    refined.push(last);
    refined
}

fn snap_coord_index(coords: &[f64], v: f64) -> usize {
    coords
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - v).abs().total_cmp(&(*b - v).abs()))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Solves a standalone simply-supported isotropic rectangular plate via the
/// exact same 3-DOF grillage engine (used by Timoshenko plate validation tests).
#[cfg(test)]
pub fn solve_rectangular_plate_grid(
    spans_m: (f64, f64),
    thickness_m: f64,
    e_kpa: f64,
    nu: f64,
    q_kn_m2: f64,
    divisions: (usize, usize),
) -> Result<super::solver::GridSolveOutput, String> {
    let (lx, ly) = spans_m;
    let (nx, ny) = divisions;
    let sx = lx / (nx as f64);
    let sy = ly / (ny as f64);
    let d_plate = e_kpa * thickness_m.powi(3) / (12.0 * (1.0 - nu * nu));

    let mut nodes = Vec::with_capacity((nx + 1) * (ny + 1));
    let idx = |ix: usize, iy: usize| -> usize { iy * (nx + 1) + ix };
    for iy in 0..=ny {
        for ix in 0..=nx {
            let on_edge = ix == 0 || ix == nx || iy == 0 || iy == ny;
            nodes.push(GridNode {
                id: idx(ix, iy),
                point: Point::new(ix as f64 * sx, iy as f64 * sy),
                fix_w: on_edge,
                fix_rx: false,
                fix_ry: false,
                point_load_kn: 0.0,
            });
        }
    }

    let mut members = Vec::new();
    let mut mid = 0usize;
    // X-direction strips (length sx, tributary width sy for interior, sy/2 for edge).
    // Kirchhoff plate biharmonic equivalence (Hambly 1991 §3.3):
    // EI_x/s = D, EI_y/s = D and GJ_x/s + GJ_y/s = 2D => GJ_strip = D * s.
    for iy in 0..=ny {
        let strip_w = if iy == 0 || iy == ny { sy * 0.5 } else { sy };
        let ei = d_plate * strip_w;
        let gj = d_plate * strip_w;
        let q = 0.5 * q_kn_m2 * strip_w;
        for ix in 0..nx {
            members.push(GridMember {
                id: mid,
                start: idx(ix, iy),
                end: idx(ix + 1, iy),
                kind: GridMemberKind::SlabX { slab_id: 1 },
                ei_knm2: ei,
                gj_knm2: gj,
                strip_width_m: strip_w,
                q_kn_m: q,
            });
            mid += 1;
        }
    }
    // Y-direction strips (length sy, tributary width sx for interior, sx/2 for edge).
    for ix in 0..=nx {
        let strip_w = if ix == 0 || ix == nx { sx * 0.5 } else { sx };
        let ei = d_plate * strip_w;
        let gj = d_plate * strip_w;
        let q = 0.5 * q_kn_m2 * strip_w;
        for iy in 0..ny {
            members.push(GridMember {
                id: mid,
                start: idx(ix, iy),
                end: idx(ix, iy + 1),
                kind: GridMemberKind::SlabY { slab_id: 1 },
                ei_knm2: ei,
                gj_knm2: gj,
                strip_width_m: strip_w,
                q_kn_m: q,
            });
            mid += 1;
        }
    }

    solve_grid(&nodes, &members)
}

struct BeamGridSegment {
    ix0: usize,
    iy0: usize,
    ix1: usize,
    iy1: usize,
    beam_id: u64,
    width_m: f64,
    height_m: f64,
    self_wt_kn_m: f64,
}

struct SlabMeshInfo {
    slab_id: u64,
    label: String,
    room_name: String,
    polygon: Vec<Point>,
    is_irregular: bool,
    thickness_m: f64,
    kind: SlabKind,
    q_kn_m2: f64,
    short_span_m: f64,
}

/// Builds and solves the coupled slab+beam grillage for the active floor of `project`.
pub fn analyse_project_grid(project: &Project) -> GridAnalysisReport {
    let mut report = GridAnalysisReport::default();
    if project.structure.slabs.is_empty() && project.structure.beams.is_empty() {
        return report;
    }

    let settings = project.structure.settings;
    let e_kpa = concrete_ecs_kpa(settings.fck_mpa);
    let g_kpa = e_kpa / (2.0 * (1.0 + CONCRETE_NU));
    let labels = project.structure_labels();
    let label_of = |id: u64| labels.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));

    // Collect slab polygons and surface loads.
    let mut slabs: Vec<SlabMeshInfo> = Vec::new();
    let mut raw_x: Vec<f64> = Vec::new();
    let mut raw_y: Vec<f64> = Vec::new();

    for slab in &project.structure.slabs {
        let Some(room) = project.rooms.iter().find(|r| r.id == slab.room_id) else {
            continue;
        };
        let Some(poly) = project.room_polygon(room) else {
            continue;
        };
        if poly.len() < 3 || polygon_area(&poly) < 0.25 {
            continue;
        }
        for p in &poly {
            raw_x.push(p.x);
            raw_y.push(p.y);
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
        let bbox_area = (max_x - min_x) * (max_y - min_y);
        let poly_area = polygon_area(&poly);
        let is_irregular = poly.len() > 4 || (bbox_area > 1e-4 && poly_area / bbox_area < 0.94);

        let live = if settings.per_room_live_loads {
            room_live_load_kn_m2(room.display_name(), settings.live_kn_m2)
        } else {
            settings.live_kn_m2
        };
        let q = slab.kind.self_weight(slab.thickness_m) + settings.finish_kn_m2 + live;
        let panels = crate::domain::structure::decompose_slab_panels(&poly);
        let short_span_m = panels
            .iter()
            .map(|p| p.0.min(p.1))
            .fold(0.0_f64, f64::max)
            .max((max_x - min_x).min(max_y - min_y));

        slabs.push(SlabMeshInfo {
            slab_id: slab.id,
            label: label_of(slab.id),
            room_name: room.display_name().to_owned(),
            polygon: poly,
            is_irregular,
            thickness_m: slab.thickness_m,
            kind: slab.kind,
            q_kn_m2: q,
            short_span_m,
        });
    }

    // Collect valid beams and columns.
    let mut beams_geom = Vec::new();
    for beam in project.valid_beams() {
        if let Some((a, b)) = project.beam_endpoints(beam) {
            raw_x.push(a.x);
            raw_x.push(b.x);
            raw_y.push(a.y);
            raw_y.push(b.y);
            beams_geom.push((*beam, a, b, label_of(beam.id)));
        }
    }
    let columns: Vec<(u64, String, Point)> = project
        .fixtures
        .iter()
        .filter(|f| f.kind == FixtureKind::Column)
        .map(|f| {
            raw_x.push(f.center.x);
            raw_y.push(f.center.y);
            (f.id, label_of(f.id), f.center)
        })
        .collect();

    if raw_x.len() < 2 || raw_y.len() < 2 {
        return report;
    }

    let xs = build_grid_coords(&raw_x, 0.70, 18);
    let ys = build_grid_coords(&raw_y, 0.70, 18);
    if xs.len() < 2 || ys.len() < 2 {
        return report;
    }
    let nx = xs.len() - 1;
    let ny = ys.len() - 1;

    // Classify every cell `(ix, iy)` by which slab polygon contains its midpoint.
    let mut cell_slab: Vec<Vec<Option<usize>>> = vec![vec![None; nx]; ny];
    for iy in 0..ny {
        let cy = 0.5 * (ys[iy] + ys[iy + 1]);
        for ix in 0..nx {
            let cx = 0.5 * (xs[ix] + xs[ix + 1]);
            let pt = Point::new(cx, cy);
            for (s_idx, s) in slabs.iter().enumerate() {
                if polygon_contains_point(&s.polygon, pt) {
                    cell_slab[iy][ix] = Some(s_idx);
                    break;
                }
            }
        }
    }

    // Track active grid nodes (either touched by a slab cell or a beam).
    let mut active_mask = vec![vec![false; xs.len()]; ys.len()];
    for iy in 0..ny {
        for ix in 0..nx {
            if cell_slab[iy][ix].is_some() {
                active_mask[iy][ix] = true;
                active_mask[iy][ix + 1] = true;
                active_mask[iy + 1][ix] = true;
                active_mask[iy + 1][ix + 1] = true;
            }
        }
    }

    // Rasterize beams onto grid edges along their span.
    let mut beam_segments: Vec<BeamGridSegment> = Vec::new();
    for (beam, a, b, _) in &beams_geom {
        let ix0 = snap_coord_index(&xs, a.x);
        let iy0 = snap_coord_index(&ys, a.y);
        let ix1 = snap_coord_index(&xs, b.x);
        let iy1 = snap_coord_index(&ys, b.y);
        let self_wt = CONCRETE_KN_M3 * beam.width_m * beam.height_m;

        if iy0 == iy1 && ix0 != ix1 {
            let (lo, hi) = (ix0.min(ix1), ix0.max(ix1));
            for ix in lo..hi {
                active_mask[iy0][ix] = true;
                active_mask[iy0][ix + 1] = true;
                beam_segments.push(BeamGridSegment {
                    ix0: ix,
                    iy0,
                    ix1: ix + 1,
                    iy1: iy0,
                    beam_id: beam.id,
                    width_m: beam.width_m,
                    height_m: beam.height_m,
                    self_wt_kn_m: self_wt,
                });
            }
        } else if ix0 == ix1 && iy0 != iy1 {
            let (lo, hi) = (iy0.min(iy1), iy0.max(iy1));
            for iy in lo..hi {
                active_mask[iy][ix0] = true;
                active_mask[iy + 1][ix0] = true;
                beam_segments.push(BeamGridSegment {
                    ix0,
                    iy0: iy,
                    ix1: ix0,
                    iy1: iy + 1,
                    beam_id: beam.id,
                    width_m: beam.width_m,
                    height_m: beam.height_m,
                    self_wt_kn_m: self_wt,
                });
            }
        } else if (ix0, iy0) != (ix1, iy1) {
            // Skew / angled beam connects its two endpoint grid nodes directly.
            active_mask[iy0][ix0] = true;
            active_mask[iy1][ix1] = true;
            beam_segments.push(BeamGridSegment {
                ix0,
                iy0,
                ix1,
                iy1,
                beam_id: beam.id,
                width_m: beam.width_m,
                height_m: beam.height_m,
                self_wt_kn_m: self_wt,
            });
        }
    }

    // Assign compact node indices along the shorter dimension to minimize Cholesky bandwidth.
    let mut grid_to_node = vec![vec![None::<usize>; xs.len()]; ys.len()];
    let mut nodes: Vec<GridNode> = Vec::new();
    let iterate_x_inner = xs.len() <= ys.len();
    if iterate_x_inner {
        for iy in 0..ys.len() {
            for ix in 0..xs.len() {
                if active_mask[iy][ix] {
                    let id = nodes.len();
                    grid_to_node[iy][ix] = Some(id);
                    nodes.push(GridNode {
                        id,
                        point: Point::new(xs[ix], ys[iy]),
                        fix_w: false,
                        fix_rx: false,
                        fix_ry: false,
                        point_load_kn: 0.0,
                    });
                }
            }
        }
    } else {
        for ix in 0..xs.len() {
            for iy in 0..ys.len() {
                if active_mask[iy][ix] {
                    let id = nodes.len();
                    grid_to_node[iy][ix] = Some(id);
                    nodes.push(GridNode {
                        id,
                        point: Point::new(xs[ix], ys[iy]),
                        fix_w: false,
                        fix_rx: false,
                        fix_ry: false,
                        point_load_kn: 0.0,
                    });
                }
            }
        }
    }

    if nodes.is_empty() {
        return report;
    }

    // Support vertical DOFs at column positions.
    let mut support_labels: HashMap<usize, String> = HashMap::new();
    for (_, col_label, center) in &columns {
        if let Some((nid, _)) = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, n.point.distance(*center)))
            .filter(|(_, d)| *d <= 0.65)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            nodes[nid].fix_w = true;
            support_labels
                .entry(nid)
                .or_insert_with(|| col_label.clone());
        }
    }

    // If there are fewer than 3 column supports (e.g. walls without columns),
    // restrain perimeter slab nodes so the grid remains statically determinate.
    let fixed_count = nodes.iter().filter(|n| n.fix_w).count();
    if fixed_count < 3 {
        for iy in 0..ys.len() {
            for ix in 0..xs.len() {
                let Some(nid) = grid_to_node[iy][ix] else {
                    continue;
                };
                let adj_cells = [
                    (iy > 0 && ix > 0)
                        .then(|| cell_slab[iy - 1][ix - 1])
                        .flatten(),
                    (iy > 0 && ix < nx).then(|| cell_slab[iy - 1][ix]).flatten(),
                    (iy < ny && ix > 0).then(|| cell_slab[iy][ix - 1]).flatten(),
                    (iy < ny && ix < nx).then(|| cell_slab[iy][ix]).flatten(),
                ];
                if adj_cells.iter().filter(|c| c.is_some()).count() < 4 {
                    nodes[nid].fix_w = true;
                }
            }
        }
    }

    // Apply concentrated loads (water tanks / heavy equipment) to the nearest active node.
    for fix in &project.fixtures {
        let Some(load_kn) = fix
            .structural_load_kn
            .or_else(|| fix.kind.default_structural_load_kn())
        else {
            continue;
        };
        if let Some((nid, _)) = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, n.point.distance(fix.center)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            nodes[nid].point_load_kn += load_kn;
        }
    }

    let mut members: Vec<GridMember> = Vec::new();
    let mut mid = 0usize;

    // Emit X-direction slab strip members.
    for iy in 0..ys.len() {
        for ix in 0..nx {
            let below = (iy > 0).then(|| cell_slab[iy - 1][ix]).flatten();
            let above = (iy < ny).then(|| cell_slab[iy][ix]).flatten();
            if below.is_none() && above.is_none() {
                continue;
            }
            let (Some(n0), Some(n1)) = (grid_to_node[iy][ix], grid_to_node[iy][ix + 1]) else {
                continue;
            };
            let mut ei = 0.0;
            let mut gj = 0.0;
            let mut q = 0.0;
            let mut strip_w = 0.0;
            let mut primary_slab = 0u64;
            for (opt, dy) in [
                (below, if iy > 0 { ys[iy] - ys[iy - 1] } else { 0.0 }),
                (above, if iy < ny { ys[iy + 1] - ys[iy] } else { 0.0 }),
            ] {
                if let Some(s_idx) = opt {
                    let s = &slabs[s_idx];
                    primary_slab = s.slab_id;
                    let w_half = dy * 0.5;
                    let stiff_factor = match s.kind {
                        SlabKind::Solid => 1.0,
                        SlabKind::Ribbed => 0.72,
                    };
                    let d_plate = e_kpa * s.thickness_m.powi(3)
                        / (12.0 * (1.0 - CONCRETE_NU * CONCRETE_NU))
                        * stiff_factor;
                    ei += d_plate * w_half;
                    gj += d_plate * w_half;
                    q += 0.5 * s.q_kn_m2 * w_half;
                    strip_w += w_half;
                }
            }
            members.push(GridMember {
                id: mid,
                start: n0,
                end: n1,
                kind: GridMemberKind::SlabX {
                    slab_id: primary_slab,
                },
                ei_knm2: ei,
                gj_knm2: gj,
                strip_width_m: strip_w.max(1e-4),
                q_kn_m: q,
            });
            mid += 1;
        }
    }

    // Emit Y-direction slab strip members.
    for ix in 0..xs.len() {
        for iy in 0..ny {
            let left = (ix > 0).then(|| cell_slab[iy][ix - 1]).flatten();
            let right = (ix < nx).then(|| cell_slab[iy][ix]).flatten();
            if left.is_none() && right.is_none() {
                continue;
            }
            let (Some(n0), Some(n1)) = (grid_to_node[iy][ix], grid_to_node[iy + 1][ix]) else {
                continue;
            };
            let mut ei = 0.0;
            let mut gj = 0.0;
            let mut q = 0.0;
            let mut strip_w = 0.0;
            let mut primary_slab = 0u64;
            for (opt, dx) in [
                (left, if ix > 0 { xs[ix] - xs[ix - 1] } else { 0.0 }),
                (right, if ix < nx { xs[ix + 1] - xs[ix] } else { 0.0 }),
            ] {
                if let Some(s_idx) = opt {
                    let s = &slabs[s_idx];
                    primary_slab = s.slab_id;
                    let w_half = dx * 0.5;
                    let stiff_factor = match s.kind {
                        SlabKind::Solid => 1.0,
                        SlabKind::Ribbed => 0.72,
                    };
                    let d_plate = e_kpa * s.thickness_m.powi(3)
                        / (12.0 * (1.0 - CONCRETE_NU * CONCRETE_NU))
                        * stiff_factor;
                    ei += d_plate * w_half;
                    gj += d_plate * w_half;
                    q += 0.5 * s.q_kn_m2 * w_half;
                    strip_w += w_half;
                }
            }
            members.push(GridMember {
                id: mid,
                start: n0,
                end: n1,
                kind: GridMemberKind::SlabY {
                    slab_id: primary_slab,
                },
                ei_knm2: ei,
                gj_knm2: gj,
                strip_width_m: strip_w.max(1e-4),
                q_kn_m: q,
            });
            mid += 1;
        }
    }

    // Emit coupled beam members along grid edges (NBR 6118 §14.6.7.1 cracked torsion 0.15*GJ).
    for seg in beam_segments {
        let (Some(n0), Some(n1)) = (
            grid_to_node[seg.iy0][seg.ix0],
            grid_to_node[seg.iy1][seg.ix1],
        ) else {
            continue;
        };
        let i_beam = seg.width_m * seg.height_m.powi(3) / 12.0;
        let j_beam = rectangular_torsion_j(seg.width_m, seg.height_m) * 0.15;
        // Include wall load resting directly above this beam segment if any wall aligns with it.
        let p0 = nodes[n0].point;
        let p1 = nodes[n1].point;
        let mid_pt = Point::new(0.5 * (p0.x + p1.x), 0.5 * (p0.y + p1.y));
        let wall_load = project
            .walls
            .iter()
            .find(|w| {
                let (nearest, _) = nearest_point_on_segment(mid_pt, w.start, w.end);
                nearest.distance(mid_pt) <= 0.25
            })
            .map(|w| w.thickness() * w.mean_height(project.wall_height) * settings.masonry_kn_m3)
            .unwrap_or(0.0);

        members.push(GridMember {
            id: mid,
            start: n0,
            end: n1,
            kind: GridMemberKind::Beam {
                beam_id: seg.beam_id,
            },
            ei_knm2: e_kpa * i_beam,
            gj_knm2: g_kpa * j_beam,
            strip_width_m: 1.0,
            q_kn_m: seg.self_wt_kn_m + wall_load,
        });
        mid += 1;
    }

    let solved = match solve_grid(&nodes, &members) {
        Ok(s) => s,
        Err(err) => {
            report.error = Some(err);
            return report;
        }
    };

    report.node_count = nodes.len();
    report.member_count = members.len();
    report.total_applied_kn = solved.total_applied_kn;
    report.total_reaction_kn = solved.total_reaction_kn;
    report.equilibrium_error_kn = solved.equilibrium_error_kn;

    // Aggregate per-slab deflection and unit moments (mx, my, mxy in kN·m/m).
    for s in &slabs {
        let mut w_max_mm = 0.0_f64;
        let mut mx_pos = 0.0_f64;
        let mut my_pos = 0.0_f64;
        let mut mx_neg = 0.0_f64;
        let mut my_neg = 0.0_f64;
        let mut mxy_max = 0.0_f64;

        for mr in &solved.member_results {
            match mr.kind {
                GridMemberKind::SlabX { slab_id } if slab_id == s.slab_id => {
                    let sw = mr.strip_width_m.max(1e-4);
                    w_max_mm = w_max_mm.max(mr.w_max_mm);
                    mx_pos = mx_pos
                        .max(mr.m_mid_knm / sw)
                        .max(mr.m_start_knm / sw)
                        .max(mr.m_end_knm / sw);
                    mx_neg = mx_neg
                        .max((-mr.m_start_knm / sw).max(0.0))
                        .max((-mr.m_end_knm / sw).max(0.0));
                    mxy_max = mxy_max.max(mr.torsion_knm / sw);
                }
                GridMemberKind::SlabY { slab_id } if slab_id == s.slab_id => {
                    let sw = mr.strip_width_m.max(1e-4);
                    w_max_mm = w_max_mm.max(mr.w_max_mm);
                    my_pos = my_pos
                        .max(mr.m_mid_knm / sw)
                        .max(mr.m_start_knm / sw)
                        .max(mr.m_end_knm / sw);
                    my_neg = my_neg
                        .max((-mr.m_start_knm / sw).max(0.0))
                        .max((-mr.m_end_knm / sw).max(0.0));
                    mxy_max = mxy_max.max(mr.torsion_knm / sw);
                }
                _ => {}
            }
        }
        // Kirchhoff plate Poisson coupling (Hambly 1991 §3.3):
        // m_x = m_bar_x + ν * m_bar_y, m_y = m_bar_y + ν * m_bar_x.
        let mx_plate_pos = mx_pos + CONCRETE_NU * my_pos;
        let my_plate_pos = my_pos + CONCRETE_NU * mx_pos;
        let mx_plate_neg = mx_neg + CONCRETE_NU * my_neg;
        let my_plate_neg = my_neg + CONCRETE_NU * mx_neg;
        // Long-term creep multiplier (~2.5 per NBR 6118 §17.3.2.1.2) vs L/250 limit.
        let w_long_term_mm = w_max_mm * 2.5;
        let limit_mm = (s.short_span_m * 1000.0) / 250.0;
        let util = if limit_mm > 1e-6 {
            w_long_term_mm / limit_mm
        } else {
            0.0
        };
        report.slabs.push(GridSlabResult {
            slab_id: s.slab_id,
            label: s.label.clone(),
            room: s.room_name.clone(),
            is_irregular: s.is_irregular,
            short_span_m: s.short_span_m,
            w_elastic_mm: w_max_mm,
            w_long_term_mm,
            limit_mm,
            mx_pos_knm_m: mx_plate_pos,
            my_pos_knm_m: my_plate_pos,
            mx_neg_knm_m: mx_plate_neg,
            my_neg_knm_m: my_plate_neg,
            mxy_max_knm_m: mxy_max,
            light: Light::from_utilisation(util),
        });
    }

    // Aggregate per-beam coupled moments, shear, torsion and deflection.
    for (beam, _, _, b_label) in &beams_geom {
        let mut m_pos = 0.0_f64;
        let mut m_neg = 0.0_f64;
        let mut v_max = 0.0_f64;
        let mut t_max = 0.0_f64;
        let mut w_max = 0.0_f64;
        let mut found = false;
        for mr in &solved.member_results {
            if mr.kind == (GridMemberKind::Beam { beam_id: beam.id }) {
                found = true;
                m_pos = m_pos
                    .max(mr.m_mid_knm)
                    .max(mr.m_start_knm)
                    .max(mr.m_end_knm);
                m_neg = m_neg
                    .max((-mr.m_start_knm).max(0.0))
                    .max((-mr.m_end_knm).max(0.0));
                v_max = v_max.max(mr.v_max_kn);
                t_max = t_max.max(mr.torsion_knm);
                w_max = w_max.max(mr.w_max_mm);
            }
        }
        if found {
            report.beams.push(GridBeamResult {
                beam_id: beam.id,
                label: b_label.clone(),
                m_pos_knm: m_pos,
                m_neg_knm: m_neg,
                v_max_kn: v_max,
                t_max_knm: t_max,
                w_elastic_mm: w_max,
            });
        }
    }

    // Support reactions.
    for (i, node) in nodes.iter().enumerate() {
        if node.fix_w && solved.node_reaction_kn[i].abs() > 1e-3 {
            report.reactions.push(GridReaction {
                label: support_labels
                    .get(&i)
                    .cloned()
                    .unwrap_or_else(|| format!("A{}", i + 1)),
                point: node.point,
                reaction_kn: solved.node_reaction_kn[i],
            });
        }
    }

    // Cell deflection heatmap for SVG visualization.
    for iy in 0..ny {
        for ix in 0..nx {
            if cell_slab[iy][ix].is_none() {
                continue;
            }
            let corners = [
                grid_to_node[iy][ix],
                grid_to_node[iy][ix + 1],
                grid_to_node[iy + 1][ix + 1],
                grid_to_node[iy + 1][ix],
            ];
            let mut sum_w = 0.0;
            let mut count = 0.0;
            for c in corners.into_iter().flatten() {
                sum_w += solved.node_w_m[c] * 1000.0;
                count += 1.0;
            }
            if count > 0.0 {
                report.heatmap_cells.push(GridCellHeat {
                    min: Point::new(xs[ix], ys[iy]),
                    max: Point::new(xs[ix + 1], ys[iy + 1]),
                    w_elastic_mm: sum_w / count,
                });
            }
        }
    }
    report.max_elastic_w_mm = report
        .heatmap_cells
        .iter()
        .map(|c| c.w_elastic_mm.abs())
        .fold(0.0_f64, f64::max);

    report
}
