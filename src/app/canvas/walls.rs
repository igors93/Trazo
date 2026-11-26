//! Walls in plan: solid and hollow runs, multilayer assemblies and the
//! gaps, leaves and swings of their openings.

use eframe::egui::{self, Color32, Painter, Pos2, Shape, Stroke, Vec2};

use crate::app::{Selection, theme};
use crate::domain::model::{OpeningKind, Project, Wall};

use super::{Marks, ViewTransform, wall_screen_width};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum WallTone {
    Normal,
    Hovered,
    Selected,
}

/// Paints a straight run of wall from `from` to `to` with square ends, so
/// walls meeting at a corner close it without a notch.
pub(super) fn wall_run(painter: &Painter, from: Pos2, to: Pos2, width: f32, color: Color32) {
    let along = to - from;
    if along.length() < 0.01 {
        return;
    }
    let direction = along.normalized();
    let normal = egui::vec2(-direction.y, direction.x) * (width / 2.0);
    painter.add(Shape::convex_polygon(
        vec![from + normal, to + normal, to - normal, from - normal],
        color,
        Stroke::NONE,
    ));
}

/// Paints the hollow architectural interior core of a wall segment inset by
/// `border` on all four sides, leaving crisp double-line outer contours and
/// clean end caps / jambs.
pub(super) fn wall_core_run(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    width: f32,
    border: f32,
    fill: Color32,
) {
    let inner_width = width - 2.0 * border;
    if inner_width < 1.0 {
        return;
    }
    let along = to - from;
    let len = along.length();
    if len <= 2.0 * border + 0.5 {
        return;
    }
    let dir = along / len;
    wall_run(
        painter,
        from + dir * border,
        to - dir * border,
        inner_width,
        fill,
    );
}

/// Computes the screen-space solid masonry runs of a wall between its openings,
/// extending by half the wall thickness at wall endpoints so corners close.
pub(super) fn wall_solid_runs(
    wall: &Wall,
    project: &Project,
    view: ViewTransform,
    wall_width: f32,
) -> Vec<(Pos2, Pos2)> {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let screen_length = a.distance(b);
    if screen_length < 0.01 {
        return Vec::new();
    }
    let direction = (b - a) / screen_length;
    let cap = direction * (wall_width / 2.0);
    let mut openings: Vec<_> = project
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect();
    openings.sort_by(|left, right| left.t.total_cmp(&right.t));

    let mut runs = Vec::with_capacity(openings.len() + 1);
    let mut cursor = 0.0;
    let mut run_start = a - cap;
    for opening in openings {
        let half = view.pixels(opening.width()) / 2.0;
        let center = opening.t as f32 * screen_length;
        let left = center - half;
        let right = center + half;
        if left > cursor {
            runs.push((run_start, a + direction * left));
        }
        cursor = right;
        run_start = a + direction * right;
    }
    if cursor < screen_length {
        runs.push((run_start, b + cap));
    }
    runs
}

pub(super) fn paint_wall_openings(
    painter: &Painter,
    view: ViewTransform,
    wall: &Wall,
    project: &Project,
    tone: WallTone,
    marks: Marks<'_>,
    border_px: f32,
) {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let screen_length = a.distance(b);
    if screen_length < 0.01 {
        return;
    }
    let direction = (b - a) / screen_length;
    let normal = egui::vec2(-direction.y, direction.x);
    let wall_width = wall_screen_width(view.scale, wall.thickness());
    let color = match tone {
        WallTone::Selected => theme::SELECTION,
        WallTone::Hovered => theme::HOVER,
        WallTone::Normal => theme::WALL,
    };
    let mut openings: Vec<_> = project
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect();
    openings.sort_by(|left, right| left.t.total_cmp(&right.t));
    for opening in openings {
        let op_selected = marks.selected(Selection::Opening(opening.id));
        let op_hovered = marks.hovered(Selection::Opening(opening.id));
        let half = view.pixels(opening.width()) / 2.0;
        let center = opening.t as f32 * screen_length;
        let left = center - half;
        let right = center + half;
        let start = a + direction * left;
        let end = a + direction * right;
        let op_color = if op_selected {
            theme::SELECTION
        } else if op_hovered {
            theme::HOVER
        } else {
            Color32::TRANSPARENT
        };
        let jamb = Stroke::new(
            border_px.max(1.2),
            if op_color == Color32::TRANSPARENT {
                color
            } else {
                op_color
            },
        );
        let half_wall = wall_width / 2.0;
        for point in [start, end] {
            painter.line_segment(
                [point - normal * half_wall, point + normal * half_wall],
                jamb,
            );
        }
        match opening.kind {
            OpeningKind::Door | OpeningKind::Gate => {
                use crate::domain::openings::DoorType;
                let leaf_color = if op_color == Color32::TRANSPARENT {
                    theme::WALL
                } else {
                    op_color
                };
                let arc_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                let leaf = half * 2.0;
                let side = if opening.flip_side { -normal } else { normal };
                let (hinge, along) = if opening.flip_hinge {
                    (end, -direction)
                } else {
                    (start, direction)
                };
                let mut outline_extent = leaf + 4.0;
                match opening.effective_door_type() {
                    DoorType::Swing => {
                        painter.line_segment(
                            [hinge, hinge + side * leaf],
                            Stroke::new(2.2, leaf_color),
                        );
                        let arc: Vec<Pos2> = (0..=20)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 20.0;
                                hinge + along * (theta.sin() * leaf) + side * (theta.cos() * leaf)
                            })
                            .collect();
                        painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                    }
                    DoorType::DoubleSwing => {
                        outline_extent = half + 4.0;
                        for (h, dir_h) in [(start, direction), (end, -direction)] {
                            painter
                                .line_segment([h, h + side * half], Stroke::new(2.0, leaf_color));
                            let arc: Vec<Pos2> = (0..=14)
                                .map(|step| {
                                    let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 14.0;
                                    h + dir_h * (theta.sin() * half) + side * (theta.cos() * half)
                                })
                                .collect();
                            painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                        }
                    }
                    DoorType::Sliding => {
                        outline_extent = half_wall + 8.0;
                        let track_off = side * (half_wall + 2.5);
                        let leaf_off = side * (half_wall + 5.0);
                        painter.line_segment(
                            [start + track_off, end + track_off],
                            Stroke::new(1.0, arc_color),
                        );
                        let slide_end = hinge + along * leaf;
                        painter.line_segment(
                            [hinge + leaf_off, slide_end + leaf_off],
                            Stroke::new(2.4, leaf_color),
                        );
                        // Sliding direction arrow.
                        let mid = start.lerp(end, 0.5) + side * (half_wall + 9.0);
                        let tip = mid + along * (half * 0.45);
                        painter.line_segment(
                            [mid - along * (half * 0.45), tip],
                            Stroke::new(1.1, arc_color),
                        );
                        painter.line_segment(
                            [tip, tip - along * 4.5 + side * 2.5],
                            Stroke::new(1.1, arc_color),
                        );
                        painter.line_segment(
                            [tip, tip - along * 4.5 - side * 2.5],
                            Stroke::new(1.1, arc_color),
                        );
                    }
                    DoorType::PocketSliding => {
                        outline_extent = half_wall + 6.0;
                        let pocket_end = hinge - along * leaf;
                        let p_off = normal * (half_wall * 0.45);
                        painter.extend(Shape::dashed_line(
                            &[
                                hinge + p_off,
                                pocket_end + p_off,
                                pocket_end - p_off,
                                hinge - p_off,
                            ],
                            Stroke::new(1.0, arc_color),
                            4.0,
                            3.0,
                        ));
                        painter.line_segment(
                            [hinge, hinge + along * (leaf * 0.65)],
                            Stroke::new(2.4, leaf_color),
                        );
                    }
                    DoorType::Pivot => {
                        let pivot = hinge + along * (leaf * 0.20);
                        let short_r = leaf * 0.20;
                        let long_r = leaf * 0.80;
                        outline_extent = long_r + 4.0;
                        painter.circle_filled(pivot, 2.5, leaf_color);
                        painter.line_segment(
                            [pivot - side * short_r, pivot + side * long_r],
                            Stroke::new(2.2, leaf_color),
                        );
                        let big_arc: Vec<Pos2> = (0..=16)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 16.0;
                                pivot
                                    + along * (theta.sin() * long_r)
                                    + side * (theta.cos() * long_r)
                            })
                            .collect();
                        let small_arc: Vec<Pos2> = (0..=10)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 10.0;
                                pivot
                                    - along * (theta.sin() * short_r)
                                    - side * (theta.cos() * short_r)
                            })
                            .collect();
                        painter.add(Shape::line(big_arc, Stroke::new(1.0, arc_color)));
                        painter.add(Shape::line(small_arc, Stroke::new(1.0, arc_color)));
                    }
                    DoorType::Bifold => {
                        outline_extent = half * 0.65 + 4.0;
                        let segs = 4;
                        let step_along = leaf * 0.72 / segs as f32;
                        let peak = half * 0.55;
                        let pts: Vec<Pos2> = (0..=segs)
                            .map(|i| {
                                let s = if i % 2 == 1 {
                                    side * peak
                                } else {
                                    egui::Vec2::ZERO
                                };
                                hinge + along * (step_along * i as f32) + s
                            })
                            .collect();
                        painter.add(Shape::line(pts, Stroke::new(2.0, leaf_color)));
                        painter.extend(Shape::dashed_line(
                            &[start, end],
                            Stroke::new(1.0, arc_color),
                            4.0,
                            3.0,
                        ));
                    }
                    DoorType::Revolving => {
                        outline_extent = half + 4.0;
                        let mid = start.lerp(end, 0.5);
                        painter.circle_stroke(mid, half, Stroke::new(1.2, arc_color));
                        let diag1 = (direction + normal).normalized() * half;
                        let diag2 = (direction - normal).normalized() * half;
                        painter
                            .line_segment([mid - diag1, mid + diag1], Stroke::new(2.0, leaf_color));
                        painter
                            .line_segment([mid - diag2, mid + diag2], Stroke::new(2.0, leaf_color));
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        side,
                        -(half_wall + 4.0),
                        outline_extent,
                        op_color,
                    );
                }
            }
            OpeningKind::SaloonDoor => {
                let leaf_color = if op_color == Color32::TRANSPARENT {
                    theme::WALL
                } else {
                    op_color
                };
                let arc_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                // Two half leaves, hinged at each jamb, swinging both ways.
                for (hinge, toward) in [(start, 1.0), (end, -1.0)] {
                    painter
                        .line_segment([hinge, hinge + normal * half], Stroke::new(2.0, leaf_color));
                    let flip = if opening.flip_side { -1.0 } else { 1.0 };
                    for side in [flip, -flip] {
                        let arc: Vec<Pos2> = (0..=12)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 12.0;
                                hinge
                                    + direction * (toward * theta.sin() * half)
                                    + normal * (side * theta.cos() * half)
                            })
                            .collect();
                        if side == flip {
                            painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                        } else {
                            painter.extend(Shape::dashed_line(
                                &arc,
                                Stroke::new(1.0, arc_color),
                                3.0,
                                3.0,
                            ));
                        }
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half + 4.0),
                        half + 4.0,
                        op_color,
                    );
                }
            }
            OpeningKind::Gap => {
                // No leaf: a dashed line marks the open span.
                let gap_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                painter.extend(Shape::dashed_line(
                    &[start, end],
                    Stroke::new(1.2, gap_color),
                    6.0,
                    4.0,
                ));
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half_wall + 4.0),
                        half_wall + 4.0,
                        op_color,
                    );
                }
            }
            OpeningKind::Window => {
                use crate::domain::openings::WindowType;
                let window_color = if op_color == Color32::TRANSPARENT {
                    theme::WINDOW
                } else {
                    op_color
                };
                let offset = (half_wall - 0.5).max(2.5);
                let side = if opening.flip_side { -normal } else { normal };
                painter.add(Shape::convex_polygon(
                    vec![
                        start + normal * offset,
                        end + normal * offset,
                        end - normal * offset,
                        start - normal * offset,
                    ],
                    window_color.gamma_multiply(0.12),
                    Stroke::NONE,
                ));
                for s in [-1.0, 1.0] {
                    painter.line_segment(
                        [start + normal * offset * s, end + normal * offset * s],
                        Stroke::new(1.5, window_color),
                    );
                }
                let mid = start.lerp(end, 0.5);
                let overlap = (half * 0.14).min(6.0);
                match opening.effective_window_type() {
                    WindowType::Sliding => {
                        // Two slightly offset overlapping sliding sashes.
                        let off = offset * 0.35;
                        painter.line_segment(
                            [
                                start + normal * off,
                                mid + direction * overlap + normal * off,
                            ],
                            Stroke::new(1.3, window_color),
                        );
                        painter.line_segment(
                            [mid - direction * overlap - normal * off, end - normal * off],
                            Stroke::new(1.3, window_color),
                        );
                    }
                    WindowType::Basculante => {
                        // High-sill awning window: dashed horizontal projection on both sides + central axis.
                        let proj = offset + 4.0;
                        painter.extend(Shape::dashed_line(
                            &[start + side * proj, end + side * proj],
                            Stroke::new(1.0, window_color),
                            4.0,
                            3.0,
                        ));
                        painter.line_segment([start, end], Stroke::new(1.0, window_color));
                        painter.circle_filled(mid, 2.0, window_color);
                    }
                    WindowType::MaximAr => {
                        // Outward-projecting maxim-ar sash triangle/trapezoid in dashed lines.
                        let proj = offset + 7.0;
                        painter.line_segment([start, end], Stroke::new(1.0, window_color));
                        painter.extend(Shape::dashed_line(
                            &[start, mid + side * proj, end],
                            Stroke::new(1.1, window_color),
                            4.0,
                            3.0,
                        ));
                    }
                    WindowType::Casement | WindowType::TiltTurn => {
                        // Two opening leaves with technical quarter arcs.
                        let leaf_r = half * 0.75;
                        for (h, dir_h) in [(start, direction), (end, -direction)] {
                            painter.line_segment(
                                [h, h + side * leaf_r],
                                Stroke::new(1.4, window_color),
                            );
                            let arc: Vec<Pos2> = (0..=10)
                                .map(|step| {
                                    let th = std::f32::consts::FRAC_PI_2 * step as f32 / 10.0;
                                    h + dir_h * (th.sin() * leaf_r) + side * (th.cos() * leaf_r)
                                })
                                .collect();
                            painter.extend(Shape::dashed_line(
                                &arc,
                                Stroke::new(0.9, window_color),
                                3.5,
                                2.5,
                            ));
                        }
                        if opening.effective_window_type() == WindowType::TiltTurn {
                            painter.extend(Shape::dashed_line(
                                &[start, mid + side * (offset + 5.0), end],
                                Stroke::new(0.9, window_color),
                                3.0,
                                2.5,
                            ));
                        }
                    }
                    WindowType::Fixed => {
                        // Single fixed glass pane with small central cross mark.
                        painter.line_segment([start, end], Stroke::new(1.3, window_color));
                        let cross = 3.0;
                        painter.line_segment(
                            [
                                mid - direction * cross - normal * cross,
                                mid + direction * cross + normal * cross,
                            ],
                            Stroke::new(1.0, window_color),
                        );
                        painter.line_segment(
                            [
                                mid - direction * cross + normal * cross,
                                mid + direction * cross - normal * cross,
                            ],
                            Stroke::new(1.0, window_color),
                        );
                    }
                    WindowType::BayWindow => {
                        // Outward-projecting trapezoidal bay window silhouette.
                        let depth = (offset + 12.0).max(half * 0.45);
                        let inset = half * 0.35;
                        let p1 = start + direction * inset + side * depth;
                        let p2 = end - direction * inset + side * depth;
                        painter.add(Shape::line(
                            vec![start, p1, p2, end],
                            Stroke::new(1.8, window_color),
                        ));
                        painter.line_segment([start, end], Stroke::new(0.9, window_color));
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half_wall + 4.0),
                        half_wall + 4.0,
                        op_color,
                    );
                }
            }
        }
    }
}

pub(super) fn paint_wall_assembly_segment(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    normal: egui::Vec2,
    wall_width: f32,
    assembly: &crate::domain::wall_layers::WallAssembly,
) {
    let total = assembly.total_thickness() as f32;
    for (start_m, end_m, layer) in assembly.layer_offsets() {
        let s0 = (start_m as f32 / total) * wall_width;
        let s1 = (end_m as f32 / total) * wall_width;
        let [r, g, b] = layer.color;
        painter.add(Shape::convex_polygon(
            vec![
                from + normal * s0,
                to + normal * s0,
                to + normal * s1,
                from + normal * s1,
            ],
            Color32::from_rgb(r, g, b),
            Stroke::NONE,
        ));
    }
    let half = wall_width * 0.5;
    let outline = Stroke::new(0.8, theme::WALL);
    painter.line_segment([from - normal * half, to - normal * half], outline);
    painter.line_segment([from + normal * half, to + normal * half], outline);
}

/// A dashed frame around a selected or hovered opening.
#[allow(clippy::too_many_arguments)]
pub(super) fn outline_opening(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    direction: Vec2,
    normal: Vec2,
    near: f32,
    far: f32,
    color: Color32,
) {
    let start = start - direction * 4.0;
    let end = end + direction * 4.0;
    let corners = [
        start + normal * near,
        end + normal * near,
        end + normal * far,
        start + normal * far,
        start + normal * near,
    ];
    painter.extend(Shape::dashed_line(
        &corners,
        Stroke::new(1.2, color),
        4.0,
        3.0,
    ));
}
