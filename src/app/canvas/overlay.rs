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
        Pos2::new(
            rect.right() - 12.0 - (NAVIGATION_BUTTON + 6.0),
            rect.top() + 12.0,
        ),
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
    let font = theme::regular(10.5);
    let end_label = format!("{} m", meters_label(meters));
    // The labels are centred on the ticks: the plate leaves the same room
    // past the "0" on the left and past the end label on the right.
    let half_zero = painter
        .layout_no_wrap("0".to_owned(), font.clone(), theme::TEXT_2)
        .size()
        .x
        / 2.0;
    let half_end = painter
        .layout_no_wrap(end_label.clone(), font.clone(), theme::TEXT_2)
        .size()
        .x
        / 2.0;
    const PAD: f32 = 10.0;
    let origin = Pos2::new(
        view.rect.left() + 12.0 + PAD + half_zero,
        view.rect.bottom() - 22.0,
    );
    let plate = Rect::from_min_max(
        Pos2::new(origin.x - half_zero - PAD, origin.y - 30.0),
        Pos2::new(origin.x + length + half_end + PAD, origin.y + 10.0),
    );
    if !view.rect.contains_rect(plate) {
        return;
    }
    paint_floating_plate(painter, plate);
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
        font.clone(),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(bar.center().x, label_y),
        Align2::CENTER_CENTER,
        meters_label(meters / 2.0),
        font.clone(),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(bar.right(), label_y),
        Align2::CENTER_CENTER,
        end_label,
        font,
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

pub use crate::domain::snap::SnapKind;

/// Thin dashed guide line for extension, perpendicular and tangent snaps.
pub fn paint_snap_guide(painter: &Painter, from: Pos2, to: Pos2) {
    let delta = to - from;
    let total = delta.length();
    if total < 2.0 {
        return;
    }
    let dir = delta / total;
    let stroke = Stroke::new(1.1, theme::ACCENT.gamma_multiply(0.7));
    let dash = 6.0;
    let gap = 4.0;
    let mut cursor = 0.0;
    while cursor < total {
        let end = (cursor + dash).min(total);
        painter.line_segment([from + dir * cursor, from + dir * end], stroke);
        cursor += dash + gap;
    }
}

/// The marker of the point a click will use, using standard CAD glyphs for
/// each object snap kind (square for endpoint, triangle for midpoint, X for
/// intersection, circle for centre, right-angle for perpendicular, etc.).
pub fn paint_snap_marker(painter: &Painter, position: Pos2, kind: SnapKind) {
    let stroke = Stroke::new(1.8, theme::ACCENT);
    let fill = theme::ACCENT.gamma_multiply(0.15);
    match kind {
        SnapKind::Endpoint => {
            let square = Rect::from_center_size(position, vec2(11.0, 11.0));
            painter.rect_filled(square, 1.0, fill);
            painter.rect_stroke(square, 1.0, stroke, StrokeKind::Middle);
        }
        SnapKind::Midpoint => {
            let triangle = vec![
                position + vec2(0.0, -7.0),
                position + vec2(6.5, 5.0),
                position + vec2(-6.5, 5.0),
            ];
            painter.add(Shape::convex_polygon(triangle, fill, stroke));
        }
        SnapKind::Intersection => {
            let r = 6.0;
            painter.line_segment([position + vec2(-r, -r), position + vec2(r, r)], stroke);
            painter.line_segment([position + vec2(-r, r), position + vec2(r, -r)], stroke);
        }
        SnapKind::Center => {
            painter.circle(position, 6.5, fill, stroke);
            painter.circle_filled(position, 1.8, theme::ACCENT);
        }
        SnapKind::Perpendicular => {
            let base = position + vec2(-6.0, 6.0);
            painter.line_segment([base + vec2(0.0, -12.0), base], stroke);
            painter.line_segment([base, base + vec2(12.0, 0.0)], stroke);
            let corner = Rect::from_min_max(base + vec2(0.0, -6.0), base + vec2(6.0, 0.0));
            painter.rect_filled(corner, 0.0, fill);
            painter.rect_stroke(
                corner,
                0.0,
                Stroke::new(1.2, theme::ACCENT),
                StrokeKind::Middle,
            );
        }
        SnapKind::Tangent => {
            painter.circle(position, 5.5, fill, stroke);
            painter.line_segment(
                [position + vec2(-7.5, -5.5), position + vec2(7.5, -5.5)],
                stroke,
            );
        }
        SnapKind::Extension => {
            let r = 6.0;
            painter.line_segment([position + vec2(-r, 0.0), position + vec2(r, 0.0)], stroke);
            painter.line_segment([position + vec2(0.0, -r), position + vec2(0.0, r)], stroke);
        }
        SnapKind::Grid => {
            painter.circle(
                position,
                3.6,
                theme::ACCENT,
                Stroke::new(1.5, Color32::WHITE),
            );
        }
        SnapKind::Face => {
            let diamond = vec![
                position + vec2(0.0, -6.5),
                position + vec2(6.5, 0.0),
                position + vec2(0.0, 6.5),
                position + vec2(-6.5, 0.0),
            ];
            painter.add(Shape::convex_polygon(diamond, fill, stroke));
        }
    }

    if !matches!(kind, SnapKind::Grid | SnapKind::Endpoint | SnapKind::Face) {
        let galley = painter.layout_no_wrap(
            kind.label().to_owned(),
            theme::regular(11.0),
            theme::ACCENT_TEXT,
        );
        let badge = Rect::from_min_size(
            position + vec2(10.0, -22.0),
            galley.size() + vec2(10.0, 4.0),
        );
        painter.rect(
            badge,
            4.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 232),
            Stroke::new(1.0, theme::ACCENT_RING),
            StrokeKind::Inside,
        );
        painter.galley(badge.min + vec2(5.0, 2.0), galley, theme::ACCENT_TEXT);
    }
}

/// Floating CAD command badge showing what the user is typing directly on the
/// canvas (distance, angle, dimensions or coordinates) and how it parses.
pub fn paint_numeric_input_hud(
    painter: &Painter,
    canvas_rect: Rect,
    anchor: Option<Pos2>,
    typed: &str,
    summary: &str,
    valid: bool,
) {
    let input_galley =
        painter.layout_no_wrap(format!("{typed}│"), theme::strong(13.5), theme::TEXT);
    let hint_color = if valid {
        theme::ACCENT_TEXT
    } else {
        theme::WARNING
    };
    let hint_galley = painter.layout_no_wrap(summary.to_owned(), theme::regular(11.5), hint_color);
    let width = input_galley.size().x.max(hint_galley.size().x) + 20.0;
    let height = input_galley.size().y + hint_galley.size().y + 14.0;
    let preferred = anchor
        .map(|pos| pos + vec2(18.0, 20.0))
        .unwrap_or_else(|| Pos2::new(canvas_rect.left() + 20.0, canvas_rect.top() + 20.0));
    let min_x = preferred.x.clamp(
        canvas_rect.left() + 12.0,
        (canvas_rect.right() - width - 12.0).max(canvas_rect.left() + 12.0),
    );
    let min_y = preferred.y.clamp(
        canvas_rect.top() + 12.0,
        (canvas_rect.bottom() - height - 12.0).max(canvas_rect.top() + 12.0),
    );
    let badge = Rect::from_min_size(Pos2::new(min_x, min_y), vec2(width, height));
    paint_floating_plate(painter, badge);
    painter.rect_stroke(
        badge,
        8.0,
        Stroke::new(
            1.2,
            if valid {
                theme::ACCENT_RING
            } else {
                theme::WARNING
            },
        ),
        StrokeKind::Inside,
    );
    painter.galley(badge.min + vec2(10.0, 6.0), input_galley, theme::TEXT);
    painter.galley(badge.min + vec2(10.0, 6.0 + 18.0), hint_galley, hint_color);
}

/// Interactive CAD grip square drawn on editable vertices of the selection.
pub fn paint_vertex_grip(painter: &Painter, position: Pos2, active: bool) {
    let size = if active { 11.0 } else { 9.0 };
    let rect = Rect::from_center_size(position, vec2(size, size));
    let fill = if active {
        theme::ACCENT
    } else {
        Color32::WHITE
    };
    let stroke = if active {
        Stroke::new(1.8, Color32::WHITE)
    } else {
        Stroke::new(1.8, theme::ACCENT)
    };
    painter.rect_filled(rect, 1.5, fill);
    painter.rect_stroke(rect, 1.5, stroke, StrokeKind::Middle);
}

/// The mark of a constraint (∥, ⊥, ●, ↔ 1,20 m) at `position`; stronger
/// when it concerns the selection.
pub fn paint_constraint_badge(painter: &Painter, position: Pos2, text: &str, strong: bool) {
    let color = if strong {
        theme::ACCENT_TEXT
    } else {
        theme::MUTED
    };
    let galley = painter.layout_no_wrap(text.to_owned(), theme::strong(11.0), color);
    let badge =
        Rect::from_center_size(position + vec2(0.0, -12.0), galley.size() + vec2(10.0, 4.0));
    painter.rect(
        badge,
        8.0,
        Color32::WHITE,
        Stroke::new(
            1.0,
            if strong {
                theme::ACCENT_RING
            } else {
                theme::BORDER_STRONG
            },
        ),
        StrokeKind::Inside,
    );
    painter.galley(badge.min + vec2(5.0, 2.0), galley, color);
}

/// A dashed frame around a selected group, with its name on the top edge.
pub fn paint_group_frame(painter: &Painter, rect: Rect, name: &str) {
    let rect = rect.expand(6.0);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(Shape::dashed_line(
        &corners,
        Stroke::new(1.2, theme::ACCENT.gamma_multiply(0.8)),
        5.0,
        3.0,
    ));
    let galley = painter.layout_no_wrap(name.to_owned(), theme::regular(11.0), theme::ACCENT_TEXT);
    let badge = Rect::from_min_size(
        rect.left_top() + vec2(6.0, -galley.size().y - 6.0),
        galley.size() + vec2(10.0, 4.0),
    );
    painter.rect(
        badge,
        3.0,
        Color32::WHITE,
        Stroke::new(1.0, theme::ACCENT_RING),
        StrokeKind::Inside,
    );
    painter.galley(badge.min + vec2(5.0, 2.0), galley, theme::ACCENT_TEXT);
}

/// The selection box between `start` and `end`: a blue window with a solid
/// edge (whole elements inside) or, drawn right to left, a green crossing
/// box with a dashed edge (whatever it touches).
pub fn paint_selection_box(painter: &Painter, start: Pos2, end: Pos2, crossing: bool) {
    let rect = Rect::from_two_pos(start, end);
    let color = if crossing {
        theme::SUCCESS
    } else {
        theme::ACCENT
    };
    painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let stroke = Stroke::new(1.2, color);
    if crossing {
        painter.extend(Shape::dashed_line(&corners, stroke, 6.0, 4.0));
    } else {
        painter.add(Shape::line(corners.to_vec(), stroke));
    }
    let label = if crossing { "Cruzamento" } else { "Janela" };
    let galley = painter.layout_no_wrap(label.to_owned(), theme::regular(11.0), color);
    let anchor = end + vec2(10.0, 10.0);
    let badge = Rect::from_min_size(anchor, galley.size() + vec2(10.0, 4.0));
    painter.rect(
        badge,
        3.0,
        Color32::WHITE,
        Stroke::new(1.0, color.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    painter.galley(badge.min + vec2(5.0, 2.0), galley, color);
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
