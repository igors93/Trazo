//! Dimension-aware furniture symbols shared by the canvas and vector exports.
//! Coordinates in each design use the object's own 0..1 frame; small details
//! can be omitted before tessellation for readable, inexpensive small previews.

use super::{FixtureKind, SymbolShape, Weight, circle, ellipse, line, rect};
use crate::domain::geometry::Point;
use Weight::{Detail, Fine, Outline};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

struct Pen {
    w: f64,
    d: f64,
    fine: bool,
    shapes: Vec<SymbolShape>,
}

impl Pen {
    fn new(w: f64, d: f64, fine: bool) -> Self {
        Self {
            w,
            d,
            fine,
            shapes: Vec::new(),
        }
    }

    fn point(&self, u: f64, v: f64) -> Point {
        Point::new((u - 0.5) * self.w, (v - 0.5) * self.d)
    }

    fn visible(&self, weight: Weight) -> bool {
        self.fine || weight != Fine
    }

    fn path(&mut self, points: &[(f64, f64)], closed: bool, weight: Weight) {
        if self.visible(weight) {
            self.shapes.push(SymbolShape::Polyline {
                points: points.iter().map(|&(u, v)| self.point(u, v)).collect(),
                closed,
                dashed: false,
                weight,
            });
        }
    }

    fn stroke(&mut self, points: &[(f64, f64)], weight: Weight) {
        self.path(points, false, weight);
    }

    fn box_at(&mut self, bounds: [f64; 4], rounding: f64, weight: Weight) {
        if !self.visible(weight) {
            return;
        }
        let [u0, v0, u1, v1] = bounds;
        let a = self.point(u0, v0);
        let b = self.point(u1, v1);
        let r = ((b.x - a.x).min(b.y - a.y) * rounding).max(0.0);
        if r <= 1e-10 {
            self.shapes.push(rect(a.x, a.y, b.x, b.y, weight));
            return;
        }
        let steps = if r >= 0.12 {
            8
        } else if r >= 0.035 {
            6
        } else {
            4
        };
        let mut points = Vec::with_capacity(4 * (steps + 1));
        for (cx, cy, start) in [
            (b.x - r, a.y + r, -FRAC_PI_2),
            (b.x - r, b.y - r, 0.0),
            (a.x + r, b.y - r, FRAC_PI_2),
            (a.x + r, a.y + r, PI),
        ] {
            for step in 0..=steps {
                let angle = start + FRAC_PI_2 * step as f64 / steps as f64;
                points.push(Point::new(cx + r * angle.cos(), cy + r * angle.sin()));
            }
        }
        self.shapes.push(SymbolShape::Polyline {
            points,
            closed: true,
            dashed: false,
            weight,
        });
    }

    fn oval(&mut self, u: f64, v: f64, ru: f64, rv: f64, weight: Weight) {
        if self.visible(weight) {
            let p = self.point(u, v);
            self.shapes
                .push(ellipse(p.x, p.y, ru * self.w, rv * self.d, weight));
        }
    }

    fn dot_ring(&mut self, u: f64, v: f64, r: f64, weight: Weight) {
        if self.visible(weight) {
            let p = self.point(u, v);
            self.shapes
                .push(circle(p.x, p.y, r * self.w.min(self.d), weight));
        }
    }

    fn drain(&mut self, u: f64, v: f64, r: f64) {
        self.dot_ring(u, v, r, Detail);
        let ru = r * self.w.min(self.d) / self.w * 0.45;
        let rv = r * self.w.min(self.d) / self.d * 0.45;
        self.stroke(&[(u - ru, v), (u + ru, v)], Fine);
        self.stroke(&[(u, v - rv), (u, v + rv)], Fine);
    }

    fn faucet(&mut self, u: f64, v: f64) {
        self.dot_ring(u, v, 0.026, Detail);
        self.stroke(
            &[
                (u - 0.012, v),
                (u - 0.012, v + 0.10),
                (u + 0.012, v + 0.10),
                (u + 0.012, v),
            ],
            Detail,
        );
        self.stroke(&[(u - 0.05, v), (u + 0.05, v)], Fine);
    }

    fn chair(&mut self, x: f64, y: f64, width: f64, depth: f64, turns: u8) {
        let mut chair = Self::new(width, depth, self.fine);
        chair.box_at([0.0, 0.0, 1.0, 1.0], 0.16, Outline);
        chair.stroke(&[(0.06, 0.19), (0.94, 0.19)], Detail);
        chair.box_at([0.12, 0.28, 0.88, 0.90], 0.16, Fine);
        for shape in &mut chair.shapes {
            let transform = |p: &mut Point| {
                for _ in 0..turns {
                    (p.x, p.y) = (-p.y, p.x);
                }
                p.x += x;
                p.y += y;
            };
            match shape {
                SymbolShape::Polyline { points, .. } => points.iter_mut().for_each(transform),
                SymbolShape::Circle { center, .. } => transform(center),
            }
        }
        self.shapes.extend(chair.shapes);
    }

    fn table(&mut self, dining: bool) {
        let kind = if dining {
            FixtureKind::DiningSet
        } else {
            FixtureKind::Table
        };
        let count = kind.seat_count(self.w).unwrap_or(2);
        let per_side = if dining { (count - 2) / 2 } else { count / 2 };
        let chair_w = (self.w / per_side as f64 * 0.74).min(0.44);
        let chair_d = (self.d * 0.44).clamp(0.12, 0.40);
        let gap = self.d.min(self.w) * 0.06;
        for i in 0..per_side {
            let x = -self.w * 0.5 + self.w * (i as f64 + 0.5) / per_side as f64;
            self.chair(x, -self.d * 0.5 - gap - chair_d * 0.5, chair_w, chair_d, 0);
            self.chair(x, self.d * 0.5 + gap + chair_d * 0.5, chair_w, chair_d, 2);
        }
        if dining {
            let end_w = (self.d * 0.72).min(0.44);
            self.chair(-self.w * 0.5 - gap - chair_d * 0.5, 0.0, end_w, chair_d, 3);
            self.chair(self.w * 0.5 + gap + chair_d * 0.5, 0.0, end_w, chair_d, 1);
        }
        self.box_at([0.0, 0.0, 1.0, 1.0], 0.07, Outline);
        self.box_at([0.025, 0.035, 0.975, 0.965], 0.07, Fine);
    }

    fn bed(&mut self, double: bool) {
        self.box_at([0.0, 0.0, 1.0, 1.0], 0.035, Outline);
        self.box_at([0.0, 0.0, 1.0, 0.085], 0.10, Detail);
        self.box_at([0.025, 0.10, 0.975, 0.98], 0.06, Detail);
        let pillows = if double { 2 } else { 1 };
        for i in 0..pillows {
            let u0 = 0.08 + 0.84 * i as f64 / pillows as f64;
            let u1 = 0.08 + 0.84 * (i + 1) as f64 / pillows as f64 - 0.025;
            self.box_at([u0, 0.14, u1, 0.33], 0.20, Detail);
            self.box_at([u0 + 0.015, 0.155, u1 - 0.015, 0.315], 0.20, Fine);
        }
        self.box_at([0.04, 0.41, 0.96, 0.97], 0.035, Detail);
        self.stroke(&[(0.04, 0.48), (0.96, 0.48)], Detail);
        self.stroke(&[(0.07, 0.51), (0.93, 0.51)], Fine);
        self.stroke(&[(0.78, 0.97), (0.96, 0.83), (0.96, 0.97)], Fine);
    }

    fn upholstered(&mut self, armchair: bool) {
        self.box_at([0.0, 0.0, 1.0, 1.0], 0.10, Outline);
        let arm = if armchair {
            0.17
        } else {
            (0.16 / self.w).clamp(0.07, 0.16)
        };
        self.box_at([0.02, 0.12, arm, 0.98], 0.18, Detail);
        self.box_at([1.0 - arm, 0.12, 0.98, 0.98], 0.18, Detail);
        let seats = if armchair {
            1
        } else {
            FixtureKind::Sofa.seat_count(self.w).unwrap_or(1)
        };
        let slot = (1.0 - 2.0 * arm) / seats as f64;
        for i in 0..seats {
            let u0 = arm + i as f64 * slot + 0.009;
            let u1 = arm + (i + 1) as f64 * slot - 0.009;
            self.box_at([u0, 0.035, u1, 0.29], 0.15, Detail);
            self.box_at([u0, 0.32, u1, 0.96], 0.10, Detail);
            self.box_at([u0 + 0.014, 0.345, u1 - 0.014, 0.94], 0.12, Fine);
        }
        self.path(
            &[
                (arm + 0.025, 0.33),
                (arm + 0.12, 0.29),
                (arm + 0.16, 0.47),
                (arm + 0.06, 0.51),
            ],
            true,
            Fine,
        );
    }
}

pub(super) fn interior_symbol(
    kind: FixtureKind,
    width: f64,
    depth: f64,
    fine: bool,
) -> Vec<SymbolShape> {
    let mut p = Pen::new(width, depth, fine);
    match kind {
        FixtureKind::Counter => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.02, Outline);
            p.stroke(&[(0.0, 0.075), (1.0, 0.075)], Detail);
            p.stroke(&[(0.0, 0.93), (1.0, 0.93)], Detail);
            let modules = (width / 0.60).ceil().clamp(1.0, 12.0) as usize;
            for i in 1..modules {
                let u = i as f64 / modules as f64;
                p.stroke(&[(u, 0.10), (u, 0.90)], Fine);
            }
            for i in 0..modules {
                let u = (i as f64 + 0.5) / modules as f64;
                p.stroke(&[(u - 0.045, 0.87), (u + 0.045, 0.87)], Fine);
            }
        }
        FixtureKind::KitchenSink => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.02, Outline);
            p.box_at([0.035, 0.06, 0.965, 0.94], 0.06, Detail);
            let basins = if width >= 1.70 { 2 } else { 1 };
            for i in 0..basins {
                let u0 = 0.08 + i as f64 * 0.40;
                p.box_at([u0, 0.25, u0 + 0.36, 0.85], 0.20, Detail);
                p.box_at([u0 + 0.018, 0.28, u0 + 0.342, 0.82], 0.20, Fine);
                p.drain(u0 + 0.18, 0.64, 0.033);
            }
            p.faucet(if basins == 2 { 0.46 } else { 0.26 }, 0.13);
            if basins == 1 {
                for i in 0..6 {
                    let u = 0.57 + i as f64 * 0.062;
                    p.stroke(&[(u, 0.27), (u, 0.83)], Detail);
                }
            }
        }
        FixtureKind::Stove => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.035, Outline);
            p.box_at([0.045, 0.04, 0.955, 0.81], 0.05, Fine);
            for (u, v, r) in [
                (0.26, 0.24, 0.14),
                (0.74, 0.24, 0.115),
                (0.26, 0.64, 0.115),
                (0.74, 0.64, 0.14),
            ] {
                p.dot_ring(u, v, r, Detail);
                p.dot_ring(u, v, r * 0.47, Fine);
                p.stroke(&[(u - r * 1.1, v), (u + r * 1.1, v)], Fine);
                p.stroke(&[(u, v - r * 1.1), (u, v + r * 1.1)], Fine);
            }
            p.stroke(&[(0.0, 0.85), (1.0, 0.85)], Detail);
            for i in 0..4 {
                p.dot_ring(0.20 + i as f64 * 0.20, 0.925, 0.027, Detail);
            }
        }
        FixtureKind::WoodStove => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.02, Outline);
            p.box_at([0.05, 0.06, 0.77, 0.71], 0.03, Detail);
            for (u, v, r) in [(0.25, 0.36, 0.16), (0.59, 0.29, 0.12), (0.59, 0.57, 0.09)] {
                p.dot_ring(u, v, r, Detail);
                p.dot_ring(u, v, r * 0.55, Fine);
            }
            p.box_at([0.82, 0.04, 0.97, 0.29], 0.04, Outline);
            p.box_at([0.10, 0.79, 0.46, 0.95], 0.04, Detail);
            p.box_at([0.53, 0.79, 0.85, 0.95], 0.04, Detail);
            p.stroke(&[(0.21, 0.87), (0.35, 0.87)], Fine);
            p.stroke(&[(0.63, 0.87), (0.76, 0.87)], Fine);
        }
        FixtureKind::Grill => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.02, Outline);
            p.box_at([0.07, 0.10, 0.93, 0.78], 0.03, Detail);
            p.box_at([0.105, 0.14, 0.895, 0.74], 0.03, Fine);
            let bars = (width / 0.085).round().clamp(4.0, 24.0) as usize;
            for i in 1..bars {
                let u = 0.07 + 0.86 * i as f64 / bars as f64;
                p.stroke(&[(u, 0.10), (u, 0.78)], Detail);
            }
            p.stroke(&[(0.17, 0.86), (0.83, 0.86)], Detail);
            p.box_at([0.32, 0.89, 0.68, 0.96], 0.15, Fine);
        }
        FixtureKind::Fridge => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.045, Outline);
            p.stroke(&[(0.015, 0.90), (0.985, 0.90)], Detail);
            p.box_at([0.16, 0.94, 0.56, 0.98], 0.25, Detail);
            p.box_at([0.07, 0.065, 0.93, 0.83], 0.05, Fine);
            for v in [0.29, 0.53] {
                p.stroke(&[(0.09, v), (0.91, v)], Fine);
            }
            if width >= 0.90 {
                p.stroke(&[(0.5, 0.03), (0.5, 0.90)], Detail);
                p.box_at([0.65, 0.94, 0.87, 0.98], 0.25, Detail);
            }
        }
        FixtureKind::Washbasin => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.08, Outline);
            let count = if width >= 0.9 { 2 } else { 1 };
            for i in 0..count {
                let u = (i as f64 + 0.5) / count as f64;
                let ru = 0.39 / count as f64;
                p.oval(u, 0.58, ru, 0.32, Detail);
                p.oval(u, 0.59, ru * 0.86, 0.25, Fine);
                p.drain(u, 0.67, 0.027);
                p.faucet(u, 0.14);
            }
        }
        FixtureKind::Toilet => {
            p.oval(0.5, 0.625, 0.44, 0.375, Outline);
            p.box_at([0.0, 0.0, 1.0, 0.29], 0.13, Outline);
            p.stroke(&[(0.07, 0.235), (0.93, 0.235)], Fine);
            p.dot_ring(0.69, 0.125, 0.048, Detail);
            p.oval(0.5, 0.635, 0.32, 0.285, Detail);
            p.oval(0.5, 0.65, 0.245, 0.225, Fine);
            p.stroke(&[(0.30, 0.32), (0.70, 0.32)], Detail);
        }
        FixtureKind::Shower => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.015, Outline);
            p.stroke(
                &[(0.035, 0.04), (0.035, 0.96), (0.965, 0.96), (0.965, 0.04)],
                Detail,
            );
            p.stroke(&[(0.05, 0.90), (0.49, 0.90)], Detail);
            p.stroke(&[(0.51, 0.93), (0.95, 0.93)], Detail);
            p.box_at([0.43, 0.88, 0.47, 0.94], 0.10, Fine);
            p.box_at([0.46, 0.10, 0.66, 0.22], 0.10, Detail);
            p.stroke(&[(0.56, 0.04), (0.56, 0.10)], Detail);
            p.dot_ring(0.30, 0.08, 0.033, Detail);
            p.drain(0.77, 0.73, 0.045);
            for q in [(0.07, 0.09), (0.93, 0.09), (0.07, 0.84), (0.93, 0.84)] {
                p.stroke(&[q, (0.77, 0.73)], Fine);
            }
        }
        FixtureKind::Table => p.table(false),
        FixtureKind::DiningSet => p.table(true),
        FixtureKind::BedDouble => p.bed(true),
        FixtureKind::BedSingle => p.bed(false),
        FixtureKind::Sofa => p.upholstered(false),
        FixtureKind::Armchair => p.upholstered(true),
        FixtureKind::Wardrobe => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.015, Outline);
            p.stroke(&[(0.02, 0.88), (0.98, 0.88)], Detail);
            let bays = (width / 0.45).ceil().clamp(1.0, 12.0) as usize;
            for i in 1..bays {
                let u = i as f64 / bays as f64;
                p.stroke(&[(u, 0.04), (u, 0.98)], Detail);
            }
            for i in 0..bays {
                let u = (i as f64 + 0.5) / bays as f64;
                p.stroke(&[(u - 0.025, 0.93), (u + 0.025, 0.93)], Detail);
                p.stroke(
                    &[
                        (u - 0.30 / bays as f64, 0.36),
                        (u + 0.30 / bays as f64, 0.36),
                    ],
                    Fine,
                );
            }
        }
        FixtureKind::Shelves => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.015, Outline);
            p.stroke(&[(0.03, 0.12), (0.97, 0.12)], Detail);
            p.stroke(&[(0.03, 0.94), (0.97, 0.94)], Detail);
            let bays = (width / 0.42).ceil().clamp(1.0, 12.0) as usize;
            for i in 0..bays {
                let u0 = i as f64 / bays as f64;
                if i > 0 {
                    p.stroke(&[(u0, 0.03), (u0, 0.97)], Detail);
                }
                for j in 0..3 {
                    let u = u0 + (0.10 + j as f64 * 0.13) / bays as f64;
                    p.box_at(
                        [u, 0.22, u + 0.09 / bays as f64, 0.82 - j as f64 * 0.08],
                        0.01,
                        Fine,
                    );
                }
            }
        }
        FixtureKind::Desk => {
            let chair_w = (width * 0.36).min(0.48);
            let chair_d = (depth * 0.65).min(0.45);
            p.chair(
                0.0,
                depth * 0.5 + chair_d * 0.5 + depth * 0.06,
                chair_w,
                chair_d,
                2,
            );
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.035, Outline);
            p.box_at([0.30, 0.16, 0.70, 0.24], 0.06, Detail);
            p.stroke(&[(0.50, 0.24), (0.50, 0.35)], Detail);
            p.box_at([0.43, 0.34, 0.57, 0.39], 0.08, Detail);
            p.box_at([0.30, 0.51, 0.69, 0.70], 0.07, Detail);
            p.oval(0.79, 0.61, 0.027, 0.065, Detail);
            p.stroke(&[(0.25, 0.04), (0.25, 0.94)], Fine);
            for i in 0..8 {
                let u = 0.33 + i as f64 * 0.044;
                p.stroke(&[(u, 0.56), (u, 0.65)], Fine);
            }
        }
        FixtureKind::Washer => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.045, Outline);
            p.stroke(&[(0.02, 0.19), (0.98, 0.19)], Detail);
            p.box_at([0.08, 0.045, 0.37, 0.135], 0.10, Detail);
            p.dot_ring(0.76, 0.09, 0.045, Detail);
            p.dot_ring(0.50, 0.59, 0.315, Detail);
            p.dot_ring(0.50, 0.59, 0.26, Fine);
            p.box_at([0.74, 0.53, 0.81, 0.65], 0.20, Detail);
            p.stroke(
                &[
                    (0.34, 0.72),
                    (0.42, 0.67),
                    (0.51, 0.72),
                    (0.60, 0.67),
                    (0.66, 0.72),
                ],
                Fine,
            );
        }
        FixtureKind::TvStand => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.025, Outline);
            p.box_at([0.12, 0.16, 0.88, 0.27], 0.08, Detail);
            p.stroke(&[(0.50, 0.27), (0.50, 0.48)], Detail);
            p.box_at([0.41, 0.46, 0.59, 0.52], 0.08, Detail);
            p.stroke(&[(0.02, 0.91), (0.98, 0.91)], Detail);
            for u in [0.33, 0.67] {
                p.stroke(&[(u, 0.57), (u, 0.91)], Fine);
            }
            p.box_at([0.39, 0.68, 0.61, 0.79], 0.08, Fine);
        }
        FixtureKind::Bathtub => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.40, Outline);
            p.box_at([0.055, 0.10, 0.92, 0.90], 0.43, Detail);
            p.box_at([0.085, 0.16, 0.89, 0.84], 0.43, Fine);
            p.drain(0.80, 0.50, 0.035);
            p.faucet(0.955, 0.40);
            p.dot_ring(0.955, 0.29, 0.025, Fine);
            p.dot_ring(0.955, 0.71, 0.025, Fine);
        }
        FixtureKind::Car => {
            // +Y is the front. Mirrors and wheels are part of the shared symbol.
            for (u, v) in [(0.01, 0.20), (0.99, 0.20), (0.01, 0.78), (0.99, 0.78)] {
                p.box_at([u - 0.025, v - 0.065, u + 0.025, v + 0.065], 0.15, Outline);
            }
            p.path(
                &[
                    (0.12, 0.0),
                    (0.88, 0.0),
                    (0.97, 0.06),
                    (1.0, 0.18),
                    (1.0, 0.88),
                    (0.89, 0.98),
                    (0.11, 0.98),
                    (0.0, 0.88),
                    (0.0, 0.18),
                    (0.03, 0.06),
                ],
                true,
                Outline,
            );
            p.path(
                &[(0.14, 0.50), (0.86, 0.50), (0.92, 0.69), (0.08, 0.69)],
                true,
                Detail,
            );
            p.path(
                &[(0.14, 0.26), (0.86, 0.26), (0.91, 0.12), (0.09, 0.12)],
                true,
                Detail,
            );
            p.box_at([0.14, 0.26, 0.86, 0.50], 0.06, Detail);
            p.box_at([-0.05, 0.57, 0.025, 0.63], 0.15, Outline);
            p.box_at([0.975, 0.57, 1.05, 0.63], 0.15, Outline);
            p.stroke(
                &[(0.09, 0.69), (0.16, 0.91), (0.84, 0.91), (0.91, 0.69)],
                Detail,
            );
            p.stroke(&[(0.08, 0.93), (0.30, 0.95)], Detail);
            p.stroke(&[(0.70, 0.95), (0.92, 0.93)], Detail);
            for u in [0.07, 0.93] {
                p.stroke(&[(u, 0.28), (u, 0.52)], Fine);
            }
            p.stroke(&[(0.08, 0.43), (0.14, 0.43)], Fine);
            p.stroke(&[(0.86, 0.43), (0.92, 0.43)], Fine);
        }
        FixtureKind::Umbrella => {
            let r = width.min(depth) * 0.5;
            let mut vertices = Vec::new();
            for i in 0..8 {
                let a = TAU * (i as f64 + 0.5) / 8.0;
                vertices.push(Point::new(r * a.cos(), r * a.sin()));
            }
            p.shapes.push(SymbolShape::Polyline {
                points: vertices.clone(),
                closed: true,
                dashed: false,
                weight: Outline,
            });
            for v in vertices {
                p.shapes
                    .push(line(&[(0.0, 0.0), (v.x, v.y)], false, Detail));
            }
            p.dot_ring(0.5, 0.5, 0.045, Outline);
            p.dot_ring(0.5, 0.5, 0.018, Fine);
        }
        FixtureKind::SunLounger => {
            p.box_at([0.0, 0.0, 1.0, 1.0], 0.045, Outline);
            p.box_at([0.07, 0.035, 0.93, 0.965], 0.07, Detail);
            p.stroke(&[(0.04, 0.35), (0.96, 0.35)], Detail);
            p.stroke(&[(0.07, 0.70), (0.93, 0.70)], Detail);
            p.box_at([0.20, 0.08, 0.80, 0.21], 0.20, Detail);
            p.box_at([0.02, 0.40, 0.10, 0.65], 0.15, Detail);
            p.box_at([0.90, 0.40, 0.98, 0.65], 0.15, Detail);
            for i in 0..7 {
                let v = 0.40 + i as f64 * 0.08;
                p.stroke(&[(0.14, v), (0.86, v)], Fine);
            }
        }
        _ => {}
    }
    p.shapes
}
