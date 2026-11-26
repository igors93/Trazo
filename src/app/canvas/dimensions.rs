//! Measurements on the plan: element tags, dimension chains, associative
//! dimensions and the measure tool's line.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke};

use crate::app::theme;
use crate::domain::{
    annotations::{self, centimetres},
    geometry::{Point, format_meters},
    model::Project,
};

use super::ViewTransform;

/// Door and window type tags ("P1 80/210") beside each opening: on the side
/// a door does not swing to, or past its leaf when that side is taken. A tag
/// that would cover another label is left out until the view is closer.
pub(super) fn paint_frame_tags(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    occupied: &mut Vec<Rect>,
) {
    if view.scale < 24.0 {
        return;
    }
    let types = annotations::frame_types(project);
    for opening in &project.openings {
        let (Some(frame), Some(wall)) = (
            annotations::frame_of(&types, opening),
            project.walls.iter().find(|wall| wall.id == opening.wall_id),
        ) else {
            continue;
        };
        let length = wall.length().max(1e-4);
        let along = Point::new(
            (wall.end.x - wall.start.x) / length,
            (wall.end.y - wall.start.y) / length,
        );
        let mut normal = Point::new(-along.y, along.x);
        if opening.kind.is_door() && !opening.flip_side {
            normal = Point::new(-normal.x, -normal.y);
        }
        let center = Point::new(
            wall.start.x + (wall.end.x - wall.start.x) * opening.t,
            wall.start.y + (wall.end.y - wall.start.y) * opening.t,
        );
        let galley = painter.layout_no_wrap(frame.tag(), theme::strong(10.0), theme::ACCENT_TEXT);
        let size = galley.size() + egui::vec2(8.0, 2.0);
        let across = |direction: Point| {
            view.metres(
                direction.x.abs() as f32 * size.x / 2.0 + direction.y.abs() as f32 * size.y / 2.0,
            )
        };
        let slide = opening.width() / 2.0 + across(along) + view.metres(4.0);
        let rect_at = |side: f64, beyond: f64, shift: f64| {
            let offset = wall.thickness() / 2.0 + beyond + view.metres(4.0) + across(normal);
            Rect::from_center_size(
                view.to_screen(Point::new(
                    center.x + normal.x * offset * side + along.x * shift,
                    center.y + normal.y * offset * side + along.y * shift,
                )),
                size,
            )
        };
        let leaf = if opening.kind.is_door() {
            opening.width()
        } else {
            0.0
        };
        let candidates = [
            rect_at(1.0, 0.0, 0.0),
            rect_at(-1.0, leaf, 0.0),
            rect_at(1.0, 0.0, slide),
            rect_at(1.0, 0.0, -slide),
            rect_at(-1.0, 0.0, slide),
            rect_at(-1.0, 0.0, -slide),
        ];
        let Some(rect) = candidates
            .into_iter()
            .find(|rect| !occupied.iter().any(|taken| taken.intersects(*rect)))
        else {
            continue;
        };
        occupied.push(rect.expand(2.0));
        painter.rect(
            rect,
            3.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            Stroke::new(0.8, theme::ACCENT_RING),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.min + egui::vec2(4.0, 1.0), galley, theme::ACCENT_TEXT);
    }
}

/// Dimension chains in centimetres above and to the left of the building:
/// one with every wall face, one with the total. Returns the bands they
/// take, so labels keep clear of them.
pub(super) fn paint_dimensions(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
) -> Vec<Rect> {
    let Some(dims) = annotations::building_dimensions(project) else {
        return Vec::new();
    };
    let stroke = Stroke::new(1.0, theme::MUTED);
    let tick = Stroke::new(1.4, theme::TEXT_2);
    let font = theme::regular(10.5);
    let corner = view.to_screen(dims.min);
    let total = [dims.min, dims.max];

    // Along the top, reading left to right.
    for (row, stations) in [
        (corner.y - 24.0, dims.xs.clone()),
        (corner.y - 46.0, vec![total[0].x, total[1].x]),
    ] {
        let xs: Vec<f32> = stations
            .iter()
            .map(|x| view.to_screen(Point::new(*x, 0.0)).x)
            .collect();
        if let (Some(&first), Some(&last)) = (xs.first(), xs.last()) {
            painter.line_segment([Pos2::new(first, row), Pos2::new(last, row)], stroke);
        }
        for x in &xs {
            painter.line_segment([Pos2::new(*x, row - 5.0), Pos2::new(*x, row + 8.0)], stroke);
            painter.line_segment(
                [Pos2::new(x - 3.5, row + 3.5), Pos2::new(x + 3.5, row - 3.5)],
                tick,
            );
        }
        for (pair, span) in stations.windows(2).zip(xs.windows(2)) {
            let galley =
                painter.layout_no_wrap(centimetres(pair[1] - pair[0]), font.clone(), theme::TEXT_2);
            if galley.size().x + 4.0 <= span[1] - span[0] {
                let at = Pos2::new((span[0] + span[1]) / 2.0, row - 3.0);
                painter.galley(
                    at - egui::vec2(galley.size().x / 2.0, galley.size().y),
                    galley,
                    theme::TEXT_2,
                );
            }
        }
    }

    // Down the left side, reading bottom to top.
    for (column, stations) in [
        (corner.x - 24.0, dims.ys.clone()),
        (corner.x - 46.0, vec![total[0].y, total[1].y]),
    ] {
        let ys: Vec<f32> = stations
            .iter()
            .map(|y| view.to_screen(Point::new(0.0, *y)).y)
            .collect();
        if let (Some(&first), Some(&last)) = (ys.first(), ys.last()) {
            painter.line_segment([Pos2::new(column, first), Pos2::new(column, last)], stroke);
        }
        for y in &ys {
            painter.line_segment(
                [Pos2::new(column - 5.0, *y), Pos2::new(column + 8.0, *y)],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(column - 3.5, y + 3.5),
                    Pos2::new(column + 3.5, y - 3.5),
                ],
                tick,
            );
        }
        for (pair, span) in stations.windows(2).zip(ys.windows(2)) {
            let galley =
                painter.layout_no_wrap(centimetres(pair[1] - pair[0]), font.clone(), theme::TEXT_2);
            let (width, height) = (galley.size().x, galley.size().y);
            if width + 4.0 <= span[1] - span[0] {
                let middle = (span[0] + span[1]) / 2.0;
                let at = Pos2::new(column - 3.0 - height, middle + width / 2.0);
                painter.add(
                    egui::epaint::TextShape::new(at, galley, theme::TEXT_2)
                        .with_angle(-std::f32::consts::FRAC_PI_2),
                );
            }
        }
    }
    let far = view.to_screen(dims.max);
    vec![
        Rect::from_min_max(
            Pos2::new(corner.x, corner.y - 62.0),
            Pos2::new(far.x, corner.y - 16.0),
        ),
        Rect::from_min_max(
            Pos2::new(corner.x - 62.0, corner.y),
            Pos2::new(corner.x - 16.0, far.y),
        ),
    ]
}

/// A measured distance: a dimension line with architectural ticks and its
/// length (plus Δx and Δy when it runs at an angle).
pub fn paint_measurement(painter: &Painter, view: ViewTransform, start: Point, end: Point) {
    let (a, b) = (view.to_screen(start), view.to_screen(end));
    let length = a.distance(b);
    let color = theme::ACCENT_STRONG;
    painter.line_segment([a, b], Stroke::new(1.6, color));
    for point in [a, b] {
        painter.circle_filled(point, 2.6, color);
    }
    if length < 1.0 {
        return;
    }
    let along = (b - a) / length;
    let normal = egui::vec2(-along.y, along.x);
    let slash = (along + normal).normalized() * 6.0;
    for point in [a, b] {
        painter.line_segment([point - slash, point + slash], Stroke::new(2.0, color));
    }
    let (dx, dy) = ((end.x - start.x).abs(), (end.y - start.y).abs());
    let mut text = format_meters(start.distance(end));
    if dx > 0.005 && dy > 0.005 {
        text.push_str(&format!("\nΔx {dx:.2} · Δy {dy:.2}").replace('.', ","));
    }
    let galley = painter.layout(text, theme::strong(12.0), color, f32::INFINITY);
    let center = a.lerp(b, 0.5) - normal * (galley.size().y / 2.0 + 10.0);
    let badge = Rect::from_center_size(center, galley.size() + egui::vec2(12.0, 6.0));
    painter.rect(
        badge,
        4.0,
        Color32::WHITE,
        Stroke::new(1.0, theme::ACCENT_RING),
        egui::StrokeKind::Inside,
    );
    painter.galley(badge.min + egui::vec2(6.0, 3.0), galley, color);
}

pub(super) fn paint_associative_dimensions(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
) {
    use crate::domain::dimensions::ResolvedDimension;
    let line_stroke = Stroke::new(1.0, theme::TEXT_2);
    let tick_stroke = Stroke::new(1.5, theme::WALL);
    for dim in &project.dimensions {
        if !dim.visible {
            continue;
        }
        let Some(resolved) = project.resolve_dimension(dim) else {
            continue;
        };
        match &resolved {
            ResolvedDimension::Linear {
                p1,
                p2,
                dim_p1,
                dim_p2,
                label,
                ..
            } => {
                let s1 = view.to_screen(*p1);
                let s2 = view.to_screen(*p2);
                let d1 = view.to_screen(*dim_p1);
                let d2 = view.to_screen(*dim_p2);
                if s1.distance(d1) > 1.5 {
                    painter.line_segment([s1, d1], line_stroke);
                }
                if s2.distance(d2) > 1.5 {
                    painter.line_segment([s2, d2], line_stroke);
                }
                painter.line_segment([d1, d2], line_stroke);
                for end in [d1, d2] {
                    painter.line_segment(
                        [end + egui::vec2(-3.5, 3.5), end + egui::vec2(3.5, -3.5)],
                        tick_stroke,
                    );
                }
                let mid = d1.lerp(d2, 0.5);
                let galley =
                    painter.layout_no_wrap(label.clone(), theme::regular(11.0), theme::TEXT);
                let pill = Rect::from_center_size(
                    mid + egui::vec2(0.0, -9.0),
                    galley.size() + egui::vec2(6.0, 2.0),
                );
                painter.rect_filled(
                    pill,
                    3.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
                painter.galley(pill.min + egui::vec2(3.0, 1.0), galley, theme::TEXT);
            }
            ResolvedDimension::Angular {
                vertex,
                start_angle,
                sweep,
                radius,
                label,
                ..
            } => {
                let arc_pts: Vec<Pos2> = resolved
                    .arc_points(20)
                    .into_iter()
                    .map(|p| view.to_screen(p))
                    .collect();
                if arc_pts.len() >= 2 {
                    painter.add(egui::Shape::line(arc_pts, line_stroke));
                }
                let mid_angle = start_angle + sweep * 0.5;
                let text_world =
                    crate::domain::drawing::point_at_angle(*vertex, radius + 0.18, mid_angle);
                let text_pos = view.to_screen(text_world);
                painter.text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    label,
                    theme::regular(11.0),
                    theme::TEXT,
                );
            }
        }
    }
}
