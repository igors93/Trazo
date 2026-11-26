//! The canvas view: fitting the plan, zooming, and the bounds used to
//! frame a selection.

use eframe::egui;

use super::{FloorPlanApp, Selection};
use crate::domain::geometry::Point;

impl FloorPlanApp {
    pub(super) fn fit_view_requested(&mut self) {
        self.fit_pending = true;
    }

    pub(super) fn fit_view(&mut self, rect: egui::Rect) {
        self.fit_pending = false;
        if let Some((min, max)) = self.project.bounds() {
            self.fit_bounds(rect, min, max, 240.0);
        } else {
            self.pan = egui::Vec2::ZERO;
            self.scale = 60.0;
        }
    }

    /// Centres the box `min`–`max` in `rect` at the largest scale that shows
    /// it whole, never above `max_scale`.
    pub(super) fn fit_bounds(&mut self, rect: egui::Rect, min: Point, max: Point, max_scale: f32) {
        let width = (max.x - min.x).max(0.25) as f32;
        let height = (max.y - min.y).max(0.25) as f32;
        self.scale = ((rect.width() - 100.0).max(80.0) / width)
            .min((rect.height() - 100.0).max(80.0) / height)
            .clamp(3.0, max_scale);
        let scale = f64::from(self.scale);
        self.pan = egui::vec2(
            (-(min.x + max.x) * scale / 2.0) as f32,
            (-(min.y + max.y) * scale / 2.0) as f32,
        );
    }

    /// A distance on screen, in metres at the current zoom.
    pub(super) fn screen_metres(&self, pixels: f32) -> f64 {
        f64::from(pixels) / f64::from(self.scale)
    }

    pub(super) fn prepare_canvas_view(&mut self, rect: egui::Rect) {
        if self.fit_pending {
            self.fit_view(rect);
        } else if let Some((min, max)) = self.zoom_target.take() {
            self.fit_bounds(rect, min, max, 120.0);
        } else if let Some(previous_center) = self.last_canvas_center {
            self.pan += previous_center - rect.center();
        }
        self.last_canvas_center = Some(rect.center());
        self.last_canvas_rect = Some(rect);
    }

    /// Zooms by `factor` around the centre of the view.
    pub(super) fn zoom_by(&mut self, factor: f32) {
        let scale = (self.scale * factor).clamp(3.0, 240.0);
        self.pan *= scale / self.scale;
        self.scale = scale;
    }

    pub(super) fn zoom_percent(&self) -> f32 {
        self.scale / 60.0 * 100.0
    }

    /// The box an element occupies, for zooming to it.
    pub(super) fn selection_bounds(&self, selection: Selection) -> Option<(Point, Point)> {
        let span = |points: &[Point]| {
            let mut min = points[0];
            let mut max = points[0];
            for point in points {
                min = Point::new(min.x.min(point.x), min.y.min(point.y));
                max = Point::new(max.x.max(point.x), max.y.max(point.y));
            }
            (
                Point::new(min.x - 0.75, min.y - 0.75),
                Point::new(max.x + 0.75, max.y + 0.75),
            )
        };
        match selection {
            Selection::Wall(id) => {
                let wall = self.project.walls.iter().find(|wall| wall.id == id)?;
                Some(span(&[wall.start, wall.end]))
            }
            Selection::Room(id) => {
                let room = self.project.rooms.iter().find(|room| room.id == id)?;
                let (min, max) = self.project.room_bounds(room)?;
                Some(span(&[min, max]))
            }
            Selection::Terrain(_) => {
                let terrain = self.project.terrain.as_ref()?;
                Some(span(&[terrain.min, terrain.max]))
            }
            Selection::Fixture(id) => {
                let fixture = self.project.fixtures.iter().find(|f| f.id == id)?;
                let (low, high) = fixture.visual_bounds();
                Some(span(&[low, high]))
            }
            Selection::Drawing(id) => {
                let (low, high) = self.project.drawing(id)?.shape.bounds();
                Some(span(&[low, high]))
            }
            Selection::Opening(id) => {
                let opening = self.project.openings.iter().find(|o| o.id == id)?;
                let center = self.opening_center(id)?;
                let half = opening.width() / 2.0 + 1.0;
                Some(span(&[
                    Point::new(center.x - half, center.y - half),
                    Point::new(center.x + half, center.y + half),
                ]))
            }
        }
    }

    /// The middle of an opening, on its wall's centre line.
    pub(super) fn opening_center(&self, id: u64) -> Option<Point> {
        let opening = self.project.openings.iter().find(|o| o.id == id)?;
        let wall = self
            .project
            .walls
            .iter()
            .find(|wall| wall.id == opening.wall_id)?;
        Some(Point::new(
            wall.start.x + (wall.end.x - wall.start.x) * opening.t,
            wall.start.y + (wall.end.y - wall.start.y) * opening.t,
        ))
    }

    /// Zooms to `selection`, or to everything selected when it is part of
    /// a selection of several elements.
    pub(super) fn zoom_to_selection(&mut self, selection: Selection) {
        let bounds = if self.is_multi_selection() && self.is_selected(selection) {
            self.project
                .elements_bounds(&self.selected())
                .map(|(low, high)| {
                    (
                        Point::new(low.x - 0.75, low.y - 0.75),
                        Point::new(high.x + 0.75, high.y + 0.75),
                    )
                })
        } else {
            self.selection_bounds(selection)
        };
        if let Some(bounds) = bounds {
            self.zoom_target = Some(bounds);
        }
    }

    /// Turns light mode on or off and remembers the choice (#164).
    pub(super) fn set_light_mode(&mut self, on: bool) {
        self.light_mode = on;
        self.light_mode_chosen = Some(on);
        self.apply_light_mode();
        self.message = if on {
            "Modo leve ligado: sem animações contínuas e com menos memória para desfazer."
                .to_owned()
        } else {
            "Modo leve desligado.".to_owned()
        };
    }

    /// Applies light mode to what it changes outside the frame loop.
    pub(super) fn apply_light_mode(&self) {
        crate::domain::history::set_history_budget(if self.light_mode {
            crate::domain::history::LIGHT_HISTORY_BYTES
        } else {
            usize::MAX
        });
    }
}
