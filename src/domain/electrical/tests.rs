use super::forecast::{minimum_lighting_va, minimum_outlets};
use super::sizing::size_circuit;
use super::*;
use crate::domain::{
    fixtures::FixtureKind,
    geometry::Point,
    model::{OpeningKind, Project},
};

fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
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
}

#[test]
fn nbr_5410_minimums_for_lighting_and_outlets() {
    assert_eq!(minimum_lighting_va(5.0), 100.0);
    assert_eq!(minimum_lighting_va(6.0), 100.0);
    assert_eq!(minimum_lighting_va(9.9), 100.0);
    assert_eq!(minimum_lighting_va(10.0), 160.0);
    assert_eq!(minimum_lighting_va(20.0), 280.0);
    // A 4 × 3 m bedroom (14 m perimeter) needs 3 outlets; a kitchen of
    // the same size needs 4 (one per 3.5 m).
    assert_eq!(minimum_outlets(RoomClass::Living, 12.0, 14.0), 3);
    assert_eq!(minimum_outlets(RoomClass::KitchenService, 12.0, 14.0), 4);
    assert_eq!(minimum_outlets(RoomClass::Bathroom, 3.0, 7.0), 1);
    assert_eq!(minimum_outlets(RoomClass::Other, 2.0, 6.0), 1);
    assert_eq!(outlet_power_va(RoomClass::KitchenService, 2), 600.0);
    assert_eq!(outlet_power_va(RoomClass::KitchenService, 3), 100.0);
    assert_eq!(outlet_power_va(RoomClass::Living, 0), 100.0);
    assert_eq!(
        RoomClass::from_name("Área de Serviço"),
        RoomClass::KitchenService
    );
    assert_eq!(RoomClass::from_name("Suíte master"), RoomClass::Living);
    assert_eq!(RoomClass::from_name("Lavabo"), RoomClass::Bathroom);
    assert_eq!(RoomClass::from_name("Circulação"), RoomClass::Other);
}

#[test]
fn circuits_are_sized_with_breaker_then_wire() {
    // 1270 VA at 127 V = 10 A: 10 A breaker, 1.5 mm² for lighting.
    assert_eq!(
        size_circuit(10.0, CircuitKind::Lighting),
        (Some(10.0), Some(1.5))
    );
    // Outlets never go below 2.5 mm².
    assert_eq!(
        size_circuit(8.0, CircuitKind::GeneralOutlets),
        (Some(10.0), Some(2.5))
    );
    // A 5500 W shower at 220 V = 25 A: 25 A breaker, 4 mm² (32 A).
    assert_eq!(
        size_circuit(25.0, CircuitKind::SpecificOutlet),
        (Some(25.0), Some(4.0))
    );
    // 130 A has no breaker in the list.
    assert_eq!(size_circuit(130.0, CircuitKind::GeneralOutlets).0, None);
}

#[test]
fn generating_meets_the_norm_and_circuits_follow_the_rules() {
    let mut project = house();
    let shower = project.add_fixture(FixtureKind::Shower, Point::new(0.6, 5.4), 0.9, 0.9, 0);
    assert!(shower.is_some());
    let report = project.generate_electrical_points();
    assert_eq!(report.lights, 3);
    assert_eq!(report.specific, 1);
    assert!(report.switches == 0, "no doors yet, so no switch");
    for forecast in project.electrical_forecast() {
        assert!(
            forecast.missing().is_empty(),
            "{}: {:?}",
            forecast.name,
            forecast.missing()
        );
    }
    // Every outlet stands inside its room, on a wall face.
    for device in project.electrical_devices() {
        assert!(
            project.room_holding(device.center).is_some(),
            "{:?}",
            device.kind
        );
    }
    // Running it again adds nothing.
    assert_eq!(project.generate_electrical_points().total(), 0);

    let circuits = project.assign_circuits();
    let schedule = project.load_schedule();
    assert_eq!(schedule.circuits.len() as u32, circuits);
    assert_eq!(schedule.unassigned, 0);
    assert!(
        schedule
            .circuits
            .iter()
            .all(|c| c.kind != CircuitKind::Mixed)
    );
    // The shower has a circuit of its own at 220 V with an RCD.
    let shower_circuit = schedule
        .circuits
        .iter()
        .find(|c| c.kind == CircuitKind::SpecificOutlet)
        .unwrap();
    assert_eq!(shower_circuit.points, 1);
    assert_eq!(shower_circuit.voltage_v, 220.0);
    assert_eq!(shower_circuit.breaker_a, Some(25.0));
    assert_eq!(shower_circuit.wire_mm2, Some(4.0));
    assert!(shower_circuit.needs_rcd);
    // Kitchen outlets do not share a circuit with the living room.
    let kitchen = schedule
        .circuits
        .iter()
        .find(|c| c.rooms.contains(&"Cozinha".to_owned()) && c.kind == CircuitKind::GeneralOutlets)
        .unwrap();
    assert!(!kitchen.rooms.contains(&"Sala".to_owned()));
    assert!(kitchen.needs_rcd);
    assert!(schedule.warnings.is_empty(), "{:?}", schedule.warnings);
}

#[test]
fn a_switch_goes_beside_the_door_and_follows_the_room_lights() {
    let mut project = house();
    let wall = project
        .walls
        .iter()
        .find(|wall| {
            (wall.start.y - 0.0).abs() < 1e-6
                && (wall.end.y - 0.0).abs() < 1e-6
                && wall.start.x.min(wall.end.x) < 1.0
        })
        .unwrap()
        .id;
    project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
    let report = project.generate_electrical_points();
    assert!(report.switches >= 1);
    project.assign_circuits();
    let sala = project
        .rooms
        .iter()
        .find(|r| r.display_name() == "Sala")
        .unwrap()
        .id;
    let in_sala = |kind: FixtureKind| {
        project
            .electrical_devices()
            .find(|d| d.kind == kind && project.room_holding(d.center).map(|r| r.id) == Some(sala))
            .and_then(|d| d.electrical)
            .and_then(|data| data.circuit)
    };
    assert_eq!(
        in_sala(FixtureKind::Switch),
        in_sala(FixtureKind::CeilingLight)
    );
}

#[test]
fn device_data_is_validated() {
    let mut project = house();
    let (w, d) = FixtureKind::OutletLow.default_size();
    let id = project
        .add_fixture(FixtureKind::OutletLow, Point::new(1.0, 1.0), w, d, 0)
        .unwrap();
    assert!(
        project
            .update_electrical(id, Some(Some(3)), Some(600.0), Some(1.2))
            .is_ok()
    );
    let data = project
        .fixtures
        .iter()
        .find(|f| f.id == id)
        .unwrap()
        .electrical
        .unwrap();
    assert_eq!(
        (data.circuit, data.power_va, data.height_m),
        (Some(3), 600.0, 1.2)
    );
    assert!(
        project
            .update_electrical(id, None, Some(-1.0), None)
            .is_err()
    );
    assert!(
        project
            .update_electrical(id, Some(Some(0)), None, None)
            .is_err()
    );
    assert!(
        project
            .update_electrical(id, None, None, Some(9.0))
            .is_err()
    );
    // Furniture is not an electrical device.
    let table = project
        .add_fixture(FixtureKind::Table, Point::new(2.5, 2.0), 1.0, 0.8, 0)
        .unwrap();
    assert!(
        project
            .update_electrical(table, None, Some(100.0), None)
            .is_err()
    );
}

#[test]
fn boundary_device_is_assigned_to_the_room_it_faces() {
    let mut project = Project::default();
    let sala = project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    let banheiro = project
        .add_room(Point::new(4.0, 0.0), Point::new(6.0, 3.0), "Banheiro")
        .unwrap();
    let (w, d) = FixtureKind::OutletLow.default_size();
    // Both outlets sit right on the shared wall centreline (x = 4.0):
    // quarter_turns = 1 faces left (-X, into Sala);
    // quarter_turns = 3 faces right (+X, into Banheiro).
    let into_sala = project
        .add_fixture(FixtureKind::OutletLow, Point::new(4.0, 1.0), w, d, 1)
        .unwrap();
    let into_bath = project
        .add_fixture(FixtureKind::OutletLow, Point::new(4.0, 2.0), w, d, 3)
        .unwrap();

    let sala_dev = project.fixtures.iter().find(|f| f.id == into_sala).unwrap();
    let bath_dev = project.fixtures.iter().find(|f| f.id == into_bath).unwrap();
    assert_eq!(project.room_for_fixture(sala_dev).map(|r| r.id), Some(sala));
    assert_eq!(
        project.room_for_fixture(bath_dev).map(|r| r.id),
        Some(banheiro)
    );

    project.assign_circuits();
    let sala_circuit = project
        .fixtures
        .iter()
        .find(|f| f.id == into_sala)
        .unwrap()
        .electrical
        .unwrap()
        .circuit
        .unwrap();
    let bath_circuit = project
        .fixtures
        .iter()
        .find(|f| f.id == into_bath)
        .unwrap()
        .electrical
        .unwrap()
        .circuit
        .unwrap();
    assert_ne!(sala_circuit, bath_circuit);
    let schedule = project.load_schedule();
    let sala_row = schedule
        .circuits
        .iter()
        .find(|c| c.number == sala_circuit)
        .unwrap();
    let bath_row = schedule
        .circuits
        .iter()
        .find(|c| c.number == bath_circuit)
        .unwrap();
    assert!(!sala_row.needs_rcd);
    assert!(bath_row.needs_rcd);
}
