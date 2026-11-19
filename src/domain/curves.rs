//! Straight segments and circular arcs, and where lines and circles meet
//! them: the cutting edges for trimming and the boundaries for extending.

use std::f64::consts::TAU;

use super::drawing::{Shape, angle_of, angle_within, point_at_angle};
use super::geometry::{EPSILON, Point, line_intersection};

/// How far past its ends a segment still counts as reached (a micrometre).
const REACH: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Segment(Point, Point),
    /// Part of a circle from angle `start`, turning `sweep` (a whole circle
    /// turns `TAU`).
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        sweep: f64,
    },
}

impl Curve {
    /// The pieces of a drawing shape (a spline by its drawn path).
    pub fn of_shape(shape: &Shape) -> Vec<Curve> {
        match shape {
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => vec![Curve::Arc {
                center: *center,
                radius: *radius,
                start: *start,
                sweep: *sweep,
            }],
            Shape::Circle { center, radius } => vec![Curve::Arc {
                center: *center,
                radius: *radius,
                start: 0.0,
                sweep: TAU,
            }],
            _ => shape
                .outline()
                .windows(2)
                .filter(|pair| pair[0].distance(pair[1]) > EPSILON)
                .map(|pair| Curve::Segment(pair[0], pair[1]))
                .collect(),
        }
    }

    /// Whether `point`, known to lie on the curve's line or circle, falls
    /// within the curve.
    fn holds(&self, point: Point) -> bool {
        match *self {
            Curve::Segment(a, b) => {
                let length = a.distance(b);
                if length <= EPSILON {
                    return point.distance(a) <= REACH;
                }
                let t = point.minus(a).dot(b.minus(a)) / (length * length);
                t >= -REACH / length && t <= 1.0 + REACH / length
            }
            Curve::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                sweep.abs() >= TAU - 1e-9
                    || angle_within(angle_of(center, point), start, sweep)
                    || point.distance(point_at_angle(center, radius, start)) <= REACH
                    || point.distance(point_at_angle(center, radius, start + sweep)) <= REACH
            }
        }
    }
}

/// Where the line through `a` and `b` (beyond them too) meets `curve`, as
/// parameters `t` along it (`a + t(b - a)`).
pub fn line_hits(a: Point, b: Point, curve: &Curve) -> Vec<f64> {
    let direction = b.minus(a);
    let length_squared = direction.dot(direction);
    if length_squared <= EPSILON * EPSILON {
        return Vec::new();
    }
    match *curve {
        Curve::Segment(c, d) => line_intersection(a, b, c, d)
            .filter(|(point, _, _)| curve.holds(*point))
            .map(|(_, t, _)| vec![t])
            .unwrap_or_default(),
        Curve::Arc { center, radius, .. } => {
            // |a + t·d - c|² = r²
            let offset = a.minus(center);
            let half_b = offset.dot(direction);
            let c_term = offset.dot(offset) - radius * radius;
            let discriminant = half_b * half_b - length_squared * c_term;
            if discriminant < -EPSILON {
                return Vec::new();
            }
            let root = discriminant.max(0.0).sqrt();
            let mut hits: Vec<f64> = [(-half_b - root), (-half_b + root)]
                .into_iter()
                .map(|value| value / length_squared)
                .filter(|t| curve.holds(a.lerp(b, *t)))
                .collect();
            hits.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
            hits
        }
    }
}

/// Where the circle around `center` meets `curve`, as angles on it.
pub fn circle_hits(center: Point, radius: f64, curve: &Curve) -> Vec<f64> {
    match *curve {
        Curve::Segment(a, b) => {
            let probe = Curve::Arc {
                center,
                radius,
                start: 0.0,
                sweep: TAU,
            };
            line_hits(a, b, &probe)
                .into_iter()
                .filter(|t| *t >= -REACH && *t <= 1.0 + REACH)
                .map(|t| angle_of(center, a.lerp(b, t)))
                .collect()
        }
        Curve::Arc {
            center: other,
            radius: other_radius,
            ..
        } => {
            let gap = center.distance(other);
            if gap <= EPSILON
                || gap > radius + other_radius + EPSILON
                || gap < (radius - other_radius).abs() - EPSILON
            {
                return Vec::new();
            }
            // Along the line of centres to the chord, then across it.
            let along = (radius * radius - other_radius * other_radius + gap * gap) / (2.0 * gap);
            let across = (radius * radius - along * along).max(0.0).sqrt();
            let unit = other.minus(center).scaled(1.0 / gap);
            let foot = center.translated(unit.scaled(along));
            let normal = Point::new(-unit.y, unit.x);
            let mut points = vec![foot.translated(normal.scaled(across))];
            if across > 1e-9 {
                points.push(foot.translated(normal.scaled(-across)));
            }
            points
                .into_iter()
                .filter(|point| curve.holds(*point))
                .map(|point| angle_of(center, point))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_meet_segments_and_arcs_only_within_them() {
        let across = Curve::Segment(Point::new(2.0, -1.0), Point::new(2.0, 1.0));
        assert_eq!(
            line_hits(Point::new(0.0, 0.0), Point::new(1.0, 0.0), &across),
            vec![2.0]
        );
        let short = Curve::Segment(Point::new(2.0, 1.0), Point::new(2.0, 3.0));
        assert!(line_hits(Point::new(0.0, 0.0), Point::new(1.0, 0.0), &short).is_empty());

        let circle = Curve::Arc {
            center: Point::new(5.0, 0.0),
            radius: 1.0,
            start: 0.0,
            sweep: TAU,
        };
        let hits = line_hits(Point::new(0.0, 0.0), Point::new(1.0, 0.0), &circle);
        assert_eq!(hits.len(), 2);
        assert!((hits[0] - 4.0).abs() < 1e-9 && (hits[1] - 6.0).abs() < 1e-9);
        // Half of it (the lower half on screen, y > 0) still meets the line
        // at its ends.
        let half = Curve::Arc {
            center: Point::new(5.0, 0.0),
            radius: 1.0,
            start: 0.0,
            sweep: std::f64::consts::PI,
        };
        assert_eq!(
            line_hits(Point::new(0.0, 0.0), Point::new(1.0, 0.0), &half).len(),
            2
        );
        let upper = Curve::Arc {
            center: Point::new(5.0, 0.5),
            radius: 1.0,
            start: 0.0,
            sweep: std::f64::consts::PI,
        };
        assert!(line_hits(Point::new(0.0, 0.0), Point::new(1.0, 0.0), &upper).is_empty());
    }

    #[test]
    fn circles_meet_segments_and_other_circles() {
        let segment = Curve::Segment(Point::new(-3.0, 0.0), Point::new(3.0, 0.0));
        let mut angles = circle_hits(Point::ORIGIN, 2.0, &segment);
        angles.sort_by(f64::total_cmp);
        assert_eq!(angles.len(), 2);
        assert!(angles[0].abs() < 1e-9);
        assert!((angles[1] - std::f64::consts::PI).abs() < 1e-9);

        let other = Curve::Arc {
            center: Point::new(2.0, 0.0),
            radius: 2.0,
            start: 0.0,
            sweep: TAU,
        };
        let meets = circle_hits(Point::ORIGIN, 2.0, &other);
        assert_eq!(meets.len(), 2);
        for angle in meets {
            let point = point_at_angle(Point::ORIGIN, 2.0, angle);
            assert!((point.distance(Point::new(2.0, 0.0)) - 2.0).abs() < 1e-9);
        }
        let apart = Curve::Arc {
            center: Point::new(10.0, 0.0),
            radius: 1.0,
            start: 0.0,
            sweep: TAU,
        };
        assert!(circle_hits(Point::ORIGIN, 2.0, &apart).is_empty());
        assert_eq!(
            Curve::of_shape(&Shape::rectangle(Point::ORIGIN, Point::new(2.0, 1.0))).len(),
            4
        );
    }
}
