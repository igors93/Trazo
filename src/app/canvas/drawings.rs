//! Lines, arcs, circles and other drawing entities on the plan, and the
//! shape being drawn.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape as EguiShape, Stroke, vec2};

use super::{Marks, ViewTransform};
use crate::app::{Selection, theme};
use crate::domain::drawing::Shape;
use crate::domain::geometry::Point;
use crate::domain::layers::LineType;
use crate::domain::model::Project;

/// Paints `shape` as a solid or styled line on screen.
pub fn paint_shape(painter: &Painter, view: ViewTransform, shape: &Shape, stroke: Stroke) {
    paint_shape_styled(painter, view, shape, stroke, LineType::Solid);
}

pub fn paint_shape_styled(
    painter: &Painter,
    view: ViewTransform,
    shape: &Shape,
    stroke: Stroke,
    line_type: LineType,
) {
    let points: Vec<Pos2> = shape
        .outline()
        .into_iter()
        .map(|point| view.to_screen(point))
        .collect();
    if points.len() < 2 {
        return;
    }
    if let Some((dash, gap)) = line_type.screen_dash_gap() {
        painter.extend(EguiShape::dashed_line(&points, stroke, dash, gap));
    } else {
        painter.add(EguiShape::line(points, stroke));
    }
}

/// The project's drawing entities, with layer visibility, colour, weight and
/// line type applied, and the selected and hovered ones highlighted.
pub fn paint_drawings(painter: &Painter, view: ViewTransform, project: &Project, marks: Marks<'_>) {
    for drawing in &project.drawings {
        if !project.is_drawing_visible(drawing) {
            continue;
        }
        let layer = project.effective_drawing_layer(drawing);
        let line_type = layer.map(|l| l.line_type).unwrap_or(LineType::Solid);
        let stroke = if marks.selected(Selection::Drawing(drawing.id)) {
            Stroke::new(2.4, theme::SELECTION)
        } else if marks.hovered(Selection::Drawing(drawing.id)) {
            Stroke::new(2.0, theme::HOVER)
        } else if let Some(layer) = layer {
            let [r, g, b] = layer.color;
            Stroke::new(layer.screen_stroke_width(), Color32::from_rgb(r, g, b))
        } else {
            Stroke::new(1.3, theme::TEXT)
        };
        paint_shape_styled(painter, view, &drawing.shape, stroke, line_type);
    }
}

/// The shape being drawn, with the points clicked so far.
pub fn paint_draw_draft(
    painter: &Painter,
    view: ViewTransform,
    shape: Option<&Shape>,
    points: &[Point],
) {
    if let Some(shape) = shape {
        paint_shape(painter, view, shape, Stroke::new(1.6, theme::ACCENT));
        // A circle shows its radius.
        if let (Shape::Circle { center, .. }, Some(last)) = (shape, shape.outline().first()) {
            painter.extend(EguiShape::dashed_line(
                &[view.to_screen(*center), view.to_screen(*last)],
                Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)),
                4.0,
                3.0,
            ));
        }
    }
    for point in points {
        painter.rect(
            Rect::from_center_size(view.to_screen(*point), vec2(6.0, 6.0)),
            1.0,
            theme::PANEL,
            Stroke::new(1.4, theme::ACCENT),
            eframe::egui::StrokeKind::Middle,
        );
    }
}
