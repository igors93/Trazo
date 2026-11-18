use serde::{Deserialize, Serialize};

use super::geometry::{Point, nearest_point_on_segment};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,

    pub name: String,

    pub walls: Vec<Wall>,

    pub rooms: Vec<Room>,

    pub openings: Vec<Opening>,

    #[serde(default)]
    pub terrain: Option<Terrain>,

    next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: 1,

            name: "Minha planta".to_owned(),

            walls: Vec::new(),

            rooms: Vec::new(),

            openings: Vec::new(),

            terrain: None,

            next_id: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub id: u64,

    pub start: Point,

    pub end: Point,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Terrain {
    pub id: u64,

    pub min: Point,

    pub max: Point,
}

impl Terrain {
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn area(&self) -> f32 {
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
    pub fn length(&self) -> f32 {
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
}

impl OpeningKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Door => "Porta",

            Self::Window => "Janela",
        }
    }

    pub fn width(self) -> f32 {
        match self {
            Self::Door => 0.9,

            Self::Window => 1.2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub id: u64,
    pub wall_id: u64,
    pub kind: OpeningKind,
    pub t: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
}

impl Opening {
    pub fn width(&self) -> f32 {
        self.width.unwrap_or_else(|| self.kind.width())
    }
}

fn is_collinear_and_overlapping(a: Point, b: Point, p1: Point, p2: Point) -> Option<(f32, f32)> {
    let len = a.distance(b);

    if len < 1e-4 {
        return None;
    }

    let u = Point::new((b.x - a.x) / len, (b.y - a.y) / len);

    let n = Point::new(-u.y, u.x);

    let dist1 = ((p1.x - a.x) * n.x + (p1.y - a.y) * n.y).abs();

    let dist2 = ((p2.x - a.x) * n.x + (p2.y - a.y) * n.y).abs();

    if dist1 > 0.02 || dist2 > 0.02 {
        return None;
    }

    let t1 = (p1.x - a.x) * u.x + (p1.y - a.y) * u.y;

    let t2 = (p2.x - a.x) * u.x + (p2.y - a.y) * u.y;

    let w_min = t1.min(t2);

    let w_max = t1.max(t2);

    let overlap_start = w_min.max(0.0);

    let overlap_end = w_max.min(len);

    if overlap_end - overlap_start > 0.04 {
        Some((w_min, w_max))
    } else {
        None
    }
}

impl Project {
    pub fn content_bounds(&self) -> Option<(Point, Point)> {
        if self.walls.is_empty() {
            return None;
        }

        let mut min = Point::new(f32::INFINITY, f32::INFINITY);

        let mut max = Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY);

        for wall in &self.walls {
            min.x = min.x.min(wall.start.x.min(wall.end.x));

            min.y = min.y.min(wall.start.y.min(wall.end.y));

            max.x = max.x.max(wall.start.x.max(wall.end.x));

            max.y = max.y.max(wall.start.y.max(wall.end.y));
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

    pub fn resize_terrain(&mut self, width: f32, height: f32) -> bool {
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
        }
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

        self.walls.push(Wall { id, start, end });

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

        let split_t = split_dist / total_len;

        self.walls[wall_idx].end = split_point;

        let new_id = self.id();

        self.walls.push(Wall {
            id: new_id,

            start: split_point,

            end: old_end,
        });

        for room in &mut self.rooms {
            if let Some(pos) = room.wall_ids.iter().position(|&id| id == wall_id) {
                room.wall_ids.insert(pos + 1, new_id);
            }
        }

        for opening in &mut self.openings {
            if opening.wall_id == wall_id {
                if opening.t >= split_t {
                    opening.wall_id = new_id;

                    opening.t = ((opening.t - split_t) / (1.0 - split_t)).clamp(0.01, 0.99);
                } else {
                    opening.t = (opening.t / split_t).clamp(0.01, 0.99);
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

        let dir = Point::new((end.x - start.x) / len, (end.y - start.y) / len);

        let mut sub_walls: Vec<(f32, f32, u64)> = Vec::new();

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

                let duplicate = (w1.start.distance(w2.start) < 0.02
                    && w1.end.distance(w2.end) < 0.02)
                    || (w1.start.distance(w2.end) < 0.02 && w1.end.distance(w2.start) < 0.02);

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
                        }
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
        for wall in &self.walls {
            if !wall.start.x.is_finite()
                || !wall.start.y.is_finite()
                || !wall.end.x.is_finite()
                || !wall.end.y.is_finite()
            {
                return Err("O projeto contém coordenadas inválidas em paredes.".to_owned());
            }
        }

        if let Some(terrain) = &mut self.terrain {
            if !terrain.min.x.is_finite()
                || !terrain.min.y.is_finite()
                || !terrain.max.x.is_finite()
                || !terrain.max.y.is_finite()
            {
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
        if self.next_id <= max_id {
            self.next_id = max_id + 1;
        }

        // 4. Deduplicate shared walls
        self.deduplicate_walls();

        Ok(warnings)
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

        if self
            .terrain
            .as_ref()
            .is_some_and(|terrain| !terrain.contains(min) || !terrain.contains(max))
        {
            return None;
        }

        let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];

        let mut all_wall_ids = Vec::new();

        for index in 0..4 {
            let edge_walls =
                self.find_or_create_edge_walls(corners[index], corners[(index + 1) % 4])?;

            all_wall_ids.extend(edge_walls);
        }

        let id = self.id();

        self.rooms.push(Room {
            id,

            name,

            legacy_kind: None,

            wall_ids: all_wall_ids,
        });

        Some(id)
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

    pub fn room_area(&self, room: &Room) -> Option<f32> {
        if let Some(poly) = self.room_polygon(room) {
            let area = super::geometry::polygon_area(&poly);

            if area > 0.01 {
                return Some(area);
            }
        }

        None
    }

    pub fn add_opening(&mut self, wall_id: u64, kind: OpeningKind, t: f32) -> Option<u64> {
        let wall = self.walls.iter().find(|wall| wall.id == wall_id)?;

        if wall.length() < kind.width() + 0.3 {
            return None;
        }

        let half = kind.width() / (2.0 * wall.length());

        let t = t.clamp(
            half + 0.08 / wall.length(),
            1.0 - half - 0.08 / wall.length(),
        );

        if self.openings.iter().any(|opening| {
            opening.wall_id == wall_id
                && ((opening.t - t).abs() * wall.length())
                    < (opening.kind.width() + kind.width()) / 2.0 + 0.15
        }) {
            return None;
        }

        let id = self.id();
        self.openings.push(Opening {
            id,
            wall_id,
            kind,
            t,
            width: None,
        });
        Some(id)
    }

    pub fn nearest_opening(&self, point: Point, max_distance: f32) -> Option<u64> {
        self.openings
            .iter()
            .filter_map(|op| {
                let wall = self.walls.iter().find(|w| w.id == op.wall_id)?;
                let center = Point::new(
                    wall.start.x + (wall.end.x - wall.start.x) * op.t,
                    wall.start.y + (wall.end.y - wall.start.y) * op.t,
                );
                let dist = point.distance(center);
                let hit_radius = (op.width() / 2.0).max(max_distance);
                (dist <= hit_radius).then_some((op.id, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    pub fn delete_opening(&mut self, id: u64) -> bool {
        let len_before = self.openings.len();
        self.openings.retain(|op| op.id != id);
        self.openings.len() < len_before
    }

    pub fn update_opening_width(&mut self, id: u64, new_width: f32) -> Result<(), String> {
        if !new_width.is_finite() || new_width < 0.25 || new_width > 10.0 {
            return Err("A largura deve estar entre 0,25 m e 10,00 m.".to_owned());
        }
        let op_idx = self
            .openings
            .iter()
            .position(|o| o.id == id)
            .ok_or("Abertura não encontrada.")?;
        let wall_id = self.openings[op_idx].wall_id;
        let wall = self
            .walls
            .iter()
            .find(|w| w.id == wall_id)
            .ok_or("Parede associada não encontrada.")?;
        let wall_len = wall.length();

        if wall_len < new_width + 0.16 {
            return Err("A parede é curta demais para esta largura de abertura.".to_owned());
        }

        let half_t = new_width / (2.0 * wall_len);
        let min_t = half_t + 0.08 / wall_len;
        let max_t = 1.0 - half_t - 0.08 / wall_len;

        let cur_t = self.openings[op_idx].t.clamp(min_t, max_t);

        for other in &self.openings {
            if other.id != id && other.wall_id == wall_id {
                let dist_along_wall = (other.t - cur_t).abs() * wall_len;
                let min_dist = (other.width() + new_width) / 2.0 + 0.10;
                if dist_along_wall < min_dist {
                    return Err(
                        "A nova largura faria a abertura sobrepor outra na mesma parede."
                            .to_owned(),
                    );
                }
            }
        }

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

    pub fn resize_wall_length(&mut self, id: u64, new_length: f32) -> Result<(), String> {
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
                if other.start.distance(old_end) < 0.01 {
                    other.start = new_end;
                }
                if other.end.distance(old_end) < 0.01 {
                    other.end = new_end;
                }
            }
        }
        Ok(())
    }

    pub fn resize_room_dimensions(
        &mut self,
        id: u64,
        name: &str,
        new_width: f32,
        new_height: f32,
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

        if !name.trim().is_empty() {
            self.rooms[room_idx].name = name.trim().to_owned();
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

        let old_wall_ids = self.rooms[room_idx].wall_ids.clone();
        let is_shared = self
            .rooms
            .iter()
            .any(|r| r.id != id && r.wall_ids.iter().any(|w| old_wall_ids.contains(w)));
        if is_shared {
            return Err("Cômodo possui parede compartilhada. Ajuste as paredes individualmente para preservar a junção.".to_owned());
        }

        self.walls.retain(|w| !old_wall_ids.contains(&w.id));
        self.openings.retain(|o| !old_wall_ids.contains(&o.wall_id));

        let corners = [
            anchor,
            Point::new(new_max.x, anchor.y),
            new_max,
            Point::new(anchor.x, new_max.y),
        ];
        let mut new_ids = Vec::with_capacity(4);
        for i in 0..4 {
            let wid = self
                .add_wall(corners[i], corners[(i + 1) % 4])
                .ok_or("Erro ao criar parede do cômodo.")?;
            new_ids.push(wid);
        }
        self.rooms[room_idx].wall_ids = new_ids;
        Ok(())
    }

    pub fn nearest_wall(&self, point: Point, max_distance: f32) -> Option<(u64, f32)> {
        self.walls
            .iter()
            .filter_map(|wall| {
                let (nearest, t) = nearest_point_on_segment(point, wall.start, wall.end);

                (nearest.distance(point) <= max_distance).then_some((
                    wall.id,
                    t,
                    nearest.distance(point),
                ))
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
                .fold(f32::INFINITY, f32::min),
            walls
                .iter()
                .map(|wall| wall.start.y.min(wall.end.y))
                .fold(f32::INFINITY, f32::min),
        );

        let max = Point::new(
            walls
                .iter()
                .map(|wall| wall.start.x.max(wall.end.x))
                .fold(f32::NEG_INFINITY, f32::max),
            walls
                .iter()
                .map(|wall| wall.start.y.max(wall.end.y))
                .fold(f32::NEG_INFINITY, f32::max),
        );

        Some((min, max))
    }

    pub fn room_centroid(&self, room: &Room) -> Option<Point> {
        if let Some(poly) = self.room_polygon(room) {
            Some(crate::domain::geometry::polygon_centroid(&poly))
        } else if let Some((min, max)) = self.room_bounds(room) {
            Some(Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0))
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
                if wall.start.distance(old_start) < 0.01 || wall.start.distance(old_end) < 0.01 {
                    moving_points.push(wall.start);
                }

                if wall.end.distance(old_start) < 0.01 || wall.end.distance(old_end) < 0.01 {
                    moving_points.push(wall.end);
                }
            }
        }

        let min_x = moving_points
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min);

        let max_x = moving_points
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);

        let min_y = moving_points
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);

        let max_y = moving_points
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);

        let delta =
            self.clamp_delta_to_terrain(Point::new(min_x, min_y), Point::new(max_x, max_y), delta);

        for (other_index, wall) in self.walls.iter_mut().enumerate() {
            if other_index == index {
                wall.start = wall.start.translated(delta);

                wall.end = wall.end.translated(delta);
            } else {
                if wall.start.distance(old_start) < 0.01 || wall.start.distance(old_end) < 0.01 {
                    wall.start = wall.start.translated(delta);
                }

                if wall.end.distance(old_start) < 0.01 || wall.end.distance(old_end) < 0.01 {
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
        if self.walls.is_empty() && self.terrain.is_none() {
            return None;
        }

        let mut min = self
            .terrain
            .as_ref()
            .map(|t| t.min)
            .unwrap_or(Point::new(f32::INFINITY, f32::INFINITY));

        let mut max = self
            .terrain
            .as_ref()
            .map(|t| t.max)
            .unwrap_or(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY));

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
}
