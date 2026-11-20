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
    BedDouble,
    BedSingle,
    Sofa,
    Armchair,
    Wardrobe,
    Desk,
    DiningSet,
    Washer,
    TvStand,
    Bathtub,
    // Electrical devices (see `electrical.rs`).
    OutletLow,
    OutletMid,
    OutletHigh,
    OutletSpecific,
    Switch,
    SwitchTwoWay,
    CeilingLight,
    WallLight,
    Panel,
    DataPoint,
}

impl FixtureKind {
    pub const ALL: [FixtureKind; 21] = [
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
        FixtureKind::BedDouble,
        FixtureKind::BedSingle,
        FixtureKind::Sofa,
        FixtureKind::Armchair,
        FixtureKind::Wardrobe,
        FixtureKind::Desk,
        FixtureKind::DiningSet,
        FixtureKind::Washer,
        FixtureKind::TvStand,
        FixtureKind::Bathtub,
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
            Self::BedDouble => "Cama de casal",
            Self::BedSingle => "Cama de solteiro",
            Self::Sofa => "Sofá",
            Self::Armchair => "Poltrona",
            Self::Wardrobe => "Guarda-roupa",
            Self::Desk => "Escrivaninha",
            Self::DiningSet => "Mesa de jantar",
            Self::Washer => "Máquina de lavar",
            Self::TvStand => "Rack / TV",
            Self::Bathtub => "Banheira",
            Self::OutletLow => "Tomada baixa",
            Self::OutletMid => "Tomada média",
            Self::OutletHigh => "Tomada alta",
            Self::OutletSpecific => "Tomada de uso específico (TUE)",
            Self::Switch => "Interruptor simples",
            Self::SwitchTwoWay => "Interruptor paralelo",
            Self::CeilingLight => "Ponto de luz no teto",
            Self::WallLight => "Arandela",
            Self::Panel => "Quadro de distribuição",
            Self::DataPoint => "Ponto de dados/TV",
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
            Self::BedDouble => "cama_casal",
            Self::BedSingle => "cama_solteiro",
            Self::Sofa => "sofa",
            Self::Armchair => "poltrona",
            Self::Wardrobe => "guarda_roupa",
            Self::Desk => "escrivaninha",
            Self::DiningSet => "mesa_jantar",
            Self::Washer => "lavadora",
            Self::TvStand => "rack_tv",
            Self::Bathtub => "banheira",
            Self::OutletLow => "tomada_baixa",
            Self::OutletMid => "tomada_media",
            Self::OutletHigh => "tomada_alta",
            Self::OutletSpecific => "tomada_tue",
            Self::Switch => "interruptor",
            Self::SwitchTwoWay => "interruptor_paralelo",
            Self::CeilingLight => "luz_teto",
            Self::WallLight => "arandela",
            Self::Panel => "quadro",
            Self::DataPoint => "ponto_dados",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain(Self::ELECTRICAL)
            .find(|kind| kind.key() == key)
    }

    /// Usual width (along the wall) and depth, in metres.
    pub fn default_size(self) -> (f64, f64) {
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
            Self::BedDouble => (1.6, 2.0),
            Self::BedSingle => (0.9, 1.95),
            Self::Sofa => (2.1, 0.85),
            Self::Armchair => (0.85, 0.85),
            Self::Wardrobe => (1.8, 0.6),
            Self::Desk => (1.4, 0.65),
            Self::DiningSet => (1.8, 0.9),
            Self::Washer => (0.6, 0.6),
            Self::TvStand => (1.6, 0.45),
            Self::Bathtub => (1.7, 0.8),
            Self::OutletLow | Self::OutletMid | Self::OutletHigh | Self::OutletSpecific => {
                (0.3, 0.22)
            }
            Self::Switch | Self::SwitchTwoWay => (0.24, 0.24),
            Self::CeilingLight => (0.4, 0.4),
            Self::WallLight => (0.3, 0.2),
            Self::Panel => (0.5, 0.2),
            Self::DataPoint => (0.26, 0.2),
        }
    }

    /// Recommended functional clearance in front of the fixture (metres) for
    /// ergonomics and accessibility checks.
    pub fn front_clearance_m(self) -> f64 {
        match self {
            Self::Toilet | Self::Washbasin | Self::KitchenSink | Self::Stove | Self::Counter => {
                0.60
            }
            Self::Fridge | Self::Washer | Self::Wardrobe => 0.75,
            Self::Desk | Self::DiningSet | Self::Table => 0.70,
            Self::Sofa | Self::Armchair => 0.60,
            Self::BedDouble | Self::BedSingle => 0.55,
            Self::Shower | Self::Bathtub => 0.60,
            Self::WoodStove | Self::Grill => 0.80,
            Self::Shelves | Self::TvStand => 0.50,
            // Wall and ceiling devices take no floor space; the panel needs
            // free access in front of it.
            Self::Panel => 0.70,
            Self::OutletLow
            | Self::OutletMid
            | Self::OutletHigh
            | Self::OutletSpecific
            | Self::Switch
            | Self::SwitchTwoWay
            | Self::CeilingLight
            | Self::WallLight
            | Self::DataPoint => 0.0,
        }
    }

    /// Kinds whose symbol does not say what they are by itself, so their
    /// name is written on them when there is room.
    pub fn needs_label(self) -> bool {
        matches!(
            self,
            Self::Counter
                | Self::Fridge
                | Self::Grill
                | Self::WoodStove
                | Self::Shelves
                | Self::Wardrobe
                | Self::TvStand
        )
    }
}

pub const MIN_SIZE: f64 = 0.2;
pub const MAX_SIZE: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: u64,
    pub kind: FixtureKind,
    pub center: Point,
    /// Along the fixture's back (the wall it stands against).
    pub width: f64,
    /// From the back to the front.
    pub depth: f64,
    /// Clockwise quarter turns as seen on screen: 0 has its back up, 1 to
    /// the right, 2 down and 3 to the left.
    #[serde(default)]
    pub quarter_turns: u8,
    /// Circuit, power and height, for electrical devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub electrical: Option<super::electrical::ElectricalData>,
}

pub fn valid_size(width: f64, depth: f64) -> bool {
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
    pub fn extent(&self) -> (f64, f64) {
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
        radius: f64,
        weight: Weight,
    },
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64, weight: Weight) -> SymbolShape {
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

fn line(points: &[(f64, f64)], dashed: bool, weight: Weight) -> SymbolShape {
    SymbolShape::Polyline {
        points: points.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
        closed: false,
        dashed,
        weight,
    }
}

fn circle(x: f64, y: f64, radius: f64, weight: Weight) -> SymbolShape {
    SymbolShape::Circle {
        center: Point::new(x, y),
        radius,
        weight,
    }
}

fn ellipse(x: f64, y: f64, rx: f64, ry: f64, weight: Weight) -> SymbolShape {
    SymbolShape::Polyline {
        points: (0..24)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 24.0;
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
pub fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    use Weight::{Detail, Outline};
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let outline = rect(-hw, -hd, hw, hd, Outline);
    let small = width.min(depth);
    match kind {
        FixtureKind::OutletLow
        | FixtureKind::OutletMid
        | FixtureKind::OutletHigh
        | FixtureKind::OutletSpecific
        | FixtureKind::Switch
        | FixtureKind::SwitchTwoWay
        | FixtureKind::CeilingLight
        | FixtureKind::WallLight
        | FixtureKind::Panel
        | FixtureKind::DataPoint => super::electrical::symbol(kind, width, depth),
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
                let x = x0 + (x1 - x0) * i as f64 / bars as f64;
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
            let slot = width / basins as f64;
            let mut shapes = vec![outline];
            for i in 0..basins {
                let x = -hw + slot * (i as f64 + 0.5);
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
                let x = -hw + width * (i as f64 + 0.5) / chairs as f64;
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
        FixtureKind::BedDouble => {
            let head = -hd + 0.12;
            let pillow_y1 = (-hd + 0.50).min(0.0);
            let fold_y = (-hd + depth * 0.42).min(hd - 0.15);
            vec![
                outline,
                line(&[(-hw, head), (hw, head)], false, Detail),
                rect(-hw + 0.10, head + 0.04, -0.05, pillow_y1, Detail),
                rect(0.05, head + 0.04, hw - 0.10, pillow_y1, Detail),
                line(&[(-hw, fold_y), (hw, fold_y)], false, Detail),
            ]
        }
        FixtureKind::BedSingle => {
            let head = -hd + 0.10;
            let pillow_y1 = (-hd + 0.46).min(0.0);
            let fold_y = (-hd + depth * 0.42).min(hd - 0.15);
            vec![
                outline,
                line(&[(-hw, head), (hw, head)], false, Detail),
                rect(-hw + 0.10, head + 0.04, hw - 0.10, pillow_y1, Detail),
                line(&[(-hw, fold_y), (hw, fold_y)], false, Detail),
            ]
        }
        FixtureKind::Sofa => {
            let back = -hd + (depth * 0.24).clamp(0.12, 0.25);
            let arm = (width * 0.09).clamp(0.10, 0.22);
            let inner_w = (width - 2.0 * arm).max(0.2);
            let seats = (inner_w / 0.65).round().clamp(2.0, 4.0) as usize;
            let mut shapes = vec![
                outline,
                rect(-hw, -hd, -hw + arm, hd, Detail),
                rect(hw - arm, -hd, hw, hd, Detail),
                rect(-hw + arm, -hd, hw - arm, back, Detail),
            ];
            for i in 1..seats {
                let x = -hw + arm + inner_w * i as f64 / seats as f64;
                shapes.push(line(&[(x, back), (x, hd)], false, Detail));
            }
            shapes
        }
        FixtureKind::Armchair => {
            let back = -hd + (depth * 0.26).clamp(0.12, 0.24);
            let arm = (width * 0.18).clamp(0.10, 0.20);
            vec![
                outline,
                rect(-hw, -hd, -hw + arm, hd, Detail),
                rect(hw - arm, -hd, hw, hd, Detail),
                rect(-hw + arm, -hd, hw - arm, back, Detail),
            ]
        }
        FixtureKind::Wardrobe => vec![
            outline,
            line(&[(-hw, -hd), (hw, hd)], true, Detail),
            line(&[(-hw, hd), (hw, -hd)], true, Detail),
            line(&[(-hw, hd - 0.05), (hw, hd - 0.05)], false, Detail),
        ],
        FixtureKind::Desk => vec![
            outline,
            rect(-0.22, hd + 0.06, 0.22, hd + 0.46, Detail),
            line(&[(-0.22, hd + 0.36), (0.22, hd + 0.36)], false, Detail),
        ],
        FixtureKind::DiningSet => {
            let mut shapes = vec![outline];
            let per_side = (width / 0.6).floor().clamp(1.0, 4.0) as usize;
            for i in 0..per_side {
                let x = -hw + width * (i as f64 + 0.5) / per_side as f64;
                shapes.push(rect(x - 0.20, -hd - 0.42, x + 0.20, -hd - 0.06, Detail));
                shapes.push(rect(x - 0.20, hd + 0.06, x + 0.20, hd + 0.42, Detail));
            }
            shapes.push(rect(-hw - 0.42, -0.20, -hw - 0.06, 0.20, Detail));
            shapes.push(rect(hw + 0.06, -0.20, hw + 0.42, 0.20, Detail));
            shapes
        }
        FixtureKind::Washer => vec![
            outline,
            circle(0.0, 0.02, small * 0.32, Detail),
            line(&[(-hw, -hd + 0.10), (hw, -hd + 0.10)], false, Detail),
        ],
        FixtureKind::TvStand => vec![
            outline,
            rect(-hw * 0.72, -hd + 0.06, hw * 0.72, -hd + 0.14, Outline),
        ],
        FixtureKind::Bathtub => vec![
            outline,
            rect(-hw + 0.08, -hd + 0.08, hw - 0.08, hd - 0.08, Detail),
            circle(hw - 0.22, 0.0, 0.035, Detail),
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
            electrical: None,
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
