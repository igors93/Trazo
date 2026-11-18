use serde::{Deserialize, Serialize};

use super::geometry::{Point, nearest_point_on_segment};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub walls: Vec<Wall>,
    pub rooms: Vec<Room>,
    pub openings: Vec<Opening>,
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
            next_id: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wall {
    pub id: u64,
    pub start: Point,
    pub end: Point,
}

impl Wall {
    pub fn length(&self) -> f32 {
        self.start.distance(self.end)
    }

    pub fn point_at(&self, t: f32) -> Point {
        Point::new(
            self.start.x + (self.end.x - self.start.x) * t,
            self.start.y + (self.end.y - self.start.y) * t,
        )
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
    pub const ALL: [Self; 7] = [
        Self::Bedroom,
        Self::LivingRoom,
        Self::Kitchen,
        Self::Bathroom,
        Self::DiningRoom,
        Self::Office,
        Self::Other,
    ];

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    pub id: u64,
    pub kind: RoomKind,
    pub wall_ids: [u64; 4],
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Opening {
    pub id: u64,
    pub wall_id: u64,
    pub kind: OpeningKind,
    pub t: f32,
}

impl Project {
    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_wall(&mut self, start: Point, end: Point) -> Option<u64> {
        if start.distance(end) < 0.24 {
            return None;
        }
        let id = self.id();
        self.walls.push(Wall { id, start, end });
        Some(id)
    }

    pub fn add_room(&mut self, first: Point, second: Point, kind: RoomKind) -> Option<u64> {
        let min = Point::new(first.x.min(second.x), first.y.min(second.y));
        let max = Point::new(first.x.max(second.x), first.y.max(second.y));
        if max.x - min.x < 0.5 || max.y - min.y < 0.5 {
            return None;
        }
        let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
        let mut ids = [0; 4];
        for index in 0..4 {
            ids[index] = self.add_wall(corners[index], corners[(index + 1) % 4])?;
        }
        let id = self.id();
        self.rooms.push(Room {
            id,
            kind,
            wall_ids: ids,
        });
        Some(id)
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
        });
        Some(id)
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

    pub fn bounds(&self) -> Option<(Point, Point)> {
        if self.walls.is_empty() {
            return None;
        }
        Some((
            Point::new(
                self.walls
                    .iter()
                    .map(|w| w.start.x.min(w.end.x))
                    .fold(f32::INFINITY, f32::min),
                self.walls
                    .iter()
                    .map(|w| w.start.y.min(w.end.y))
                    .fold(f32::INFINITY, f32::min),
            ),
            Point::new(
                self.walls
                    .iter()
                    .map(|w| w.start.x.max(w.end.x))
                    .fold(f32::NEG_INFINITY, f32::max),
                self.walls
                    .iter()
                    .map(|w| w.start.y.max(w.end.y))
                    .fold(f32::NEG_INFINITY, f32::max),
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_creates_four_walls_and_connected_move() {
        let mut project = Project::default();
        let room_id = project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(4.0, 3.0),
                RoomKind::Bedroom,
            )
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
}
