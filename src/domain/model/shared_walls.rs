//! Moving walls (including partitions shared between rooms) and resizing
//! rectangular rooms while keeping adjacent rooms, openings and dimensions
//! consistent in a single transaction.

use super::Project;
use crate::domain::geometry::{JOIN_TOLERANCE, POINT_TOLERANCE, Point};

const MIN_WALL_LENGTH: f64 = 0.25;

impl Project {
    /// Moves wall `id` by `delta` (clamped to terrain bounds), stretching or
    /// shrinking any connected walls, preserving physical opening positions
    /// and validating every affected room transactionally.
    pub fn move_wall_checked(&mut self, id: u64, delta: Point) -> Result<(), String> {
        let Some(index) = self.walls.iter().position(|wall| wall.id == id) else {
            return Err("Parede não encontrada.".to_owned());
        };
        if !delta.is_finite() {
            return Err("Deslocamento inválido.".to_owned());
        }

        let old_start = self.walls[index].start;
        let old_end = self.walls[index].end;
        let anchors = [old_start, old_end];
        self.translate_wall_anchors(&[id], &anchors, delta, true)
    }

    /// Moves wall `id` by `delta`, ignoring the move if it would break any
    /// opening or connected room.
    pub fn move_wall(&mut self, id: u64, delta: Point) {
        let _ = self.move_wall_checked(id, delta);
    }

    /// Translates every wall endpoint matching any point in `anchors` (plus
    /// every wall in `primary_walls`) by `delta`, preserving physical opening
    /// positions on stretched/shrunk adjacent walls and validating all
    /// affected rooms.
    pub(super) fn translate_wall_anchors(
        &mut self,
        primary_walls: &[u64],
        anchors: &[Point],
        delta: Point,
        clamp_to_terrain: bool,
    ) -> Result<(), String> {
        let matches_anchor = |pt: Point| {
            anchors
                .iter()
                .any(|anchor| pt.near(*anchor, JOIN_TOLERANCE))
        };

        let mut moving_points: Vec<Point> = anchors.to_vec();
        for wall in &self.walls {
            if primary_walls.contains(&wall.id) {
                moving_points.push(wall.start);
                moving_points.push(wall.end);
            } else {
                if matches_anchor(wall.start) {
                    moving_points.push(wall.start);
                }
                if matches_anchor(wall.end) {
                    moving_points.push(wall.end);
                }
            }
        }
        if moving_points.is_empty() {
            return Ok(());
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

        let delta = if clamp_to_terrain {
            self.clamp_delta_to_terrain(Point::new(min_x, min_y), Point::new(max_x, max_y), delta)
        } else {
            if let Some(terrain) = &self.terrain {
                let moved_min = Point::new(min_x + delta.x, min_y + delta.y);
                let moved_max = Point::new(max_x + delta.x, max_y + delta.y);
                if !terrain.contains(moved_min) || !terrain.contains(moved_max) {
                    return Err("As novas dimensões ultrapassam os limites do terreno.".to_owned());
                }
            }
            delta
        };

        if delta.x.abs() <= 1e-9 && delta.y.abs() <= 1e-9 {
            return Ok(());
        }

        let mut tx = self.clone();
        let mut affected_walls: Vec<u64> = Vec::new();
        // (wall_id, old_len, start_moved, end_moved) for walls that resized.
        let mut resized_walls: Vec<(u64, f64, bool, bool)> = Vec::new();

        for wall in &mut tx.walls {
            let old_len = wall.length();
            let is_primary = primary_walls.contains(&wall.id);
            let start_moved = is_primary || matches_anchor(wall.start);
            let end_moved = is_primary || matches_anchor(wall.end);
            if !start_moved && !end_moved {
                continue;
            }
            if start_moved {
                wall.start = wall.start.translated(delta).rounded();
            }
            if end_moved {
                wall.end = wall.end.translated(delta).rounded();
            }
            if wall.length() < MIN_WALL_LENGTH - POINT_TOLERANCE {
                return Err(
                    "Mover a parede reduziria uma parede ligada para menos de 0,25 m.".to_owned(),
                );
            }
            affected_walls.push(wall.id);
            if start_moved != end_moved {
                resized_walls.push((wall.id, old_len, start_moved, end_moved));
            }
        }

        // Preserve physical positions of openings on stationary portions of resized walls.
        for (wall_id, old_len, start_moved, end_moved) in resized_walls {
            let Some(wall) = tx.walls.iter().find(|w| w.id == wall_id) else {
                continue;
            };
            let new_len = wall.length();
            if new_len <= POINT_TOLERANCE {
                continue;
            }
            for op in tx.openings.iter_mut().filter(|o| o.wall_id == wall_id) {
                if !start_moved && end_moved {
                    let dist_from_start = op.t * old_len;
                    op.t = dist_from_start / new_len;
                } else if start_moved && !end_moved {
                    let dist_from_end = (1.0 - op.t) * old_len;
                    op.t = 1.0 - dist_from_end / new_len;
                }
            }
        }

        tx.revalidate_openings_on_walls(&affected_walls)?;
        tx.validate_affected_rooms(&affected_walls)?;
        *self = tx;
        Ok(())
    }

    /// Resizes a rectangular room to `new_width × new_height` (keeping its
    /// min-X / min-Y corner anchored). Works both for standalone rooms and
    /// for rooms sharing walls with neighbouring rooms, adjusting all
    /// connected rooms and preserving openings transactionally.
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

        if !self.room_is_rectangle(&self.rooms[room_idx]) {
            return Err(
                "Este cômodo não é retangular: ajuste a forma pelos cantos e paredes.".to_owned(),
            );
        }

        let bounds = self
            .room_bounds(&self.rooms[room_idx])
            .ok_or("Geometria do cômodo inválida.")?;
        let old_width = bounds.1.x - bounds.0.x;
        let old_height = bounds.1.y - bounds.0.y;
        let dx = new_width - old_width;
        let dy = new_height - old_height;

        if dx.abs() <= POINT_TOLERANCE && dy.abs() <= POINT_TOLERANCE {
            return Ok(());
        }

        let mut tx = self.clone();
        let wall_ids = tx.rooms[room_idx].wall_ids.clone();

        // Step 1: adjust width by sliding the room's right-edge wall(s) by `(dx, 0)`.
        if dx.abs() > POINT_TOLERANCE {
            let mut right_walls = Vec::new();
            let mut right_anchors = Vec::new();
            for &wid in &wall_ids {
                let Some(wall) = tx.walls.iter().find(|w| w.id == wid) else {
                    continue;
                };
                if (wall.start.x - bounds.1.x).abs() < JOIN_TOLERANCE
                    && (wall.end.x - bounds.1.x).abs() < JOIN_TOLERANCE
                {
                    right_walls.push(wid);
                    right_anchors.push(wall.start);
                    right_anchors.push(wall.end);
                }
            }
            if right_walls.is_empty() {
                return Err(
                    "Não foi possível localizar a parede direita do cômodo para redimensionar."
                        .to_owned(),
                );
            }
            tx.translate_wall_anchors(&right_walls, &right_anchors, Point::new(dx, 0.0), false)?;
        }

        // Step 2: adjust height by sliding the room's max-Y edge wall(s) by `(0, dy)`.
        if dy.abs() > POINT_TOLERANCE {
            let mut bottom_walls = Vec::new();
            let mut bottom_anchors = Vec::new();
            for &wid in &wall_ids {
                let Some(wall) = tx.walls.iter().find(|w| w.id == wid) else {
                    continue;
                };
                if (wall.start.y - bounds.1.y).abs() < JOIN_TOLERANCE
                    && (wall.end.y - bounds.1.y).abs() < JOIN_TOLERANCE
                {
                    bottom_walls.push(wid);
                    bottom_anchors.push(wall.start);
                    bottom_anchors.push(wall.end);
                }
            }
            if bottom_walls.is_empty() {
                return Err(
                    "Não foi possível localizar a parede inferior do cômodo para redimensionar."
                        .to_owned(),
                );
            }
            tx.translate_wall_anchors(&bottom_walls, &bottom_anchors, Point::new(0.0, dy), false)?;
        }

        *self = tx;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{annotations::building_dimensions, model::OpeningKind};

    #[test]
    fn moving_shared_wall_updates_both_rooms_preserves_openings_and_updates_dimensions() {
        let mut project = Project::default();
        let q1 = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto 1")
            .unwrap();
        let q2 = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto 2")
            .unwrap();

        // Find the shared vertical wall at x = 4.0 and put a door on it.
        let shared_wall = project
            .walls
            .iter()
            .find(|w| (w.start.x - 4.0).abs() < 1e-4 && (w.end.x - 4.0).abs() < 1e-4)
            .unwrap()
            .id;
        let shared_door = project
            .add_opening(shared_wall, OpeningKind::Door, 0.5)
            .unwrap();

        // Also put a window on Quarto 2's top wall (4, 0)-(8, 0) at x = 6.0 (t = 0.5).
        let q2_top = project
            .walls
            .iter()
            .find(|w| {
                (w.start.y).abs() < 1e-4 && (w.end.y).abs() < 1e-4 && w.start.x.min(w.end.x) >= 3.9
            })
            .unwrap()
            .id;
        let q2_window = project
            .add_opening(q2_top, OpeningKind::Window, 0.5)
            .unwrap();

        // Move the shared wall 40 cm to the right (+0.40 m).
        project
            .move_wall_checked(shared_wall, Point::new(0.40, 0.0))
            .unwrap();

        let r1 = project.rooms.iter().find(|r| r.id == q1).unwrap();
        let r2 = project.rooms.iter().find(|r| r.id == q2).unwrap();
        assert!((project.room_area(r1).unwrap() - 13.20).abs() < 1e-3);
        assert!((project.room_area(r2).unwrap() - 10.80).abs() < 1e-3);

        // Both openings survive and the window on Quarto 2 stays at x = 6.0 m.
        assert!(project.openings.iter().any(|o| o.id == shared_door));
        let win = project.openings.iter().find(|o| o.id == q2_window).unwrap();
        let top_wall = project.walls.iter().find(|w| w.id == q2_top).unwrap();
        let win_pos = top_wall.start.lerp(top_wall.end, win.t);
        assert!((win_pos.x - 6.0).abs() < 1e-3);

        // Automatic dimensions reflect the moved wall at x = 4.40 m.
        let dims = building_dimensions(&project).unwrap();
        assert!(dims.xs.iter().any(|&x| (x - 4.40).abs() < 1e-3));

        // Moving too far right (collapsing Quarto 2 or crushing its 1.20 m window) is rejected.
        assert!(
            project
                .move_wall_checked(shared_wall, Point::new(3.0, 0.0))
                .is_err()
        );
        let r1 = project.rooms.iter().find(|r| r.id == q1).unwrap();
        assert!((project.room_area(r1).unwrap() - 13.20).abs() < 1e-3);
    }

    #[test]
    fn resizing_room_with_shared_wall_adjusts_neighbour_transactionally() {
        let mut project = Project::default();
        let q1 = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto 1")
            .unwrap();
        let q2 = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto 2")
            .unwrap();

        // Grow Quarto 1 width from 4.00 m to 4.50 m -> Quarto 2 shrinks to 3.50 m.
        project.resize_room_dimensions(q1, 4.5, 3.0).unwrap();
        let r1 = project.rooms.iter().find(|r| r.id == q1).unwrap();
        let r2 = project.rooms.iter().find(|r| r.id == q2).unwrap();
        assert!((project.room_area(r1).unwrap() - 13.5).abs() < 1e-3);
        assert!((project.room_area(r2).unwrap() - 10.5).abs() < 1e-3);

        // Attempting to grow Quarto 1 past Quarto 2's wall (width = 7.90 m) fails cleanly.
        assert!(project.resize_room_dimensions(q1, 7.9, 3.0).is_err());
        let r2 = project.rooms.iter().find(|r| r.id == q2).unwrap();
        assert!((project.room_area(r2).unwrap() - 10.5).abs() < 1e-3);
    }

    #[test]
    fn resizing_room_with_split_shared_edge_updates_all_three_rooms_and_blocks_opening_overlap() {
        let mut project = Project::default();
        // Left room (0..4 x 0..4) meets two stacked rooms on the right (4..8 x 0..2 and 4..8 x 2..4),
        // so its right edge is split into two wall segments.
        let left = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        let top_right = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 2.0), "Banho")
            .unwrap();
        let bottom_right = project
            .add_room(Point::new(4.0, 2.0), Point::new(8.0, 4.0), "Lavanderia")
            .unwrap();

        project.resize_room_dimensions(left, 4.5, 4.0).unwrap();
        let area_of = |p: &Project, id: u64| {
            let room = p.rooms.iter().find(|r| r.id == id).unwrap();
            p.room_area(room).unwrap()
        };
        assert!((area_of(&project, left) - 18.0).abs() < 1e-3);
        assert!((area_of(&project, top_right) - 7.0).abs() < 1e-3);
        assert!((area_of(&project, bottom_right) - 7.0).abs() < 1e-3);

        // Put two windows on Banho's top wall (4.5..8.0, length 3.5 m).
        let banho_top = project
            .walls
            .iter()
            .find(|w| {
                w.start.y.abs() < 1e-4 && w.end.y.abs() < 1e-4 && w.start.x.min(w.end.x) >= 4.4
            })
            .unwrap()
            .id;
        project
            .add_opening_sized(banho_top, OpeningKind::Window, 0.25, Some(1.0))
            .unwrap();
        project
            .add_opening_sized(banho_top, OpeningKind::Window, 0.75, Some(1.0))
            .unwrap();

        // Shrinking Banho's width to 1.80 m (left width 6.20 m) cannot fit two 1.0 m windows + margins.
        assert!(project.resize_room_dimensions(left, 6.2, 4.0).is_err());
        assert!((area_of(&project, top_right) - 7.0).abs() < 1e-3);
    }

    #[test]
    fn moving_horizontal_shared_wall_updates_stacked_rooms_and_y_dimensions() {
        let mut project = Project::default();
        let top = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Cozinha")
            .unwrap();
        let bottom = project
            .add_room(Point::new(0.0, 3.0), Point::new(4.0, 6.0), "Copa")
            .unwrap();
        let shared = project
            .walls
            .iter()
            .find(|w| (w.start.y - 3.0).abs() < 1e-4 && (w.end.y - 3.0).abs() < 1e-4)
            .unwrap()
            .id;
        project
            .move_wall_checked(shared, Point::new(0.0, -0.5))
            .unwrap();
        let r_top = project.rooms.iter().find(|r| r.id == top).unwrap();
        let r_bottom = project.rooms.iter().find(|r| r.id == bottom).unwrap();
        assert!((project.room_area(r_top).unwrap() - 10.0).abs() < 1e-3);
        assert!((project.room_area(r_bottom).unwrap() - 14.0).abs() < 1e-3);
        let dims = building_dimensions(&project).unwrap();
        assert!(dims.ys.iter().any(|&y| (y - 2.5).abs() < 1e-3));
    }
}
