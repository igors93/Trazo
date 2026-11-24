//! Timber roofs (*meia-água*, *duas águas* and *quatro águas*, #114): tiles,
//! battens, rafters, purlins, wall plates (*frechais*), main beams, pitched
//! trusses (*tesouras*), hip rafters (*espigões*), overhangs and NBR 6123
//! wind uplift sized to ABNT NBR 7190 and NBR 6120.

mod analysis;
mod marks;
mod materials;
mod sizing;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::domain::geometry::Point;

pub use materials::{TileKind, TimberClass, TimberSpecies, timber_design, timber_design_species};
pub use sizing::{RoofElement, RoofElementResult, RoofReaction, RoofResult, RoofUpliftSummary};

/// Roof geometry style (#114).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofStyle {
    /// Mono-pitch (*meia-água*): one plane sloping from `high_side` to its opposite.
    #[default]
    MonoPitch,
    /// Gable (*duas águas*): central ridge parallel to the eaves, two symmetric planes.
    Gable,
    /// Hip (*quatro águas*): four planes sloping to every perimeter edge with hip rafters.
    Hip,
}

impl RoofStyle {
    pub const ALL: [Self; 3] = [Self::MonoPitch, Self::Gable, Self::Hip];

    pub fn label(self) -> &'static str {
        match self {
            Self::MonoPitch => "Meia-água",
            Self::Gable => "Duas águas",
            Self::Hip => "Quatro águas",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::MonoPitch => "meia_agua",
            Self::Gable => "duas_aguas",
            Self::Hip => "quatro_aguas",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "meia_agua" | "meia-agua" | "meia_água" | "meia-água" | "mono_pitch" | "monopitch" => {
                Some(Self::MonoPitch)
            }
            "duas_aguas" | "duas-aguas" | "duas_águas" | "duas-águas" | "gable" => {
                Some(Self::Gable)
            }
            "quatro_aguas" | "quatro-aguas" | "quatro_águas" | "quatro-águas" | "hip" => {
                Some(Self::Hip)
            }
            _ => None,
        }
    }
}

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

/// A timber roof over a plan rectangle (internal coordinates,
/// y down: north is the smaller y). Supports mono-pitch (*meia-água*),
/// gable (*duas águas*) and hip (*quatro águas*) geometries with optional
/// overhangs and pitched timber trusses (#114).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub id: u64,
    pub min: Point,
    pub max: Point,
    #[serde(default)]
    pub style: RoofStyle,
    pub high_side: RoofSide,
    /// Height of the roof at the ridge / high edge and at the eaves / low edge, metres.
    pub high_m: f64,
    pub low_m: f64,
    /// Perimeter eave overhang (*beiral*), metres (0.0 to 1.50).
    #[serde(default)]
    pub overhang_m: f64,
    /// Force timber trusses (*tesouras*) even on mono-pitch roofs; always used on gable/hip.
    #[serde(default)]
    pub use_trusses: bool,
    #[serde(default)]
    pub tile: TileKind,
    #[serde(default)]
    pub timber: TimberClass,
    /// Largest distance between main beams or trusses, metres.
    #[serde(default = "default_beam_spacing")]
    pub beam_spacing_m: f64,
    /// Fixed width by height of the main roof beams, centimetres. When absent,
    /// the lightest passing catalogue section is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam_section_cm: Option<(f64, f64)>,
    /// Additional main-beam positions measured along the eave from `min`, m.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_beam_positions_m: Vec<f64>,
    /// Purlin spacing on the horizontal projection, metres.
    #[serde(default = "default_purlin_spacing")]
    pub purlin_spacing_m: f64,
    #[serde(default = "default_rafter_spacing")]
    pub rafter_spacing_m: f64,
    /// No ceiling: the tiles and battens stay in sight.
    #[serde(default = "default_exposed")]
    pub exposed: bool,
    /// Verified design resistance of the complete lateral path from the roof
    /// plane through its bracing, connections and vertical supports to soil.
    /// `None` means that top restraint cannot be credited in the wind model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lateral_bracing_kn: Option<f64>,
}

impl Roof {
    /// A roof with the usual spacings (defaults to mono-pitch).
    pub fn new(min: Point, max: Point, high_side: RoofSide, high_m: f64, low_m: f64) -> Self {
        Self {
            id: 0,
            min: Point::new(min.x.min(max.x), min.y.min(max.y)),
            max: Point::new(min.x.max(max.x), min.y.max(max.y)),
            style: RoofStyle::MonoPitch,
            high_side,
            high_m,
            low_m,
            overhang_m: 0.0,
            use_trusses: false,
            tile: TileKind::default(),
            timber: TimberClass::default(),
            beam_spacing_m: default_beam_spacing(),
            beam_section_cm: None,
            extra_beam_positions_m: Vec::new(),
            purlin_spacing_m: default_purlin_spacing(),
            rafter_spacing_m: default_rafter_spacing(),
            exposed: true,
            lateral_bracing_kn: None,
        }
    }

    fn along_x(&self) -> bool {
        match self.style {
            RoofStyle::Hip => (self.max.x - self.min.x) >= (self.max.y - self.min.y),
            RoofStyle::MonoPitch | RoofStyle::Gable => {
                matches!(self.high_side, RoofSide::North | RoofSide::South)
            }
        }
    }

    /// Full horizontal span perpendicular to the eaves/ridge, metres.
    pub fn run_m(&self) -> f64 {
        if self.along_x() {
            self.max.y - self.min.y
        } else {
            self.max.x - self.min.x
        }
    }

    /// Horizontal run of a single sloped water plane (full `run_m` for mono-pitch,
    /// half `run_m` for symmetric gable and hip roofs), metres.
    pub fn pitch_run_m(&self) -> f64 {
        match self.style {
            RoofStyle::MonoPitch => self.run_m(),
            RoofStyle::Gable | RoofStyle::Hip => self.run_m() * 0.5,
        }
    }

    /// Length of the eaves (parallel to the ridge), metres.
    pub fn eaves_m(&self) -> f64 {
        if self.along_x() {
            self.max.x - self.min.x
        } else {
            self.max.y - self.min.y
        }
    }

    pub fn slope(&self) -> f64 {
        (self.high_m - self.low_m) / self.pitch_run_m().max(1e-6)
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
        match self.style {
            RoofStyle::MonoPitch => self.high_m - self.slope() * self.distance_from_high(p),
            RoofStyle::Gable => {
                let half = self.run_m() * 0.5;
                let dist_from_ridge = (self.distance_from_high(p) - half).abs();
                (self.high_m - self.slope() * dist_from_ridge).max(self.low_m)
            }
            RoofStyle::Hip => {
                let dx = (p.x - self.min.x).min(self.max.x - p.x).max(0.0);
                let dy = (p.y - self.min.y).min(self.max.y - p.y).max(0.0);
                let d_edge = dx.min(dy).min(self.pitch_run_m());
                (self.low_m + self.slope() * d_edge).min(self.high_m)
            }
        }
    }

    /// The point at `along` on the high or the low edge (for Hip, uses the
    /// two long eave edges perpendicular to the truss span).
    fn edge_point(&self, along: f64, high: bool) -> Point {
        if self.style == RoofStyle::Hip {
            return if self.along_x() {
                Point::new(
                    self.min.x + along,
                    if high { self.min.y } else { self.max.y },
                )
            } else {
                Point::new(
                    if high { self.min.x } else { self.max.x },
                    self.min.y + along,
                )
            };
        }
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
            self.overhang_m,
            self.beam_spacing_m,
            self.purlin_spacing_m,
            self.rafter_spacing_m,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite {
            return Err("Medidas do telhado inválidas.".to_owned());
        }
        if !(0.0..=1.5).contains(&self.overhang_m) {
            return Err("O beiral deve ficar entre 0 e 1,50 m.".to_owned());
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
        if self
            .lateral_bracing_kn
            .is_some_and(|capacity| !capacity.is_finite() || !(0.0..=100_000.0).contains(&capacity))
        {
            return Err(
                "A resistência de contraventamento deve ficar entre 0 e 100.000 kN.".to_owned(),
            );
        }
        if let Some((width, height)) = self.beam_section_cm
            && (!(5.0..=30.0).contains(&width) || !(8.0..=50.0).contains(&height) || height < width)
        {
            return Err(
                "A secao fixa das vigas deve ter largura de 5 a 30 cm e altura de 8 a 50 cm."
                    .to_owned(),
            );
        }
        let mut extra = self.extra_beam_positions_m.clone();
        extra.sort_by(f64::total_cmp);
        if extra.iter().any(|position| {
            !position.is_finite() || *position <= 0.15 || *position >= self.eaves_m() - 0.15
        }) || extra.windows(2).any(|pair| pair[1] - pair[0] < 0.15)
        {
            return Err(
                "As vigas extras devem ficar dentro do beiral e separadas por pelo menos 15 cm."
                    .to_owned(),
            );
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
