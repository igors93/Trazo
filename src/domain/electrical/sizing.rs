//! NBR 5410 Table 36 conductor ampacity, breaker sizing, load schedule and
//! automatic circuit assignment.

use crate::domain::{fixtures::Fixture, model::Project};

use super::{CircuitKind, CircuitRow, DeviceRole, LoadSchedule, RoomClass};

/// Copper PVC 70 °C ampacity in embedded conduit (NBR 5410 Table 36, method B1,
/// two loaded conductors at 30 °C): `(section mm², capacity A)`.
pub const WIRE_CAPACITY: [(f64, f64); 9] = [
    (1.5, 17.5),
    (2.5, 24.0),
    (4.0, 32.0),
    (6.0, 41.0),
    (10.0, 57.0),
    (16.0, 76.0),
    (25.0, 101.0),
    (35.0, 125.0),
    (50.0, 151.0),
];

/// Copper PVC 70 °C ampacity in embedded conduit (NBR 5410 Table 36, method B1,
/// THREE loaded conductors at 30 °C — required per NBR 5410 Table 46 for both
/// 2F+N and 3F+N feeders/entrances): `(section mm², capacity A)`.
pub const WIRE_CAPACITY_3_LOADED: [(f64, f64); 9] = [
    (1.5, 15.5),
    (2.5, 21.0),
    (4.0, 28.0),
    (6.0, 36.0),
    (10.0, 50.0),
    (16.0, 68.0),
    (25.0, 89.0),
    (35.0, 110.0),
    (50.0, 134.0),
];

/// Standard ratings of miniature circuit breakers, in amperes.
pub const BREAKERS: [f64; 12] = [
    10.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 70.0, 80.0, 100.0, 125.0,
];

/// Sizes a circuit's breaker and wire section (1.5 mm² lighting, 2.5 mm² outlets).
pub fn size_circuit(current_a: f64, kind: CircuitKind) -> (Option<f64>, Option<f64>) {
    let breaker = BREAKERS
        .iter()
        .copied()
        .find(|rating| *rating + 1e-9 >= current_a);
    let minimum = if kind == CircuitKind::Lighting {
        1.5
    } else {
        2.5
    };
    let wire = breaker.and_then(|rating| {
        WIRE_CAPACITY
            .iter()
            .find(|(section, capacity)| *section >= minimum && *capacity + 1e-9 >= rating)
            .map(|(section, _)| *section)
    });
    (breaker, wire)
}

impl Project {
    /// The load schedule: one row per circuit number, sized by NBR 5410.
    pub fn load_schedule(&self) -> LoadSchedule {
        let settings = self.electrical;
        let mut schedule = LoadSchedule::default();
        let mut by_circuit: std::collections::BTreeMap<u32, Vec<&Fixture>> =
            std::collections::BTreeMap::new();
        for device in self.electrical_devices() {
            if let Some(number) = device.electrical.and_then(|data| data.circuit) {
                by_circuit.entry(number).or_default().push(device);
            }
        }
        for (number, devices) in by_circuit {
            let mut lighting = 0;
            let mut general = 0;
            let mut specific = 0;
            let mut rooms: Vec<String> = Vec::new();
            let mut wet = false;
            let mut has_bathroom = false;
            let mut power = 0.0;
            for device in &devices {
                match device.kind.role() {
                    Some(DeviceRole::Lighting) => lighting += 1,
                    Some(DeviceRole::GeneralOutlet) => general += 1,
                    Some(DeviceRole::SpecificOutlet) => specific += 1,
                    _ => {}
                }
                power += device.electrical.map_or(0.0, |data| data.power_va);
                if let Some(room) = self.room_for_fixture(device) {
                    let class = Self::room_class(room);
                    wet |= class.is_wet() || class == RoomClass::Balcony;
                    has_bathroom |= class == RoomClass::Bathroom;
                    let name = room.display_name().to_owned();
                    if !rooms.contains(&name) {
                        rooms.push(name);
                    }
                }
            }
            let kind = match (lighting > 0, general > 0, specific > 0) {
                (true, false, false) => CircuitKind::Lighting,
                (false, true, false) => CircuitKind::GeneralOutlets,
                (false, false, true) => CircuitKind::SpecificOutlet,
                (false, false, false) => CircuitKind::Lighting,
                _ => CircuitKind::Mixed,
            };
            let voltage = if kind == CircuitKind::SpecificOutlet {
                settings.specific_voltage_v
            } else {
                settings.voltage_v
            };
            let current = power / voltage.max(1.0);
            let (breaker, wire) = size_circuit(current, kind);
            if kind == CircuitKind::Mixed {
                schedule.warnings.push(format!(
                    "Circuito {number}: separe iluminação e tomadas em circuitos diferentes (NBR 5410 9.5.3)."
                ));
            }
            if specific > 1 || (specific == 1 && devices.len() > 1 && current > 10.0) {
                schedule.warnings.push(format!(
                    "Circuito {number}: equipamentos de uso específico acima de 10 A pedem circuito exclusivo."
                ));
            }
            if breaker.is_none() || wire.is_none() {
                schedule.warnings.push(format!(
                    "Circuito {number}: {current:.1} A é demais para um circuito; divida as cargas."
                ));
            }
            schedule.total_va += power;
            schedule.circuits.push(CircuitRow {
                number,
                kind,
                rooms,
                points: devices.len(),
                power_va: power,
                voltage_v: voltage,
                current_a: current,
                wire_mm2: wire,
                breaker_a: breaker,
                // NBR 5410 §5.1.3.2.2: 30 mA DR required on ALL bathroom circuits
                // (including lighting) and on outlets in kitchens, service areas,
                // garages and outdoor/balcony spaces.
                needs_rcd: has_bathroom || (wet && kind != CircuitKind::Lighting),
                poles: 1,
                phases: Vec::new(),
                length_m: None,
                drop_percent: None,
                wire_by_drop: false,
                board: None,
            });
        }
        self.complete_schedule(&mut schedule);
        schedule.unassigned = self
            .electrical_devices()
            .filter(|device| {
                matches!(
                    device.kind.role(),
                    Some(
                        DeviceRole::Lighting
                            | DeviceRole::GeneralOutlet
                            | DeviceRole::SpecificOutlet
                    )
                ) && device.electrical.and_then(|data| data.circuit).is_none()
            })
            .count();
        if schedule.unassigned > 0 {
            schedule.warnings.push(format!(
                "{} ponto(s) sem circuito: use Distribuir circuitos.",
                schedule.unassigned
            ));
        }
        for forecast in self.electrical_forecast() {
            let missing = forecast.missing();
            if !missing.is_empty() && (forecast.lights + forecast.outlets) > 0 {
                schedule
                    .warnings
                    .push(format!("{}: {}.", forecast.name, missing.join(", ")));
            }
        }
        schedule
    }

    /// Numbers every circuit again: lighting first (up to 10 A each), then
    /// outlets (kitchens and service areas on circuits of their own, up to
    /// 12 A each), then one circuit per specific-use outlet. Switches follow
    /// the lights of their room. Returns how many circuits there are.
    pub fn assign_circuits(&mut self) -> u32 {
        let voltage = self.electrical.voltage_v.max(1.0);
        let lighting_cap = voltage * 10.0;
        let outlet_cap = voltage * 12.0;
        let room_of: Vec<(usize, Option<u64>, Option<RoomClass>)> = self
            .fixtures
            .iter()
            .enumerate()
            .filter(|(_, fixture)| fixture.kind.is_electrical())
            .map(|(index, fixture)| {
                let room = self.room_for_fixture(fixture);
                (index, room.map(|r| r.id), room.map(Self::room_class))
            })
            .collect();
        let room_order: Vec<u64> = self.rooms.iter().map(|room| room.id).collect();
        let order = |room: Option<u64>| {
            room.and_then(|id| room_order.iter().position(|r| *r == id))
                .unwrap_or(usize::MAX)
        };
        let mut next = 1;
        let mut light_of_room: Vec<(Option<u64>, u32)> = Vec::new();

        // Packs devices into circuits up to `cap`, keeping each group apart.
        let pack = |fixtures: &mut Vec<Fixture>,
                    mut items: Vec<(usize, Option<u64>)>,
                    cap: f64,
                    next: &mut u32,
                    remember: bool,
                    light_of_room: &mut Vec<(Option<u64>, u32)>| {
            items.sort_by_key(|(_, room)| order(*room));
            let mut load = 0.0;
            let mut open = false;
            for (index, room) in items {
                let power = fixtures[index].electrical.map_or(0.0, |d| d.power_va);
                if !open || load + power > cap + 1e-6 {
                    if open {
                        *next += 1;
                    }
                    open = true;
                    load = 0.0;
                }
                load += power;
                if let Some(data) = fixtures[index].electrical.as_mut() {
                    data.circuit = Some(*next);
                }
                if remember && !light_of_room.iter().any(|(r, _)| *r == room) {
                    light_of_room.push((room, *next));
                }
            }
            if open {
                *next += 1;
            }
        };

        let pick = |role: DeviceRole, wet: Option<bool>| -> Vec<(usize, Option<u64>)> {
            room_of
                .iter()
                .filter(|(index, _, class)| {
                    self.fixtures[*index].kind.role() == Some(role)
                        && wet.is_none_or(|wet| class.is_some_and(|c| c.is_wet()) == wet)
                })
                .map(|(index, room, _)| (*index, *room))
                .collect()
        };
        let lights = pick(DeviceRole::Lighting, None);
        let wet_outlets = pick(DeviceRole::GeneralOutlet, Some(true));
        let dry_outlets = pick(DeviceRole::GeneralOutlet, Some(false));
        let specific = pick(DeviceRole::SpecificOutlet, None);
        let switches = pick(DeviceRole::Switch, None);

        let mut fixtures = std::mem::take(&mut self.fixtures);
        for (index, _) in room_of.iter().map(|(i, r, _)| (*i, *r)) {
            if let Some(data) = fixtures[index].electrical.as_mut() {
                data.circuit = None;
            }
        }
        pack(
            &mut fixtures,
            lights,
            lighting_cap,
            &mut next,
            true,
            &mut light_of_room,
        );
        // Kitchen and service outlets: a circuit per room at least.
        // Deduplicate preserving first-seen room order even when fixtures are interleaved.
        let mut wet_rooms: Vec<Option<u64>> = Vec::new();
        for &(_, r) in &wet_outlets {
            if !wet_rooms.contains(&r) {
                wet_rooms.push(r);
            }
        }
        for room in wet_rooms {
            let items: Vec<(usize, Option<u64>)> = wet_outlets
                .iter()
                .copied()
                .filter(|(_, r)| *r == room)
                .collect();
            pack(
                &mut fixtures,
                items,
                outlet_cap,
                &mut next,
                false,
                &mut light_of_room,
            );
        }
        pack(
            &mut fixtures,
            dry_outlets,
            outlet_cap,
            &mut next,
            false,
            &mut light_of_room,
        );
        for item in specific {
            pack(
                &mut fixtures,
                vec![item],
                f64::INFINITY,
                &mut next,
                false,
                &mut light_of_room,
            );
        }
        for (index, room) in switches {
            let circuit = light_of_room
                .iter()
                .find(|(r, _)| *r == room)
                .map(|(_, circuit)| *circuit);
            if let Some(data) = fixtures[index].electrical.as_mut() {
                data.circuit = circuit;
            }
        }
        self.fixtures = fixtures;
        next - 1
    }
}
