//! Fixtures and furniture placed in the plan: counters, sinks, stoves, the
//! barbecue grill, bathroom fittings, tables and shelves.
//!
//! Each kind has a plan symbol made of simple shapes in the fixture's own
//! frame, so the canvas, the PNG/PDF export and the ribbon icons all draw
//! the same thing from one definition.

use serde::{Deserialize, Serialize};

use super::geometry::Point;

mod symbols;

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
    Car,
    Umbrella,
    SunLounger,
    // Electrical devices (see `electrical.rs`).
    OutletLow,
    OutletMid,
    OutletHigh,
    OutletSpecific,
    Switch,
    SwitchTwoWay,
    SwitchIntermediate,
    CeilingLight,
    WallLight,
    Panel,
    DataPoint,
    // Structural elements (see `structure.rs`).
    Column,
    Footing,
    /// Pile cap over bored pile(s) (*bloco sobre broca / estaca*).
    PileCap,
    WaterTank,
    HeavyEquipment,
    /// A timber post (*mourão*), round when width = depth (#146).
    TimberPost,
}

impl FixtureKind {
    pub const ALL: [FixtureKind; 24] = [
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
        FixtureKind::Car,
        FixtureKind::Umbrella,
        FixtureKind::SunLounger,
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
            Self::Car => "Carro",
            Self::Umbrella => "Guarda-sol",
            Self::SunLounger => "Cadeira de sol",
            Self::OutletLow => "Tomada baixa",
            Self::OutletMid => "Tomada média",
            Self::OutletHigh => "Tomada alta",
            Self::OutletSpecific => "Tomada de uso específico (TUE)",
            Self::Switch => "Interruptor simples",
            Self::SwitchTwoWay => "Interruptor paralelo",
            Self::SwitchIntermediate => "Interruptor intermediário",
            Self::CeilingLight => "Ponto de luz no teto",
            Self::WallLight => "Arandela",
            Self::Panel => "Quadro de distribuição",
            Self::DataPoint => "Ponto de dados/TV",
            Self::Column => "Pilar",
            Self::Footing => "Sapata",
            Self::PileCap => "Bloco sobre broca",
            Self::WaterTank => "Caixa-d'água",
            Self::HeavyEquipment => "Equipamento pesado",
            Self::TimberPost => "Mourão de madeira",
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
            Self::Car => "carro",
            Self::Umbrella => "guarda_sol",
            Self::SunLounger => "cadeira_sol",
            Self::OutletLow => "tomada_baixa",
            Self::OutletMid => "tomada_media",
            Self::OutletHigh => "tomada_alta",
            Self::OutletSpecific => "tomada_tue",
            Self::Switch => "interruptor",
            Self::SwitchTwoWay => "interruptor_paralelo",
            Self::SwitchIntermediate => "interruptor_intermediario",
            Self::CeilingLight => "luz_teto",
            Self::WallLight => "arandela",
            Self::Panel => "quadro",
            Self::DataPoint => "ponto_dados",
            Self::Column => "pilar",
            Self::Footing => "sapata",
            Self::PileCap => "bloco_broca",
            Self::WaterTank => "caixa_agua",
            Self::HeavyEquipment => "equipamento_pesado",
            Self::TimberPost => "mourao",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain(Self::ELECTRICAL)
            .chain(Self::STRUCTURAL)
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
            Self::Car => (1.85, 4.50),
            Self::Umbrella => (2.40, 2.40),
            Self::SunLounger => (0.75, 1.95),
            Self::OutletLow | Self::OutletMid | Self::OutletHigh | Self::OutletSpecific => {
                (0.3, 0.22)
            }
            Self::Switch | Self::SwitchTwoWay | Self::SwitchIntermediate => (0.24, 0.24),
            Self::CeilingLight => (0.4, 0.4),
            Self::WallLight => (0.3, 0.2),
            Self::Panel => (0.5, 0.2),
            Self::DataPoint => (0.26, 0.2),
            Self::Column => (0.30, 0.14),
            Self::Footing => (0.80, 0.80),
            Self::PileCap => (0.45, 0.45),
            Self::WaterTank => (1.20, 1.20),
            Self::HeavyEquipment => (1.00, 1.00),
            Self::TimberPost => (0.15, 0.15),
        }
    }

    /// Seats represented by the dimension-aware furniture symbol. The head
    /// chairs are included for a dining set, so a 1.80 m set has six seats.
    pub fn seat_count(self, width: f64) -> Option<usize> {
        if !width.is_finite() || width <= 0.0 {
            return None;
        }
        match self {
            Self::Sofa => Some(((width - 0.32) / 0.65).round().clamp(1.0, 6.0) as usize),
            Self::Armchair => Some(1),
            Self::Table => Some(2 * (width / 0.60).floor().clamp(1.0, 10.0) as usize),
            Self::DiningSet => {
                Some(2 * ((width - 0.35) / 0.60).floor().clamp(1.0, 10.0) as usize + 2)
            }
            _ => None,
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
            Self::Car => 0.60,
            Self::SunLounger | Self::Umbrella => 0.50,
            // Wall and ceiling devices take no floor space; the panel needs
            // free access in front of it.
            Self::Panel => 0.70,
            Self::OutletLow
            | Self::OutletMid
            | Self::OutletHigh
            | Self::OutletSpecific
            | Self::Switch
            | Self::SwitchTwoWay
            | Self::SwitchIntermediate
            | Self::CeilingLight
            | Self::WallLight
            | Self::DataPoint
            | Self::Column
            | Self::Footing
            | Self::PileCap
            | Self::WaterTank
            | Self::HeavyEquipment
            | Self::TimberPost => 0.0,
        }
    }

    /// Smallest allowed side (metres) for this fixture kind.
    ///
    /// Columns can be as narrow as 14 cm (NBR 6118), so they allow down to
    /// 10 cm; every other fixture keeps the 20 cm minimum.
    pub fn min_size(self) -> f64 {
        match self {
            Self::Column | Self::TimberPost => MIN_COLUMN_SIZE,
            _ => MIN_SIZE,
        }
    }

    /// Sanitary and wet-area fixtures that depend on hydraulic/drainage points.
    pub fn is_plumbing(self) -> bool {
        matches!(
            self,
            Self::KitchenSink
                | Self::Washbasin
                | Self::Toilet
                | Self::Shower
                | Self::Bathtub
                | Self::Washer
        )
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
                | Self::WaterTank
                | Self::HeavyEquipment
        )
    }
}

/// Default minimum side for fixtures (20 cm).
pub const MIN_SIZE: f64 = 0.2;
/// Columns go down to 14 cm (NBR 6118), so their smallest side is 10 cm.
pub const MIN_COLUMN_SIZE: f64 = 0.1;
pub const MAX_SIZE: f64 = 8.0;

fn is_zero_angle(angle: &f64) -> bool {
    angle.abs() < 1e-9
}

/// Specifications of the bored pile(s) (*brocas / estacas*) under a
/// [`FixtureKind::PileCap`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PileCapSpec {
    /// Pile shaft diameter, in metres (default `0.25` m = Ø25 cm).
    pub diameter_m: f64,
    /// Pile depth below ground, in metres (default `4.00` m).
    pub depth_m: f64,
    /// Number of piles grouped under this cap (default `1`, up to `4`).
    pub count: u32,
    /// Starter-bar anchorage length descending inside the pile, in metres
    /// (default `2.00` m; ≥ `1.50` m required to anchor wind uplift).
    pub anchor_depth_m: f64,
}

impl Default for PileCapSpec {
    fn default() -> Self {
        Self {
            diameter_m: 0.25,
            depth_m: 4.0,
            count: 1,
            anchor_depth_m: 2.0,
        }
    }
}

impl PileCapSpec {
    pub fn valid(self) -> bool {
        self.diameter_m.is_finite()
            && (0.15..=0.80).contains(&self.diameter_m)
            && self.depth_m.is_finite()
            && (2.0..=20.0).contains(&self.depth_m)
            && (1..=4).contains(&self.count)
            && self.anchor_depth_m.is_finite()
            && (0.0..=self.depth_m).contains(&self.anchor_depth_m)
    }
}

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
    /// Additional free rotation in radians (clockwise on screen), used e.g.
    /// for columns aligned to angled walls.
    #[serde(default, skip_serializing_if = "is_zero_angle")]
    pub angle_rad: f64,
    /// Circuit, power and height, for electrical devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub electrical: Option<super::electrical::ElectricalData>,
    /// Characteristic concentrated load applied to the structure, in kN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_load_kn: Option<f64>,
    /// Bored-pile parameters when `kind == FixtureKind::PileCap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pile: Option<PileCapSpec>,
}

pub fn valid_size(kind: FixtureKind, width: f64, depth: f64) -> bool {
    let min = kind.min_size();
    width.is_finite()
        && depth.is_finite()
        && (min..=MAX_SIZE).contains(&width)
        && (min..=MAX_SIZE).contains(&depth)
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
    /// Total clockwise rotation angle in radians (screen / plan coordinates).
    pub fn rotation_rad(self) -> f64 {
        f64::from(self.quarter_turns % 4) * std::f64::consts::FRAC_PI_2 + self.angle_rad
    }

    /// A point of the fixture's own frame (x along the width, y from the
    /// back at -depth/2 to the front at +depth/2) in plan coordinates.
    pub fn to_world(self, local: Point) -> Point {
        let rotated = if self.angle_rad.abs() < 1e-9 {
            rotate(local, self.quarter_turns)
        } else {
            let (s, c) = self.rotation_rad().sin_cos();
            Point::new(local.x * c - local.y * s, local.x * s + local.y * c)
        };
        Point::new(self.center.x + rotated.x, self.center.y + rotated.y)
    }

    pub fn to_local(self, world: Point) -> Point {
        let offset = Point::new(world.x - self.center.x, world.y - self.center.y);
        if self.angle_rad.abs() < 1e-9 {
            rotate(offset, 4 - self.quarter_turns % 4)
        } else {
            let (s, c) = self.rotation_rad().sin_cos();
            Point::new(offset.x * c + offset.y * s, -offset.x * s + offset.y * c)
        }
    }

    /// Width and depth as laid on the plan's X and Y axes.
    pub fn extent(&self) -> (f64, f64) {
        if self.angle_rad.abs() < 1e-9 {
            if self.quarter_turns.is_multiple_of(2) {
                (self.width, self.depth)
            } else {
                (self.depth, self.width)
            }
        } else {
            let (s, c) = self.rotation_rad().sin_cos();
            (
                c.abs() * self.width + s.abs() * self.depth,
                s.abs() * self.width + c.abs() * self.depth,
            )
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
        if local.x.abs() <= self.width / 2.0 + 1e-4 && local.y.abs() <= self.depth / 2.0 + 1e-4 {
            return true;
        }
        // Chairs and mirrors are selectable without making the empty space
        // around the whole furniture body a hit target. Reject distant points
        // before allocating any symbol geometry during hover hit testing.
        if !matches!(
            self.kind,
            FixtureKind::Table | FixtureKind::DiningSet | FixtureKind::Desk | FixtureKind::Car
        ) {
            return false;
        }
        let reach = 0.5 + 0.06 * self.width.max(self.depth);
        if local.x.abs() > self.width / 2.0 + reach || local.y.abs() > self.depth / 2.0 + reach {
            return false;
        }
        symbol_with_detail(self.kind,self.width,self.depth,false).iter().any(|shape| {
            matches!(shape, SymbolShape::Polyline { points, closed: true, weight: Weight::Outline, .. }
                if super::geometry::polygon_contains_point(points,local))
        })
    }

    /// Display extents include parts beyond the nominal body, while `bounds`
    /// keeps its physical dimension contract for structural/clearance checks.
    pub fn visual_bounds(&self) -> (Point, Point) {
        let (mut low, mut high) = self.bounds();
        let mut include = |p: Point| {
            low.x = low.x.min(p.x);
            low.y = low.y.min(p.y);
            high.x = high.x.max(p.x);
            high.y = high.y.max(p.y);
        };
        for shape in self.symbol() {
            match shape {
                SymbolShape::Polyline { points, .. } => points
                    .into_iter()
                    .map(|p| self.to_world(p))
                    .for_each(&mut include),
                SymbolShape::Circle { center, radius, .. } => {
                    let c = self.to_world(center);
                    include(Point::new(c.x - radius, c.y - radius));
                    include(Point::new(c.x + radius, c.y + radius));
                }
            }
        }
        (low, high)
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
    /// Seams, drain slots and small controls, omitted at small screen scales.
    Fine,
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
        points: (0..64)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 64.0;
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
    symbol_with_detail(kind, width, depth, true)
}

/// Small previews skip fine details before allocating their geometry. Exports
/// retain all detail, regardless of the current viewport zoom.
pub fn symbol_with_detail(
    kind: FixtureKind,
    width: f64,
    depth: f64,
    fine: bool,
) -> Vec<SymbolShape> {
    if !valid_size(kind, width, depth) {
        return Vec::new();
    }
    if kind.is_electrical() {
        super::electrical::symbol(kind, width, depth)
    } else if kind.is_structural() {
        super::structure::symbol(kind, width, depth)
    } else {
        symbols::interior_symbol(kind, width, depth, fine)
    }
}

/// Local extents of the actual drawing, including chairs and handles outside
/// the furniture body. Preview fitting must not guess these from the kind.
pub fn symbol_bounds(shapes: &[SymbolShape]) -> Option<(Point, Point)> {
    let mut low = Point::new(f64::INFINITY, f64::INFINITY);
    let mut high = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut include = |p: Point| {
        low.x = low.x.min(p.x);
        low.y = low.y.min(p.y);
        high.x = high.x.max(p.x);
        high.y = high.y.max(p.y);
    };
    for shape in shapes {
        match shape {
            SymbolShape::Polyline { points, .. } => points.iter().copied().for_each(&mut include),
            SymbolShape::Circle { center, radius, .. } => {
                include(Point::new(center.x - radius, center.y - radius));
                include(Point::new(center.x + radius, center.y + radius));
            }
        }
    }
    (low.x <= high.x && low.y <= high.y).then_some((low, high))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn detailed_symbols_keep_the_same_outlines_when_simplified() {
        for kind in FixtureKind::ALL {
            let (w, d) = kind.default_size();
            let full = symbol(kind, w, d);
            let simple = symbol_with_detail(kind, w, d, false);
            let retained: Vec<_> =
                full.iter()
                    .filter(|s| match s {
                        SymbolShape::Polyline { weight, .. }
                        | SymbolShape::Circle { weight, .. } => *weight != Weight::Fine,
                    })
                    .cloned()
                    .collect();
            assert_eq!(simple, retained, "{kind:?}");
            assert!(full.len() > simple.len(), "{kind:?} has no fine detail");
        }
    }

    #[test]
    fn chairs_are_selectable_and_included_in_visual_extents_after_rotation() {
        for turns in 0..4 {
            let mut f = fixture(turns);
            f.kind = FixtureKind::DiningSet;
            (f.width, f.depth) = (1.8, 0.9);
            f.angle_rad = 0.31;
            let on_chair = f.to_world(Point::new(-0.45, -0.70));
            assert!(f.contains(on_chair));
            assert!(!f.contains(f.to_world(Point::new(-1.25, -0.85))));
            let (low, high) = f.visual_bounds();
            assert!(on_chair.x >= low.x && on_chair.x <= high.x);
            assert!(on_chair.y >= low.y && on_chair.y <= high.y);
            let (a, b) = f.bounds();
            assert!(low.x < a.x && low.y < a.y && high.x > b.x && high.y > b.y);
        }
        assert_eq!(FixtureKind::DiningSet.seat_count(1.2), Some(4));
        assert_eq!(FixtureKind::DiningSet.seat_count(1.8), Some(6));
        assert_eq!(FixtureKind::DiningSet.seat_count(2.4), Some(8));
        assert_eq!(FixtureKind::Sofa.seat_count(2.1), Some(3));
        assert_eq!(FixtureKind::Sofa.seat_count(2.5), Some(3));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        #[test]
        fn resized_furniture_has_finite_bounded_geometry(w in 0.2f64..=8.0, d in 0.2f64..=8.0) {
            for kind in FixtureKind::ALL {
                let shapes = symbol(kind,w,d);
                let (low,high) = symbol_bounds(&shapes).unwrap();
                prop_assert!(low.x.is_finite() && low.y.is_finite() && high.x.is_finite() && high.y.is_finite());
                prop_assert!(low.x < high.x && low.y < high.y);
                let mut vertices = 0;
                for shape in shapes {
                    match shape {
                        SymbolShape::Polyline { points, closed, .. } => {
                            prop_assert!(points.len() >= if closed {3} else {2}, "invalid vertex count for {:?}",kind);
                            vertices += points.len();
                            for p in points {
                                prop_assert!(p.x.is_finite() && p.y.is_finite());
                                prop_assert!(p.x >= low.x-1e-9 && p.x <= high.x+1e-9);
                                prop_assert!(p.y >= low.y-1e-9 && p.y <= high.y+1e-9);
                            }
                        }
                        SymbolShape::Circle { radius, .. } => prop_assert!(radius.is_finite() && radius > 0.0),
                    }
                }
                prop_assert!(vertices < 4096);
            }
        }
    }

    fn fixture(turns: u8) -> Fixture {
        Fixture {
            id: 1,
            kind: FixtureKind::Counter,
            center: Point::new(10.0, 5.0),
            width: 2.0,
            depth: 0.6,
            quarter_turns: turns,
            angle_rad: 0.0,
            electrical: None,
            structural_load_kn: None,
            pile: None,
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
        for kind in FixtureKind::ALL
            .into_iter()
            .chain(FixtureKind::ELECTRICAL)
            .chain(FixtureKind::STRUCTURAL)
        {
            let (width, depth) = kind.default_size();
            assert!(valid_size(kind, width, depth), "{kind:?}");
            assert!(!symbol(kind, width, depth).is_empty(), "{kind:?}");
            assert_eq!(FixtureKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(FixtureKind::from_key("piscina"), None);
        // Wide washbasins get two basins: a shared sink.
        let double = symbol(FixtureKind::Washbasin, 1.2, 0.5);
        let single = symbol(FixtureKind::Washbasin, 0.6, 0.45);
        assert!(double.len() > single.len());

        // Garage and poolside fixtures: realistic average dimensions and rich CAD symbols.
        assert_eq!(FixtureKind::from_key("carro"), Some(FixtureKind::Car));
        assert_eq!(FixtureKind::Car.default_size(), (1.85, 4.50));
        assert!(symbol(FixtureKind::Car, 1.85, 4.50).len() >= 8);

        assert_eq!(
            FixtureKind::from_key("guarda_sol"),
            Some(FixtureKind::Umbrella)
        );
        assert_eq!(FixtureKind::Umbrella.default_size(), (2.40, 2.40));
        assert!(symbol(FixtureKind::Umbrella, 2.40, 2.40).len() >= 9);

        assert_eq!(
            FixtureKind::from_key("cadeira_sol"),
            Some(FixtureKind::SunLounger)
        );
        assert_eq!(FixtureKind::SunLounger.default_size(), (0.75, 1.95));
        assert!(symbol(FixtureKind::SunLounger, 0.75, 1.95).len() >= 4);
    }

    #[test]
    fn minimum_size_is_per_kind() {
        // Columns allow 14 cm (down to 10 cm), while furniture and other
        // fixtures enforce the 20 cm minimum.
        assert!(valid_size(FixtureKind::Column, 0.30, 0.14));
        assert!(valid_size(FixtureKind::Column, 0.10, 0.10));
        assert!(!valid_size(FixtureKind::Column, 0.09, 0.30));

        assert!(!valid_size(FixtureKind::Table, 0.14, 0.50));
        assert!(valid_size(FixtureKind::Table, 0.20, 0.20));
        assert!(!valid_size(FixtureKind::Footing, 0.14, 0.80));
        assert!(valid_size(FixtureKind::Footing, 0.20, 0.20));
    }
}
