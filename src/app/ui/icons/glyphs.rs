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
    Move,
    Copy,
    Cut,
    Paste,
    Duplicate,
    Rotate,
    Mirror,
    /// Mirror upside down (a level axis).
    MirrorVertical,
    Offset,
    Trim,
    Extend,
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
        Glyph::Move => {
            // Four arrows out of the centre.
            pen.polyline(&[(12.0, 3.0), (12.0, 21.0)], ink);
            pen.polyline(&[(3.0, 12.0), (21.0, 12.0)], ink);
            for (head, a, b) in [
                ((12.0, 3.0), (9.5, 5.5), (14.5, 5.5)),
                ((12.0, 21.0), (9.5, 18.5), (14.5, 18.5)),
                ((3.0, 12.0), (5.5, 9.5), (5.5, 14.5)),
                ((21.0, 12.0), (18.5, 9.5), (18.5, 14.5)),
            ] {
                pen.polyline(&[a, head, b], accent);
            }
        }
        Glyph::Copy => {
            pen.fill(&[(9.0, 9.0), (20.0, 9.0), (20.0, 20.0), (9.0, 20.0)], soft);
            pen.rect(9.0, 9.0, 20.0, 20.0, accent);
            pen.polyline(
                &[
                    (15.0, 5.5),
                    (15.0, 4.0),
                    (4.0, 4.0),
                    (4.0, 15.0),
                    (5.5, 15.0),
                ],
                ink,
            );
        }
        Glyph::Cut => {
            pen.circle(7.0, 17.5, 3.0, ink);
            pen.circle(17.0, 17.5, 3.0, ink);
            pen.polyline(&[(9.2, 15.4), (17.5, 3.5)], accent);
            pen.polyline(&[(14.8, 15.4), (6.5, 3.5)], accent);
        }
        Glyph::Paste => {
            pen.fill(&[(5.0, 6.0), (19.0, 6.0), (19.0, 21.0), (5.0, 21.0)], soft);
            pen.rect(5.0, 6.0, 19.0, 21.0, ink);
            pen.rect(9.0, 3.5, 15.0, 8.0, accent);
            pen.polyline(&[(8.5, 13.0), (15.5, 13.0)], thin);
            pen.polyline(&[(8.5, 16.5), (13.5, 16.5)], thin);
        }
        Glyph::Duplicate => {
            pen.rect(4.0, 4.0, 14.0, 14.0, ink);
            pen.fill(
                &[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)],
                soft,
            );
            pen.rect(10.0, 10.0, 20.0, 20.0, accent);
            pen.polyline(&[(15.0, 12.5), (15.0, 17.5)], thin);
            pen.polyline(&[(12.5, 15.0), (17.5, 15.0)], thin);
        }
        Glyph::Rotate => {
            pen.dashed(&[(5.0, 19.0), (19.0, 19.0)], 2.0, 1.6, thin);
            pen.polyline(&[(5.0, 19.0), (14.5, 7.5)], ink);
            vertex(5.0, 19.0);
            pen.arc(5.0, 19.0, 9.0, -0.05, -0.85, accent);
            pen.polyline(&[(11.0, 12.0), (11.6, 14.6), (14.2, 14.2)], accent);
        }
        Glyph::Mirror | Glyph::MirrorVertical => {
            let flip = |(x, y): (f32, f32)| {
                if glyph == Glyph::MirrorVertical {
                    (y, x)
                } else {
                    (x, y)
                }
            };
            let left = [(3.5, 18.0), (9.5, 6.0), (9.5, 18.0)].map(flip);
            let right = [(20.5, 18.0), (14.5, 6.0), (14.5, 18.0)].map(flip);
            pen.fill(&right, soft);
            pen.closed(&left, ink);
            pen.closed(&right, accent);
            pen.dashed(&[flip((12.0, 2.5)), flip((12.0, 21.5))], 2.0, 1.6, thin);
        }
        Glyph::Offset => {
            pen.polyline(&[(4.0, 20.0), (4.0, 9.0), (15.0, 9.0)], ink);
            pen.polyline(&[(9.0, 20.0), (9.0, 14.0), (15.0, 14.0)], accent);
            pen.polyline(&[(18.0, 9.0), (18.0, 14.0)], thin);
            pen.polyline(&[(16.5, 10.5), (18.0, 9.0), (19.5, 10.5)], thin);
            pen.polyline(&[(16.5, 12.5), (18.0, 14.0), (19.5, 12.5)], thin);
        }
        Glyph::Trim => {
            pen.polyline(&[(12.0, 3.0), (12.0, 21.0)], ink);
            pen.polyline(&[(3.0, 12.0), (12.0, 12.0)], accent);
            pen.dashed(&[(12.0, 12.0), (21.0, 12.0)], 1.8, 1.8, thin);
            vertex(12.0, 12.0);
            pen.polyline(&[(16.0, 9.0), (19.0, 15.0)], thin);
            pen.polyline(&[(19.0, 9.0), (16.0, 15.0)], thin);
        }
        Glyph::Extend => {
            pen.polyline(&[(20.0, 3.0), (20.0, 21.0)], ink);
            pen.polyline(&[(3.0, 12.0), (10.0, 12.0)], accent);
            pen.dashed(&[(10.0, 12.0), (20.0, 12.0)], 1.8, 1.8, thin);
            pen.polyline(&[(14.5, 9.0), (17.5, 12.0), (14.5, 15.0)], accent);
            vertex(20.0, 12.0);
        }
    }
}
