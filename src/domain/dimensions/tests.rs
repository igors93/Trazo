use super::*;
use crate::domain::model::OpeningKind;

#[test]
fn associative_aligned_internal_and_angular_dimensions_follow_wall_changes() {
    let mut project = Project::default();
    let room_id = project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
        .unwrap();
    let wall_id = project.rooms[0].wall_ids[0];

    // Aligned dimension linked to wall endpoints.
    let aligned_id = project.add_wall_aligned_dimension(wall_id, -0.4).unwrap();
    let resolved = project
        .resolve_dimension(
            project
                .dimensions
                .iter()
                .find(|d| d.id == aligned_id)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(resolved.label(), "400");

    // Internal room dimensions discount wall thickness (0.12 m -> 388 cm and 288 cm).
    assert_eq!(project.generate_room_internal_dimensions(), 2);
    assert_eq!(project.generate_room_internal_dimensions(), 0); // idempotent
    let internal_labels: Vec<String> = project
        .dimensions
        .iter()
        .filter(|d| matches!(d.kind, DimensionKind::Internal { .. }))
        .filter_map(|d| project.resolve_dimension(d))
        .map(|r| r.label().to_owned())
        .collect();
    assert!(internal_labels.contains(&"388".to_owned()));
    assert!(internal_labels.contains(&"288".to_owned()));

    // Resizing the room automatically updates BOTH aligned and internal dimensions!
    project.resize_room_dimensions(room_id, 5.0, 3.5).unwrap();
    let updated = project
        .resolve_dimension(
            project
                .dimensions
                .iter()
                .find(|d| d.id == aligned_id)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(updated.label(), "500");

    // Editable text override.
    project
        .update_dimension_override(aligned_id, Some("500 (conf. obra)".to_owned()))
        .unwrap();
    let overridden = project
        .resolve_dimension(
            project
                .dimensions
                .iter()
                .find(|d| d.id == aligned_id)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(overridden.label(), "500 (conf. obra)");

    // Angular dimension: 90° corner.
    let ang_id = project
        .add_dimension(DimensionKind::Angular {
            vertex: DimensionAnchor::from_point(Point::new(0.0, 0.0)),
            ray_a: DimensionAnchor::from_point(Point::new(2.0, 0.0)),
            ray_b: DimensionAnchor::from_point(Point::new(0.0, 2.0)),
            radius: 0.8,
        })
        .unwrap();
    let ang = project
        .resolve_dimension(project.dimensions.iter().find(|d| d.id == ang_id).unwrap())
        .unwrap();
    assert_eq!(ang.label(), "90°");
    assert!(project.delete_dimension(ang_id));
}

#[test]
fn opening_chain_locates_multiple_openings_and_sums_to_wall_length() {
    let mut project = Project::default();
    let wall_id = project
        .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
        .unwrap();
    // Default door is 0.90 m centered at x = 1.50 m (jambs at 1.05 and 1.95).
    project
        .add_opening(wall_id, OpeningKind::Door, 1.5 / 6.0)
        .unwrap();
    // Window 1.20 m centered at x = 4.20 m (jambs at 3.60 and 4.80).
    project
        .add_opening(wall_id, OpeningKind::Window, 4.2 / 6.0)
        .unwrap();

    let summary = project
        .generate_opening_dimension_chains(Some(wall_id), 0.45)
        .unwrap();
    assert_eq!(summary.walls_processed, 1);
    assert_eq!(summary.dimensions_added, 5);

    // Second run is strictly idempotent.
    let second = project
        .generate_opening_dimension_chains(Some(wall_id), 0.45)
        .unwrap();
    assert_eq!(second.walls_processed, 1);
    assert_eq!(second.dimensions_added, 0);

    let resolved: Vec<_> = project
        .dimensions
        .iter()
        .filter_map(|d| project.resolve_dimension(d))
        .collect();
    let labels: Vec<&str> = resolved.iter().map(|r| r.label()).collect();
    assert_eq!(labels, vec!["105", "90", "165", "120", "120"]);

    // Every segment in the chain sits on the exact same horizontal offset line and sums to 6.00 m.
    let mut total_m = 0.0;
    let mut expected_y: Option<f64> = None;
    for r in &resolved {
        if let ResolvedDimension::Linear {
            dim_p1,
            dim_p2,
            value_m,
            ..
        } = r
        {
            total_m += value_m;
            assert!((dim_p1.y - dim_p2.y).abs() < 1e-9);
            if let Some(y) = expected_y {
                assert!((dim_p1.y - y).abs() < 1e-9);
            } else {
                expected_y = Some(dim_p1.y);
            }
        } else {
            panic!("expected linear dimension");
        }
    }
    assert!((total_m - 6.0).abs() < 1e-9);
}

#[test]
fn opening_chain_offsets_outward_from_room_interior_on_all_four_walls() {
    let mut project = Project::default();
    let _room_id = project
        .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
        .unwrap();
    let wall_ids = project.rooms[0].wall_ids.clone();
    assert_eq!(wall_ids.len(), 4);
    for wid in &wall_ids {
        project.add_opening(*wid, OpeningKind::Window, 0.5).unwrap();
    }

    let summary = project
        .generate_opening_dimension_chains(None, 0.50)
        .unwrap();
    assert_eq!(summary.walls_processed, 4);
    assert_eq!(summary.dimensions_added, 12); // 3 segments per wall × 4 walls

    for dim in &project.dimensions {
        let Some(ResolvedDimension::Linear { dim_p1, dim_p2, .. }) = project.resolve_dimension(dim)
        else {
            panic!("expected resolved linear dimension");
        };
        let mid = dim_p1.lerp(dim_p2, 0.5);
        // Every chain must lie strictly OUTSIDE the room [0, 5] × [0, 4].
        let strictly_outside = mid.x < -0.1 || mid.x > 5.1 || mid.y < -0.1 || mid.y > 4.1;
        assert!(
            strictly_outside,
            "chain midpoint ({:.2}, {:.2}) should be outside room [0..5]x[0..4]",
            mid.x, mid.y
        );
    }
}

#[test]
fn opening_chain_updates_associatively_when_opening_moves_or_resizes() {
    let mut project = Project::default();
    let wall_id = project
        .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
        .unwrap();
    let op_id = project
        .add_opening(wall_id, OpeningKind::Window, 0.5)
        .unwrap(); // 1.20 m centered at 2.00 m -> 140, 120, 140

    project
        .generate_opening_dimension_chains(Some(wall_id), 0.45)
        .unwrap();
    let labels_before: Vec<String> = project
        .dimensions
        .iter()
        .filter_map(|d| project.resolve_dimension(d))
        .map(|r| r.label().to_owned())
        .collect();
    assert_eq!(labels_before, vec!["140", "120", "140"]);

    // Move opening to x = 1.60 m (t = 0.40) and widen to 1.60 m -> jambs at 0.80 and 2.40 -> 80, 160, 160.
    let op = project.openings.iter_mut().find(|o| o.id == op_id).unwrap();
    op.t = 0.40;
    op.width = Some(1.60);

    let labels_after: Vec<String> = project
        .dimensions
        .iter()
        .filter_map(|d| project.resolve_dimension(d))
        .map(|r| r.label().to_owned())
        .collect();
    assert_eq!(labels_after, vec!["80", "160", "160"]);
}

#[test]
fn opening_chain_works_on_angled_wall_and_generate_site_dimensions_is_idempotent() {
    let mut project = Project::default();
    let _room_id = project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
        .unwrap();
    let north_wall = project.rooms[0].wall_ids[0];
    project
        .add_opening(north_wall, OpeningKind::Window, 0.5)
        .unwrap();

    // Angled 3-4-5 wall (length 5.00 m) with a 1.00 m door centered at midpoint.
    let angled = project
        .add_wall(Point::new(6.0, 0.0), Point::new(9.0, 4.0))
        .unwrap();
    let door_id = project.add_opening(angled, OpeningKind::Door, 0.5).unwrap();
    project
        .openings
        .iter_mut()
        .find(|o| o.id == door_id)
        .unwrap()
        .width = Some(1.0);

    let first = project.generate_site_dimensions(0.45);
    assert_eq!(first.internal_added, 2);
    assert_eq!(first.opening_chains.walls_processed, 2);
    assert_eq!(first.opening_chains.dimensions_added, 6);
    assert_eq!(first.total_added(), 8);

    // Repeating site generation adds nothing.
    let second = project.generate_site_dimensions(0.45);
    assert_eq!(second.total_added(), 0);

    // Angled wall chain lengths: 200, 100, 200 (sum = 500 cm).
    let angled_labels: Vec<String> = project
        .dimensions
        .iter()
        .filter(|d| match d.kind {
            DimensionKind::Aligned { a, b, .. } => {
                matches!(
                    a,
                    DimensionAnchor::WallStart { wall_id } if wall_id == angled
                ) || matches!(
                    b,
                    DimensionAnchor::WallEnd { wall_id } if wall_id == angled
                ) || matches!(
                    a,
                    DimensionAnchor::OpeningLeft { opening_id } if opening_id == door_id
                )
            }
            _ => false,
        })
        .filter_map(|d| project.resolve_dimension(d))
        .map(|r| r.label().to_owned())
        .collect();
    assert_eq!(angled_labels, vec!["200", "100", "200"]);
}
