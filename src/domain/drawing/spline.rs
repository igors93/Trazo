//! Smooth curves through given points: a centripetal Catmull-Rom spline,
//! which passes through every point without loops or overshoot at sharp
//! turns.

use crate::domain::geometry::Point;

/// The curve through `points` as `per_segment` straight pieces between
/// each pair of points; closed curves end where they start.
pub fn spline_points(points: &[Point], closed: bool, per_segment: usize) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        let mut path = points.to_vec();
        if closed && let Some(first) = points.first() {
            path.push(*first);
        }
        return path;
    }
    let at = |index: isize| -> Point {
        if closed {
            points[index.rem_euclid(n as isize) as usize]
        } else if index < 0 {
            // Mirror the first point beyond the start.
            let (a, b) = (points[0], points[1]);
            Point::new(2.0 * a.x - b.x, 2.0 * a.y - b.y)
        } else if index as usize >= n {
            let (a, b) = (points[n - 1], points[n - 2]);
            Point::new(2.0 * a.x - b.x, 2.0 * a.y - b.y)
        } else {
            points[index as usize]
        }
    };
    let segments = if closed { n } else { n - 1 };
    let steps = per_segment.max(1);
    let mut path = Vec::with_capacity(segments * steps + 1);
    path.push(points[0]);
    for segment in 0..segments as isize {
        let (p0, p1, p2, p3) = (
            at(segment - 1),
            at(segment),
            at(segment + 1),
            at(segment + 2),
        );
        // Knot spacing by the square root of distance (centripetal).
        let knot = |a: Point, b: Point| a.distance(b).sqrt().max(1e-6);
        let t1 = knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        for step in 1..=steps {
            let t = t1 + (t2 - t1) * step as f64 / steps as f64;
            let blend = |a: Point, b: Point, ta: f64, tb: f64| {
                a.scaled((tb - t) / (tb - ta))
                    .translated(b.scaled((t - ta) / (tb - ta)))
            };
            let a1 = blend(p0, p1, 0.0, t1);
            let a2 = blend(p1, p2, t1, t2);
            let a3 = blend(p2, p3, t2, t3);
            let b1 = blend(a1, a2, 0.0, t2);
            let b2 = blend(a2, a3, t1, t3);
            path.push(if step == steps {
                p2
            } else {
                blend(b1, b2, t1, t2)
            });
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_splines_return_to_the_start() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let path = spline_points(&square, true, 8);
        assert_eq!(path.len(), 4 * 8 + 1);
        assert_eq!(path.first(), path.last());
        // A smooth loop bulges past the straight sides a little, not much.
        let (min_x, max_x) = path
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p.x), hi.max(p.x))
            });
        assert!(min_x < 0.0 && min_x > -0.5 && max_x > 2.0 && max_x < 2.5);
    }
}
