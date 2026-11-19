//! Polygons drawn corner by corner: cleaning them up, checking that their
//! sides do not cross, and finding room inside them for a label.

use super::{POINT_TOLERANCE, Point, distance_to_segment, polygon_centroid, segments_cross};

/// The polygon without repeated corners, a repeated closing point or
/// corners that sit on a straight edge (such as where a wall was split).
pub fn simplify_polygon(vertices: &[Point]) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::with_capacity(vertices.len());
    for &vertex in vertices {
        if points
            .last()
            .is_none_or(|last| !last.near(vertex, POINT_TOLERANCE))
        {
            points.push(vertex);
        }
    }
    while points.len() > 1 && points[0].near(points[points.len() - 1], POINT_TOLERANCE) {
        points.pop();
    }
    // Drop corners that lie on the line between their neighbours.
    let mut changed = true;
    while changed && points.len() > 3 {
        changed = false;
        for index in 0..points.len() {
            let previous = points[(index + points.len() - 1) % points.len()];
            let next = points[(index + 1) % points.len()];
            if distance_to_segment(points[index], previous, next) < POINT_TOLERANCE {
                points.remove(index);
                changed = true;
                break;
            }
        }
    }
    points
}

/// Whether no two edges of the polygon cross or touch away from their
/// shared corners.
pub fn polygon_is_simple(vertices: &[Point]) -> bool {
    let n = vertices.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if vertices[i].near(vertices[j], POINT_TOLERANCE) {
                return false;
            }
        }
    }
    for i in 0..n {
        let (a, b) = (vertices[i], vertices[(i + 1) % n]);
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (c, d) = (vertices[j], vertices[(j + 1) % n]);
            if segments_cross(a, b, c, d)
                || distance_to_segment(c, a, b) < POINT_TOLERANCE
                || distance_to_segment(a, c, d) < POINT_TOLERANCE
            {
                return false;
            }
        }
    }
    true
}

pub fn polygon_perimeter(vertices: &[Point]) -> f64 {
    (0..vertices.len())
        .map(|i| vertices[i].distance(vertices[(i + 1) % vertices.len()]))
        .sum()
}

/// Where a label fits inside the polygon, even an L or a U whose centroid
/// falls outside it: the centre of the widest horizontal span through it,
/// with that span's width and the height of the vertical span there.
pub fn interior_box(vertices: &[Point]) -> Option<(Point, f64, f64)> {
    if vertices.len() < 3 {
        return None;
    }
    // Inside stretches along a horizontal (`horizontal`) or vertical line.
    let spans = |level: f64, horizontal: bool| -> Vec<(f64, f64)> {
        let mut crossings = Vec::new();
        for i in 0..vertices.len() {
            let (a, b) = (vertices[i], vertices[(i + 1) % vertices.len()]);
            let (a_level, b_level, a_pos, b_pos) = if horizontal {
                (a.y, b.y, a.x, b.x)
            } else {
                (a.x, b.x, a.y, b.y)
            };
            if (a_level > level) != (b_level > level) {
                crossings.push(a_pos + (level - a_level) / (b_level - a_level) * (b_pos - a_pos));
            }
        }
        crossings.sort_by(f64::total_cmp);
        crossings
            .chunks(2)
            .filter(|pair| pair.len() == 2)
            .map(|pair| (pair[0], pair[1]))
            .collect()
    };
    let (min_y, max_y) = vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), p| {
            (low.min(p.y), high.max(p.y))
        });
    let centroid = polygon_centroid(vertices);
    let mut best: Option<(Point, f64, f64)> = None;
    // Try the centroid's level first, then a few others across the shape.
    let levels = std::iter::once(centroid.y)
        .chain((1..8).map(|step| min_y + (max_y - min_y) * f64::from(step) / 8.0));
    for level in levels {
        for (left, right) in spans(level, true) {
            let x = (left + right) / 2.0;
            let Some((bottom, top)) = spans(x, false)
                .into_iter()
                .find(|(low, high)| *low <= level && level <= *high)
            else {
                continue;
            };
            let (width, height) = (right - left, top - bottom);
            let center = Point::new(x, (bottom + top) / 2.0);
            // Prefer the box that holds the most, as a square-ish area.
            let score = width.min(height * 2.5) * height.min(width);
            if best.is_none_or(|(_, w, h)| score > w.min(h * 2.5) * h.min(w)) {
                best = Some((center, width, height));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::polygon_contains_point;

    #[test]
    fn polygons_are_simplified_checked_and_labelled_inside() {
        let l_shape = vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 0.0),
        ];
        let simple = simplify_polygon(&l_shape);
        assert_eq!(simple.len(), 6);
        assert!(polygon_is_simple(&simple));
        assert_eq!(polygon_perimeter(&simple), 16.0);
        let bowtie = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(4.0, 0.0),
            Point::new(0.0, 4.0),
        ];
        assert!(!polygon_is_simple(&bowtie));

        // A U: the centroid is in the notch, but the label goes in a leg.
        let u_shape = [
            Point::new(0.0, 0.0),
            Point::new(6.0, 0.0),
            Point::new(6.0, 5.0),
            Point::new(4.0, 5.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 5.0),
            Point::new(0.0, 5.0),
        ];
        let (center, width, height) = interior_box(&u_shape).unwrap();
        assert!(polygon_contains_point(&u_shape, center));
        assert!(width > 1.9 && height > 1.9);
    }
}
