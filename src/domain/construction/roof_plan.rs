//! The roof plan (#71, *planta de cobertura*): each roof's water planes
//! with their slope against the tile's minimum, the ridge, hips, eaves and
//! verges, the gutters along the eaves and the downpipes that empty them.
//!
//! Downpipes follow the usual rule of thumb of 1 cm² of downpipe section
//! per m² of roof (a 100 mm downpipe for every 78 m²), at least one per
//! gutter. The final rainwater design follows NBR 10844 with the city's
//! rainfall intensity.

use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::roof::{Roof, RoofSide, RoofStyle, TileKind};

/// Roof area one 100 mm downpipe drains by the rule of thumb, m².
pub const DOWNPIPE_AREA_M2: f64 = 78.5;
/// Gutter fall towards the downpipes (NBR 10844 minimum), percent.
pub const GUTTER_FALL_PERCENT: f64 = 0.5;

/// One water plane.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofPlane {
    pub polygon: Vec<Point>,
    /// Unit vector of the water's way down, in the model.
    pub downhill: Point,
}

impl RoofPlane {
    /// Horizontal area, m².
    pub fn area_m2(&self) -> f64 {
        polygon_area(&self.polygon)
    }

    pub fn centroid(&self) -> Point {
        let n = self.polygon.len().max(1) as f64;
        let sum = self.polygon.iter().fold(Point::new(0.0, 0.0), |acc, p| {
            Point::new(acc.x + p.x, acc.y + p.y)
        });
        Point::new(sum.x / n, sum.y / n)
    }
}

/// What a roof line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofLineKind {
    /// Cumeeira.
    Ridge,
    /// Espigão.
    Hip,
    /// Beiral, where the water leaves the roof.
    Eave,
    /// Oitão / empena: a sloping side edge.
    Verge,
    /// The high edge of a mono-pitch roof (*rufo*).
    HighEdge,
}

impl RoofLineKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ridge => "cumeeira",
            Self::Hip => "espigão",
            Self::Eave => "beiral",
            Self::Verge => "oitão",
            Self::HighEdge => "rufo (borda alta)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoofLine {
    pub kind: RoofLineKind,
    pub from: Point,
    pub to: Point,
}

/// A gutter along an eave and the downpipes that empty it.
#[derive(Clone, Debug, PartialEq)]
pub struct Gutter {
    pub from: Point,
    pub to: Point,
    /// Roof area that drains into it, m².
    pub area_m2: f64,
    pub downpipes: Vec<Point>,
}

impl Gutter {
    pub fn length_m(&self) -> f64 {
        self.from.distance(self.to)
    }

    /// Design flow by NBR 10844 (Q = I·A/60), L/min, for a rainfall
    /// intensity in mm/h.
    pub fn flow_l_min(&self, intensity_mm_h: f64) -> f64 {
        intensity_mm_h * self.area_m2 / 60.0
    }
}

/// The plan of one roof.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofPlan {
    pub roof_id: u64,
    pub label: String,
    pub style: RoofStyle,
    pub tile: TileKind,
    pub slope_percent: f64,
    pub min_slope_percent: f64,
    pub planes: Vec<RoofPlane>,
    pub lines: Vec<RoofLine>,
    pub gutters: Vec<Gutter>,
}

impl RoofPlan {
    pub fn slope_ok(&self) -> bool {
        self.slope_percent + 1e-6 >= self.min_slope_percent
    }

    pub fn area_m2(&self) -> f64 {
        self.planes.iter().map(RoofPlane::area_m2).sum()
    }
}

fn polygon_area(points: &[Point]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let twice: f64 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    twice.abs() / 2.0
}

/// Whether the eaves run along X (the ridge is parallel to X).
fn along_x(roof: &Roof) -> bool {
    match roof.style {
        RoofStyle::Hip => (roof.max.x - roof.min.x) >= (roof.max.y - roof.min.y),
        RoofStyle::MonoPitch | RoofStyle::Gable => {
            matches!(roof.high_side, RoofSide::North | RoofSide::South)
        }
    }
}

fn gutter(from: Point, to: Point, area_m2: f64) -> Gutter {
    let count = (area_m2 / DOWNPIPE_AREA_M2).ceil().max(1.0) as usize;
    let downpipes = match count {
        1 => vec![from],
        _ => (0..count)
            .map(|i| {
                let t = i as f64 / (count - 1) as f64;
                Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
            })
            .collect(),
    };
    Gutter {
        from,
        to,
        area_m2,
        downpipes,
    }
}

/// The plan of one roof.
pub fn roof_plan(roof: &Roof, label: &str) -> RoofPlan {
    let oh = roof.overhang_m.clamp(0.0, 1.5);
    let (a, c) = (
        Point::new(roof.min.x - oh, roof.min.y - oh),
        Point::new(roof.max.x + oh, roof.max.y + oh),
    );
    let (nw, ne, se, sw) = (a, Point::new(c.x, a.y), c, Point::new(a.x, c.y));
    let line = |kind, from, to| RoofLine { kind, from, to };
    let plane = |polygon: Vec<Point>, downhill: Point| RoofPlane { polygon, downhill };
    let mut planes = Vec::new();
    let mut lines = Vec::new();
    let mut gutters = Vec::new();
    match roof.style {
        RoofStyle::MonoPitch => {
            let (high, low, sides, downhill) = match roof.high_side {
                RoofSide::North => (
                    (nw, ne),
                    (sw, se),
                    [(nw, sw), (ne, se)],
                    Point::new(0.0, 1.0),
                ),
                RoofSide::South => (
                    (sw, se),
                    (nw, ne),
                    [(nw, sw), (ne, se)],
                    Point::new(0.0, -1.0),
                ),
                RoofSide::West => (
                    (nw, sw),
                    (ne, se),
                    [(nw, ne), (sw, se)],
                    Point::new(1.0, 0.0),
                ),
                RoofSide::East => (
                    (ne, se),
                    (nw, sw),
                    [(nw, ne), (sw, se)],
                    Point::new(-1.0, 0.0),
                ),
            };
            let p = plane(vec![nw, ne, se, sw], downhill);
            let area = p.area_m2();
            planes.push(p);
            lines.push(line(RoofLineKind::HighEdge, high.0, high.1));
            lines.push(line(RoofLineKind::Eave, low.0, low.1));
            for (from, to) in sides {
                lines.push(line(RoofLineKind::Verge, from, to));
            }
            gutters.push(gutter(low.0, low.1, area));
        }
        RoofStyle::Gable => {
            if along_x(roof) {
                let my = (a.y + c.y) / 2.0;
                let (rw, re) = (Point::new(a.x, my), Point::new(c.x, my));
                planes.push(plane(vec![nw, ne, re, rw], Point::new(0.0, -1.0)));
                planes.push(plane(vec![rw, re, se, sw], Point::new(0.0, 1.0)));
                lines.push(line(RoofLineKind::Ridge, rw, re));
                lines.push(line(RoofLineKind::Eave, nw, ne));
                lines.push(line(RoofLineKind::Eave, sw, se));
                lines.push(line(RoofLineKind::Verge, nw, sw));
                lines.push(line(RoofLineKind::Verge, ne, se));
                gutters.push(gutter(nw, ne, planes[0].area_m2()));
                gutters.push(gutter(sw, se, planes[1].area_m2()));
            } else {
                let mx = (a.x + c.x) / 2.0;
                let (rn, rs) = (Point::new(mx, a.y), Point::new(mx, c.y));
                planes.push(plane(vec![nw, rn, rs, sw], Point::new(-1.0, 0.0)));
                planes.push(plane(vec![rn, ne, se, rs], Point::new(1.0, 0.0)));
                lines.push(line(RoofLineKind::Ridge, rn, rs));
                lines.push(line(RoofLineKind::Eave, nw, sw));
                lines.push(line(RoofLineKind::Eave, ne, se));
                lines.push(line(RoofLineKind::Verge, nw, ne));
                lines.push(line(RoofLineKind::Verge, sw, se));
                gutters.push(gutter(nw, sw, planes[0].area_m2()));
                gutters.push(gutter(ne, se, planes[1].area_m2()));
            }
        }
        RoofStyle::Hip => {
            let (w, d) = (c.x - a.x, c.y - a.y);
            let (r0, r1) = if along_x(roof) {
                let my = (a.y + c.y) / 2.0;
                (Point::new(a.x + d / 2.0, my), Point::new(c.x - d / 2.0, my))
            } else {
                let mx = (a.x + c.x) / 2.0;
                (Point::new(mx, a.y + w / 2.0), Point::new(mx, c.y - w / 2.0))
            };
            if along_x(roof) {
                planes.push(plane(vec![nw, ne, r1, r0], Point::new(0.0, -1.0)));
                planes.push(plane(vec![r0, r1, se, sw], Point::new(0.0, 1.0)));
                planes.push(plane(vec![nw, r0, sw], Point::new(-1.0, 0.0)));
                planes.push(plane(vec![ne, se, r1], Point::new(1.0, 0.0)));
                lines.push(line(RoofLineKind::Hip, nw, r0));
                lines.push(line(RoofLineKind::Hip, sw, r0));
                lines.push(line(RoofLineKind::Hip, ne, r1));
                lines.push(line(RoofLineKind::Hip, se, r1));
            } else {
                planes.push(plane(vec![nw, r0, r1, sw], Point::new(-1.0, 0.0)));
                planes.push(plane(vec![r0, ne, se, r1], Point::new(1.0, 0.0)));
                planes.push(plane(vec![nw, ne, r0], Point::new(0.0, -1.0)));
                planes.push(plane(vec![sw, r1, se], Point::new(0.0, 1.0)));
                lines.push(line(RoofLineKind::Hip, nw, r0));
                lines.push(line(RoofLineKind::Hip, ne, r0));
                lines.push(line(RoofLineKind::Hip, sw, r1));
                lines.push(line(RoofLineKind::Hip, se, r1));
            }
            if r0.distance(r1) > 0.01 {
                lines.push(line(RoofLineKind::Ridge, r0, r1));
            }
            for (from, to) in [(nw, ne), (ne, se), (se, sw), (sw, nw)] {
                lines.push(line(RoofLineKind::Eave, from, to));
                let area = planes
                    .iter()
                    .find(|p| {
                        p.polygon.iter().any(|q| q.distance(from) < 1e-6)
                            && p.polygon.iter().any(|q| q.distance(to) < 1e-6)
                    })
                    .map_or(0.0, RoofPlane::area_m2);
                gutters.push(gutter(from, to, area));
            }
        }
    }
    RoofPlan {
        roof_id: roof.id,
        label: label.to_owned(),
        style: roof.style,
        tile: roof.tile,
        slope_percent: roof.slope() * 100.0,
        min_slope_percent: roof.tile.min_slope_percent(),
        planes,
        lines,
        gutters,
    }
}

/// The plans of every roof of the active floor.
pub fn roof_plans(project: &Project) -> Vec<RoofPlan> {
    let labels: std::collections::HashMap<u64, String> = project
        .structure_report()
        .roofs
        .into_iter()
        .map(|r| (r.id, r.label))
        .collect();
    project
        .structure
        .roofs
        .iter()
        .map(|roof| {
            let label = labels
                .get(&roof.id)
                .cloned()
                .unwrap_or_else(|| format!("Telhado {}", roof.id));
            roof_plan(roof, &label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_roof(style: RoofStyle) -> Roof {
        let mut roof = Roof::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 6.0),
            RoofSide::North,
            3.5,
            2.6,
        );
        roof.style = style;
        roof
    }

    #[test]
    fn a_gable_has_a_ridge_two_planes_and_two_gutters_draining_the_whole_roof() {
        let plan = roof_plan(&rect_roof(RoofStyle::Gable), "T1");
        assert_eq!(plan.planes.len(), 2);
        assert_eq!(plan.gutters.len(), 2);
        assert!(plan.lines.iter().any(|l| l.kind == RoofLineKind::Ridge));
        let drained: f64 = plan.gutters.iter().map(|g| g.area_m2).sum();
        assert!((drained - 60.0).abs() < 1e-6);
        assert!((plan.area_m2() - 60.0).abs() < 1e-6);
    }

    #[test]
    fn a_hip_roof_has_four_planes_four_hips_and_gutters_all_round() {
        let plan = roof_plan(&rect_roof(RoofStyle::Hip), "T1");
        assert_eq!(plan.planes.len(), 4);
        assert_eq!(
            plan.lines
                .iter()
                .filter(|l| l.kind == RoofLineKind::Hip)
                .count(),
            4
        );
        assert_eq!(plan.gutters.len(), 4);
        assert!((plan.area_m2() - 60.0).abs() < 1e-6);
    }

    #[test]
    fn a_large_roof_gets_more_downpipes_and_a_flat_one_fails_the_tile() {
        let mut roof = Roof::new(
            Point::new(0.0, 0.0),
            Point::new(20.0, 10.0),
            RoofSide::North,
            3.0,
            2.8,
        );
        roof.tile = TileKind::Ceramic;
        let plan = roof_plan(&roof, "T1");
        assert_eq!(plan.gutters.len(), 1);
        assert_eq!(plan.gutters[0].downpipes.len(), 3);
        assert!(!plan.slope_ok());
    }
}
