//! Fixtures and furniture placed in the plan: counters, sinks, stoves, the
//! barbecue grill, bathroom fittings, tables and shelves.
//!
//! Each kind has a plan symbol made of simple shapes in the fixture's own
//! frame, so the canvas, the PNG/PDF export and the ribbon icons all draw
//! the same thing from one definition.

use serde::{Deserialize, Serialize};

use super::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    Counter,
    KitchenSink,
    Stove,
    WoodStove,
    Grill,
    Fridge,
    Washbasin,
    Toilet,
    Shower,
    Table,
    Shelves,
}

impl FixtureKind {
    pub const ALL: [FixtureKind; 11] = [
        FixtureKind::Counter,
        FixtureKind::KitchenSink,
        FixtureKind::Stove,
        FixtureKind::WoodStove,
        FixtureKind::Grill,
        FixtureKind::Fridge,
        FixtureKind::Washbasin,
        FixtureKind::Toilet,
        FixtureKind::Shower,
        FixtureKind::Table,
        FixtureKind::Shelves,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Counter => "Bancada",
            Self::KitchenSink => "Pia",
            Self::Stove => "Fogão",
            Self::WoodStove => "Fogão a lenha",
            Self::Grill => "Churrasqueira",
            Self::Fridge => "Geladeira",
            Self::Washbasin => "Lavatório",
            Self::Toilet => "Vaso sanitário",
            Self::Shower => "Chuveiro",
            Self::Table => "Mesa",
            Self::Shelves => "Prateleira",
        }
    }

    /// The name tools and files use (no accents or spaces).
    pub fn key(self) -> &'static str {
        match self {
            Self::Counter => "bancada",
            Self::KitchenSink => "pia",
            Self::Stove => "fogao",
            Self::WoodStove => "fogao_lenha",
            Self::Grill => "churrasqueira",
            Self::Fridge => "geladeira",
            Self::Washbasin => "lavatorio",
            Self::Toilet => "vaso",
            Self::Shower => "chuveiro",
            Self::Table => "mesa",
            Self::Shelves => "prateleira",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// Usual width (along the wall) and depth, in metres.
    pub fn default_size(self) -> (f32, f32) {
        match self {
            Self::Counter => (1.5, 0.6),
            Self::KitchenSink => (1.2, 0.6),
            Self::Stove => (0.6, 0.6),
            Self::WoodStove => (1.0, 0.6),
            Self::Grill => (1.0, 0.6),
            Self::Fridge => (0.7, 0.7),
            Self::Washbasin => (0.6, 0.45),
            Self::Toilet => (0.4, 0.65),
            Self::Shower => (0.9, 0.9),
            Self::Table => (2.0, 0.9),
            Self::Shelves => (1.0, 0.4),
        }
    }

    /// Kinds whose symbol does not say what they are by itself, so their
    /// name is written on them when there is room.
    pub fn needs_label(self) -> bool {
        matches!(
            self,
            Self::Counter | Self::Fridge | Self::Grill | Self::WoodStove | Self::Shelves
        )
    }
}

pub const MIN_SIZE: f32 = 0.2;
pub const MAX_SIZE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: u64,
    pub kind: FixtureKind,
    pub center: Point,
    /// Along the fixture's back (the wall it stands against).
    pub width: f32,
    /// From the back to the front.
    pub depth: f32,
    /// Clockwise quarter turns as seen on screen: 0 has its back up, 1 to
    /// the right, 2 down and 3 to the left.
    #[serde(default)]
    pub quarter_turns: u8,
}

pub fn valid_size(width: f32, depth: f32) -> bool {
    width.is_finite()
        && depth.is_finite()
        && (MIN_SIZE..=MAX_SIZE).contains(&width)
        && (MIN_SIZE..=MAX_SIZE).contains(&depth)
}

/// Rotates `local` by `turns` clockwise quarter turns on screen (model Y
/// grows downwards).
fn rotate(local: Point, turns: u8) -> Point {
    let (mut x, mut y) = (local.x, local.y);
    for _ in 0..turns % 4 {
        (x, y) = (-y, x);
    }
    Point::new(x, y)
}

impl Fixture {
    /// A point of the fixture's own frame (x along the width, y from the
    /// back at -depth/2 to the front at +depth/2) in plan coordinates.
    pub fn to_world(self, local: Point) -> Point {
        let rotated = rotate(local, self.quarter_turns);
        Point::new(self.center.x + rotated.x, self.center.y + rotated.y)
    }

    fn to_local(self, world: Point) -> Point {
        let offset = Point::new(world.x - self.center.x, world.y - self.center.y);
        rotate(offset, 4 - self.quarter_turns % 4)
    }

    /// Width and depth as laid on the plan's X and Y axes.
    pub fn extent(&self) -> (f32, f32) {
        if self.quarter_turns.is_multiple_of(2) {
            (self.width, self.depth)
        } else {
            (self.depth, self.width)
        }
    }

    pub fn bounds(&self) -> (Point, Point) {
        let (x, y) = self.extent();
        (
            Point::new(self.center.x - x / 2.0, self.center.y - y / 2.0),
            Point::new(self.center.x + x / 2.0, self.center.y + y / 2.0),
        )
    }

    pub fn contains(&self, point: Point) -> bool {
        let local = self.to_local(point);
        local.x.abs() <= self.width / 2.0 + 1e-4 && local.y.abs() <= self.depth / 2.0 + 1e-4
    }

    pub fn symbol(&self) -> Vec<SymbolShape> {
        symbol(self.kind, self.width, self.depth)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    /// The outline of the fixture.
    Outline,
    /// Inner details: basins, burners, bars.
    Detail,
}

/// One shape of a plan symbol, in the fixture's own frame (metres).
#[derive(Clone, Debug, PartialEq)]
pub enum SymbolShape {
    Polyline {
        points: Vec<Point>,
        closed: bool,
        dashed: bool,
        weight: Weight,
    },
    Circle {
        center: Point,
        radius: f32,
        weight: Weight,
    },
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, weight: Weight) -> SymbolShape {
    SymbolShape::Polyline {
        points: vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ],
        closed: true,
        dashed: false,
        weight,
    }
}

fn line(points: &[(f32, f32)], dashed: bool, weight: Weight) -> SymbolShape {
    SymbolShape::Polyline {
        points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
        closed: false,
        dashed,
        weight,
    }
}

fn circle(x: f32, y: f32, radius: f32, weight: Weight) -> SymbolShape {
    SymbolShape::Circle {
        center: Point::new(x, y),
        radius,
        weight,
    }
}

fn ellipse(x: f32, y: f32, rx: f32, ry: f32, weight: Weight) -> SymbolShape {
    SymbolShape::Polyline {
        points: (0..24)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 24.0;
                Point::new(x + rx * a.cos(), y + ry * a.sin())
            })
            .collect(),
        closed: true,
        dashed: false,
        weight,
    }
}

/// The plan symbol of a fixture of `kind` measuring `width` × `depth`. The
/// back (against the wall) is at y = -depth/2.
pub fn symbol(kind: FixtureKind, width: f32, depth: f32) -> Vec<SymbolShape> {
    use Weight::{Detail, Outline};
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let outline = rect(-hw, -hd, hw, hd, Outline);
    let small = width.min(depth);
    match kind {
        FixtureKind::Counter => vec![
            outline,
            line(&[(-hw, hd - 0.05), (hw, hd - 0.05)], false, Detail),
        ],
        FixtureKind::KitchenSink => {
            let basin_w = (hw - 0.1).clamp(0.08, 0.3);
            vec![
                outline,
                rect(-basin_w, -hd + 0.14, basin_w, hd - 0.08, Detail),
                circle(0.0, 0.03, 0.03, Detail),
                circle(0.0, -hd + 0.07, 0.025, Detail),
            ]
        }
        FixtureKind::Stove => {
            let radius = small * 0.15;
            let mut shapes = vec![outline];
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                shapes.push(circle(x * hw * 0.48, y * hd * 0.45 - 0.02, radius, Detail));
            }
            shapes.push(line(&[(-hw, hd - 0.06), (hw, hd - 0.06)], false, Detail));
            shapes
        }
        FixtureKind::WoodStove => {
            let plate_front = hd - 0.18;
            vec![
                outline,
                rect(-hw + 0.06, -hd + 0.06, hw - 0.22, plate_front, Detail),
                circle(-hw * 0.45, (-hd + plate_front) / 2.0, small * 0.13, Detail),
                circle(hw * 0.05, (-hd + plate_front) / 2.0, small * 0.1, Detail),
                rect(-0.15, plate_front + 0.03, 0.15, hd - 0.03, Detail),
                rect(hw - 0.17, -hd + 0.03, hw - 0.03, -hd + 0.17, Outline),
            ]
        }
        FixtureKind::Grill => {
            let (x0, x1, y0, y1) = (-hw + 0.08, hw - 0.08, -hd + 0.08, hd - 0.12);
            let mut shapes = vec![outline, rect(x0, y0, x1, y1, Detail)];
            let bars = ((x1 - x0) / 0.08).floor().max(1.0) as usize;
            for i in 1..bars {
                let x = x0 + (x1 - x0) * i as f32 / bars as f32;
                shapes.push(line(&[(x, y0), (x, y1)], false, Detail));
            }
            shapes
        }
        FixtureKind::Fridge => vec![
            outline,
            line(&[(-hw, hd - 0.06), (hw, hd - 0.06)], false, Detail),
            line(
                &[(hw - 0.1, -hd + 0.08), (hw - 0.1, hd - 0.12)],
                false,
                Detail,
            ),
        ],
        FixtureKind::Washbasin => {
            let basins = if width >= 0.9 { 2 } else { 1 };
            let slot = width / basins as f32;
            let mut shapes = vec![outline];
            for i in 0..basins {
                let x = -hw + slot * (i as f32 + 0.5);
                shapes.push(ellipse(
                    x,
                    0.03,
                    (slot / 2.0 - 0.07).clamp(0.05, 0.22),
                    (hd - 0.08).clamp(0.05, 0.16),
                    Detail,
                ));
                shapes.push(circle(x, -hd + 0.05, 0.02, Detail));
            }
            shapes
        }
        FixtureKind::Toilet => {
            let tank = (depth * 0.28).clamp(0.08, 0.2);
            vec![
                rect(-hw, -hd, hw, -hd + tank, Outline),
                ellipse(
                    0.0,
                    (-hd + tank + hd) / 2.0,
                    hw * 0.85,
                    (depth - tank) / 2.0,
                    Outline,
                ),
            ]
        }
        FixtureKind::Shower => vec![
            outline,
            line(&[(-hw, -hd), (hw, hd)], false, Detail),
            line(&[(-hw, hd), (hw, -hd)], false, Detail),
            circle(0.0, 0.0, 0.04, Detail),
        ],
        FixtureKind::Table => {
            let mut shapes = vec![outline];
            let chairs = (width / 0.6).floor().max(1.0) as usize;
            for i in 0..chairs {
                let x = -hw + width * (i as f32 + 0.5) / chairs as f32;
                shapes.push(rect(x - 0.2, -hd - 0.45, x + 0.2, -hd - 0.08, Detail));
                shapes.push(rect(x - 0.2, hd + 0.08, x + 0.2, hd + 0.45, Detail));
            }
            shapes
        }
        FixtureKind::Shelves => vec![
            outline,
            line(&[(-hw, -hd), (hw, hd)], true, Detail),
            line(&[(-hw, hd), (hw, -hd)], true, Detail),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(turns: u8) -> Fixture {
        Fixture {
            id: 1,
            kind: FixtureKind::Counter,
            center: Point::new(10.0, 5.0),
            width: 2.0,
            depth: 0.6,
            quarter_turns: turns,
        }
    }

    #[test]
    fn quarter_turns_put_the_back_on_each_side() {
        let back = Point::new(0.0, -0.3);
        let expected = [(10.0, 4.7), (10.3, 5.0), (10.0, 5.3), (9.7, 5.0)];
        for (turns, (x, y)) in expected.into_iter().enumerate() {
            let world = fixture(turns as u8).to_world(back);
            assert!(
                (world.x - x).abs() < 1e-5 && (world.y - y).abs() < 1e-5,
                "{turns}: {world:?}"
            );
        }
        assert_eq!(fixture(1).extent(), (0.6, 2.0));
    }

    #[test]
    fn containment_follows_the_rotation() {
        let lying = fixture(0);
        assert!(lying.contains(Point::new(10.9, 5.2)));
        assert!(!lying.contains(Point::new(10.0, 5.5)));
        let standing = fixture(1);
        assert!(standing.contains(Point::new(10.2, 5.9)));
        assert!(!standing.contains(Point::new(10.9, 5.0)));
    }

    #[test]
    fn every_kind_has_a_symbol_and_a_unique_key() {
        for kind in FixtureKind::ALL {
            let (width, depth) = kind.default_size();
            assert!(valid_size(width, depth));
            assert!(!symbol(kind, width, depth).is_empty(), "{kind:?}");
            assert_eq!(FixtureKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(FixtureKind::from_key("piscina"), None);
        // Wide washbasins get two basins: a shared sink.
        let double = symbol(FixtureKind::Washbasin, 1.2, 0.5);
        let single = symbol(FixtureKind::Washbasin, 0.6, 0.45);
        assert!(double.len() > single.len());
    }
}
