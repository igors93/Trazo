//! Configurable building setbacks and the derived buildable limit.

use serde::{Deserialize, Serialize};

use super::{
    geometry::distance_to_segment,
    model::{Project, Terrain},
};
use crate::domain::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Setbacks {
    pub front: f64,
    pub rear: f64,
    pub side: f64,
}

impl Default for Setbacks {
    fn default() -> Self {
        Self {
            front: 0.0,
            rear: 0.0,
            side: 0.0,
        }
    }
}

impl Setbacks {
    pub fn is_active(self) -> bool {
        self.front > 0.0 || self.rear > 0.0 || self.side > 0.0
    }

    pub fn validate(self) -> Result<Self, String> {
        if [self.front, self.rear, self.side]
            .into_iter()
            .any(|value| !value.is_finite() || !(0.0..=1000.0).contains(&value))
        {
            return Err("Os recuos devem estar entre 0 e 1.000 m.".to_owned());
        }
        Ok(self)
    }
}

impl Terrain {
    fn setback_for_edge(&self, a: Point, b: Point, setbacks: Setbacks) -> f64 {
        let midpoint = a.lerp(b, 0.5);
        let distances = [
            (midpoint.y - self.min.y).abs(),
            (self.max.y - midpoint.y).abs(),
            (midpoint.x - self.min.x).abs(),
            (self.max.x - midpoint.x).abs(),
        ];
        let closest = distances
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(index, _)| index)
            .unwrap_or(2);
        match closest {
            0 => setbacks.front,
            1 => setbacks.rear,
            _ => setbacks.side,
        }
    }

    /// True when a point is inside the terrain and respects every applicable
    /// boundary distance. This also works for simple concave terrain polygons.
    pub fn contains_with_setbacks(&self, point: Point, setbacks: Setbacks) -> bool {
        if !self.contains(point) {
            return false;
        }
        let outline = self.outline();
        (0..outline.len()).all(|index| {
            let a = outline[index];
            let b = outline[(index + 1) % outline.len()];
            distance_to_segment(point, a, b) + 1e-6 >= self.setback_for_edge(a, b, setbacks)
        })
    }

    /// Parallel segments used to draw the configured building limit.
    pub fn setback_segments(&self, setbacks: Setbacks) -> Vec<(Point, Point)> {
        if !setbacks.is_active() {
            return Vec::new();
        }
        let outline = self.outline();
        let signed_twice_area: f64 = (0..outline.len())
            .map(|index| {
                let a = outline[index];
                let b = outline[(index + 1) % outline.len()];
                a.x * b.y - b.x * a.y
            })
            .sum();
        let normal_sign = if signed_twice_area >= 0.0 { 1.0 } else { -1.0 };

        (0..outline.len())
            .filter_map(|index| {
                let a = outline[index];
                let b = outline[(index + 1) % outline.len()];
                let delta = b.minus(a);
                let length = delta.length();
                if length <= 1e-9 {
                    return None;
                }
                let amount = self.setback_for_edge(a, b, setbacks);
                let normal = Point::new(
                    -delta.y / length * normal_sign * amount,
                    delta.x / length * normal_sign * amount,
                );
                Some((a.translated(normal), b.translated(normal)))
            })
            .collect()
    }
}

impl Project {
    pub fn set_setbacks(&mut self, setbacks: Setbacks) -> Result<(), String> {
        self.setbacks = setbacks.validate()?;
        Ok(())
    }

    pub fn point_inside_buildable_limit(&self, point: Point) -> bool {
        self.terrain
            .as_ref()
            .is_none_or(|terrain| terrain.contains_with_setbacks(point, self.setbacks))
    }

    pub fn content_inside_buildable_limit(&self) -> bool {
        self.walls.iter().all(|wall| {
            self.point_inside_buildable_limit(wall.start)
                && self.point_inside_buildable_limit(wall.end)
                && self.point_inside_buildable_limit(wall.start.lerp(wall.end, 0.5))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangular_setbacks_define_and_draw_the_buildable_limit() {
        let terrain = Terrain {
            id: 1,
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 20.0),
            vertices: Vec::new(),
        };
        let setbacks = Setbacks {
            front: 5.0,
            rear: 3.0,
            side: 1.5,
        };

        assert!(terrain.contains_with_setbacks(Point::new(5.0, 10.0), setbacks));
        assert!(!terrain.contains_with_setbacks(Point::new(5.0, 4.99), setbacks));
        assert!(!terrain.contains_with_setbacks(Point::new(1.49, 10.0), setbacks));

        let segments = terrain.setback_segments(setbacks);
        assert_eq!(segments.len(), 4);
        assert!(segments.iter().any(|(a, b)| a.y == 5.0 && b.y == 5.0));
        assert!(segments.iter().any(|(a, b)| a.y == 17.0 && b.y == 17.0));
    }

    #[test]
    fn inclined_boundary_uses_the_side_distance() {
        let terrain = Terrain {
            id: 1,
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(8.0, 10.0),
                Point::new(0.0, 10.0),
            ],
        };
        let setbacks = Setbacks {
            front: 1.0,
            rear: 1.0,
            side: 2.0,
        };
        assert!(!terrain.contains_with_setbacks(Point::new(8.0, 5.0), setbacks));
        assert!(terrain.contains_with_setbacks(Point::new(5.0, 5.0), setbacks));
    }

    #[test]
    fn configured_limit_rejects_new_walls_in_the_setback_strip() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(20.0, 20.0));
        project
            .set_setbacks(Setbacks {
                front: 3.0,
                rear: 2.0,
                side: 1.5,
            })
            .unwrap();

        assert!(
            project
                .add_wall(Point::new(1.0, 5.0), Point::new(1.0, 10.0))
                .is_none()
        );
        assert!(
            project
                .add_wall(Point::new(2.0, 5.0), Point::new(18.0, 5.0))
                .is_some()
        );
    }
}
