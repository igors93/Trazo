//! The Trazo mark, used as the window and taskbar icon: a blue tile in the
//! interface's accent colour, with a white plan corner and a door swing. It is
//! drawn from geometry here, so no image file has to ship with the program.

use eframe::egui::{Color32, IconData};

use super::theme;

/// Subsamples per pixel side, for smooth edges.
const SAMPLES: u32 = 4;

fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (x * x + y * y).sqrt()
}

/// Whether `p` lies in a rounded square of side 24 with corner `radius`.
fn in_tile(p: (f32, f32), radius: f32) -> bool {
    let clamp = |v: f32| v.clamp(radius, 24.0 - radius);
    let (cx, cy) = (clamp(p.0), clamp(p.1));
    let (dx, dy) = (p.0 - cx, p.1 - cy);
    (0.0..=24.0).contains(&p.0)
        && (0.0..=24.0).contains(&p.1)
        && dx * dx + dy * dy <= radius * radius
}

/// How white the mark is at `p` (0 none, 1 full), in the logo's 24-unit frame.
fn mark(p: (f32, f32)) -> f32 {
    // The plan corner: a top wall and a wall going down from its middle.
    let walls = distance_to_segment(p, (5.5, 6.5), (18.5, 6.5)).min(distance_to_segment(
        p,
        (12.0, 6.5),
        (12.0, 18.5),
    ));
    if walls <= 1.3 {
        return 1.0;
    }
    // The door swing: a quarter circle around (12, 18.5), radius 6.
    let (dx, dy) = (p.0 - 12.0, p.1 - 18.5);
    let on_arc = dx >= 0.0 && dy <= 0.0 && ((dx * dx + dy * dy).sqrt() - 6.0).abs() <= 0.6;
    if on_arc { 0.85 } else { 0.0 }
}

/// The Trazo mark as a square RGBA icon of `size` pixels.
pub fn window_icon(size: u32) -> IconData {
    let accent: Color32 = theme::ACCENT;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let unit = 24.0 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let (mut tile, mut white) = (0.0f32, 0.0f32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let p = (
                        (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * unit,
                        (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * unit,
                    );
                    if in_tile(p, 6.0) {
                        tile += 1.0;
                        white += mark(p);
                    }
                }
            }
            let samples = (SAMPLES * SAMPLES) as f32;
            let alpha = tile / samples;
            // Blend white over the blue inside the tile.
            let w = if tile > 0.0 { white / tile } else { 0.0 };
            let blend = |c: u8| (c as f32 * (1.0 - w) + 255.0 * w).round() as u8;
            rgba.extend_from_slice(&[
                blend(accent.r()),
                blend(accent.g()),
                blend(accent.b()),
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    IconData {
        rgba,
        width: size,
        height: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_the_blue_tile_with_the_white_mark() {
        let icon = window_icon(64);
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        let pixel = |x: usize, y: usize| {
            let i = (y * 64 + x) * 4;
            &icon.rgba[i..i + 4]
        };
        // Rounded corners are transparent.
        assert_eq!(pixel(0, 0)[3], 0);
        // The tile is the interface's accent blue.
        let accent = theme::ACCENT;
        assert_eq!(pixel(5, 40), &[accent.r(), accent.g(), accent.b(), 255]);
        // The top wall is white.
        assert_eq!(pixel(32, 17), &[255, 255, 255, 255]);
    }
}
