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
