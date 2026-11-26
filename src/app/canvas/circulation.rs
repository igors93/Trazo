//! Stairs, ramps and level changes in plan.

use eframe::egui::{self, Color32, Painter, Pos2, Shape, Stroke};

use crate::app::theme;
use crate::domain::model::Project;

use super::ViewTransform;

pub(super) fn paint_circulations(painter: &Painter, view: ViewTransform, project: &Project) {
    let fill = Color32::from_rgba_unmultiplied(238, 244, 250, 210);
    let outline = Stroke::new(1.4, theme::WALL);
    let step_stroke = Stroke::new(1.0, theme::TEXT_2);
    let rail_stroke = Stroke::new(2.2, theme::ACCENT_TEXT);
    for elem in &project.circulations {
        let geom = elem.plan_geometry();
        for poly in &geom.polygons {
            let pts: Vec<Pos2> = poly.iter().map(|p| view.to_screen(*p)).collect();
            if pts.len() >= 3 {
                painter.add(Shape::convex_polygon(pts.clone(), fill, outline));
            }
        }
        for (a, b) in &geom.lines {
            painter.line_segment([view.to_screen(*a), view.to_screen(*b)], step_stroke);
        }
        for (a, b) in &geom.railing_lines {
            painter.line_segment([view.to_screen(*a), view.to_screen(*b)], rail_stroke);
        }
        if let Some((a, b)) = geom.arrow {
            let sa = view.to_screen(a);
            let sb = view.to_screen(b);
            painter.line_segment([sa, sb], Stroke::new(1.4, theme::ACCENT));
            let dir = (sb - sa).normalized();
            let perp = egui::vec2(-dir.y, dir.x);
            painter.add(Shape::convex_polygon(
                vec![sb, sb - dir * 8.0 + perp * 4.0, sb - dir * 8.0 - perp * 4.0],
                theme::ACCENT,
                Stroke::NONE,
            ));
        }
        if !geom.label.is_empty() {
            painter.text(
                view.to_screen(geom.label_pos),
                egui::Align2::CENTER_CENTER,
                &geom.label,
                theme::strong(10.0),
                theme::TEXT,
            );
        }
    }
}
