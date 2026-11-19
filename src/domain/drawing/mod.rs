//! Generic CAD entities drawn over the plan: lines, polylines, rectangles,
//! arcs, circles and splines. They are plain geometry, with no walls or
//! rooms behind them: construction lines, details, outlines and notes.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use super::geometry::{EPSILON, Point, distance_to_segment, line_intersection};

mod spline;
pub use spline::spline_points;

/// The smallest size a shape can have: a millimetre.
const MIN_SIZE: f64 = 1e-3;
/// Segments per quarter of a circle when a curve is drawn as straight lines.
const ARC_STEPS_PER_QUARTER: f64 = 16.0;

/// One entity of the drawing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub id: u64,
    pub shape: Shape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Line {
        start: Point,
        end: Point,
    },
    /// An open or closed chain of straight segments. A rectangle is a
    /// closed one with four upright sides.
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    /// Part of a circle from angle `start`, turning `sweep` radians (positive
    /// from +x towards +y in plan coordinates, so clockwise on screen).
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        sweep: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// A smooth curve through its points.
    Spline {
        points: Vec<Point>,
        closed: bool,
    },
}

/// The point at `angle` on the circle around `center`.
pub fn point_at_angle(center: Point, radius: f64, angle: f64) -> Point {
    Point::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// The direction of `point` seen from `center`, in radians.
pub fn angle_of(center: Point, point: Point) -> f64 {
    (point.y - center.y).atan2(point.x - center.x)
}

/// Whether the direction `angle` falls within an arc starting at `start`
/// and turning `sweep`.
pub fn angle_within(angle: f64, start: f64, sweep: f64) -> bool {
    if sweep >= 0.0 {
        (angle - start).rem_euclid(TAU) <= sweep + EPSILON
    } else {
        (start - angle).rem_euclid(TAU) <= -sweep + EPSILON
    }
}

impl Shape {
    /// The upright rectangle with opposite corners `a` and `b`.
    pub fn rectangle(a: Point, b: Point) -> Self {
        Shape::Polyline {
            points: vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
            closed: true,
        }
    }

    /// The arc that starts at `start`, passes through `through` and ends at
    /// `end`; `None` when the three points are in line.
    pub fn arc_through(start: Point, through: Point, end: Point) -> Option<Self> {
        let bisector = |a: Point, b: Point| {
            let middle = a.midpoint(b);
            let along = b.minus(a);
            (middle, middle.translated(Point::new(-along.y, along.x)))
        };
        let (p1, p2) = bisector(start, through);
        let (p3, p4) = bisector(through, end);
        let (center, _, _) = line_intersection(p1, p2, p3, p4)?;
        let radius = center.distance(start);
        if !radius.is_finite() || radius < MIN_SIZE || radius > 1e5 {
            return None;
        }
        let from = angle_of(center, start);
        let forward = (angle_of(center, end) - from).rem_euclid(TAU);
        let middle = (angle_of(center, through) - from).rem_euclid(TAU);
        let sweep = if middle <= forward {
            forward
        } else {
            forward - TAU
        };
        Some(Shape::Arc {
            center,
            radius,
            start: from,
            sweep,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "Linha",
            Shape::Polyline { .. } if self.is_rectangle() => "Retângulo",
            Shape::Polyline { .. } => "Polilinha",
            Shape::Arc { .. } => "Arco",
            Shape::Circle { .. } => "Círculo",
            Shape::Spline { .. } => "Spline",
        }
    }

    /// Whether this is a closed polyline with four upright sides.
    pub fn is_rectangle(&self) -> bool {
        match self {
            Shape::Polyline {
                points,
                closed: true,
            } if points.len() == 4 => (0..4).all(|i| {
                let (a, b) = (points[i], points[(i + 1) % 4]);
                (a.x - b.x).abs() < EPSILON || (a.y - b.y).abs() < EPSILON
            }),
            _ => false,
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Shape::Line { .. } | Shape::Arc { .. } => false,
            Shape::Circle { .. } => true,
            Shape::Polyline { closed, .. } | Shape::Spline { closed, .. } => *closed,
        }
    }

    /// Why the shape cannot be drawn, if it cannot.
    pub fn validate(&self) -> Result<(), String> {
        let finite = |points: &[Point]| points.iter().all(|point| point.is_finite());
        match self {
            Shape::Line { start, end } => {
                if !finite(&[*start, *end]) || start.distance(*end) < MIN_SIZE {
                    return Err("A linha precisa de dois pontos diferentes.".to_owned());
                }
            }
            Shape::Polyline { points, closed } | Shape::Spline { points, closed } => {
                let needed = if *closed { 3 } else { 2 };
                if points.len() < needed || !finite(points) {
                    return Err(format!("São necessários pelo menos {needed} pontos."));
                }
                if self.length() < MIN_SIZE {
                    return Err("O desenho não tem comprimento.".to_owned());
                }
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                if !finite(&[*center])
                    || !radius.is_finite()
                    || *radius < MIN_SIZE
                    || !start.is_finite()
                    || !sweep.is_finite()
                    || sweep.abs() < 1e-6
                    || sweep.abs() > TAU
                {
                    return Err("O arco precisa de três pontos fora de uma reta.".to_owned());
                }
            }
            Shape::Circle { center, radius } => {
                if !finite(&[*center]) || !radius.is_finite() || *radius < MIN_SIZE {
                    return Err("O círculo precisa de um raio.".to_owned());
                }
            }
        }
        Ok(())
    }

    /// The shape moved by `delta`.
    pub fn translated(&self, delta: Point) -> Self {
        let mut moved = self.clone();
        moved.for_each_point(|point| *point = point.translated(delta));
        moved
    }

    /// Visits every stored point (ends, corners, control points, centres).
    pub fn for_each_point(&mut self, mut visit: impl FnMut(&mut Point)) {
        match self {
            Shape::Line { start, end } => {
                visit(start);
                visit(end);
            }
            Shape::Polyline { points, .. } | Shape::Spline { points, .. } => {
                points.iter_mut().for_each(visit);
            }
            Shape::Arc { center, .. } | Shape::Circle { center, .. } => visit(center),
        }
    }

    /// The path of the shape as straight segments, closed shapes ending
    /// where they start.
    pub fn outline(&self) -> Vec<Point> {
        let arc = |center: Point, radius: f64, start: f64, sweep: f64| {
            let steps = (sweep.abs() / (TAU / 4.0) * ARC_STEPS_PER_QUARTER)
                .ceil()
                .max(2.0) as usize;
            (0..=steps)
                .map(|i| point_at_angle(center, radius, start + sweep * i as f64 / steps as f64))
                .collect::<Vec<_>>()
        };
        match self {
            Shape::Line { start, end } => vec![*start, *end],
            Shape::Polyline { points, closed } => {
                let mut path = points.clone();
                if *closed && let Some(first) = points.first() {
                    path.push(*first);
                }
                path
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => arc(*center, *radius, *start, *sweep),
            Shape::Circle { center, radius } => arc(*center, *radius, 0.0, TAU),
            Shape::Spline { points, closed } => spline_points(points, *closed, 12),
        }
    }

    /// The length along the shape (the perimeter of closed ones).
    pub fn length(&self) -> f64 {
        match self {
            Shape::Arc { radius, sweep, .. } => radius * sweep.abs(),
            Shape::Circle { radius, .. } => radius * TAU,
            _ => self
                .outline()
                .windows(2)
                .map(|pair| pair[0].distance(pair[1]))
                .sum(),
        }
    }

    /// How far `point` is from the line of the shape.
    pub fn distance_to(&self, point: Point) -> f64 {
        match self {
            Shape::Circle { center, radius } => (center.distance(point) - radius).abs(),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                if angle_within(angle_of(*center, point), *start, *sweep) {
                    (center.distance(point) - radius).abs()
                } else {
                    let first = point_at_angle(*center, *radius, *start);
                    let last = point_at_angle(*center, *radius, start + sweep);
                    point.distance(first).min(point.distance(last))
                }
            }
            _ => self
                .outline()
                .windows(2)
                .map(|pair| distance_to_segment(point, pair[0], pair[1]))
                .fold(f64::INFINITY, f64::min),
        }
    }

    /// The smallest upright box around the shape.
    pub fn bounds(&self) -> (Point, Point) {
        self.outline().iter().fold(
            (
                Point::new(f64::INFINITY, f64::INFINITY),
                Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            ),
            |(low, high), point| {
                (
                    Point::new(low.x.min(point.x), low.y.min(point.y)),
                    Point::new(high.x.max(point.x), high.y.max(point.y)),
                )
            },
        )
    }

    /// The points that define the shape: ends and corners, the ends of an
    /// arc, a circle's centre, a spline's control points.
    pub fn key_points(&self) -> Vec<Point> {
        match self {
            Shape::Line { start, end } => vec![*start, *end],
            Shape::Polyline { points, .. } | Shape::Spline { points, .. } => points.clone(),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => vec![
                point_at_angle(*center, *radius, *start),
                point_at_angle(*center, *radius, start + sweep),
            ],
            Shape::Circle { center, .. } => vec![*center],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_measure_and_locate_themselves() {
        let line = Shape::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(3.0, 4.0),
        };
        assert_eq!(line.length(), 5.0);
        assert!((line.distance_to(Point::new(0.0, 5.0)) - 3.0).abs() < 1e-9);

        let rectangle = Shape::rectangle(Point::new(0.0, 0.0), Point::new(4.0, 2.0));
        assert!(rectangle.is_rectangle() && rectangle.is_closed());
        assert_eq!(rectangle.label(), "Retângulo");
        assert_eq!(rectangle.length(), 12.0);
        assert_eq!(
            rectangle.bounds(),
            (Point::new(0.0, 0.0), Point::new(4.0, 2.0))
        );

        let circle = Shape::Circle {
            center: Point::new(1.0, 1.0),
            radius: 2.0,
        };
        assert!((circle.length() - 4.0 * std::f64::consts::PI).abs() < 1e-9);
        assert!((circle.distance_to(Point::new(1.0, 4.0)) - 1.0).abs() < 1e-9);
        assert!(
            Shape::Circle {
                center: Point::ORIGIN,
                radius: 0.0
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn arcs_pass_through_their_three_points() {
        // A half circle of radius 2 over the top (y grows downwards on
        // screen, so "over" is negative y).
        let arc = Shape::arc_through(
            Point::new(-2.0, 0.0),
            Point::new(0.0, -2.0),
            Point::new(2.0, 0.0),
        )
        .unwrap();
        let Shape::Arc {
            center,
            radius,
            sweep,
            ..
        } = arc
        else {
            panic!("not an arc");
        };
        assert!(center.near(Point::ORIGIN, 1e-9) && (radius - 2.0).abs() < 1e-9);
        assert!((sweep.abs() - std::f64::consts::PI).abs() < 1e-9);
        assert!(arc.distance_to(Point::new(0.0, -2.0)) < 1e-9);
        // The other half is not part of it.
        assert!((arc.distance_to(Point::new(0.0, 2.0)) - 8f64.sqrt()).abs() < 1e-9);
        assert!(arc.validate().is_ok());
        assert!(
            Shape::arc_through(
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(2.0, 2.0)
            )
            .is_none()
        );
    }

    #[test]
    fn splines_run_through_their_points() {
        let points = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 1.0),
            Point::new(4.0, 0.0),
        ];
        let spline = Shape::Spline {
            points: points.to_vec(),
            closed: false,
        };
        let path = spline.outline();
        for point in points {
            assert!(path.iter().any(|sample| sample.near(point, 1e-9)));
        }
        assert!(spline.length() > 4.4);
        assert_eq!(spline.key_points().len(), 3);
    }
}
