use serde::{Deserialize, Serialize};

pub const GRID_METERS: f32 = 0.25;
pub const WALL_THICKNESS_METERS: f32 = 0.12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn snapped_to(self, step: f32) -> Self {
        Self::new(
            (self.x / step).round() * step,
            (self.y / step).round() * step,
        )
    }

    pub fn translated(self, delta: Self) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y)
    }
}

pub fn nearest_point_on_segment(point: Point, start: Point, end: Point) -> (Point, f32) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return (start, 0.0);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    (Point::new(start.x + t * dx, start.y + t * dy), t)
}

pub fn format_meters(value: f32) -> String {
    format!("{value:.2} m").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_stays_on_segment() {
        let (point, t) = nearest_point_on_segment(
            Point::new(5.0, 3.0),
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
        );
        assert_eq!(point, Point::new(4.0, 0.0));
        assert_eq!(t, 1.0);
    }
}
