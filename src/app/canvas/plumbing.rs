//! The plumbing discipline on screen (#96): the points in their colours,
//! with their tags and heights when the zoom leaves room for them.

use eframe::egui::{self, Color32, FontId, Painter, Rect, Stroke, vec2};

use super::ViewTransform;
use crate::domain::layers::LayerCategory;
use crate::domain::model::Project;
use crate::domain::plumbing::points::{PointKind, plumbing_points};

pub(super) fn color(kind: PointKind) -> Color32 {
    let [r, g, b] = kind.color();
    Color32::from_rgb(r, g, b)
}

/// Paints the plumbing points when the plumbing layer is shown.
pub(super) fn paint_plumbing(painter: &Painter, view: ViewTransform, project: &Project) {
    if !project.category_visible(LayerCategory::Plumbing) {
        return;
    }
    let points = plumbing_points(project);
    let r = (view.scale * 0.05).clamp(3.0, 6.0);
    for point in &points.points {
        let at = view.to_screen(point.at);
        let ink = color(point.kind);
        match point.kind {
            PointKind::Cold | PointKind::Hot => {
                painter.circle(at, r, Color32::WHITE, Stroke::new(1.6, ink));
            }
            PointKind::Drain => {
                painter.circle_filled(at, r, ink);
            }
            PointKind::Gas => {
                painter.rect(
                    Rect::from_center_size(at, vec2(2.0 * r, 2.0 * r)),
                    0.0,
                    Color32::WHITE,
                    Stroke::new(1.6, ink),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
    // Below this zoom the tags would pile up; the symbols still show.
    if view.scale < 60.0 {
        return;
    }
    let size = (view.scale * 0.11).clamp(9.0, 12.0);
    let mut fixture = None;
    let mut row = 0.0;
    for point in &points.points {
        if fixture != Some(point.fixture_id) {
            fixture = Some(point.fixture_id);
            row = 0.0;
        }
        let ink = color(point.kind);
        let text = format!(
            "{} {}",
            point.kind.tag(),
            format!("{:.2}", point.height_m).replace('.', ",")
        );
        let galley = painter.layout_no_wrap(text, FontId::proportional(size), ink);
        let pos = view.to_screen(point.at) + vec2(r + 3.0, -galley.size().y / 2.0 + row);
        painter.rect_filled(
            Rect::from_min_size(pos, galley.size()).expand2(vec2(2.0, 0.5)),
            2.0,
            Color32::from_white_alpha(215),
        );
        row += galley.size().y + 1.0;
        painter.galley(pos, galley, ink);
    }
}
