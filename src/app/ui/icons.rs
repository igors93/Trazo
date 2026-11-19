//! Line icons drawn with the painter on a 24 × 24 grid, so they stay crisp at
//! any size and need no font glyphs or image files. Each icon has two tones:
//! the ink (outline) and an accent for the part that tells it apart.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

use crate::domain::fixtures::{self, FixtureKind, SymbolShape, Weight};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Select,
    Terrain,
    Wall,
    Room,
    Door,
    Window,
    Delete,
    Undo,
    Redo,
    New,
    Open,
    Save,
    Export,
    ExportImage,
    ExportPdf,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Snap,
    Ortho,
    Grid,
    Center,
    Keyboard,
    PanelLeft,
    PanelRight,
    Search,
    ChevronDown,
    ChevronUp,
    ChevronRight,
    Plan,
    Close,
    Info,
    Check,
    Warning,
    Ai,
    SaloonDoor,
    /// The plan symbol of a fixture, as the ribbon shows it.
    Fixture(FixtureKind),
}

/// Draws a fixture's plan symbol fitted in `rect`.
fn paint_fixture_icon(painter: &Painter, rect: Rect, kind: FixtureKind, ink: Color32, accent: Color32) {
    let (width, depth) = kind.default_size();
    // Tables draw chairs beyond their top.
    let reach_y = if kind == FixtureKind::Table { depth + 0.9 } else { depth };
    let scale = (rect.width() * 0.86 / width).min(rect.height() * 0.86 / reach_y);
    let at = |x: f32, y: f32| rect.center() + vec2(x * scale, y * scale);
    for shape in fixtures::symbol(kind, width, depth) {
        match shape {
            SymbolShape::Polyline {
                points,
                closed,
                dashed,
                weight,
            } => {
                let stroke = match weight {
                    Weight::Outline => Stroke::new(1.4, ink),
                    Weight::Detail => Stroke::new(1.0, accent),
                };
                let mut points: Vec<Pos2> = points.iter().map(|p| at(p.x, p.y)).collect();
                if dashed {
                    if closed {
                        points.push(points[0]);
                    }
                    painter.extend(Shape::dashed_line(&points, stroke, 2.5, 2.0));
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
                    Weight::Detail => Stroke::new(1.0, accent),
                };
                painter.circle_stroke(at(center.x, center.y), (radius * scale).max(1.0), stroke);
            }
        }
    }
}

struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    ink: Stroke,
    accent: Stroke,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x * self.scale, y * self.scale)
    }

    fn polyline(&self, points: &[(f32, f32)], stroke: Stroke) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
        for end in [points[0], points[points.len() - 1]] {
            self.painter
                .circle_filled(end, stroke.width * 0.5, stroke.color);
        }
        self.painter.add(Shape::line(points, stroke));
    }

    fn line(&self, points: &[(f32, f32)]) {
        self.polyline(points, self.ink);
    }

    fn accent_line(&self, points: &[(f32, f32)]) {
        self.polyline(points, self.accent);
    }

    fn closed(&self, points: &[(f32, f32)], stroke: Stroke) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.add(Shape::closed_line(points, stroke));
    }

    fn fill(&self, points: &[(f32, f32)], color: Color32) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, color, Stroke::NONE));
    }

    fn circle(&self, x: f32, y: f32, r: f32, stroke: Stroke) {
        self.painter
            .circle_stroke(self.p(x, y), r * self.scale, stroke);
    }

    fn dot(&self, x: f32, y: f32, r: f32, color: Color32) {
        self.painter.circle_filled(self.p(x, y), r * self.scale, color);
    }

    /// An arc around (x, y); angles in radians, clockwise on screen from +x.
    fn arc(&self, x: f32, y: f32, r: f32, from: f32, to: f32, stroke: Stroke) {
        let steps = ((to - from).abs() / 0.18).ceil().max(4.0) as usize;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let a = from + (to - from) * i as f32 / steps as f32;
                (x + r * a.cos(), y + r * a.sin())
            })
            .collect();
        self.polyline(&points, stroke);
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, stroke: Stroke) {
        self.closed(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], stroke);
    }

    fn dashed(&self, points: &[(f32, f32)], dash: f32, gap: f32, stroke: Stroke) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.extend(Shape::dashed_line(
            &points,
            stroke,
            dash * self.scale,
            gap * self.scale,
        ));
    }
}

/// Paints `icon` centred in `rect` (square or not), with `ink` for its
/// outline and `accent` for its distinguishing detail.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, ink: Color32, accent: Color32) {
    if let Icon::Fixture(kind) = icon {
        paint_fixture_icon(painter, rect, kind, ink, accent);
        return;
    }
    let side = rect.width().min(rect.height());
    let scale = side / 24.0;
    let width = (1.55 * scale).max(1.0);
    let pen = Pen {
        painter,
        origin: rect.center() - vec2(12.0 * scale, 12.0 * scale),
        scale,
        ink: Stroke::new(width, ink),
        accent: Stroke::new(width, accent),
    };
    let soft = accent.gamma_multiply(0.18);
    let ink_s = pen.ink;
    let acc_s = pen.accent;
    let thin = Stroke::new((width * 0.75).max(0.9), accent);

    match icon {
        Icon::Select => {
            let arrow = [
                (6.0, 3.0),
                (6.0, 19.0),
                (10.2, 15.2),
                (13.0, 21.0),
                (15.6, 19.8),
                (12.9, 14.2),
                (18.5, 14.2),
            ];
            pen.fill(&[(6.0, 3.0), (6.0, 19.0), (10.2, 15.2), (18.5, 14.2)], soft);
            pen.closed(&arrow, ink_s);
        }
        Icon::Terrain => {
            pen.fill(&[(3.5, 5.0), (20.5, 5.0), (20.5, 19.0), (3.5, 19.0)], soft);
            pen.dashed(
                &[(3.5, 5.0), (20.5, 5.0), (20.5, 19.0), (3.5, 19.0), (3.5, 5.0)],
                3.2,
                1.8,
                acc_s,
            );
            for (x, y) in [(3.5, 5.0), (20.5, 5.0), (20.5, 19.0), (3.5, 19.0)] {
                pen.dot(x, y, 1.9, ink);
            }
        }
        Icon::Wall => {
            pen.fill(&[(2.5, 8.5), (21.5, 8.5), (21.5, 15.5), (2.5, 15.5)], soft);
            for x in [4.0, 8.5, 13.0, 17.5] {
                pen.polyline(&[(x, 15.5), (x + 4.0, 8.5)], thin);
            }
            pen.rect(2.5, 8.5, 21.5, 15.5, ink_s);
        }
        Icon::Room => {
            pen.fill(&[(4.0, 4.0), (20.0, 4.0), (20.0, 20.0), (4.0, 20.0)], soft);
            pen.line(&[(9.0, 20.0), (4.0, 20.0), (4.0, 4.0), (20.0, 4.0), (20.0, 20.0), (15.0, 20.0)]);
            pen.accent_line(&[(9.0, 20.0), (9.0, 14.0)]);
            pen.arc(9.0, 20.0, 6.0, -FRAC_PI_2, 0.0, thin);
        }
        Icon::Door => {
            pen.line(&[(2.0, 19.0), (6.0, 19.0)]);
            pen.line(&[(18.0, 19.0), (22.0, 19.0)]);
            pen.fill(&[(6.0, 19.0), (6.0, 7.0), (18.0, 19.0)], soft);
            pen.line(&[(6.0, 19.0), (6.0, 7.0)]);
            pen.arc(6.0, 19.0, 12.0, -FRAC_PI_2, 0.0, acc_s);
        }
        Icon::Window => {
            pen.line(&[(2.0, 12.0), (5.0, 12.0)]);
            pen.line(&[(19.0, 12.0), (22.0, 12.0)]);
            pen.fill(&[(5.0, 8.5), (19.0, 8.5), (19.0, 15.5), (5.0, 15.5)], soft);
            pen.rect(5.0, 8.5, 19.0, 15.5, ink_s);
            pen.accent_line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Delete => {
            pen.line(&[(4.0, 6.5), (20.0, 6.5)]);
            pen.line(&[(9.0, 6.5), (9.5, 3.8), (14.5, 3.8), (15.0, 6.5)]);
            pen.line(&[(6.0, 6.5), (7.0, 20.5), (17.0, 20.5), (18.0, 6.5)]);
            pen.accent_line(&[(10.2, 10.5), (10.2, 16.5)]);
            pen.accent_line(&[(13.8, 10.5), (13.8, 16.5)]);
        }
        Icon::Undo | Icon::Redo => {
            let m = |x: f32| if icon == Icon::Redo { 24.0 - x } else { x };
            pen.accent_line(&[(m(9.0), 14.0), (m(4.0), 9.0), (m(9.0), 4.0)]);
            let mut path = vec![(m(4.0), 9.0), (m(14.5), 9.0)];
            for i in 0..=12 {
                let a = -FRAC_PI_2 + PI * i as f32 / 12.0;
                path.push((m(14.5 + 5.5 * a.cos()), 14.5 + 5.5 * a.sin()));
            }
            path.push((m(10.0), 20.0));
            pen.line(&path);
        }
        Icon::New => {
            pen.line(&[(14.0, 3.0), (6.0, 3.0), (6.0, 21.0), (18.0, 21.0), (18.0, 7.0), (14.0, 3.0), (14.0, 7.0), (18.0, 7.0)]);
            pen.accent_line(&[(12.0, 10.5), (12.0, 17.5)]);
            pen.accent_line(&[(8.5, 14.0), (15.5, 14.0)]);
        }
        Icon::Open => {
            pen.line(&[(3.0, 19.0), (3.0, 5.0), (9.0, 5.0), (11.0, 7.5), (19.0, 7.5), (19.0, 10.5)]);
            pen.fill(&[(3.0, 19.0), (6.0, 11.0), (22.0, 11.0), (19.0, 19.0)], soft);
            pen.closed(&[(3.0, 19.0), (6.0, 11.0), (22.0, 11.0), (19.0, 19.0)], acc_s);
        }
        Icon::Save => {
            pen.closed(&[(3.5, 3.5), (16.5, 3.5), (20.5, 7.5), (20.5, 20.5), (3.5, 20.5)], ink_s);
            pen.fill(&[(7.5, 13.5), (16.5, 13.5), (16.5, 20.5), (7.5, 20.5)], soft);
            pen.accent_line(&[(7.5, 20.5), (7.5, 13.5), (16.5, 13.5), (16.5, 20.5)]);
            pen.line(&[(7.5, 3.5), (7.5, 8.0), (14.5, 8.0), (14.5, 3.5)]);
        }
        Icon::Export => {
            pen.line(&[(4.0, 13.0), (4.0, 20.0), (20.0, 20.0), (20.0, 13.0)]);
            pen.accent_line(&[(12.0, 15.0), (12.0, 3.5)]);
            pen.accent_line(&[(7.5, 8.0), (12.0, 3.5), (16.5, 8.0)]);
        }
        Icon::ExportImage => {
            pen.rect(3.0, 4.0, 21.0, 20.0, ink_s);
            pen.fill(&[(3.5, 18.5), (9.0, 12.0), (13.0, 16.0)], soft);
            pen.accent_line(&[(3.5, 18.5), (9.0, 12.0), (13.0, 16.0), (16.0, 13.0), (20.5, 18.0)]);
            pen.dot(8.5, 8.8, 1.9, accent);
        }
        Icon::ExportPdf => {
            pen.line(&[(14.0, 3.0), (6.0, 3.0), (6.0, 21.0), (18.0, 21.0), (18.0, 7.0), (14.0, 3.0), (14.0, 7.0), (18.0, 7.0)]);
            pen.accent_line(&[(9.0, 11.5), (15.0, 11.5)]);
            pen.accent_line(&[(9.0, 14.5), (15.0, 14.5)]);
            pen.accent_line(&[(9.0, 17.5), (13.0, 17.5)]);
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            pen.circle(10.5, 10.5, 6.8, ink_s);
            pen.line(&[(15.6, 15.6), (20.5, 20.5)]);
            pen.accent_line(&[(7.5, 10.5), (13.5, 10.5)]);
            if icon == Icon::ZoomIn {
                pen.accent_line(&[(10.5, 7.5), (10.5, 13.5)]);
            }
        }
        Icon::ZoomFit => {
            pen.line(&[(3.5, 8.5), (3.5, 3.5), (8.5, 3.5)]);
            pen.line(&[(15.5, 3.5), (20.5, 3.5), (20.5, 8.5)]);
            pen.line(&[(20.5, 15.5), (20.5, 20.5), (15.5, 20.5)]);
            pen.line(&[(8.5, 20.5), (3.5, 20.5), (3.5, 15.5)]);
            pen.fill(&[(8.5, 8.5), (15.5, 8.5), (15.5, 15.5), (8.5, 15.5)], soft);
            pen.rect(8.5, 8.5, 15.5, 15.5, acc_s);
        }
        Icon::Snap => {
            let mut outer = vec![(5.5, 3.5)];
            let mut inner = vec![(10.0, 3.5)];
            for i in 0..=12 {
                let a = PI - PI * i as f32 / 12.0;
                outer.push((12.0 + 6.5 * a.cos(), 13.0 + 6.5 * a.sin()));
                inner.push((12.0 + 2.0 * a.cos(), 13.0 + 2.0 * a.sin()));
            }
            outer.push((18.5, 3.5));
            inner.push((14.0, 3.5));
            pen.line(&outer);
            pen.line(&inner);
            pen.line(&[(5.5, 3.5), (10.0, 3.5)]);
            pen.line(&[(14.0, 3.5), (18.5, 3.5)]);
            pen.fill(&[(5.5, 3.5), (10.0, 3.5), (10.0, 7.5), (5.5, 7.5)], accent);
            pen.fill(&[(14.0, 3.5), (18.5, 3.5), (18.5, 7.5), (14.0, 7.5)], accent);
        }
        Icon::Ortho => {
            pen.line(&[(5.0, 3.5), (5.0, 19.0), (20.5, 19.0)]);
            pen.accent_line(&[(2.5, 6.5), (5.0, 3.5), (7.5, 6.5)]);
            pen.accent_line(&[(17.5, 16.5), (20.5, 19.0), (17.5, 21.5)]);
            pen.polyline(&[(5.0, 15.0), (9.0, 15.0), (9.0, 19.0)], thin);
        }
        Icon::Grid => {
            pen.rect(3.5, 3.5, 20.5, 20.5, ink_s);
            for t in [9.17, 14.83] {
                pen.polyline(&[(t, 3.5), (t, 20.5)], thin);
                pen.polyline(&[(3.5, t), (20.5, t)], thin);
            }
        }
        Icon::Center => {
            pen.dashed(
                &[(3.5, 3.5), (20.5, 3.5), (20.5, 20.5), (3.5, 20.5), (3.5, 3.5)],
                2.6,
                1.8,
                ink_s,
            );
            pen.circle(12.0, 12.0, 4.0, acc_s);
            pen.accent_line(&[(12.0, 6.0), (12.0, 8.0)]);
            pen.accent_line(&[(12.0, 16.0), (12.0, 18.0)]);
            pen.accent_line(&[(6.0, 12.0), (8.0, 12.0)]);
            pen.accent_line(&[(16.0, 12.0), (18.0, 12.0)]);
            pen.dot(12.0, 12.0, 1.2, accent);
        }
        Icon::Keyboard => {
            pen.rect(2.5, 6.0, 21.5, 18.0, ink_s);
            for x in [6.0, 9.33, 12.67, 16.0] {
                pen.dot(x + 0.3, 10.0, 0.95, accent);
            }
            pen.dot(18.8, 10.0, 0.95, accent);
            pen.accent_line(&[(8.0, 14.0), (16.0, 14.0)]);
        }
        Icon::PanelLeft | Icon::PanelRight => {
            pen.rect(3.0, 4.5, 21.0, 19.5, ink_s);
            let (x0, x1) = if icon == Icon::PanelLeft {
                (3.0, 9.5)
            } else {
                (14.5, 21.0)
            };
            pen.fill(&[(x0, 4.5), (x1, 4.5), (x1, 19.5), (x0, 19.5)], soft);
            let x = if icon == Icon::PanelLeft { x1 } else { x0 };
            pen.accent_line(&[(x, 4.5), (x, 19.5)]);
        }
        Icon::Search => {
            pen.circle(10.8, 10.8, 6.5, ink_s);
            pen.accent_line(&[(15.6, 15.6), (20.5, 20.5)]);
        }
        Icon::ChevronDown => pen.line(&[(7.0, 9.5), (12.0, 14.5), (17.0, 9.5)]),
        Icon::ChevronUp => pen.line(&[(7.0, 14.5), (12.0, 9.5), (17.0, 14.5)]),
        Icon::ChevronRight => pen.line(&[(9.5, 7.0), (14.5, 12.0), (9.5, 17.0)]),
        Icon::Plan => {
            pen.fill(&[(3.5, 3.5), (20.5, 3.5), (20.5, 20.5), (3.5, 20.5)], soft);
            pen.rect(3.5, 3.5, 20.5, 20.5, ink_s);
            pen.accent_line(&[(3.5, 12.0), (9.0, 12.0)]);
            pen.accent_line(&[(12.5, 12.0), (12.5, 20.5)]);
            pen.accent_line(&[(12.5, 3.5), (12.5, 7.5)]);
            pen.accent_line(&[(16.0, 12.0), (20.5, 12.0)]);
        }
        Icon::Close => {
            pen.line(&[(6.5, 6.5), (17.5, 17.5)]);
            pen.line(&[(17.5, 6.5), (6.5, 17.5)]);
        }
        Icon::Info => {
            pen.circle(12.0, 12.0, 9.0, ink_s);
            pen.accent_line(&[(12.0, 11.0), (12.0, 16.5)]);
            pen.dot(12.0, 7.8, 1.2, accent);
        }
        Icon::Check => pen.accent_line(&[(5.0, 12.5), (9.8, 17.0), (19.0, 7.0)]),
        Icon::SaloonDoor => {
            pen.line(&[(2.0, 13.0), (5.0, 13.0)]);
            pen.line(&[(19.0, 13.0), (22.0, 13.0)]);
            pen.line(&[(5.0, 13.0), (5.0, 6.0)]);
            pen.line(&[(19.0, 13.0), (19.0, 6.0)]);
            pen.arc(5.0, 13.0, 7.0, -FRAC_PI_2, 0.0, acc_s);
            pen.arc(19.0, 13.0, 7.0, -PI, -FRAC_PI_2, acc_s);
            pen.arc(5.0, 13.0, 7.0, 0.0, FRAC_PI_2, thin);
            pen.arc(19.0, 13.0, 7.0, FRAC_PI_2, PI, thin);
        }
        Icon::Fixture(_) => {}
        Icon::Ai => {
            // A four-pointed spark with a small one beside it.
            let vertical = [(10.5, 3.5), (12.6, 12.0), (10.5, 20.5), (8.4, 12.0)];
            let horizontal = [(2.5, 12.0), (10.5, 9.9), (18.5, 12.0), (10.5, 14.1)];
            pen.fill(&vertical, soft);
            pen.fill(&horizontal, soft);
            pen.closed(
                &[
                    (10.5, 3.5),
                    (12.6, 9.9),
                    (18.5, 12.0),
                    (12.6, 14.1),
                    (10.5, 20.5),
                    (8.4, 14.1),
                    (2.5, 12.0),
                    (8.4, 9.9),
                ],
                ink_s,
            );
            pen.accent_line(&[(18.5, 2.5), (18.5, 7.5)]);
            pen.accent_line(&[(16.0, 5.0), (21.0, 5.0)]);
        }
        Icon::Warning => {
            pen.fill(&[(12.0, 3.5), (21.5, 20.0), (2.5, 20.0)], soft);
            pen.closed(&[(12.0, 3.5), (21.5, 20.0), (2.5, 20.0)], ink_s);
            pen.accent_line(&[(12.0, 9.5), (12.0, 14.0)]);
            pen.dot(12.0, 17.0, 1.15, accent);
        }
    }
}

/// The Trazo mark: a blue tile with a white plan corner and a door swing.
pub fn paint_logo(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 6.0, crate::app::theme::ACCENT);
    let scale = rect.width() / 24.0;
    let origin = rect.min;
    let p = |x: f32, y: f32| origin + vec2(x * scale, y * scale);
    let white = Color32::WHITE;
    let wall = Stroke::new(2.6 * scale, white);
    painter.line_segment([p(5.5, 6.5), p(18.5, 6.5)], wall);
    painter.line_segment([p(12.0, 6.5), p(12.0, 18.5)], wall);
    let arc: Vec<Pos2> = (0..=10)
        .map(|i| {
            let a = FRAC_PI_2 * i as f32 / 10.0;
            p(12.0 + 6.0 * a.sin(), 18.5 - 6.0 * a.cos())
        })
        .collect();
    painter.add(Shape::line(
        arc,
        Stroke::new(1.2 * scale, white.gamma_multiply(0.85)),
    ));
}
