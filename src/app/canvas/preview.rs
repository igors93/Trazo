//! The rubber-band preview of the element being drawn.

use eframe::egui::{self, Painter, Rect, Stroke};

use super::labels::paint_preview_measure;
use crate::app::theme;
use crate::domain::{geometry::format_meters, model::Project, wall_reference::WallReference};

use super::{Preview, ViewTransform, dash_dot, wall_screen_width, wall_stroke_width};

pub(super) fn paint_preview(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    preview: Preview,
    reference: WallReference,
    occupied: &[Rect],
) {
    let valid_color = theme::ACCENT;
    let invalid_color = theme::DANGER;
    let thickness = project.masonry.thickness();
    match preview {
        Preview::Wall(ref_a, ref_b) => {
            let (a, b) = reference.offset_segment(ref_a, ref_b, thickness);
            let valid = a.distance(b) >= 0.24
                && project
                    .terrain
                    .as_ref()
                    .is_none_or(|t| t.contains(a) && t.contains(b));
            let color = if valid { valid_color } else { invalid_color };
            painter.line_segment(
                [view.to_screen(a), view.to_screen(b)],
                Stroke::new(
                    wall_screen_width(view.scale, thickness),
                    color.gamma_multiply(0.85),
                ),
            );
            if reference != WallReference::Center {
                super::overlay::paint_snap_guide(
                    painter,
                    view.to_screen(ref_a),
                    view.to_screen(ref_b),
                );
            }
            painter.circle_filled(view.to_screen(ref_a), 3.5, color);
            painter.circle_filled(view.to_screen(ref_b), 4.5, color);
            let midpoint = view.to_screen(ref_a).lerp(view.to_screen(ref_b), 0.5);
            let label = if reference == WallReference::Center {
                format_meters(ref_a.distance(ref_b))
            } else {
                format!(
                    "{} ({})",
                    format_meters(ref_a.distance(ref_b)),
                    reference.short_label()
                )
            };
            paint_preview_measure(
                painter,
                midpoint + egui::vec2(0.0, -22.0),
                view.rect,
                view.to_screen(ref_b),
                &label,
                view.rect,
                occupied,
            );
        }
        Preview::Room(ref_a, ref_b) => {
            let w = (ref_b.x - ref_a.x).abs();
            let h = (ref_b.y - ref_a.y).abs();
            let shifted = reference.offset_rect(ref_a, ref_b, thickness);
            let valid = w >= 0.5
                && h >= 0.5
                && shifted.is_some_and(|(min, max)| {
                    project
                        .terrain
                        .as_ref()
                        .is_none_or(|t| t.contains(min) && t.contains(max))
                });
            let color = if valid { valid_color } else { invalid_color };
            let (draw_a, draw_b) = shifted.unwrap_or((ref_a, ref_b));
            let rect = Rect::from_two_pos(view.to_screen(draw_a), view.to_screen(draw_b));
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ];
            for index in 0..4 {
                painter.line_segment(
                    [corners[index], corners[(index + 1) % 4]],
                    Stroke::new(
                        wall_screen_width(view.scale, thickness),
                        color.gamma_multiply(0.85),
                    ),
                );
            }
            if reference != WallReference::Center {
                let ref_rect = Rect::from_two_pos(view.to_screen(ref_a), view.to_screen(ref_b));
                let ref_corners = [
                    ref_rect.left_top(),
                    ref_rect.right_top(),
                    ref_rect.right_bottom(),
                    ref_rect.left_bottom(),
                ];
                for index in 0..4 {
                    super::overlay::paint_snap_guide(
                        painter,
                        ref_corners[index],
                        ref_corners[(index + 1) % 4],
                    );
                }
            }
            let room_label = if reference == WallReference::Center {
                format!(
                    "{} × {} · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    w * h
                )
            } else {
                format!(
                    "{} × {} ({}) · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    reference.short_label(),
                    w * h
                )
            }
            .replace('.', ",");
            paint_preview_measure(
                painter,
                rect.center(),
                rect.shrink(wall_stroke_width(view.scale) / 2.0 + 4.0),
                view.to_screen(ref_b),
                &room_label,
                view.rect,
                occupied,
            );
        }
        Preview::Terrain(a, b) => {
            let w = (b.x - a.x).abs();
            let h = (b.y - a.y).abs();
            let valid = w >= 0.25 && h >= 0.25;
            let color = if valid {
                theme::TERRAIN_LINE
            } else {
                invalid_color
            };
            let rect = Rect::from_two_pos(view.to_screen(a), view.to_screen(b));
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
            dash_dot(
                painter,
                &[
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ],
                Stroke::new(2.0, color),
            );
            paint_preview_measure(
                painter,
                rect.center(),
                rect.shrink(4.0),
                view.to_screen(b),
                &format!(
                    "{} × {} · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    w * h
                )
                .replace('.', ","),
                view.rect,
                occupied,
            );
        }
    }
}
