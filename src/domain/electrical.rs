//! Electrical design on the floor plan, following ABNT NBR 5410: devices
//! (outlets, switches, lights, panel, data points), the minimum load forecast
//! of each room, circuit sizing (current, wire gauge, breaker, RCD) and the
//! load schedule.
//!
//! Devices are fixtures of an electrical kind carrying [`ElectricalData`], so
//! they select, move, copy, rotate, group, undo and export like any other
//! object, and stand against the face of the nearest wall.

use serde::{Deserialize, Serialize};

use super::fixtures::{Fixture, FixtureKind, SymbolShape, Weight};
use super::geometry::{self, Point};
use super::model::{OpeningKind, Project, Room};

/// What a device needs to be designed: its circuit, power and height.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectricalData {
    /// Circuit number in the load schedule, once assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit: Option<u32>,
    /// Apparent power, in volt-amperes.
    #[serde(default)]
    pub power_va: f64,
    /// Mounting height above the floor, in metres (0 for ceiling points).
    #[serde(default)]
    pub height_m: f64,
}

/// Project-wide electrical settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectricalSettings {
    /// Phase-to-neutral voltage of general circuits: 127 or 220 V.
    #[serde(default = "default_voltage")]
    pub voltage_v: f64,
    /// Voltage of specific-use circuits (showers, ovens): usually 220 V.
    #[serde(default = "default_specific_voltage")]
    pub specific_voltage_v: f64,
    /// How the utility supplies the building (automatic from the demand).
    #[serde(default)]
    pub supply: super::distribution::Supply,
}

fn default_voltage() -> f64 {
    127.0
}

fn default_specific_voltage() -> f64 {
    220.0
}

impl Default for ElectricalSettings {
    fn default() -> Self {
        Self {
            voltage_v: default_voltage(),
            specific_voltage_v: default_specific_voltage(),
            supply: super::distribution::Supply::default(),
        }
    }
}

impl ElectricalSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// What a device does in the installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceRole {
    /// A light point.
    Lighting,
    /// A general-use outlet (TUG).
    GeneralOutlet,
    /// A specific-use outlet (TUE): shower, oven, air conditioner.
    SpecificOutlet,
    /// A switch: commands lights, draws no power.
    Switch,
    /// The distribution board.
    Panel,
    /// Data, phone or TV: low voltage, not in the load schedule.
    Data,
}

impl FixtureKind {
    pub const ELECTRICAL: [FixtureKind; 11] = [
        FixtureKind::OutletLow,
        FixtureKind::OutletMid,
        FixtureKind::OutletHigh,
        FixtureKind::OutletSpecific,
        FixtureKind::Switch,
        FixtureKind::SwitchTwoWay,
        FixtureKind::SwitchIntermediate,
        FixtureKind::CeilingLight,
        FixtureKind::WallLight,
        FixtureKind::Panel,
        FixtureKind::DataPoint,
    ];

    pub fn is_electrical(self) -> bool {
        self.role().is_some()
    }

    pub fn role(self) -> Option<DeviceRole> {
        Some(match self {
            Self::OutletLow | Self::OutletMid | Self::OutletHigh => DeviceRole::GeneralOutlet,
            Self::OutletSpecific => DeviceRole::SpecificOutlet,
            Self::Switch | Self::SwitchTwoWay | Self::SwitchIntermediate => DeviceRole::Switch,
            Self::CeilingLight | Self::WallLight => DeviceRole::Lighting,
            Self::Panel => DeviceRole::Panel,
            Self::DataPoint => DeviceRole::Data,
            _ => return None,
        })
    }

    /// Whether the device goes on a wall (everything but ceiling points and
    /// structural elements, which stand on the wall's centre line).
    pub fn mounts_on_wall(self) -> bool {
        self != Self::CeilingLight && !self.is_structural()
    }

    /// The usual power and height of a new device of this kind.
    pub fn default_electrical(self) -> Option<ElectricalData> {
        let (power_va, height_m) = match self {
            Self::OutletLow => (100.0, 0.30),
            Self::OutletMid => (100.0, 1.20),
            Self::OutletHigh => (100.0, 2.20),
            Self::OutletSpecific => (1500.0, 0.30),
            Self::Switch | Self::SwitchTwoWay | Self::SwitchIntermediate => (0.0, 1.10),
            Self::CeilingLight => (100.0, 0.0),
            Self::WallLight => (60.0, 2.00),
            Self::Panel => (0.0, 1.50),
            Self::DataPoint => (0.0, 0.30),
            _ => return None,
        };
        Some(ElectricalData {
            circuit: None,
            power_va,
            height_m,
        })
    }
}

/// The plan symbol of an electrical device, in the fixture's frame: the back
/// (against the wall) at y = -depth/2. Shapes follow common Brazilian
/// practice: triangles for outlets (emptier the lower they are mounted),
/// circles for switches and lights, a hatched box for the panel.
pub(super) fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    use Weight::{Detail, Outline};
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let poly = |points: &[(f64, f64)], closed: bool, weight: Weight| SymbolShape::Polyline {
        points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
        closed,
        dashed: false,
        weight,
    };
    let circle = |x: f64, y: f64, radius: f64, weight: Weight| SymbolShape::Circle {
        center: Point::new(x, y),
        radius,
        weight,
    };
    // An outlet: a triangle with its base on the wall, filled with hatching
    // from the wall up to `fill` (0 empty, 1 full).
    let outlet = |fill: f64| {
        let base = hw * 0.85;
        let mut shapes = vec![poly(&[(-base, -hd), (base, -hd), (0.0, hd)], true, Outline)];
        let rows = 5;
        for row in 1..rows {
            let t = f64::from(row) / f64::from(rows);
            if t > fill {
                break;
            }
            let y = -hd + t * 2.0 * hd;
            let half = base * (1.0 - t);
            shapes.push(poly(&[(-half, y), (half, y)], false, Detail));
        }
        shapes
    };
    let radius = width.min(depth) * 0.42;
    match kind {
        FixtureKind::OutletLow => outlet(0.0),
        FixtureKind::OutletMid => outlet(0.5),
        FixtureKind::OutletHigh => outlet(1.0),
        FixtureKind::OutletSpecific => {
            let mut shapes = outlet(0.0);
            shapes.push(circle(0.0, -hd / 3.0, radius * 0.35, Outline));
            shapes
        }
        FixtureKind::Switch | FixtureKind::SwitchTwoWay | FixtureKind::SwitchIntermediate => {
            let r = radius * 0.8;
            let mut shapes = vec![
                circle(0.0, 0.0, r, Outline),
                poly(&[(0.0, -hd), (0.0, -r)], false, Detail),
            ];
            if kind == FixtureKind::SwitchIntermediate {
                // Four-way (intermediate) switch: crossed strokes inside the circle.
                let d = r * 0.55;
                shapes.push(poly(&[(-d, d), (d, -d)], false, Outline));
                shapes.push(poly(&[(-d, -d), (d, d)], false, Outline));
            } else {
                // One stroke for a one-way switch, two for a two-way (three-way).
                let strokes: &[f64] = if kind == FixtureKind::Switch {
                    &[0.0]
                } else {
                    &[-r * 0.45, r * 0.45]
                };
                for dx in strokes {
                    shapes.push(poly(
                        &[(dx - r * 0.35, r * 0.55), (dx + r * 0.35, -r * 0.55)],
                        false,
                        Outline,
                    ));
                }
            }
            shapes
        }
        FixtureKind::CeilingLight => {
            let d = radius * std::f64::consts::FRAC_1_SQRT_2;
            vec![
                circle(0.0, 0.0, radius, Outline),
                poly(&[(-d, -d), (d, d)], false, Detail),
                poly(&[(-d, d), (d, -d)], false, Detail),
            ]
        }
        FixtureKind::WallLight => {
            let r = hw.min(depth) * 0.9;
            let arc: Vec<(f64, f64)> = (0..=12)
                .map(|i| {
                    let a = std::f64::consts::PI * f64::from(i) / 12.0;
                    (r * a.cos(), -hd + r * a.sin())
                })
                .collect();
            vec![poly(&arc, true, Outline)]
        }
        FixtureKind::Panel => {
            let mut shapes = vec![poly(
                &[(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)],
                true,
                Outline,
            )];
            // Diagonal hatching, clipped to the box.
            let step = width / 6.0;
            let mut x = -hw - depth;
            while x < hw {
                let (x0, y0) = (x.max(-hw), -hd + (x.max(-hw) - x));
                let x1 = (x + depth).min(hw);
                let y1 = -hd + (x1 - x);
                if x1 > x0 && y0 < hd {
                    shapes.push(poly(&[(x0, y0), (x1, y1.min(hd))], false, Detail));
                }
                x += step;
            }
            shapes
        }
        FixtureKind::DataPoint => vec![
            poly(
                &[
                    (-hw * 0.8, -hd),
                    (hw * 0.8, -hd),
                    (hw * 0.8, hd),
                    (-hw * 0.8, hd),
                ],
                true,
                Outline,
            ),
            poly(&[(-hw * 0.8, hd), (hw * 0.8, -hd)], false, Detail),
        ],
        _ => Vec::new(),
    }
}

/// How NBR 5410 treats a room when forecasting its loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomClass {
    /// Bathrooms and washrooms.
    Bathroom,
    /// Kitchens, pantries, service areas and laundries.
    KitchenService,
    /// Balconies and terraces.
    Balcony,
    /// Living rooms, bedrooms and offices.
    Living,
    /// Halls, corridors and everything else.
    Other,
}

impl RoomClass {
    /// Guesses the class from the room's name.
    pub fn from_name(name: &str) -> Self {
        let name = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
        if has(&[
            "banheiro",
            "lavabo",
            "banho",
            "sanitário",
            "sanitario",
            "wc",
        ]) {
            Self::Bathroom
        } else if has(&[
            "cozinha",
            "copa",
            "serviço",
            "servico",
            "lavanderia",
            "gourmet",
        ]) {
            Self::KitchenService
        } else if has(&["varanda", "sacada", "terraço", "terraco", "alpendre"]) {
            Self::Balcony
        } else if has(&[
            "sala",
            "estar",
            "jantar",
            "quarto",
            "dormitório",
            "dormitorio",
            "suíte",
            "suite",
            "escritório",
            "escritorio",
            "home",
        ]) {
            Self::Living
        } else {
            Self::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bathroom => "Banheiro",
            Self::KitchenService => "Cozinha/serviço",
            Self::Balcony => "Varanda",
            Self::Living => "Sala/quarto",
            Self::Other => "Outros",
        }
    }

    /// Whether outlets here are "wet" (600 VA, exclusive circuits, RCD).
    pub fn is_wet(self) -> bool {
        matches!(self, Self::Bathroom | Self::KitchenService)
    }
}

/// NBR 5410 9.5.2.1.2: at least 100 VA for the first 6 m², plus 60 VA for
/// each whole 4 m² beyond that.
pub fn minimum_lighting_va(area_m2: f64) -> f64 {
    if area_m2 <= 6.0 {
        100.0
    } else {
        100.0 + 60.0 * ((area_m2 - 6.0) / 4.0 + 1e-9).floor()
    }
}

/// NBR 5410 9.5.2.2.1: the minimum number of general-use outlets.
pub fn minimum_outlets(class: RoomClass, area_m2: f64, perimeter_m: f64) -> usize {
    let per = |length: f64| ((perimeter_m / length) - 1e-9).ceil().max(1.0) as usize;
    match class {
        RoomClass::Bathroom | RoomClass::Balcony => 1,
        RoomClass::KitchenService => per(3.5),
        RoomClass::Living => per(5.0),
        RoomClass::Other if area_m2 <= 6.0 => 1,
        RoomClass::Other => per(5.0),
    }
}

/// NBR 5410 9.5.2.2.2: the power of the `index`-th outlet (from 0) of a room.
pub fn outlet_power_va(class: RoomClass, index: usize) -> f64 {
    if class.is_wet() && index < 3 {
        600.0
    } else {
        100.0
    }
}

/// The NBR 5410 forecast for one room, next to what is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomForecast {
    pub room_id: u64,
    pub name: String,
    pub class: RoomClass,
    pub area_m2: f64,
    pub perimeter_m: f64,
    pub min_lighting_va: f64,
    pub min_outlets: usize,
    pub min_outlet_va: f64,
    pub lights: usize,
    pub lighting_va: f64,
    pub outlets: usize,
    pub outlet_va: f64,
}

impl RoomForecast {
    /// What the room still lacks, in words; empty when it meets the norm.
    pub fn missing(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if self.lights == 0 {
            missing.push("sem ponto de luz".to_owned());
        } else if self.lighting_va + 1e-6 < self.min_lighting_va {
            missing.push(format!(
                "iluminação {:.0} VA (mín. {:.0} VA)",
                self.lighting_va, self.min_lighting_va
            ));
        }
        if self.outlets < self.min_outlets {
            missing.push(format!(
                "{} tomada(s) de {} exigida(s)",
                self.outlets, self.min_outlets
            ));
        }
        missing
    }
}

/// Wire capacity for copper, PVC 70 °C, installation method B1, two loaded
/// conductors at 30 °C (NBR 5410 table 36): (section mm², capacity A).
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

/// Standard ratings of miniature circuit breakers, in amperes.
pub const BREAKERS: [f64; 12] = [
    10.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 70.0, 80.0, 100.0, 125.0,
];

/// The circuit's kind in the schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitKind {
    Lighting,
    GeneralOutlets,
    SpecificOutlet,
    /// Lights and outlets on one circuit, which NBR 5410 asks to separate.
    Mixed,
}

impl CircuitKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::GeneralOutlets => "Tomadas (TUG)",
            Self::SpecificOutlet => "Uso específico (TUE)",
            Self::Mixed => "Misto",
        }
    }

    /// The abbreviation used on the board and on sheets.
    pub fn short(self) -> &'static str {
        match self {
            Self::Lighting => "Ilum.",
            Self::GeneralOutlets => "TUG",
            Self::SpecificOutlet => "TUE",
            Self::Mixed => "Misto",
        }
    }
}

/// One row of the load schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitRow {
    pub number: u32,
    pub kind: CircuitKind,
    pub rooms: Vec<String>,
    pub points: usize,
    pub power_va: f64,
    pub voltage_v: f64,
    pub current_a: f64,
    /// Smallest section that carries the breaker's current, in mm².
    pub wire_mm2: Option<f64>,
    pub breaker_a: Option<f64>,
    /// Whether NBR 5410 5.1.3.2.2 asks for a 30 mA RCD (wet or outdoor areas).
    pub needs_rcd: bool,
    /// Breaker poles: 2 for 220 V between the phases of a 127 V network.
    pub poles: u8,
    /// The phases it is connected to (0 = A, 1 = B, 2 = C).
    pub phases: Vec<u8>,
    /// Cable length to the farthest point, once there is a board.
    pub length_m: Option<f64>,
    /// Voltage drop at the farthest point, in percent.
    pub drop_percent: Option<f64>,
    /// Whether the wire was raised above the breaker's to limit the drop.
    pub wire_by_drop: bool,
    /// Serving board label when the project has sub-boards (`QDG`, `QDC1`, …).
    pub board: Option<String>,
}

/// The circuits, their totals and what needs attention.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadSchedule {
    pub circuits: Vec<CircuitRow>,
    pub total_va: f64,
    pub unassigned: usize,
    pub warnings: Vec<String>,
    /// Installed loads with the demand factors applied.
    pub demand: super::distribution::Demand,
    /// The supply in use (resolved when automatic).
    pub supply: super::distribution::Supply,
    /// Installed load on each phase after balancing, in VA.
    pub phase_va: [f64; 3],
    /// Main breaker, feeder and conduit of the utility connection.
    pub entrance: Option<super::distribution::ServiceEntrance>,
    /// Secondary distribution boards (QDC) fed from the main board.
    pub sub_boards: Vec<super::distribution::SubBoardSummary>,
}

/// Breaker and wire for a circuit drawing `current_a`: the smallest standard
/// breaker at or above the current, then the smallest section that carries
/// the breaker's rating and meets the minimum for the circuit's use
/// (1.5 mm² for lighting, 2.5 mm² for outlets).
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

/// The room a tag takes on the plan, in metres: its text is drawn about
/// 0.13 m high on screen and on paper.
pub fn tag_size(text: &str) -> (f64, f64) {
    (0.075 * text.chars().count() as f64 + 0.06, 0.2)
}

/// What generating the minimum installation added.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationReport {
    pub lights: usize,
    pub outlets: usize,
    pub switches: usize,
    pub specific: usize,
}

impl GenerationReport {
    pub fn total(&self) -> usize {
        self.lights + self.outlets + self.switches + self.specific
    }
}

/// A straight piece of a room's boundary, with the side facing into the room.
struct Side {
    from: Point,
    to: Point,
    /// Unit vector from the wall into the room.
    inward: Point,
    wall_thickness: f64,
}

impl Side {
    fn length(&self) -> f64 {
        self.from.distance(self.to)
    }

    fn at(&self, s: f64) -> Point {
        self.from.lerp(self.to, s / self.length().max(1e-9))
    }

    /// The fixture rotation whose back faces this wall, if the wall is
    /// horizontal or vertical (fixtures turn in quarter turns).
    fn quarter_turns(&self) -> Option<u8> {
        let back = Point::new(-self.inward.x, -self.inward.y);
        if back.y < -0.99 {
            Some(0)
        } else if back.x > 0.99 {
            Some(1)
        } else if back.y > 0.99 {
            Some(2)
        } else if back.x < -0.99 {
            Some(3)
        } else {
            None
        }
    }
}

impl Project {
    /// The fixtures that are electrical devices.
    pub fn electrical_devices(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind.is_electrical())
    }

    /// The room whose outline holds `point`, if any.
    pub fn room_holding(&self, point: Point) -> Option<&Room> {
        self.rooms.iter().find(|room| {
            self.room_polygon(room)
                .is_some_and(|polygon| geometry::polygon_contains_point(&polygon, point))
        })
    }

    fn polygon_edge_distance(polygon: &[Point], point: Point) -> f64 {
        (0..polygon.len())
            .map(|i| {
                geometry::distance_to_segment(point, polygon[i], polygon[(i + 1) % polygon.len()])
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// The room that strictly contains `point` (clear of shared boundaries).
    fn room_strictly_holding(&self, point: Point) -> Option<&Room> {
        const EDGE_EPS: f64 = 1e-3;
        self.rooms.iter().find(|room| {
            self.room_polygon(room).is_some_and(|polygon| {
                geometry::polygon_contains_point(&polygon, point)
                    && Self::polygon_edge_distance(&polygon, point) > EDGE_EPS
            })
        })
    }

    /// The room a fixture belongs to.
    ///
    /// Wall-mounted devices stand with their back against the wall and their
    /// front facing into the room they serve (`+Y` in local coordinates), so a
    /// device placed right on a shared wall boundary is assigned to the room
    /// its front faces rather than whichever room happens to come first.
    pub fn room_for_fixture(&self, fixture: &Fixture) -> Option<&Room> {
        if fixture.kind != FixtureKind::CeilingLight {
            let front_probes = [
                fixture.to_world(Point::new(0.0, fixture.depth * 0.45)),
                fixture.to_world(Point::new(0.0, fixture.depth * 0.5 + 0.05)),
                fixture.to_world(Point::new(fixture.width * 0.25, fixture.depth * 0.45)),
                fixture.to_world(Point::new(-fixture.width * 0.25, fixture.depth * 0.45)),
            ];
            for probe in front_probes {
                if let Some(room) = self.room_strictly_holding(probe) {
                    return Some(room);
                }
            }
        }
        if let Some(room) = self.room_strictly_holding(fixture.center) {
            return Some(room);
        }
        let samples = [
            fixture.center,
            fixture.to_world(Point::new(fixture.width * 0.35, fixture.depth * 0.35)),
            fixture.to_world(Point::new(-fixture.width * 0.35, fixture.depth * 0.35)),
            fixture.to_world(Point::new(fixture.width * 0.35, -fixture.depth * 0.35)),
            fixture.to_world(Point::new(-fixture.width * 0.35, -fixture.depth * 0.35)),
        ];
        self.rooms
            .iter()
            .filter_map(|room| {
                let poly = self.room_polygon(room)?;
                let hits = samples
                    .iter()
                    .filter(|&&p| {
                        geometry::polygon_contains_point(&poly, p)
                            && Self::polygon_edge_distance(&poly, p) > 1e-4
                    })
                    .count();
                (hits > 0).then_some((room, hits))
            })
            .max_by_key(|&(_, hits)| hits)
            .map(|(room, _)| room)
            .or_else(|| self.room_holding(fixture.center))
    }

    fn room_class(room: &Room) -> RoomClass {
        RoomClass::from_name(room.display_name())
    }

    fn room_perimeter(&self, room: &Room) -> f64 {
        self.room_outline(room)
            .map(|outline| {
                (0..outline.len())
                    .map(|i| outline[i].distance(outline[(i + 1) % outline.len()]))
                    .fold(0.0, |total, side| total + side)
            })
            .unwrap_or(0.0)
    }

    /// The NBR 5410 forecast of every room, next to the devices drawn in it.
    pub fn electrical_forecast(&self) -> Vec<RoomForecast> {
        self.rooms
            .iter()
            .map(|room| {
                let class = Self::room_class(room);
                let area = self
                    .room_usable_area(room)
                    .or_else(|| self.room_area(room))
                    .unwrap_or(0.0);
                let perimeter = self.room_perimeter(room);
                let min_outlets = minimum_outlets(class, area, perimeter);
                let mut forecast = RoomForecast {
                    room_id: room.id,
                    name: room.display_name().to_owned(),
                    class,
                    area_m2: area,
                    perimeter_m: perimeter,
                    min_lighting_va: minimum_lighting_va(area),
                    min_outlets,
                    min_outlet_va: (0..min_outlets).map(|i| outlet_power_va(class, i)).sum(),
                    lights: 0,
                    lighting_va: 0.0,
                    outlets: 0,
                    outlet_va: 0.0,
                };
                for device in self.electrical_devices() {
                    if self.room_for_fixture(device).map(|r| r.id) != Some(room.id) {
                        continue;
                    }
                    let power = device.electrical.map_or(0.0, |data| data.power_va);
                    match device.kind.role() {
                        Some(DeviceRole::Lighting) => {
                            forecast.lights += 1;
                            forecast.lighting_va += power;
                        }
                        Some(DeviceRole::GeneralOutlet) => {
                            forecast.outlets += 1;
                            forecast.outlet_va += power;
                        }
                        _ => {}
                    }
                }
                forecast
            })
            .collect()
    }

    /// The load schedule: one row per circuit number, sized by NBR 5410.
    pub fn load_schedule(&self) -> LoadSchedule {
        let settings = self.electrical;
        let mut schedule = LoadSchedule::default();
        let mut numbers: Vec<u32> = self
            .electrical_devices()
            .filter_map(|device| device.electrical.and_then(|data| data.circuit))
            .collect();
        numbers.sort_unstable();
        numbers.dedup();
        for number in numbers {
            let devices: Vec<&Fixture> = self
                .electrical_devices()
                .filter(|device| device.electrical.and_then(|d| d.circuit) == Some(number))
                .collect();
            let mut lighting = 0;
            let mut general = 0;
            let mut specific = 0;
            let mut rooms: Vec<String> = Vec::new();
            let mut wet = false;
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
                needs_rcd: wet && kind != CircuitKind::Lighting,
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
        let mut wet_rooms: Vec<Option<u64>> = wet_outlets.iter().map(|(_, r)| *r).collect();
        wet_rooms.dedup();
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

    /// The straight sides of a room, each with the wall's inner normal.
    fn room_sides(&self, room: &Room) -> Vec<Side> {
        let Some(outline) = self.room_outline(room) else {
            return Vec::new();
        };
        let fallback = self.masonry.thickness();
        (0..outline.len())
            .filter_map(|i| {
                let (from, to) = (outline[i], outline[(i + 1) % outline.len()]);
                let along = to.minus(from).normalized()?;
                let middle = from.lerp(to, 0.5);
                let mut inward = Point::new(-along.y, along.x);
                if !geometry::polygon_contains_point(
                    &outline,
                    middle.translated(inward.scaled(0.05)),
                ) {
                    inward = inward.scaled(-1.0);
                }
                let wall_thickness = self
                    .walls
                    .iter()
                    .filter(|wall| room.wall_ids.contains(&wall.id))
                    .find(|wall| geometry::distance_to_segment(middle, wall.start, wall.end) < 0.02)
                    .map_or(fallback, |wall| wall.thickness());
                Some(Side {
                    from,
                    to,
                    inward,
                    wall_thickness,
                })
            })
            .collect()
    }

    /// Stretches along `side` taken by doors and windows, as distances from
    /// the side's start, widened by `clearance` on each end.
    fn side_openings(&self, side: &Side, clearance: f64) -> Vec<(f64, f64)> {
        self.openings
            .iter()
            .filter_map(|opening| {
                let wall = self.walls.iter().find(|wall| wall.id == opening.wall_id)?;
                let center = wall.start.lerp(wall.end, opening.t);
                if geometry::distance_to_segment(center, side.from, side.to) > 0.05 {
                    return None;
                }
                let s = side.from.distance(center);
                let half = opening.width() / 2.0 + clearance;
                Some((s - half, s + half))
            })
            .collect()
    }

    /// Places a wall device of `kind` on `side`, at distance `s` from its
    /// start, standing on the wall's face. Returns the new id.
    fn place_on_side(&mut self, kind: FixtureKind, side: &Side, s: f64) -> Option<u64> {
        let turns = side.quarter_turns()?;
        let (width, depth) = kind.default_size();
        let center = side
            .at(s)
            .translated(side.inward.scaled(side.wall_thickness / 2.0 + depth / 2.0));
        self.add_fixture(kind, center, width, depth, turns)
    }

    /// Adds what NBR 5410 asks for and is missing in each room: a ceiling
    /// light sized to the room, general-use outlets spread along the walls
    /// (clear of doors and windows), a switch beside the door, and a
    /// specific-use point at each shower. Existing devices count; nothing is
    /// removed or moved.
    pub fn generate_electrical_points(&mut self) -> GenerationReport {
        let mut report = GenerationReport::default();
        let forecasts = self.electrical_forecast();
        let rooms: Vec<Room> = self.rooms.clone();
        for (room, forecast) in rooms.iter().zip(forecasts) {
            let class = forecast.class;
            // Lighting.
            if forecast.lights == 0
                && let Some((anchor, _, _)) = self.room_label_anchor(room)
            {
                let (w, d) = FixtureKind::CeilingLight.default_size();
                if let Some(id) = self.add_fixture(FixtureKind::CeilingLight, anchor, w, d, 0) {
                    self.set_device_power(id, forecast.min_lighting_va);
                    report.lights += 1;
                }
            }
            let sides = self.room_sides(room);
            // Outlets, spread evenly over the usable length of the walls.
            let missing = forecast.min_outlets.saturating_sub(forecast.outlets);
            if missing > 0 {
                // Above counters and basins in wet rooms; low elsewhere.
                let kind = if class.is_wet() {
                    FixtureKind::OutletMid
                } else {
                    FixtureKind::OutletLow
                };
                let usable: Vec<(usize, f64)> = sides
                    .iter()
                    .enumerate()
                    .filter(|(_, side)| side.quarter_turns().is_some() && side.length() > 0.9)
                    .map(|(i, side)| (i, side.length()))
                    .collect();
                let total: f64 = usable.iter().map(|(_, length)| length).sum();
                let mut placed = 0;
                if total > 0.0 {
                    let step = total / missing as f64;
                    for n in 0..missing {
                        let mut target = step * (n as f64 + 0.5);
                        for (i, length) in &usable {
                            if target > *length {
                                target -= length;
                                continue;
                            }
                            let side = &sides[*i];
                            let blocked = self.side_openings(side, 0.15);
                            let free = |s: f64| {
                                s > 0.35
                                    && s < side.length() - 0.35
                                    && !blocked.iter().any(|(a, b)| s > *a && s < *b)
                            };
                            // Nearest free spot along this side.
                            let spot = (0..40)
                                .flat_map(|k| [f64::from(k) * 0.1, -f64::from(k) * 0.1])
                                .map(|delta| target + delta)
                                .find(|s| free(*s));
                            if let Some(s) = spot
                                && let Some(id) = self.place_on_side(kind, side, s)
                            {
                                let index = forecast.outlets + placed;
                                self.set_device_power(id, outlet_power_va(class, index));
                                placed += 1;
                            }
                            break;
                        }
                    }
                }
                report.outlets += placed;
            }
            // A switch on the room side of the first door, by the jamb.
            let has_switch = self.electrical_devices().any(|device| {
                device.kind.role() == Some(DeviceRole::Switch)
                    && self.room_for_fixture(device).map(|r| r.id) == Some(room.id)
            });
            if !has_switch {
                'sides: for side in &sides {
                    for opening in &self.openings.clone() {
                        if !matches!(opening.kind, OpeningKind::Door | OpeningKind::SaloonDoor) {
                            continue;
                        }
                        let Some(wall) = self.walls.iter().find(|w| w.id == opening.wall_id) else {
                            continue;
                        };
                        let center = wall.start.lerp(wall.end, opening.t);
                        if geometry::distance_to_segment(center, side.from, side.to) > 0.05 {
                            continue;
                        }
                        let s = side.from.distance(center);
                        let offset = opening.width() / 2.0 + 0.2;
                        for candidate in [s + offset, s - offset] {
                            let blocked = self.side_openings(side, 0.05);
                            let spot = side.at(candidate);
                            let crowded = self
                                .electrical_devices()
                                .any(|device| device.center.distance(spot) < 0.55);
                            if !crowded
                                && candidate > 0.15
                                && candidate < side.length() - 0.15
                                && !blocked
                                    .iter()
                                    .any(|(a, b)| candidate > *a && candidate < *b)
                                && self
                                    .place_on_side(FixtureKind::Switch, side, candidate)
                                    .is_some()
                            {
                                report.switches += 1;
                                break 'sides;
                            }
                        }
                    }
                }
            }
        }
        // An electric shower needs a specific-use point of its own.
        let showers: Vec<Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| fixture.kind == FixtureKind::Shower)
            .copied()
            .collect();
        for shower in showers {
            let served = self.electrical_devices().any(|device| {
                device.kind == FixtureKind::OutletSpecific
                    && device.center.distance(shower.center) < 1.0
            });
            if served {
                continue;
            }
            let (w, d) = FixtureKind::OutletSpecific.default_size();
            if let Some(id) = self.add_fixture(
                FixtureKind::OutletSpecific,
                shower.center,
                w,
                d,
                shower.quarter_turns,
            ) {
                if let Some(device) = self.fixtures.iter_mut().find(|f| f.id == id)
                    && let Some(data) = device.electrical.as_mut()
                {
                    data.power_va = 5500.0;
                    data.height_m = 2.20;
                }
                report.specific += 1;
            }
        }
        report
    }

    /// The tag of each device, where it goes (in front of the device, in the
    /// room) and what it says: the circuit, and the power when it is not the
    /// usual 100 VA of a general outlet ("C2 · 600 VA").
    pub fn electrical_tags(&self) -> Vec<(Point, String)> {
        self.electrical_devices()
            .filter_map(|device| {
                let data = device.electrical?;
                let role = device.kind.role()?;
                let circuit = data.circuit.map(|n| format!("C{n}"));
                let power = (data.power_va > 0.0).then(|| format!("{:.0} VA", data.power_va));
                let text = match role {
                    DeviceRole::Panel => "QD".to_owned(),
                    DeviceRole::Data => return None,
                    DeviceRole::Switch => circuit?,
                    DeviceRole::GeneralOutlet if (data.power_va - 100.0).abs() < 1e-6 => circuit?,
                    _ => [circuit, power]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · "),
                };
                if text.is_empty() {
                    return None;
                }
                // The front of a device faces away from its wall.
                let front = match device.quarter_turns % 4 {
                    0 => Point::new(0.0, 1.0),
                    1 => Point::new(-1.0, 0.0),
                    2 => Point::new(0.0, -1.0),
                    _ => Point::new(1.0, 0.0),
                };
                // Facing sideways, the text runs along the front: it starts
                // past the symbol instead of covering it.
                let sideways = front.x.abs() * tag_size(&text).0 / 2.0;
                let reach = if device.kind.mounts_on_wall() {
                    device.depth / 2.0 + 0.12 + sideways
                } else {
                    device.depth / 2.0 + 0.12
                };
                Some((device.center, front, reach, text))
            })
            .fold(
                Vec::new(),
                |mut tags: Vec<(Point, String)>, (center, front, reach, text)| {
                    // Push a tag further out while it would touch an earlier one.
                    let mut anchor = center.translated(front.scaled(reach));
                    for _ in 0..4 {
                        let size = tag_size(&text);
                        let clash = tags.iter().any(|(other, other_text)| {
                            let other_size = tag_size(other_text);
                            (anchor.x - other.x).abs() < (size.0 + other_size.0) / 2.0
                                && (anchor.y - other.y).abs() < (size.1 + other_size.1) / 2.0
                        });
                        if !clash {
                            break;
                        }
                        anchor = anchor.translated(front.scaled(0.2));
                    }
                    tags.push((anchor, text));
                    tags
                },
            )
    }

    fn set_device_power(&mut self, id: u64, power_va: f64) {
        if let Some(device) = self.fixtures.iter_mut().find(|fixture| fixture.id == id)
            && let Some(data) = device.electrical.as_mut()
        {
            data.power_va = power_va;
        }
    }

    /// Changes a device's circuit, power or height; `None` keeps the value.
    pub fn update_electrical(
        &mut self,
        id: u64,
        circuit: Option<Option<u32>>,
        power_va: Option<f64>,
        height_m: Option<f64>,
    ) -> Result<(), String> {
        let device = self
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.id == id && fixture.kind.is_electrical())
            .ok_or("Ponto elétrico não encontrado.")?;
        let data = device.electrical.get_or_insert_with(|| {
            device.kind.default_electrical().unwrap_or(ElectricalData {
                circuit: None,
                power_va: 0.0,
                height_m: 0.0,
            })
        });
        if let Some(power) = power_va {
            if !power.is_finite() || !(0.0..=50_000.0).contains(&power) {
                return Err("A potência deve ficar entre 0 e 50000 VA.".to_owned());
            }
            data.power_va = power;
        }
        if let Some(height) = height_m {
            if !height.is_finite() || !(0.0..=6.0).contains(&height) {
                return Err("A altura deve ficar entre 0 e 6 m.".to_owned());
            }
            data.height_m = height;
        }
        if let Some(circuit) = circuit {
            if circuit == Some(0) {
                return Err("Os circuitos são numerados a partir de 1.".to_owned());
            }
            data.circuit = circuit;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .find(|c| {
                c.rooms.contains(&"Cozinha".to_owned()) && c.kind == CircuitKind::GeneralOutlets
            })
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
                .find(|d| {
                    d.kind == kind && project.room_holding(d.center).map(|r| r.id) == Some(sala)
                })
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
}
