use std::fmt::Write as _;

use crate::domain::{
    annotations::{frame_of, frame_types},
    geometry::{Point, format_meters},
    model::Project,
};

use super::escaped;

#[derive(Clone, Copy)]
pub(super) struct LabelBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl LabelBox {
    fn centered(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: x - width / 2.0,
            y: y - height / 2.0,
            width,
            height,
        }
    }

    fn intersects(self, other: Self, gap: f64) -> bool {
        self.x < other.x + other.width + gap
            && self.x + self.width + gap > other.x
            && self.y < other.y + other.height + gap
            && self.y + self.height + gap > other.y
    }
}

fn estimated_text_width(value: &str, font_size: f64) -> f64 {
    value
        .chars()
        .map(|ch| {
            let ratio = if ch.is_whitespace() {
                0.36
            } else if ch.is_uppercase() {
                0.78
            } else if ch.is_ascii_digit() {
                0.66
            } else if ch.is_ascii_punctuation() {
                0.48
            } else {
                0.68
            };
            ratio * font_size
        })
        .sum::<f64>()
        + 2.0
}

pub(super) fn fit_export_text(
    value: &str,
    max_width: f64,
    preferred: u32,
    minimum: u32,
    truncate: bool,
) -> Option<(String, f64)> {
    for size in (minimum..=preferred).rev() {
        if estimated_text_width(value, size as f64) <= max_width {
            return Some((value.to_owned(), size as f64));
        }
    }
    if !truncate {
        return None;
    }
    let chars: Vec<char> = value.chars().collect();
    for count in (0..chars.len()).rev() {
        let candidate = format!("{}…", chars[..count].iter().collect::<String>().trim_end());
        if estimated_text_width(&candidate, minimum as f64) <= max_width {
            return Some((candidate, minimum as f64));
        }
    }
    None
}

pub(super) fn write_room_labels(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f64, f64),
    wall_width: f64,
) -> Vec<LabelBox> {
    let mut occupied = Vec::new();
    for room in &project.rooms {
        let Some((anchor, width, height)) = project.room_label_anchor(room) else {
            continue;
        };
        let half = Point::new(width / 2.0, height / 2.0);
        let (x, y) = point(anchor.minus(half));
        let (right, bottom) = point(anchor.translated(half));
        let padding = wall_width / 2.0 + 8.0;
        let inner = LabelBox {
            x: x + padding,
            y: y + padding,
            width: right - x - padding * 2.0,
            height: bottom - y - padding * 2.0,
        };
        if inner.width <= 0.0 || inner.height <= 0.0 {
            continue;
        }
        let display_name = room
            .display_name()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let Some((name, name_size)) = fit_export_text(&display_name, inner.width, 20, 10, true)
        else {
            continue;
        };
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
        let measure = fit_export_text(&dimensions, inner.width, 14, 10, false)
            .filter(|(_, size)| name_size * 1.2 + 4.0 + size * 1.2 <= inner.height);
        let block_height =
            name_size * 1.2 + measure.as_ref().map_or(0.0, |(_, size)| 4.0 + size * 1.2);
        if block_height > inner.height {
            continue;
        }
        let block_width = estimated_text_width(&name, name_size).max(
            measure
                .as_ref()
                .map_or(0.0, |(text, size)| estimated_text_width(text, *size)),
        );
        let block = LabelBox::centered(
            inner.x + inner.width / 2.0,
            inner.y + inner.height / 2.0,
            block_width,
            block_height,
        );
        if occupied
            .iter()
            .any(|other: &LabelBox| other.intersects(block, 4.0))
        {
            continue;
        }
        let clip_id = format!("room-label-{}", room.id);
        let _ = writeln!(
            svg,
            "<defs><clipPath id=\"{clip_id}\"><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\"/></clipPath></defs><g clip-path=\"url(#{clip_id})\">",
            inner.x, inner.y, inner.width, inner.height
        );
        let name_y = block.y + name_size;
        let center_x = inner.x + inner.width / 2.0;
        let _ = writeln!(
            svg,
            "<text x=\"{center_x:.1}\" y=\"{name_y:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{name_size:.1}\" font-weight=\"700\" fill=\"#1c2b3c\">{}</text>",
            escaped(&name)
        );
        if let Some((measure, measure_size)) = measure {
            let measure_y = block.y + name_size * 1.2 + 4.0 + measure_size;
            let _ = writeln!(
                svg,
                "<text x=\"{center_x:.1}\" y=\"{measure_y:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{measure_size:.1}\" fill=\"#627489\">{measure}</text>"
            );
        }
        svg.push_str("</g>\n");
        occupied.push(block);
    }
    occupied
}

/// Door and window type tags ("P1 80/210") beside each opening, on the
/// side a door does not swing to, or past its leaf when that side is taken.
pub(super) fn write_frame_tags(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f64, f64),
    pixels_per_meter: f64,
    occupied: &mut Vec<LabelBox>,
) {
    let types = frame_types(project);
    let font_size = 12.0;
    for opening in &project.openings {
        let (Some(frame), Some(wall)) = (
            frame_of(&types, opening),
            project.walls.iter().find(|wall| wall.id == opening.wall_id),
        ) else {
            continue;
        };
        let tag = frame.tag();
        let width = estimated_text_width(&tag, font_size) + 6.0;
        let height = font_size * 1.2 + 2.0;
        let a = point(wall.start);
        let b = point(wall.end);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).sqrt().max(1e-4);
        let mut normal = (-dy / length, dx / length);
        if opening.kind.is_door() && !opening.flip_side {
            normal = (-normal.0, -normal.1);
        }
        let center = (a.0 + dx * opening.t, a.1 + dy * opening.t);
        let leaf = if opening.kind.is_door() {
            opening.width() * pixels_per_meter
        } else {
            0.0
        };
        let along = (dx / length, dy / length);
        let across = normal.0.abs() * width / 2.0 + normal.1.abs() * height / 2.0;
        let lengthwise = along.0.abs() * width / 2.0 + along.1.abs() * height / 2.0;
        let slide = opening.width() * pixels_per_meter / 2.0 + lengthwise + 4.0;
        let candidate = |side: f64, beyond: f64, shift: f64| {
            let offset = wall.thickness() * pixels_per_meter / 2.0 + beyond + 5.0 + across;
            LabelBox::centered(
                center.0 + normal.0 * offset * side + along.0 * shift,
                center.1 + normal.1 * offset * side + along.1 * shift,
                width,
                height,
            )
        };
        let candidates = [
            candidate(1.0, 0.0, 0.0),
            candidate(-1.0, leaf, 0.0),
            candidate(1.0, 0.0, slide),
            candidate(1.0, 0.0, -slide),
            candidate(-1.0, 0.0, slide),
            candidate(-1.0, 0.0, -slide),
        ];
        let Some(rect) = candidates
            .into_iter()
            .find(|rect| !occupied.iter().any(|other| other.intersects(*rect, 3.0)))
        else {
            continue;
        };
        let _ = writeln!(
            svg,
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{width:.1}\" height=\"{height:.1}\" rx=\"3\" fill=\"#ffffff\" fill-opacity=\"0.9\" stroke=\"#a9c7ec\" stroke-width=\"0.8\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{font_size:.1}\" font-weight=\"700\" fill=\"#1659b0\">{}</text>",
            rect.x,
            rect.y,
            rect.x + width / 2.0,
            rect.y + height / 2.0 + font_size * 0.35,
            escaped(&tag)
        );
        occupied.push(rect);
    }
}

pub(super) fn write_wall_labels(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f64, f64),
    pixels_per_meter: f64,
    wall_width: f64,
    occupied: &mut Vec<LabelBox>,
) {
    let room_boxes: Vec<_> = project
        .rooms
        .iter()
        .filter_map(|room| {
            let (min, max) = project.room_bounds(room)?;
            let (x, y) = point(min);
            let (right, bottom) = point(max);
            Some(LabelBox {
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        })
        .collect();
    for wall in &project.walls {
        if project
            .rooms
            .iter()
            .any(|room| room.wall_ids.contains(&wall.id))
            || wall.length() * pixels_per_meter < 24.0
        {
            continue;
        }
        let label = format_meters(wall.length());
        let font_size = 13.0;
        let width = estimated_text_width(&label, font_size) + 16.0;
        let height = font_size * 1.2 + 8.0;
        let a = point(wall.start);
        let b = point(wall.end);
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let length = (dx * dx + dy * dy).sqrt();
        let normal = (-dy / length, dx / length);
        let clearance =
            wall_width / 2.0 + normal.0.abs() * width / 2.0 + normal.1.abs() * height / 2.0 + 5.0;
        let mut placed = false;
        for fraction in [0.5, 0.35, 0.65] {
            for side in [1.0, -1.0] {
                let center = (
                    a.0 + dx * fraction + normal.0 * clearance * side,
                    a.1 + dy * fraction + normal.1 * clearance * side,
                );
                let rect = LabelBox::centered(center.0, center.1, width, height);
                if room_boxes.iter().any(|room| room.intersects(rect, 2.0))
                    || occupied.iter().any(|other| other.intersects(rect, 4.0))
                {
                    continue;
                }
                let _ = writeln!(
                    svg,
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{width:.1}\" height=\"{height:.1}\" rx=\"4\" fill=\"#ffffff\" stroke=\"#dce6f1\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{font_size:.1}\" fill=\"#3b4e63\">{label}</text>",
                    rect.x,
                    rect.y,
                    center.0,
                    center.1 + font_size * 0.35
                );
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
