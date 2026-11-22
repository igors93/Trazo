use std::collections::HashMap;

use super::sizing::size_balance_beam;
use crate::domain::conduits::PlanMark;
use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::structure::{ColumnPosition, ColumnResult};

fn sample_column(id: u64, label: &str, load: f64) -> ColumnResult {
    ColumnResult {
        id,
        label: label.to_owned(),
        width_m: 0.2,
        depth_m: 0.2,
        position: ColumnPosition::Edge,
        load_kn: load,
        upper_load_kn: 0.0,
        area_needed_cm2: 0.0,
        area_cm2: 400.0,
        height_m: 2.8,
        slenderness: 48.5,
    }
}

#[test]
fn the_lever_follows_the_textbook_statics() {
    // P1 = 100 kN, l = 4 m, e = 0.30 m: R1 = 100·4/3.7 = 108.1 kN,
    // ΔP = 8.1 kN, M = 30 kN·m, FS = 150 / 8.108 = 18.5.
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    let a = project
        .add_fixture(FixtureKind::Column, Point::new(0.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    let b = project
        .add_fixture(FixtureKind::Column, Point::new(4.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(0.40, 5.0), 0.80, 0.80, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(4.10, 5.0), 1.00, 1.00, 0)
        .unwrap();
    project.add_balance_beam(a, b).unwrap();
    let none = HashMap::new();
    let effects = project.balance_effects(
        &[sample_column(a, "P1", 100.0), sample_column(b, "P2", 150.0)],
        &none,
    );
    let r = &effects.results[0];
    assert!((r.eccentricity_m - 0.30).abs() < 1e-9);
    assert!((r.reaction_kn - 100.0 * 4.0 / 3.7).abs() < 1e-9);
    assert!((r.relief_kn - 100.0 * 0.3 / 3.7).abs() < 1e-9);
    assert!((r.moment_knm - 30.0).abs() < 1e-9);
    assert!(!r.uplift());
    assert!(r.overturning_ok());
    assert!((r.pair_overturning_fs - 150.0 / r.relief_kn).abs() < 1e-9);
    assert!(r.pair_soil_ok());
    assert!(r.height_m.is_some());
    assert!(r.stirrups.starts_with('Ø'));
    assert!((effects.relief[&b] - r.relief_kn / 2.0).abs() < 1e-9);
    // A light interior column is lifted.
    let lifted = project.balance_effects(
        &[sample_column(a, "P1", 100.0), sample_column(b, "P2", 5.0)],
        &none,
    );
    assert!(lifted.results[0].uplift());
    assert!(!lifted.results[0].overturning_ok());
    assert!(lifted.results[0].strip_alternative.is_some());
}

#[test]
fn pair_overturning_requires_fs_at_least_1_5_and_proposes_strip_alternative() {
    // P1 = 100 kN, l = 4 m, e = 0.30 m → ΔP = 8.108 kN.
    // With N2 = 10 kN: no uplift (10 > 8.1), but FS = 10 / 8.108 = 1.23 < 1.5!
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    let a = project
        .add_fixture(FixtureKind::Column, Point::new(0.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    let b = project
        .add_fixture(FixtureKind::Column, Point::new(4.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(0.40, 5.0), 0.80, 0.80, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(4.10, 5.0), 0.80, 0.80, 0)
        .unwrap();
    project.add_balance_beam(a, b).unwrap();
    let none = HashMap::new();
    let marginal = project.balance_effects(
        &[sample_column(a, "P1", 100.0), sample_column(b, "P2", 10.0)],
        &none,
    );
    let r = &marginal.results[0];
    assert!(!r.uplift(), "N2 > ΔP so no uplift");
    assert!(
        !r.overturning_ok(),
        "FS = {} < 1.5 must fail NBR 6122 overturning",
        r.pair_overturning_fs
    );
    let strip = r
        .strip_alternative
        .as_ref()
        .expect("strip-footing alternative proposed when FS < 1.5");
    assert!(strip.viable);
    assert!(strip.width_m >= 0.40 && strip.pressure_kpa <= strip.soil_kpa);
}

#[test]
fn pair_soil_pressure_checks_both_boundary_and_anchor_footings() {
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    project.structure.settings.soil_kpa = 150.0;
    let a = project
        .add_fixture(FixtureKind::Column, Point::new(0.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    let b = project
        .add_fixture(FixtureKind::Column, Point::new(4.10, 5.0), 0.20, 0.20, 0)
        .unwrap();
    // Small 60×60 cm boundary footing (0.36 m²) under 100 kN → σ1 > 300 kPa > 150 kPa!
    project
        .add_fixture(FixtureKind::Footing, Point::new(0.30, 5.0), 0.60, 0.60, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(4.10, 5.0), 1.00, 1.00, 0)
        .unwrap();
    project.add_balance_beam(a, b).unwrap();
    let none = HashMap::new();
    let effects = project.balance_effects(
        &[sample_column(a, "P1", 100.0), sample_column(b, "P2", 120.0)],
        &none,
    );
    let r = &effects.results[0];
    assert!(!r.pair_soil_ok());
    assert!(r.boundary_pressure_kpa.unwrap() > 150.0);
    assert!(r.anchor_pressure_kpa.unwrap() < 150.0);
    assert!(r.pair_soil_ratio() > 1.5);
}

#[test]
fn boundary_footings_are_centred_by_balance_beams_to_interior_columns() {
    // A 4 × 4 m room against the left lot line, as in the gourmet area.
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    project
        .add_room(Point::new(0.0, 2.0), Point::new(4.0, 6.0), "Gourmet")
        .unwrap();
    project.generate_structure();
    let before = project.structure_report();
    let eccentric: Vec<_> = before
        .footings
        .iter()
        .filter(|f| f.eccentricity_m > 0.05)
        .collect();
    assert_eq!(eccentric.len(), 2, "the two boundary footings");

    let (added, alone) = project.generate_balance_beams();
    assert_eq!((added, alone), (2, 0));
    assert_eq!(project.generate_balance_beams(), (0, 0), "already there");
    let after = project.structure_report();
    assert_eq!(after.balance_beams.len(), 2);
    for beam in &after.balance_beams {
        assert!(!beam.uplift());
        assert!(beam.overturning_ok());
        assert!(beam.pair_soil_ok());
        assert!(beam.height_m.is_some());
        let footing = after
            .footings
            .iter()
            .find(|f| Some(f.id) == beam.footing_id)
            .unwrap();
        // Centred under R1: no overturning, the reaction as its load.
        assert!(footing.overturning_fs.is_infinite());
        assert!((footing.load_kn - beam.reaction_kn).abs() < 1e-9);
        assert!(beam.reaction_kn > beam.column_load_kn);
    }
    // The interior footings carry less (half the relief).
    let interior_before: f64 = before
        .footings
        .iter()
        .filter(|f| f.eccentricity_m <= 0.05)
        .map(|f| f.load_kn)
        .sum();
    let interior_after: f64 = after
        .footings
        .iter()
        .filter(|f| {
            !after
                .balance_beams
                .iter()
                .any(|b| b.footing_id == Some(f.id))
        })
        .map(|f| f.load_kn)
        .sum();
    let relief: f64 = after.balance_beams.iter().map(|b| b.relief_kn / 2.0).sum();
    assert!((interior_before - interior_after - relief).abs() < 1e-6);
    // They are drawn, kept across floors and go with their columns.
    let marks = project.structure_marks();
    assert!(
        marks
            .iter()
            .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with("VA1")))
    );
    let ground = project.ensure_default_floor();
    project.add_floor("Superior", true, false);
    assert!(project.structure.balance_beams.is_empty());
    project.switch_floor(ground).unwrap();
    assert_eq!(project.structure.balance_beams.len(), 2);
    let column = project.structure.balance_beams[0].column;
    project.delete_fixture(column);
    assert_eq!(project.structure.balance_beams.len(), 1);
}

#[test]
fn the_section_grows_with_the_moment() {
    let (small, ..) = size_balance_beam(0.20, 1.4 * 20.0, 1.4 * 10.0, 25.0);
    let (large, ..) = size_balance_beam(0.20, 1.4 * 120.0, 1.4 * 40.0, 25.0);
    assert!(small.unwrap() < large.unwrap());
    let (none, flexure, ..) = size_balance_beam(0.20, 1.4 * 2000.0, 1.4 * 50.0, 25.0);
    assert!(none.is_none() && flexure > 1.0);
}

#[test]
fn shared_anchor_column_checks_cumulative_uplift_across_multiple_balance_beams() {
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(8.0, 6.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(8.0, 6.0), "Galpão")
        .unwrap();
    // Place two opposite boundary columns at x = 0.07 and x = 7.93, both anchored
    // to a single interior column at x = 4.0.
    let c_left = project
        .add_fixture(FixtureKind::Column, Point::new(0.07, 3.0), 0.14, 0.30, 0)
        .unwrap();
    let c_mid = project
        .add_fixture(FixtureKind::Column, Point::new(4.0, 3.0), 0.14, 0.30, 0)
        .unwrap();
    let c_right = project
        .add_fixture(FixtureKind::Column, Point::new(7.93, 3.0), 0.14, 0.30, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(0.50, 3.0), 1.0, 1.0, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(4.0, 3.0), 1.0, 1.0, 0)
        .unwrap();
    project
        .add_fixture(FixtureKind::Footing, Point::new(7.50, 3.0), 1.0, 1.0, 0)
        .unwrap();
    project.add_balance_beam(c_left, c_mid).unwrap();
    project.add_balance_beam(c_right, c_mid).unwrap();

    // Manually synthesize columns so N2 = 60 kN while each boundary column has
    // large P1 = 300 kN -> relief per beam ≈ 36.2 kN, ΣΔP ≈ 72.4 kN > 60 kN (uplift!).
    let cols = vec![
        crate::domain::structure::ColumnResult {
            id: c_left,
            label: "P1".into(),
            width_m: 0.14,
            depth_m: 0.30,
            position: crate::domain::structure::ColumnPosition::Edge,
            load_kn: 300.0,
            upper_load_kn: 0.0,
            area_needed_cm2: 400.0,
            area_cm2: 420.0,
            height_m: 2.8,
            slenderness: 69.2,
        },
        crate::domain::structure::ColumnResult {
            id: c_mid,
            label: "P2".into(),
            width_m: 0.14,
            depth_m: 0.30,
            position: crate::domain::structure::ColumnPosition::Interior,
            load_kn: 60.0,
            upper_load_kn: 0.0,
            area_needed_cm2: 360.0,
            area_cm2: 420.0,
            height_m: 2.8,
            slenderness: 69.2,
        },
        crate::domain::structure::ColumnResult {
            id: c_right,
            label: "P3".into(),
            width_m: 0.14,
            depth_m: 0.30,
            position: crate::domain::structure::ColumnPosition::Edge,
            load_kn: 300.0,
            upper_load_kn: 0.0,
            area_needed_cm2: 400.0,
            area_cm2: 420.0,
            height_m: 2.8,
            slenderness: 69.2,
        },
    ];
    let effects = project.balance_effects(&cols, &std::collections::HashMap::new());
    assert_eq!(effects.results.len(), 2);
    // Individually N2 / relief ≈ 60 / 36.2 = 1.66 >= 1.5, but cumulatively
    // N2 / (2 * 36.2) = 0.83 < 1.5 -> MUST fail overturning!
    for res in &effects.results {
        assert!(
            res.pair_overturning_fs < 1.0,
            "cumulative uplift must give FS < 1.0, got {}",
            res.pair_overturning_fs
        );
        assert!(!res.overturning_ok());
    }
}
