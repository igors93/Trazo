use serde::{Deserialize, Serialize};

use super::drawing::Drawing;
use super::fixtures::{self, Fixture, FixtureKind};
use super::geometry::{
    JOIN_TOLERANCE, Point, distance_to_segment, nearest_point_on_segment, round_to_resolution,
};

mod drawings;
mod rooms;
use super::masonry::{self, Masonry};

/// Versão mais recente do formato `.trazo` gravado em disco.
pub const CURRENT_PROJECT_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,

    pub name: String,

    pub walls: Vec<Wall>,

    pub rooms: Vec<Room>,

    pub openings: Vec<Opening>,

    #[serde(default)]
    pub terrain: Option<Terrain>,

    /// Counters, sinks, stoves, bathroom fittings, tables...
    #[serde(default)]
    pub fixtures: Vec<Fixture>,

    /// The brick or block new walls are built with.
    #[serde(default)]
    pub masonry: Masonry,

    /// Height of the walls (pé-direito), for quantity estimates.
    #[serde(default = "default_wall_height")]
    pub wall_height: f64,

    /// Lines, arcs, circles and other plain drawing entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<Drawing>,

    next_id: u64,
}

fn default_wall_height() -> f64 {
    2.8
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: CURRENT_PROJECT_VERSION,

            name: "Minha planta".to_owned(),

            walls: Vec::new(),

            rooms: Vec::new(),

            openings: Vec::new(),

            terrain: None,

            fixtures: Vec::new(),

            masonry: Masonry::default(),

            wall_height: default_wall_height(),

            drawings: Vec::new(),

            next_id: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub id: u64,

    pub start: Point,

    pub end: Point,

    /// What it is built with; walls from older files have none and keep
    /// the old 12 cm thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub masonry: Option<Masonry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Terrain {
    pub id: u64,

    pub min: Point,

    pub max: Point,
}

impl Terrain {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

impl Wall {
    /// The finished thickness: block plus plaster.
    pub fn thickness(&self) -> f64 {
        self.masonry
            .as_ref()
            .map(Masonry::thickness)
            .unwrap_or(masonry::LEGACY_THICKNESS)
    }

    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]

pub enum RoomKind {
    Bedroom,

    LivingRoom,

    Kitchen,

    Bathroom,

    DiningRoom,

    Office,

    Other,
}

impl RoomKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bedroom => "Quarto",

            Self::LivingRoom => "Sala",

            Self::Kitchen => "Cozinha",

            Self::Bathroom => "Banheiro",

            Self::DiningRoom => "Sala de jantar",

            Self::Office => "Escritório",

            Self::Other => "Outro cômodo",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: u64,

    #[serde(default)]
    pub name: String,

    // Reads room types from older project files without assigning a preset to new rooms.
    #[serde(default, rename = "kind", skip_serializing_if = "Option::is_none")]
    pub legacy_kind: Option<RoomKind>,

    pub wall_ids: Vec<u64>,
}

impl Room {
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            self.legacy_kind.map(RoomKind::label).unwrap_or("Cômodo")
        } else {
            &self.name
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]

pub enum OpeningKind {
    Door,

    Window,

    /// Double-acting café ("saloon") doors: two half leaves swinging both ways.
    SaloonDoor,

    /// An open span with no leaf, such as the open side of a covered area.
    Gap,
}

impl OpeningKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Door => "Porta",

            Self::Window => "Janela",

            Self::SaloonDoor => "Porta vai-e-vem",

            Self::Gap => "Vão livre",
        }
    }

    /// Usual height of the opening; `None` for open spans, which reach the
    /// top of the wall.
    pub fn default_height(self) -> Option<f64> {
        match self {
            Self::Door | Self::SaloonDoor => Some(2.10),
            Self::Window => Some(1.00),
            Self::Gap => None,
        }
    }

    pub fn is_door(self) -> bool {
        matches!(self, Self::Door | Self::SaloonDoor)
    }

    pub fn default_width(self) -> f64 {
        match self {
            Self::Door => 0.9,

            Self::Window => 1.2,

            Self::SaloonDoor => 0.8,

            Self::Gap => 2.0,
        }
    }

    pub fn width(self) -> f64 {
        self.default_width()
    }

    pub fn min_width(self) -> f64 {
        match self {
            Self::Door => 0.60,

            Self::Window => 0.40,

            Self::SaloonDoor => 0.60,

            Self::Gap => 0.50,
        }
    }

    pub fn max_width(self) -> f64 {
        match self {
            Self::Door => 2.00,

            Self::Window => 4.00,

            Self::SaloonDoor => 1.80,

            Self::Gap => 30.00,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub id: u64,
    pub wall_id: u64,
    pub kind: OpeningKind,
    pub t: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// The leaf swings to the other side of the wall.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_side: bool,
    /// The hinge is on the jamb towards the wall's end instead of its start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_hinge: bool,
    /// Height of the opening; the kind's usual one when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Sill height (peitoril) of a window; 1,10 m when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sill: Option<f64>,
}

impl Opening {
    pub fn width(&self) -> f64 {
        self.width.unwrap_or_else(|| self.kind.width())
    }

    /// Height of the opening; `None` for open spans up to the ceiling.
    pub fn height(&self) -> Option<f64> {
        self.height.or(self.kind.default_height())
    }

    /// Height of the window sill above the floor; 0 for doors.
    pub fn sill(&self) -> f64 {
        match self.kind {
            OpeningKind::Window => self.sill.unwrap_or(1.10),
            _ => 0.0,
        }
    }
}

/// Where segment `p1`-`p2` lies along `a`-`b` (distances from `a`), when it
/// runs on the same line (within 2 cm) and overlaps it by more than 4 cm.
fn is_collinear_and_overlapping(a: Point, b: Point, p1: Point, p2: Point) -> Option<(f64, f64)> {
    let len = a.distance(b);
    let u = b.minus(a).normalized()?;
    let n = Point::new(-u.y, u.x);
    if p1.minus(a).dot(n).abs() > 0.02 || p2.minus(a).dot(n).abs() > 0.02 {
        return None;
    }
    let t1 = p1.minus(a).dot(u);
    let t2 = p2.minus(a).dot(u);
    let (w_min, w_max) = (t1.min(t2), t1.max(t2));
    (w_max.min(len) - w_min.max(0.0) > 0.04).then_some((w_min, w_max))
}

impl Project {
    pub fn content_bounds(&self) -> Option<(Point, Point)> {
        if self.walls.is_empty() && self.fixtures.is_empty() {
            return self.drawings_bounds();
        }

        let mut min = Point::new(f64::INFINITY, f64::INFINITY);

        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

        for wall in &self.walls {
            min.x = min.x.min(wall.start.x.min(wall.end.x));

            min.y = min.y.min(wall.start.y.min(wall.end.y));

            max.x = max.x.max(wall.start.x.max(wall.end.x));

            max.y = max.y.max(wall.start.y.max(wall.end.y));
        }

        for fixture in &self.fixtures {
            let (low, high) = fixture.bounds();
            min = Point::new(min.x.min(low.x), min.y.min(low.y));
            max = Point::new(max.x.max(high.x), max.y.max(high.y));
        }

        if let Some((low, high)) = self.drawings_bounds() {
            min = Point::new(min.x.min(low.x), min.y.min(low.y));
            max = Point::new(max.x.max(high.x), max.y.max(high.y));
        }

        Some((min, max))
    }

    pub fn content_fits_terrain(&self) -> bool {
        let (Some(terrain), Some((min, max))) = (&self.terrain, self.content_bounds()) else {
            return false;
        };

        max.x - min.x <= terrain.width() + 0.001 && max.y - min.y <= terrain.height() + 0.001
    }

    pub fn content_inside_terrain(&self) -> bool {
        let (Some(terrain), Some((min, max))) = (&self.terrain, self.content_bounds()) else {
            return true;
        };

        min.x >= terrain.min.x - 0.001
            && min.y >= terrain.min.y - 0.001
            && max.x <= terrain.max.x + 0.001
            && max.y <= terrain.max.y + 0.001
    }

    pub fn center_content_on_terrain(&mut self) -> bool {
        let (Some(terrain), Some((min, max))) = (&self.terrain, self.content_bounds()) else {
            return false;
        };

        if !self.content_fits_terrain() {
            return false;
        }

        let delta = Point::new(
            (terrain.min.x + terrain.max.x - min.x - max.x) / 2.0,
            (terrain.min.y + terrain.max.y - min.y - max.y) / 2.0,
        );

        for wall in &mut self.walls {
            wall.start = wall.start.translated(delta);

            wall.end = wall.end.translated(delta);
        }

        for fixture in &mut self.fixtures {
            fixture.center = fixture.center.translated(delta);
        }
        self.translate_drawings(delta);

        true
    }

    pub fn set_terrain(&mut self, first: Point, second: Point) -> Option<u64> {
        let min = Point::new(first.x.min(second.x), first.y.min(second.y));

        let max = Point::new(first.x.max(second.x), first.y.max(second.y));

        if max.x - min.x < 0.25 || max.y - min.y < 0.25 {
            return None;
        }

        let id = self
            .terrain
            .as_ref()
            .map(|terrain| terrain.id)
            .unwrap_or_else(|| self.id());

        self.terrain = Some(Terrain { id, min, max });

        Some(id)
    }

    pub fn resize_terrain(&mut self, width: f64, height: f64) -> bool {
        if !width.is_finite() || !height.is_finite() || width < 0.25 || height < 0.25 {
            return false;
        }

        let Some(terrain) = &mut self.terrain else {
            return false;
        };

        terrain.max = Point::new(terrain.min.x + width, terrain.min.y + height);

        true
    }

    pub fn move_terrain(&mut self, id: u64, delta: Point) {
        if let Some(terrain) = &mut self.terrain
            && terrain.id == id
        {
            terrain.min = terrain.min.translated(delta);

            terrain.max = terrain.max.translated(delta);

            for wall in &mut self.walls {
                wall.start = wall.start.translated(delta);

                wall.end = wall.end.translated(delta);
            }

            for fixture in &mut self.fixtures {
                fixture.center = fixture.center.translated(delta);
            }
            self.translate_drawings(delta);
        }
    }

    /// Builds wall `id` with `material`.
    pub fn set_wall_masonry(&mut self, id: u64, material: &Masonry) -> bool {
        let Some(wall) = self.walls.iter_mut().find(|wall| wall.id == id) else {
            return false;
        };
        wall.masonry = Some(material.clone());
        true
    }

    /// Builds every wall with `material`.
    pub fn set_all_walls_masonry(&mut self, material: &Masonry) {
        for wall in &mut self.walls {
            wall.masonry = Some(material.clone());
        }
    }

    /// Wall face area (lengths × wall height, less door and window areas)
    /// and the blocks it takes, each wall with its own material.
    pub fn masonry_estimate(&self) -> (f64, f64) {
        let mut area = 0.0;
        let mut blocks = 0.0;
        for wall in &self.walls {
            let material = wall.masonry.as_ref().unwrap_or(&self.masonry);
            let openings: f64 = self
                .openings
                .iter()
                .filter(|opening| opening.wall_id == wall.id)
                .map(|opening| {
                    opening.width()
                        * opening
                            .height()
                            .unwrap_or(self.wall_height)
                            .min(self.wall_height)
                })
                .sum();
            let face = (wall.length() * self.wall_height - openings).max(0.0);
            area += face;
            blocks += face * material.blocks_per_square_metre();
        }
        (area, blocks)
    }

    /// Whether `fixture` lies inside the terrain (or there is none).
    fn fixture_fits(&self, fixture: &Fixture) -> bool {
        let (low, high) = fixture.bounds();
        self.terrain
            .as_ref()
            .is_none_or(|terrain| terrain.contains(low) && terrain.contains(high))
    }

    /// Places a fixture centred at `center`; it must fit in the terrain.
    pub fn add_fixture(
        &mut self,
        kind: FixtureKind,
        center: Point,
        width: f64,
        depth: f64,
        quarter_turns: u8,
    ) -> Option<u64> {
        if !fixtures::valid_size(width, depth) || !center.x.is_finite() || !center.y.is_finite() {
            return None;
        }
        let fixture = Fixture {
            id: 0,
            kind,
            center,
            width,
            depth,
            quarter_turns: quarter_turns % 4,
        };
        if !self.fixture_fits(&fixture) {
            return None;
        }
        let id = self.id();
        self.fixtures.push(Fixture { id, ..fixture });
        Some(id)
    }

    /// Changes a fixture's position, size or rotation all at once; nothing
    /// changes when the result would not fit.
    pub fn update_fixture(
        &mut self,
        id: u64,
        center: Option<Point>,
        size: Option<(f64, f64)>,
        quarter_turns: Option<u8>,
    ) -> Result<(), String> {
        let index = self
            .fixtures
            .iter()
            .position(|fixture| fixture.id == id)
            .ok_or("Objeto não encontrado.")?;
        let mut updated = self.fixtures[index];
        if let Some(center) = center {
            updated.center = center;
        }
        if let Some((width, depth)) = size {
            if !fixtures::valid_size(width, depth) {
                return Err(format!(
                    "Largura e profundidade devem estar entre {:.2} e {:.2} m.",
                    fixtures::MIN_SIZE,
                    fixtures::MAX_SIZE
                )
                .replace('.', ","));
            }
            updated.width = width;
            updated.depth = depth;
        }
        if let Some(turns) = quarter_turns {
            updated.quarter_turns = turns % 4;
        }
        if !updated.center.x.is_finite() || !updated.center.y.is_finite() {
            return Err("Posição inválida.".to_owned());
        }
        if !self.fixture_fits(&updated) {
            return Err("O objeto precisa ficar dentro do terreno.".to_owned());
        }
        self.fixtures[index] = updated;
        Ok(())
    }

    /// Moves a fixture, stopping at the terrain's edge like walls do.
    pub fn move_fixture(&mut self, id: u64, delta: Point) {
        let Some(index) = self.fixtures.iter().position(|fixture| fixture.id == id) else {
            return;
        };
        let (low, high) = self.fixtures[index].bounds();
        let delta = self.clamp_delta_to_terrain(low, high, delta);
        self.fixtures[index].center = self.fixtures[index].center.translated(delta);
    }

    /// Turns a fixture a quarter turn clockwise about its centre.
    pub fn rotate_fixture(&mut self, id: u64) -> Result<(), String> {
        let turns = self
            .fixtures
            .iter()
            .find(|fixture| fixture.id == id)
            .map(|fixture| fixture.quarter_turns + 1)
            .ok_or("Objeto não encontrado.")?;
        self.update_fixture(id, None, None, Some(turns))
            .map_err(|_| "Girado, o objeto sairia do terreno.".to_owned())
    }

    pub fn delete_fixture(&mut self, id: u64) -> bool {
        let before = self.fixtures.len();
        self.fixtures.retain(|fixture| fixture.id != id);
        self.fixtures.len() < before
    }

    pub fn fixture_at(&self, point: Point) -> Option<u64> {
        // The last placed is drawn on top, so it is picked first.
        self.fixtures
            .iter()
            .rev()
            .find(|fixture| fixture.contains(point))
            .map(|fixture| fixture.id)
    }

    pub fn delete_terrain(&mut self, id: u64) -> bool {
        if self
            .terrain
            .as_ref()
            .is_some_and(|terrain| terrain.id == id)
        {
            self.terrain = None;

            true
        } else {
            false
        }
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;

        self.next_id += 1;

        id
    }

    pub fn add_wall(&mut self, start: Point, end: Point) -> Option<u64> {
        if start.distance(end) < 0.24 {
            return None;
        }

        if self
            .terrain
            .as_ref()
            .is_some_and(|terrain| !terrain.contains(start) || !terrain.contains(end))
        {
            return None;
        }

        let id = self.id();

        self.walls.push(Wall {
            id,
            start,
            end,
            masonry: Some(self.masonry.clone()),
        });

        Some(id)
    }

    pub fn split_wall_at(&mut self, wall_id: u64, split_point: Point) -> Option<u64> {
        let wall_idx = self.walls.iter().position(|w| w.id == wall_id)?;

        let old_start = self.walls[wall_idx].start;
        let old_end = self.walls[wall_idx].end;
        let total_len = old_start.distance(old_end);
        let split_dist = old_start.distance(split_point);

        if split_dist < 0.15 || (total_len - split_dist) < 0.15 {
            return None;
        }

        // Verify that the split point does not cut through any opening on this wall
        for opening in &self.openings {
            if opening.wall_id == wall_id {
                let center_dist = opening.t * total_len;
                let half_w = opening.width() / 2.0;
                let op_start = center_dist - half_w;
                let op_end = center_dist + half_w;
                // If split point falls within the opening's footprint (with 0.04m safety margin), reject split!
                if split_dist > (op_start - 0.04) && split_dist < (op_end + 0.04) {
                    return None;
                }
            }
        }

        self.walls[wall_idx].end = split_point;

        let new_id = self.id();
        let built_with = self.walls[wall_idx].masonry.clone();

        self.walls.push(Wall {
            id: new_id,
            start: split_point,
            end: old_end,
            masonry: built_with,
        });

        for room in &mut self.rooms {
            if let Some(pos) = room.wall_ids.iter().position(|&id| id == wall_id) {
                room.wall_ids.insert(pos + 1, new_id);
            }
        }

        let seg2_len = total_len - split_dist;
        for opening in &mut self.openings {
            if opening.wall_id == wall_id {
                let center_dist = opening.t * total_len;
                if center_dist >= split_dist {
                    // Belongs to the second segment
                    opening.wall_id = new_id;
                    let rel_center = center_dist - split_dist;
                    let margin_t = (opening.width() / 2.0 + 0.04) / seg2_len;
                    opening.t = (rel_center / seg2_len).clamp(margin_t, 1.0 - margin_t);
                } else {
                    // Belongs to the first segment
                    let margin_t = (opening.width() / 2.0 + 0.04) / split_dist;
                    opening.t = (center_dist / split_dist).clamp(margin_t, 1.0 - margin_t);
                }
            }
        }

        Some(new_id)
    }

    fn find_or_create_edge_walls(&mut self, start: Point, end: Point) -> Option<Vec<u64>> {
        let len = start.distance(end);

        if len < 0.24 {
            return None;
        }

        let mut split_occurred = true;

        while split_occurred {
            split_occurred = false;

            for i in 0..self.walls.len() {
                let wall = &self.walls[i];

                let wall_id = wall.id;

                let (w_start, w_end) = (wall.start, wall.end);

                if let Some((w_min, w_max)) =
                    is_collinear_and_overlapping(start, end, w_start, w_end)
                {
                    if w_min < -0.05 && w_max > 0.05 && self.split_wall_at(wall_id, start).is_some()
                    {
                        split_occurred = true;

                        break;
                    }

                    if w_min < len - 0.05
                        && w_max > len + 0.05
                        && self.split_wall_at(wall_id, end).is_some()
                    {
                        split_occurred = true;

                        break;
                    }
                }
            }
        }

        let dir = end.minus(start).scaled(1.0 / len);

        let mut sub_walls: Vec<(f64, f64, u64)> = Vec::new();

        for wall in &self.walls {
            if let Some((w_min, w_max)) =
                is_collinear_and_overlapping(start, end, wall.start, wall.end)
                && w_min >= -0.05
                && w_max <= len + 0.05
            {
                sub_walls.push((w_min.max(0.0), w_max.min(len), wall.id));
            }
        }

        sub_walls.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut result_ids = Vec::new();

        let mut cursor = 0.0;

        for (w_start_t, w_end_t, wall_id) in sub_walls {
            if w_start_t > cursor + 0.05 {
                let gap_p1 = Point::new(start.x + cursor * dir.x, start.y + cursor * dir.y);

                let gap_p2 = Point::new(start.x + w_start_t * dir.x, start.y + w_start_t * dir.y);

                if let Some(new_id) = self.add_wall(gap_p1, gap_p2) {
                    result_ids.push(new_id);
                }
            }

            if !result_ids.contains(&wall_id) {
                result_ids.push(wall_id);
            }

            cursor = cursor.max(w_end_t);
        }

        if cursor < len - 0.05 {
            let gap_p1 = Point::new(start.x + cursor * dir.x, start.y + cursor * dir.y);

            let gap_p2 = end;

            if let Some(new_id) = self.add_wall(gap_p1, gap_p2) {
                result_ids.push(new_id);
            }
        }

        if result_ids.is_empty() {
            let id = self.add_wall(start, end)?;

            result_ids.push(id);
        }

        Some(result_ids)
    }

    pub fn deduplicate_walls(&mut self) {
        let mut i = 0;

        while i < self.walls.len() {
            let mut j = i + 1;

            while j < self.walls.len() {
                let w1 = &self.walls[i];

                let w2 = &self.walls[j];

                let same_dir = w1.start.distance(w2.start) < 0.02 && w1.end.distance(w2.end) < 0.02;
                let opposite_dir =
                    w1.start.distance(w2.end) < 0.02 && w1.end.distance(w2.start) < 0.02;
                let duplicate = same_dir || opposite_dir;

                if duplicate {
                    let keep_id = w1.id;
                    let remove_id = w2.id;

                    for room in &mut self.rooms {
                        for wid in &mut room.wall_ids {
                            if *wid == remove_id {
                                *wid = keep_id;
                            }
                        }
                    }

                    for opening in &mut self.openings {
                        if opening.wall_id == remove_id {
                            opening.wall_id = keep_id;
                            if opposite_dir {
                                opening.t = (1.0 - opening.t).clamp(0.01, 0.99);
                            }
                        }
                    }

                    // Remove overlapping or exact duplicate openings on keep_id
                    let mut op_idx = 0;
                    while op_idx < self.openings.len() {
                        if self.openings[op_idx].wall_id == keep_id {
                            let op1_kind = self.openings[op_idx].kind;
                            let op1_t = self.openings[op_idx].t;
                            let is_dup = self.openings.iter().enumerate().any(|(k, op2)| {
                                k != op_idx
                                    && op2.wall_id == keep_id
                                    && op2.kind == op1_kind
                                    && (op2.t - op1_t).abs() < 0.04
                            });
                            if is_dup {
                                self.openings.remove(op_idx);
                                continue;
                            }
                        }
                        op_idx += 1;
                    }

                    self.walls.remove(j);
                } else {
                    j += 1;
                }
            }

            i += 1;
        }
    }

    pub fn validate_and_repair(&mut self) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();

        // 1. Validate coordinates
        if self
            .walls
            .iter()
            .any(|wall| !wall.start.is_finite() || !wall.end.is_finite())
        {
            return Err("O projeto contém coordenadas inválidas em paredes.".to_owned());
        }
        self.normalize_precision();

        if let Some(terrain) = &mut self.terrain {
            if !terrain.min.is_finite() || !terrain.max.is_finite() {
                return Err("O projeto contém coordenadas inválidas no terreno.".to_owned());
            }
            if terrain.min.x >= terrain.max.x || terrain.min.y >= terrain.max.y {
                let x0 = terrain.min.x.min(terrain.max.x);
                let x1 = terrain.min.x.max(terrain.max.x);
                let y0 = terrain.min.y.min(terrain.max.y);
                let y1 = terrain.min.y.max(terrain.max.y);
                if (x1 - x0) < 0.25 || (y1 - y0) < 0.25 {
                    warnings
                        .push("Terreno com medidas nulas ou inválidas foi removido.".to_owned());
                    self.terrain = None;
                } else {
                    terrain.min = Point::new(x0, y0);
                    terrain.max = Point::new(x1, y1);
                    warnings.push("Limites do terreno foram normalizados.".to_owned());
                }
            }
        }

        // 2. Referential integrity
        let existing_walls: std::collections::HashSet<u64> =
            self.walls.iter().map(|w| w.id).collect();

        let orig_openings = self.openings.len();
        self.openings
            .retain(|op| existing_walls.contains(&op.wall_id) && op.t.is_finite());
        if self.openings.len() < orig_openings {
            warnings.push(format!(
                "{} aberturas com paredes inexistentes foram removidas.",
                orig_openings - self.openings.len()
            ));
        }

        for op in &mut self.openings {
            op.t = op.t.clamp(0.01, 0.99);
            if let Some(w) = op.width
                && (!w.is_finite() || w <= 0.0)
            {
                op.width = None;
            }
        }

        let orig_rooms = self.rooms.len();
        self.rooms.retain_mut(|room| {
            room.wall_ids.retain(|id| existing_walls.contains(id));
            room.wall_ids.len() >= 3
        });
        if self.rooms.len() < orig_rooms {
            warnings.push(format!(
                "{} cômodos com paredes ausentes foram removidos.",
                orig_rooms - self.rooms.len()
            ));
        }

        // 3. Ensure next_id validity
        let mut max_id = 0u64;
        for w in &self.walls {
            max_id = max_id.max(w.id);
        }
        for r in &self.rooms {
            max_id = max_id.max(r.id);
        }
        for o in &self.openings {
            max_id = max_id.max(o.id);
        }
        if let Some(t) = &self.terrain {
            max_id = max_id.max(t.id);
        }
        let orig_fixtures = self.fixtures.len();
        self.fixtures.retain(|fixture| {
            fixture.center.x.is_finite()
                && fixture.center.y.is_finite()
                && fixtures::valid_size(fixture.width, fixture.depth)
        });
        if self.fixtures.len() < orig_fixtures {
            warnings.push(format!(
                "{} objetos com medidas inválidas foram removidos.",
                orig_fixtures - self.fixtures.len()
            ));
        }
        for fixture in &mut self.fixtures {
            fixture.quarter_turns %= 4;
            max_id = max_id.max(fixture.id);
        }
        let (dropped, highest) = self.repair_drawings();
        if dropped > 0 {
            warnings.push(format!("{dropped} desenhos inválidos foram removidos."));
        }
        max_id = max_id.max(highest);
        if self.next_id <= max_id {
            self.next_id = max_id + 1;
        }

        // 4. Deduplicate shared walls
        let orig_walls = self.walls.len();
        self.deduplicate_walls();
        if self.walls.len() < orig_walls {
            warnings.push(format!(
                "{} paredes duplicadas foram unificadas.",
                orig_walls - self.walls.len()
            ));
        }

        Ok(warnings)
    }

    /// Keeps every stored length to the resolution of `geometry`, so files
    /// hold "3.47" rather than the noise of float arithmetic (or of the
    /// single-precision files of older versions).
    pub fn normalize_precision(&mut self) {
        for wall in &mut self.walls {
            wall.start = wall.start.rounded();
            wall.end = wall.end.rounded();
        }
        if let Some(terrain) = &mut self.terrain {
            terrain.min = terrain.min.rounded();
            terrain.max = terrain.max.rounded();
        }
        for opening in &mut self.openings {
            // Positions along a wall are fractions: a micro-unit is a
            // micrometre per metre of wall.
            opening.t = (opening.t * 1e6).round() / 1e6;
            opening.width = opening.width.map(round_to_resolution);
            opening.height = opening.height.map(round_to_resolution);
            opening.sill = opening.sill.map(round_to_resolution);
        }
        for fixture in &mut self.fixtures {
            fixture.center = fixture.center.rounded();
            fixture.width = round_to_resolution(fixture.width);
            fixture.depth = round_to_resolution(fixture.depth);
        }
        self.wall_height = round_to_resolution(self.wall_height);
        for drawing in &mut self.drawings {
            drawing
                .shape
                .for_each_point(|point| *point = point.rounded());
        }
    }

    pub fn add_room(
        &mut self,

        first: Point,

        second: Point,

        name: impl Into<String>,
    ) -> Option<u64> {
        let name = name.into().trim().to_owned();

        if name.is_empty() {
            return None;
        }

        let min = Point::new(first.x.min(second.x), first.y.min(second.y));

        let max = Point::new(first.x.max(second.x), first.y.max(second.y));

        if max.x - min.x < 0.5 || max.y - min.y < 0.5 {
            return None;
        }

        let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
        self.add_polygon_room(&corners, &name).ok()
    }

    pub fn room_polygon(&self, room: &Room) -> Option<Vec<Point>> {
        let room_walls: Vec<&Wall> = self
            .walls
            .iter()
            .filter(|wall| room.wall_ids.contains(&wall.id))
            .collect();

        if room_walls.len() < 3 {
            return None;
        }

        let mut segments: Vec<(Point, Point)> =
            room_walls.iter().map(|w| (w.start, w.end)).collect();

        let mut polygon = Vec::with_capacity(segments.len());

        let first = segments.remove(0);

        polygon.push(first.0);

        let mut current_tip = first.1;

        while !segments.is_empty() {
            if let Some(pos) = segments
                .iter()
                .position(|s| s.0.distance(current_tip) < 0.05)
            {
                let matched = segments.remove(pos);

                polygon.push(current_tip);

                current_tip = matched.1;
            } else {
                let pos = segments
                    .iter()
                    .position(|s| s.1.distance(current_tip) < 0.05)?;
                let matched = segments.remove(pos);

                polygon.push(current_tip);

                current_tip = matched.0;
            }
        }

        if current_tip.distance(polygon[0]) < 0.05 {
            Some(polygon)
        } else {
            None
        }
    }

    pub fn room_area(&self, room: &Room) -> Option<f64> {
        if let Some(poly) = self.room_polygon(room) {
            let area = super::geometry::polygon_area(&poly);

            if area > 0.01 {
                return Some(area);
            }
        }

        None
    }

    pub fn add_opening(&mut self, wall_id: u64, kind: OpeningKind, t: f64) -> Option<u64> {
        self.add_opening_sized(wall_id, kind, t, None)
    }

    /// Inserts an opening centred at `t` along the wall, `width` wide (the
    /// kind's usual width when `None`). The rules are those of resizing: 8 cm
    /// of wall at each end and 10 cm between openings, measured with the
    /// real widths, so an opening that could be widened to a size can also
    /// be inserted at it.
    pub fn add_opening_sized(
        &mut self,
        wall_id: u64,
        kind: OpeningKind,
        t: f64,
        width: Option<f64>,
    ) -> Option<u64> {
        let size = width.unwrap_or_else(|| kind.width());
        if !size.is_finite() || size < kind.min_width() - 1e-4 || size > kind.max_width() + 1e-4 {
            return None;
        }
        let wall = self.walls.iter().find(|wall| wall.id == wall_id)?;
        let length = wall.length();

        if length < size + 0.16 {
            return None;
        }

        let half = size / (2.0 * length);

        let t = t.clamp(half + 0.08 / length, 1.0 - half - 0.08 / length);

        if self.openings.iter().any(|opening| {
            opening.wall_id == wall_id
                && ((opening.t - t).abs() * length) < (opening.width() + size) / 2.0 + 0.10
        }) {
            return None;
        }

        let id = self.id();
        self.openings.push(Opening {
            id,
            wall_id,
            kind,
            t,
            width: width.map(|_| size),
            flip_side: false,
            flip_hinge: false,
            height: None,
            sill: None,
        });
        Some(id)
    }

    /// Sets the height and, for windows, the sill of an opening.
    pub fn set_opening_heights(
        &mut self,
        id: u64,
        height: Option<f64>,
        sill: Option<f64>,
    ) -> Result<(), String> {
        let opening = self
            .openings
            .iter_mut()
            .find(|opening| opening.id == id)
            .ok_or("Abertura não encontrada.")?;
        if let Some(height) = height {
            if !height.is_finite() || !(0.3..=3.5).contains(&height) {
                return Err("A altura deve estar entre 0,30 e 3,50 m.".to_owned());
            }
            opening.height = Some(height);
        }
        if let Some(sill) = sill {
            if opening.kind != OpeningKind::Window {
                return Err("Só janelas têm peitoril.".to_owned());
            }
            if !sill.is_finite() || !(0.0..=2.5).contains(&sill) {
                return Err("O peitoril deve estar entre 0 e 2,50 m.".to_owned());
            }
            opening.sill = Some(sill);
        }
        Ok(())
    }

    /// Turns a door the other way: to the other side of the wall (`side`)
    /// and/or with its hinge on the other jamb (`hinge`).
    pub fn flip_opening(&mut self, id: u64, side: bool, hinge: bool) -> bool {
        let Some(opening) = self.openings.iter_mut().find(|opening| opening.id == id) else {
            return false;
        };
        if side {
            opening.flip_side = !opening.flip_side;
        }
        if hinge {
            opening.flip_hinge = !opening.flip_hinge;
        }
        true
    }

    /// Makes a door swing towards the side of its wall where `point` is.
    pub fn swing_towards(&mut self, id: u64, point: Point) {
        let Some(wall_id) = self
            .openings
            .iter()
            .find(|opening| opening.id == id)
            .map(|opening| opening.wall_id)
        else {
            return;
        };
        let Some(wall) = self.walls.iter().find(|wall| wall.id == wall_id) else {
            return;
        };
        let length = wall.length().max(1e-4);
        let (dx, dy) = (
            (wall.end.x - wall.start.x) / length,
            (wall.end.y - wall.start.y) / length,
        );
        // The leaf swings along (-dy, dx) unless flipped.
        let side = (point.x - wall.start.x) * -dy + (point.y - wall.start.y) * dx;
        if let Some(opening) = self.openings.iter_mut().find(|opening| opening.id == id) {
            opening.flip_side = side < 0.0;
        }
    }

    pub fn nearest_opening(&self, point: Point, max_distance: f64) -> Option<u64> {
        self.openings
            .iter()
            .filter_map(|op| {
                let wall = self.walls.iter().find(|w| w.id == op.wall_id)?;
                let center = Point::new(
                    wall.start.x + (wall.end.x - wall.start.x) * op.t,
                    wall.start.y + (wall.end.y - wall.start.y) * op.t,
                );
                // Measured to the opening's stretch of wall, so a long open
                // span is picked near it rather than metres away.
                let half = op.width() / 2.0 / wall.length().max(1e-4);
                let (from, to) = (op.t - half, op.t + half);
                let along = |t: f64| {
                    Point::new(
                        wall.start.x + (wall.end.x - wall.start.x) * t,
                        wall.start.y + (wall.end.y - wall.start.y) * t,
                    )
                };
                let (_, t) = nearest_point_on_segment(point, along(from), along(to));
                let closest = along(from + (to - from) * t);
                let dist = point.distance(closest);
                let hit_radius = (op.width() / 2.0).min(0.45).max(max_distance);
                (dist <= hit_radius).then_some((op.id, point.distance(center)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    pub fn delete_opening(&mut self, id: u64) -> bool {
        let len_before = self.openings.len();
        self.openings.retain(|op| op.id != id);
        self.openings.len() < len_before
    }

    pub fn validate_opening_width(&self, id: u64, new_width: f64) -> Result<(), String> {
        let op = self
            .openings
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| "Abertura não encontrada.".to_owned())?;
        let kind = op.kind;
        if !new_width.is_finite() || new_width < kind.min_width() || new_width > kind.max_width() {
            return Err(format!(
                "Valor fora dos limites permitidos: a largura para {} deve estar entre {:.2} m e {:.2} m.",
                kind.label().to_lowercase(),
                kind.min_width(),
                kind.max_width()
            )
            .replace('.', ","));
        }
        let wall = self
            .walls
            .iter()
            .find(|w| w.id == op.wall_id)
            .ok_or_else(|| "Parede associada não encontrada.".to_owned())?;
        let wall_len = wall.length();

        if wall_len < new_width + 0.16 {
            return Err("Abertura maior que a parede disponível.".to_owned());
        }

        let half_t = new_width / (2.0 * wall_len);
        let min_t = half_t + 0.08 / wall_len;
        let max_t = 1.0 - half_t - 0.08 / wall_len;
        let cur_t = op.t.clamp(min_t, max_t);

        for other in &self.openings {
            if other.id != id && other.wall_id == op.wall_id {
                let dist_along_wall = (other.t - cur_t).abs() * wall_len;
                let min_dist = (other.width() + new_width) / 2.0 + 0.10;
                if dist_along_wall < min_dist {
                    return Err(
                        "A nova largura sobrepõe outra abertura na mesma parede.".to_owned()
                    );
                }
            }
        }
        Ok(())
    }

    pub fn update_opening_width(&mut self, id: u64, new_width: f64) -> Result<(), String> {
        self.validate_opening_width(id, new_width)?;
        let op_idx = self.openings.iter().position(|o| o.id == id).unwrap();
        let wall_id = self.openings[op_idx].wall_id;
        let wall = self.walls.iter().find(|w| w.id == wall_id).unwrap();
        let wall_len = wall.length();

        let half_t = new_width / (2.0 * wall_len);
        let min_t = half_t + 0.08 / wall_len;
        let max_t = 1.0 - half_t - 0.08 / wall_len;
        let cur_t = self.openings[op_idx].t.clamp(min_t, max_t);

        self.openings[op_idx].width = Some(new_width);
        self.openings[op_idx].t = cur_t;
        Ok(())
    }

    pub fn move_opening_along_wall(&mut self, id: u64, target_point: Point) -> bool {
        let Some(op_idx) = self.openings.iter().position(|o| o.id == id) else {
            return false;
        };
        let wall_id = self.openings[op_idx].wall_id;
        let Some(wall) = self.walls.iter().find(|w| w.id == wall_id) else {
            return false;
        };
        let wall_len = wall.length();
        let op_width = self.openings[op_idx].width();

        let (_, raw_t) = nearest_point_on_segment(target_point, wall.start, wall.end);
        let half_t = op_width / (2.0 * wall_len);
        let min_t = half_t + 0.08 / wall_len;
        let max_t = 1.0 - half_t - 0.08 / wall_len;
        if min_t >= max_t {
            return false;
        }
        let clamped_t = raw_t.clamp(min_t, max_t);

        for other in &self.openings {
            if other.id != id && other.wall_id == wall_id {
                let dist_along = (other.t - clamped_t).abs() * wall_len;
                let min_dist = (other.width() + op_width) / 2.0 + 0.10;
                if dist_along < min_dist {
                    return false;
                }
            }
        }

        self.openings[op_idx].t = clamped_t;
        true
    }

    pub fn resize_wall_length(&mut self, id: u64, new_length: f64) -> Result<(), String> {
        if !new_length.is_finite() || new_length < 0.25 || new_length > 1000.0 {
            return Err("O comprimento deve estar entre 0,25 m e 1.000 m.".to_owned());
        }
        let wall_idx = self
            .walls
            .iter()
            .position(|w| w.id == id)
            .ok_or("Parede não encontrada.")?;

        let old_start = self.walls[wall_idx].start;
        let old_end = self.walls[wall_idx].end;
        let old_length = old_start.distance(old_end);
        if old_length < 1e-4 {
            return Err("Parede inválida.".to_owned());
        }

        let dir = Point::new(
            (old_end.x - old_start.x) / old_length,
            (old_end.y - old_start.y) / old_length,
        );
        let new_end = Point::new(
            old_start.x + dir.x * new_length,
            old_start.y + dir.y * new_length,
        );

        if let Some(terrain) = &self.terrain
            && !terrain.contains(new_end)
        {
            return Err("O novo comprimento ultrapassa os limites do terreno.".to_owned());
        }

        self.walls[wall_idx].end = new_end;
        for other in &mut self.walls {
            if other.id != id {
                if other.start.near(old_end, JOIN_TOLERANCE) {
                    other.start = new_end;
                }
                if other.end.near(old_end, JOIN_TOLERANCE) {
                    other.end = new_end;
                }
            }
        }
        Ok(())
    }

    pub fn rename_room(&mut self, id: u64, name: &str) -> Result<bool, String> {
        let room_idx = self
            .rooms
            .iter()
            .position(|r| r.id == id)
            .ok_or("Cômodo não encontrado.")?;
        let trimmed = name.trim();
        if self.rooms[room_idx].name != trimmed {
            self.rooms[room_idx].name = trimmed.to_owned();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn resize_room_dimensions(
        &mut self,
        id: u64,
        new_width: f64,
        new_height: f64,
    ) -> Result<(), String> {
        if !new_width.is_finite() || !new_height.is_finite() || new_width < 0.5 || new_height < 0.5
        {
            return Err("As dimensões do cômodo devem ser de pelo menos 0,50 m.".to_owned());
        }
        let room_idx = self
            .rooms
            .iter()
            .position(|r| r.id == id)
            .ok_or("Cômodo não encontrado.")?;

        let old_wall_ids = self.rooms[room_idx].wall_ids.clone();
        if !self.room_is_rectangle(&self.rooms[room_idx]) {
            return Err(
                "Este cômodo não é retangular: ajuste a forma pelos cantos e paredes.".to_owned(),
            );
        }
        if old_wall_ids.len() != 4 {
            return Err(
                "Cômodo não possui formato retangular padrão de 4 paredes para redimensionamento."
                    .to_owned(),
            );
        }

        let is_shared = self
            .rooms
            .iter()
            .any(|r| r.id != id && r.wall_ids.iter().any(|w| old_wall_ids.contains(w)));
        if is_shared {
            return Err(
                "Cômodo possui parede compartilhada. Ajuste as paredes individualmente para preservar a junção."
                    .to_owned(),
            );
        }

        let bounds = self
            .room_bounds(&self.rooms[room_idx])
            .ok_or("Geometria do cômodo inválida.")?;
        let anchor = bounds.0;
        let new_max = Point::new(anchor.x + new_width, anchor.y + new_height);

        if let Some(terrain) = &self.terrain
            && !terrain.contains(new_max)
        {
            return Err("As novas dimensões ultrapassam os limites do terreno.".to_owned());
        }

        let mut bottom_wall = None;
        let mut right_wall = None;
        let mut top_wall = None;
        let mut left_wall = None;

        for &wid in &old_wall_ids {
            let wall = self
                .walls
                .iter()
                .find(|w| w.id == wid)
                .ok_or("Parede do cômodo não encontrada.")?;
            let is_horizontal = (wall.start.y - wall.end.y).abs() < 0.05;
            let is_vertical = (wall.start.x - wall.end.x).abs() < 0.05;
            if is_horizontal {
                let mid_y = (wall.start.y + wall.end.y) / 2.0;
                if (mid_y - bounds.0.y).abs() < (mid_y - bounds.1.y).abs() {
                    bottom_wall = Some(wid);
                } else {
                    top_wall = Some(wid);
                }
            } else if is_vertical {
                let mid_x = (wall.start.x + wall.end.x) / 2.0;
                if (mid_x - bounds.0.x).abs() < (mid_x - bounds.1.x).abs() {
                    left_wall = Some(wid);
                } else {
                    right_wall = Some(wid);
                }
            }
        }

        let (Some(b_id), Some(r_id), Some(t_id), Some(l_id)) =
            (bottom_wall, right_wall, top_wall, left_wall)
        else {
            return Err(
                "Não foi possível mapear as paredes retangulares do cômodo para redimensionar."
                    .to_owned(),
            );
        };

        // Check if openings fit on the resized walls
        for (wid, new_len) in [
            (b_id, new_width),
            (t_id, new_width),
            (l_id, new_height),
            (r_id, new_height),
        ] {
            let openings_on_wall: Vec<_> =
                self.openings.iter().filter(|o| o.wall_id == wid).collect();
            for op in &openings_on_wall {
                let op_w = op.width();
                if new_len < op_w + 0.10 {
                    return Err(format!(
                        "A nova dimensão ({:.2} m) é pequena demais para a {} ({:.2} m).",
                        new_len,
                        op.kind.label(),
                        op_w
                    )
                    .replace('.', ","));
                }
            }
            if openings_on_wall.len() > 1 {
                let total_op_w: f64 = openings_on_wall.iter().map(|o| o.width() + 0.10).sum();
                if new_len < total_op_w {
                    return Err(format!(
                        "A nova dimensão ({:.2} m) não comporta todas as aberturas da parede.",
                        new_len
                    )
                    .replace('.', ","));
                }
            }
        }

        // Apply new wall geometry
        if let Some(w) = self.walls.iter_mut().find(|w| w.id == b_id) {
            let reversed = w.start.x > w.end.x;
            if reversed {
                w.start = Point::new(new_max.x, anchor.y);
                w.end = Point::new(anchor.x, anchor.y);
            } else {
                w.start = Point::new(anchor.x, anchor.y);
                w.end = Point::new(new_max.x, anchor.y);
            }
        }
        if let Some(w) = self.walls.iter_mut().find(|w| w.id == t_id) {
            let reversed = w.start.x > w.end.x;
            if reversed {
                w.start = Point::new(new_max.x, new_max.y);
                w.end = Point::new(anchor.x, new_max.y);
            } else {
                w.start = Point::new(anchor.x, new_max.y);
                w.end = Point::new(new_max.x, new_max.y);
            }
        }
        if let Some(w) = self.walls.iter_mut().find(|w| w.id == l_id) {
            let reversed = w.start.y > w.end.y;
            if reversed {
                w.start = Point::new(anchor.x, new_max.y);
                w.end = Point::new(anchor.x, anchor.y);
            } else {
                w.start = Point::new(anchor.x, anchor.y);
                w.end = Point::new(anchor.x, new_max.y);
            }
        }
        if let Some(w) = self.walls.iter_mut().find(|w| w.id == r_id) {
            let reversed = w.start.y > w.end.y;
            if reversed {
                w.start = Point::new(new_max.x, new_max.y);
                w.end = Point::new(new_max.x, anchor.y);
            } else {
                w.start = Point::new(new_max.x, anchor.y);
                w.end = Point::new(new_max.x, new_max.y);
            }
        }

        // Clamp openings so they stay within wall bounds
        for (wid, new_len) in [
            (b_id, new_width),
            (t_id, new_width),
            (l_id, new_height),
            (r_id, new_height),
        ] {
            for op in self.openings.iter_mut().filter(|o| o.wall_id == wid) {
                let margin_t = (op.width() / 2.0 + 0.05) / new_len;
                op.t = op.t.clamp(margin_t, 1.0 - margin_t);
            }
        }

        Ok(())
    }

    pub fn nearest_wall(&self, point: Point, max_distance: f64) -> Option<(u64, f64)> {
        self.walls
            .iter()
            .filter_map(|wall| {
                let (_, t) = nearest_point_on_segment(point, wall.start, wall.end);
                let distance = distance_to_segment(point, wall.start, wall.end);

                (distance <= max_distance).then_some((wall.id, t, distance))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(id, t, _)| (id, t))
    }

    pub fn room_bounds(&self, room: &Room) -> Option<(Point, Point)> {
        let walls: Vec<_> = self
            .walls
            .iter()
            .filter(|wall| room.wall_ids.contains(&wall.id))
            .collect();

        if walls.is_empty() {
            return None;
        }

        let min = Point::new(
            walls
                .iter()
                .map(|wall| wall.start.x.min(wall.end.x))
                .fold(f64::INFINITY, f64::min),
            walls
                .iter()
                .map(|wall| wall.start.y.min(wall.end.y))
                .fold(f64::INFINITY, f64::min),
        );

        let max = Point::new(
            walls
                .iter()
                .map(|wall| wall.start.x.max(wall.end.x))
                .fold(f64::NEG_INFINITY, f64::max),
            walls
                .iter()
                .map(|wall| wall.start.y.max(wall.end.y))
                .fold(f64::NEG_INFINITY, f64::max),
        );

        Some((min, max))
    }

    pub fn room_centroid(&self, room: &Room) -> Option<Point> {
        if let Some(poly) = self.room_polygon(room) {
            Some(crate::domain::geometry::polygon_centroid(&poly))
        } else if let Some((min, max)) = self.room_bounds(room) {
            Some(min.midpoint(max))
        } else {
            None
        }
    }

    pub fn room_at(&self, point: Point) -> Option<u64> {
        self.rooms.iter().rev().find_map(|room| {
            if let Some(poly) = self.room_polygon(room) {
                if super::geometry::polygon_contains_point(&poly, point) {
                    return Some(room.id);
                }
            } else if let Some((min, max)) = self.room_bounds(room)
                && point.x > min.x
                && point.x < max.x
                && point.y > min.y
                && point.y < max.y
            {
                return Some(room.id);
            }

            None
        })
    }

    pub fn delete_room(&mut self, id: u64) -> bool {
        let Some(index) = self.rooms.iter().position(|room| room.id == id) else {
            return false;
        };

        let room_to_delete = self.rooms.remove(index);

        let deleted_wall_ids = room_to_delete.wall_ids;

        let mut walls_to_remove = Vec::new();

        for wall_id in deleted_wall_ids {
            let used_by_other = self.rooms.iter().any(|r| r.wall_ids.contains(&wall_id));

            if !used_by_other {
                walls_to_remove.push(wall_id);
            }
        }

        self.walls
            .retain(|wall| !walls_to_remove.contains(&wall.id));

        self.openings
            .retain(|opening| !walls_to_remove.contains(&opening.wall_id));

        true
    }

    pub fn delete_wall(&mut self, id: u64) {
        self.walls.retain(|wall| wall.id != id);

        self.openings.retain(|opening| opening.wall_id != id);

        self.rooms.retain(|room| !room.wall_ids.contains(&id));
    }

    pub fn move_wall(&mut self, id: u64, delta: Point) {
        let Some(index) = self.walls.iter().position(|wall| wall.id == id) else {
            return;
        };

        let old_start = self.walls[index].start;

        let old_end = self.walls[index].end;

        let mut moving_points = vec![old_start, old_end];

        for wall in &self.walls {
            if wall.id != id {
                if wall.start.near(old_start, JOIN_TOLERANCE)
                    || wall.start.near(old_end, JOIN_TOLERANCE)
                {
                    moving_points.push(wall.start);
                }

                if wall.end.near(old_start, JOIN_TOLERANCE)
                    || wall.end.near(old_end, JOIN_TOLERANCE)
                {
                    moving_points.push(wall.end);
                }
            }
        }

        let min_x = moving_points
            .iter()
            .map(|p| p.x)
            .fold(f64::INFINITY, f64::min);

        let max_x = moving_points
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max);

        let min_y = moving_points
            .iter()
            .map(|p| p.y)
            .fold(f64::INFINITY, f64::min);

        let max_y = moving_points
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);

        let delta =
            self.clamp_delta_to_terrain(Point::new(min_x, min_y), Point::new(max_x, max_y), delta);

        for (other_index, wall) in self.walls.iter_mut().enumerate() {
            if other_index == index {
                wall.start = wall.start.translated(delta);

                wall.end = wall.end.translated(delta);
            } else {
                if wall.start.near(old_start, JOIN_TOLERANCE)
                    || wall.start.near(old_end, JOIN_TOLERANCE)
                {
                    wall.start = wall.start.translated(delta);
                }

                if wall.end.near(old_start, JOIN_TOLERANCE)
                    || wall.end.near(old_end, JOIN_TOLERANCE)
                {
                    wall.end = wall.end.translated(delta);
                }
            }
        }
    }

    pub fn move_room(&mut self, id: u64, delta: Point) -> bool {
        let Some(room) = self.rooms.iter().find(|room| room.id == id) else {
            return false;
        };

        let wall_ids = room.wall_ids.clone();

        let is_shared = self
            .rooms
            .iter()
            .any(|r| r.id != id && r.wall_ids.iter().any(|w| wall_ids.contains(w)));

        if is_shared {
            return false;
        }

        let delta = self
            .room_bounds(room)
            .map(|(min, max)| self.clamp_delta_to_terrain(min, max, delta))
            .unwrap_or(delta);

        for wall in &mut self.walls {
            if wall_ids.contains(&wall.id) {
                wall.start = wall.start.translated(delta);

                wall.end = wall.end.translated(delta);
            }
        }

        true
    }

    fn clamp_delta_to_terrain(&self, min: Point, max: Point, delta: Point) -> Point {
        let Some(terrain) = &self.terrain else {
            return delta;
        };

        let (min, max) = (
            Point::new(min.x.min(max.x), min.y.min(max.y)),
            Point::new(min.x.max(max.x), min.y.max(max.y)),
        );

        if max.x - min.x > terrain.width() + 0.001 || max.y - min.y > terrain.height() + 0.001 {
            return delta;
        }

        let max_origin_x = (terrain.max.x - (max.x - min.x)).max(terrain.min.x);

        let max_origin_y = (terrain.max.y - (max.y - min.y)).max(terrain.min.y);

        Point::new(
            (min.x + delta.x).clamp(terrain.min.x, max_origin_x) - min.x,
            (min.y + delta.y).clamp(terrain.min.y, max_origin_y) - min.y,
        )
    }

    pub fn bounds(&self) -> Option<(Point, Point)> {
        if self.walls.is_empty()
            && self.fixtures.is_empty()
            && self.drawings.is_empty()
            && self.terrain.is_none()
        {
            return None;
        }

        let mut min = self
            .terrain
            .as_ref()
            .map(|t| t.min)
            .unwrap_or(Point::new(f64::INFINITY, f64::INFINITY));

        let mut max = self
            .terrain
            .as_ref()
            .map(|t| t.max)
            .unwrap_or(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));

        if let Some((content_min, content_max)) = self.content_bounds() {
            min.x = min.x.min(content_min.x);

            min.y = min.y.min(content_min.y);

            max.x = max.x.max(content_max.x);

            max.y = max.y.max(content_max.y);
        }

        Some((min, max))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]

    fn terrain_keeps_exact_area_when_resized_and_moved() {
        let mut project = Project::default();

        let id = project
            .set_terrain(Point::new(0.0, 0.0), Point::new(5.0, 10.0))
            .unwrap();

        assert_eq!(project.terrain.as_ref().unwrap().area(), 50.0);

        assert_eq!(
            project.bounds(),
            Some((Point::new(0.0, 0.0), Point::new(5.0, 10.0)))
        );

        assert!(project.resize_terrain(50.0, 50.0));

        assert_eq!(project.terrain.as_ref().unwrap().area(), 2500.0);

        project.move_terrain(id, Point::new(3.0, 4.0));

        assert_eq!(
            project.bounds(),
            Some((Point::new(3.0, 4.0), Point::new(53.0, 54.0)))
        );

        assert!(project.delete_terrain(id));

        assert_eq!(project.bounds(), None);
    }

    #[test]

    fn existing_room_can_be_centered_inside_terrain() {
        let mut project = Project::default();

        project.add_room(Point::new(-20.0, 2.0), Point::new(-10.0, 10.0), "Garagem");

        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 50.0));

        assert!(!project.content_inside_terrain());

        assert!(project.content_fits_terrain());

        assert!(project.center_content_on_terrain());

        assert_eq!(
            project.content_bounds(),
            Some((Point::new(0.0, 21.0), Point::new(10.0, 29.0)))
        );

        assert!(project.content_inside_terrain());
    }

    #[test]

    fn new_rooms_and_moves_stay_within_terrain() {
        let mut project = Project::default();

        let terrain_id = project
            .set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 50.0))
            .unwrap();

        assert!(
            project
                .add_room(Point::new(11.0, 1.0), Point::new(15.0, 5.0), "Fora")
                .is_none()
        );

        let room_id = project
            .add_room(Point::new(1.0, 1.0), Point::new(5.0, 5.0), "Dentro")
            .unwrap();

        project.move_room(room_id, Point::new(20.0, 0.0));

        assert_eq!(
            project.content_bounds(),
            Some((Point::new(6.0, 1.0), Point::new(10.0, 5.0)))
        );

        project.move_terrain(terrain_id, Point::new(3.0, 2.0));

        assert_eq!(
            project.content_bounds(),
            Some((Point::new(9.0, 3.0), Point::new(13.0, 7.0)))
        );

        assert!(project.content_inside_terrain());
    }

    #[test]

    fn room_creates_four_walls_and_connected_move() {
        let mut project = Project::default();

        let room_id = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
            .unwrap();

        let first_wall = project
            .rooms
            .iter()
            .find(|room| room.id == room_id)
            .unwrap()
            .wall_ids[0];

        project.move_wall(first_wall, Point::new(0.0, 0.25));

        assert_eq!(project.walls.len(), 4);

        assert_eq!(project.walls[1].start.y, 0.25);

        assert_eq!(project.walls[3].end.y, 0.25);
    }

    #[test]

    fn openings_are_inserted_at_their_own_width_and_respect_real_widths() {
        let mut project = Project::default();
        // A 1 m wall takes a 0.70 m door, though not the usual 0.90 m one.
        let short = project
            .add_wall(Point::new(0.0, 0.0), Point::new(1.0, 0.0))
            .unwrap();
        assert!(project.add_opening(short, OpeningKind::Door, 0.5).is_none());
        let door = project
            .add_opening_sized(short, OpeningKind::Door, 0.5, Some(0.7))
            .unwrap();
        let width = project
            .openings
            .iter()
            .find(|o| o.id == door)
            .unwrap()
            .width();
        assert!((width - 0.7).abs() < 1e-6);
        assert!(
            project
                .add_opening_sized(short, OpeningKind::Door, 0.5, Some(3.0))
                .is_none()
        );

        // A 9 m open span blocks a door anywhere on it.
        let long = project
            .add_wall(Point::new(0.0, 5.0), Point::new(10.0, 5.0))
            .unwrap();
        project
            .add_opening_sized(long, OpeningKind::Gap, 0.5, Some(9.0))
            .unwrap();
        assert!(project.add_opening(long, OpeningKind::Door, 0.2).is_none());
    }

    #[test]
    fn openings_cannot_overlap() {
        let mut project = Project::default();

        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();

        assert!(project.add_opening(wall, OpeningKind::Door, 0.5).is_some());

        assert!(
            project
                .add_opening(wall, OpeningKind::Window, 0.5)
                .is_none()
        );
    }

    #[test]

    fn moving_and_deleting_room_updates_its_walls_and_openings() {
        let mut project = Project::default();

        let room_id = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Meu quarto")
            .unwrap();

        let wall_id = project.rooms[0].wall_ids[0];

        project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        assert_eq!(project.room_at(Point::new(2.0, 1.0)), Some(room_id));

        project.move_room(room_id, Point::new(1.0, 0.5));

        assert_eq!(project.room_at(Point::new(2.0, 1.0)), Some(room_id));

        assert_eq!(project.walls[0].start, Point::new(1.0, 0.5));

        assert!(project.delete_room(room_id));

        assert!(project.rooms.is_empty());

        assert!(project.walls.is_empty());

        assert!(project.openings.is_empty());
    }

    #[test]
    fn adjacent_rooms_share_walls_without_duplication() {
        let mut project = Project::default();
        let r1 = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        assert_eq!(project.walls.len(), 4);

        let r2 = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
            .unwrap();
        // The shared wall at x = 4.0 is reused, so total walls is 7 instead of 8!
        assert_eq!(project.walls.len(), 7);

        let r1_walls = &project.rooms.iter().find(|r| r.id == r1).unwrap().wall_ids;
        let r2_walls = &project.rooms.iter().find(|r| r.id == r2).unwrap().wall_ids;
        let shared: Vec<_> = r1_walls.iter().filter(|w| r2_walls.contains(w)).collect();
        assert_eq!(shared.len(), 1);
    }

    #[test]
    fn deleting_room_preserves_shared_wall_and_openings() {
        let mut project = Project::default();
        let r1 = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let r2 = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
            .unwrap();
        assert_eq!(project.walls.len(), 7);

        let r1_walls = project
            .rooms
            .iter()
            .find(|r| r.id == r1)
            .unwrap()
            .wall_ids
            .clone();
        let r2_walls = project
            .rooms
            .iter()
            .find(|r| r.id == r2)
            .unwrap()
            .wall_ids
            .clone();
        let shared_wall_id = *r1_walls.iter().find(|w| r2_walls.contains(w)).unwrap();

        // Add a door to the shared wall
        let door_id = project
            .add_opening(shared_wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        // Delete Room 1
        assert!(project.delete_room(r1));
        assert_eq!(project.rooms.len(), 1);
        // Shared wall is preserved in Room 2!
        assert!(project.walls.iter().any(|w| w.id == shared_wall_id));
        assert_eq!(project.walls.len(), 4);
        // Door is preserved!
        assert!(project.openings.iter().any(|o| o.id == door_id));
    }

    #[test]
    fn moving_connected_wall_respects_terrain_bounds_for_all_affected_points() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let _r = project
            .add_room(Point::new(1.0, 1.0), Point::new(5.0, 5.0), "Quarto")
            .unwrap();

        let top_wall = project.rooms[0].wall_ids[2];
        // Try to move top wall far up beyond terrain (y + 20)
        project.move_wall(top_wall, Point::new(0.0, 20.0));

        // All walls must stay within terrain bounds (y <= 10.0)
        for wall in &project.walls {
            assert!(wall.start.y <= 10.001);
            assert!(wall.end.y <= 10.001);
        }
    }

    #[test]
    fn room_polygon_and_area_are_calculated_correctly() {
        let mut project = Project::default();
        let _r = project
            .add_room(Point::new(1.0, 2.0), Point::new(5.0, 6.0), "Estudo")
            .unwrap();
        let room = &project.rooms[0];
        let poly = project.room_polygon(room).unwrap();
        assert_eq!(poly.len(), 4);
        assert!((project.room_area(room).unwrap() - 16.0).abs() < 1e-3);
    }

    #[test]
    fn split_wall_rejects_cutting_through_door_and_preserves_intact() {
        let mut project = Project::default();
        let wall_id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0))
            .unwrap();
        let _door = project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        // Attempting to split at x = 2.5m (cuts through the door!)
        let split_res = project.split_wall_at(wall_id, Point::new(2.5, 0.0));
        assert!(split_res.is_none());
        assert_eq!(project.walls.len(), 1);

        // Splitting safely away from the door at x = 0.5m
        let safe_split = project.split_wall_at(wall_id, Point::new(0.5, 0.0));
        assert!(safe_split.is_some());
        assert_eq!(project.walls.len(), 2);
    }

    #[test]
    fn deduplicate_walls_inverts_t_when_directions_are_opposite() {
        let mut project = Project::default();
        let w1 = project.id();
        project.walls.push(Wall {
            id: w1,
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            masonry: None,
        });
        let w2 = project.id();
        project.walls.push(Wall {
            id: w2,
            start: Point::new(10.0, 0.0),
            end: Point::new(0.0, 0.0),
            masonry: None,
        });

        // Opening on w2 at t = 0.20 (physically at x = 8.0m)
        let op_id = project.id();
        project.openings.push(Opening {
            id: op_id,
            wall_id: w2,
            kind: OpeningKind::Door,
            t: 0.20,
            width: None,
            flip_side: false,
            flip_hinge: false,
            height: None,
            sill: None,
        });

        project.deduplicate_walls();
        assert_eq!(project.walls.len(), 1);
        assert_eq!(project.openings.len(), 1);
        assert_eq!(project.openings[0].wall_id, w1);
        // t must now be 1.0 - 0.20 = 0.80, keeping physical x = 8.0m!
        assert!((project.openings[0].t - 0.80).abs() < 1e-3);
    }

    #[test]
    fn opening_kind_limits_and_validation() {
        let mut project = Project::default();
        let w_id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        let door_id = project.add_opening(w_id, OpeningKind::Door, 0.3).unwrap();

        // 1. Kind limits
        assert_eq!(OpeningKind::Door.min_width(), 0.60);
        assert_eq!(OpeningKind::Door.max_width(), 2.00);
        assert_eq!(OpeningKind::Window.min_width(), 0.40);
        assert_eq!(OpeningKind::Window.max_width(), 4.00);

        // 2. Reject smaller than min_width
        let err_too_small = project.update_opening_width(door_id, 0.50);
        assert!(err_too_small.is_err());
        assert!(
            err_too_small
                .unwrap_err()
                .contains("Valor fora dos limites")
        );

        // 3. Reject larger than max_width
        let err_too_large = project.update_opening_width(door_id, 2.50);
        assert!(err_too_large.is_err());
        assert!(
            err_too_large
                .unwrap_err()
                .contains("Valor fora dos limites")
        );

        // 4. Reject larger than wall length (minus margins)
        // Wall is 3.0m, wall_len < 2.90 + 0.16 => 3.0 < 3.06
        let err_wall_too_short = project.update_opening_width(door_id, 1.95);
        // Door max is 2.00. 1.95 is <= max_width, but 1.95 + 0.16 = 2.11 <= 3.0, so 1.95 fits!
        assert!(err_wall_too_short.is_ok());

        // Now test on a short wall of 1.0m:
        let short_w = project
            .add_wall(Point::new(10.0, 0.0), Point::new(11.0, 0.0))
            .unwrap();
        let short_door = project.id();
        project.openings.push(Opening {
            id: short_door,
            wall_id: short_w,
            kind: OpeningKind::Door,
            t: 0.5,
            width: Some(0.70),
            flip_side: false,
            flip_hinge: false,
            height: None,
            sill: None,
        });
        // 0.90 + 0.16 = 1.06 > 1.00 => should fail with wall too short / abertura maior que a parede
        let err_short = project.update_opening_width(short_door, 0.90);
        assert!(err_short.is_err());
        assert!(
            err_short
                .unwrap_err()
                .contains("Abertura maior que a parede")
        );

        // 5. Reject collision / overlap with another opening on same wall.
        // The door is still 1.95 m wide; inserting is checked against real
        // widths, so narrow it first to make room for the window.
        project.update_opening_width(door_id, 0.90).unwrap();
        let _op2_id = project.add_opening(w_id, OpeningKind::Window, 0.8).unwrap();
        // Expanding door to collide with op2
        let err_overlap = project.update_opening_width(door_id, 1.80);
        assert!(err_overlap.is_err());
        assert!(err_overlap.unwrap_err().contains("sobrepõe outra abertura"));
    }
}
