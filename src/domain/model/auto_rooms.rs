use std::collections::HashSet;

use super::{Project, Room};
use crate::domain::geometry::{
    JOIN_TOLERANCE, Point, polygon_area, polygon_centroid, polygon_is_simple,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedRoom {
    pub corners: Vec<Point>,
    pub wall_ids: Vec<u64>,
    pub area: f64,
}

impl Project {
    /// Finds the bounded faces formed by connected wall endpoints. The
    /// half-edge walk returns each interior face once and ignores the outer
    /// unbounded face.
    pub fn detect_enclosed_rooms(&self) -> Vec<DetectedRoom> {
        let mut nodes: Vec<Point> = Vec::new();
        let mut node_for = |point: Point| {
            if let Some(index) = nodes
                .iter()
                .position(|candidate| candidate.near(point, JOIN_TOLERANCE))
            {
                index
            } else {
                nodes.push(point);
                nodes.len() - 1
            }
        };
        let mut edges = Vec::new();
        for wall in &self.walls {
            let start = node_for(wall.start);
            let end = node_for(wall.end);
            if start != end {
                edges.push((start, end, wall.id));
            }
        }
        drop(node_for);

        let mut adjacency: Vec<Vec<(usize, u64)>> = vec![Vec::new(); nodes.len()];
        for &(start, end, wall_id) in &edges {
            adjacency[start].push((end, wall_id));
            adjacency[end].push((start, wall_id));
        }
        for (index, neighbours) in adjacency.iter_mut().enumerate() {
            neighbours.sort_by(|(left, _), (right, _)| {
                let left = nodes[*left].minus(nodes[index]);
                let right = nodes[*right].minus(nodes[index]);
                left.y.atan2(left.x).total_cmp(&right.y.atan2(right.x))
            });
        }

        let mut visited = HashSet::new();
        let mut found = Vec::new();
        for &(start_node, end_node, start_wall) in &edges {
            for start in [
                (start_node, end_node, start_wall),
                (end_node, start_node, start_wall),
            ] {
                if visited.contains(&start) {
                    continue;
                }
                let mut current = start;
                let mut corners = Vec::new();
                let mut wall_ids = Vec::new();
                let mut local = HashSet::new();
                let mut closed = false;
                for _ in 0..edges.len().saturating_mul(2).max(1) {
                    if !local.insert(current) {
                        break;
                    }
                    visited.insert(current);
                    corners.push(nodes[current.0]);
                    wall_ids.push(current.2);
                    let neighbours = &adjacency[current.1];
                    let Some(reverse) = neighbours
                        .iter()
                        .position(|&(node, wall)| node == current.0 && wall == current.2)
                    else {
                        break;
                    };
                    // Previous angular neighbour keeps the face on the right
                    // in screen coordinates (where Y grows down).
                    let next = neighbours[(reverse + neighbours.len() - 1) % neighbours.len()];
                    current = (current.1, next.0, next.1);
                    if current == start {
                        closed = true;
                        break;
                    }
                }
                if !closed || corners.len() < 3 || !polygon_is_simple(&corners) {
                    continue;
                }
                let signed_twice: f64 = (0..corners.len())
                    .map(|index| {
                        let next = (index + 1) % corners.len();
                        corners[index].cross(corners[next])
                    })
                    .sum();
                if signed_twice <= 0.02 {
                    continue;
                }
                let area = polygon_area(&corners);
                let mut unique_walls = wall_ids.clone();
                unique_walls.sort_unstable();
                unique_walls.dedup();
                let already_exists = self.rooms.iter().any(|room| {
                    let mut existing = room.wall_ids.clone();
                    existing.sort_unstable();
                    existing.dedup();
                    existing == unique_walls
                        || self.room_polygon(room).is_some_and(|polygon| {
                            (polygon_area(&polygon) - area).abs() < 0.01
                                && polygon_centroid(&polygon).near(polygon_centroid(&corners), 0.01)
                        })
                });
                if !already_exists {
                    found.push(DetectedRoom {
                        corners,
                        wall_ids,
                        area,
                    });
                }
            }
        }
        found.sort_by(|left, right| {
            polygon_centroid(&left.corners)
                .y
                .total_cmp(&polygon_centroid(&right.corners).y)
                .then(
                    polygon_centroid(&left.corners)
                        .x
                        .total_cmp(&polygon_centroid(&right.corners).x),
                )
        });
        found
    }

    /// Creates named room entities for every newly detected bounded face.
    pub fn create_rooms_from_enclosed_walls(&mut self) -> usize {
        let detected = self.detect_enclosed_rooms();
        let first_number = self.rooms.len() + 1;
        for (index, detected) in detected.iter().enumerate() {
            let id = self.id();
            self.rooms.push(Room {
                id,
                name: format!("Ambiente {}", first_number + index),
                legacy_kind: None,
                wall_ids: detected.wall_ids.clone(),
                hatch: None,
            });
        }
        detected.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_two_faces_that_share_a_wall_without_creating_duplicates() {
        let mut project = Project::default();
        for (start, end) in [
            (Point::new(0.0, 0.0), Point::new(8.0, 0.0)),
            (Point::new(8.0, 0.0), Point::new(8.0, 3.0)),
            (Point::new(8.0, 3.0), Point::new(0.0, 3.0)),
            (Point::new(0.0, 3.0), Point::new(0.0, 0.0)),
            (Point::new(4.0, 0.0), Point::new(4.0, 3.0)),
        ] {
            project.add_wall(start, end).unwrap();
        }
        // Split the long top and bottom walls at the partition endpoints.
        let top = project.walls[0].id;
        let bottom = project.walls[2].id;
        project.split_wall_at(top, Point::new(4.0, 0.0)).unwrap();
        project.split_wall_at(bottom, Point::new(4.0, 3.0)).unwrap();

        let detected = project.detect_enclosed_rooms();
        assert_eq!(detected.len(), 2);
        assert!(detected.iter().all(|room| (room.area - 12.0).abs() < 1e-6));
        assert_eq!(project.create_rooms_from_enclosed_walls(), 2);
        assert_eq!(project.create_rooms_from_enclosed_walls(), 0);
        assert_eq!(project.rooms.len(), 2);
    }
}
