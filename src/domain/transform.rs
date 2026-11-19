//! Rigid transforms of the plan: moving, turning about a point and
//! mirroring across a line, applied to points, directions, drawing shapes
//! and the quarter turns of fixtures.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::drawing::Shape;
use super::geometry::{EPSILON, Point};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    Translate(Point),
    /// Turns by `angle` radians about `center`: positive from +x towards
    /// +y in plan coordinates, which is clockwise on screen.
    Rotate {
        center: Point,
        angle: f64,
    },
    /// Reflects across the line through `a` and `b`.
    Mirror {
        a: Point,
        b: Point,
    },
}

/// `angle` brought into (-π, π].
pub fn normalize_angle(angle: f64) -> f64 {
    let turned = (angle + PI).rem_euclid(TAU) - PI;
    // A half turn reads as +π however rounding left it.
    if turned <= -PI + 1e-12 { PI } else { turned }
}

impl Transform {
    /// The mirror across the line through `a` and `b`, which must differ.
    pub fn mirror(a: Point, b: Point) -> Result<Self, String> {
        if !a.is_finite() || !b.is_finite() || a.distance(b) < 1e-3 {
            return Err("O eixo de espelhamento precisa de dois pontos diferentes.".to_owned());
        }
        Ok(Transform::Mirror { a, b })
    }

    /// Turns by `degrees` counter-clockwise on screen (as angles are typed,
    /// Y up) about `center`.
    pub fn rotate_degrees(center: Point, degrees: f64) -> Self {
        Transform::Rotate {
            center,
            angle: -degrees.to_radians(),
        }
    }

    pub fn is_valid(&self) -> bool {
        match *self {
            Transform::Translate(delta) => delta.is_finite(),
            Transform::Rotate { center, angle } => center.is_finite() && angle.is_finite(),
            Transform::Mirror { a, b } => a.is_finite() && b.is_finite() && a.distance(b) >= 1e-3,
        }
    }

    /// Whether the transform turns shapes over (a mirror does).
    pub fn reflects(&self) -> bool {
        matches!(self, Transform::Mirror { .. })
    }

    /// Where `point` goes, kept to the stored resolution.
    pub fn apply(&self, point: Point) -> Point {
        self.apply_exact(point).rounded()
    }

    fn apply_exact(&self, point: Point) -> Point {
        match *self {
            Transform::Translate(delta) => point.translated(delta),
            Transform::Rotate { center, angle } => {
                let (sin, cos) = angle.sin_cos();
                let offset = point.minus(center);
                // Whole quarter turns stay exact.
                let (sin, cos) = (snap_unit(sin), snap_unit(cos));
                Point::new(
                    center.x + offset.x * cos - offset.y * sin,
                    center.y + offset.x * sin + offset.y * cos,
                )
            }
            Transform::Mirror { a, b } => {
                let Some(along) = b.minus(a).normalized() else {
                    return point;
                };
                let offset = point.minus(a);
                let foot = along.scaled(offset.dot(along));
                a.translated(foot.scaled(2.0))
                    .translated(offset.scaled(-1.0))
            }
        }
    }

    /// Where a direction at `angle` radians points afterwards.
    pub fn apply_angle(&self, angle: f64) -> f64 {
        normalize_angle(match *self {
            Transform::Translate(_) => angle,
            Transform::Rotate { angle: turn, .. } => angle + turn,
            Transform::Mirror { a, b } => {
                let axis = (b.y - a.y).atan2(b.x - a.x);
                2.0 * axis - angle
            }
        })
    }

    /// A fixture's quarter turns afterwards, or `None` when it would stand
    /// askew (fixtures only stand square to the plan).
    pub fn quarter_turns(&self, turns: u8) -> Option<u8> {
        // The back of a fixture with `q` quarter turns points at (q - 1)·90°.
        let back = (f64::from(turns % 4) - 1.0) * FRAC_PI_2;
        let after = self.apply_angle(back) / FRAC_PI_2;
        let steps = after.round();
        ((after - steps).abs() < 1e-6).then(|| ((steps as i64 + 1).rem_euclid(4)) as u8)
    }

    /// The shape afterwards.
    pub fn apply_shape(&self, shape: &Shape) -> Shape {
        match shape {
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => Shape::Arc {
                center: self.apply(*center),
                radius: *radius,
                start: self.apply_angle(*start),
                sweep: if self.reflects() { -sweep } else { *sweep },
            },
            _ => {
                let mut moved = shape.clone();
                moved.for_each_point(|point| *point = self.apply(*point));
                moved
            }
        }
    }
}

/// Sine and cosine of whole quarter turns, without the 1e-17 residues.
fn snap_unit(value: f64) -> f64 {
    for exact in [-1.0, 0.0, 1.0] {
        if (value - exact).abs() < EPSILON {
            return exact;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_turn_and_reflect_exactly() {
        let quarter = Transform::rotate_degrees(Point::new(1.0, 1.0), 90.0);
        // 90° counter-clockwise on screen: right of the centre goes up (-y).
        assert_eq!(quarter.apply(Point::new(3.0, 1.0)), Point::new(1.0, -1.0));
        let mirror = Transform::mirror(Point::new(2.0, 0.0), Point::new(2.0, 5.0)).unwrap();
        assert_eq!(mirror.apply(Point::new(0.5, 3.0)), Point::new(3.5, 3.0));
        let diagonal = Transform::mirror(Point::ORIGIN, Point::new(1.0, 1.0)).unwrap();
        assert_eq!(diagonal.apply(Point::new(2.0, 0.0)), Point::new(0.0, 2.0));
        assert!(Transform::mirror(Point::ORIGIN, Point::ORIGIN).is_err());
        assert_eq!(
            Transform::Translate(Point::new(1.0, -2.0)).apply(Point::new(1.0, 1.0)),
            Point::new(2.0, -1.0)
        );
    }

    #[test]
    fn fixtures_keep_square_or_refuse() {
        // A quarter turn clockwise on screen adds one quarter turn.
        let clockwise = Transform::Rotate {
            center: Point::ORIGIN,
            angle: FRAC_PI_2,
        };
        assert_eq!(clockwise.quarter_turns(0), Some(1));
        assert_eq!(clockwise.quarter_turns(3), Some(0));
        let askew = Transform::rotate_degrees(Point::ORIGIN, 30.0);
        assert_eq!(askew.quarter_turns(0), None);
        // Across an upright axis the back stays up; across a level one it
        // turns down; left and right swap.
        let upright = Transform::mirror(Point::ORIGIN, Point::new(0.0, 1.0)).unwrap();
        assert_eq!(upright.quarter_turns(0), Some(0));
        assert_eq!(upright.quarter_turns(1), Some(3));
        let level = Transform::mirror(Point::ORIGIN, Point::new(1.0, 0.0)).unwrap();
        assert_eq!(level.quarter_turns(0), Some(2));
        assert_eq!(level.quarter_turns(1), Some(1));
    }

    #[test]
    fn arcs_keep_their_points_when_turned_or_mirrored() {
        let arc = Shape::arc_through(
            Point::new(-2.0, 0.0),
            Point::new(0.0, -2.0),
            Point::new(2.0, 0.0),
        )
        .unwrap();
        for transform in [
            Transform::rotate_degrees(Point::new(3.0, 1.0), 37.0),
            Transform::mirror(Point::new(0.0, 1.0), Point::new(4.0, 3.0)).unwrap(),
            Transform::Translate(Point::new(2.5, -1.0)),
        ] {
            let moved = transform.apply_shape(&arc);
            for point in arc.key_points() {
                let image = transform.apply(point);
                assert!(moved.distance_to(image) < 1e-4, "{transform:?}");
            }
            // The middle of the arc maps to the middle of its image.
            let middle = transform.apply(Point::new(0.0, -2.0));
            assert!(moved.distance_to(middle) < 1e-4, "{transform:?}");
            assert!(moved.validate().is_ok());
        }
        assert!((normalize_angle(3.0 * PI) - PI).abs() < 1e-12);
        assert!((normalize_angle(-PI) - PI).abs() < 1e-12);
    }
}
