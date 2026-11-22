//! Mono-pitch timber roofs (*meia-água*, #114): tiles, battens, rafters,
//! purlins and main beams sized to ABNT NBR 7190 and NBR 6120.

mod analysis;
mod marks;
mod materials;
mod sizing;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::domain::geometry::Point;

pub use materials::{TileKind, TimberClass, TimberSpecies, timber_design, timber_design_species};
pub use sizing::{RoofElement, RoofElementResult, RoofReaction, RoofResult};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofSide {
    #[default]
    West,
    East,
    North,
    South,
}

impl RoofSide {
    pub const ALL: [Self; 4] = [Self::West, Self::East, Self::North, Self::South];

    pub fn label(self) -> &'static str {
        match self {
            Self::West => "oeste",
            Self::East => "leste",
            Self::North => "norte",
            Self::South => "sul",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "oeste" | "esquerda" | "west" => Some(Self::West),
            "leste" | "direita" | "east" => Some(Self::East),
            "norte" | "cima" | "north" => Some(Self::North),
            "sul" | "baixo" | "south" => Some(Self::South),
            _ => None,
        }
    }

    /// The side the water runs to.
    pub fn opposite(self) -> Self {
        match self {
            Self::West => Self::East,
            Self::East => Self::West,
            Self::North => Self::South,
            Self::South => Self::North,
        }
    }
}

fn default_beam_spacing() -> f64 {
    3.0
}
fn default_purlin_spacing() -> f64 {
    1.2
}
fn default_rafter_spacing() -> f64 {
    0.5
}
fn default_exposed() -> bool {
    true
}

/// A mono-pitch timber roof over a plan rectangle (internal coordinates,
/// y down: north is the smaller y).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub id: u64,
    pub min: Point,
    pub max: Point,
    pub high_side: RoofSide,
    /// Height of the roof at the high and at the low edge, metres.
    pub high_m: f64,
    pub low_m: f64,
    #[serde(default)]
    pub tile: TileKind,
    #[serde(default)]
    pub timber: TimberClass,
    /// Largest distance between main beams, metres.
    #[serde(default = "default_beam_spacing")]
    pub beam_spacing_m: f64,
    /// Purlin spacing on the horizontal projection, metres.
    #[serde(default = "default_purlin_spacing")]
    pub purlin_spacing_m: f64,
    #[serde(default = "default_rafter_spacing")]
    pub rafter_spacing_m: f64,
    /// No ceiling: the tiles and battens stay in sight.
    #[serde(default = "default_exposed")]
    pub exposed: bool,
}

impl Roof {
    /// A roof with the usual spacings.
    pub fn new(min: Point, max: Point, high_side: RoofSide, high_m: f64, low_m: f64) -> Self {
        Self {
            id: 0,
            min: Point::new(min.x.min(max.x), min.y.min(max.y)),
            max: Point::new(min.x.max(max.x), min.y.max(max.y)),
            high_side,
            high_m,
            low_m,
            tile: TileKind::default(),
            timber: TimberClass::default(),
            beam_spacing_m: default_beam_spacing(),
            purlin_spacing_m: default_purlin_spacing(),
            rafter_spacing_m: default_rafter_spacing(),
            exposed: true,
        }
    }

    fn along_x(&self) -> bool {
        matches!(self.high_side, RoofSide::North | RoofSide::South)
    }

    /// Horizontal distance from the high to the low edge, metres.
    pub fn run_m(&self) -> f64 {
        if self.along_x() {
            self.max.y - self.min.y
        } else {
            self.max.x - self.min.x
        }
    }

    /// Length of the eaves (the edges the slope runs between), metres.
    pub fn eaves_m(&self) -> f64 {
        if self.along_x() {
            self.max.x - self.min.x
        } else {
            self.max.y - self.min.y
        }
    }

    pub fn slope(&self) -> f64 {
        (self.high_m - self.low_m) / self.run_m().max(1e-6)
    }

    pub fn cos(&self) -> f64 {
        1.0 / (1.0 + self.slope().powi(2)).sqrt()
    }

    /// Distance from the high edge, metres.
    fn distance_from_high(&self, p: Point) -> f64 {
        let d = match self.high_side {
            RoofSide::West => p.x - self.min.x,
            RoofSide::East => self.max.x - p.x,
            RoofSide::North => p.y - self.min.y,
            RoofSide::South => self.max.y - p.y,
        };
        d.clamp(0.0, self.run_m())
    }

    /// Position along the eaves, metres.
    pub(crate) fn along(&self, p: Point) -> f64 {
        if self.along_x() {
            p.x - self.min.x
        } else {
            p.y - self.min.y
        }
    }

    /// Height of the roof over `p`, metres.
    pub fn height_at(&self, p: Point) -> f64 {
        self.high_m - self.slope() * self.distance_from_high(p)
    }

    /// The point at `along` on the high or the low edge.
    fn edge_point(&self, along: f64, high: bool) -> Point {
        let side = if high {
            self.high_side
        } else {
            self.high_side.opposite()
        };
        match side {
            RoofSide::West => Point::new(self.min.x, self.min.y + along),
            RoofSide::East => Point::new(self.max.x, self.min.y + along),
            RoofSide::North => Point::new(self.min.x + along, self.min.y),
            RoofSide::South => Point::new(self.min.x + along, self.max.y),
        }
    }

    /// Which edge `p` lies on (within `tolerance`, within the eaves):
    /// `Some(true)` the high one, `Some(false)` the low one.
    pub(crate) fn edge_of(&self, p: Point, tolerance: f64) -> Option<bool> {
        if !self.on_edge(p, tolerance) {
            return None;
        }
        Some(self.distance_from_high(p) < self.run_m() / 2.0)
    }

    /// Whether `p` lies on the high or low edge (within `tolerance`) and
    /// within the eaves.
    fn on_edge(&self, p: Point, tolerance: f64) -> bool {
        let d = match self.high_side {
            RoofSide::West | RoofSide::East => {
                (p.x - self.min.x).abs().min((p.x - self.max.x).abs())
            }
            RoofSide::North | RoofSide::South => {
                (p.y - self.min.y).abs().min((p.y - self.max.y).abs())
            }
        };
        let along = self.along(p);
        d <= tolerance && along >= -tolerance && along <= self.eaves_m() + tolerance
    }

    fn check(&self) -> Result<(), String> {
        let finite = [
            self.min.x,
            self.min.y,
            self.max.x,
            self.max.y,
            self.high_m,
            self.low_m,
            self.beam_spacing_m,
            self.purlin_spacing_m,
            self.rafter_spacing_m,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite {
            return Err("Medidas do telhado inválidas.".to_owned());
        }
        if self.run_m() < 1.0 || self.eaves_m() < 1.0 {
            return Err("O telhado precisa de pelo menos 1 × 1 m.".to_owned());
        }
        if self.run_m() > 15.0 {
            return Err("Meia-água com mais de 15 m de caimento: divida em duas águas.".to_owned());
        }
        if !(1.5..=12.0).contains(&self.high_m) || !(1.5..=12.0).contains(&self.low_m) {
            return Err("As alturas do telhado devem ficar entre 1,50 e 12,00 m.".to_owned());
        }
        if self.high_m <= self.low_m + 0.01 {
            return Err("O lado alto precisa ser mais alto que o lado baixo.".to_owned());
        }
        if !(1.0..=6.0).contains(&self.beam_spacing_m) {
            return Err("O espaçamento das vigas deve ficar entre 1 e 6 m.".to_owned());
        }
        if !(0.5..=3.0).contains(&self.purlin_spacing_m) {
            return Err("O espaçamento das terças deve ficar entre 0,5 e 3 m.".to_owned());
        }
        if !(0.3..=1.0).contains(&self.rafter_spacing_m) {
            return Err("O espaçamento dos caibros deve ficar entre 0,3 e 1 m.".to_owned());
        }
        Ok(())
    }
}
