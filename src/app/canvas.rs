use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::theme;
use crate::domain::{
    annotations::{self, centimetres},
    fixtures::{Fixture, SymbolShape, Weight},
    geometry::{GRID_METERS, Point, WALL_THICKNESS_METERS, format_meters},
    model::{OpeningKind, Project, Wall},
};

use super::Selection;

mod labels;
pub mod overlay;
use labels::{paint_preview_measure, paint_room_labels, paint_terrain_label, paint_wall_labels};

#[derive(Clone, Copy)]
pub struct ViewTransform {
    pub rect: Rect,
    pub scale: f32,
    pub pan: Vec2,
}

impl ViewTransform {
    pub fn to_screen(self, point: Point) -> Pos2 {
        self.rect.center() + self.pan + egui::vec2(point.x * self.scale, point.y * self.scale)
    }

    pub fn to_world(self, position: Pos2) -> Point {
        let offset = position - self.rect.center() - self.pan;
        Point::new(offset.x / self.scale, offset.y / self.scale)
    }
}

#[derive(Clone, Copy)]
pub enum Preview {
    Wall(Point, Point),
    Room(Point, Point),
    Terrain(Point, Point),
}

/// What the canvas shows besides the project itself.
#[derive(Clone, Copy, Default)]
pub struct PaintOptions {
    pub selection: Option<Selection>,
    /// The element under the pointer, highlighted before it is picked.
    pub hover: Option<Selection>,
    pub preview: Option<Preview>,
    pub show_grid: bool,
    /// The fixture about to be placed, drawn under the pointer.
    pub ghost: Option<Fixture>,
    /// Dimension chains around the building.
    pub dimensions: bool,
}

/// Dimension chains in centimetres above and to the left of the building:
/// one with every wall face, one with the total. Returns the bands they
/// take, so labels keep clear of them.
fn paint_dimensions(painter: &Painter, view: ViewTransform, project: &Project) -> Vec<Rect> {
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
        painter.line_segment(
            [Pos2::new(xs[0], row), Pos2::new(*xs.last().unwrap(), row)],
            stroke,
        );
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
                painter.galley(at - egui::vec2(galley.size().x / 2.0, galley.size().y), galley, theme::TEXT_2);
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
        painter.line_segment(
            [Pos2::new(column, ys[0]), Pos2::new(column, *ys.last().unwrap())],
            stroke,
        );
        for y in &ys {
            painter.line_segment([Pos2::new(column - 5.0, *y), Pos2::new(column + 8.0, *y)], stroke);
            painter.line_segment(
                [Pos2::new(column - 3.5, y + 3.5), Pos2::new(column + 3.5, y - 3.5)],
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
        Rect::from_min_max(Pos2::new(corner.x, corner.y - 62.0), Pos2::new(far.x, corner.y - 16.0)),
        Rect::from_min_max(Pos2::new(corner.x - 62.0, corner.y), Pos2::new(corner.x - 16.0, far.y)),
    ]
}

#[derive(Clone, Copy, PartialEq)]
pub enum FixtureTone {
    Normal,
    Hovered,
    Selected,
    Ghost,
}

/// Draws a fixture's plan symbol (see `domain::fixtures`).
pub fn paint_fixture(painter: &Painter, view: ViewTransform, fixture: &Fixture, tone: FixtureTone) {
    let (ink, detail, fill) = match tone {
        FixtureTone::Normal => (theme::TEXT_2, theme::MUTED, Color32::WHITE),
        FixtureTone::Hovered => (theme::HOVER, theme::HOVER, Color32::WHITE),
        FixtureTone::Selected => (theme::SELECTION, theme::SELECTION, theme::ACCENT_SOFTER),
        FixtureTone::Ghost => (
            theme::ACCENT,
            theme::ACCENT.gamma_multiply(0.8),
            theme::ACCENT.gamma_multiply(0.08),
        ),
    };
    let screen = |local: Point| view.to_screen(fixture.to_world(local));
    let (hw, hd) = (fixture.width / 2.0, fixture.depth / 2.0);
    painter.add(Shape::convex_polygon(
        [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)]
            .iter()
            .map(|(x, y)| screen(Point::new(*x, *y)))
            .collect(),
        fill,
        Stroke::NONE,
    ));
    for shape in fixture.symbol() {
        match shape {
            SymbolShape::Polyline {
                points,
                closed,
                dashed,
                weight,
            } => {
                let stroke = match weight {
                    Weight::Outline => Stroke::new(1.4, ink),
                    Weight::Detail => Stroke::new(0.9, detail),
                };
                let mut points: Vec<Pos2> = points.into_iter().map(screen).collect();
                if dashed {
                    if closed {
                        points.push(points[0]);
                    }
                    painter.extend(Shape::dashed_line(&points, stroke, 4.0, 3.0));
                } else if closed {
                    painter.add(Shape::closed_line(points, stroke));
                } else {
                    painter.add(Shape::line(points, stroke));
                }
            }
            SymbolShape::Circle {
                center,
                radius,
                weight,
            } => {
                let stroke = match weight {
                    Weight::Outline => Stroke::new(1.4, ink),
                    Weight::Detail => Stroke::new(0.9, detail),
                };
                painter.circle_stroke(screen(center), radius * view.scale, stroke);
            }
        }
    }
    if fixture.kind.needs_label() {
        let (x, y) = fixture.extent();
        let galley = painter.layout_no_wrap(
            fixture.kind.label().to_owned(),
            theme::regular(10.5),
            ink,
        );
        let room = egui::vec2(x * view.scale - 6.0, y * view.scale - 4.0);
        if galley.size().x <= room.x && galley.size().y <= room.y {
            let center = view.to_screen(fixture.center);
            let backing = Rect::from_center_size(center, galley.size() + egui::vec2(6.0, 2.0));
            painter.rect_filled(backing, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 215));
            painter.galley(center - galley.size() / 2.0, galley, ink);
        }
    }
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

/// A property line: long dash, short dash.
fn dash_dot(painter: &Painter, points: &[Pos2], stroke: Stroke) {
    painter.extend(Shape::dashed_line_with_offset(
        points,
        stroke,
        &[14.0, 2.5],
        &[4.0, 4.0],
        0.0,
    ));
}

pub fn paint(painter: &Painter, view: ViewTransform, project: &Project, options: PaintOptions) {
    let PaintOptions {
        selection,
        hover,
        preview,
        show_grid,
        ghost,
        dimensions,
    } = options;
    painter.rect_filled(view.rect, 0.0, theme::CANVAS);
    if show_grid {
        paint_grid(painter, view);
    }

    if let Some(terrain) = &project.terrain {
        let rect = Rect::from_two_pos(view.to_screen(terrain.min), view.to_screen(terrain.max));
        let selected = selection == Some(Selection::Terrain(terrain.id));
        let hovered = hover == Some(Selection::Terrain(terrain.id));
        painter.rect_filled(
            rect,
            0.0,
            if selected {
                theme::TERRAIN_FILL_SELECTED
            } else {
                theme::TERRAIN_FILL
            },
        );
        let (width, color) = if selected {
            (2.2, theme::SELECTION)
        } else if hovered {
            (2.0, theme::HOVER)
        } else {
            (1.6, theme::TERRAIN_LINE)
        };
        dash_dot(
            painter,
            &[
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
                rect.left_top(),
            ],
            Stroke::new(width, color),
        );
        for corner in [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ] {
            painter.circle_filled(corner, 3.0, color);
        }
    }

    for room in &project.rooms {
        let selected = selection == Some(Selection::Room(room.id));
        let hovered = hover == Some(Selection::Room(room.id));
        let fill_color = if selected {
            theme::ROOM_FILL_SELECTED
        } else if hovered {
            theme::ROOM_FILL_HOVER
        } else {
            theme::ROOM_FILL
        };

        if let Some(poly) = project.room_polygon(room) {
            let triangles = crate::domain::geometry::triangulate_polygon(&poly);
            for tri in triangles {
                let p0 = view.to_screen(tri[0]);
                let p1 = view.to_screen(tri[1]);
                let p2 = view.to_screen(tri[2]);
                painter.add(egui::Shape::convex_polygon(
                    vec![p0, p1, p2],
                    fill_color,
                    Stroke::NONE,
                ));
            }
        } else if let Some((min, max)) = project.room_bounds(room) {
            let rect = Rect::from_two_pos(view.to_screen(min), view.to_screen(max));
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(204, 63, 51, 120)),
                egui::StrokeKind::Inside,
            );
        }
    }

    for wall in &project.walls {
        let in_room = |target: Option<Selection>| {
            project.rooms.iter().any(|room| {
                target == Some(Selection::Room(room.id)) && room.wall_ids.contains(&wall.id)
            })
        };
        let tone = if selection == Some(Selection::Wall(wall.id)) || in_room(selection) {
            WallTone::Selected
        } else if hover == Some(Selection::Wall(wall.id)) || in_room(hover) {
            WallTone::Hovered
        } else {
            WallTone::Normal
        };
        paint_wall(painter, view, wall, project, tone, selection, hover);
    }

    for fixture in &project.fixtures {
        let tone = if selection == Some(Selection::Fixture(fixture.id)) {
            FixtureTone::Selected
        } else if hover == Some(Selection::Fixture(fixture.id)) {
            FixtureTone::Hovered
        } else {
            FixtureTone::Normal
        };
        paint_fixture(painter, view, fixture, tone);
    }

    let mut occupied = Vec::new();
    if dimensions {
        occupied.extend(paint_dimensions(painter, view, project));
    }

    paint_room_labels(painter, view, project, &mut occupied);
    paint_wall_labels(painter, view, project, selection, &mut occupied);
    paint_terrain_label(painter, view, project, &mut occupied);

    if project.walls.is_empty()
        && project.rooms.is_empty()
        && project.terrain.is_none()
        && preview.is_none()
    {
        paint_empty_state(painter, view.rect);
    }

    if let Some(preview) = preview {
        paint_preview(painter, view, project, preview, &occupied);
    }
    if let Some(ghost) = &ghost {
        paint_fixture(painter, view, ghost, FixtureTone::Ghost);
    }
}

fn paint_empty_state(painter: &Painter, area: Rect) {
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

fn paint_preview(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    preview: Preview,
    occupied: &[Rect],
) {
    let valid_color = theme::ACCENT;
    let invalid_color = theme::DANGER;
    match preview {
        Preview::Wall(a, b) => {
            let valid = a.distance(b) >= 0.24
                && project
                    .terrain
                    .as_ref()
                    .is_none_or(|t| t.contains(a) && t.contains(b));
            let color = if valid { valid_color } else { invalid_color };
            painter.line_segment(
                [view.to_screen(a), view.to_screen(b)],
                Stroke::new(
                    wall_screen_width(view.scale, project.masonry.thickness()),
                    color.gamma_multiply(0.85),
                ),
            );
            painter.circle_filled(view.to_screen(a), 3.5, color);
            painter.circle_filled(view.to_screen(b), 4.5, color);
            let midpoint = view.to_screen(a).lerp(view.to_screen(b), 0.5);
            paint_preview_measure(
                painter,
                midpoint + egui::vec2(0.0, -22.0),
                view.rect,
                view.to_screen(b),
                &format_meters(a.distance(b)),
                view.rect,
                occupied,
            );
        }
        Preview::Room(a, b) => {
            let w = (b.x - a.x).abs();
            let h = (b.y - a.y).abs();
            let min = Point::new(a.x.min(b.x), a.y.min(b.y));
            let max = Point::new(a.x.max(b.x), a.y.max(b.y));
            let valid = w >= 0.5
                && h >= 0.5
                && project
                    .terrain
                    .as_ref()
                    .is_none_or(|t| t.contains(min) && t.contains(max));
            let color = if valid { valid_color } else { invalid_color };
            let rect = Rect::from_two_pos(view.to_screen(a), view.to_screen(b));
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
                        wall_screen_width(view.scale, project.masonry.thickness()),
                        color.gamma_multiply(0.85),
                    ),
                );
            }
            paint_preview_measure(
                painter,
                rect.center(),
                rect.shrink(wall_stroke_width(view.scale) / 2.0 + 4.0),
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

fn paint_grid(painter: &Painter, view: ViewTransform) {
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
    let color = |value: f32| {
        if (value / major_every).fract().abs() < 0.001
            || (value / major_every).fract().abs() > 0.999
        {
            theme::GRID_MAJOR
        } else {
            theme::GRID_MINOR
        }
    };
    for index in start_x..=end_x {
        let value = index as f32 * step;
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
        let value = index as f32 * step;
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

#[derive(Clone, Copy, PartialEq)]
enum WallTone {
    Normal,
    Hovered,
    Selected,
}

/// Paints a straight run of wall from `from` to `to` with square ends, so
/// walls meeting at a corner close it without a notch.
fn wall_run(painter: &Painter, from: Pos2, to: Pos2, width: f32, color: Color32) {
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

fn paint_wall(
    painter: &Painter,
    view: ViewTransform,
    wall: &Wall,
    project: &Project,
    tone: WallTone,
    selection: Option<Selection>,
    hover: Option<Selection>,
) {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let direction = (b - a).normalized();
    let normal = egui::vec2(-direction.y, direction.x);
    let screen_length = a.distance(b);
    let wall_width = wall_screen_width(view.scale, wall.thickness());
    let cap = direction * (wall_width / 2.0);
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
    let mut cursor = 0.0;
    let mut run_start = a - cap;
    for opening in openings {
        let op_selected = selection == Some(Selection::Opening(opening.id));
        let op_hovered = hover == Some(Selection::Opening(opening.id));
        let half = opening.width() * view.scale / 2.0;
        let center = opening.t * screen_length;
        let left = center - half;
        let right = center + half;
        if left > cursor {
            wall_run(painter, run_start, a + direction * left, wall_width, color);
        }
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
            1.2,
            if op_color == Color32::TRANSPARENT {
                color
            } else {
                op_color
            },
        );
        let half_wall = wall_width / 2.0;
        for point in [start, end] {
            painter.line_segment([point - normal * half_wall, point + normal * half_wall], jamb);
        }
        match opening.kind {
            OpeningKind::Door => {
                let leaf_color = if op_color == Color32::TRANSPARENT {
                    theme::WALL
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
                painter.line_segment([hinge, hinge + side * leaf], Stroke::new(2.2, leaf_color));
                let arc: Vec<Pos2> = (0..=20)
                    .map(|step| {
                        let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 20.0;
                        hinge + along * (theta.sin() * leaf) + side * (theta.cos() * leaf)
                    })
                    .collect();
                painter.add(Shape::line(
                    arc,
                    Stroke::new(
                        1.0,
                        if op_color == Color32::TRANSPARENT {
                            theme::DOOR_ARC
                        } else {
                            op_color
                        },
                    ),
                ));
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        side,
                        -(half_wall + 4.0),
                        leaf + 4.0,
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
                    painter.line_segment(
                        [hinge, hinge + normal * half],
                        Stroke::new(2.0, leaf_color),
                    );
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
                let window_color = if op_color == Color32::TRANSPARENT {
                    theme::WINDOW
                } else {
                    op_color
                };
                let offset = (half_wall - 0.5).max(2.5);
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
                for side in [-1.0, 1.0] {
                    painter.line_segment(
                        [start + normal * offset * side, end + normal * offset * side],
                        Stroke::new(1.6, window_color),
                    );
                }
                painter.line_segment([start, end], Stroke::new(1.0, window_color));
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
        cursor = right;
        run_start = end;
    }
    wall_run(painter, run_start, b + cap, wall_width, color);
}

/// A dashed frame around a selected or hovered opening.
#[allow(clippy::too_many_arguments)]
fn outline_opening(
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
    painter.extend(Shape::dashed_line(&corners, Stroke::new(1.2, color), 4.0, 3.0));
}

fn wall_stroke_width(scale: f32) -> f32 {
    (scale * WALL_THICKNESS_METERS).clamp(1.5, 10.0)
}

/// A wall drawn at its real thickness, never thinner than a hairline.
fn wall_screen_width(scale: f32, thickness: f32) -> f32 {
    (scale * thickness).max(1.5)
}

#[cfg(test)]
mod tests {
    use super::wall_stroke_width;

    #[test]
    fn wall_lines_get_thinner_when_zoomed_out() {
        assert!(wall_stroke_width(18.0) < 3.0);
        assert!(wall_stroke_width(60.0) > 6.0);
    }
}
