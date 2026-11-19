//! Offsetting drawing shapes: a parallel copy at a given distance, on the
//! side of a point the person picks (CAD's OFFSET).

use super::drawing::Shape;
use super::geometry::{EPSILON, Point, distance_to_segment, line_intersection, polygon_is_simple};

/// The smallest distance worth offsetting by (a millimetre).
pub const MIN_OFFSET: f64 = 1e-3;

/// The unit normal to the left of `a`→`b` in plan coordinates.
fn left_normal(a: Point, b: Point) -> Option<Point> {
    b.minus(a)
        .normalized()
        .map(|along| Point::new(-along.y, along.x))
}

/// +1 when `toward` is on the left of the nearest piece of `path`, -1 on
/// its right; `None` when it lies on it.
fn side_of(path: &[Point], toward: Point) -> Option<f64> {
    let (a, b) = path
        .windows(2)
        .filter(|pair| pair[0].distance(pair[1]) > EPSILON)
        .map(|pair| (pair[0], pair[1]))
        .min_by(|x, y| {
            distance_to_segment(toward, x.0, x.1).total_cmp(&distance_to_segment(toward, y.0, y.1))
        })?;
    let normal = left_normal(a, b)?;
    let side = toward.minus(a).dot(normal);
    (side.abs() > EPSILON).then_some(side.signum())
}

/// The corners of a chain of points moved `distance` to one side, each
/// corner where the moved sides meet (mitred).
fn offset_chain(points: &[Point], closed: bool, distance: f64) -> Result<Vec<Point>, String> {
    let n = points.len();
    let segments = if closed { n } else { n - 1 };
    let mut moved: Vec<(Point, Point)> = Vec::with_capacity(segments);
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let normal = left_normal(a, b).ok_or("O desenho tem pontos repetidos.")?;
        let shift = normal.scaled(distance);
        moved.push((a.translated(shift), b.translated(shift)));
    }
    let corner = |before: (Point, Point), after: (Point, Point)| {
        line_intersection(before.0, before.1, after.0, after.1)
            .map(|(point, _, _)| point)
            // In line: the shared end.
            .unwrap_or(after.0)
    };
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let point = if closed {
            corner(moved[(i + segments - 1) % segments], moved[i % segments])
        } else if i == 0 {
            moved[0].0
        } else if i == n - 1 {
            moved[segments - 1].1
        } else {
            corner(moved[i - 1], moved[i])
        };
        result.push(point.rounded());
    }
    // Each side must keep its direction: a side that flips means the
    // distance swallowed it.
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let (c, d) = (result[i], result[(i + 1) % n]);
        if b.minus(a).dot(d.minus(c)) <= EPSILON || c.distance(d) < MIN_OFFSET {
            return Err("A distância é grande demais para esta forma desse lado.".to_owned());
        }
    }
    if closed && !polygon_is_simple(&result) {
        return Err("A forma deslocada se cruzaria; use uma distância menor.".to_owned());
    }
    Ok(result)
}

/// `shape` offset by `distance` towards `toward`.
pub fn offset_shape(shape: &Shape, distance: f64, toward: Point) -> Result<Shape, String> {
    if !distance.is_finite() || distance < MIN_OFFSET {
        return Err("Informe uma distância de deslocamento maior que zero.".to_owned());
    }
    if !toward.is_finite() {
        return Err("Ponto inválido.".to_owned());
    }
    let on_the_line =
        || "Clique ao lado do desenho, não sobre ele, para escolher o lado.".to_owned();
    let shape = match shape {
        Shape::Line { start, end } => {
            let normal = left_normal(*start, *end).ok_or("Linha sem comprimento.")?;
            let side = side_of(&[*start, *end], toward).ok_or_else(on_the_line)?;
            let shift = normal.scaled(side * distance);
            Shape::Line {
                start: start.translated(shift).rounded(),
                end: end.translated(shift).rounded(),
            }
        }
        Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => {
            let from_center = center.distance(toward);
            if (from_center - radius).abs() <= EPSILON {
                return Err(on_the_line());
            }
            let radius = if from_center < *radius {
                radius - distance
            } else {
                radius + distance
            };
            if radius < MIN_OFFSET {
                return Err(
                    "A distância é maior que o raio: não sobra curva para dentro.".to_owned(),
                );
            }
            match shape {
                Shape::Arc { start, sweep, .. } => Shape::Arc {
                    center: *center,
                    radius,
                    start: *start,
                    sweep: *sweep,
                },
                _ => Shape::Circle {
                    center: *center,
                    radius,
                },
            }
        }
        Shape::Polyline { points, closed } => {
            let mut path = points.clone();
            if *closed {
                path.push(points[0]);
            }
            let side = side_of(&path, toward).ok_or_else(on_the_line)?;
            Shape::Polyline {
                points: offset_chain(points, *closed, side * distance)?,
                closed: *closed,
            }
        }
        Shape::Spline { points, closed } => {
            // The curve's own side, then each control point moved along the
            // curve's normal there.
            let side = side_of(&shape.outline(), toward).ok_or_else(on_the_line)?;
            let n = points.len();
            let mut moved = Vec::with_capacity(n);
            for i in 0..n {
                let (before, after) = if *closed {
                    (points[(i + n - 1) % n], points[(i + 1) % n])
                } else {
                    (points[i.saturating_sub(1)], points[(i + 1).min(n - 1)])
                };
                let normal = left_normal(before, after).ok_or("O desenho tem pontos repetidos.")?;
                moved.push(
                    points[i]
                        .translated(normal.scaled(side * distance))
                        .rounded(),
                );
            }
            let result = Shape::Spline {
                points: moved,
                closed: *closed,
            };
            // The offset curve must stay on the chosen side all along.
            let original = shape.outline();
            if result
                .outline()
                .iter()
                .any(|point| side_of(&original, *point) != Some(side))
            {
                return Err("A distância é grande demais para esta curva desse lado.".to_owned());
            }
            result
        }
    };
    shape.validate()?;
    Ok(shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_rectangles_offset_to_the_picked_side() {
        let line = Shape::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(4.0, 0.0),
        };
        assert_eq!(
            offset_shape(&line, 0.5, Point::new(2.0, 3.0)).unwrap(),
            Shape::Line {
                start: Point::new(0.0, 0.5),
                end: Point::new(4.0, 0.5)
            }
        );
        assert!(offset_shape(&line, 0.5, Point::new(2.0, 0.0)).is_err());
        assert!(offset_shape(&line, 0.0, Point::new(2.0, 1.0)).is_err());

        let rectangle = Shape::rectangle(Point::new(0.0, 0.0), Point::new(4.0, 2.0));
        let outside = offset_shape(&rectangle, 0.5, Point::new(5.0, 1.0)).unwrap();
        assert_eq!(
            outside.bounds(),
            (Point::new(-0.5, -0.5), Point::new(4.5, 2.5))
        );
        assert!(outside.is_rectangle());
        let inside = offset_shape(&rectangle, 0.5, Point::new(2.0, 1.0)).unwrap();
        assert_eq!(
            inside.bounds(),
            (Point::new(0.5, 0.5), Point::new(3.5, 1.5))
        );
        // 1,5 m inwards swallows a 2 m wide rectangle.
        assert!(offset_shape(&rectangle, 1.5, Point::new(2.0, 1.0)).is_err());
    }

    #[test]
    fn open_polylines_mitre_their_corners() {
        let elbow = Shape::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 0.0),
                Point::new(4.0, 3.0),
            ],
            closed: false,
        };
        let Shape::Polyline { points, .. } =
            offset_shape(&elbow, 1.0, Point::new(2.0, 1.0)).unwrap()
        else {
            panic!("not a polyline");
        };
        assert_eq!(
            points,
            vec![
                Point::new(0.0, 1.0),
                Point::new(3.0, 1.0),
                Point::new(3.0, 3.0)
            ]
        );
    }

    #[test]
    fn circles_arcs_and_splines_offset_too() {
        let circle = Shape::Circle {
            center: Point::ORIGIN,
            radius: 2.0,
        };
        assert_eq!(
            offset_shape(&circle, 0.5, Point::new(0.5, 0.0)).unwrap(),
            Shape::Circle {
                center: Point::ORIGIN,
                radius: 1.5
            }
        );
        assert_eq!(
            offset_shape(&circle, 0.5, Point::new(3.0, 0.0)).unwrap(),
            Shape::Circle {
                center: Point::ORIGIN,
                radius: 2.5
            }
        );
        assert!(offset_shape(&circle, 2.5, Point::new(0.5, 0.0)).is_err());

        let spline = Shape::Spline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 1.0),
                Point::new(4.0, 0.0),
            ],
            closed: false,
        };
        let moved = offset_shape(&spline, 0.2, Point::new(2.0, 3.0)).unwrap();
        // About 0,2 m from the original all along.
        for point in moved.outline() {
            let gap = spline.distance_to(point);
            assert!((gap - 0.2).abs() < 0.05, "{gap}");
        }
    }
}
