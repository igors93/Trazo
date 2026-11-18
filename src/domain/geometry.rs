use serde::{Deserialize, Serialize};

pub const GRID_METERS: f32 = 0.25;
pub const WALL_THICKNESS_METERS: f32 = 0.12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn snapped_to(self, step: f32) -> Self {
        Self::new(
            (self.x / step).round() * step,
            (self.y / step).round() * step,
        )
    }

    pub fn translated(self, delta: Self) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y)
    }
}

pub fn nearest_point_on_segment(point: Point, start: Point, end: Point) -> (Point, f32) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return (start, 0.0);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    (Point::new(start.x + t * dx, start.y + t * dy), t)
}

pub fn format_meters(value: f32) -> String {
    format!("{value:.2} m").replace('.', ",")
}

pub fn polygon_area(vertices: &[Point]) -> f32 {
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
        return Point::new(0.0, 0.0);
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
        let sum_x: f32 = vertices.iter().map(|p| p.x).sum();
        let sum_y: f32 = vertices.iter().map(|p| p.y).sum();
        return Point::new(sum_x / n as f32, sum_y / n as f32);
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
    let sign = |p1: Point, p2: Point, p3: Point| -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < -1e-5) || (d2 < -1e-5) || (d3 < -1e-5);
    let has_pos = (d1 > 1e-5) || (d2 > 1e-5) || (d3 > 1e-5);
    !(has_neg && has_pos)
}

fn segments_cross(p1: Point, p2: Point, p3: Point, p4: Point) -> bool {
    let ccw = |a: Point, b: Point, c: Point| -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    };
    let d1 = ccw(p1, p2, p3);
    let d2 = ccw(p1, p2, p4);
    let d3 = ccw(p3, p4, p1);
    let d4 = ccw(p3, p4, p2);

    ((d1 > 1e-4 && d2 < -1e-4) || (d1 < -1e-4 && d2 > 1e-4))
        && ((d3 > 1e-4 && d4 < -1e-4) || (d3 < -1e-4 && d4 > 1e-4))
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

    let tri_sum: f32 = triangles
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
        let area: f32 = tris
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
        let l_area: f32 = l_tris
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
