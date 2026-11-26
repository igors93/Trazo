//! What sits behind the plan: the grid, the floor underlay and the empty
//! state card.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::theme;
use crate::domain::{
    geometry::{GRID_METERS, Point},
    model::Project,
};

use super::{ViewTransform, wall_run, wall_screen_width};

pub(super) fn paint_empty_state(painter: &Painter, area: Rect) {
    let card = Rect::from_center_size(area.center(), egui::vec2(460.0, 132.0));
    if !area.contains_rect(card) {
        return;
    }
    let shadow = egui::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(22, 50, 92, 30),
    };
    painter.add(shadow.as_shape(card, 10));
    painter.rect(
        card,
        10.0,
        theme::PANEL,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(
        Pos2::new(card.left() + 44.0, card.center().y - 14.0),
        egui::vec2(34.0, 34.0),
    );
    painter.rect_filled(icon.expand(6.0), 9.0, theme::ACCENT_SOFT);
    crate::app::ui::icons::paint(
        painter,
        icon,
        crate::app::ui::icons::Icon::Plan,
        theme::ACCENT_TEXT,
        theme::ACCENT,
    );
    let left = card.left() + 84.0;
    painter.text(
        Pos2::new(left, card.top() + 30.0),
        egui::Align2::LEFT_CENTER,
        "Comece seu projeto",
        theme::strong(16.0),
        theme::TEXT,
    );
    painter.text(
        Pos2::new(left, card.top() + 56.0),
        egui::Align2::LEFT_CENTER,
        "Crie um terreno com medidas exatas ou desenhe",
        theme::regular(13.0),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(left, card.top() + 74.0),
        egui::Align2::LEFT_CENTER,
        "paredes e cômodos diretamente na grade.",
        theme::regular(13.0),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(left, card.top() + 104.0),
        egui::Align2::LEFT_CENTER,
        "Terreno  T    ·    Parede  P    ·    Cômodo  C    ·    Ajuda  F1",
        theme::regular(11.5),
        theme::MUTED,
    );
}

/// Paints the floor directly below as a translucent architectural underlay reference.
pub(super) fn paint_floor_underlay(painter: &Painter, view: ViewTransform, project: &Project) {
    let Some(underlay) = project.underlay_floor() else {
        return;
    };
    let ghost_wall = Color32::from_rgba_unmultiplied(110, 140, 175, 78);
    let ghost_line = Stroke::new(1.1, Color32::from_rgba_unmultiplied(100, 130, 165, 115));
    for wall in &underlay.data.walls {
        let a = view.to_screen(wall.start);
        let b = view.to_screen(wall.end);
        let w = wall_screen_width(view.scale, wall.thickness());
        wall_run(painter, a, b, w, ghost_wall);
        painter.extend(Shape::dashed_line(&[a, b], ghost_line, 6.0, 4.0));
    }
    for elem in &underlay.data.circulations {
        let geom = elem.plan_geometry();
        for poly in &geom.polygons {
            let mut pts: Vec<Pos2> = poly.iter().map(|p| view.to_screen(*p)).collect();
            if let Some(first) = pts.first().copied() {
                pts.push(first);
                painter.extend(Shape::dashed_line(&pts, ghost_line, 5.0, 4.0));
            }
        }
    }
}

pub(super) fn paint_grid(painter: &Painter, view: ViewTransform) {
    let step = if view.scale < 8.0 {
        5.0
    } else if view.scale < 20.0 {
        1.0
    } else {
        GRID_METERS
    };
    let major_every = if step >= 5.0 { 25.0 } else { 1.0 };
    let first = view.to_world(view.rect.left_top());
    let last = view.to_world(view.rect.right_bottom());
    let start_x = (first.x / step).floor() as i32;
    let end_x = (last.x / step).ceil() as i32;
    let start_y = (first.y / step).floor() as i32;
    let end_y = (last.y / step).ceil() as i32;
    let color = |value: f64| {
        if (value / major_every).fract().abs() < 0.001
            || (value / major_every).fract().abs() > 0.999
        {
            theme::GRID_MAJOR
        } else {
            theme::GRID_MINOR
        }
    };
    for index in start_x..=end_x {
        let value = f64::from(index) * step;
        let x = view.to_screen(Point::new(value, 0.0)).x;
        painter.line_segment(
            [
                Pos2::new(x, view.rect.top()),
                Pos2::new(x, view.rect.bottom()),
            ],
            Stroke::new(1.0, color(value)),
        );
    }
    for index in start_y..=end_y {
        let value = f64::from(index) * step;
        let y = view.to_screen(Point::new(0.0, value)).y;
        painter.line_segment(
            [
                Pos2::new(view.rect.left(), y),
                Pos2::new(view.rect.right(), y),
            ],
            Stroke::new(1.0, color(value)),
        );
    }
}
