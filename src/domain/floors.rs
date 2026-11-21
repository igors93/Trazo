//! Multi-storey building support: floor levels with elevation (nível),
//! ceiling height (pé-direito), slab thickness and visual underlay reference
//! from the storey directly below.

use serde::{Deserialize, Serialize};

use super::{
    circulation::CirculationElement,
    dimensions::DimensionEntity,
    drawing::Drawing,
    fixtures::Fixture,
    model::{Opening, Project, Room, Wall},
    notes::{AnnotationNote, format_elevation},
};

/// Snapshot of a floor's own plan geometry stored when that floor is not active.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorPlanData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<Wall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<Room>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<Opening>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<Fixture>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<Drawing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<DimensionEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<AnnotationNote>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub circulations: Vec<CirculationElement>,
    /// Conduits, beams and slabs name devices, columns and rooms of their
    /// own floor, so they travel with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conduits: Vec<super::conduits::Conduit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<super::structure::Beam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balance_beams: Vec<super::foundations::BalanceBeam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wall_foundations: Vec<super::wall_foundations::WallFoundation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roofs: Vec<super::roof::Roof>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slabs: Vec<super::structure::Slab>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloorLevel {
    pub id: u64,
    pub name: String,
    /// Finished floor elevation in metres relative to ground (`±0,00`).
    pub elevation_m: f64,
    /// Floor-to-ceiling height (pé-direito) in metres.
    #[serde(default = "default_ceiling_height")]
    pub ceiling_height_m: f64,
    /// Structural slab thickness in metres.
    #[serde(default = "default_slab_thickness")]
    pub slab_thickness_m: f64,
    /// Plan contents when this floor is inactive.
    #[serde(default)]
    pub data: FloorPlanData,
}

fn default_ceiling_height() -> f64 {
    2.80
}

fn default_slab_thickness() -> f64 {
    0.15
}

impl FloorLevel {
    pub fn label_with_elevation(&self) -> String {
        format!("{} ({})", self.name, format_elevation(self.elevation_m))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFloors {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<FloorLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_floor_id: Option<u64>,
    #[serde(default = "default_true")]
    pub show_underlay: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ProjectFloors {
    fn default() -> Self {
        Self {
            levels: Vec::new(),
            active_floor_id: None,
            show_underlay: true,
        }
    }
}

impl ProjectFloors {
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    pub fn sort_by_elevation(&mut self) {
        self.levels.sort_by(|a, b| {
            a.elevation_m
                .total_cmp(&b.elevation_m)
                .then(a.id.cmp(&b.id))
        });
    }
}

impl Project {
    /// Ensures at least the ground floor (`Térreo`) exists in `self.floors`.
    pub fn ensure_default_floor(&mut self) -> u64 {
        if let Some(active) = self.floors.active_floor_id
            && self.floors.levels.iter().any(|l| l.id == active)
        {
            return active;
        }
        if let Some(first) = self.floors.levels.first() {
            let id = first.id;
            self.floors.active_floor_id = Some(id);
            return id;
        }
        let id = 1;
        self.floors.levels.push(FloorLevel {
            id,
            name: "Térreo".to_owned(),
            elevation_m: 0.0,
            ceiling_height_m: self.wall_height.max(2.0),
            slab_thickness_m: default_slab_thickness(),
            data: FloorPlanData::default(),
        });
        self.floors.active_floor_id = Some(id);
        id
    }

    fn take_live_floor_data(&mut self) -> FloorPlanData {
        FloorPlanData {
            walls: std::mem::take(&mut self.walls),
            rooms: std::mem::take(&mut self.rooms),
            openings: std::mem::take(&mut self.openings),
            fixtures: std::mem::take(&mut self.fixtures),
            drawings: std::mem::take(&mut self.drawings),
            dimensions: std::mem::take(&mut self.dimensions),
            notes: std::mem::take(&mut self.notes),
            circulations: std::mem::take(&mut self.circulations),
            conduits: std::mem::take(&mut self.conduits),
            beams: std::mem::take(&mut self.structure.beams),
            balance_beams: std::mem::take(&mut self.structure.balance_beams),
            wall_foundations: std::mem::take(&mut self.structure.wall_foundations),
            roofs: std::mem::take(&mut self.structure.roofs),
            slabs: std::mem::take(&mut self.structure.slabs),
        }
    }

    fn install_live_floor_data(&mut self, data: FloorPlanData) {
        self.walls = data.walls;
        self.rooms = data.rooms;
        self.openings = data.openings;
        self.fixtures = data.fixtures;
        self.drawings = data.drawings;
        self.dimensions = data.dimensions;
        self.notes = data.notes;
        self.circulations = data.circulations;
        self.conduits = data.conduits;
        self.structure.beams = data.beams;
        self.structure.balance_beams = data.balance_beams;
        self.structure.wall_foundations = data.wall_foundations;
        self.structure.roofs = data.roofs;
        self.structure.slabs = data.slabs;
    }

    pub fn active_floor(&self) -> Option<&FloorLevel> {
        let id = self.floors.active_floor_id?;
        self.floors.levels.iter().find(|l| l.id == id)
    }

    /// Returns the floor directly below the active floor when underlay reference is enabled.
    pub fn underlay_floor(&self) -> Option<&FloorLevel> {
        if !self.floors.show_underlay {
            return None;
        }
        let active = self.active_floor()?;
        self.floors
            .levels
            .iter()
            .filter(|l| l.id != active.id && l.elevation_m < active.elevation_m - 1e-4)
            .max_by(|a, b| a.elevation_m.total_cmp(&b.elevation_m))
    }

    /// Switches the active editing storey, preserving the previous storey's geometry.
    pub fn switch_floor(&mut self, target_id: u64) -> Result<(), String> {
        let active_id = self.ensure_default_floor();
        if active_id == target_id {
            return Ok(());
        }
        if !self.floors.levels.iter().any(|l| l.id == target_id) {
            return Err("Pavimento não encontrado.".to_owned());
        }

        let live_data = self.take_live_floor_data();
        let live_height = self.wall_height;
        if let Some(current) = self.floors.levels.iter_mut().find(|l| l.id == active_id) {
            current.data = live_data;
            current.ceiling_height_m = live_height;
        }

        let target = self
            .floors
            .levels
            .iter_mut()
            .find(|l| l.id == target_id)
            .unwrap();
        let target_data = std::mem::take(&mut target.data);
        let target_height = target.ceiling_height_m.max(2.0);
        self.install_live_floor_data(target_data);
        self.wall_height = target_height;
        self.floors.active_floor_id = Some(target_id);
        Ok(())
    }

    /// Adds a new storey above the highest floor or below the lowest floor and activates it.
    pub fn add_floor(&mut self, name: &str, above: bool, copy_walls_from_current: bool) -> u64 {
        let _active_id = self.ensure_default_floor();
        self.floors.sort_by_elevation();

        let (elevation_m, ceiling_height_m) = if above {
            let top = self.floors.levels.last().unwrap();
            let elev = ((top.elevation_m + top.ceiling_height_m + top.slab_thickness_m) * 100.0)
                .round()
                / 100.0;
            (elev, top.ceiling_height_m)
        } else {
            let bottom = self.floors.levels.first().unwrap();
            let h = bottom.ceiling_height_m;
            let elev = ((bottom.elevation_m - h - bottom.slab_thickness_m) * 100.0).round() / 100.0;
            (elev, h)
        };

        let copied_walls = if copy_walls_from_current {
            self.walls.clone()
        } else {
            Vec::new()
        };

        let next_id = self.floors.levels.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        let clean_name = if name.trim().is_empty() {
            if above {
                let upper_count = self
                    .floors
                    .levels
                    .iter()
                    .filter(|l| l.elevation_m > 0.01)
                    .count()
                    + 1;
                format!("{upper_count}º Pavimento")
            } else {
                let sub_count = self
                    .floors
                    .levels
                    .iter()
                    .filter(|l| l.elevation_m < -0.01)
                    .count()
                    + 1;
                if sub_count == 1 {
                    "Subsolo".to_owned()
                } else {
                    format!("{sub_count}º Subsolo")
                }
            }
        } else {
            name.trim().to_owned()
        };

        self.floors.levels.push(FloorLevel {
            id: next_id,
            name: clean_name,
            elevation_m,
            ceiling_height_m,
            slab_thickness_m: default_slab_thickness(),
            data: FloorPlanData {
                walls: copied_walls,
                ..Default::default()
            },
        });
        self.floors.sort_by_elevation();
        let _ = self.switch_floor(next_id);
        next_id
    }

    pub fn remove_floor(&mut self, id: u64) -> Result<(), String> {
        if self.floors.levels.len() <= 1 {
            return Err("O projeto precisa manter pelo menos um pavimento.".to_owned());
        }
        if !self.floors.levels.iter().any(|l| l.id == id) {
            return Err("Pavimento não encontrado.".to_owned());
        }
        if self.floors.active_floor_id == Some(id) {
            let fallback = self
                .floors
                .levels
                .iter()
                .find(|l| l.id != id)
                .map(|l| l.id)
                .unwrap();
            self.switch_floor(fallback)?;
        }
        self.floors.levels.retain(|l| l.id != id);
        Ok(())
    }

    /// Updates metadata for a floor and keeps `self.wall_height` synced if active.
    pub fn update_floor_metadata(
        &mut self,
        id: u64,
        name: &str,
        elevation_m: f64,
        ceiling_height_m: f64,
        slab_thickness_m: f64,
    ) -> Result<(), String> {
        let ch = ceiling_height_m.clamp(2.0, 12.0);
        let st = slab_thickness_m.clamp(0.08, 0.80);
        let level = self
            .floors
            .levels
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or_else(|| "Pavimento não encontrado.".to_owned())?;
        if !name.trim().is_empty() {
            level.name = name.trim().to_owned();
        }
        level.elevation_m = (elevation_m * 100.0).round() / 100.0;
        level.ceiling_height_m = ch;
        level.slab_thickness_m = st;
        if self.floors.active_floor_id == Some(id) {
            self.wall_height = ch;
        }
        self.floors.sort_by_elevation();
        Ok(())
    }

    /// Total constructed area summed across all floors of the building.
    pub fn total_multistorey_constructed_area(&self) -> f64 {
        let mut total = self.area_summary().constructed;
        let active_id = self.floors.active_floor_id;
        for level in &self.floors.levels {
            if Some(level.id) == active_id {
                continue;
            }
            let wall_area: f64 = level
                .data
                .walls
                .iter()
                .map(|w| w.length() * w.thickness())
                .sum();
            let mut temp = Project::default();
            temp.walls = level.data.walls.clone();
            temp.rooms = level.data.rooms.clone();
            let rooms_usable: f64 = temp
                .rooms
                .iter()
                .filter_map(|r| temp.room_usable_area(r))
                .sum();
            total += rooms_usable + wall_area;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn multistorey_floors_switch_cleanly_preserve_geometry_and_expose_underlay() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 4.0), "Sala Térreo")
            .unwrap();
        let ground_id = project.ensure_default_floor();
        assert_eq!(project.floors.levels[0].name, "Térreo");
        assert!((project.floors.levels[0].elevation_m - 0.0).abs() < 1e-6);

        // Add upper floor (1º Pavimento) at +2.95 m (2.80 ceiling + 0.15 slab) copying walls.
        let upper_id = project.add_floor("1º Pavimento", true, true);
        assert_eq!(project.floors.active_floor_id, Some(upper_id));
        assert!((project.active_floor().unwrap().elevation_m - 2.95).abs() < 1e-6);
        assert_eq!(project.walls.len(), 4); // copied from ground floor
        assert!(project.rooms.is_empty()); // fresh rooms on upper floor

        // Underlay returns ground floor with its original room and walls!
        let underlay = project
            .underlay_floor()
            .expect("should have underlay below");
        assert_eq!(underlay.id, ground_id);
        assert_eq!(underlay.data.rooms.len(), 1);
        assert_eq!(underlay.data.walls.len(), 4);

        // Add a room on upper floor and change its ceiling height to 2.60 m.
        project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 4.0), "Suíte")
            .unwrap();
        project
            .update_floor_metadata(upper_id, "1º Pavimento", 2.95, 2.60, 0.15)
            .unwrap();
        assert!((project.wall_height - 2.60).abs() < 1e-6);

        // Total constructed area spans both floors (~2x single floor).
        assert!(project.total_multistorey_constructed_area() > 45.0);

        // Switching back to ground floor restores Térreo room and 2.80 m ceiling height.
        project.switch_floor(ground_id).unwrap();
        assert_eq!(project.rooms[0].name, "Sala Térreo");
        assert!((project.wall_height - 2.80).abs() < 1e-6);
        assert!(project.underlay_floor().is_none());

        // Add basement below ground floor.
        let sub_id = project.add_floor("", false, false);
        assert!(project.active_floor().unwrap().elevation_m < -2.0);
        assert_eq!(project.active_floor().unwrap().name, "Subsolo");

        // JSON round-trip preserves all 3 floors cleanly.
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.floors.levels.len(), 3);
        assert_eq!(loaded.floors.active_floor_id, Some(sub_id));

        assert!(project.remove_floor(sub_id).is_ok());
        assert_eq!(project.floors.levels.len(), 2);
    }
}
