//! Automatic wall junctions. A junction is derived from wall geometry, so it
//! stays correct after moving a wall and never needs a persisted cache.

use super::{
    geometry::{
        JOIN_TOLERANCE, Point, distance_to_segment, line_intersection, nearest_point_on_segment,
    },
    model::Project,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallJunctionKind {
    L,
    T,
    X,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WallJunction {
    pub point: Point,
    pub kind: WallJunctionKind,
    pub wall_ids: Vec<u64>,
    /// Convex patch joining the finished faces of every incident wall.
    pub outline: Vec<Point>,
}

#[derive(Clone, Copy)]
struct Ray {
    direction: Point,
    half_width: f64,
    wall_id: u64,
}

fn convex_hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.near(*b, 1e-8));
    if points.len() <= 2 {
        return points;
    }
    let cross = |a: Point, b: Point, c: Point| b.minus(a).cross(c.minus(a));
    let mut lower = Vec::new();
    for &point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 1e-10
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper = Vec::new();
    for &point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 1e-10
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

impl Project {
    pub fn wall_junctions(&self) -> Vec<WallJunction> {
        let mut candidates: Vec<Point> = Vec::new();
        for (index, first) in self.walls.iter().enumerate() {
            for second in self.walls.iter().skip(index + 1) {
                let Some((point, t, u)) =
                    line_intersection(first.start, first.end, second.start, second.end)
                else {
                    continue;
                };
                let first_margin = JOIN_TOLERANCE / first.length().max(JOIN_TOLERANCE);
                let second_margin = JOIN_TOLERANCE / second.length().max(JOIN_TOLERANCE);
                if (-first_margin..=1.0 + first_margin).contains(&t)
                    && (-second_margin..=1.0 + second_margin).contains(&u)
                    && !candidates
                        .iter()
                        .any(|candidate| candidate.near(point, JOIN_TOLERANCE))
                {
                    candidates.push(point);
                }
            }
        }

        candidates
            .into_iter()
            .filter_map(|point| {
                let mut rays: Vec<Ray> = Vec::new();
                for wall in &self.walls {
                    if distance_to_segment(point, wall.start, wall.end) > JOIN_TOLERANCE {
                        continue;
                    }
                    let (_, t) = nearest_point_on_segment(point, wall.start, wall.end);
                    let margin = JOIN_TOLERANCE / wall.length().max(JOIN_TOLERANCE);
                    let mut add_ray = |target: Point| {
                        let Some(direction) = target.minus(point).normalized() else {
                            return;
                        };
                        if let Some(existing) = rays
                            .iter_mut()
                            .find(|ray| ray.direction.dot(direction) > 0.9999)
                        {
                            if wall.thickness() / 2.0 > existing.half_width {
                                existing.half_width = wall.thickness() / 2.0;
                                existing.wall_id = wall.id;
                            }
                        } else {
                            rays.push(Ray {
                                direction,
                                half_width: wall.thickness() / 2.0,
                                wall_id: wall.id,
                            });
                        }
                    };
                    if t > margin {
                        add_ray(wall.start);
                    }
                    if t < 1.0 - margin {
                        add_ray(wall.end);
                    }
                }

                let kind = match rays.len() {
                    2 if rays[0].direction.dot(rays[1].direction).abs() < 0.9999 => {
                        WallJunctionKind::L
                    }
                    3 => WallJunctionKind::T,
                    count if count >= 4 => WallJunctionKind::X,
                    _ => return None,
                };
                let mut wall_ids: Vec<u64> = rays.iter().map(|ray| ray.wall_id).collect();
                wall_ids.sort_unstable();
                wall_ids.dedup();
                rays.sort_by(|a, b| {
                    a.direction
                        .y
                        .atan2(a.direction.x)
                        .total_cmp(&b.direction.y.atan2(b.direction.x))
                });
                let mut boundary: Vec<Point> = rays
                    .iter()
                    .flat_map(|ray| {
                        let normal =
                            Point::new(-ray.direction.y, ray.direction.x).scaled(ray.half_width);
                        [point.translated(normal), point.minus(normal)]
                    })
                    .collect();
                // Include miter intersections of adjacent rays' facing edges (clamped by miter limit)
                // so L-, T-, and X-junction patches fill the full outer corner rather than chamfering it.
                for idx in 0..rays.len() {
                    let a = rays[idx];
                    let b = rays[(idx + 1) % rays.len()];
                    let na = Point::new(-a.direction.y, a.direction.x).scaled(a.half_width);
                    let nb = Point::new(-b.direction.y, b.direction.x).scaled(b.half_width);
                    let miter_limit = a.half_width.max(b.half_width) * 2.5;
                    for (pa, pb) in [
                        (point.translated(na), point.minus(nb)),
                        (point.minus(na), point.translated(nb)),
                    ] {
                        if let Some((corner, _, _)) = line_intersection(
                            pa,
                            pa.translated(a.direction),
                            pb,
                            pb.translated(b.direction),
                        ) && corner.is_finite()
                            && corner.distance(point) <= miter_limit
                        {
                            boundary.push(corner);
                        }
                    }
                }
                let outline = convex_hull(boundary);
                (outline.len() >= 3).then_some(WallJunction {
                    point: point.rounded(),
                    kind,
                    wall_ids,
                    outline,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{geometry::polygon_area, masonry::Masonry};

    #[test]
    fn detects_l_t_and_x_junctions_at_arbitrary_angles_and_widths() {
        let mut l = Project::default();
        l.add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        let diagonal = l
            .add_wall(Point::new(0.0, 0.0), Point::new(-2.0, -1.3))
            .unwrap();
        let thick = Masonry {
            block_width: 0.19,
            ..Default::default()
        };
        l.set_wall_masonry(diagonal, &thick);
        let joins = l.wall_junctions();
        assert_eq!(joins.len(), 1);
        assert_eq!(joins[0].kind, WallJunctionKind::L);
        assert!(polygon_area(&joins[0].outline) > 0.01);

        let mut t = Project::default();
        t.add_wall(Point::new(-3.0, 0.0), Point::new(3.0, 0.0));
        t.add_wall(Point::new(0.0, -2.0), Point::new(0.0, 0.0));
        assert_eq!(t.wall_junctions()[0].kind, WallJunctionKind::T);

        let mut x = t.clone();
        x.add_wall(Point::new(0.0, 0.0), Point::new(1.5, 2.5));
        assert_eq!(x.wall_junctions()[0].kind, WallJunctionKind::X);
    }

    #[test]
    fn collinear_continuations_are_not_false_junctions() {
        let mut project = Project::default();
        project.add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
        project.add_wall(Point::new(2.0, 0.0), Point::new(4.0, 0.0));
        assert!(project.wall_junctions().is_empty());
    }
}
