use super::*;
use crate::domain::{conduits::PlanMark, fixtures::FixtureKind, geometry::Point, model::Project};

use crate::domain::model::OpeningKind;

#[test]
fn shared_beam_supports_are_memoized_and_deep_imports_are_bounded() {
    let mut project = Project::default();
    let from = project
        .add_fixture(FixtureKind::Column, Point::new(0.0, 0.0), 0.2, 0.2, 0)
        .unwrap();
    let to = project
        .add_fixture(FixtureKind::Column, Point::new(4.0, 0.0), 0.2, 0.2, 0)
        .unwrap();
    project.structure.beams.push(Beam {
        id: 100,
        from,
        to,
        from_support: None,
        to_support: None,
        width_m: 0.2,
        height_m: 0.4,
    });
    for id in 101..=240 {
        project.structure.beams.push(Beam {
            id,
            from,
            to,
            from_support: Some(BeamSupport::Beam {
                beam_id: id - 1,
                t: 0.0,
            }),
            to_support: Some(BeamSupport::Beam {
                beam_id: id - 1,
                t: 1.0,
            }),
            width_m: 0.2,
            height_m: 0.4,
        });
    }
    // Both ends reuse the same host at every level. Without memoization this
    // expands into 2^40 recursive resolutions despite there being only 41 beams.
    let beam = project.structure.beams[40];
    assert_eq!(
        project.beam_endpoints(&beam),
        Some((Point::new(0.0, 0.0), Point::new(4.0, 0.0)))
    );
    assert_eq!(
        project.beam_support_depth(&beam, &mut std::collections::HashSet::new()),
        40
    );
    assert!(
        project
            .beam_endpoints(&project.structure.beams[140])
            .is_none()
    );
    assert!(project.prune_structure() > 0);
}

fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
        .unwrap();
    project
}

#[test]
fn a_continuous_beam_matches_the_textbook() {
    // Two equal spans with a uniform load: -wL²/8 over the middle
    // support, 3wL/8 at the ends and 10wL/8 in the middle.
    let (w, l) = (10.0, 4.0);
    let moments = continuous_beam(&[(l, w, 1.0), (l, w, 1.0)]);
    assert!(moments[0].0.abs() < 1e-9);
    assert!((moments[0].1 + w * l * l / 8.0).abs() < 1e-9);
    assert!((moments[1].0 - moments[0].1).abs() < 1e-9);
    // One simply supported span: 5wL⁴/384EI at mid-span.
    let d = span_deflection(l, w, 0.0, 0.0, 1000.0);
    assert!(
        (d - 5.0 * w * l.powi(4) / 384.0 / 1000.0).abs() < 1e-4,
        "{d}"
    );
    assert_eq!(SlabKind::Ribbed.minimum_thickness(3.2), 0.12);
    assert_eq!(SlabKind::Solid.minimum_thickness(4.4), 0.11);
    assert_eq!(natural("V2", "V10"), Ordering::Less);
}

#[test]
fn an_overhang_transfers_moment_and_load_to_both_supports() {
    // Uniform 10 kN/m load over a 1 m left overhang and a 4 m span.
    // The overhang fixes M_A = -q a²/2; equilibrium then determines
    // R_B = q(L²-a²)/(2L) and R_A = q(L+a)-R_B.
    let (q, a, l) = (10.0, 1.0, 4.0);
    let ends =
        continuous_beam_with_end_moments(&[(l, q, 1.0)], &[Vec::new()], (-q * a * a / 2.0, 0.0));
    assert!((ends[0].0 + 5.0).abs() < 1e-9);
    assert!(ends[0].1.abs() < 1e-9);
    let r_a = q * a + q * l / 2.0 + (ends[0].1 - ends[0].0) / l;
    let r_b = q * (l + a) - r_a;
    assert!((r_a - 31.25).abs() < 1e-9);
    assert!((r_b - 18.75).abs() < 1e-9);
}

#[test]
fn two_overhangs_keep_a_multispan_beam_in_equilibrium() {
    let q = 10.0;
    let spans = [(4.0, q, 1.0), (4.0, q, 1.0)];
    let ends = continuous_beam_with_end_moments(
        &spans,
        &[Vec::new(), Vec::new()],
        (-q * 1.0_f64.powi(2) / 2.0, -q * 2.0_f64.powi(2) / 2.0),
    );
    assert!((ends[0].0 + 5.0).abs() < 1e-9);
    assert!((ends[1].1 + 20.0).abs() < 1e-9);
    assert!((ends[0].1 - ends[1].0).abs() < 1e-9);
    let mut reactions = [q, 0.0, 2.0 * q];
    for (i, ((l, w, _), (left, right))) in spans.iter().zip(&ends).enumerate() {
        let v_left = w * l / 2.0 + (right - left) / l;
        reactions[i] += v_left;
        reactions[i + 1] += w * l - v_left;
    }
    assert!((reactions.iter().sum::<f64>() - 110.0).abs() < 1e-9);
    assert!((reactions[0] * 1.0 + reactions[1] * 5.0 + reactions[2] * 9.0 - 605.0).abs() < 1e-9);
}

#[test]
fn generating_lays_columns_beams_slabs_and_footings() {
    let mut project = house();
    let wall = project
        .walls
        .iter()
        .find(|w| (w.start.y - w.end.y).abs() < 1e-6 && w.start.y.abs() < 1e-6)
        .unwrap()
        .id;
    project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
    let report = project.generate_structure();
    // Six junctions of the two rooms.
    assert_eq!(report.columns, 6, "{report:?}");
    assert_eq!(report.beams, 7);
    assert_eq!(report.slabs, 2);
    assert_eq!(report.footings, 6);
    // Again, nothing new.
    assert_eq!(project.generate_structure().total(), 0);
    let labels = project.structure_labels();
    let first = project
        .structural_columns()
        .min_by(|a, b| reading(a.center, b.center))
        .unwrap();
    assert_eq!(labels[&first.id], "P1");
    // Two horizontal continuous beams (V1, V2) and three vertical ones.
    let lines = project.beam_lines();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].label, "V1");
    assert_eq!(lines[0].spans.len(), 2);

    let structure = project.structure_report();
    assert!(
        structure.slabs.iter().all(SlabResult::ok),
        "{:?}",
        structure.slabs
    );
    assert!(structure.columns.iter().all(|c| c.load_kn > 0.0));
    assert!(structure.footings.iter().all(FootingResult::ok));
    // The slab loads all reach the columns: the reactions add up to the
    // slabs, the beams and nothing else (one floor, no walls above).
    let slabs: f64 = project
        .structure
        .slabs
        .iter()
        .map(|slab| {
            let room = project.rooms.iter().find(|r| r.id == slab.room_id).unwrap();
            let result = structure.slabs.iter().find(|r| r.id == slab.id).unwrap();
            result.load_kn_m2 * project.room_area(room).unwrap()
        })
        .sum();
    let beams: f64 = structure
        .beams
        .iter()
        .flat_map(|l| &l.spans)
        .map(|s| CONCRETE_KN_M3 * s.width_m * s.height_m * s.length_m)
        .sum();
    let reactions: f64 = structure.beams.iter().flat_map(|l| &l.reactions).sum();
    assert!(
        ((slabs + beams) - reactions).abs() < 0.01 * reactions,
        "{slabs} + {beams} vs {reactions}"
    );
    assert!(structure.warnings.is_empty(), "{:?}", structure.warnings);
    // The plan shows the beams and every label.
    let marks = project.structure_marks();
    for label in ["P1", "V1", "L1", "S1"] {
        assert!(
                marks
                    .iter()
                    .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with(&format!("{label} ")))),
                "{label}"
            );
    }
    let svg = beam_diagrams_svg(&project);
    assert!(svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok());
    assert!(svg.contains(">V1<"));
}

#[test]
fn concentrated_load_flows_from_slab_to_beams_columns_and_footings() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    project.generate_structure();
    let room = project.rooms[0].id;
    project.set_slab(room, SlabKind::Solid, Some(0.12)).unwrap();
    let before = project.structure_report();
    let before_columns: f64 = before.columns.iter().map(|column| column.load_kn).sum();
    let before_moment = before
        .beams
        .iter()
        .flat_map(|line| &line.spans)
        .map(|span| span.moment_span)
        .fold(0.0, f64::max);

    let tank = project
        .add_fixture(FixtureKind::WaterTank, Point::new(2.0, 2.0), 1.20, 1.20, 0)
        .unwrap();
    let load_kn = project
        .fixtures
        .iter()
        .find(|fixture| fixture.id == tank)
        .and_then(|fixture| fixture.structural_load_kn)
        .unwrap();
    let after = project.structure_report();
    let after_columns: f64 = after.columns.iter().map(|column| column.load_kn).sum();
    let after_moment = after
        .beams
        .iter()
        .flat_map(|line| &line.spans)
        .map(|span| span.moment_span)
        .fold(0.0, f64::max);

    assert!((after_columns - before_columns - load_kn).abs() < 1e-6);
    assert!(after_moment > before_moment);
    assert_eq!(after.concentrated_loads.len(), 1);
    let result = &after.concentrated_loads[0];
    assert_eq!(result.label, "CC1");
    assert!(result.supported && result.slab.is_some());
    assert!(result.local_moment_knm_m > 0.0);
    assert!(result.punching_ratio.is_some());
    assert_eq!(after.slabs[0].point_load_kn, load_kn);
    assert!(
        crate::domain::verdict::structure_verdicts(&project, &after)
            .iter()
            .any(|verdict| verdict.element.starts_with("Carga CC1"))
    );
    assert!(
        project
            .structure_marks()
            .iter()
            .any(|mark| matches!(mark, PlanMark::Text { text, .. } if text.starts_with("CC1 ")))
    );

    project
        .update_fixture(tank, Some(Point::new(8.0, 8.0)), None, None)
        .unwrap();
    let unsupported = project.structure_report();
    assert!(!unsupported.concentrated_loads[0].supported);
    assert!(
        unsupported
            .warnings
            .iter()
            .any(|warning| warning.contains("sem laje ou viga"))
    );
}

#[test]
fn a_load_directly_on_a_beam_skips_the_slab_local_check() {
    let mut project = house();
    project.generate_structure();
    let beam = project.structure.beams[0];
    let (a, b) = project.beam_endpoints(&beam).unwrap();
    project
        .add_fixture(FixtureKind::HeavyEquipment, a.midpoint(b), 0.40, 0.40, 0)
        .unwrap();
    let report = project.structure_report();
    let load = &report.concentrated_loads[0];
    assert!(load.supported);
    assert!(load.slab.is_none());
    assert!(load.support.contains("direto na viga"));
    assert_eq!(load.punching_ratio, None);
}

#[test]
fn undersized_elements_are_flagged() {
    let mut project = house();
    project.generate_structure();
    let slab = project.structure.slabs[0];
    project
        .set_slab(slab.room_id, SlabKind::Solid, Some(0.06))
        .unwrap();
    let beam = project.structure.beams[0].id;
    project.update_beam(beam, None, Some(0.15)).unwrap();
    project.structure.settings.floors = 6;
    let report = project.structure_report();
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("é pouco para o vão"))
    );
    assert!(report.warnings.iter().any(|w| w.contains("pede altura")));
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("seção") || w.contains("m² é pouco"))
    );
    // Bad sections are refused.
    assert!(project.update_beam(beam, Some(0.02), None).is_err());
    assert!(
        project
            .set_slab(slab.room_id, SlabKind::Solid, Some(2.0))
            .is_err()
    );
}

#[test]
fn each_floor_keeps_its_beams_slabs_and_conduits() {
    // Regression: switching floors left these behind, and the pruning
    // on the next edit deleted them.
    let mut project = house();
    project.generate_structure();
    let (w, d) = FixtureKind::Panel.default_size();
    let panel = project
        .add_fixture(FixtureKind::Panel, Point::new(0.5, 0.2), w, d, 0)
        .unwrap();
    let (w, d) = FixtureKind::CeilingLight.default_size();
    let light = project
        .add_fixture(FixtureKind::CeilingLight, Point::new(2.0, 2.0), w, d, 0)
        .unwrap();
    project
        .add_conduit(panel, light, crate::domain::conduits::ConduitRun::Ceiling)
        .unwrap();
    let (beams, slabs) = (
        project.structure.beams.clone(),
        project.structure.slabs.clone(),
    );
    let conduits = project.conduits.clone();
    let ground = project.ensure_default_floor();
    project.add_floor("Superior", true, false);
    assert!(project.structure.beams.is_empty() && project.conduits.is_empty());
    // An edit upstairs prunes, and must not touch the floor below.
    project.prune_conduits();
    project.prune_structure();
    project.switch_floor(ground).unwrap();
    assert_eq!(project.structure.beams, beams);
    assert_eq!(project.structure.slabs, slabs);
    assert_eq!(project.conduits, conduits);
}

#[test]
fn columns_and_footings_on_a_boundary_wall_stand_flush_inside_the_lot() {
    // B23: a room against the left lot line (x = 0).
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    project
        .add_room(Point::new(0.0, 2.0), Point::new(5.0, 6.0), "Gourmet")
        .unwrap();
    // Placing a column on that wall moves it inside instead of refusing it.
    let (at, turns, angle) =
        project.structural_placement_fitted(FixtureKind::Column, Point::new(0.05, 4.0), 0.30, 0.14);
    assert!(
        project
            .add_fixture(FixtureKind::Column, at, 0.30, 0.14, turns)
            .is_some()
    );
    assert!((at.x - 0.07).abs() < 0.002, "{at:?}");
    let _ = angle;
    // Generating lays every corner column, boundary ones flush, and
    // reports nothing skipped; the beams still find them.
    let mut generated = Project::default();
    generated.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    generated
        .add_room(Point::new(0.0, 2.0), Point::new(4.0, 6.0), "Gourmet")
        .unwrap();
    let report = generated.generate_structure();
    assert_eq!(report.columns, 4, "{report:?}");
    assert_eq!(report.skipped, 0);
    assert_eq!(report.beams, 4);
    let boundary: Vec<_> = generated
        .structural_columns()
        .filter(|c| c.center.x < 1.0)
        .collect();
    assert_eq!(boundary.len(), 2);
    for column in &boundary {
        let (low, _) = column.bounds();
        assert!(low.x >= -1e-9, "inside the lot: {column:?}");
    }
    // Their footings sit inside the lot too, off centre from the column.
    for footing in generated
        .fixtures
        .iter()
        .filter(|f| f.kind == FixtureKind::Footing)
    {
        let (low, _) = footing.bounds();
        assert!(low.x >= -1e-9, "{footing:?}");
    }
    let report = generated.structure_report();
    assert!(
        report.footings.iter().any(|f| f.eccentricity_m > 0.1),
        "boundary footings are eccentric"
    );
}

#[test]
fn beams_go_with_their_columns() {
    let mut project = house();
    project.generate_structure();
    let column = project.structural_columns().next().unwrap().id;
    let attached = project.beams_of(&[column]).len();
    assert!(attached >= 2);
    let before = project.structure.beams.len();
    project.delete_fixture(column);
    assert!(project.beams_of(&[column]).is_empty());
    assert_eq!(project.structure.beams.len(), before - attached);
    // Removed any other way, the pruning on each edit catches it.
    let other = project.structural_columns().next().unwrap().id;
    let left = project.beams_of(&[other]).len();
    project.fixtures.retain(|f| f.id != other);
    assert_eq!(project.prune_structure(), left);
    // A column placed near a junction snaps onto it.
    let (at, _) = project.structural_placement(Point::new(4.2, 0.1));
    assert!(at.distance(Point::new(4.0, 0.0)) < 1e-6, "{at:?}");
}

#[test]
fn branson_deflection_uses_uncracked_inertia_below_mr_and_cracked_as_above_mr() {
    let settings = StructureSettings::default(); // fck = 25 MPa
    let modulus = settings.modulus_kn_m2();
    let (b, h): (f64, f64) = (0.15, 0.40);
    let ic = b * h.powi(3) / 12.0;

    // Light load below cracking moment Mr: beam stays uncracked (I_eq == I_c),
    // unlike the old fixed 0.5 * I_c approximation.
    let uncracked = effective_beam_stiffness(b, h, settings.fck_mpa, modulus, 5.0, 4.0);
    assert!((uncracked.i_eq_m4 - ic).abs() < 1e-12);
    assert!(uncracked.m_r_knm > 10.0);
    assert!(uncracked.as_cm2 >= 0.0015 * b * h * 10_000.0 - 1e-6);

    // Heavy service moment well above Mr: I_eq drops smoothly toward the
    // cracked transformed inertia I_II computed from the pre-sized steel.
    let cracked = effective_beam_stiffness(b, h, settings.fck_mpa, modulus, 60.0, 48.0);
    assert!(cracked.i_eq_m4 < ic);
    assert!(cracked.i_eq_m4 >= cracked.i_cracked_m4 - 1e-12);
    assert!(cracked.as_cm2 > uncracked.as_cm2);
}

#[test]
fn footings_check_eccentric_pressure_overturning_and_punching_shear() {
    // Concentric well-proportioned footing passes all checks.
    let concentric = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.0, 0.0, 25.0);
    assert!(!concentric.outside_middle_third);
    assert!(concentric.overturning_fs.is_infinite());
    assert!(concentric.max_pressure_kpa <= 110.0);
    assert!(concentric.punching_ratio < 1.0);

    // Large eccentricity pushes load outside the middle third, raises peak
    // soil pressure and drops overturning FS below 1.5.
    let eccentric = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.36, 0.0, 25.0);
    assert!(eccentric.outside_middle_third);
    assert!(eccentric.overturning_fs < 1.5);
    assert!(eccentric.max_pressure_kpa > 300.0);

    // Heavy load on a shallow footing triggers punching shear failure.
    let punched = check_footing(600.0, 1.8, 1.8, 0.25, 0.30, 0.14, 0.0, 0.0, 25.0);
    assert!(punched.punching_ratio > 1.0);

    // Integration check on a project with an off-centre footing.
    let mut project = house();
    project.generate_structure();
    let footing_id = project
        .fixtures
        .iter()
        .find(|f| f.kind == FixtureKind::Footing)
        .unwrap()
        .id;
    // Shift footing so the column sits near its edge (eccentric).
    if let Some(f) = project.fixtures.iter_mut().find(|f| f.id == footing_id) {
        f.width = 1.0;
        f.depth = 1.0;
        f.center = f.center.translated(Point::new(0.40, 0.0));
    }
    let report = project.structure_report();
    let res = report.footings.iter().find(|f| f.id == footing_id).unwrap();
    assert!(!res.ok());
    assert!(res.overturning_fs < 1.5);
    assert!(report.warnings.iter().any(|w| w.contains("tombamento")));
}

#[test]
fn l_shaped_slab_uses_panel_spans_and_yield_line_load_distribution() {
    // An L-shaped room with 3 m wings inside a 6 × 6 m bounding box must
    // report lx = 3.0 m (wing width), not 6.0 m (bounding box).
    let l_outline = vec![
        Point::new(0.0, 0.0),
        Point::new(6.0, 0.0),
        Point::new(6.0, 3.0),
        Point::new(3.0, 3.0),
        Point::new(3.0, 6.0),
        Point::new(0.0, 6.0),
    ];
    let panels = decompose_slab_panels(&l_outline);
    assert_eq!(panels.len(), 2);
    let lx = panels.iter().map(|(x, _)| *x).fold(0.0_f64, f64::max);
    assert!((lx - 3.0).abs() < 1e-6, "got lx = {lx}");

    // 3 × 6 m rectangular slab with 45° yield lines:
    // short edges take lx²/4 = 2.25 / 18 = 12.5% each,
    // long edges take (lx/2)*(ly - lx/2) = 6.75 / 18 = 37.5% each.
    let rect = vec![
        Point::new(0.0, 0.0),
        Point::new(6.0, 0.0),
        Point::new(6.0, 3.0),
        Point::new(0.0, 3.0),
    ];
    let supported = vec![
        (1, Point::new(0.0, 0.0), Point::new(6.0, 0.0)), // long
        (2, Point::new(6.0, 0.0), Point::new(6.0, 3.0)), // short
        (3, Point::new(6.0, 3.0), Point::new(0.0, 3.0)), // long
        (4, Point::new(0.0, 3.0), Point::new(0.0, 0.0)), // short
    ];
    let shares = yield_line_shares(&rect, &supported);
    assert!((shares[&1] - 0.375).abs() < 1e-6, "long: {}", shares[&1]);
    assert!((shares[&3] - 0.375).abs() < 1e-6, "long: {}", shares[&3]);
    assert!((shares[&2] - 0.125).abs() < 1e-6, "short: {}", shares[&2]);
    assert!((shares[&4] - 0.125).abs() < 1e-6, "short: {}", shares[&4]);
    let sum: f64 = shares.values().sum();
    assert!((sum - 1.0).abs() < 1e-9);
}

#[test]
fn angled_columns_and_beam_on_beam_supports_transfer_loads_accurately() {
    // 1. Angled wall at 45°: generated columns inherit the 45° angle instead
    // of snapping to quarter turns.
    let mut angled = Project::default();
    angled.set_terrain(Point::new(-2.0, -2.0), Point::new(10.0, 10.0));
    angled
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 3.0))
        .unwrap();
    angled.generate_structure();
    let col = angled.structural_columns().next().unwrap();
    assert!(
        (col.rotation_rad() - std::f64::consts::FRAC_PI_4).abs() < 1e-4,
        "expected 45° column, got {} rad",
        col.rotation_rad()
    );
    // Local +X axis of the column runs along the 45° wall.
    let tip = col.to_world(Point::new(0.15, 0.0));
    let delta = tip.minus(col.center);
    assert!((delta.x - delta.y).abs() < 1e-6);

    // 2. Secondary beam resting on two primary girders transfers its exact
    // self-weight to the girders and down to the four corner columns.
    let mut frame = Project::default();
    frame.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
    let c1 = frame
        .add_fixture(FixtureKind::Column, Point::new(0.0, 0.0), 0.30, 0.14, 0)
        .unwrap();
    let c2 = frame
        .add_fixture(FixtureKind::Column, Point::new(4.0, 0.0), 0.30, 0.14, 0)
        .unwrap();
    let c3 = frame
        .add_fixture(FixtureKind::Column, Point::new(0.0, 4.0), 0.30, 0.14, 0)
        .unwrap();
    let c4 = frame
        .add_fixture(FixtureKind::Column, Point::new(4.0, 4.0), 0.30, 0.14, 0)
        .unwrap();
    let g1 = frame.add_beam(c1, c2, 0.15, 0.40).unwrap();
    let g2 = frame.add_beam(c3, c4, 0.15, 0.40).unwrap();

    let before = frame.structure_report();
    let g1_moment_before = before
        .beams
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.beam == g1)
        .unwrap()
        .moment_span;

    // Add a secondary beam spanning from midpoint of g1 to midpoint of g2.
    let sec = frame
        .add_beam_on_supports(
            BeamSupport::Beam {
                beam_id: g1,
                t: 0.5,
            },
            BeamSupport::Beam {
                beam_id: g2,
                t: 0.5,
            },
            0.15,
            0.35,
        )
        .unwrap();
    let after = frame.structure_report();
    let g1_span_after = after
        .beams
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.beam == g1)
        .unwrap();
    let sec_weight = CONCRETE_KN_M3 * 0.15 * 0.35 * 4.0;
    // Point load at mid-span increases primary girder's mid-span moment by P*L/4.
    let expected_extra_moment = (sec_weight / 2.0) * 4.0 / 4.0;
    assert!(
        ((g1_span_after.moment_span - g1_moment_before) - expected_extra_moment).abs() < 0.05,
        "extra moment {} vs expected {}",
        g1_span_after.moment_span - g1_moment_before,
        expected_extra_moment
    );
    // Deleting a column removes its girder AND the secondary beam resting on it.
    let affected = frame.beams_of(&[c1]);
    assert!(affected.contains(&g1) && affected.contains(&sec));
}

#[test]
fn upper_floor_loads_flow_down_to_aligned_columns_and_flag_misaligned_transfers() {
    let mut project = house();
    project.generate_structure();
    let single_floor = project.structure_report();
    let single_total_col_load: f64 = single_floor.columns.iter().map(|c| c.load_kn).sum();

    // Create a real second storey with its own structure aligned over the ground floor.
    let ground = project.ensure_default_floor();
    let _upper = project.add_floor("Superior", true, true);
    project.generate_structure();
    // Switch back to ground floor and verify upper loads accumulate naturally
    // even with `settings.floors == 1`.
    project.switch_floor(ground).unwrap();
    let two_storey = project.structure_report();
    assert!(two_storey.misaligned_columns.is_empty());
    assert!(two_storey.columns.iter().all(|c| c.upper_load_kn > 0.0));
    let two_storey_total: f64 = two_storey.columns.iter().map(|c| c.load_kn).sum();
    assert!(
        two_storey_total > single_total_col_load * 1.8,
        "expected real upper floor load to add up: {two_storey_total} vs {single_total_col_load}"
    );

    // Now add a misaligned column on the upper floor sitting at the midpoint
    // of a ground-floor beam (transfer girder) and another over open space.
    let upper_id = project
        .floors
        .levels
        .iter()
        .find(|l| l.id != ground)
        .unwrap()
        .id;
    project.switch_floor(upper_id).unwrap();
    let on_beam = project
        .add_fixture(FixtureKind::Column, Point::new(2.0, 0.0), 0.30, 0.14, 0)
        .unwrap();
    let floating = project
        .add_fixture(FixtureKind::Column, Point::new(2.0, 2.0), 0.30, 0.14, 0)
        .unwrap();
    let _ = (on_beam, floating);
    project.switch_floor(ground).unwrap();

    let with_transfers = project.structure_report();
    assert_eq!(with_transfers.misaligned_columns.len(), 2);
    assert!(
        with_transfers
            .misaligned_columns
            .iter()
            .any(|m| m.transfer_beam.is_some())
    );
    assert!(
        with_transfers
            .misaligned_columns
            .iter()
            .any(|m| m.transfer_beam.is_none())
    );
    assert!(
        with_transfers
            .warnings
            .iter()
            .any(|w| w.contains("viga de transição"))
    );
    assert!(
        with_transfers
            .warnings
            .iter()
            .any(|w| w.contains("sem prumada abaixo"))
    );
}

#[test]
fn eccentric_footing_pressure_is_continuous_and_monotonic_across_middle_third() {
    let inside = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.16, 0.0, 25.0);
    let outside = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.17, 0.0, 25.0);
    assert!(!inside.outside_middle_third);
    assert!(outside.outside_middle_third);
    // Moving the load further off-centre must ALWAYS increase peak edge pressure.
    assert!(
        outside.max_pressure_kpa > inside.max_pressure_kpa,
        "outside {} must exceed inside {}",
        outside.max_pressure_kpa,
        inside.max_pressure_kpa
    );
    // Standard rigid footing (1.10×1.00×0.30 m, 200 kN) passes NBR 6118 §22.6.2 punching.
    let rigid = check_footing(200.0, 1.10, 1.00, 0.30, 0.30, 0.14, 0.0, 0.0, 25.0);
    assert!(
        rigid.punching_ratio < 1.0,
        "rigid footing should pass punching, got {}",
        rigid.punching_ratio
    );
}

#[test]
fn multi_storey_multiplier_does_not_double_count_walls_quadratically() {
    let mut p2 = house();
    p2.generate_structure();
    p2.structure.settings.floors = 2;
    let r2 = p2.structure_report();

    let mut p3 = house();
    p3.generate_structure();
    p3.structure.settings.floors = 3;
    let r3 = p3.structure_report();

    // Any single beam carries at most 1 storey of walls above it, so dead load
    // on a typical beam is identical for 2-storey and 3-storey buildings.
    let d2: f64 = r2
        .beams
        .iter()
        .flat_map(|b| &b.spans)
        .map(|s| s.dead_kn_m)
        .sum();
    let d3: f64 = r3
        .beams
        .iter()
        .flat_map(|b| &b.spans)
        .map(|s| s.dead_kn_m)
        .sum();
    assert!(
        (d3 - d2).abs() < 1e-6,
        "3-storey beam dead load {d3} should equal 2-storey {d2}"
    );

    // Total column load grows linearly (~1.5× from 2 to 3 floors, never quadratically).
    let n2: f64 = r2.columns.iter().map(|c| c.load_kn).sum();
    let n3: f64 = r3.columns.iter().map(|c| c.load_kn).sum();
    assert!(n3 > n2 * 1.4 && n3 < n2 * 1.75, "n2={n2}, n3={n3}");
}

#[test]
fn pile_cap_on_boundary_wall_sizes_broca_and_resolves_wind_uplift_verdict() {
    use crate::domain::{
        fixtures::PileCapSpec,
        soil::{SoilKind, SoilSurvey, SptReading},
        verdict::{Light, structure_verdicts},
    };

    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    project.wall_height = 2.50;
    let w = project
        .add_wall(Point::new(0.075, 0.075), Point::new(9.925, 0.075))
        .unwrap();
    let _ = project.set_wall_height(w, Some(2.50), Some(2.50));
    project.structure.soil_survey = Some(SoilSurvey {
        name: "SP-01".to_owned(),
        footing_depth_m: 1.0,
        water_table_m: Some(8.0),
        readings: vec![
            SptReading {
                depth_m: 1.0,
                n: 5,
                soil: SoilKind::ClayeySilt,
            },
            SptReading {
                depth_m: 2.0,
                n: 7,
                soil: SoilKind::ClayeySilt,
            },
            SptReading {
                depth_m: 3.0,
                n: 9,
                soil: SoilKind::Clay,
            },
            SptReading {
                depth_m: 4.0,
                n: 11,
                soil: SoilKind::Clay,
            },
            SptReading {
                depth_m: 5.0,
                n: 13,
                soil: SoilKind::SandySilt,
            },
        ],
        extra_boreholes: Vec::new(),
    });

    let mut cols = Vec::new();
    for x in [0.10, 3.35, 6.65, 9.90] {
        let cid = project
            .add_fixture(FixtureKind::Column, Point::new(x, 0.10), 0.20, 0.20, 0)
            .unwrap();
        cols.push(cid);
        let (cap_center, turns, _) = project.structural_placement_fitted(
            FixtureKind::PileCap,
            Point::new(x, 0.10),
            0.45,
            0.45,
        );
        let bid = project
            .add_fixture(FixtureKind::PileCap, cap_center, 0.45, 0.45, turns)
            .unwrap();
        project
            .set_pile_cap_spec(
                bid,
                PileCapSpec {
                    diameter_m: 0.25,
                    depth_m: 4.0,
                    count: 1,
                    anchor_depth_m: 2.0,
                },
            )
            .unwrap();
    }
    for pair in cols.windows(2) {
        project.add_beam(pair[0], pair[1], 0.15, 0.30).unwrap();
    }
    let _ = project.generate_wall_foundations();

    let report = project.structure_report();
    assert_eq!(report.pile_caps.len(), 4);
    assert!(report.footings.is_empty());
    for cap in &report.pile_caps {
        assert!(cap.uses_survey);
        assert!(cap.above_water_table);
        assert!(
            cap.allowable_kn > 30.0,
            "expected >30 kN, got {}",
            cap.allowable_kn
        );
        assert!(
            cap.uplift_capacity_kn > 15.0,
            "expected shaft uplift capacity >15 kN"
        );
        assert!(cap.ok(), "pile cap {} should pass: {cap:?}", cap.label);
    }

    // Traffic lights: zero red verdicts on PileCaps and zero red frame uplift verdicts!
    let verdicts = structure_verdicts(&project, &report);
    for v in &verdicts {
        if v.element.starts_with("Bloco sobre broca") {
            assert_eq!(
                v.light(),
                Light::Green,
                "expected green on {}: {v:?}",
                v.element
            );
        }
        if v.element.starts_with("Pórtico") {
            assert_ne!(
                v.light(),
                Light::Red,
                "anchored pile caps should resolve frame wind uplift red verdict: {v:?}"
            );
        }
    }

    // Shallow water table (2.0 m) must flag the 4.0 m manual broca as RED!
    if let Some(survey) = &mut project.structure.soil_survey {
        survey.water_table_m = Some(2.0);
    }
    let wet_report = project.structure_report();
    assert!(
        wet_report
            .pile_caps
            .iter()
            .all(|c| !c.above_water_table && !c.ok())
    );
    let wet_verdicts = structure_verdicts(&project, &wet_report);
    assert!(
        wet_verdicts
            .iter()
            .filter(|v| v.element.starts_with("Bloco sobre broca"))
            .all(|v| v.light() == Light::Red)
    );
}
