use super::*;
use crate::domain::model::Project;

use crate::domain::geometry::Point;

fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
        .unwrap();
    project.generate_structure();
    // A complete foundation: the walls rest on ground beams or strip
    // footings, and the footings take their loads (#172 checks the weight).
    project.generate_wall_foundations();
    project.size_footings();
    project
}

#[test]
fn lights_follow_the_utilisation_thresholds() {
    assert_eq!(Light::from_utilisation(0.5), Light::Green);
    assert_eq!(Light::from_utilisation(0.85), Light::Green);
    assert_eq!(Light::from_utilisation(0.9), Light::Yellow);
    assert_eq!(Light::from_utilisation(1.0), Light::Yellow);
    assert_eq!(Light::from_utilisation(1.01), Light::Red);
    assert_eq!(Light::from_utilisation(f64::NAN), Light::Red);
    let check = Check::new("x", 0.6, String::new(), String::new());
    assert!((check.margin_percent() - 40.0).abs() < 1e-9);
}

#[test]
fn a_generated_house_has_no_red_and_every_element_is_judged() {
    let project = house();
    let summary = safety_summary(&project);
    let report = project.structure_report();
    let elements = report.slabs.len()
        + report.beams.iter().map(|l| l.spans.len()).sum::<usize>()
        + report.columns.len()
        + report.footings.len()
        + report.masonry.bands.len()
        + report.masonry.panels.len()
        + report.frames.len()
        + report.wall_foundations.len()
        + 2 // ±X and ±Y building wind verdicts (NBR 6123 / NBR 8681 ELU-EQU)
        + 2; // the site questions (#156) and the independent cross-check (#172)
    assert_eq!(summary.verdicts.len(), elements);
    assert_eq!(summary.red, 0, "{:?}", summary.verdicts);
    assert_eq!(summary.green + summary.yellow, elements);
    assert!(summary.verdicts.iter().all(|v| !v.checks.is_empty()));
    assert!(!summary.assumptions.is_empty());
}

#[test]
fn a_shallow_beam_turns_red_with_what_to_change_and_comes_first() {
    let mut project = house();
    let beam = project.structure.beams[0].id;
    project.update_beam(beam, None, Some(0.20)).unwrap();
    let summary = safety_summary(&project);
    assert!(summary.red >= 1);
    assert_eq!(summary.worst(), Some(Light::Red));
    let first = &summary.verdicts[0];
    assert_eq!(first.light(), Light::Red);
    let height = first.checks.iter().find(|c| c.name == "Altura").unwrap();
    assert_eq!(height.light, Light::Red);
    assert!(height.margin_percent() < 0.0);
    assert!(
        height.simple.contains("Aumente a viga"),
        "{}",
        height.simple
    );
    assert!(height.detail.contains("L/10") || height.detail.contains("L/12"));
}

#[test]
fn electrical_circuits_get_current_and_voltage_drop_checks() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(60.0, 10.0));
    let (w, d) = crate::domain::fixtures::FixtureKind::Panel.default_size();
    project
        .add_fixture(
            crate::domain::fixtures::FixtureKind::Panel,
            Point::new(0.0, 0.0),
            w,
            d,
            0,
        )
        .unwrap();
    let (w, d) = crate::domain::fixtures::FixtureKind::OutletSpecific.default_size();
    let far = project
        .add_fixture(
            crate::domain::fixtures::FixtureKind::OutletSpecific,
            Point::new(50.0, 0.0),
            w,
            d,
            0,
        )
        .unwrap();
    project
        .update_electrical(far, Some(Some(1)), Some(4400.0), None)
        .unwrap();
    let verdicts = electrical_verdicts(&project.load_schedule());
    let circuit = verdicts
        .iter()
        .find(|v| v.element.starts_with("Circuito C1"))
        .expect("circuit C1 verdict");
    let names: Vec<&str> = circuit.checks.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Corrente", "Queda de tensão"]);
    // The wire was raised to keep the drop within 4 %: not red.
    assert!(circuit.light() != Light::Red);
    // Service entrance also receives a verdict now.
    assert!(
        verdicts
            .iter()
            .any(|v| v.element.starts_with("Entrada de energia"))
    );
}

#[test]
fn reinforcement_and_branson_checks_appear_in_structure_verdicts() {
    let project = house();
    let summary = safety_summary(&project);

    let beam = summary
        .verdicts
        .iter()
        .find(|v| v.element.starts_with("Viga "))
        .expect("beam verdict");
    let deflection = beam.checks.iter().find(|c| c.name == "Flecha").unwrap();
    assert!(deflection.detail.contains("Mr ="));
    assert!(deflection.detail.contains("I_eq/Ic ="));
    assert!(beam.checks.iter().any(|c| c.name == "Armadura / Domínio"));
    assert!(beam.checks.iter().any(|c| c.name == "Cortante (biela)"));

    let column = summary
        .verdicts
        .iter()
        .find(|v| v.element.starts_with("Pilar "))
        .expect("column verdict");
    let steel_ratio = column
        .checks
        .iter()
        .find(|c| c.name == "Taxa de aço")
        .unwrap();
    assert!(steel_ratio.detail.contains("traspasse l0 ="));

    let slab = summary
        .verdicts
        .iter()
        .find(|v| v.element.starts_with("Laje "))
        .expect("slab verdict");
    assert!(slab.checks.iter().any(|c| c.name == "Armadura (domínio)"));
}
