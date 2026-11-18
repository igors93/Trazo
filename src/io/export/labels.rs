use std::fmt::Write as _;

use crate::domain::{
    geometry::{Point, format_meters},
    model::Project,
};

use super::escaped;

#[derive(Clone, Copy)]
pub(super) struct LabelBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl LabelBox {
    fn centered(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x - width / 2.0,
            y: y - height / 2.0,
            width,
            height,
        }
    }

    fn intersects(self, other: Self, gap: f32) -> bool {
        self.x < other.x + other.width + gap
            && self.x + self.width + gap > other.x
            && self.y < other.y + other.height + gap
            && self.y + self.height + gap > other.y
    }
}

fn estimated_text_width(value: &str, font_size: f32) -> f32 {
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
        .sum::<f32>()
        + 2.0
}

pub(super) fn fit_export_text(
    value: &str,
    max_width: f32,
    preferred: u32,
    minimum: u32,
    truncate: bool,
) -> Option<(String, f32)> {
    for size in (minimum..=preferred).rev() {
        if estimated_text_width(value, size as f32) <= max_width {
            return Some((value.to_owned(), size as f32));
        }
    }
    if !truncate {
        return None;
    }
    let chars: Vec<char> = value.chars().collect();
    for count in (0..chars.len()).rev() {
        let candidate = format!("{}…", chars[..count].iter().collect::<String>().trim_end());
        if estimated_text_width(&candidate, minimum as f32) <= max_width {
            return Some((candidate, minimum as f32));
        }
    }
    None
}

pub(super) fn write_room_labels(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f32, f32),
    wall_width: f32,
) -> Vec<LabelBox> {
    let mut occupied = Vec::new();
    for room in &project.rooms {
        let Some((min, max)) = project.room_bounds(room) else {
            continue;
        };
        let (x, y) = point(min);
        let (right, bottom) = point(max);
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
        let dimensions = format!(
            "{} × {}",
            format_meters(max.x - min.x),
            format_meters(max.y - min.y)
        );
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
            "<text x=\"{center_x:.1}\" y=\"{name_y:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{name_size:.1}\" font-weight=\"700\" fill=\"#1e3a4a\">{}</text>",
            escaped(&name)
        );
        if let Some((measure, measure_size)) = measure {
            let measure_y = block.y + name_size * 1.2 + 4.0 + measure_size;
            let _ = writeln!(
                svg,
                "<text x=\"{center_x:.1}\" y=\"{measure_y:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{measure_size:.1}\" fill=\"#64748b\">{measure}</text>"
            );
        }
        svg.push_str("</g>\n");
        occupied.push(block);
    }
    occupied
}

pub(super) fn write_wall_labels(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f32, f32),
    pixels_per_meter: f32,
    wall_width: f32,
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
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{width:.1}\" height=\"{height:.1}\" rx=\"4\" fill=\"#ffffff\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{font_size:.1}\" fill=\"#52657a\">{label}</text>",
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
