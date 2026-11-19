//! Points, segments and polygons in metres, computed in double precision.
//!
//! Every comparison goes through a named tolerance, so "the same point" or
//! "on the wall" means the same thing everywhere in the editor.

use serde::{Deserialize, Serialize};

mod polygon;
pub use polygon::{interior_box, polygon_is_simple, polygon_perimeter, simplify_polygon};

#[cfg(test)]
mod property_tests;

pub const GRID_METERS: f64 = 0.25;
pub const WALL_THICKNESS_METERS: f64 = 0.12;

/// Below this two values are numerically equal (a nanometre).
pub const EPSILON: f64 = 1e-9;
/// Stored coordinates are kept to a hundredth of a millimetre, which also
/// wipes out the noise of older single-precision files.
const STEPS_PER_METRE: f64 = 100_000.0;
/// Points closer than this are the same point on a drawing (half a millimetre).
pub const POINT_TOLERANCE: f64 = 5e-4;
/// Wall ends closer than this are joined (a centimetre).
pub const JOIN_TOLERANCE: f64 = 0.01;

/// `value` rounded to the stored resolution.
pub fn round_to_resolution(value: f64) -> f64 {
    // Dividing by a whole number gives the double nearest the decimal value
    // (4.12 and not 4.120000000000001).
    let rounded = (value * STEPS_PER_METRE).round() / STEPS_PER_METRE;
    // Avoid "-0" in files and on screen.
    if rounded == 0.0 { 0.0 } else { rounded }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Whether `other` is within `tolerance` metres.
    pub fn near(self, other: Self, tolerance: f64) -> bool {
        self.distance(other) <= tolerance
    }

    pub fn snapped_to(self, step: f64) -> Self {
        Self::new(
            (self.x / step).round() * step,
            (self.y / step).round() * step,
        )
    }

    pub fn translated(self, delta: Self) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y)
    }

    pub fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    pub fn scaled(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// The z component of the cross product of the two vectors.
    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// The unit vector in this direction, or `None` for a null vector.
    pub fn normalized(self) -> Option<Self> {
        let length = self.length();
        (length > EPSILON).then(|| self.scaled(1.0 / length))
    }

    /// The point a fraction `t` of the way to `other`.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }

    pub fn midpoint(self, other: Self) -> Self {
        self.lerp(other, 0.5)
    }

    /// Rounded to the stored resolution.
    pub fn rounded(self) -> Self {
        Self::new(round_to_resolution(self.x), round_to_resolution(self.y))
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// Where the lines through `a`-`b` and `c`-`d` cross, with the parameters
/// along each (`a + t(b - a)`, `c + u(d - c)`); `None` when parallel.
pub fn line_intersection(a: Point, b: Point, c: Point, d: Point) -> Option<(Point, f64, f64)> {
    let r = b.minus(a);
    let s = d.minus(c);
    let denominator = r.cross(s);
    // Parallel within a micro-radian.
    if denominator.abs() <= 1e-6 * r.length() * s.length() {
        return None;
    }
    let offset = c.minus(a);
    let t = offset.cross(s) / denominator;
    let u = offset.cross(r) / denominator;
    Some((a.lerp(b, t), t, u))
}

/// Distance from `point` to the segment `start`-`end`.
pub fn distance_to_segment(point: Point, start: Point, end: Point) -> f64 {
    nearest_point_on_segment(point, start, end)
        .0
        .distance(point)
}

pub fn nearest_point_on_segment(point: Point, start: Point, end: Point) -> (Point, f64) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= EPSILON * EPSILON {
        return (start, 0.0);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    (Point::new(start.x + t * dx, start.y + t * dy), t)
}

pub fn format_meters(value: f64) -> String {
    format!("{value:.2} m").replace('.', ",")
}

pub fn polygon_area(vertices: &[Point]) -> f64 {
    if vertices.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += vertices[i].x * vertices[j].y - vertices[j].x * vertices[i].y;
    }
    (area / 2.0).abs()
}

pub fn polygon_centroid(vertices: &[Point]) -> Point {
    if vertices.len() < 3 {
        return Point::ORIGIN;
    }
    let mut signed_area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let factor = vertices[i].x * vertices[j].y - vertices[j].x * vertices[i].y;
        signed_area += factor;
        cx += (vertices[i].x + vertices[j].x) * factor;
        cy += (vertices[i].y + vertices[j].y) * factor;
    }
    signed_area *= 0.5;
    if signed_area.abs() < 1e-4 {
        let sum_x: f64 = vertices.iter().map(|p| p.x).sum();
        let sum_y: f64 = vertices.iter().map(|p| p.y).sum();
        return Point::new(sum_x / n as f64, sum_y / n as f64);
    }
    Point::new(cx / (6.0 * signed_area), cy / (6.0 * signed_area))
}

pub fn polygon_contains_point(vertices: &[Point], point: Point) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = vertices.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = vertices[i];
        let pj = vertices[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn triangle_contains_point(a: Point, b: Point, c: Point, p: Point) -> bool {
    let sign = |p1: Point, p2: Point, p3: Point| -> f64 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < -1e-5) || (d2 < -1e-5) || (d3 < -1e-5);
    let has_pos = (d1 > 1e-5) || (d2 > 1e-5) || (d3 > 1e-5);
    !(has_neg && has_pos)
}

/// Whether the segments cross at a point inside both (touching ends do
/// not count).
fn segments_cross(p1: Point, p2: Point, p3: Point, p4: Point) -> bool {
    let Some((_, t, u)) = line_intersection(p1, p2, p3, p4) else {
        return false;
    };
    let inner = |value: f64, length: f64| {
        let margin = POINT_TOLERANCE / length.max(EPSILON);
        value > margin && value < 1.0 - margin
    };
    inner(t, p1.distance(p2)) && inner(u, p3.distance(p4))
}

pub fn triangulate_polygon(vertices: &[Point]) -> Vec<[Point; 3]> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }
    let total_area = polygon_area(vertices);
    if total_area < 1e-4 {
        return Vec::new();
    }

    // Check for self-intersection between non-adjacent edges
    for i in 0..n {
        let p1 = vertices[i];
        let p2 = vertices[(i + 1) % n];
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let p3 = vertices[j];
            let p4 = vertices[(j + 1) % n];
            if segments_cross(p1, p2, p3, p4) {
                return Vec::new();
            }
        }
    }

    if n == 3 {
        return vec![[vertices[0], vertices[1], vertices[2]]];
    }

    // Ear clipping triangulation for simple polygons
    let mut indices: Vec<usize> = (0..n).collect();
    let mut triangles = Vec::with_capacity(n - 2);

    // Determine winding orientation
    let mut signed_area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        signed_area += vertices[i].x * vertices[j].y - vertices[j].x * vertices[i].y;
    }
    let ccw = signed_area >= 0.0;

    let mut iterations = 0;
    let max_iterations = n * n;
    while indices.len() > 3 && iterations < max_iterations {
        iterations += 1;
        let mut ear_found = false;
        let count = indices.len();
        for i in 0..count {
            let prev = indices[(i + count - 1) % count];
            let curr = indices[i];
            let next = indices[(i + 1) % count];
            let a = vertices[prev];
            let b = vertices[curr];
            let c = vertices[next];

            let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            let convex = if ccw { cross > 1e-5 } else { cross < -1e-5 };
            if !convex {
                continue;
            }

            // Check if any other vertex lies inside the candidate ear
            let mut has_inside = false;
            for &idx in &indices {
                if idx != prev
                    && idx != curr
                    && idx != next
                    && triangle_contains_point(a, b, c, vertices[idx])
                {
                    has_inside = true;
                    break;
                }
            }
            if !has_inside {
                triangles.push([a, b, c]);
                indices.remove(i);
                ear_found = true;
                break;
            }
        }
        if !ear_found {
            return Vec::new();
        }
    }

    if indices.len() == 3 {
        triangles.push([
            vertices[indices[0]],
            vertices[indices[1]],
            vertices[indices[2]],
        ]);
    }

    if triangles.len() != n - 2 {
        return Vec::new();
    }

    let tri_sum: f64 = triangles
        .iter()
        .map(|t| {
            ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x)).abs()
                / 2.0
        })
        .sum();
    if (tri_sum - total_area).abs() > 0.05 {
        return Vec::new();
    }

    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_precision_keeps_fractions_of_a_millimetre() {
        // 2 km from the origin a tenth of a millimetre still shows.
        let far = Point::new(2000.0001, 0.0);
        assert!((far.x - 2000.0).abs() > 5e-5);
        assert_eq!(
            Point::new(4.1200004, -0.0000001).rounded(),
            Point::new(4.12, 0.0)
        );
        assert!(Point::new(1.0, 1.0).near(Point::new(1.0004, 1.0), POINT_TOLERANCE));
    }

    #[test]
    fn lines_cross_where_expected_and_parallels_never() {
        assert!(segments_cross(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
        ));
        // Sharing an end is not crossing.
        assert!(!segments_cross(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
        ));
        let (beyond, t, _) = line_intersection(
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(5.0, -1.0),
            Point::new(5.0, 1.0),
        )
        .unwrap();
        assert!(beyond.near(Point::new(5.0, 0.0), 1e-12) && (t - 1.25).abs() < 1e-12);
        assert!(
            line_intersection(
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
                Point::new(1.0, 1.0),
            )
            .is_none()
        );

        let gap = distance_to_segment(
            Point::new(2.0, 3.0),
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
        );
        assert!((gap - 3.0).abs() < 1e-12);
    }

    #[test]
    fn projection_stays_on_segment() {
        let (point, t) = nearest_point_on_segment(
            Point::new(5.0, 3.0),
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
        );
        assert_eq!(point, Point::new(4.0, 0.0));
        assert_eq!(t, 1.0);
    }

    #[test]
    fn polygon_area_and_point_in_polygon() {
        let poly = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
            Point::new(0.0, 3.0),
        ];
        assert_eq!(polygon_area(&poly), 12.0);
        assert_eq!(polygon_centroid(&poly), Point::new(2.0, 1.5));
        assert!(polygon_contains_point(&poly, Point::new(2.0, 1.5)));
        assert!(!polygon_contains_point(&poly, Point::new(5.0, 1.5)));

        let tris = triangulate_polygon(&poly);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn triangulate_concave_quad_and_complex_polygons() {
        // Concave quad (dart)
        let dart = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(2.0, 1.0),
            Point::new(4.0, 2.0),
        ];
        let tris = triangulate_polygon(&dart);
        assert_eq!(tris.len(), 2);
        let area: f64 = tris
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x))
                    .abs()
                    / 2.0
            })
            .sum();
        assert!((area - polygon_area(&dart)).abs() < 1e-3);

        // L-shaped concave 6-vertex polygon
        let l_shape = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        let l_tris = triangulate_polygon(&l_shape);
        assert_eq!(l_tris.len(), 4);
        let l_area: f64 = l_tris
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x))
                    .abs()
                    / 2.0
            })
            .sum();
        assert!((l_area - 12.0).abs() < 1e-3);

        // Degenerate collinear polygon (area ≈ 0)
        let degen = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
        ];
        assert!(triangulate_polygon(&degen).is_empty());

        // Self-intersecting polygon (hourglass/bowtie)
        let bowtie = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(4.0, 0.0),
            Point::new(0.0, 4.0),
        ];
        assert!(triangulate_polygon(&bowtie).is_empty());
    }
}
