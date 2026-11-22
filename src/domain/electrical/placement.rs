//! Automatic NBR 5410 placement of lights, outlets, switches and showers.

use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::{self, Point},
    model::{OpeningKind, Project, Room},
};

use super::{DeviceRole, GenerationReport, outlet_power_va};

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
}
