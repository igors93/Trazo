//! Orthogonal slab panel decomposition (L-shaped rooms) and 45° yield-line
//! load distribution to perimeter beams.

use std::collections::HashMap;

use crate::domain::geometry::Point;

/// cleanly along the wall.
pub(super) fn wall_orientation(along: Point) -> (u8, f64) {
    let len = along.x.hypot(along.y);
    if len < 1e-6 {
        return (0, 0.0);
    }
    let mut theta = along.y.atan2(along.x);
    if theta >= std::f64::consts::FRAC_PI_2 {
        theta -= std::f64::consts::PI;
    } else if theta < -std::f64::consts::FRAC_PI_2 {
        theta += std::f64::consts::PI;
    }
    let tol = 0.5_f64.to_radians();
    if theta.abs() <= tol {
        (0, 0.0)
    } else if (theta.abs() - std::f64::consts::FRAC_PI_2).abs() <= tol {
        (1, 0.0)
    } else {
        (0, theta)
    }
}

/// Splits a slab polygon into rectangular sub-panels (trying horizontal and
/// vertical strip decompositions and picking the one with the smaller
/// governing short span `lx`), so L- and U-shaped rooms get their real panel
/// spans instead of the outer bounding box.
pub fn decompose_slab_panels(outline: &[Point]) -> Vec<(f64, f64)> {
    if outline.len() < 3 {
        return Vec::new();
    }
    let bbox = || {
        let (mut min, mut max) = (outline[0], outline[0]);
        for p in outline {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
        let (w, h) = (max.x - min.x, max.y - min.y);
        vec![(w.min(h), w.max(h))]
    };
    let slice_spans = |level: f64, horizontal: bool| -> Vec<(f64, f64)> {
        let mut crossings = Vec::new();
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let (al, bl, ap, bp) = if horizontal {
                (a.y, b.y, a.x, b.x)
            } else {
                (a.x, b.x, a.y, b.y)
            };
            if (al > level) != (bl > level) {
                crossings.push(ap + (level - al) / (bl - al) * (bp - ap));
            }
        }
        crossings.sort_by(f64::total_cmp);
        crossings
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|pair| pair[1] - pair[0] > 1e-4)
            .map(|pair| (pair[0], pair[1]))
            .collect()
    };
    let decompose = |horizontal: bool| -> Vec<(f64, f64)> {
        let mut cuts: Vec<f64> = outline
            .iter()
            .map(|p| if horizontal { p.y } else { p.x })
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        type Strip = (f64, f64, Vec<(f64, f64)>);
        let mut strips: Vec<Strip> = Vec::new();
        for pair in cuts.windows(2) {
            let (c0, c1) = (pair[0], pair[1]);
            if c1 - c0 <= 1e-4 {
                continue;
            }
            let intervals = slice_spans(0.5 * (c0 + c1), horizontal);
            if intervals.is_empty() {
                continue;
            }
            let same_as_prev = strips.last().is_some_and(|(_, _, prev)| {
                prev.len() == intervals.len()
                    && prev.iter().zip(&intervals).all(|((a0, a1), (b0, b1))| {
                        (a0 - b0).abs() < 1e-3 && (a1 - b1).abs() < 1e-3
                    })
            });
            if same_as_prev {
                if let Some(last) = strips.last_mut() {
                    last.1 = c1;
                }
            } else {
                strips.push((c0, c1, intervals));
            }
        }
        let mut panels = Vec::new();
        for (c0, c1, intervals) in strips {
            let h = c1 - c0;
            for (p0, p1) in intervals {
                let w = p1 - p0;
                panels.push((w.min(h), w.max(h)));
            }
        }
        panels
    };
    let h_panels = decompose(true);
    let v_panels = decompose(false);
    let gov = |panels: &[(f64, f64)]| panels.iter().map(|(lx, _)| *lx).fold(0.0_f64, f64::max);
    match (h_panels.is_empty(), v_panels.is_empty()) {
        (true, true) => bbox(),
        (false, true) => h_panels,
        (true, false) => v_panels,
        (false, false) => {
            let (gh, gv) = (gov(&h_panels), gov(&v_panels));
            if (gh - gv).abs() < 1e-4 {
                if h_panels.len() <= v_panels.len() {
                    h_panels
                } else {
                    v_panels
                }
            } else if gh < gv {
                h_panels
            } else {
                v_panels
            }
        }
    }
}

/// Distributes a slab's total load among its supporting beam segments by the
/// 45° yield-line method (NBR 6118 14.7.6.1): each interior point sheds its
/// load to the nearest supported edge, producing exact triangular/trapezoidal
/// tributary regions for rectangles and natural 45° regions for L-shaped or
/// irregular slabs. Returned fractions sum to 1.0.
pub fn yield_line_shares(
    outline: &[Point],
    supported: &[(u64, Point, Point)],
) -> HashMap<u64, f64> {
    let mut shares: HashMap<u64, f64> = HashMap::new();
    if outline.len() < 3 || supported.is_empty() {
        return shares;
    }
    if outline.len() == 4 {
        let mut xs: Vec<f64> = outline.iter().map(|p| p.x).collect();
        let mut ys: Vec<f64> = outline.iter().map(|p| p.y).collect();
        xs.sort_by(f64::total_cmp);
        ys.sort_by(f64::total_cmp);
        let (x0, x1) = (xs[0], xs[3]);
        let (y0, y1) = (ys[0], ys[3]);
        let is_rect = outline.iter().all(|p| {
            ((p.x - x0).abs() < 1e-4 || (p.x - x1).abs() < 1e-4)
                && ((p.y - y0).abs() < 1e-4 || (p.y - y1).abs() < 1e-4)
        });
        if is_rect && x1 - x0 > 1e-4 && y1 - y0 > 1e-4 {
            let w = x1 - x0;
            let h = y1 - y0;
            let lx = w.min(h);
            let ly = w.max(h);
            let short_area = lx * lx / 4.0;
            let long_area = 0.5 * lx * (ly - 0.5 * lx);
            let rect_edges = [
                (Point::new(x0, y0), Point::new(x1, y0), w),
                (Point::new(x1, y0), Point::new(x1, y1), h),
                (Point::new(x1, y1), Point::new(x0, y1), w),
                (Point::new(x0, y1), Point::new(x0, y0), h),
            ];
            let mut per_edge: Vec<Vec<(u64, f64)>> = vec![Vec::new(); 4];
            for (edge_idx, &(ea, eb, len)) in rect_edges.iter().enumerate() {
                let Some(dir) = eb.minus(ea).normalized() else {
                    continue;
                };
                for &(beam_id, sa, sb) in supported {
                    if sa.minus(ea).cross(dir).abs() <= 0.12
                        && sb.minus(ea).cross(dir).abs() <= 0.12
                    {
                        let s0 = sa.minus(ea).dot(dir);
                        let s1 = sb.minus(ea).dot(dir);
                        let overlap = s0.max(s1).min(len) - s0.min(s1).max(0.0);
                        if overlap > 0.05 {
                            per_edge[edge_idx].push((beam_id, overlap));
                        }
                    }
                }
            }
            if per_edge.iter().all(|e| !e.is_empty()) {
                let total_area = w * h;
                for (edge_idx, &(_, _, len)) in rect_edges.iter().enumerate() {
                    let edge_area = if (len - lx).abs() <= (len - ly).abs() {
                        short_area
                    } else {
                        long_area
                    };
                    let sum_overlap: f64 = per_edge[edge_idx].iter().map(|(_, o)| *o).sum();
                    for &(beam_id, overlap) in &per_edge[edge_idx] {
                        *shares.entry(beam_id).or_default() +=
                            (edge_area / total_area) * (overlap / sum_overlap);
                    }
                }
                return shares;
            }
        }
    }

    let (mut min, mut max) = (outline[0], outline[0]);
    for p in outline {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    let nx = 32usize;
    let ny = 32usize;
    let dx = (max.x - min.x) / nx as f64;
    let dy = (max.y - min.y) / ny as f64;
    if dx <= 1e-6 || dy <= 1e-6 {
        return shares;
    }
    let mut hits = 0.0;
    for ix in 0..nx {
        let x = min.x + (ix as f64 + 0.5) * dx;
        for iy in 0..ny {
            let y = min.y + (iy as f64 + 0.5) * dy;
            let pt = Point::new(x, y);
            if !crate::domain::geometry::polygon_contains_point(outline, pt) {
                continue;
            }
            let mut min_d = f64::INFINITY;
            let mut closest: Vec<u64> = Vec::new();
            for &(beam_id, sa, sb) in supported {
                let d = crate::domain::geometry::distance_to_segment(pt, sa, sb);
                if d + 1e-6 < min_d {
                    min_d = d;
                    closest.clear();
                    closest.push(beam_id);
                } else if (d - min_d).abs() <= 1e-6 {
                    closest.push(beam_id);
                }
            }
            if !closest.is_empty() {
                let w = 1.0 / closest.len() as f64;
                for beam_id in closest {
                    *shares.entry(beam_id).or_default() += w;
                }
                hits += 1.0;
            }
        }
    }
    if hits > 0.0 {
        for v in shares.values_mut() {
            *v /= hits;
        }
    }
    shares
}
