//! Review markups drawn over the plan.

use eframe::egui::{self, Color32, Painter, Rect, Shape, Stroke};

use crate::app::theme;
use crate::domain::model::Project;

use super::ViewTransform;

pub fn paint_markup(
    painter: &Painter,
    view: ViewTransform,
    markup: &crate::domain::markups::Markup,
    label: bool,
) {
    use crate::domain::markups::MarkupKind;
    if markup.validate().is_err() {
        return;
    }
    let [r, g, b] = markup.color;
    let ink = Color32::from_rgb(r, g, b).gamma_multiply(if markup.resolved { 0.55 } else { 1.0 });
    let shape = markup.shape();
    let path: Vec<_> = shape
        .outline()
        .into_iter()
        .map(|p| view.to_screen(p))
        .collect();
    if matches!(markup.kind, MarkupKind::Rectangle | MarkupKind::Circle) {
        painter.add(Shape::convex_polygon(
            path.clone(),
            ink.gamma_multiply(0.10),
            Stroke::NONE,
        ));
    }
    painter.add(Shape::line(path, Stroke::new(2.2, ink)));
    if markup.kind == MarkupKind::Arrow {
        let end = view.to_screen(markup.points[1]);
        let start = view.to_screen(markup.points[0]);
        let along = (end - start).normalized();
        let across = egui::vec2(-along.y, along.x);
        painter.add(Shape::convex_polygon(
            vec![
                end,
                end - along * 13.0 + across * 5.5,
                end - along * 13.0 - across * 5.5,
            ],
            ink,
            Stroke::NONE,
        ));
    }
    if label {
        let anchor = view.to_screen(markup.points[0]);
        let text = format!(
            "{}{}",
            markup.label(),
            if markup.resolved { " ✓" } else { "" }
        );
        let galley = painter.layout_no_wrap(text, theme::strong(12.0), Color32::WHITE);
        let rect = Rect::from_min_size(
            anchor + egui::vec2(3.0, -23.0),
            galley.size() + egui::vec2(10.0, 6.0),
        );
        painter.rect_filled(rect, 4.0, ink);
        painter.galley(rect.min + egui::vec2(5.0, 3.0), galley, Color32::WHITE);
    }
}

pub fn paint_markups(painter: &Painter, view: ViewTransform, project: &Project) {
    for m in &project.markups {
        if project.markup_shown(m) {
            paint_markup(painter, view, m, true);
        }
    }
}
