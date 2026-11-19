//! Rooms of any shape: an L, a U or angled sides, drawn corner by corner.
//! Their walls are shared with neighbouring rooms just as rectangles are.

use super::{Project, Room};
use crate::domain::geometry::{self, POINT_TOLERANCE, Point};

impl Project {
    /// A room of any shape (an L, a U, angled sides) from its corners in
    /// order; walls are shared with neighbouring rooms like rectangles are.
    pub fn add_polygon_room(&mut self, corners: &[Point], name: &str) -> Result<u64, String> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err("Dê um nome ao cômodo.".to_owned());
        }
        if corners.iter().any(|corner| !corner.is_finite()) {
            return Err("Coordenadas inválidas.".to_owned());
        }
        let corners = geometry::simplify_polygon(corners);
        if corners.len() < 3 {
            return Err("Um cômodo precisa de pelo menos 3 cantos.".to_owned());
        }
        if !geometry::polygon_is_simple(&corners) {
            return Err("Os lados do cômodo não podem se cruzar.".to_owned());
        }
        let n = corners.len();
        if (0..n).any(|i| corners[i].distance(corners[(i + 1) % n]) < 0.24) {
            return Err("Cada lado do cômodo precisa de pelo menos 0,25 m.".to_owned());
        }
        if geometry::polygon_area(&corners) < 0.25 {
            return Err("O cômodo precisa de pelo menos 0,25 m².".to_owned());
        }
        if self
            .terrain
            .as_ref()
            .is_some_and(|terrain| corners.iter().any(|corner| !terrain.contains(*corner)))
        {
            return Err("O cômodo precisa caber dentro do terreno.".to_owned());
        }

        let mut tx = self.clone();
        let mut all_wall_ids = Vec::new();
        for index in 0..n {
            let edge_walls = tx
                .find_or_create_edge_walls(corners[index], corners[(index + 1) % n])
                .ok_or("Não foi possível criar as paredes do cômodo.")?;
            all_wall_ids.extend(edge_walls);
        }

        let id = tx.id();
        tx.rooms.push(Room {
            id,
            name,
            legacy_kind: None,
            wall_ids: all_wall_ids,
        });
        *self = tx;
        Ok(id)
    }

    /// The room's corners, without the extra ones where its walls were split.
    pub fn room_outline(&self, room: &Room) -> Option<Vec<Point>> {
        self.room_polygon(room)
            .map(|polygon| geometry::simplify_polygon(&polygon))
    }

    /// Whether the room is an upright rectangle, the shape whose width and
    /// length can be typed.
    pub fn room_is_rectangle(&self, room: &Room) -> bool {
        self.room_outline(room).is_some_and(|outline| {
            outline.len() == 4
                && (0..4).all(|i| {
                    let (a, b) = (outline[i], outline[(i + 1) % 4]);
                    (a.x - b.x).abs() < POINT_TOLERANCE || (a.y - b.y).abs() < POINT_TOLERANCE
                })
        })
    }

    /// Where the room's name goes, with the width and height free around
    /// that point.
    pub fn room_label_anchor(&self, room: &Room) -> Option<(Point, f64, f64)> {
        if !self.room_is_rectangle(room)
            && let Some(anchor) = self
                .room_outline(room)
                .and_then(|outline| geometry::interior_box(&outline))
        {
            return Some(anchor);
        }
        let (min, max) = self.room_bounds(room)?;
        Some((min.midpoint(max), max.x - min.x, max.y - min.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_of_any_shape_share_walls_like_rectangles() {
        let mut project = Project::default();
        // An L: 4 × 4 m less a 2 × 2 m corner.
        let l_room = project
            .add_polygon_room(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(4.0, 2.0),
                    Point::new(2.0, 2.0),
                    Point::new(2.0, 4.0),
                    Point::new(0.0, 4.0),
                ],
                "Sala em L",
            )
            .unwrap();
        let room = project.rooms.iter().find(|r| r.id == l_room).unwrap();
        assert_eq!(project.room_area(room), Some(12.0));
        assert!(!project.room_is_rectangle(room));
        let (anchor, _, _) = project.room_label_anchor(room).unwrap();
        assert!(geometry::polygon_contains_point(
            &project.room_polygon(room).unwrap(),
            anchor
        ));
        assert!(project.resize_room_dimensions(l_room, 5.0, 5.0).is_err());

        // A rectangle in the notch shares the two inner walls.
        project
            .add_room(Point::new(2.0, 2.0), Point::new(4.0, 4.0), "Lavabo")
            .unwrap();
        assert_eq!(project.walls.len(), 8);
        let lavabo = project.rooms.last().unwrap();
        assert!(project.room_is_rectangle(lavabo));

        // Crossing sides, tiny sides and a missing name are refused.
        let bowtie = [
            Point::new(10.0, 0.0),
            Point::new(14.0, 4.0),
            Point::new(14.0, 0.0),
            Point::new(10.0, 4.0),
        ];
        assert!(project.add_polygon_room(&bowtie, "X").is_err());
        let sliver = [
            Point::new(10.0, 0.0),
            Point::new(10.1, 0.0),
            Point::new(10.1, 3.0),
        ];
        assert!(project.add_polygon_room(&sliver, "X").is_err());
        assert!(project.add_polygon_room(&bowtie[..3], " ").is_err());
        assert_eq!(project.rooms.len(), 2);
    }
}
