use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, Vec2};

use crate::domain::{
    geometry::{GRID_METERS, Point, WALL_THICKNESS_METERS, format_meters},
    model::{OpeningKind, Project, Wall},
};

use super::Selection;

mod labels;
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

pub fn paint(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selection: Option<Selection>,
    preview: Option<Preview>,
) {
    painter.rect_filled(view.rect, 0.0, Color32::from_rgb(248, 250, 251));
    paint_grid(painter, view);

    if let Some(terrain) = &project.terrain {
        let rect = Rect::from_two_pos(view.to_screen(terrain.min), view.to_screen(terrain.max));
        let selected = selection == Some(Selection::Terrain(terrain.id));
        painter.rect_filled(
            rect,
            0.0,
            if selected {
                Color32::from_rgb(227, 239, 255)
            } else {
                Color32::from_rgb(239, 246, 252)
            },
        );
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(79, 124, 169)),
            egui::StrokeKind::Inside,
        );
    }

    for room in &project.rooms {
        if let Some((min, max)) = project.room_bounds(room) {
            let rect = Rect::from_two_pos(view.to_screen(min), view.to_screen(max));
            let selected = selection == Some(Selection::Room(room.id));
            painter.rect_filled(
                rect,
                0.0,
                if selected {
                    Color32::from_rgb(212, 243, 237)
                } else {
                    Color32::from_rgb(234, 247, 244)
                },
            );
        }
    }

    for wall in &project.walls {
        let selected = selection == Some(Selection::Wall(wall.id))
            || project.rooms.iter().any(|room| {
                selection == Some(Selection::Room(room.id)) && room.wall_ids.contains(&wall.id)
            });
        paint_wall(painter, view, wall, project, selected);
    }

    let mut occupied = Vec::new();
    paint_room_labels(painter, view, project, &mut occupied);
    paint_wall_labels(painter, view, project, selection, &mut occupied);
    paint_terrain_label(painter, view, project, &mut occupied);

    if let Some(preview) = preview {
        match preview {
            Preview::Wall(a, b) => {
                painter.line_segment(
                    [view.to_screen(a), view.to_screen(b)],
                    Stroke::new(
                        wall_stroke_width(view.scale),
                        Color32::from_rgb(40, 176, 158),
                    ),
                );
                painter.circle_filled(view.to_screen(b), 5.0, Color32::from_rgb(40, 176, 158));
                let midpoint = view.to_screen(a).lerp(view.to_screen(b), 0.5);
                paint_preview_measure(
                    painter,
                    midpoint + egui::vec2(0.0, -22.0),
                    view.rect,
                    view.to_screen(b),
                    &format_meters(a.distance(b)),
                    view.rect,
                    &occupied,
                );
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
                        Stroke::new(
                            wall_stroke_width(view.scale),
                            Color32::from_rgb(40, 176, 158),
                        ),
                    );
                }
                paint_preview_measure(
                    painter,
                    rect.center(),
                    rect.shrink(wall_stroke_width(view.scale) / 2.0 + 4.0),
                    view.to_screen(b),
                    &format!(
                        "{} × {}",
                        format_meters((b.x - a.x).abs()),
                        format_meters((b.y - a.y).abs())
                    ),
                    view.rect,
                    &occupied,
                );
            }
            Preview::Terrain(a, b) => {
                let rect = Rect::from_two_pos(view.to_screen(a), view.to_screen(b));
                painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(78, 130, 183, 30));
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(2.0, Color32::from_rgb(79, 124, 169)),
                    egui::StrokeKind::Inside,
                );
                paint_preview_measure(
                    painter,
                    rect.center(),
                    rect.shrink(4.0),
                    view.to_screen(b),
                    &format!(
                        "{} × {} · {:.2} m²",
                        format_meters((b.x - a.x).abs()),
                        format_meters((b.y - a.y).abs()),
                        (b.x - a.x).abs() * (b.y - a.y).abs()
                    )
                    .replace('.', ","),
                    view.rect,
                    &occupied,
                );
            }
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
    let first = view.to_world(view.rect.left_top());
    let last = view.to_world(view.rect.right_bottom());
    let start_x = (first.x / step).floor() as i32;
    let end_x = (last.x / step).ceil() as i32;
    let start_y = (first.y / step).floor() as i32;
    let end_y = (last.y / step).ceil() as i32;
    for index in start_x..=end_x {
        let x = view.to_screen(Point::new(index as f32 * step, 0.0)).x;
        let major = (index as f32 * step).rem_euclid(1.0) < 0.001;
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
        let y = view.to_screen(Point::new(0.0, index as f32 * step)).y;
        let major = (index as f32 * step).rem_euclid(1.0) < 0.001;
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
    let wall_width = wall_stroke_width(view.scale);
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
            Stroke::new(wall_width, color),
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
                        hinge
                            + direction * (theta.sin() * half * 2.0)
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
    painter.line_segment([a + direction * cursor, b], Stroke::new(wall_width, color));
}

fn wall_stroke_width(scale: f32) -> f32 {
    (scale * WALL_THICKNESS_METERS).clamp(1.5, 10.0)
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
