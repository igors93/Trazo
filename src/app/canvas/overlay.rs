//! Drawing aids painted over the plan: graphic scale, north arrow, the CAD
//! crosshair and snap markers, and where the navigation bar sits.

use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};

use super::ViewTransform;
use crate::app::theme;

/// Buttons in the navigation bar at the top right of the view.
pub const NAVIGATION_BUTTONS: usize = 4;
pub const NAVIGATION_BUTTON: f32 = 30.0;

/// Where the floating navigation bar sits in the canvas `rect`.
pub fn navigation_rect(rect: Rect) -> Rect {
    let height = NAVIGATION_BUTTONS as f32 * NAVIGATION_BUTTON + 9.0 + 8.0;
    Rect::from_min_size(
        Pos2::new(rect.right() - 12.0 - (NAVIGATION_BUTTON + 6.0), rect.top() + 12.0),
        vec2(NAVIGATION_BUTTON + 6.0, height),
    )
}

/// The floating plate behind the navigation bar.
pub fn paint_floating_plate(painter: &Painter, rect: Rect) {
    let shadow = egui::Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(22, 50, 92, 34),
    };
    painter.add(shadow.as_shape(rect, 8));
    painter.rect(
        rect,
        8.0,
        theme::PANEL,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
}

/// The length of a graphic scale bar that reads well at `scale` px per metre.
pub fn scale_bar_length(scale: f32) -> f32 {
    const STEPS: [f32; 12] = [
        0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0,
    ];
    STEPS
        .iter()
        .copied()
        .rev()
        .find(|meters| meters * scale <= 150.0)
        .unwrap_or(STEPS[0])
}

fn meters_label(value: f32) -> String {
    if value.fract().abs() < 1e-3 {
        format!("{value:.0}")
    } else {
        format!("{value}").replace('.', ",")
    }
}

/// A four-part graphic scale at the bottom left of the view.
pub fn paint_scale_bar(painter: &Painter, view: ViewTransform) {
    let meters = scale_bar_length(view.scale);
    let length = meters * view.scale;
    if length < 20.0 {
        return;
    }
    let origin = Pos2::new(view.rect.left() + 22.0, view.rect.bottom() - 22.0);
    let plate = Rect::from_min_max(
        Pos2::new(origin.x - 12.0, origin.y - 30.0),
        Pos2::new(origin.x + length + 30.0, origin.y + 10.0),
    );
    if !view.rect.contains_rect(plate) {
        return;
    }
    painter.rect(
        plate,
        6.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 225),
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let bar = Rect::from_min_size(Pos2::new(origin.x, origin.y - 5.0), vec2(length, 5.0));
    for part in 0..4 {
        let segment = Rect::from_min_size(
            Pos2::new(bar.left() + length * part as f32 / 4.0, bar.top()),
            vec2(length / 4.0, bar.height()),
        );
        painter.rect_filled(
            segment,
            0.0,
            if part % 2 == 0 {
                theme::WALL
            } else {
                theme::PANEL
            },
        );
    }
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0, theme::WALL), StrokeKind::Middle);
    let label_y = bar.top() - 9.0;
    painter.text(
        Pos2::new(bar.left(), label_y),
        Align2::CENTER_CENTER,
        "0",
        theme::regular(10.5),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(bar.center().x, label_y),
        Align2::CENTER_CENTER,
        meters_label(meters / 2.0),
        theme::regular(10.5),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(bar.right(), label_y),
        Align2::CENTER_CENTER,
        format!("{} m", meters_label(meters)),
        theme::regular(10.5),
        theme::TEXT_2,
    );
}

/// A north arrow at the bottom right of the view.
pub fn paint_north_arrow(painter: &Painter, rect: Rect) {
    let center = Pos2::new(rect.right() - 38.0, rect.bottom() - 40.0);
    if !rect.contains(center - vec2(30.0, 50.0)) {
        return;
    }
    painter.circle(
        center,
        17.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 225),
        Stroke::new(1.0, theme::BORDER_STRONG),
    );
    let tip = center + vec2(0.0, -13.0);
    let tail = center + vec2(0.0, 11.0);
    let left = center + vec2(-6.5, 7.0);
    let right = center + vec2(6.5, 7.0);
    painter.add(Shape::convex_polygon(
        vec![tip, tail, left],
        theme::WALL,
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(
        vec![tip, right, tail],
        Stroke::new(1.0, theme::WALL),
    ));
    painter.text(
        center + vec2(0.0, -27.0),
        Align2::CENTER_CENTER,
        "N",
        theme::strong(12.0),
        theme::TEXT,
    );
}

/// The drawing crosshair that replaces the pointer while a drawing tool is
/// active, with a gap so the snap marker stays visible.
pub fn paint_crosshair(painter: &Painter, position: Pos2) {
    let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(28, 43, 60, 210));
    let (gap, arm) = (5.0, 26.0);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let direction = vec2(dx, dy);
        painter.line_segment(
            [position + direction * gap, position + direction * arm],
            stroke,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// The end of an existing wall.
    Endpoint,
    /// A grid intersection.
    Grid,
}

/// The marker of the point a click will use: a square on wall ends, as in
/// CAD object snaps, and a dot on the grid.
pub fn paint_snap_marker(painter: &Painter, position: Pos2, kind: SnapKind) {
    match kind {
        SnapKind::Endpoint => {
            let square = Rect::from_center_size(position, vec2(11.0, 11.0));
            painter.rect_filled(square, 1.0, theme::ACCENT.gamma_multiply(0.15));
            painter.rect_stroke(
                square,
                1.0,
                Stroke::new(1.8, theme::ACCENT),
                StrokeKind::Middle,
            );
        }
        SnapKind::Grid => {
            painter.circle(position, 3.6, theme::ACCENT, Stroke::new(1.5, Color32::WHITE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_bar_stays_between_readable_lengths() {
        for scale in [3.0, 8.0, 20.0, 60.0, 120.0, 240.0] {
            let pixels = scale_bar_length(scale) * scale;
            assert!(pixels <= 150.0, "{scale}: {pixels}");
            assert!(pixels >= 55.0, "{scale}: {pixels}");
        }
    }

    #[test]
    fn navigation_bar_sits_inside_the_view() {
        let view = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        assert!(view.contains_rect(navigation_rect(view)));
    }
}
