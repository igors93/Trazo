use std::f64::consts::PI;

use super::bond::{
    basic_anchorage_m, bond_strength_mpa, required_anchorage_m, required_anchorage_with_cover_m,
};
use super::sizing::{
    beam_design_stirrups, beam_stirrups, choose_mesh, flexural_demand, flexural_design_demand,
};
use super::*;
use crate::domain::{
    limits,
    model::Project,
    structure::{StructureReport, effective_beam_stiffness},
};

use crate::domain::geometry::Point;

fn frame_project() -> Project {
    use crate::domain::fixtures::FixtureKind;
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(11.0, 6.0));
    let mut columns = Vec::new();
    for x in [0.0, 5.5, 8.5] {
        columns.push(
            project
                .add_fixture(FixtureKind::Column, Point::new(x, 0.0), 0.30, 0.35, 0)
                .unwrap(),
        );
    }
    project
        .add_beam(columns[0], columns[1], 0.20, 0.45)
        .unwrap();
    project
        .add_beam(columns[1], columns[2], 0.20, 0.45)
        .unwrap();
    let equipment = project
        .add_fixture(
            FixtureKind::HeavyEquipment,
            Point::new(1.9, 0.0),
            1.2,
            1.2,
            0,
        )
        .unwrap();
    // Keep demands above minimum steel, so changing the links must affect steel.
    project
        .fixtures
        .iter_mut()
        .find(|f| f.id == equipment)
        .unwrap()
        .structural_load_kn = Some(60.0);
    project
}

#[test]
fn design_force_inputs_do_not_receive_a_second_load_factor() {
    assert_eq!(
        flexural_design_demand(0.20, 0.45, 0.04, 70.0, 25.0),
        flexural_demand(0.20, 0.45, 0.04, 50.0, 25.0),
    );
    assert_eq!(
        beam_design_stirrups(4.0, 0.25, 0.60, 849.8, 40.0),
        beam_stirrups(4.0, 0.25, 0.60, 607.0, 40.0),
    );
    // Independent section equilibrium for Md = 70 kN.m, not 1.4 * 70.
    let demand = flexural_design_demand(0.20, 0.45, 0.04, 70.0, 25.0);
    let steel_force = demand.required_cm2 / 10_000.0 * (500.0 / 1.15 * 1000.0);
    let x = steel_force / (0.85 * (25.0 / 1.4 * 1000.0) * 0.20 * 0.80);
    let capacity = steel_force * (0.41 - 0.40 * x);
    assert!((capacity - 70.0).abs() < 1e-8);
}

#[test]
fn beam_steel_tracks_its_own_elu_envelope_when_frame_links_and_wind_change() {
    let mut project = frame_project();
    let mut demands = Vec::new();
    let mut steel_demands = Vec::new();
    for wind in [25.0, 55.0] {
        project.structure.wind.v0_ms = wind;
        for (rigid, fixed) in [(true, true), (true, false), (false, true)] {
            project.structure.settings.frame_rigid_joints = rigid;
            project.structure.settings.frame_fixed_bases = fixed;
            let structure = project.cached_structure_report();
            assert!(
                structure.frame_failures.is_empty(),
                "{:?}",
                structure.frame_failures
            );
            let reinforcement = project.cached_reinforcement_report();
            assert_eq!(reinforcement.beams.len(), 2);
            let frame = &structure.frames[0];
            assert_eq!(frame.beams.len(), 2);
            let positive = frame
                .beams
                .iter()
                .map(|b| b.ultimate.max_positive_moment_knm)
                .fold(0.0, f64::max);
            let negative = frame
                .beams
                .iter()
                .map(|b| b.ultimate.min_negative_moment_knm)
                .fold(0.0, f64::min);
            let shear = frame
                .beams
                .iter()
                .map(|b| b.ultimate.max_shear_kn)
                .fold(0.0, f64::max);
            assert!((positive - frame.envelope.elu_max_beam_pos_knm).abs() < 1e-9);
            assert!((negative - frame.envelope.elu_min_beam_neg_knm).abs() < 1e-9);
            assert!((shear - frame.envelope.elu_max_beam_shear_kn).abs() < 1e-9);
            assert_ne!(frame.beams[0].ultimate, frame.beams[1].ultimate);

            let mut case_demands = Vec::new();
            let mut case_steel = Vec::new();
            for beam in &reinforcement.beams {
                let envelope = &frame
                    .beams
                    .iter()
                    .find(|b| b.beam_id == beam.beam_id)
                    .unwrap()
                    .ultimate;
                assert_eq!(beam.frame_envelope.as_ref(), Some(envelope));
                for code in [
                    &envelope.positive_combination,
                    &envelope.negative_combination,
                    &envelope.shear_combination,
                ] {
                    assert!(
                        frame
                            .combinations
                            .iter()
                            .any(|c| c.code == code && c.category.is_ultimate())
                    );
                }
                assert_eq!(
                    beam.stirrups,
                    beam_design_stirrups(
                        beam.length_m,
                        beam.width_m,
                        beam.height_m,
                        envelope.max_shear_kn,
                        project.structure.settings.fck_mpa
                    )
                );
                assert!(matches!(&beam.top, BeamTopBars::Continuous(_)));
                let top = beam.top.left();
                assert!(top.length_each_m >= beam.length_m + 2.0 * beam.top_anchorage_m - 1e-9);
                assert!(beam.bottom.required_cm2 >= beam.service_minimum_cm2);
                // Check resistance from independent force equilibrium. Above
                // minimum steel, it must match Md, without another gamma factor.
                let fck = project.structure.settings.fck_mpa;
                let minimum = 0.0015 * beam.width_m * beam.height_m * 10_000.0;
                for (bars, md, lower_bound) in [
                    (
                        &beam.bottom,
                        envelope.max_positive_moment_knm,
                        minimum.max(beam.service_minimum_cm2),
                    ),
                    (top, -envelope.min_negative_moment_knm, minimum),
                ] {
                    let force = bars.required_cm2 / 10_000.0 * (500.0 / 1.15 * 1000.0);
                    let x = force / (0.85 * (fck / 1.4 * 1000.0) * beam.width_m * 0.80);
                    let resistance = force * (beam.height_m - 0.04 - 0.40 * x);
                    assert!(resistance + 1e-7 >= md, "{resistance} < {md}");
                    if bars.required_cm2 > lower_bound + 1e-6 {
                        assert!((resistance - md).abs() < 1e-7, "{resistance} != {md}");
                    }
                }
                let rows: Vec<_> = reinforcement
                    .schedule
                    .iter()
                    .filter(|r| r.element == beam.label)
                    .collect();
                assert_eq!(rows.len(), 3); // bottom + one continuous top run + stirrups
                assert!((rows.iter().map(|r| r.mass_kg).sum::<f64>() - beam.mass_kg).abs() < 1e-9);
                case_demands
                    .push(envelope.max_positive_moment_knm - envelope.min_negative_moment_knm);
                case_steel.push(beam.bottom.required_cm2 + top.required_cm2);
            }
            demands.push(case_demands);
            steel_demands.push(case_steel);

            // Legacy forces must no longer change frame beam reinforcement.
            let mut changed = (*structure).clone();
            for span in changed.beams.iter_mut().flat_map(|l| &mut l.spans) {
                span.moment_span = 1_000_000.0;
                span.moment_left = -1_000_000.0;
                span.moment_right = -1_000_000.0;
                span.shear = 1_000_000.0;
            }
            assert_eq!(
                reinforcement.beams,
                build_reinforcement_report(&project, &changed).beams
            );
        }
    }
    assert!(demands.windows(2).any(|pair| {
        pair[0]
            .iter()
            .zip(&pair[1])
            .any(|(a, b)| (a - b).abs() > 1e-4)
    }));
    assert!(steel_demands.windows(2).any(|pair| {
        pair[0]
            .iter()
            .zip(&pair[1])
            .any(|(a, b)| (a - b).abs() > 1e-4)
    }));
}

#[test]
fn frame_beam_design_keeps_elu_demand_and_service_stiffness_minimum_distinct() {
    let project = frame_project();
    let mut structure = project.structure_report();
    let frame_beam = &mut structure.frames[0].beams[0];
    let id = frame_beam.beam_id;
    frame_beam.ultimate.max_positive_moment_knm = 110.0;
    frame_beam.ultimate.min_negative_moment_knm = -50.0;
    frame_beam.ultimate.max_shear_kn = 180.0;
    let report = build_reinforcement_report(&project, &structure);
    let beam = report.beams.iter().find(|b| b.beam_id == id).unwrap();
    let fck = project.structure.settings.fck_mpa;
    assert_eq!(
        beam.bottom.required_cm2,
        flexural_design_demand(beam.width_m, beam.height_m, 0.04, 110.0, fck).required_cm2
    );
    assert_eq!(
        beam.top.left().required_cm2,
        flexural_design_demand(beam.width_m, beam.height_m, 0.04, 50.0, fck).required_cm2
    );

    let span = structure
        .beams
        .iter_mut()
        .flat_map(|l| &mut l.spans)
        .find(|s| s.beam == id)
        .unwrap();
    span.branson_as_cm2 = 8.0;
    let report = build_reinforcement_report(&project, &structure);
    let with_service = report.beams.iter().find(|b| b.beam_id == id).unwrap();
    assert_eq!(with_service.service_minimum_cm2, 8.0);
    assert_eq!(with_service.bottom.required_cm2, 8.0);
    assert!(with_service.bottom.provided_cm2 >= 8.0);
    assert_eq!(with_service.frame_envelope, beam.frame_envelope);
    assert_eq!(with_service.top, beam.top);
}

#[test]
fn failed_or_missing_frame_analysis_cannot_silently_issue_legacy_beam_steel() {
    let mut project = frame_project();
    project.structure.settings.frame_rigid_joints = false;
    project.structure.settings.frame_fixed_bases = false;
    let structure = project.structure_report();
    let reinforcement = build_reinforcement_report(&project, &structure);
    assert!(!structure.frame_failures.is_empty());
    assert!(reinforcement.is_empty());
    assert!(reinforcement.schedule.is_empty());
    assert!(
        reinforcement
            .warnings
            .iter()
            .any(|w| w.contains("análise do pórtico falhou"))
    );

    project.structure.settings.frame_fixed_bases = true;
    let mut structure = project.structure_report();
    structure.frames.clear();
    let reinforcement = build_reinforcement_report(&project, &structure);
    assert!(reinforcement.beams.is_empty());
    assert!(
        reinforcement
            .warnings
            .iter()
            .any(|w| w.contains("falta a envoltória ELU"))
    );
}

#[test]
fn secondary_beam_without_columns_retains_the_characteristic_sizing_path() {
    use crate::domain::{fixtures::FixtureKind, structure::BeamSupport};
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(7.0, 7.0));
    let mut columns = Vec::new();
    for (x, y) in [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (4.0, 4.0)] {
        columns.push(
            project
                .add_fixture(FixtureKind::Column, Point::new(x, y), 0.30, 0.35, 0)
                .unwrap(),
        );
    }
    let a = project
        .add_beam(columns[0], columns[1], 0.20, 0.45)
        .unwrap();
    let b = project
        .add_beam(columns[2], columns[3], 0.20, 0.45)
        .unwrap();
    let id = project
        .add_beam_on_supports(
            BeamSupport::Beam { beam_id: a, t: 0.5 },
            BeamSupport::Beam { beam_id: b, t: 0.5 },
            0.15,
            0.35,
        )
        .unwrap();
    let structure = project.structure_report();
    let report = build_reinforcement_report(&project, &structure);
    let secondary = report.beams.iter().find(|b| b.beam_id == id).unwrap();
    assert!(secondary.frame_envelope.is_none());
    assert!(matches!(&secondary.top, BeamTopBars::Supports { .. }));
    assert!(
        report
            .beams
            .iter()
            .filter(|b| b.beam_id != id)
            .all(|b| b.frame_envelope.is_some())
    );
}

#[test]
fn bar_geometry_and_mass_match_the_physical_section() {
    assert!((bar_area_cm2(10.0) - PI / 4.0).abs() < 1e-5);
    assert!((bar_mass_kg_m(10.0) - 0.6165).abs() < 0.002);
    assert!((bar_mass_kg_m(12.5) - 0.963).abs() < 0.003);
}

#[test]
fn flexural_steel_matches_the_existing_branson_input() {
    let fck: f64 = 25.0;
    let modulus = 0.85 * 5600.0 * fck.sqrt() * 1000.0;
    let old = effective_beam_stiffness(0.15, 0.40, fck, modulus, 45.0, 30.0);
    let demand = flexural_demand(0.15, 0.40, 0.04, 45.0, fck);
    assert!(demand.domain_ok);
    assert!((demand.required_cm2 - old.as_cm2).abs() / old.as_cm2 < 0.01);
}

#[test]
fn generated_house_has_reinforcement_and_a_balanced_schedule() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.5), "Sala")
        .unwrap();
    project.generate_structure();
    let structure = project.structure_report();
    let report = build_reinforcement_report(&project, &structure);
    assert!(!report.beams.is_empty());
    assert!(!report.columns.is_empty());
    assert!(!report.slabs.is_empty());
    assert!(!report.footings.is_empty());
    assert!(!report.schedule.is_empty());
    assert!(report.total_mass_kg() > 0.0);
    assert!(
        report
            .beams
            .iter()
            .all(|beam| beam.bottom.provided_cm2 + 1e-9 >= beam.bottom.required_cm2)
    );
    assert!(
        report
            .columns
            .iter()
            .all(|column| column.longitudinal.provided_cm2 + 1e-9
                >= column.longitudinal.required_cm2)
    );
    let sum: f64 = report.schedule.iter().map(|row| row.mass_kg).sum();
    assert!((sum - report.total_mass_kg()).abs() < 1e-9);
    let csv = steel_schedule_csv(&report);
    assert!(csv.contains("Forma"));
    assert!(csv.contains("Diâmetro (mm)"));
    assert!(csv.contains("Massa total teórica"));
    assert!(csv.contains("Resumo por bitola (CA-50)"));
}

#[test]
fn nbr_6118_bond_anchorage_and_lap_lengths_match_hand_calculation() {
    // C25, CA-50 Ø10 mm:
    // fctm = 0.3 * 25^(2/3) = 2.56496 MPa
    // fctd = 0.7 * fctm / 1.4 = 1.28248 MPa
    // fbd (good, η1=2.25, η2=1.0) = 2.88559 MPa
    // fbd (poor, η1=2.25, η2=0.7) = 2.01991 MPa
    // fyd = 500 / 1.15 = 434.7826 MPa
    // lb (good) = (0.010 / 4) * (434.7826 / 2.88559) = 0.37668 m
    // lb (poor) = 0.37668 / 0.7 = 0.53812 m
    let fbd_good = bond_strength_mpa(25.0, BondZone::Good);
    let fbd_poor = bond_strength_mpa(25.0, BondZone::Poor);
    assert!((fbd_good - 2.8856).abs() < 0.002);
    assert!((fbd_poor - 2.0199).abs() < 0.002);
    assert!((fbd_poor / fbd_good - 0.7).abs() < 1e-9);

    let lb_good = basic_anchorage_m(10.0, 25.0, BondZone::Good);
    let lb_poor = basic_anchorage_m(10.0, 25.0, BondZone::Poor);
    assert!((lb_good - 0.3767).abs() < 0.002);
    assert!((lb_poor - 0.5381).abs() < 0.002);

    // Hooked anchorage with As,calc == As,ef uses α1 = 0.7
    let lb_nec_hook = required_anchorage_m(10.0, 25.0, BondZone::Good, true, 2.0, 2.0);
    assert!((lb_nec_hook - 0.7 * lb_good).abs() < 1e-6);

    // Straight lap splice with α0t = 1.4 and As,calc == As,ef
    let as_col = 4.0 * bar_area_cm2(10.0);
    let l0 = lap_splice_m(10.0, 25.0, BondZone::Good, 1.4, as_col, as_col);
    assert!((l0 - 1.4 * lb_good).abs() < 1e-6);
}

#[test]
fn diameter_summary_reconciles_with_schedule_and_commercial_bars() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.8), "Sala")
        .unwrap();
    project.generate_structure();
    let structure = project.structure_report();
    let report = build_reinforcement_report(&project, &structure);

    assert!(!report.by_diameter.is_empty());
    let sum_mass: f64 = report.by_diameter.iter().map(|r| r.mass_kg).sum();
    assert!((sum_mass - report.total_mass_kg()).abs() < 1e-9);
    let sum_waste: f64 = report
        .by_diameter
        .iter()
        .map(|r| r.mass_with_waste_kg)
        .sum();
    assert!((sum_waste - report.total_mass_with_waste_kg()).abs() < 1e-9);
    for row in &report.by_diameter {
        let expected_bars =
            ((row.total_length_m * STEEL_WASTE_FACTOR) / COMMERCIAL_BAR_LENGTH_M).ceil() as u32;
        assert_eq!(row.bars_12m, expected_bars);
    }
    assert!(report.total_bars_12m() > 0);
    assert!(report.beams.iter().all(|b| b.bottom_anchorage_m >= 0.10));
    assert!(report.beams.iter().all(|b| b.top_anchorage_m >= 0.10));
    assert!(report.columns.iter().all(|c| c.lap_length_m >= 0.40));
}

#[test]
fn reinforcement_detail_svg_is_valid_svg_with_all_sections() {
    let mut project = Project::default();
    project.name = "Casa Teste #110".to_owned();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.8), "Sala")
        .unwrap();
    project.generate_structure();
    let structure = project.structure_report();
    let report = build_reinforcement_report(&project, &structure);
    let svg = reinforcement_detail_svg(&project, &structure, &report);

    assert!(svg.contains("DETALHAMENTO ESQUEMÁTICO DE ARMADURAS"));
    assert!(svg.contains("VIGA DE REFERÊNCIA"));
    assert!(svg.contains("PILAR DE REFERÊNCIA"));
    assert!(svg.contains("SAPATA DE REFERÊNCIA"));
    assert!(svg.contains("RESUMO DE AÇO POR BITOLA"));
    assert!(svg.contains("Branson B12:"));
    assert!(
        svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok(),
        "reinforcement_detail_svg must produce valid SVG"
    );
}

#[test]
fn reinforcement_is_refused_outside_the_structural_limits() {
    let mut project = Project::default();
    project.structure.settings.floors = limits::MAX_STOREYS + 1;
    let report = build_reinforcement_report(&project, &StructureReport::default());
    assert!(report.is_empty());
    assert!(report.warnings[0].contains("pavimentos"));
}

#[test]
fn nbr_6118_hook_cover_stirrup_overflow_and_slab_spacing_rules_hold() {
    // NBR 6118 §9.4.2.5: α1 = 0.7 only when cover >= 3Ø.
    // For Ø16 mm (3Ø = 48 mm) with 30 mm cover, hooked anchorage must NOT get the 0.7 discount.
    let lb_16_30mm =
        required_anchorage_with_cover_m(16.0, 25.0, BondZone::Good, true, 0.030, 4.0, 4.0);
    let lb_16_straight = basic_anchorage_m(16.0, 25.0, BondZone::Good);
    assert!((lb_16_30mm - lb_16_straight).abs() < 1e-6);

    // For Ø10 mm (3Ø = 30 mm) with 30 mm cover, hooked anchorage gets α1 = 0.7.
    let lb_10_30mm =
        required_anchorage_with_cover_m(10.0, 25.0, BondZone::Good, true, 0.030, 4.0, 4.0);
    let lb_10_straight = basic_anchorage_m(10.0, 25.0, BondZone::Good);
    assert!((lb_10_30mm - 0.7 * lb_10_straight).abs() < 1e-6);

    // Stirrups: 25×60 cm C40 beam with V_k = 607 kN -> V_sd = 850 kN <= V_Rd2 = 907 kN,
    // yet required Asw/s = 32.06 cm²/m > 31.42 cm²/m (Ø10 c/5): must NOT silently pass!
    let st = beam_stirrups(4.0, 0.25, 0.60, 607.0, 40.0);
    assert!(!st.shear_capacity_ok);

    // Slab spacing cap (NBR 6118 §20.1): h = 8 cm -> s_max = min(2h, 20) = 16 cm -> max 15 cm standard.
    let mesh = choose_mesh(1.5, 4.0, 4.0, 16.0);
    assert!(mesh.spacing_cm <= 16.0);
}
