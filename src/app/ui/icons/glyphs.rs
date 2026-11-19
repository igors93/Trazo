//! Icons of the drafting and editing tools added after the original set,
//! drawn on the same 24 × 24 grid with the same pen.

use std::f32::consts::PI;

use eframe::egui::{Color32, Stroke};

use super::Pen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Line,
    Polyline,
    Rectangle,
    Arc,
    Circle,
    Spline,
}

/// Paints `glyph` with `pen`; `soft` fills, `thin` draws details.
pub(super) fn paint(pen: &Pen, glyph: Glyph, soft: Color32, thin: Stroke) {
    let ink = pen.ink;
    let accent = pen.accent;
    let vertex = |x: f32, y: f32| pen.dot(x, y, 1.9, accent.color);
    match glyph {
        Glyph::Line => {
            pen.polyline(&[(4.0, 19.0), (20.0, 5.0)], ink);
            vertex(4.0, 19.0);
            vertex(20.0, 5.0);
        }
        Glyph::Polyline => {
            pen.polyline(&[(3.0, 18.0), (9.0, 7.0), (15.0, 15.0), (21.0, 5.0)], ink);
            for (x, y) in [(3.0, 18.0), (9.0, 7.0), (15.0, 15.0), (21.0, 5.0)] {
                vertex(x, y);
            }
        }
        Glyph::Rectangle => {
            pen.fill(&[(4.0, 6.0), (20.0, 6.0), (20.0, 18.0), (4.0, 18.0)], soft);
            pen.rect(4.0, 6.0, 20.0, 18.0, ink);
            vertex(4.0, 6.0);
            vertex(20.0, 18.0);
        }
        Glyph::Arc => {
            pen.arc(12.0, 17.0, 8.0, PI, 2.0 * PI, ink);
            pen.dashed(&[(4.0, 17.0), (20.0, 17.0)], 2.0, 1.6, thin);
            vertex(4.0, 17.0);
            vertex(12.0, 9.0);
            vertex(20.0, 17.0);
        }
        Glyph::Circle => {
            pen.circle(12.0, 12.0, 8.0, ink);
            pen.dashed(&[(12.0, 12.0), (20.0, 12.0)], 2.0, 1.6, thin);
            vertex(12.0, 12.0);
        }
        Glyph::Spline => {
            // An S: the top half of one circle, then the bottom of the next.
            pen.arc(7.5, 12.0, 4.5, PI, 2.0 * PI, ink);
            pen.arc(16.5, 12.0, 4.5, PI, 0.0, ink);
            vertex(3.0, 12.0);
            vertex(12.0, 12.0);
            vertex(21.0, 12.0);
        }
    }
}
