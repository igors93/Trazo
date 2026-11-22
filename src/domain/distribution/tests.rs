use super::calculations::{
    drop_percent, earth_section, general_demand_factor, specific_demand_factor,
};
use super::*;
use crate::domain::{
    electrical::CircuitKind,
    model::Project,
    utility_profiles::{UtilityOverrides, UtilityProfile},
};

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;

fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(30.0, 30.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(5.0, 0.0), Point::new(8.0, 4.0), "Cozinha")
        .unwrap();
    project
        .add_room(Point::new(0.0, 4.0), Point::new(2.0, 6.0), "Banheiro")
        .unwrap();
    project
        .add_fixture(FixtureKind::Shower, Point::new(0.6, 5.4), 0.9, 0.9, 0)
        .unwrap();
    project.generate_electrical_points();
    project.assign_circuits();
    project
}

#[test]
fn demand_factors_follow_the_usual_tables() {
    assert_eq!(general_demand_factor(0.8), 0.86);
    assert_eq!(general_demand_factor(3.4), 0.59);
    assert_eq!(general_demand_factor(12.0), 0.24);
    assert_eq!(specific_demand_factor(1), 1.0);
    assert_eq!(specific_demand_factor(4), 0.76);
    assert_eq!(specific_demand_factor(40), 0.38);
    assert_eq!(earth_section(10.0), 10.0);
    assert_eq!(earth_section(25.0), 16.0);
    assert_eq!(earth_section(70.0), 35.0);
    // 10 m to a 1270 VA load at 127 V on 2.5 mm²: 2·(1/56)·10·10/2.5/127.
    let drop = drop_percent(&[(10.0, 10.0)], 2.5, 127.0);
    assert!((drop - 1.125).abs() < 0.01, "{drop}");
}

#[test]
fn the_board_balances_phases_and_sizes_the_entrance() {
    let project = house();
    let schedule = project.load_schedule();
    // The 220 V shower on a 127 V network goes between two phases, so
    // the supply is at least two-phase.
    let shower = schedule
        .circuits
        .iter()
        .find(|row| row.kind == CircuitKind::SpecificOutlet)
        .unwrap();
    assert_eq!(shower.poles, 2);
    assert_eq!(shower.phases.len(), 2);
    assert_eq!(schedule.supply, Supply::Two);
    // Every circuit is on a phase and the two phases carry the load.
    assert!(schedule.circuits.iter().all(|row| !row.phases.is_empty()));
    let total: f64 = schedule.phase_va.iter().sum();
    assert!((total - schedule.total_va).abs() < 1e-6);
    assert!(schedule.phase_va[2] == 0.0);
    let (a, b) = (schedule.phase_va[0], schedule.phase_va[1]);
    assert!((a - b).abs() / a.max(b) <= 0.10, "{a} × {b}");
    // Demand below the installed load; entrance breaker and feeder.
    assert!(schedule.demand.demand_va < schedule.total_va);
    let entrance = schedule.entrance.unwrap();
    assert!(entrance.breaker_a.unwrap() >= entrance.current_a);
    assert!(entrance.wire_mm2.unwrap() >= 6.0);
    assert!(schedule.warnings.is_empty(), "{:?}", schedule.warnings);
}

#[test]
fn a_long_run_gets_a_thicker_wire_for_voltage_drop() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(60.0, 10.0));
    let (w, d) = FixtureKind::Panel.default_size();
    project
        .add_fixture(FixtureKind::Panel, Point::new(0.0, 0.0), w, d, 0)
        .unwrap();
    let (w, d) = FixtureKind::OutletSpecific.default_size();
    let far = project
        .add_fixture(FixtureKind::OutletSpecific, Point::new(50.0, 0.0), w, d, 0)
        .unwrap();
    project
        .update_electrical(far, Some(Some(1)), Some(4400.0), None)
        .unwrap();
    let row = project.load_schedule().circuits[0].clone();
    // 20 A needs a 20 A breaker and 2.5 mm², but 50 m away the drop
    // asks for more.
    assert_eq!(row.breaker_a, Some(20.0));
    assert!(row.wire_by_drop);
    assert!(row.wire_mm2.unwrap() > 2.5);
    assert!(row.drop_percent.unwrap() <= MAX_DROP_PERCENT);
    assert!(row.length_m.unwrap() > 50.0);
}

#[test]
fn a_forced_single_phase_supply_warns_about_two_pole_circuits() {
    let mut project = house();
    project.electrical.supply = Supply::Single;
    let schedule = project.load_schedule();
    assert_eq!(schedule.supply, Supply::Single);
    assert!(schedule.warnings.iter().any(|w| w.contains("bifásica")));
    assert!(schedule.circuits.iter().all(|row| row.phases == vec![0]));
}

#[test]
fn the_single_line_diagram_is_valid_svg_with_every_circuit() {
    let project = house();
    let svg = single_line_svg(&project);
    let tree = svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default());
    assert!(tree.is_ok());
    for row in project.load_schedule().circuits {
        assert!(svg.contains(&format!(">C{}<", row.number)));
    }
    assert!(svg.contains("DPS"));
    assert!(svg.contains("Demanda provável"));
    let table = build_panel_schedule(&project);
    assert_eq!(table.rows.len(), project.load_schedule().circuits.len());
    // An empty project still draws a page.
    assert!(single_line_svg(&Project::default()).contains("Nenhum circuito"));
}

#[test]
fn sub_boards_are_modelled_with_dedicated_feeders_and_shortened_terminal_runs() {
    use super::super::conduits::ConduitRun;

    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(50.0, 20.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(30.0, 0.0), Point::new(35.0, 3.0), "Edícula")
        .unwrap();
    let (pw, pd) = FixtureKind::Panel.default_size();
    let main = project
        .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
        .unwrap();
    let sub = project
        .add_fixture(FixtureKind::Panel, Point::new(30.4, 0.4), pw, pd, 0)
        .unwrap();
    let (ow, od) = FixtureKind::OutletSpecific.default_size();
    let near_main = project
        .add_fixture(FixtureKind::OutletSpecific, Point::new(2.0, 1.5), ow, od, 0)
        .unwrap();
    let near_sub = project
        .add_fixture(
            FixtureKind::OutletSpecific,
            Point::new(32.0, 1.5),
            ow,
            od,
            0,
        )
        .unwrap();
    project
        .update_electrical(near_main, Some(Some(1)), Some(2200.0), None)
        .unwrap();
    project
        .update_electrical(near_sub, Some(Some(2)), Some(2200.0), None)
        .unwrap();

    project
        .add_conduit(main, near_main, ConduitRun::Ceiling)
        .unwrap();
    project.add_conduit(main, sub, ConduitRun::Floor).unwrap();
    project
        .add_conduit(sub, near_sub, ConduitRun::Ceiling)
        .unwrap();

    let schedule = project.load_schedule();
    assert_eq!(schedule.sub_boards.len(), 1);
    let qdc = &schedule.sub_boards[0];
    assert_eq!(qdc.panel_id, sub);
    assert_eq!(qdc.label, "QDC1");
    assert_eq!(qdc.circuits, vec![2]);
    assert!(qdc.feeder_length_m.unwrap() >= 30.0);
    assert!(qdc.breaker_a.is_some());
    assert!(qdc.wire_mm2.is_some());

    let c1 = schedule.circuits.iter().find(|c| c.number == 1).unwrap();
    let c2 = schedule.circuits.iter().find(|c| c.number == 2).unwrap();
    assert_eq!(c1.board.as_deref(), Some("QDG"));
    assert_eq!(c2.board.as_deref(), Some("QDC1"));
    // Terminal run for C2 starts at QDC1 (~2 m away), not QDG (~32 m away).
    assert!(c2.length_m.unwrap() < 10.0);

    let svg = single_line_svg(&project);
    assert!(svg.contains("QDC1"));
    assert!(svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok());
}

#[test]
fn utility_profiles_and_regional_overrides_change_demand_and_entrance() {
    let mut project = house();
    let generic = project.load_schedule();

    // CEMIG (ND 5.1) uses a more conservative residential lighting/TUG demand
    // table and 40 A / 10 mm² entrance floor, traced to ND 5.1.
    project.electrical.utility = UtilityProfile::Cemig;
    project.electrical.voltage_v = 127.0;
    let cemig = project.load_schedule();
    assert!(cemig.demand.general_factor >= generic.demand.general_factor);
    let cemig_ent = cemig.entrance.unwrap();
    assert!(cemig_ent.breaker_a.unwrap() >= 40.0);
    assert!(cemig_ent.wire_mm2.unwrap() >= 10.0);
    assert!(single_line_svg(&project).contains("CEMIG ND 5.1"));

    // COPEL (NTC 901100) uses 9 kVA mono / 15 kVA bi cutoffs, 40 A / 10 mm²
    // floor and its own TUE table without arbitrary scaling multipliers.
    project.electrical.utility = UtilityProfile::Copel;
    project.electrical.voltage_v = 220.0;
    let copel = project.load_schedule();
    let copel_ent = copel.entrance.unwrap();
    assert!(copel_ent.breaker_a.unwrap() >= 40.0);
    assert!(copel_ent.wire_mm2.unwrap() >= 10.0);
    assert!(single_line_svg(&project).contains("COPEL NTC 901100"));

    // Custom regional overrides can tailor thresholds for any utility.
    project.electrical.utility = UtilityProfile::Generic;
    project.electrical.voltage_v = 127.0;
    project.electrical.custom_rules = Some(UtilityOverrides {
        min_breaker_a: Some(50.0),
        min_wire_mm2: Some(16.0),
        ..Default::default()
    });
    let custom = project.load_schedule();
    let custom_ent = custom.entrance.unwrap();
    assert!(custom_ent.breaker_a.unwrap() >= 50.0);
    assert!(custom_ent.wire_mm2.unwrap() >= 16.0);

    // Soil and occupancy presets update structural settings cleanly.
    use super::super::structure::{OccupancyClass, SoilClass, room_live_load_kn_m2};
    let mut struct_settings = project.structure.settings;
    struct_settings.apply_soil_class(SoilClass::SoftClay);
    assert!((struct_settings.soil_kpa - 80.0).abs() < 1e-9);
    struct_settings.apply_occupancy(OccupancyClass::Office);
    assert!((struct_settings.live_kn_m2 - 2.0).abs() < 1e-9);
    assert!((room_live_load_kn_m2("Garagem", 1.5) - 3.0).abs() < 1e-9);
    assert!((room_live_load_kn_m2("Varanda", 1.5) - 2.5).abs() < 1e-9);
}

#[test]
fn entrance_uses_3_loaded_ampacity_and_does_not_dilute_specific_tue_current() {
    let mut project = Project::default();
    project.electrical.voltage_v = 220.0;
    project.electrical.specific_voltage_v = 220.0;
    project.electrical.supply = Supply::Three;
    project.electrical.utility = UtilityProfile::Generic;

    let (pw, pd) = FixtureKind::Panel.default_size();
    project
        .add_fixture(FixtureKind::Panel, Point::new(0.0, 0.0), pw, pd, 0)
        .unwrap();
    let (sw, sd) = FixtureKind::OutletSpecific.default_size();
    // 7,500 VA TUE shower (34.1 A @ factor 1.00) on Phase A.
    let shower = project
        .add_fixture(FixtureKind::OutletSpecific, Point::new(3.0, 0.0), sw, sd, 0)
        .unwrap();
    project
        .update_electrical(shower, Some(Some(1)), Some(7500.0), None)
        .unwrap();
    // 10,000 VA of general outlets across 5 circuits (factor 0.27).
    let (gw, gd) = FixtureKind::OutletLow.default_size();
    for c in 2..=6 {
        let out = project
            .add_fixture(
                FixtureKind::OutletLow,
                Point::new(f64::from(c), 2.0),
                gw,
                gd,
                0,
            )
            .unwrap();
        project
            .update_electrical(out, Some(Some(c)), Some(2000.0), None)
            .unwrap();
    }

    let schedule = project.load_schedule();
    let ent = schedule.entrance.unwrap();
    // Peak phase carries at least the full 34.1 A shower (NOT diluted to ~19 A by 0.27 general factor).
    assert!(
        ent.current_a >= 34.0,
        "shower current must not be diluted: got {:.1} A",
        ent.current_a
    );
    // Breaker >= 40 A on 3F+N (3 loaded conductors): Table 36 3-loaded requires >= 10 mm² (6 mm² is only 36 A!).
    assert!(ent.breaker_a.unwrap() >= 40.0);
    assert!(
        ent.wire_mm2.unwrap() >= 10.0,
        "40 A 3F+N entrance requires >= 10 mm² under NBR 5410 Table 36 (3 loaded), got {:?}",
        ent.wire_mm2
    );
}
