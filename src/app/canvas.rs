use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::domain::{
    geometry::{GRID_METERS, Point, format_meters},
    model::{OpeningKind, Project, Wall},
};

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
}

pub fn paint(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selected_wall: Option<u64>,
    preview: Option<Preview>,
) {
    painter.rect_filled(view.rect, 0.0, Color32::from_rgb(248, 250, 251));
    paint_grid(painter, view);

    for room in &project.rooms {
        if let Some((min, max)) = project.room_bounds(room) {
            let rect = Rect::from_two_pos(view.to_screen(min), view.to_screen(max));
            painter.rect_filled(rect, 0.0, Color32::from_rgb(234, 247, 244));
            let center = rect.center();
            painter.text(
                center + egui::vec2(0.0, -10.0),
                Align2::CENTER_CENTER,
                room.kind.label(),
                FontId::proportional(17.0),
                Color32::from_rgb(39, 70, 79),
            );
            painter.text(
                center + egui::vec2(0.0, 13.0),
                Align2::CENTER_CENTER,
                format!(
                    "{} × {}",
                    format_meters(max.x - min.x),
                    format_meters(max.y - min.y)
                ),
                FontId::proportional(12.0),
                Color32::from_rgb(100, 121, 129),
            );
        }
    }

    for wall in &project.walls {
        paint_wall(painter, view, wall, project, selected_wall == Some(wall.id));
    }

    if let Some(preview) = preview {
        match preview {
            Preview::Wall(a, b) => {
                painter.line_segment(
                    [view.to_screen(a), view.to_screen(b)],
                    Stroke::new(6.0, Color32::from_rgb(40, 176, 158)),
                );
                painter.circle_filled(view.to_screen(b), 5.0, Color32::from_rgb(40, 176, 158));
            }
            Preview::Room(a, b) => {
                let rect = Rect::from_two_pos(view.to_screen(a), view.to_screen(b));
                painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(40, 176, 158, 30));
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                ];
                for index in 0..4 {
                    painter.line_segment(
                        [corners[index], corners[(index + 1) % 4]],
                        Stroke::new(4.0, Color32::from_rgb(40, 176, 158)),
                    );
                }
            }
        }
    }
}

fn paint_grid(painter: &Painter, view: ViewTransform) {
    let first = view.to_world(view.rect.left_top());
    let last = view.to_world(view.rect.right_bottom());
    let start_x = (first.x / GRID_METERS).floor() as i32;
    let end_x = (last.x / GRID_METERS).ceil() as i32;
    let start_y = (first.y / GRID_METERS).floor() as i32;
    let end_y = (last.y / GRID_METERS).ceil() as i32;
    for index in start_x..=end_x {
        let x = view
            .to_screen(Point::new(index as f32 * GRID_METERS, 0.0))
            .x;
        let major = index.rem_euclid(4) == 0;
        painter.line_segment(
            [
                Pos2::new(x, view.rect.top()),
                Pos2::new(x, view.rect.bottom()),
            ],
            Stroke::new(
                1.0,
                if major {
                    Color32::from_rgb(216, 227, 231)
                } else {
                    Color32::from_rgb(235, 241, 242)
                },
            ),
        );
    }
    for index in start_y..=end_y {
        let y = view
            .to_screen(Point::new(0.0, index as f32 * GRID_METERS))
            .y;
        let major = index.rem_euclid(4) == 0;
        painter.line_segment(
            [
                Pos2::new(view.rect.left(), y),
                Pos2::new(view.rect.right(), y),
            ],
            Stroke::new(
                1.0,
                if major {
                    Color32::from_rgb(216, 227, 231)
                } else {
                    Color32::from_rgb(235, 241, 242)
                },
            ),
        );
    }
}

fn paint_wall(
    painter: &Painter,
    view: ViewTransform,
    wall: &Wall,
    project: &Project,
    selected: bool,
) {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let direction = (b - a).normalized();
    let normal = egui::vec2(-direction.y, direction.x);
    let screen_length = a.distance(b);
    let color = if selected {
        Color32::from_rgb(36, 160, 147)
    } else {
        Color32::from_rgb(36, 53, 75)
    };
    let mut openings: Vec<_> = project
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect();
    openings.sort_by(|left, right| left.t.total_cmp(&right.t));
    let mut cursor = 0.0;
    for opening in openings {
        let half = opening.kind.width() * view.scale / 2.0;
        let center = opening.t * screen_length;
        let left = center - half;
        let right = center + half;
        painter.line_segment(
            [a + direction * cursor, a + direction * left],
            Stroke::new(8.0, color),
        );
        match opening.kind {
            OpeningKind::Door => {
                let hinge = a + direction * left;
                painter.line_segment(
                    [hinge, hinge + normal * (half * 2.0)],
                    Stroke::new(2.5, Color32::from_rgb(32, 171, 151)),
                );
                let mut arc = Vec::new();
                for step in 0..=16 {
                    let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 16.0;
                    arc.push(
                        hinge + direction * (theta.sin() * half * 2.0)
                            + normal * (theta.cos() * half * 2.0),
                    );
                }
                painter.add(egui::Shape::line(
                    arc,
                    Stroke::new(1.0, Color32::from_rgb(111, 199, 185)),
                ));
            }
            OpeningKind::Window => {
                let start = a + direction * left;
                let end = a + direction * right;
                let window_color = Color32::from_rgb(35, 163, 190);
                painter.line_segment(
                    [start - normal * 3.0, end - normal * 3.0],
                    Stroke::new(2.5, window_color),
                );
                painter.line_segment(
                    [start + normal * 3.0, end + normal * 3.0],
                    Stroke::new(2.5, window_color),
                );
            }
        }
        cursor = right;
    }
    painter.line_segment([a + direction * cursor, b], Stroke::new(8.0, color));
    let midpoint = a.lerp(b, 0.5) + normal * 19.0;
    painter.text(
        midpoint,
        Align2::CENTER_CENTER,
        format_meters(wall.length()),
        FontId::proportional(11.0),
        Color32::from_rgb(88, 108, 121),
    );
}
