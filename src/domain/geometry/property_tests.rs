use proptest::prelude::*;

use super::*;

fn triangle_area(triangle: &[Point; 3]) -> f64 {
    triangle[1]
        .minus(triangle[0])
        .cross(triangle[2].minus(triangle[0]))
        .abs()
        / 2.0
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn perpendicular_lines_intersect_at_their_shared_center(
        center_x in -10_000.0f64..10_000.0,
        center_y in -10_000.0f64..10_000.0,
        angle in -std::f64::consts::PI..std::f64::consts::PI,
        length in 0.1f64..1_000.0,
    ) {
        let center = Point::new(center_x, center_y);
        let direction = Point::new(angle.cos() * length, angle.sin() * length);
        let perpendicular = Point::new(-direction.y, direction.x);
        let a = center.minus(direction);
        let b = center.translated(direction);
        let c = center.minus(perpendicular);
        let d = center.translated(perpendicular);

        let (intersection, t, u) = line_intersection(a, b, c, d).unwrap();
        let (swapped, swapped_u, swapped_t) = line_intersection(c, d, a, b).unwrap();

        prop_assert!(intersection.near(center, 1e-7));
        prop_assert!(swapped.near(intersection, 1e-7));
        prop_assert!((t - 0.5).abs() < 1e-7);
        prop_assert!((u - 0.5).abs() < 1e-7);
        prop_assert!((t - swapped_t).abs() < 1e-7);
        prop_assert!((u - swapped_u).abs() < 1e-7);
    }

    #[test]
    fn rectangle_area_and_triangulation_survive_translation_and_winding(
        x in -100.0f64..100.0,
        y in -100.0f64..100.0,
        width in 0.1f64..1_000.0,
        height in 0.1f64..1_000.0,
        dx in -10_000.0f64..10_000.0,
        dy in -10_000.0f64..10_000.0,
    ) {
        let rectangle = [
            Point::new(x, y),
            Point::new(x + width, y),
            Point::new(x + width, y + height),
            Point::new(x, y + height),
        ];
        let moved: Vec<Point> = rectangle
            .iter()
            .map(|point| point.translated(Point::new(dx, dy)))
            .collect();
        let reversed: Vec<Point> = moved.iter().copied().rev().collect();
        let expected = width * height;
        let tolerance = expected * 1e-8 + 1e-7;

        prop_assert!(polygon_is_simple(&moved));
        prop_assert!((polygon_area(&moved) - expected).abs() < tolerance);
        prop_assert!((polygon_area(&reversed) - expected).abs() < tolerance);
        prop_assert!((polygon_perimeter(&moved) - 2.0 * (width + height)).abs() < 1e-7);

        let triangles = triangulate_polygon(&reversed);
        prop_assert_eq!(triangles.len(), 2);
        let triangulated_area: f64 = triangles.iter().map(triangle_area).sum();
        prop_assert!((triangulated_area - expected).abs() < tolerance);
    }

    #[test]
    fn segment_projection_is_clamped_and_independent_of_direction(
        start_x in -1_000.0f64..1_000.0,
        start_y in -1_000.0f64..1_000.0,
        angle in -std::f64::consts::PI..std::f64::consts::PI,
        length in 0.01f64..1_000.0,
        point_x in -2_000.0f64..2_000.0,
        point_y in -2_000.0f64..2_000.0,
    ) {
        let start = Point::new(start_x, start_y);
        let end = start.translated(Point::new(angle.cos() * length, angle.sin() * length));
        let point = Point::new(point_x, point_y);

        let (nearest, t) = nearest_point_on_segment(point, start, end);
        let (nearest_reversed, reversed_t) = nearest_point_on_segment(point, end, start);

        prop_assert!((0.0..=1.0).contains(&t));
        prop_assert!(nearest.near(start.lerp(end, t), 1e-8));
        prop_assert!(nearest.near(nearest_reversed, 1e-7));
        prop_assert!((t + reversed_t - 1.0).abs() < 1e-7);
        prop_assert!((distance_to_segment(point, start, end)
            - distance_to_segment(point, end, start)).abs() < 1e-7);
    }

    #[test]
    fn stored_resolution_is_idempotent_and_bounded(value in -1_000_000.0f64..1_000_000.0) {
        let rounded = round_to_resolution(value);
        prop_assert_eq!(round_to_resolution(rounded), rounded);
        prop_assert!((rounded - value).abs() <= 0.5 / STEPS_PER_METRE + EPSILON);
        if rounded == 0.0 {
            prop_assert_eq!(rounded.to_bits(), 0.0f64.to_bits());
        }
    }

    #[test]
    fn simplification_removes_collinear_and_closing_vertices_without_changing_area(
        x in -100.0f64..100.0,
        y in -100.0f64..100.0,
        width in 0.1f64..100.0,
        height in 0.1f64..100.0,
    ) {
        let polygon = vec![
            Point::new(x, y),
            Point::new(x + width / 2.0, y),
            Point::new(x + width, y),
            Point::new(x + width, y + height),
            Point::new(x, y + height),
            Point::new(x, y),
        ];
        let simplified = simplify_polygon(&polygon);

        prop_assert_eq!(simplified.len(), 4);
        prop_assert!(polygon_is_simple(&simplified));
        prop_assert!((polygon_area(&simplified) - width * height).abs() < 1e-8);
    }
}
