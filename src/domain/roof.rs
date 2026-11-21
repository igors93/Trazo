//! Timber roofs (#114): a mono-pitch roof (*meia-água*) over a rectangle,
//! with its framing sized and its weight delivered to the structure.
//!
//! - **Loads:** the tiles (saturated) and an allowance of 0.10 kN/m² for
//!   battens and rafters, on the roof surface; a ceiling of 0.15 kN/m²
//!   unless the roof is exposed; the live load of 0.25 kN/m² on the
//!   horizontal projection (NBR 6120); each timber piece's own weight.
//! - **Framing:** tiles on battens (*ripas*) at the tile gauge, on rafters
//!   (*caibros*), on purlins (*terças*) running along the eaves, on main
//!   beams (*vigas*) from the high edge to the low edge. Sheets
//!   (fibre-cement, metal) rest on the purlins directly.
//! - **Checks (NBR 7190):** fd = kmod · fk / γw with kmod = 0.70 · 1.0 ·
//!   0.80 = 0.56, γw = 1.4 in bending and compression and 1.8 in shear;
//!   simply supported pieces under 1.4 (g + q); bending σ = M / W ≤ fc0,d,
//!   shear τ = 1.5 V / (b h) ≤ fv0,d, deflection under g + 0.2 q with
//!   Ec0,ef = kmod · Ec0,m within L / 200. The lightest commercial section
//!   that passes is chosen.
//! - **Supports:** the main beams line up with the columns found along the
//!   high and low edges, more added so none is farther apart than the
//!   spacing set. Each beam end is a point load on the column under it, or
//!   on the concrete beam under it; one with neither is reported.
//!
//! Wind uplift and the anchorage of the roof come with #147.

use serde::{Deserialize, Serialize};

use super::conduits::PlanMark;
use super::geometry::Point;
use super::model::Project;

/// What covers the roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    #[default]
    Ceramic,
    Concrete,
    FiberCement,
    Metal,
}

impl TileKind {
    pub const ALL: [Self; 4] = [
        Self::Ceramic,
        Self::Concrete,
        Self::FiberCement,
        Self::Metal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ceramic => "Telha cerâmica",
            Self::Concrete => "Telha de concreto",
            Self::FiberCement => "Fibrocimento 6 mm",
            Self::Metal => "Telha metálica",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ceramic => "ceramica",
            Self::Concrete => "concreto",
            Self::FiberCement => "fibrocimento",
            Self::Metal => "metalica",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase();
        let raw = raw.replace('â', "a").replace('é', "e").replace('á', "a");
        Self::ALL.into_iter().find(|t| t.key() == raw)
    }

    /// Saturated weight on the roof surface, kN/m².
    pub fn weight_kn_m2(self) -> f64 {
        match self {
            Self::Ceramic | Self::Concrete => 0.50,
            Self::FiberCement => 0.18,
            Self::Metal => 0.10,
        }
    }

    /// Usual minimum slope, percent (confirm with the manufacturer).
    pub fn min_slope_percent(self) -> f64 {
        match self {
            Self::Ceramic | Self::Concrete => 30.0,
            Self::FiberCement => 15.0,
            Self::Metal => 10.0,
        }
    }

    /// Batten spacing (the tile gauge, *galga*), or `None` for sheets that
    /// rest on the purlins.
    pub fn gauge_m(self) -> Option<f64> {
        match self {
            Self::Ceramic => Some(0.35),
            Self::Concrete => Some(0.32),
            Self::FiberCement | Self::Metal => None,
        }
    }

    /// Largest purlin spacing a sheet spans, metres.
    pub fn sheet_span_m(self) -> Option<f64> {
        match self {
            Self::FiberCement => Some(1.69),
            Self::Metal => Some(2.0),
            Self::Ceramic | Self::Concrete => None,
        }
    }

    /// Tiles per m² of roof surface; `None` for sheets (counted in m²).
    pub fn per_m2(self) -> Option<f64> {
        match self {
            Self::Ceramic => Some(16.0),
            Self::Concrete => Some(10.5),
            Self::FiberCement | Self::Metal => None,
        }
    }
}

/// Strength class of hardwood (NBR 7190, dicotyledons).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimberClass {
    C20,
    #[default]
    C30,
    C40,
    C60,
}

impl TimberClass {
    pub const ALL: [Self; 4] = [Self::C20, Self::C30, Self::C40, Self::C60];

    pub fn label(self) -> &'static str {
        match self {
            Self::C20 => "Folhosa C20",
            Self::C30 => "Folhosa C30",
            Self::C40 => "Folhosa C40",
            Self::C60 => "Folhosa C60",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::C20 => "c20",
            Self::C30 => "c30",
            Self::C40 => "c40",
            Self::C60 => "c60",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase();
        Self::ALL.into_iter().find(|c| c.key() == raw)
    }

    /// fc0,k, fv0,k and Ec0,m in MPa, apparent density in kg/m³
    /// (NBR 7190:1997, table 9).
    pub fn properties(self) -> (f64, f64, f64, f64) {
        match self {
            Self::C20 => (20.0, 4.0, 9_500.0, 650.0),
            Self::C30 => (30.0, 5.0, 14_500.0, 800.0),
            Self::C40 => (40.0, 6.0, 19_500.0, 950.0),
            Self::C60 => (60.0, 8.0, 24_500.0, 1_000.0),
        }
    }
}

/// kmod = kmod1 · kmod2 · kmod3 (long-term loads, service class 1–2,
/// second-grade sawn timber).
pub const KMOD: f64 = 0.70 * 1.0 * 0.80;

/// The side of the rectangle where the roof is highest.
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
    fn along(&self, p: Point) -> f64 {
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

/// A piece of the framing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofElement {
    Battens,
    Rafters,
    Purlins,
    Beams,
}

impl RoofElement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Battens => "Ripas",
            Self::Rafters => "Caibros",
            Self::Purlins => "Terças",
            Self::Beams => "Vigas",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Battens => "ripas",
            Self::Rafters => "caibros",
            Self::Purlins => "tercas",
            Self::Beams => "vigas",
        }
    }

    /// Commercial sections, width × height in cm, lightest first.
    fn catalog(self) -> &'static [(f64, f64)] {
        match self {
            Self::Battens => &[(5.0, 1.5), (5.0, 2.5)],
            Self::Rafters => &[(5.0, 6.0), (5.0, 7.0), (6.0, 8.0), (6.0, 12.0)],
            Self::Purlins => &[
                (6.0, 12.0),
                (6.0, 16.0),
                (8.0, 16.0),
                (8.0, 20.0),
                (10.0, 20.0),
                (10.0, 25.0),
            ],
            Self::Beams => &[
                (6.0, 16.0),
                (8.0, 16.0),
                (8.0, 20.0),
                (10.0, 20.0),
                (10.0, 25.0),
                (12.0, 25.0),
                (12.0, 30.0),
                (15.0, 30.0),
            ],
        }
    }
}

/// One kind of piece, sized.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofElementResult {
    pub element: RoofElement,
    /// Width × height chosen, cm (the largest tried when none passes).
    pub section_cm: (f64, f64),
    pub ok: bool,
    /// Horizontal span and spacing (or tributary width), metres.
    pub span_m: f64,
    pub spacing_m: f64,
    /// Characteristic load per metre of piece (g + q, horizontal), kN/m.
    pub load_kn_m: f64,
    pub bending_ratio: f64,
    pub shear_ratio: f64,
    pub deflection_ratio: f64,
    pub count: usize,
    pub total_length_m: f64,
    pub volume_m3: f64,
}

/// One end of a main beam bearing on the structure.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofReaction {
    pub point: Point,
    pub high: bool,
    /// Characteristic load, and its permanent part, kN.
    pub load_kn: f64,
    pub dead_kn: f64,
    /// What carries it ("P3", "V2"); `None` when nothing does.
    pub support: Option<String>,
}

/// A roof analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofResult {
    pub id: u64,
    pub label: String,
    pub slope_percent: f64,
    pub min_slope_percent: f64,
    pub surface_m2: f64,
    /// Loads on the horizontal projection, kN/m².
    pub dead_kn_m2: f64,
    pub live_kn_m2: f64,
    pub elements: Vec<RoofElementResult>,
    pub reactions: Vec<RoofReaction>,
    /// Tiles to buy (5 % extra), or `None` for sheets.
    pub tiles: Option<f64>,
    pub timber_m3: f64,
    /// Sheets spanning more than they can between purlins.
    pub sheet_span_exceeded: bool,
}

impl RoofResult {
    pub fn slope_ok(&self) -> bool {
        self.slope_percent + 1e-6 >= self.min_slope_percent
    }

    pub fn unsupported(&self) -> usize {
        self.reactions
            .iter()
            .filter(|r| r.support.is_none())
            .count()
    }
}

const LIVE_KN_M2: f64 = 0.25;
const FRAMING_KN_M2: f64 = 0.10;
const CEILING_KN_M2: f64 = 0.15;

/// A simply supported piece to size: `g` and `q` in kN per horizontal
/// metre (own weight added later), over the horizontal span `span`.
/// `inclined` pieces run down the slope; `normal` scales the load to the
/// component normal to the roof (battens lying on it).
#[derive(Clone, Copy)]
struct Piece {
    element: RoofElement,
    g: f64,
    q: f64,
    span: f64,
    spacing: f64,
    inclined: bool,
    normal: f64,
    count: usize,
    each_m: f64,
}

/// The lightest section of the catalog of the piece that passes bending,
/// shear and deflection: (section, ok, bending, shear, deflection ratios).
fn size_piece(piece: &Piece, timber: TimberClass, cos: f64) -> ((f64, f64), bool, f64, f64, f64) {
    let Piece {
        element,
        g,
        q,
        span,
        inclined,
        normal,
        ..
    } = *piece;
    let (fc0k, fv0k, e0m, density) = timber.properties();
    let fc0d = KMOD * fc0k / 1.4 * 1000.0; // kPa
    let fv0d = KMOD * fv0k / 1.8 * 1000.0;
    let e_ef = KMOD * e0m * 1000.0;
    let catalog = element.catalog();
    let mut last = (
        (0.0, 0.0),
        false,
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
    );
    for &(b_cm, h_cm) in catalog {
        let (b, h) = (b_cm / 100.0, h_cm / 100.0);
        let own = density * 9.81 / 1000.0 * b * h / if inclined { cos } else { 1.0 };
        let gk = g + own;
        let wd = 1.4 * (gk + q) * normal;
        let moment = wd * span * span / 8.0;
        let shear = wd * span / 2.0;
        let bending = moment / (b * h * h / 6.0) / fc0d;
        let shear_ratio = 1.5 * shear / (b * h) / fv0d;
        let ws = (gk + 0.2 * q) * normal;
        let inertia = b * h.powi(3) / 12.0;
        let factor = if inclined { cos * cos } else { 1.0 };
        let deflection = 5.0 * ws * span.powi(4) / (384.0 * e_ef * inertia * factor);
        let limit = span / if inclined { cos } else { 1.0 } / 200.0;
        let deflection_ratio = deflection / limit;
        let ok =
            bending <= 1.0 + 1e-9 && shear_ratio <= 1.0 + 1e-9 && deflection_ratio <= 1.0 + 1e-9;
        last = ((b_cm, h_cm), ok, bending, shear_ratio, deflection_ratio);
        if ok {
            break;
        }
    }
    last
}

/// Main-beam positions along the eaves: the ends, the supports found, and
/// more where a gap is wider than `spacing`.
pub fn beam_positions(eaves: f64, spacing: f64, anchors: &[f64]) -> Vec<f64> {
    let mut fixed: Vec<f64> = vec![0.0, eaves];
    fixed.extend(
        anchors
            .iter()
            .copied()
            .filter(|a| *a > 0.15 && *a < eaves - 0.15),
    );
    fixed.sort_by(f64::total_cmp);
    fixed.dedup_by(|a, b| (*a - *b).abs() < 0.15);
    let mut out = Vec::new();
    for pair in fixed.windows(2) {
        let gap = pair[1] - pair[0];
        let n = ((gap / spacing.max(0.5)) - 1e-9).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push(pair[0] + gap * k as f64 / n as f64);
        }
    }
    out.push(eaves);
    out
}

impl Project {
    /// Adds a roof, returning its id.
    pub fn add_roof(&mut self, mut roof: Roof) -> Result<u64, String> {
        roof.check()?;
        roof.id = self.id();
        self.structure.roofs.push(roof);
        Ok(roof.id)
    }

    /// Replaces roof `id` with `roof` (its id kept).
    pub fn update_roof(&mut self, id: u64, mut roof: Roof) -> Result<(), String> {
        roof.check()?;
        roof.id = id;
        let slot = self
            .structure
            .roofs
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Telhado não encontrado.")?;
        *slot = roof;
        Ok(())
    }

    pub fn remove_roof(&mut self, id: u64) -> bool {
        let before = self.structure.roofs.len();
        self.structure.roofs.retain(|r| r.id != id);
        self.structure.roofs.len() != before
    }

    /// Gives the walls under roof `id` the roof's height at their ends
    /// (sloped tops for the gables). Returns how many changed.
    pub fn fit_walls_to_roof(&mut self, id: u64) -> Result<usize, String> {
        let roof = *self
            .structure
            .roofs
            .iter()
            .find(|r| r.id == id)
            .ok_or("Telhado não encontrado.")?;
        let inside = |p: Point| {
            p.x >= roof.min.x - 0.3
                && p.x <= roof.max.x + 0.3
                && p.y >= roof.min.y - 0.3
                && p.y <= roof.max.y + 0.3
        };
        let walls: Vec<(u64, Point, Point)> = self
            .walls
            .iter()
            .filter(|w| inside(w.start) && inside(w.end))
            .map(|w| (w.id, w.start, w.end))
            .collect();
        let mut changed = 0;
        for (wall, start, end) in walls {
            let (a, b) = (roof.height_at(start), roof.height_at(end));
            if self.set_wall_height(wall, Some(a), Some(b)).is_ok() {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The labels of the roofs, T1, T2… in reading order.
    fn roof_label(&self, id: u64) -> String {
        let mut ids: Vec<(u64, Point)> =
            self.structure.roofs.iter().map(|r| (r.id, r.min)).collect();
        ids.sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then(a.1.x.total_cmp(&b.1.x)));
        let n = ids.iter().position(|(r, _)| *r == id).map_or(0, |i| i + 1);
        format!("T{n}")
    }

    /// Sizes the framing of `roof` and finds where its beams bear (the
    /// supports are matched by the structure report).
    pub fn analyse_roof(&self, roof: &Roof) -> RoofResult {
        let cos = roof.cos();
        let slope = roof.slope();
        let run = roof.run_m();
        let eaves = roof.eaves_m();
        let tile = roof.tile;
        let ceiling = if roof.exposed { 0.0 } else { CEILING_KN_M2 };
        let surface = (tile.weight_kn_m2() + FRAMING_KN_M2) / cos;
        let dead = surface + ceiling;
        let mut elements = Vec::new();
        let mut sheet_span_exceeded = false;
        let slope_length = run / cos;
        let sized = |piece: Piece| {
            let (section, ok, bending, shear, deflection) = size_piece(&piece, roof.timber, cos);
            let total = piece.count as f64 * piece.each_m;
            RoofElementResult {
                element: piece.element,
                section_cm: section,
                ok,
                span_m: piece.span,
                spacing_m: piece.spacing,
                load_kn_m: piece.g + piece.q,
                bending_ratio: bending,
                shear_ratio: shear,
                deflection_ratio: deflection,
                count: piece.count,
                total_length_m: total,
                volume_m3: total * section.0 * section.1 / 10_000.0,
            }
        };
        let purlin_spacing = match tile.sheet_span_m() {
            Some(max) => {
                sheet_span_exceeded = roof.purlin_spacing_m / cos > max + 1e-9;
                roof.purlin_spacing_m
            }
            None => roof.purlin_spacing_m,
        };
        if let Some(gauge) = tile.gauge_m() {
            // Battens: tiles over the gauge, span = rafter spacing.
            let rows = (slope_length / gauge).ceil() as usize + 1;
            elements.push(sized(Piece {
                element: RoofElement::Battens,
                g: tile.weight_kn_m2() * gauge,
                q: LIVE_KN_M2 * gauge * cos,
                span: roof.rafter_spacing_m,
                spacing: gauge,
                inclined: false,
                normal: cos,
                count: rows,
                each_m: eaves,
            }));
            // Rafters: down the slope, span = purlin spacing.
            let count = (eaves / roof.rafter_spacing_m).floor() as usize + 1;
            elements.push(sized(Piece {
                element: RoofElement::Rafters,
                g: (surface + ceiling) * roof.rafter_spacing_m,
                q: LIVE_KN_M2 * roof.rafter_spacing_m,
                span: purlin_spacing,
                spacing: roof.rafter_spacing_m,
                inclined: true,
                normal: 1.0,
                count,
                each_m: slope_length,
            }));
        }
        let anchors: Vec<f64> = self
            .structural_columns()
            .filter(|c| roof.on_edge(c.center, 0.35))
            .map(|c| roof.along(c.center))
            .collect();
        let positions = beam_positions(eaves, roof.beam_spacing_m, &anchors);
        let gaps: Vec<f64> = positions.windows(2).map(|p| p[1] - p[0]).collect();
        let widest = gaps.iter().copied().fold(0.0, f64::max);
        // Purlins: along the eaves, span = the widest gap between beams.
        let purlin_rows = (run / purlin_spacing).ceil() as usize + 1;
        let purlins = sized(Piece {
            element: RoofElement::Purlins,
            g: dead * purlin_spacing,
            q: LIVE_KN_M2 * purlin_spacing,
            span: widest,
            spacing: purlin_spacing,
            inclined: false,
            normal: 1.0,
            count: purlin_rows,
            each_m: eaves,
        });
        let purlin = purlins.section_cm;
        elements.push(purlins);
        let (_, _, _, density) = roof.timber.properties();
        let purlins_kn_m2 =
            density * 9.81 / 1000.0 * purlin.0 * purlin.1 / 10_000.0 / purlin_spacing;
        // Main beams: down the slope, carrying the widest tributary.
        let tributary = |i: usize| {
            let left = if i > 0 { gaps[i - 1] / 2.0 } else { 0.0 };
            let right = gaps.get(i).map_or(0.0, |g| g / 2.0);
            left + right
        };
        let widest_trib = (0..positions.len()).map(tributary).fold(0.0, f64::max);
        let beams = sized(Piece {
            element: RoofElement::Beams,
            g: (dead + purlins_kn_m2) * widest_trib,
            q: LIVE_KN_M2 * widest_trib,
            span: run,
            spacing: widest_trib,
            inclined: true,
            normal: 1.0,
            count: positions.len(),
            each_m: slope_length,
        });
        let beam = beams.section_cm;
        elements.push(beams);
        let beam_own = density * 9.81 / 1000.0 * beam.0 * beam.1 / 10_000.0 / cos;
        let mut reactions = Vec::new();
        for (i, along) in positions.iter().enumerate() {
            let t = tributary(i);
            let g = (dead + purlins_kn_m2) * t + beam_own;
            let q = LIVE_KN_M2 * t;
            for high in [true, false] {
                reactions.push(RoofReaction {
                    point: roof.edge_point(*along, high),
                    high,
                    load_kn: (g + q) * run / 2.0,
                    dead_kn: g * run / 2.0,
                    support: None,
                });
            }
        }
        let surface_m2 = slope_length * eaves;
        let timber_m3 = elements.iter().map(|e| e.volume_m3).sum();
        RoofResult {
            id: roof.id,
            label: self.roof_label(roof.id),
            slope_percent: slope * 100.0,
            min_slope_percent: tile.min_slope_percent(),
            surface_m2,
            dead_kn_m2: dead + purlins_kn_m2,
            live_kn_m2: LIVE_KN_M2,
            elements,
            reactions,
            tiles: tile.per_m2().map(|n| (surface_m2 * n * 1.05).ceil()),
            timber_m3,
            sheet_span_exceeded,
        }
    }

    /// The roof outlines, main beams and labels drawn on the plan.
    pub fn roof_marks(&self, results: &[RoofResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(roof) = self.structure.roofs.iter().find(|r| r.id == result.id) else {
                continue;
            };
            let (a, c) = (roof.min, roof.max);
            marks.push(PlanMark::Line {
                points: vec![a, Point::new(c.x, a.y), c, Point::new(a.x, c.y), a],
                dashed: true,
                heavy: false,
            });
            for pair in result.reactions.chunks(2) {
                if let [high, low] = pair {
                    marks.push(PlanMark::Line {
                        points: vec![high.point, low.point],
                        dashed: true,
                        heavy: false,
                    });
                }
            }
            let beams = result
                .elements
                .iter()
                .find(|e| e.element == RoofElement::Beams)
                .map_or(String::new(), |e| {
                    format!(" · vigas {:.0}×{:.0}", e.section_cm.0, e.section_cm.1)
                });
            marks.push(PlanMark::Text {
                anchor: a.lerp(c, 0.5),
                text: format!(
                    "{} meia-água {:.0} % → {}{}",
                    result.label,
                    result.slope_percent,
                    roof.high_side.opposite().label(),
                    beams
                ),
            });
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    fn gourmet_roof() -> Roof {
        // 5.00 × 14.75 m, high on the west at 4.30 m, low on the east at 2.60 m.
        Roof::new(
            Point::new(0.0, 0.0),
            Point::new(5.0, 14.75),
            RoofSide::West,
            4.30,
            2.60,
        )
    }

    #[test]
    fn the_slope_and_heights_follow_the_high_side() {
        let roof = gourmet_roof();
        assert!((roof.slope() - 0.34).abs() < 1e-9);
        assert!((roof.height_at(Point::new(0.0, 3.0)) - 4.30).abs() < 1e-9);
        assert!((roof.height_at(Point::new(5.0, 3.0)) - 2.60).abs() < 1e-9);
        assert!((roof.height_at(Point::new(2.5, 3.0)) - 3.45).abs() < 1e-9);
        let mut flat = roof;
        flat.low_m = 4.30;
        assert!(flat.check().is_err());
    }

    #[test]
    fn beams_line_up_with_the_columns_and_fill_wide_gaps() {
        let positions = beam_positions(14.75, 3.0, &[3.0, 6.0, 9.0, 12.0]);
        assert_eq!(positions, vec![0.0, 3.0, 6.0, 9.0, 12.0, 14.75]);
        let filled = beam_positions(10.0, 3.0, &[]);
        assert_eq!(filled.len(), 5); // 4 gaps of 2.5 m
    }

    #[test]
    fn a_beam_matches_the_hand_calculation() {
        // C30, 1.4 (g + q) = 1.4 × 3 kN/m over 4 m horizontal (flat piece).
        let piece = Piece {
            element: RoofElement::Purlins,
            g: 2.0,
            q: 1.0,
            span: 4.0,
            spacing: 1.0,
            inclined: false,
            normal: 1.0,
            count: 1,
            each_m: 4.0,
        };
        let ((b, h), ok, bending, _, _) = size_piece(&piece, TimberClass::C30, 1.0);
        assert!(ok);
        let (b_m, h_m) = (b / 100.0, h / 100.0);
        let own = 800.0 * 9.81 / 1000.0 * b_m * h_m;
        let md = 1.4 * (3.0 + own) * 16.0 / 8.0;
        let fc0d = KMOD * 30.0 / 1.4 * 1000.0;
        assert!((bending - md / (b_m * h_m * h_m / 6.0) / fc0d).abs() < 1e-9);
    }

    #[test]
    fn the_gourmet_roof_is_framed_and_bears_on_the_columns() {
        let mut project = Project::default();
        let mut columns = Vec::new();
        for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
            columns.push(
                project
                    .add_fixture(FixtureKind::Column, Point::new(0.07, y), 0.14, 0.30, 0)
                    .unwrap(),
            );
        }
        let id = project.add_roof(gourmet_roof()).unwrap();
        let roof = project.structure.roofs[0];
        let result = project.analyse_roof(&roof);
        assert_eq!(result.label, "T1");
        assert!(result.slope_ok());
        assert_eq!(result.elements.len(), 4);
        assert!(
            result.elements.iter().all(|e| e.ok),
            "{:#?}",
            result.elements
        );
        // Six beams (one per column), both ends.
        assert_eq!(result.reactions.len(), 12);
        let total: f64 = result.reactions.iter().map(|r| r.load_kn).sum();
        let expected = (result.dead_kn_m2 + result.live_kn_m2) * 5.0 * 14.75;
        // Beam own weight on top of the area loads.
        assert!(
            total > expected && total < expected * 1.15,
            "{total} vs {expected}"
        );
        assert!(result.tiles.unwrap() > 1000.0);
        // The structure report bears the high ends on the columns.
        let report = project.structure_report();
        let roof_result = &report.roofs[0];
        assert_eq!(
            roof_result
                .reactions
                .iter()
                .filter(|r| r.high && r.support.is_some())
                .count(),
            6
        );
        // The low edge has nothing under it yet (posts, #146).
        assert_eq!(roof_result.unsupported(), 6);
        let column = report.columns.iter().find(|c| c.id == columns[1]).unwrap();
        let high = roof_result
            .reactions
            .iter()
            .find(|r| r.high && (r.point.y - 3.0).abs() < 1e-6)
            .unwrap();
        assert!(
            column.load_kn >= high.load_kn,
            "{} vs {}",
            column.load_kn,
            high.load_kn
        );
        // Walls under the roof take its slope.
        project
            .add_wall(Point::new(0.0, 14.75), Point::new(5.0, 14.75))
            .unwrap();
        assert_eq!(project.fit_walls_to_roof(id).unwrap(), 1);
        let gable = project.walls.last().unwrap();
        assert_eq!((gable.height_m, gable.height_end_m), (Some(4.3), Some(2.6)));
    }
}
