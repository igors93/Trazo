//! The drawing entities of a project (lines, arcs, circles...): adding,
//! moving, finding and removing them, and keeping them valid in files.

use super::Project;
use crate::domain::drawing::{Drawing, Shape};
use crate::domain::geometry::Point;

impl Project {
    /// Adds a drawing entity; it must be a proper shape (two distinct
    /// points for a line, a radius for a circle...).
    pub fn add_drawing(&mut self, shape: Shape) -> Result<u64, String> {
        shape.validate()?;
        let id = self.id();
        self.drawings.push(Drawing { id, shape });
        Ok(id)
    }

    pub fn drawing(&self, id: u64) -> Option<&Drawing> {
        self.drawings.iter().find(|drawing| drawing.id == id)
    }

    /// Replaces the shape of a drawing, if the new one is valid.
    pub fn update_drawing(&mut self, id: u64, shape: Shape) -> Result<(), String> {
        shape.validate()?;
        let drawing = self
            .drawings
            .iter_mut()
            .find(|drawing| drawing.id == id)
            .ok_or("Desenho não encontrado.")?;
        drawing.shape = shape;
        Ok(())
    }

    pub fn move_drawing(&mut self, id: u64, delta: Point) {
        if let Some(drawing) = self.drawings.iter_mut().find(|drawing| drawing.id == id) {
            drawing.shape = drawing.shape.translated(delta);
        }
    }

    pub fn delete_drawing(&mut self, id: u64) -> bool {
        let before = self.drawings.len();
        self.drawings.retain(|drawing| drawing.id != id);
        self.drawings.len() < before
    }

    /// The drawing whose line passes within `tolerance` of `point`, the
    /// closest one (the last drawn when tied, as it is on top).
    pub fn drawing_at(&self, point: Point, tolerance: f64) -> Option<u64> {
        self.drawings
            .iter()
            .rev()
            .map(|drawing| (drawing.id, drawing.shape.distance_to(point)))
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Moves every drawing by `delta`, as the terrain and content move.
    pub(super) fn translate_drawings(&mut self, delta: Point) {
        for drawing in &mut self.drawings {
            drawing.shape = drawing.shape.translated(delta);
        }
    }

    /// The box around all drawings, if there are any.
    pub(super) fn drawings_bounds(&self) -> Option<(Point, Point)> {
        self.drawings
            .iter()
            .map(|drawing| drawing.shape.bounds())
            .reduce(|(low, high), (other_low, other_high)| {
                (
                    Point::new(low.x.min(other_low.x), low.y.min(other_low.y)),
                    Point::new(high.x.max(other_high.x), high.y.max(other_high.y)),
                )
            })
    }

    /// Drops drawings a file left invalid and rounds the rest to the stored
    /// resolution; returns how many were dropped and the highest id kept.
    pub(super) fn repair_drawings(&mut self) -> (usize, u64) {
        let before = self.drawings.len();
        self.drawings
            .retain(|drawing| drawing.shape.validate().is_ok());
        for drawing in &mut self.drawings {
            drawing
                .shape
                .for_each_point(|point| *point = point.rounded());
        }
        let highest = self
            .drawings
            .iter()
            .map(|drawing| drawing.id)
            .max()
            .unwrap_or(0);
        (before - self.drawings.len(), highest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawings_are_added_found_moved_and_kept_in_files() {
        let mut project = Project::default();
        let line = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 0.0),
            })
            .unwrap();
        let circle = project
            .add_drawing(Shape::Circle {
                center: Point::new(10.0, 0.0),
                radius: 1.0,
            })
            .unwrap();
        assert!(
            project
                .add_drawing(Shape::Line {
                    start: Point::ORIGIN,
                    end: Point::ORIGIN
                })
                .is_err()
        );
        assert_eq!(project.drawing_at(Point::new(2.0, 0.05), 0.1), Some(line));
        assert_eq!(project.drawing_at(Point::new(11.0, 0.0), 0.1), Some(circle));
        assert_eq!(project.drawing_at(Point::new(10.0, 0.0), 0.1), None);

        project.move_drawing(line, Point::new(0.0, 1.0));
        assert_eq!(project.drawing_at(Point::new(2.0, 1.0), 0.01), Some(line));
        // Drawings count in the extent of the plan.
        assert_eq!(project.bounds().unwrap().1, Point::new(11.0, 1.0));

        let json = serde_json::to_string(&project).unwrap();
        let mut loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate_and_repair().unwrap();
        assert_eq!(loaded.drawings, project.drawings);
        // New ids never reuse a drawing's.
        let next = loaded
            .add_drawing(Shape::rectangle(Point::ORIGIN, Point::new(1.0, 1.0)))
            .unwrap();
        assert!(next > circle);
        assert!(loaded.delete_drawing(line) && !loaded.delete_drawing(line));
    }
}
