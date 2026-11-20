//! Architectural area calculations kept separate by their planning meaning.

use serde::{Deserialize, Serialize};

use super::{
    geometry::{Point, line_intersection, polygon_area, polygon_is_simple},
    model::{Project, Room},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AreaSummary {
    /// Net floor area measured to the finished inner faces of walls.
    pub usable: f64,
    /// Plan area occupied by wall layers.
    pub walls: f64,
    /// Net floor plus wall footprint for every floor currently in the model.
    pub constructed: f64,
    /// Horizontal building footprint. It equals constructed area in a one-floor project.
    pub projection: f64,
    /// Gross terrain polygon area.
    pub terrain: f64,
}

fn signed_twice_area(points: &[Point]) -> f64 {
    (0..points.len())
        .map(|index| {
            let a = points[index];
            let b = points[(index + 1) % points.len()];
            a.x * b.y - b.x * a.y
        })
        .sum()
}

impl Project {
    /// Polygon measured along the finished inner wall faces.
    pub fn room_usable_polygon(&self, room: &Room) -> Option<Vec<Point>> {
        let outline = self.room_polygon(room)?;
        if outline.len() < 3 {
            return None;
        }
        let direction = if signed_twice_area(&outline) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let mut shifted = Vec::with_capacity(outline.len());
        for index in 0..outline.len() {
            let a = outline[index];
            let b = outline[(index + 1) % outline.len()];
            let wall = self.walls.iter().find(|wall| {
                (wall.start.near(a, 0.05) && wall.end.near(b, 0.05))
                    || (wall.start.near(b, 0.05) && wall.end.near(a, 0.05))
            })?;
            let delta = b.minus(a);
            let length = delta.length();
            if length <= 1e-9 {
                return None;
            }
            let half = wall.thickness() / 2.0;
            let normal = Point::new(
                -delta.y / length * direction * half,
                delta.x / length * direction * half,
            );
            shifted.push((a.translated(normal), b.translated(normal)));
        }

        let mut inner = Vec::with_capacity(shifted.len());
        for index in 0..shifted.len() {
            let previous = shifted[(index + shifted.len() - 1) % shifted.len()];
            let current = shifted[index];
            let (corner, _, _) = line_intersection(previous.0, previous.1, current.0, current.1)?;
            if !corner.is_finite() {
                return None;
            }
            inner.push(corner);
        }
        (polygon_is_simple(&inner) && polygon_area(&inner) > 0.01).then_some(inner)
    }

    pub fn room_usable_area(&self, room: &Room) -> Option<f64> {
        self.room_usable_polygon(room)
            .map(|polygon| polygon_area(&polygon))
            .or_else(|| self.room_area(room))
    }

    pub fn area_summary(&self) -> AreaSummary {
        // `fold` from +0.0: an empty `sum()` of floats is -0.0, which shows
        // as "-0,00 m²" in an empty project.
        let usable = self
            .rooms
            .iter()
            .filter_map(|room| self.room_usable_area(room))
            .fold(0.0, |total, area| total + area);
        let walls = self
            .walls
            .iter()
            .map(|wall| wall.length() * wall.thickness())
            .fold(0.0, |total, area| total + area);
        let constructed = usable + walls;
        AreaSummary {
            usable,
            walls,
            constructed,
            projection: constructed,
            terrain: self.terrain.as_ref().map_or(0.0, |terrain| terrain.area()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_net_wall_constructed_projection_and_terrain_areas() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
            .unwrap();

        let summary = project.area_summary();
        let thickness = project.masonry.thickness();
        assert!((summary.usable - (4.0 - thickness) * (3.0 - thickness)).abs() < 1e-6);
        assert!((summary.walls - 14.0 * thickness).abs() < 1e-6);
        assert!((summary.constructed - summary.usable - summary.walls).abs() < 1e-6);
        assert_eq!(summary.projection, summary.constructed);
        assert_eq!(summary.terrain, 196.0);
    }

    #[test]
    fn files_without_rooms_report_wall_and_terrain_areas_independently() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_wall(Point::new(1.0, 1.0), Point::new(6.0, 1.0))
            .unwrap();
        let summary = project.area_summary();
        assert_eq!(summary.usable, 0.0);
        assert!(summary.constructed > 0.0);
        assert_eq!(summary.terrain, 200.0);
    }

    #[test]
    fn an_empty_project_has_positive_zero_areas() {
        let summary = Project::default().area_summary();
        for value in [
            summary.usable,
            summary.walls,
            summary.constructed,
            summary.projection,
        ] {
            assert!(value == 0.0 && value.is_sign_positive(), "{value}");
        }
    }
}
