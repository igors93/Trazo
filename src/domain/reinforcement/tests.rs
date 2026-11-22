use std::f64::consts::PI;

use super::bond::{
    basic_anchorage_m, bond_strength_mpa, required_anchorage_m, required_anchorage_with_cover_m,
};
use super::sizing::{beam_stirrups, choose_mesh, flexural_demand};
use super::*;
use crate::domain::{
    limits,
    model::Project,
    structure::{StructureReport, effective_beam_stiffness},
};

use crate::domain::geometry::Point;

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
