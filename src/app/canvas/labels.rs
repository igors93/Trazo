use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};

use crate::app::theme;

use crate::domain::{
    geometry::format_meters,
    label_layout::{self, Area},
    model::Project,
};

use super::{Selection, ViewTransform, wall_stroke_width};

pub(super) fn paint_preview_measure(
    painter: &Painter,
    preferred: Pos2,
    shape_bounds: Rect,
    cursor: Pos2,
    value: &str,
    viewport: Rect,
    occupied: &[Rect],
) {
    if paint_measure(painter, preferred, value, shape_bounds, occupied) {
        return;
    }
    for offset in [
        egui::vec2(0.0, -34.0),
        egui::vec2(0.0, 34.0),
        egui::vec2(-90.0, -34.0),
        egui::vec2(90.0, 34.0),
    ] {
        if paint_measure(painter, cursor + offset, value, viewport, occupied) {
            break;
        }
    }
}

fn paint_measure(
    painter: &Painter,
    position: Pos2,
    value: &str,
    bounds: Rect,
    occupied: &[Rect],
) -> bool {
    let font = FontId::proportional(13.0);
    let galley = painter.layout_no_wrap(value.to_owned(), font, theme::ACCENT_TEXT);
    let rect = Rect::from_center_size(position, galley.size() + egui::vec2(16.0, 8.0));
    if !rect_inside(rect, bounds)
        || occupied
            .iter()
            .any(|other| other.expand(4.0).intersects(rect))
    {
        return false;
    }
    paint_badge(painter, rect, galley, theme::ACCENT_TEXT);
    true
}

pub(super) fn paint_room_labels(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    occupied: &mut Vec<Rect>,
) {
    let name_color = theme::TEXT;
    let measure_color = theme::MUTED;
    let furniture: Vec<Rect> = project
        .label_obstacles()
        .into_iter()
        .map(|(low, high)| Rect::from_two_pos(view.to_screen(low), view.to_screen(high)))
        .collect();
    for room in &project.rooms {
        let Some((anchor, width, height)) = project.room_label_anchor(room) else {
            continue;
        };
        let inner = Rect::from_center_size(
            view.to_screen(anchor),
            egui::vec2(view.pixels(width), view.pixels(height)),
        )
        .shrink(wall_stroke_width(view.scale) / 2.0 + 7.0);
        if inner.width() < 12.0 || inner.height() < 12.0 {
            continue;
        }

        let display_name = room
            .display_name()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let Some(name) = fit_text(
            painter,
            &display_name,
            inner.width(),
            17,
            10,
            name_color,
            true,
        ) else {
            continue;
        };
        if name.size().y > inner.height() {
            continue;
        }
        // Rectangles show their sides, other shapes their area.
        let dimensions = match project.room_bounds(room) {
            Some((min, max)) if project.room_is_rectangle(room) => format!(
                "{} × {}",
                format_meters(max.x - min.x),
                format_meters(max.y - min.y)
            ),
            _ => project
                .room_area(room)
                .map(|area| format!("{area:.2} m²").replace('.', ","))
                .unwrap_or_default(),
        };
        let measure = fit_text(
            painter,
            &dimensions,
            inner.width(),
            12,
            9,
            measure_color,
            false,
        )
        .filter(|galley| name.size().y + 4.0 + galley.size().y <= inner.height());
        let block_height =
            name.size().y + measure.as_ref().map_or(0.0, |galley| 4.0 + galley.size().y);
        let block_width = name
            .size()
            .x
            .max(measure.as_ref().map_or(0.0, |galley| galley.size().x));
        let taken = |block: Rect| {
            occupied
                .iter()
                .any(|other| other.expand(4.0).intersects(block))
        };
        let blocked = |block: Rect| {
            taken(block)
                || furniture
                    .iter()
                    .any(|item| item.expand(3.0).intersects(block))
        };
        // Like a room tag placed by hand: the free spot nearest the middle
        // of the room, clear of furniture; the name alone when name and
        // measure do not fit between the pieces; else the middle, over a
        // translucent halo that keeps both the text and the symbols legible.
        let full = egui::vec2(block_width, block_height);
        let (center_pos, measure, size, halo) =
            if let Some(spot) = label_spot(inner, full, &blocked) {
                (spot, measure, full, false)
            } else if let Some(spot) = measure
                .is_some()
                .then(|| label_spot(inner, name.size(), &blocked))
                .flatten()
            {
                (spot, None, name.size(), false)
            } else {
                (inner.center(), measure, full, true)
            };
        let block = Rect::from_center_size(center_pos, size);
        if taken(block) {
            continue;
        }
        if halo && furniture.iter().any(|item| item.intersects(block)) {
            painter.rect_filled(
                block.expand2(egui::vec2(4.0, 1.0)),
                3.0,
                theme::ROOM_FILL.gamma_multiply(0.72),
            );
        }
        let name_center = Pos2::new(center_pos.x, block.top() + name.size().y / 2.0);
        paint_centered_galley(painter, name_center, name, name_color);
        if let Some(measure) = measure {
            let center = Pos2::new(center_pos.x, block.bottom() - measure.size().y / 2.0);
            paint_centered_galley(painter, center, measure, measure_color);
        }
        occupied.push(block);
    }
}

/// Circuit and power tags beside each electrical device, once the plan is
/// zoomed in enough to read them.
pub(super) fn paint_electrical_tags(painter: &Painter, view: ViewTransform, project: &Project) {
    // Below this zoom the tags would pile up; the symbols still show.
    if view.scale < 50.0 {
        return;
    }
    let size = (view.scale * 0.13).clamp(9.5, 13.0);
    for (anchor, text) in project.electrical_tags() {
        let galley = painter.layout_no_wrap(text, FontId::proportional(size), theme::ELECTRICAL);
        let rect = Rect::from_center_size(view.to_screen(anchor), galley.size());
        // A light backdrop keeps the tag readable over walls and furniture.
        painter.rect_filled(
            rect.expand2(egui::vec2(3.0, 1.0)),
            3.0,
            Color32::from_white_alpha(215),
        );
        painter.galley(rect.min, galley, theme::ELECTRICAL);
    }
}

pub(super) fn paint_wall_labels(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selection: Option<Selection>,
    occupied: &mut Vec<Rect>,
) {
    let room_rects: Vec<_> = project
        .rooms
        .iter()
        .filter_map(|room| {
            let (min, max) = project.room_bounds(room)?;
            Some(Rect::from_two_pos(view.to_screen(min), view.to_screen(max)))
        })
        .collect();
    let color = theme::TEXT_2;
    for wall in &project.walls {
        // Room dimensions are shown centrally; show an individual wall only when selected.
        if project
            .rooms
            .iter()
            .any(|room| room.wall_ids.contains(&wall.id))
            && selection != Some(Selection::Wall(wall.id))
        {
            continue;
        }
        let a = view.to_screen(wall.start);
        let b = view.to_screen(wall.end);
        if a.distance(b) < 24.0 {
            continue;
        }
        let galley = painter.layout_no_wrap(
            format_meters(wall.length()),
            FontId::proportional(11.0),
            color,
        );
        let size = galley.size() + egui::vec2(16.0, 8.0);
        let direction = (b - a).normalized();
        let normal = egui::vec2(-direction.y, direction.x);
        let clearance = wall_stroke_width(view.scale) / 2.0
            + normal.x.abs() * size.x / 2.0
            + normal.y.abs() * size.y / 2.0
            + 5.0;
        let mut placed = false;
        for fraction in [0.5, 0.35, 0.65] {
            for side in [1.0, -1.0] {
                let rect =
                    Rect::from_center_size(a.lerp(b, fraction) + normal * clearance * side, size);
                if !rect_inside(rect, view.rect)
                    || room_rects
                        .iter()
                        .any(|room| room.expand(2.0).intersects(rect))
                    || occupied
                        .iter()
                        .any(|other| other.expand(4.0).intersects(rect))
                {
                    continue;
                }
                paint_badge(painter, rect, galley.clone(), color);
                occupied.push(rect);
                placed = true;
                break;
            }
            if placed {
                break;
            }
        }
    }
}

pub(super) fn paint_terrain_label(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    occupied: &mut Vec<Rect>,
) {
    let Some(terrain) = &project.terrain else {
        return;
    };
    let color = theme::TERRAIN_LINE;
    let label = format!("Terreno · {:.2} m²", terrain.area()).replace('.', ",");
    let Some(galley) = fit_text(
        painter,
        &label,
        view.rect.width() - 32.0,
        12,
        10,
        color,
        false,
    ) else {
        return;
    };
    let size = galley.size() + egui::vec2(16.0, 8.0);
    let terrain_rect = Rect::from_two_pos(view.to_screen(terrain.min), view.to_screen(terrain.max));
    let x = terrain_rect.center().x.clamp(
        view.rect.left() + size.x / 2.0,
        view.rect.right() - size.x / 2.0,
    );
    for y in [
        terrain_rect.top() - size.y / 2.0 - 5.0,
        terrain_rect.bottom() + size.y / 2.0 + 5.0,
    ] {
        let rect = Rect::from_center_size(Pos2::new(x, y), size);
        if rect_inside(rect, view.rect)
            && !occupied
                .iter()
                .any(|other| other.expand(4.0).intersects(rect))
        {
            paint_badge(painter, rect, galley, color);
            occupied.push(rect);
            break;
        }
    }
}

fn fit_text(
    painter: &Painter,
    value: &str,
    max_width: f32,
    preferred_size: i32,
    minimum_size: i32,
    color: Color32,
    truncate: bool,
) -> Option<Arc<egui::Galley>> {
    if max_width <= 0.0 {
        return None;
    }
    for size in (minimum_size..=preferred_size).rev() {
        let galley =
            painter.layout_no_wrap(value.to_owned(), FontId::proportional(size as f32), color);
        if galley.size().x <= max_width {
            return Some(galley);
        }
    }
    if !truncate {
        return None;
    }
    let font = FontId::proportional(minimum_size as f32);
    let chars: Vec<char> = value.chars().collect();
    let mut low = 0;
    let mut high = chars.len();
    let mut best = None;
    while low <= high {
        let count = low + (high - low) / 2;
        let candidate = format!("{}…", chars[..count].iter().collect::<String>().trim_end());
        let galley = painter.layout_no_wrap(candidate, font.clone(), color);
        if galley.size().x <= max_width {
            best = Some(galley);
            low = count + 1;
        } else if count == 0 {
            break;
        } else {
            high = count - 1;
        }
    }
    best
}

/// [`label_layout::label_spot`] in screen space.
fn label_spot(inner: Rect, size: egui::Vec2, blocked: &dyn Fn(Rect) -> bool) -> Option<Pos2> {
    let area = |rect: Rect| {
        Area::from_corners(
            (f64::from(rect.min.x), f64::from(rect.min.y)),
            (f64::from(rect.max.x), f64::from(rect.max.y)),
        )
    };
    let rect = |area: Area| {
        Rect::from_min_max(
            Pos2::new(area.min_x as f32, area.min_y as f32),
            Pos2::new(area.max_x as f32, area.max_y as f32),
        )
    };
    label_layout::label_spot(
        area(inner),
        (f64::from(size.x), f64::from(size.y)),
        &|block| blocked(rect(block)),
    )
    .map(|(x, y)| Pos2::new(x as f32, y as f32))
}

fn paint_centered_galley(
    painter: &Painter,
    center: Pos2,
    galley: Arc<egui::Galley>,
    color: Color32,
) {
    painter.galley(center - galley.size() / 2.0, galley, color);
}

fn paint_badge(painter: &Painter, rect: Rect, galley: Arc<egui::Galley>, color: Color32) {
    painter.rect(
        rect,
        4.0,
        Color32::WHITE,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + egui::vec2(8.0, 4.0), galley, color);
}

fn rect_inside(inner: Rect, outer: Rect) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn adjacent_room_labels_fit_without_touching() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(2.0, 2.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(2.0, 0.0), Point::new(4.0, 2.0), "Quarto")
            .unwrap();
        let context = egui::Context::default();
        context.begin_pass(egui::RawInput::default());
        let painter = context.layer_painter(egui::LayerId::background());
        let view = ViewTransform {
            rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0)),
            scale: 60.0,
            pan: egui::Vec2::ZERO,
        };
        let mut occupied = Vec::new();
        paint_room_labels(&painter, view, &project, &mut occupied);
        assert_eq!(occupied.len(), 2);
        assert!(!occupied[0].intersects(occupied[1]));
        for (room, label) in project.rooms.iter().zip(occupied.iter().copied()) {
            let (min, max) = project.room_bounds(room).unwrap();
            let room_rect = Rect::from_two_pos(view.to_screen(min), view.to_screen(max));
            assert!(rect_inside(label, room_rect));
        }
        let selected_wall = project.rooms[0].wall_ids[0];
        paint_wall_labels(
            &painter,
            view,
            &project,
            Some(Selection::Wall(selected_wall)),
            &mut occupied,
        );
        assert_eq!(occupied.len(), 3);
        assert!(!occupied[2].intersects(occupied[0]));
        context.end_pass().textures_delta.clear();
    }
}
