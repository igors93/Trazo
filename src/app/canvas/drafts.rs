//! Shapes being drawn, before they become part of the project.

use eframe::egui::{self, Painter, Pos2, Rect, Shape, Stroke};

use super::ViewTransform;
use crate::app::theme;
use crate::domain::geometry::{Point, format_meters};

/// A room being drawn corner by corner: its sides so far, the side to the
/// pointer and, dashed, the side that would close it; or, `closed`, the
/// finished outline waiting for a name.
pub fn paint_polygon_draft(
    painter: &Painter,
    view: ViewTransform,
    corners: &[Point],
    cursor: Option<Point>,
    closed: bool,
) {
    let mut outline: Vec<Point> = corners.to_vec();
    if let Some(cursor) = cursor {
        outline.push(cursor);
    }
    let color = theme::ACCENT;
    let triangles = crate::domain::geometry::triangulate_polygon(&outline);
    for triangle in triangles {
        painter.add(Shape::convex_polygon(
            triangle
                .iter()
                .map(|corner| view.to_screen(*corner))
                .collect(),
            color.gamma_multiply(0.10),
            Stroke::NONE,
        ));
    }
    let screen: Vec<Pos2> = outline
        .iter()
        .map(|corner| view.to_screen(*corner))
        .collect();
    if closed && screen.len() >= 3 {
        painter.add(Shape::closed_line(screen.clone(), Stroke::new(2.0, color)));
    } else {
        painter.add(Shape::line(screen.clone(), Stroke::new(2.0, color)));
        if screen.len() >= 3 {
            painter.extend(Shape::dashed_line(
                &[screen[screen.len() - 1], screen[0]],
                Stroke::new(1.2, color.gamma_multiply(0.7)),
                5.0,
                4.0,
            ));
        }
    }
    for (index, corner) in screen.iter().enumerate() {
        let size = if index == 0 && !closed { 9.0 } else { 6.0 };
        painter.rect(
            Rect::from_center_size(*corner, egui::vec2(size, size)),
            1.0,
            theme::PANEL,
            Stroke::new(1.4, color),
            egui::StrokeKind::Middle,
        );
    }
    // The length of the side being drawn.
    if let (Some(cursor), Some(last)) = (cursor, corners.last()) {
        let label = format_meters(last.distance(cursor));
        let galley = painter.layout_no_wrap(label, theme::strong(11.0), theme::ACCENT_TEXT);
        let middle = view.to_screen(last.midpoint(cursor));
        let rect = Rect::from_center_size(
            middle + egui::vec2(0.0, -14.0),
            galley.size() + egui::vec2(10.0, 4.0),
        );
        painter.rect(
            rect,
            3.0,
            theme::PANEL,
            Stroke::new(1.0, theme::ACCENT_RING),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.min + egui::vec2(5.0, 2.0), galley, theme::ACCENT_TEXT);
    }
}

/// What a modify tool would make, drawn see-through in the accent colour:
/// walls at their thickness, fixtures as ghosts, drawings as lines.
pub fn paint_fragment_ghost(
    painter: &Painter,
    view: ViewTransform,
    fragment: &crate::domain::model::Fragment,
) {
    let color = theme::ACCENT;
    for wall in &fragment.walls {
        let (a, b) = (view.to_screen(wall.start), view.to_screen(wall.end));
        let width = view.pixels(wall.thickness()).max(2.0);
        painter.line_segment([a, b], Stroke::new(width, color.gamma_multiply(0.30)));
        painter.line_segment([a, b], Stroke::new(1.0, color.gamma_multiply(0.8)));
    }
    for fixture in &fragment.fixtures {
        super::paint_fixture(painter, view, fixture, super::FixtureTone::Ghost);
    }
    for drawing in &fragment.drawings {
        super::drawings::paint_shape(
            painter,
            view,
            &drawing.shape,
            Stroke::new(1.6, color.gamma_multiply(0.85)),
        );
    }
}

/// The rubber band of a modify tool from its first point to the pointer;
/// for mirroring, the whole axis across the view.
pub fn paint_modify_guide(
    painter: &Painter,
    view: ViewTransform,
    anchor: Point,
    cursor: Point,
    axis: bool,
) {
    let (a, b) = (view.to_screen(anchor), view.to_screen(cursor));
    let stroke = Stroke::new(1.2, theme::ACCENT.gamma_multiply(0.8));
    let ends = if axis && a.distance(b) > 1.0 {
        let along = (b - a).normalized() * (view.rect.width() + view.rect.height());
        [a - along, b + along]
    } else {
        [a, b]
    };
    painter.extend(Shape::dashed_line(&ends, stroke, 6.0, 4.0));
    painter.rect(
        Rect::from_center_size(a, egui::vec2(7.0, 7.0)),
        1.0,
        theme::PANEL,
        Stroke::new(1.4, theme::ACCENT),
        egui::StrokeKind::Middle,
    );
}
