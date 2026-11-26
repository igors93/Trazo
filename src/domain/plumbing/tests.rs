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
