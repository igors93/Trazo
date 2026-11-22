//! ABNT NBR 5410 room classification and lighting/outlet load forecasting.

use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::{self, Point},
    model::{Project, Room},
};

use super::DeviceRole;

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
        } else if has(&[
            "varanda",
            "sacada",
            "terraço",
            "terraco",
            "alpendre",
            "garagem",
            "quintal",
            "jardim",
            "piscina",
            "churrasqueira",
            "externa",
            "externo",
        ]) {
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
        } else if self.outlet_va + 1e-6 < self.min_outlet_va {
            missing.push(format!(
                "tomadas {:.0} VA (mín. {:.0} VA)",
                self.outlet_va, self.min_outlet_va
            ));
        }
        missing
    }
}

impl Project {
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

    pub(super) fn room_class(room: &Room) -> RoomClass {
        RoomClass::from_name(room.display_name())
    }

    pub(super) fn room_perimeter(&self, room: &Room) -> f64 {
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
}
