use super::materials::KMOD;
use super::sizing::{Piece, beam_positions, size_piece};
use super::*;
use crate::domain::{fixtures::FixtureKind, geometry::Point, model::Project};

#[test]
fn timber_species_have_stable_keys_and_conservative_classes() {
    assert_eq!(
        TimberSpecies::parse("Eucalipto-citriodora"),
        Some(TimberSpecies::EucalyptusCitriodora)
    );
    assert_eq!(TimberSpecies::parse("Itaúba"), Some(TimberSpecies::Itauba));
    assert_eq!(TimberSpecies::PinusElliottii.class(), TimberClass::C20);
    assert_eq!(TimberSpecies::Cumaru.class(), TimberClass::C30);
    assert_eq!(TimberSpecies::ClassC60.class(), TimberClass::C60);
}

#[test]
fn roman_and_american_tiles_keep_their_catalogue_assumptions() {
    assert_eq!(TileKind::parse("telha_romana"), Some(TileKind::Roman));
    assert_eq!(TileKind::parse("americana"), Some(TileKind::American));
    assert_eq!(TileKind::Roman.min_slope_percent(), 35.0);
    assert_eq!(TileKind::Roman.gauge_m(), Some(0.33));
    assert_eq!(TileKind::Roman.per_m2(), Some(16.0));
    assert_eq!(TileKind::American.min_slope_percent(), 30.0);
    assert_eq!(TileKind::American.gauge_m(), Some(0.36));
    assert_eq!(TileKind::American.per_m2(), Some(12.5));
    assert!(TileKind::Roman.weight_kn_m2() > TileKind::American.weight_kn_m2());
}

fn gourmet_roof() -> Roof {
    // 5.00 × 14.75 m, high on the west at 4.30 m, low on the east at 2.60 m.
    Roof::new(
        Point::new(0.0, 0.0),
        Point::new(5.0, 14.75),
        RoofSide::West,
        4.30,
        2.60,
    )
}

#[test]
fn the_slope_and_heights_follow_the_high_side() {
    let roof = gourmet_roof();
    assert!((roof.slope() - 0.34).abs() < 1e-9);
    assert!((roof.height_at(Point::new(0.0, 3.0)) - 4.30).abs() < 1e-9);
    assert!((roof.height_at(Point::new(5.0, 3.0)) - 2.60).abs() < 1e-9);
    assert!((roof.height_at(Point::new(2.5, 3.0)) - 3.45).abs() < 1e-9);
    let mut flat = roof;
    flat.low_m = 4.30;
    assert!(flat.check().is_err());
}

#[test]
fn beams_line_up_with_the_columns_and_fill_wide_gaps() {
    let positions = beam_positions(14.75, 3.0, &[3.0, 6.0, 9.0, 12.0]);
    assert_eq!(positions, vec![0.0, 3.0, 6.0, 9.0, 12.0, 14.75]);
    let filled = beam_positions(10.0, 3.0, &[]);
    assert_eq!(filled.len(), 5); // 4 gaps of 2.5 m
}

#[test]
fn extra_main_beams_and_a_fixed_section_are_honoured() {
    let mut project = Project::default();
    for along in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
        for x in [0.07, 4.93] {
            project
                .add_fixture(FixtureKind::Column, Point::new(x, along), 0.14, 0.30, 0)
                .unwrap();
        }
    }
    let mut roof = gourmet_roof();
    roof.beam_section_cm = Some((10.0, 30.0));
    roof.extra_beam_positions_m = vec![1.5, 4.5, 7.5, 10.5];
    let id = project.add_roof(roof).unwrap();
    let result = project.analyse_roof(&project.structure.roofs[0]);
    let beams = result
        .elements
        .iter()
        .find(|element| element.element == RoofElement::Beams)
        .unwrap();
    assert_eq!(beams.section_cm, (10.0, 30.0));
    assert_eq!(beams.count, 10);
    assert_eq!(result.reactions.len(), 20);
    assert_eq!(result.id, id);
}

#[test]
fn a_beam_matches_the_hand_calculation() {
    // C30, 1.4 (g + q) = 1.4 × 3 kN/m over 4 m horizontal (flat piece).
    let piece = Piece {
        element: RoofElement::Purlins,
        g: 2.0,
        q: 1.0,
        span: 4.0,
        spacing: 1.0,
        inclined: false,
        normal: 1.0,
        count: 1,
        each_m: 4.0,
        section_cm: None,
    };
    let ((b, h), ok, bending, _, _) = size_piece(&piece, TimberClass::C30, 1.0);
    assert!(ok);
    let (b_m, h_m) = (b / 100.0, h / 100.0);
    let own = 800.0 * 9.81 / 1000.0 * b_m * h_m;
    let md = 1.4 * (3.0 + own) * 16.0 / 8.0;
    let fc0d = KMOD * 30.0 / 1.4 * 1000.0;
    assert!((bending - md / (b_m * h_m * h_m / 6.0) / fc0d).abs() < 1e-9);
}

#[test]
fn the_gourmet_roof_is_framed_and_bears_on_the_columns() {
    let mut project = Project::default();
    let mut columns = Vec::new();
    for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
        columns.push(
            project
                .add_fixture(FixtureKind::Column, Point::new(0.07, y), 0.14, 0.30, 0)
                .unwrap(),
        );
    }
    let id = project.add_roof(gourmet_roof()).unwrap();
    let roof = project.structure.roofs[0].clone();
    let result = project.analyse_roof(&roof);
    assert_eq!(result.label, "T1");
    assert!(result.slope_ok());
    assert_eq!(result.elements.len(), 4);
    assert!(
        result.elements.iter().all(|e| e.ok),
        "{:#?}",
        result.elements
    );
    let beam = result
        .elements
        .iter()
        .find(|e| e.element == RoofElement::Beams)
        .unwrap();
    assert!(beam.deflection_m > 0.0);
    assert!(beam.deflection_m < beam.deflection_limit_m);
    assert!((beam.deflection_m / beam.deflection_limit_m - beam.deflection_ratio).abs() < 1e-9);
    // Six beams (one per column), both ends.
    assert_eq!(result.reactions.len(), 12);
    let total: f64 = result.reactions.iter().map(|r| r.load_kn).sum();
    let expected = (result.dead_kn_m2 + result.live_kn_m2) * 5.0 * 14.75;
    // Beam own weight on top of the area loads.
    assert!(
        total > expected && total < expected * 1.15,
        "{total} vs {expected}"
    );
    assert!(result.tiles.unwrap() > 1000.0);
    // The structure report bears the high ends on the columns.
    let report = project.structure_report();
    let roof_result = &report.roofs[0];
    assert_eq!(
        roof_result
            .reactions
            .iter()
            .filter(|r| r.high && r.support.is_some())
            .count(),
        6
    );
    // The low edge has nothing under it yet (posts, #146).
    assert_eq!(roof_result.unsupported(), 6);
    let column = report.columns.iter().find(|c| c.id == columns[1]).unwrap();
    let high = roof_result
        .reactions
        .iter()
        .find(|r| r.high && (r.point.y - 3.0).abs() < 1e-6)
        .unwrap();
    assert!(
        column.load_kn >= high.load_kn,
        "{} vs {}",
        column.load_kn,
        high.load_kn
    );
    // Walls under the roof take its slope.
    project
        .add_wall(Point::new(0.0, 14.75), Point::new(5.0, 14.75))
        .unwrap();
    assert_eq!(project.fit_walls_to_roof(id).unwrap(), 1);
    let gable = project.walls.last().unwrap();
    assert_eq!((gable.height_m, gable.height_end_m), (Some(4.3), Some(2.6)));
}

#[test]
fn gable_and_hip_roofs_size_howe_trusses_wall_plates_hip_rafters_overhangs_and_uplift() {
    let mut project = Project::default();
    // 6.0 × 8.0 m gable roof with 0.60 m overhang, ridge at 3.80 m, eaves at 2.80 m (slope = 1.0/3.0 = 33.3%).
    let mut gable = Roof::new(
        Point::new(0.0, 0.0),
        Point::new(6.0, 8.0),
        RoofSide::West,
        3.80,
        2.80,
    );
    gable.style = RoofStyle::Gable;
    gable.overhang_m = 0.60;
    let gid = project.add_roof(gable).unwrap();
    let r_gable = project.analyse_roof(&project.structure.roofs[0]);

    // Exact gable heights: low at both eaves, high at center ridge x = 3.0.
    let g_ref = &project.structure.roofs[0];
    assert!((g_ref.height_at(Point::new(0.0, 4.0)) - 2.80).abs() < 1e-9);
    assert!((g_ref.height_at(Point::new(3.0, 4.0)) - 3.80).abs() < 1e-9);
    assert!((g_ref.height_at(Point::new(6.0, 4.0)) - 2.80).abs() < 1e-9);
    assert!((r_gable.slope_percent - 33.333333).abs() < 0.01);
    assert!(r_gable.wall_plate.is_some());
    assert!(r_gable.overhang_bending_ratio > 0.0 && r_gable.overhang_bending_ratio <= 1.0);
    assert!(r_gable.uplift.overhang_suction_kpa > r_gable.uplift.suction_kpa);

    // Exact 4-panel Howe truss statics verification (Method of Joints):
    // N_top = (3/8 * W_d) / sin(θ), N_bot = (3/8 * W_d) / tan(θ), N_diag = (1/8 * W_d) / sin(θ), N_kp = W_d / 4.
    let truss = r_gable
        .truss
        .as_ref()
        .expect("gable roof must size Howe trusses");
    assert!(truss.ok);
    let theta = (1.0_f64 / 3.0_f64).atan();
    let wd = truss.design_total_load_kn;
    assert!((truss.members[0].axial_design_kn.abs() - (0.375 * wd / theta.sin())).abs() < 1e-6);
    assert!((truss.members[1].axial_design_kn - (0.375 * wd / theta.tan())).abs() < 1e-6);
    assert!((truss.members[2].axial_design_kn.abs() - (0.125 * wd / theta.sin())).abs() < 1e-6);
    assert!((truss.members[3].axial_design_kn - (0.25 * wd)).abs() < 1e-6);

    // 4-water hip roof over 6.0 × 8.0 m with 0.50 m overhang.
    let mut hip = Roof::new(
        Point::new(10.0, 0.0),
        Point::new(16.0, 8.0),
        RoofSide::West,
        3.80,
        2.80,
    );
    hip.style = RoofStyle::Hip;
    hip.overhang_m = 0.50;
    let _hid = project.add_roof(hip).unwrap();
    let hip_ref = &project.structure.roofs[1];
    let r_hip = project.analyse_roof(hip_ref);
    assert!(r_hip.truss.is_some());
    let hips = r_hip
        .hip_rafters
        .as_ref()
        .expect("hip roof must size 4 hip rafters");
    assert_eq!(hips.count, 4);
    assert!(hips.ok);
    // Exact hip surface area identity: (W + 2*oh)*(L + 2*oh)/cos(alpha).
    let expected_area = (6.0 + 1.0) * (8.0 + 1.0) / hip_ref.cos();
    assert!((r_hip.surface_m2 - expected_area).abs() < 1e-6);
    // Hip perimeter edges at low_m, central ridge at high_m.
    assert!((hip_ref.height_at(Point::new(10.0, 4.0)) - 2.80).abs() < 1e-9);
    assert!((hip_ref.height_at(Point::new(13.0, 0.0)) - 2.80).abs() < 1e-9);
    assert!((hip_ref.height_at(Point::new(13.0, 4.0)) - 3.80).abs() < 1e-9);

    // Plan marks draw ridge & hip rafters cleanly.
    let marks = project.roof_marks(&[r_gable, r_hip]);
    assert!(marks.iter().any(|m| matches!(m, crate::domain::conduits::PlanMark::Text { text, .. } if text.contains("duas águas"))));
    assert!(marks.iter().any(|m| matches!(m, crate::domain::conduits::PlanMark::Text { text, .. } if text.contains("quatro águas"))));
    assert!(gid != 0);
}
