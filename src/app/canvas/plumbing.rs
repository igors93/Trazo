//! The plumbing discipline on screen (#96): the points in their colours,
//! with their tags and heights when the zoom leaves room for them, and the
//! water network with the pressure left at every fixture (#118).

use eframe::egui::{self, Color32, FontId, Painter, Rect, Shape, Stroke, vec2};

use super::ViewTransform;
use crate::domain::layers::LayerCategory;
use crate::domain::model::Project;
use crate::domain::plumbing::cold_water::cold_water;
use crate::domain::plumbing::network::{Network, PipeRole, Pressure};
use crate::domain::plumbing::points::{PointKind, plumbing_points};

fn pressure_color(state: Pressure) -> Color32 {
    match state {
        Pressure::Good => Color32::from_rgb(46, 125, 50),
        Pressure::Weak => Color32::from_rgb(199, 119, 0),
        Pressure::Low => Color32::from_rgb(192, 57, 43),
    }
}

/// A network's pipes: headers dashed (over the slab), branches solid, the
/// risers as rings.
pub(super) fn paint_network(
    painter: &Painter,
    view: ViewTransform,
    network: &Network,
    ink: Color32,
) {
    for pipe in &network.pipes {
        let stroke = match pipe.role {
            PipeRole::Header => Stroke::new(1.6, ink),
            PipeRole::Branch => Stroke::new(1.3, ink),
            PipeRole::Riser | PipeRole::Drop => continue,
        };
        for leg in pipe.path.windows(2) {
            let (a, b) = (view.to_screen(leg[0].at), view.to_screen(leg[1].at));
            if a.distance(b) < 0.5 {
                continue;
            }
            if pipe.role == PipeRole::Header {
                painter.extend(Shape::dashed_line(&[a, b], stroke, 7.0, 4.0));
            } else {
                painter.line_segment([a, b], stroke);
            }
        }
    }
    for group in &network.groups {
        let at = view.to_screen(group.riser_at);
        painter.circle(at, 5.5, Color32::WHITE, Stroke::new(1.5, ink));
        painter.circle_filled(at, 2.0, ink);
    }
}

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
    // The cold water: the network, the tank and the pressure at each outlet.
    let cold = cold_water(project);
    if let Some(cold) = &cold {
        let blue = color(PointKind::Cold);
        paint_network(painter, view, &cold.network, blue);
        let tank = Rect::from_center_size(
            view.to_screen(cold.tank_at),
            vec2(view.scale, view.scale).max(vec2(10.0, 10.0)),
        );
        painter.rect(
            tank,
            0.0,
            Color32::from_rgb(227, 240, 251),
            Stroke::new(1.5, blue),
            egui::StrokeKind::Middle,
        );
        painter.line_segment(
            [tank.left_top(), tank.right_bottom()],
            Stroke::new(0.8, blue),
        );
        painter.line_segment(
            [tank.right_top(), tank.left_bottom()],
            Stroke::new(0.8, blue),
        );
        for outlet in &cold.network.outlets {
            painter.circle(
                view.to_screen(outlet.node.at),
                r + 2.5,
                Color32::TRANSPARENT,
                Stroke::new(2.0, pressure_color(outlet.state)),
            );
        }
    }
    // Below this zoom the tags would pile up; the symbols still show.
    if view.scale < 60.0 {
        return;
    }
    if let Some(cold) = &cold {
        let size = (view.scale * 0.11).clamp(9.0, 12.0);
        for outlet in &cold.network.outlets {
            let ink = pressure_color(outlet.state);
            let text = format!("{:.2} m", outlet.pressure_m).replace('.', ",");
            let galley = painter.layout_no_wrap(text, FontId::proportional(size), ink);
            let pos = view.to_screen(outlet.node.at)
                - vec2(galley.size().x + r + 4.0, galley.size().y / 2.0);
            painter.rect_filled(
                Rect::from_min_size(pos, galley.size()).expand2(vec2(2.0, 0.5)),
                2.0,
                Color32::from_white_alpha(225),
            );
            painter.galley(pos, galley, ink);
        }
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
