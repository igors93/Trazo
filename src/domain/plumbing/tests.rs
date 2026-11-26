use super::points::{PointKind, plumbing_points};
use super::*;
use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

/// A bathroom and a kitchen on a 10 × 20 m lot.
pub(super) fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    project
        .add_room(Point::new(2.0, 6.0), Point::new(4.5, 8.5), "Banheiro")
        .unwrap();
    project
        .add_room(Point::new(4.5, 6.0), Point::new(8.0, 9.0), "Cozinha")
        .unwrap();
    for (kind, at) in [
        (FixtureKind::Toilet, Point::new(2.5, 6.5)),
        (FixtureKind::Washbasin, Point::new(3.4, 6.4)),
        (FixtureKind::Shower, Point::new(4.0, 8.0)),
        (FixtureKind::KitchenSink, Point::new(6.0, 6.4)),
        (FixtureKind::Stove, Point::new(7.3, 6.4)),
    ] {
        let (w, d) = kind.default_size();
        project.add_fixture(kind, at, w, d, 0).unwrap();
    }
    project
}

#[test]
fn every_fixture_gets_its_points_and_the_schedule_adds_them_up() {
    let project = house();
    let points = plumbing_points(&project);
    assert_eq!(points.count(PointKind::Cold), 4);
    assert_eq!(points.count(PointKind::Drain), 4);
    assert_eq!(points.count(PointKind::Gas), 1);
    // Electric showers: no hot water.
    assert_eq!(points.count(PointKind::Hot), 0);
    assert_eq!(points.schedule.len(), 5);
    // Toilet 6 + washbasin 1 + shower 2 + sink 3 drainage units.
    assert!((points.total_uhc - 12.0).abs() < 1e-9);
    let toilet = points
        .points
        .iter()
        .find(|p| p.fixture == FixtureKind::Toilet && p.kind == PointKind::Drain)
        .unwrap();
    assert!((toilet.diameter_mm - 100.0).abs() < 1e-9);
    assert_eq!(toilet.room.as_deref(), Some("Banheiro"));
}

#[test]
fn a_gas_or_solar_heater_brings_hot_water_to_the_taps() {
    let mut project = house();
    project.plumbing.heater = Heater::Solar;
    let points = plumbing_points(&project);
    // Washbasin, shower and sink.
    assert_eq!(points.count(PointKind::Hot), 3);
}

#[test]
fn a_tank_just_over_the_slab_leaves_the_shower_weak_and_says_how_high_to_go() {
    use super::cold_water::cold_water;
    use super::network::Pressure;
    let project = house();
    let cold = cold_water(&project).unwrap();
    // No bedroom: two people, 150 L each, one day.
    assert_eq!(cold.residents, 2);
    assert!((cold.daily_l - 300.0).abs() < 1e-9);
    assert!((cold.tank_l - 310.0).abs() < 1e-9);
    // Walls of 2.80 m and a base of 0.20 m: 0.90 m over the shower head.
    assert!((cold.tank_base_m - 3.0).abs() < 1e-9);
    let shower = cold
        .network
        .outlets
        .iter()
        .find(|o| o.fixture == FixtureKind::Shower)
        .unwrap();
    assert_eq!(shower.state, Pressure::Low);
    assert!(cold.needed_base_m > 3.0);
    let toilet = cold
        .network
        .outlets
        .iter()
        .find(|o| o.fixture == FixtureKind::Toilet)
        .unwrap();
    assert_eq!(toilet.state, Pressure::Good);
    assert!(cold.warnings.iter().any(|w| w.contains("Suba a base")));

    let mut raised = house();
    raised.plumbing.tank_base_m = Some(cold.comfort_base_m + 0.05);
    let cold = cold_water(&raised).unwrap();
    assert!(
        cold.network
            .outlets
            .iter()
            .all(|o| o.state == Pressure::Good)
    );
}

#[test]
fn a_tank_heavier_than_the_structure_expects_is_red() {
    use crate::domain::verdict::{Light, plumbing_verdicts};
    let mut project = house();
    project.plumbing.residents = Some(6);
    project
        .add_fixture(FixtureKind::WaterTank, Point::new(3.0, 7.0), 1.2, 1.2, 0)
        .unwrap();
    let tank = project.fixtures.last_mut().unwrap();
    tank.structural_load_kn = Some(5.0);
    let verdicts = plumbing_verdicts(&project);
    let weight = verdicts
        .iter()
        .find(|v| v.element == "Caixa-d'água")
        .unwrap();
    assert_eq!(weight.light(), Light::Red);
    // Pressure problems are yellow, never red.
    assert!(
        verdicts
            .iter()
            .filter(|v| v.element.starts_with("Água fria"))
            .all(|v| v.light() != Light::Red)
    );
}

#[test]
fn several_tanks_share_the_reserve_by_what_each_checked_load_holds() {
    use super::cold_water::tank_fitting_load_l;
    use crate::domain::verdict::{Light, plumbing_verdicts};
    // A 500 L tank full weighs 5.9 kN; a 1000 L one 10.81 kN.
    assert!((tank_fitting_load_l(6.0) - 500.0).abs() < 1e-9);
    assert!((tank_fitting_load_l(10.0) - 750.0).abs() < 1e-9);
    assert!((tank_fitting_load_l(10.81) - 1000.0).abs() < 1e-9);

    let mut project = house();
    // Eight people, 150 L each, one day: 1200 L of reserve.
    project.plumbing.residents = Some(8);
    for (at, load) in [(Point::new(3.0, 7.0), 6.0), (Point::new(6.0, 7.0), 6.0)] {
        project
            .add_fixture(FixtureKind::WaterTank, at, 1.2, 1.2, 0)
            .unwrap();
        project.fixtures.last_mut().unwrap().structural_load_kn = Some(load);
    }
    let weight = |project: &Project| {
        plumbing_verdicts(project)
            .into_iter()
            .find(|v| v.element == "Caixa-d'água")
            .unwrap()
    };
    // Two 500 L tanks keep 1000 L: short of 1200 L.
    assert_eq!(weight(&project).light(), Light::Red);
    project
        .add_fixture(FixtureKind::WaterTank, Point::new(5.0, 8.0), 1.2, 1.2, 0)
        .unwrap();
    project.fixtures.last_mut().unwrap().structural_load_kn = Some(10.0);
    // 500 + 500 + 750 = 1750 L ≥ 1200 L.
    let shared = weight(&project);
    assert_eq!(shared.light(), Light::Green);
    assert!(shared.checks[0].simple.contains("dividem a reserva"));
}
