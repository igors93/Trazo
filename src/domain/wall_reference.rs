//! Wall alignment reference: centreline (eixo), external finished face or
//! internal finished face.

use serde::{Deserialize, Serialize};

use super::geometry::{EPSILON, Point, line_intersection, polygon_is_simple, simplify_polygon};

/// Which line of a wall the drawn points follow: its centreline, or its
/// external (left) or internal (right) finished face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallReference {
    #[default]
    Center,
    External,
    Internal,
}

impl WallReference {
    pub const ALL: [WallReference; 3] = [
        WallReference::Center,
        WallReference::External,
        WallReference::Internal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WallReference::Center => "Eixo",
            WallReference::External => "Face externa",
            WallReference::Internal => "Face interna",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            WallReference::Center => "eixo",
            WallReference::External => "face ext.",
            WallReference::Internal => "face int.",
        }
    }

    pub fn next(self) -> Self {
        match self {
            WallReference::Center => WallReference::External,
            WallReference::External => WallReference::Internal,
            WallReference::Internal => WallReference::Center,
        }
    }

    /// Shifts a wall segment `(start, end)` from its reference line to the
    /// wall's centreline for `thickness` metres.
    ///
    /// In plan coordinates (+x right, +y down), `External` places the outer
    /// face along `start -> end` (shifting the centreline to the right /
    /// clockwise interior), while `Internal` places the inner face along
    /// `start -> end` (shifting the centreline to the left / outward).
    pub fn offset_segment(self, start: Point, end: Point, thickness: f64) -> (Point, Point) {
        let sign = match self {
            WallReference::Center => return (start, end),
            WallReference::External => 1.0,
            WallReference::Internal => -1.0,
        };
        let Some(dir) = end.minus(start).normalized() else {
            return (start, end);
        };
        // Clockwise (right-hand) unit normal in plan coordinates (+x right, +y down).
        let normal = Point::new(-dir.y, dir.x).scaled(sign * thickness * 0.5);
        (
            start.translated(normal).rounded(),
            end.translated(normal).rounded(),
        )
    }

    /// Computes the wall-centreline rectangle `(min, max)` corresponding to a
    /// drawn rectangle `(a, b)` referenced by `self`.
    pub fn offset_rect(self, a: Point, b: Point, thickness: f64) -> Option<(Point, Point)> {
        let min = Point::new(a.x.min(b.x), a.y.min(b.y));
        let max = Point::new(a.x.max(b.x), a.y.max(b.y));
        let half = thickness * 0.5;
        match self {
            WallReference::Center => Some((min, max)),
            WallReference::External => {
                let nmin = Point::new(min.x + half, min.y + half).rounded();
                let nmax = Point::new(max.x - half, max.y - half).rounded();
                (nmax.x - nmin.x > EPSILON && nmax.y - nmin.y > EPSILON).then_some((nmin, nmax))
            }
            WallReference::Internal => {
                let nmin = Point::new(min.x - half, min.y - half).rounded();
                let nmax = Point::new(max.x + half, max.y + half).rounded();
                Some((nmin, nmax))
            }
        }
    }

    /// Offsets a simple closed polygon from its reference face (`External`
    /// inward, `Internal` outward) to the wall-centreline polygon.
    pub fn offset_polygon(self, corners: &[Point], thickness: f64) -> Option<Vec<Point>> {
        if self == WallReference::Center {
            return Some(corners.to_vec());
        }
        let n = corners.len();
        if n < 3 {
            return None;
        }
        // Signed area in plan coordinates (+x right, +y down): positive = clockwise.
        let signed_twice: f64 = (0..n)
            .map(|i| {
                let a = corners[i];
                let b = corners[(i + 1) % n];
                a.x * b.y - b.x * a.y
            })
            .sum();
        if signed_twice.abs() <= EPSILON {
            return None;
        }
        let cw_sign = if signed_twice > 0.0 { 1.0 } else { -1.0 };
        let ref_sign = match self {
            WallReference::External => 1.0,  // inward
            WallReference::Internal => -1.0, // outward
            WallReference::Center => 0.0,
        };
        let shift = cw_sign * ref_sign * thickness * 0.5;

        let mut shifted_edges = Vec::with_capacity(n);
        for i in 0..n {
            let a = corners[i];
            let b = corners[(i + 1) % n];
            let dir = b.minus(a).normalized()?;
            let normal = Point::new(-dir.y, dir.x).scaled(shift);
            shifted_edges.push((a.translated(normal), b.translated(normal), normal));
        }

        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let (p1, p2, n1) = shifted_edges[(i + n - 1) % n];
            let (p3, p4, _) = shifted_edges[i];
            let corner = match line_intersection(p1, p2, p3, p4) {
                Some((pt, _, _)) if pt.is_finite() => pt.rounded(),
                _ => corners[i].translated(n1).rounded(),
            };
            out.push(corner);
        }
        let simplified = simplify_polygon(&out);
        (simplified.len() >= 3 && polygon_is_simple(&simplified)).then_some(simplified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::polygon_area;

    #[test]
    fn wall_reference_offsets_segments_rectangles_and_polygons() {
        let t = 0.20;
        // Horizontal wall left -> right: External shifts down (+y), Internal shifts up (-y).
        let (es, ee) =
            WallReference::External.offset_segment(Point::new(0.0, 0.0), Point::new(4.0, 0.0), t);
        assert!(es.near(Point::new(0.0, 0.10), 1e-6));
        assert!(ee.near(Point::new(4.0, 0.10), 1e-6));

        let (is, ie) =
            WallReference::Internal.offset_segment(Point::new(0.0, 0.0), Point::new(4.0, 0.0), t);
        assert!(is.near(Point::new(0.0, -0.10), 1e-6));
        assert!(ie.near(Point::new(4.0, -0.10), 1e-6));

        // Vertical wall top -> bottom: External shifts left (-x), Internal shifts right (+x).
        let (ves, vee) =
            WallReference::External.offset_segment(Point::new(5.0, 0.0), Point::new(5.0, 4.0), t);
        assert!(ves.near(Point::new(4.90, 0.0), 1e-6));
        assert!(vee.near(Point::new(4.90, 4.0), 1e-6));

        // Rectangle 3x4 referenced by Internal expands centrelines to 3.20x4.20 so clear interior is 3x4.
        let (rmin, rmax) = WallReference::Internal
            .offset_rect(Point::new(0.0, 0.0), Point::new(3.0, 4.0), t)
            .unwrap();
        assert!(rmin.near(Point::new(-0.10, -0.10), 1e-6));
        assert!(rmax.near(Point::new(3.10, 4.10), 1e-6));

        // Rectangle 3x4 referenced by External shrinks centrelines to 2.80x3.80 so exterior footprint is 3x4.
        let (emin, emax) = WallReference::External
            .offset_rect(Point::new(0.0, 0.0), Point::new(3.0, 4.0), t)
            .unwrap();
        assert!(emin.near(Point::new(0.10, 0.10), 1e-6));
        assert!(emax.near(Point::new(2.90, 3.90), 1e-6));

        // Too small for External returns None.
        assert!(
            WallReference::External
                .offset_rect(Point::new(0.0, 0.0), Point::new(0.15, 0.15), t)
                .is_none()
        );

        // L-shaped polygon works regardless of clockwise vs counter-clockwise winding.
        let l_cw = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        let l_ccw: Vec<Point> = l_cw.iter().copied().rev().collect();
        let ext_cw = WallReference::External.offset_polygon(&l_cw, t).unwrap();
        let ext_ccw = WallReference::External.offset_polygon(&l_ccw, t).unwrap();
        assert!((polygon_area(&ext_cw) - polygon_area(&ext_ccw)).abs() < 1e-4);
        assert!(polygon_area(&ext_cw) < polygon_area(&l_cw));

        let int_cw = WallReference::Internal.offset_polygon(&l_cw, t).unwrap();
        assert!(polygon_area(&int_cw) > polygon_area(&l_cw));

        assert_eq!(WallReference::Center.next(), WallReference::External);
        assert_eq!(WallReference::External.next(), WallReference::Internal);
        assert_eq!(WallReference::Internal.next(), WallReference::Center);
    }

    #[test]
    fn diagonal_wall_reference_preserves_length_and_exact_perpendicular_offset() {
        let start = Point::new(0.0, 0.0);
        let end = Point::new(3.0, 4.0); // length 5.0 m
        let thickness = 0.24;
        for reference in [WallReference::External, WallReference::Internal] {
            let (s, e) = reference.offset_segment(start, end, thickness);
            assert!((s.distance(e) - 5.0).abs() < 1e-4);
            assert!((s.distance(start) - thickness * 0.5).abs() < 1e-4);
            assert!((e.distance(end) - thickness * 0.5).abs() < 1e-4);
            // Offset vector must be perpendicular to the wall direction.
            let wall_dir = end.minus(start);
            let shift = s.minus(start);
            assert!(wall_dir.dot(shift).abs() < 1e-4);
        }
    }
}
