//! Structure on the floor plan: columns (*pilares*) and footings (*sapatas*)
//! as fixtures, beams (*vigas*) between columns and slabs (*lajes*) over the
//! rooms, drawn as a formwork plan (*planta de forma*) on a layer of their
//! own (#92); pre-sizing by the usual rules of thumb, with warnings (#93);
//! and the analysis of each continuous beam: loads, bending moments, shear,
//! reactions and deflection (#94).
//!
//! Columns and footings are fixtures, so they select, move, copy, rotate,
//! undo and export like any other object. Beams name the two columns they
//! join, so they follow them, and slabs name their room.
//!
//! Everything here is pre-sizing for a first layout: the final design is a
//! structural engineer's job (NBR 6118).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::conduits::PlanMark;
use super::fixtures::{Fixture, FixtureKind, SymbolShape, Weight};
use super::geometry::Point;
use super::model::Project;

/// Reinforced concrete unit weight, kN/m³.
pub const CONCRETE_KN_M3: f64 = 25.0;

/// Support specification for one end of a beam: either a column fixture id or
/// a point along another beam (`beam_id`, parameter `t` in `0.0..=1.0`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BeamSupport {
    Column { id: u64 },
    Beam { beam_id: u64, t: f64 },
}

impl BeamSupport {
    pub fn target_id(self) -> u64 {
        match self {
            Self::Column { id } => id,
            Self::Beam { beam_id, .. } => beam_id,
        }
    }
}

/// A beam between two supports (columns or other beams).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub id: u64,
    pub from: u64,
    pub to: u64,
    /// Optional explicit start support (defaults to `BeamSupport::Column { id: from }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_support: Option<BeamSupport>,
    /// Optional explicit end support (defaults to `BeamSupport::Column { id: to }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_support: Option<BeamSupport>,
    /// Section width (b), metres.
    pub width_m: f64,
    /// Section height (h), metres.
    pub height_m: f64,
}

impl Beam {
    pub fn start_support(&self) -> BeamSupport {
        self.from_support
            .unwrap_or(BeamSupport::Column { id: self.from })
    }

    pub fn end_support(&self) -> BeamSupport {
        self.to_support
            .unwrap_or(BeamSupport::Column { id: self.to })
    }
}

/// How a slab is built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlabKind {
    /// Precast lattice joists with blocks (*laje treliçada*).
    #[default]
    Ribbed,
    /// Solid cast-in-place slab (*laje maciça*).
    Solid,
}

impl SlabKind {
    pub const ALL: [SlabKind; 2] = [SlabKind::Ribbed, SlabKind::Solid];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ribbed => "Treliçada",
            Self::Solid => "Maciça",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ribbed => "trelicada",
            Self::Solid => "macica",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "trelicada" | "treliçada" | "nervurada" => Some(Self::Ribbed),
            "macica" | "maciça" => Some(Self::Solid),
            _ => None,
        }
    }

    /// Self weight of a slab of thickness `h`, kN/m² (a lattice slab with
    /// fillers weighs about 55 % of a solid one).
    pub fn self_weight(self, h: f64) -> f64 {
        match self {
            Self::Ribbed => CONCRETE_KN_M3 * h * 0.55,
            Self::Solid => CONCRETE_KN_M3 * h,
        }
    }

    /// The usual minimum thickness for the smaller span `lx`, metres.
    pub fn minimum_thickness(self, lx: f64) -> f64 {
        match self {
            Self::Solid => round_up(lx / 40.0, 0.01).max(0.08),
            Self::Ribbed => {
                if lx <= 3.5 {
                    0.12
                } else if lx <= 4.5 {
                    0.16
                } else if lx <= 5.5 {
                    0.20
                } else if lx <= 6.5 {
                    0.25
                } else {
                    0.30
                }
            }
        }
    }
}

/// A slab over a room.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slab {
    pub id: u64,
    pub room_id: u64,
    #[serde(default)]
    pub kind: SlabKind,
    pub thickness_m: f64,
}

/// Typical Brazilian soil classes (NBR 6122 / SPT correlation) with
/// indicative allowable bearing pressures. Always confirm with a local SPT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilClass {
    /// Soft clay / loose sand (N_SPT ~ 3–5): 80 kPa.
    SoftClay,
    /// Medium clay / silty sand (N_SPT ~ 6–10): 150 kPa.
    MediumClay,
    /// Medium-dense sand / stiff clay (N_SPT ~ 10–15): 200 kPa.
    MediumSand,
    /// Dense sand / very stiff clay (N_SPT ~ 15–25): 300 kPa.
    DenseSand,
    /// Weathered rock / saprolite: 500 kPa.
    Rock,
}

impl SoilClass {
    pub const ALL: [SoilClass; 5] = [
        SoilClass::SoftClay,
        SoilClass::MediumClay,
        SoilClass::MediumSand,
        SoilClass::DenseSand,
        SoilClass::Rock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SoftClay => "Argila mole (80 kPa)",
            Self::MediumClay => "Argila média (150 kPa)",
            Self::MediumSand => "Areia média (200 kPa)",
            Self::DenseSand => "Areia compacta (300 kPa)",
            Self::Rock => "Rocha / saprolito (500 kPa)",
        }
    }

    pub fn allowable_kpa(self) -> f64 {
        match self {
            Self::SoftClay => 80.0,
            Self::MediumClay => 150.0,
            Self::MediumSand => 200.0,
            Self::DenseSand => 300.0,
            Self::Rock => 500.0,
        }
    }
}

/// Typical building occupancies with their NBR 6120:2019 base live load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OccupancyClass {
    Residential,
    Office,
    Garage,
    Retail,
}

impl OccupancyClass {
    pub const ALL: [OccupancyClass; 4] = [
        OccupancyClass::Residential,
        OccupancyClass::Office,
        OccupancyClass::Garage,
        OccupancyClass::Retail,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Residential => "Residencial (1,5 kN/m²)",
            Self::Office => "Escritório (2,0 kN/m²)",
            Self::Garage => "Garagem leve (3,0 kN/m²)",
            Self::Retail => "Comércio / loja (4,0 kN/m²)",
        }
    }

    pub fn live_kn_m2(self) -> f64 {
        match self {
            Self::Residential => 1.5,
            Self::Office => 2.0,
            Self::Garage => 3.0,
            Self::Retail => 4.0,
        }
    }
}

/// NBR 6120:2019 minimum live load for a specific room, never below `base_live`.
pub fn room_live_load_kn_m2(room_name: &str, base_live: f64) -> f64 {
    let lower = room_name.to_lowercase();
    let room_min = if lower.contains("garagem") || lower.contains("estacionamento") {
        3.0
    } else if lower.contains("escada")
        || lower.contains("varanda")
        || lower.contains("sacada")
        || lower.contains("terraço")
        || lower.contains("terraco")
    {
        2.5
    } else if lower.contains("corredor")
        || lower.contains("circulação")
        || lower.contains("circulacao")
        || lower.contains("hall")
        || lower.contains("serviço")
        || lower.contains("servico")
        || lower.contains("lavanderia")
    {
        2.0
    } else {
        1.5
    };
    base_live.max(room_min)
}

/// Materials and loads for pre-sizing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StructureSettings {
    /// Concrete strength, MPa.
    pub fck_mpa: f64,
    /// Allowable soil pressure, kPa (200 kPa = 0.2 MPa).
    pub soil_kpa: f64,
    /// Floors the columns carry (each with its slab, beams and walls).
    pub floors: u32,
    /// Live load on the slabs, kN/m² (NBR 6120: 1.5 for dwellings).
    pub live_kn_m2: f64,
    /// Floor finishes and screed, kN/m².
    pub finish_kn_m2: f64,
    /// Masonry unit weight, kN/m³ (about 13 for hollow ceramic blocks).
    pub masonry_kn_m3: f64,
    /// Apply NBR 6120 per-room minimum live loads (e.g. balconies 2.5, garages 3.0 kN/m²).
    pub per_room_live_loads: bool,
}

impl Default for StructureSettings {
    fn default() -> Self {
        Self {
            fck_mpa: 25.0,
            soil_kpa: 200.0,
            floors: 1,
            live_kn_m2: 1.5,
            finish_kn_m2: 1.0,
            masonry_kn_m3: 13.0,
            per_room_live_loads: false,
        }
    }
}

impl StructureSettings {
    pub fn apply_soil_class(&mut self, soil: SoilClass) {
        self.soil_kpa = soil.allowable_kpa();
    }

    pub fn apply_occupancy(&mut self, occupancy: OccupancyClass) {
        self.live_kn_m2 = occupancy.live_kn_m2();
    }
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Secant modulus of the concrete, kN/m² (NBR 6118: 0.85 · 5600 √fck).
    pub fn modulus_kn_m2(&self) -> f64 {
        0.85 * 5600.0 * self.fck_mpa.max(10.0).sqrt() * 1000.0
    }
}

/// The structural elements that are not fixtures, and their settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectStructure {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<Beam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slabs: Vec<Slab>,
    #[serde(default, skip_serializing_if = "StructureSettings::is_default")]
    pub settings: StructureSettings,
    /// Balance beams from boundary columns to interior ones (#144).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balance_beams: Vec<super::foundations::BalanceBeam>,
    /// Ground beams and strip footings under the walls (#143).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wall_foundations: Vec<super::wall_foundations::WallFoundation>,
    /// The site's soil survey (SPT), shared by every floor (#112).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soil_survey: Option<super::soil::SoilSurvey>,
    /// Timber roofs over this floor (#114).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roofs: Vec<super::roof::Roof>,
    /// Strength class of the timber posts (#146).
    #[serde(default, skip_serializing_if = "is_default_timber")]
    pub post_timber: super::roof::TimberClass,
}

fn is_default_timber(class: &super::roof::TimberClass) -> bool {
    *class == super::roof::TimberClass::default()
}

impl ProjectStructure {
    pub fn is_empty(&self) -> bool {
        self.beams.is_empty()
            && self.slabs.is_empty()
            && self.settings.is_default()
            && self.balance_beams.is_empty()
            && self.wall_foundations.is_empty()
            && self.soil_survey.is_none()
            && self.roofs.is_empty()
            && is_default_timber(&self.post_timber)
    }
}

fn round_up(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

/// "14×30" from metres.
pub fn section_cm(b: f64, h: f64) -> String {
    format!("{:.0}×{:.0}", b * 100.0, h * 100.0)
}

/// Reading order on the plan: top to bottom, then left to right.
fn reading(a: Point, b: Point) -> Ordering {
    let row = |p: Point| (p.y / 0.3).round() as i64;
    row(a).cmp(&row(b)).then(a.x.total_cmp(&b.x))
}

impl FixtureKind {
    pub const STRUCTURAL: [FixtureKind; 5] = [
        FixtureKind::Column,
        FixtureKind::Footing,
        FixtureKind::WaterTank,
        FixtureKind::HeavyEquipment,
        FixtureKind::TimberPost,
    ];

    pub fn is_structural(self) -> bool {
        matches!(
            self,
            Self::Column
                | Self::Footing
                | Self::WaterTank
                | Self::HeavyEquipment
                | Self::TimberPost
        )
    }

    /// Default characteristic concentrated load. A 1,000 L water tank uses
    /// 9.81 kN of water plus a conservative 1.0 kN for tank and fittings.
    pub fn default_structural_load_kn(self) -> Option<f64> {
        match self {
            Self::WaterTank => Some(10.81),
            Self::HeavyEquipment => Some(15.0),
            _ => None,
        }
    }
}

/// The plan symbol of a column (cut, so hatched) or a footing (below the
/// floor, so dashed, with the edges of its sloped top).
pub(super) fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let corners = vec![
        Point::new(-hw, -hd),
        Point::new(hw, -hd),
        Point::new(hw, hd),
        Point::new(-hw, hd),
    ];
    match kind {
        FixtureKind::Column => {
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            }];
            // 45° hatching clipped to the section.
            let step = 0.035;
            let mut c = -hw - hd + step;
            while c < hw + hd {
                // Points with x - y = c inside the box.
                let x0 = (-hw).max(c - hd);
                let x1 = hw.min(c + hd);
                if x1 > x0 + 1e-6 {
                    shapes.push(SymbolShape::Polyline {
                        points: vec![Point::new(x0, x0 - c), Point::new(x1, x1 - c)],
                        closed: false,
                        dashed: false,
                        weight: Weight::Detail,
                    });
                }
                c += step;
            }
            shapes
        }
        FixtureKind::Footing => {
            let inner = 0.12_f64.min(hw * 0.5).min(hd * 0.5);
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners.clone(),
                closed: true,
                dashed: true,
                weight: Weight::Outline,
            }];
            for corner in corners {
                let toward = Point::new(corner.x.signum() * inner, corner.y.signum() * inner);
                shapes.push(SymbolShape::Polyline {
                    points: vec![corner, toward],
                    closed: false,
                    dashed: true,
                    weight: Weight::Detail,
                });
            }
            shapes
        }
        // A round post: its section and a growth ring; a square one, its
        // section crossed.
        FixtureKind::TimberPost if (width - depth).abs() < 1e-6 => vec![
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw,
                weight: Weight::Outline,
            },
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw * 0.55,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::TimberPost => vec![
            SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw, -hd), Point::new(hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::WaterTank => vec![
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw.min(hd),
                weight: Weight::Outline,
            },
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw.min(hd) * 0.72,
                weight: Weight::Detail,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw * 0.65, 0.0), Point::new(hw * 0.65, 0.0)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::HeavyEquipment => vec![
            SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw, -hd), Point::new(hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(hw, -hd), Point::new(-hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        _ => Vec::new(),
    }
}

/// A run of collinear beams through consecutive columns: one continuous
/// beam, labelled once.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamLine {
    pub label: String,
    /// Beam ids, span by span.
    pub spans: Vec<u64>,
    /// Support ids (column or supporting beam id), in order (one more than the spans).
    pub joints: Vec<u64>,
    /// Detailed support specs matching `joints` 1:1.
    pub supports: Vec<BeamSupport>,
}

/// One span of a continuous beam, analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanResult {
    pub beam: u64,
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    /// Characteristic permanent and live load, kN/m.
    pub dead_kn_m: f64,
    pub live_kn_m: f64,
    /// Bending moments (sagging positive), kN·m: at each end and the
    /// largest along the span.
    pub moment_left: f64,
    pub moment_right: f64,
    pub moment_span: f64,
    /// Largest shear, kN.
    pub shear: f64,
    /// Long-term deflection and the L/250 limit, mm.
    pub deflection_mm: f64,
    pub limit_mm: f64,
    /// The usual minimum height for this span (L/10 at an end, L/12
    /// inside a continuous beam), metres.
    pub height_min_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamLineResult {
    pub label: String,
    pub joints: Vec<u64>,
    pub spans: Vec<SpanResult>,
    /// Support reactions, kN, one per joint.
    pub reactions: Vec<f64>,
}

/// Where a column stands in the frame, for the pre-sizing factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnPosition {
    Interior,
    Edge,
    Corner,
    /// No beam reaches it.
    Isolated,
}

impl ColumnPosition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Interior => "Intermediário",
            Self::Edge => "Extremidade",
            Self::Corner => "Canto",
            Self::Isolated => "Isolado",
        }
    }

    /// Pre-sizing factor for the bending a column takes at its position.
    pub fn factor(self) -> f64 {
        match self {
            Self::Interior => 1.3,
            Self::Edge => 1.5,
            Self::Corner | Self::Isolated => 1.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnResult {
    pub id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub position: ColumnPosition,
    /// Characteristic axial load at the foundation (including real upper floors), kN.
    pub load_kn: f64,
    /// Axial load arriving from stacked columns on real upper floors above, kN.
    pub upper_load_kn: f64,
    /// Concrete area needed and drawn, cm².
    pub area_needed_cm2: f64,
    pub area_cm2: f64,
    /// Free height: the tallest wall it stands in, or the storey (#145), m.
    pub height_m: f64,
    /// Slenderness, 3,46 · height / smallest side.
    pub slenderness: f64,
}

impl ColumnResult {
    pub fn ok(&self) -> bool {
        self.area_cm2 + 1e-6 >= self.area_needed_cm2.max(360.0)
            && self.width_m.min(self.depth_m) >= 0.14 - 1e-9
            && self.slender_ok()
    }

    /// Within the simplified methods (slenderness up to 90).
    pub fn slender_ok(&self) -> bool {
        self.slenderness <= super::wall_heights::MAX_SIMPLE_SLENDERNESS + 1e-9
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootingResult {
    pub id: u64,
    pub label: String,
    pub column: Option<String>,
    pub width_m: f64,
    pub depth_m: f64,
    pub load_kn: f64,
    /// Plan size needed for the soil (along the fixture's width and depth).
    pub needed_width_m: f64,
    pub needed_depth_m: f64,
    /// Height for a rigid footing, metres.
    pub height_m: f64,
    /// Column-to-footing centre eccentricity, metres.
    pub eccentricity_m: f64,
    /// Peak soil bearing pressure accounting for eccentricity (NBR 6122), kPa.
    pub max_pressure_kpa: f64,
    /// Allowable soil bearing pressure used in the check, kPa.
    pub soil_kpa: f64,
    /// Punching shear utilisation ratio (NBR 6118 19.5): <= 1.0 passes.
    pub punching_ratio: f64,
    /// Overturning safety factor (NBR 6122): >= 1.5 required (`INFINITY` when concentric).
    pub overturning_fs: f64,
}

impl FootingResult {
    pub fn ok(&self) -> bool {
        self.width_m * self.depth_m + 1e-6 >= self.needed_width_m * self.needed_depth_m
            && self.max_pressure_kpa <= self.soil_kpa + 1e-3
            && self.punching_ratio <= 1.0 + 1e-6
            && self.overturning_fs >= 1.5 - 1e-6
    }
}

/// Mechanical checks for a spread footing per NBR 6122 (eccentric bearing
/// pressure & overturning) and NBR 6118 19.5 (punching shear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootingCheck {
    pub max_pressure_kpa: f64,
    pub outside_middle_third: bool,
    pub overturning_fs: f64,
    pub punching_ratio: f64,
}

/// Evaluates eccentric soil pressure, overturning safety factor and punching
/// shear utilisation for a rectangular footing (`bx` × `by`, height `h`)
/// carrying a column (`cw` × `cd`) with characteristic axial load `load_kn`
/// offset by `(ex, ey)` from the footing centre.
#[allow(clippy::too_many_arguments)]
pub fn check_footing(
    load_kn: f64,
    bx: f64,
    by: f64,
    h: f64,
    cw: f64,
    cd: f64,
    ex: f64,
    ey: f64,
    fck_mpa: f64,
) -> FootingCheck {
    let bx = bx.max(0.1);
    let by = by.max(0.1);
    let ex = ex.abs();
    let ey = ey.abs();
    let n_soil = 1.05 * load_kn.max(0.0);

    // Eccentricity & overturning (NBR 6122).
    let kern_ratio = 6.0 * ex / bx + 6.0 * ey / by;
    let outside_middle_third = kern_ratio > 1.0 + 1e-6;
    let overturning_fs = if ex <= 1e-6 && ey <= 1e-6 {
        f64::INFINITY
    } else {
        1.0 / (2.0 * (ex / bx + ey / by)).max(1e-9)
    };
    let max_pressure_kpa = if !outside_middle_third {
        (n_soil / (bx * by)) * (1.0 + kern_ratio)
    } else {
        let bx_eff = (bx - 2.0 * ex).max(0.01);
        let by_eff = (by - 2.0 * ey).max(0.01);
        n_soil / (bx_eff * by_eff)
    };

    // Punching shear (NBR 6118 19.5 & 22.6): column face τ_Rd2 and d/2 contour τ_Rd1.
    let fck = fck_mpa.max(10.0);
    let d = (h - 0.05).max(0.10);
    let f_sd = 1.4 * load_kn.max(0.0);
    let fcd_kn_m2 = (fck / 1.4) * 1000.0;
    let alpha_v2 = (1.0 - fck / 250.0).max(0.1);
    let tau_rd2 = 0.27 * alpha_v2 * fcd_kn_m2;
    let u0 = (2.0 * (cw.max(0.10) + cd.max(0.10))).max(0.40);
    let tau_sd0 = f_sd / (u0 * d);

    let px = (cw.max(0.10) + d).min(bx);
    let py = (cd.max(0.10) + d).min(by);
    let area_inside = px * py;
    let v_sd = (f_sd * (1.0 - area_inside / (bx * by))).max(0.0);
    let u1 = 2.0 * (px + py);
    let tau_sd1 = v_sd / (u1 * d);
    let k_size = (1.0 + (0.20 / d).sqrt()).min(2.0);
    let rho_flex = 0.002;
    let tau_rd1 = 0.13 * k_size * (100.0 * rho_flex * fck).cbrt() * 1000.0;
    let punching_ratio = (tau_sd0 / tau_rd2).max(tau_sd1 / tau_rd1);

    FootingCheck {
        max_pressure_kpa,
        outside_middle_third,
        overturning_fs,
        punching_ratio,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlabResult {
    pub id: u64,
    pub label: String,
    pub room: String,
    pub kind: SlabKind,
    pub lx_m: f64,
    pub ly_m: f64,
    pub thickness_m: f64,
    pub thickness_min_m: f64,
    /// Total characteristic load, kN/m².
    pub load_kn_m2: f64,
    /// Sum of concentrated characteristic loads supported by this slab, kN.
    pub point_load_kn: f64,
    /// Additional conservative one-metre-strip moment from concentrated loads, kN·m/m.
    pub point_moment_knm_m: f64,
    /// Whether some beam carries it.
    pub supported: bool,
}

impl SlabResult {
    pub fn ok(&self) -> bool {
        self.thickness_m + 1e-6 >= self.thickness_min_m && self.supported
    }
}

/// An upper-floor column with no aligned column directly underneath it.
#[derive(Clone, Debug, PartialEq)]
pub struct MisalignedColumn {
    pub floor_name: String,
    pub column_label: String,
    pub center: Point,
    pub load_kn: f64,
    /// Label of the lower-floor beam acting as a transfer girder, or `None`
    /// if the column drops onto open space / slab without a beam beneath it.
    pub transfer_beam: Option<String>,
}

/// A water tank or heavy item and the structural element that receives it.
#[derive(Clone, Debug, PartialEq)]
pub struct ConcentratedLoadResult {
    pub id: u64,
    pub label: String,
    pub kind: FixtureKind,
    pub center: Point,
    pub load_kn: f64,
    pub support: String,
    pub slab: Option<String>,
    pub local_moment_knm_m: f64,
    /// Punching utilisation around the loaded footprint; `None` when the
    /// load is directly on a beam or the slab system needs manufacturer data.
    pub punching_ratio: Option<f64>,
    pub supported: bool,
}

/// The pre-sizing of the whole structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructureReport {
    pub slabs: Vec<SlabResult>,
    pub beams: Vec<BeamLineResult>,
    pub columns: Vec<ColumnResult>,
    pub footings: Vec<FootingResult>,
    pub concentrated_loads: Vec<ConcentratedLoadResult>,
    pub misaligned_columns: Vec<MisalignedColumn>,
    /// Balance beams centring the boundary footings (#144).
    pub balance_beams: Vec<super::foundations::BalanceBeamResult>,
    /// Ground beams and strip footings under the walls (#143).
    pub wall_foundations: Vec<super::wall_foundations::WallFoundationResult>,
    /// Timber roofs and where they bear (#114).
    pub roofs: Vec<super::roof::RoofResult>,
    /// Timber posts and the timber beams over them (#146).
    pub timber_posts: Vec<super::timber_posts::TimberPostResult>,
    pub eave_beams: Vec<super::timber_posts::EaveBeamResult>,
    pub warnings: Vec<String>,
}

/// What generating the structure added.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructureGeneration {
    pub columns: usize,
    pub beams: usize,
    pub footings: usize,
    pub slabs: usize,
    /// Columns that could not be placed (outside the lot even when moved
    /// inside it), so the caller can say so instead of staying silent.
    pub skipped: usize,
}

impl StructureGeneration {
    pub fn total(&self) -> usize {
        self.columns + self.beams + self.footings + self.slabs
    }
}

/// Solves `a · x = b` by Gaussian elimination with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let (above, below) = a.split_at_mut(row);
            for (target, source) in below[0][col..].iter_mut().zip(&above[col][col..]) {
                *target -= factor * source;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

/// End moments (sagging positive, kN·m) of a continuous beam on pinned
/// supports, by the slope-deflection (stiffness) method: `spans` gives each
/// span's length, uniform load and EI.
#[cfg(test)]
pub fn continuous_beam(spans: &[(f64, f64, f64)]) -> Vec<(f64, f64)> {
    let empty: Vec<Vec<(f64, f64)>> = vec![Vec::new(); spans.len()];
    continuous_beam_with_point_loads(spans, &empty)
}

/// Same as [`continuous_beam`], with optional concentrated point loads
/// `(distance_from_left_m, load_kn)` per span (e.g. reactions of secondary
/// beams supported on this beam).
pub fn continuous_beam_with_point_loads(
    spans: &[(f64, f64, f64)],
    point_loads: &[Vec<(f64, f64)>],
) -> Vec<(f64, f64)> {
    let n = spans.len() + 1;
    let mut k = vec![vec![0.0; n]; n];
    let mut f = vec![0.0; n];
    let fems: Vec<(f64, f64)> = spans
        .iter()
        .enumerate()
        .map(|(i, (length, load, _))| {
            let l = length.max(1e-6);
            // Fixed-end moments (clockwise positive): -wL²/12 at left, +wL²/12 at right.
            let mut fem_ab = -load * l * l / 12.0;
            let mut fem_ba = load * l * l / 12.0;
            if let Some(loads) = point_loads.get(i) {
                for &(a, p) in loads {
                    let a = a.clamp(0.0, l);
                    let b = l - a;
                    fem_ab -= p * a * b * b / (l * l);
                    fem_ba += p * a * a * b / (l * l);
                }
            }
            (fem_ab, fem_ba)
        })
        .collect();
    for (i, (length, _, ei)) in spans.iter().enumerate() {
        let s = 2.0 * ei / length.max(1e-6);
        k[i][i] += 2.0 * s;
        k[i][i + 1] += s;
        k[i + 1][i] += s;
        k[i + 1][i + 1] += 2.0 * s;
        f[i] -= fems[i].0;
        f[i + 1] -= fems[i].1;
    }
    let theta = solve(k, f).unwrap_or_else(|| vec![0.0; n]);
    spans
        .iter()
        .enumerate()
        .map(|(i, (length, _, ei))| {
            let s = 2.0 * ei / length.max(1e-6);
            let m_ab = s * (2.0 * theta[i] + theta[i + 1]) + fems[i].0;
            let m_ba = s * (theta[i] + 2.0 * theta[i + 1]) + fems[i].1;
            (m_ab, -m_ba)
        })
        .collect()
}

/// Cracked-section stiffness summary for a rectangular RC beam span per
/// ABNT NBR 6118:2023 (17.3.2.1.1 — Branson's equivalent inertia).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamStiffness {
    /// Gross concrete inertia I_c, m⁴.
    pub ic_m4: f64,
    /// Stage II cracked transformed inertia I_II, m⁴.
    pub i_cracked_m4: f64,
    /// Branson equivalent inertia I_eq, m⁴.
    pub i_eq_m4: f64,
    /// Cracking moment M_r, kN·m.
    pub m_r_knm: f64,
    /// Pre-sized longitudinal tension steel area A_s (CA-50), cm².
    pub as_cm2: f64,
}

/// Computes Branson's equivalent moment of inertia (NBR 6118 17.3.2.1.1) for a
/// rectangular beam (`b` × `h`, metres) of concrete `fck_mpa` under design
/// characteristic moment `m_k_design` (for tension steel pre-sizing) and
/// quasi-permanent service moment `m_a_qp` (kN·m).
pub fn effective_beam_stiffness(
    b: f64,
    h: f64,
    fck_mpa: f64,
    modulus_kn_m2: f64,
    m_k_design: f64,
    m_a_qp: f64,
) -> BeamStiffness {
    let fck = fck_mpa.max(10.0);
    let ic = (b * h.powi(3) / 12.0).max(1e-9);
    // NBR 6118 8.2.5 & 17.3.1: fct,m = 0.3·fck^(2/3) (MPa), α = 1.5 for rectangular beams.
    let fct_m_kn_m2 = 0.3 * fck.powf(2.0 / 3.0) * 1000.0;
    let m_r = 1.5 * fct_m_kn_m2 * ic / (h * 0.5).max(1e-6);

    // Pre-size tension reinforcement (CA-50, fyd = 500/1.15 MPa) at ULS (γf = 1.4),
    // never below NBR 6118 Table 17.3 minimum ratio ρ_min.
    let d = (h - 0.04).max(0.7 * h).max(0.05);
    let fcd_kn_m2 = (fck / 1.4) * 1000.0;
    let fyd_kn_m2 = (500.0 / 1.15) * 1000.0;
    let m_d = 1.4 * m_k_design.abs();
    let mu = (m_d / (b * d * d * 0.85 * fcd_kn_m2).max(1e-9)).min(0.295);
    let xi = ((1.0 - (1.0 - 2.0 * mu).max(0.0).sqrt()) / 0.8).clamp(0.0, 0.45);
    let z = d * (1.0 - 0.4 * xi);
    let rho_min = if fck <= 30.0 {
        0.0015
    } else if fck <= 35.0 {
        0.00173
    } else if fck <= 40.0 {
        0.00208
    } else if fck <= 45.0 {
        0.00245
    } else {
        0.00284
    };
    let as_m2 = (m_d / (z * fyd_kn_m2).max(1e-9)).max(rho_min * b * h);

    // Stage II cracked transformed section inertia I_II.
    const ES_KN_M2: f64 = 210_000_000.0;
    let alpha_e = ES_KN_M2 / modulus_kn_m2.max(1.0);
    let as_tr = alpha_e * as_m2;
    let x_ii = (-as_tr + (as_tr * as_tr + 2.0 * b * as_tr * d).sqrt()) / b.max(1e-6);
    let i_ii = (b * x_ii.powi(3) / 3.0 + as_tr * (d - x_ii).powi(2)).min(ic);

    // Branson interpolation (NBR 6118 17.3.2.1.1).
    let m_a = m_a_qp.abs();
    let i_eq = if m_a <= m_r || m_a <= 1e-9 {
        ic
    } else {
        let r3 = (m_r / m_a).powi(3);
        (r3 * ic + (1.0 - r3) * i_ii).min(ic)
    };
    BeamStiffness {
        ic_m4: ic,
        i_cracked_m4: i_ii,
        i_eq_m4: i_eq,
        m_r_knm: m_r,
        as_cm2: as_m2 * 10_000.0,
    }
}

/// Largest deflection of a span with end moments and a uniform load, by
/// integrating the curvature twice; same units as `ei`.
#[cfg(test)]
pub fn span_deflection(length: f64, load: f64, left: f64, right: f64, ei: f64) -> f64 {
    span_deflection_with_point_loads(length, load, &[], left, right, ei)
}

/// Same as [`span_deflection`], including concentrated point loads `(a_m, p_kn)`.
pub fn span_deflection_with_point_loads(
    length: f64,
    load: f64,
    point_loads: &[(f64, f64)],
    left: f64,
    right: f64,
    ei: f64,
) -> f64 {
    let steps = 100;
    let l = length.max(1e-6);
    let dx = l / steps as f64;
    let point_v_left: f64 = point_loads
        .iter()
        .map(|&(a, p)| p * (l - a.clamp(0.0, l)) / l)
        .sum();
    let v_left = load * l / 2.0 + point_v_left + (right - left) / l;
    let moment = |x: f64| {
        let mut m = left + v_left * x - load * x * x / 2.0;
        for &(a, p) in point_loads {
            let a = a.clamp(0.0, l);
            if x > a {
                m -= p * (x - a);
            }
        }
        m
    };
    let mut slope = 0.0;
    let mut deflection = vec![0.0; steps + 1];
    for i in 0..steps {
        let (x0, x1) = (i as f64 * dx, (i + 1) as f64 * dx);
        let next = slope + (moment(x0) + moment(x1)) / 2.0 / ei * dx;
        deflection[i + 1] = deflection[i] + (slope + next) / 2.0 * dx;
        slope = next;
    }
    let end = deflection[steps];
    (0..=steps)
        .map(|i| (deflection[i] - end * i as f64 / steps as f64).abs())
        .fold(0.0, f64::max)
}

/// Converts a wall direction vector into `(quarter_turns, angle_rad)`.
/// Orthogonal walls stay exact quarter turns (`angle_rad = 0.0`); angled
/// walls carry their exact radian angle in `[-π/2, π/2)` so columns align
/// cleanly along the wall.
fn wall_orientation(along: Point) -> (u8, f64) {
    let len = along.x.hypot(along.y);
    if len < 1e-6 {
        return (0, 0.0);
    }
    let mut theta = along.y.atan2(along.x);
    if theta >= std::f64::consts::FRAC_PI_2 {
        theta -= std::f64::consts::PI;
    } else if theta < -std::f64::consts::FRAC_PI_2 {
        theta += std::f64::consts::PI;
    }
    let tol = 0.5_f64.to_radians();
    if theta.abs() <= tol {
        (0, 0.0)
    } else if (theta.abs() - std::f64::consts::FRAC_PI_2).abs() <= tol {
        (1, 0.0)
    } else {
        (0, theta)
    }
}

/// Splits a slab polygon into rectangular sub-panels (trying horizontal and
/// vertical strip decompositions and picking the one with the smaller
/// governing short span `lx`), so L- and U-shaped rooms get their real panel
/// spans instead of the outer bounding box.
pub fn decompose_slab_panels(outline: &[Point]) -> Vec<(f64, f64)> {
    if outline.len() < 3 {
        return Vec::new();
    }
    let bbox = || {
        let (mut min, mut max) = (outline[0], outline[0]);
        for p in outline {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
        let (w, h) = (max.x - min.x, max.y - min.y);
        vec![(w.min(h), w.max(h))]
    };
    let slice_spans = |level: f64, horizontal: bool| -> Vec<(f64, f64)> {
        let mut crossings = Vec::new();
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let (al, bl, ap, bp) = if horizontal {
                (a.y, b.y, a.x, b.x)
            } else {
                (a.x, b.x, a.y, b.y)
            };
            if (al > level) != (bl > level) {
                crossings.push(ap + (level - al) / (bl - al) * (bp - ap));
            }
        }
        crossings.sort_by(f64::total_cmp);
        crossings
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|pair| pair[1] - pair[0] > 1e-4)
            .map(|pair| (pair[0], pair[1]))
            .collect()
    };
    let decompose = |horizontal: bool| -> Vec<(f64, f64)> {
        let mut cuts: Vec<f64> = outline
            .iter()
            .map(|p| if horizontal { p.y } else { p.x })
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        type Strip = (f64, f64, Vec<(f64, f64)>);
        let mut strips: Vec<Strip> = Vec::new();
        for pair in cuts.windows(2) {
            let (c0, c1) = (pair[0], pair[1]);
            if c1 - c0 <= 1e-4 {
                continue;
            }
            let intervals = slice_spans(0.5 * (c0 + c1), horizontal);
            if intervals.is_empty() {
                continue;
            }
            let same_as_prev = strips.last().is_some_and(|(_, _, prev)| {
                prev.len() == intervals.len()
                    && prev.iter().zip(&intervals).all(|((a0, a1), (b0, b1))| {
                        (a0 - b0).abs() < 1e-3 && (a1 - b1).abs() < 1e-3
                    })
            });
            if same_as_prev {
                if let Some(last) = strips.last_mut() {
                    last.1 = c1;
                }
            } else {
                strips.push((c0, c1, intervals));
            }
        }
        let mut panels = Vec::new();
        for (c0, c1, intervals) in strips {
            let h = c1 - c0;
            for (p0, p1) in intervals {
                let w = p1 - p0;
                panels.push((w.min(h), w.max(h)));
            }
        }
        panels
    };
    let h_panels = decompose(true);
    let v_panels = decompose(false);
    let gov = |panels: &[(f64, f64)]| panels.iter().map(|(lx, _)| *lx).fold(0.0_f64, f64::max);
    match (h_panels.is_empty(), v_panels.is_empty()) {
        (true, true) => bbox(),
        (false, true) => h_panels,
        (true, false) => v_panels,
        (false, false) => {
            let (gh, gv) = (gov(&h_panels), gov(&v_panels));
            if (gh - gv).abs() < 1e-4 {
                if h_panels.len() <= v_panels.len() {
                    h_panels
                } else {
                    v_panels
                }
            } else if gh < gv {
                h_panels
            } else {
                v_panels
            }
        }
    }
}

/// Distributes a slab's total load among its supporting beam segments by the
/// 45° yield-line method (NBR 6118 14.7.6.1): each interior point sheds its
/// load to the nearest supported edge, producing exact triangular/trapezoidal
/// tributary regions for rectangles and natural 45° regions for L-shaped or
/// irregular slabs. Returned fractions sum to 1.0.
pub fn yield_line_shares(
    outline: &[Point],
    supported: &[(u64, Point, Point)],
) -> HashMap<u64, f64> {
    let mut shares: HashMap<u64, f64> = HashMap::new();
    if outline.len() < 3 || supported.is_empty() {
        return shares;
    }
    if outline.len() == 4 {
        let mut xs: Vec<f64> = outline.iter().map(|p| p.x).collect();
        let mut ys: Vec<f64> = outline.iter().map(|p| p.y).collect();
        xs.sort_by(f64::total_cmp);
        ys.sort_by(f64::total_cmp);
        let (x0, x1) = (xs[0], xs[3]);
        let (y0, y1) = (ys[0], ys[3]);
        let is_rect = outline.iter().all(|p| {
            ((p.x - x0).abs() < 1e-4 || (p.x - x1).abs() < 1e-4)
                && ((p.y - y0).abs() < 1e-4 || (p.y - y1).abs() < 1e-4)
        });
        if is_rect && x1 - x0 > 1e-4 && y1 - y0 > 1e-4 {
            let w = x1 - x0;
            let h = y1 - y0;
            let lx = w.min(h);
            let ly = w.max(h);
            let short_area = lx * lx / 4.0;
            let long_area = 0.5 * lx * (ly - 0.5 * lx);
            let rect_edges = [
                (Point::new(x0, y0), Point::new(x1, y0), w),
                (Point::new(x1, y0), Point::new(x1, y1), h),
                (Point::new(x1, y1), Point::new(x0, y1), w),
                (Point::new(x0, y1), Point::new(x0, y0), h),
            ];
            let mut per_edge: Vec<Vec<(u64, f64)>> = vec![Vec::new(); 4];
            for (edge_idx, &(ea, eb, len)) in rect_edges.iter().enumerate() {
                let Some(dir) = eb.minus(ea).normalized() else {
                    continue;
                };
                for &(beam_id, sa, sb) in supported {
                    if sa.minus(ea).cross(dir).abs() <= 0.12
                        && sb.minus(ea).cross(dir).abs() <= 0.12
                    {
                        let s0 = sa.minus(ea).dot(dir);
                        let s1 = sb.minus(ea).dot(dir);
                        let overlap = s0.max(s1).min(len) - s0.min(s1).max(0.0);
                        if overlap > 0.05 {
                            per_edge[edge_idx].push((beam_id, overlap));
                        }
                    }
                }
            }
            if per_edge.iter().all(|e| !e.is_empty()) {
                let total_area = w * h;
                for (edge_idx, &(_, _, len)) in rect_edges.iter().enumerate() {
                    let edge_area = if (len - lx).abs() <= (len - ly).abs() {
                        short_area
                    } else {
                        long_area
                    };
                    let sum_overlap: f64 = per_edge[edge_idx].iter().map(|(_, o)| *o).sum();
                    for &(beam_id, overlap) in &per_edge[edge_idx] {
                        *shares.entry(beam_id).or_default() +=
                            (edge_area / total_area) * (overlap / sum_overlap);
                    }
                }
                return shares;
            }
        }
    }

    let (mut min, mut max) = (outline[0], outline[0]);
    for p in outline {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    let nx = 32usize;
    let ny = 32usize;
    let dx = (max.x - min.x) / nx as f64;
    let dy = (max.y - min.y) / ny as f64;
    if dx <= 1e-6 || dy <= 1e-6 {
        return shares;
    }
    let mut hits = 0.0;
    for ix in 0..nx {
        let x = min.x + (ix as f64 + 0.5) * dx;
        for iy in 0..ny {
            let y = min.y + (iy as f64 + 0.5) * dy;
            let pt = Point::new(x, y);
            if !super::geometry::polygon_contains_point(outline, pt) {
                continue;
            }
            let mut min_d = f64::INFINITY;
            let mut closest: Vec<u64> = Vec::new();
            for &(beam_id, sa, sb) in supported {
                let d = super::geometry::distance_to_segment(pt, sa, sb);
                if d + 1e-6 < min_d {
                    min_d = d;
                    closest.clear();
                    closest.push(beam_id);
                } else if (d - min_d).abs() <= 1e-6 {
                    closest.push(beam_id);
                }
            }
            if !closest.is_empty() {
                let w = 1.0 / closest.len() as f64;
                for beam_id in closest {
                    *shares.entry(beam_id).or_default() += w;
                }
                hits += 1.0;
            }
        }
    }
    if hits > 0.0 {
        for v in shares.values_mut() {
            *v /= hits;
        }
    }
    shares
}

/// Internal transfer record passed from an upper floor to the floor below.
struct UpperColumnLoad {
    floor_name: String,
    label: String,
    center: Point,
    load_kn: f64,
}

#[derive(Clone, Copy)]
struct BeamPointLoad {
    /// Parameter along `beam.from -> beam.to`.
    t: f64,
    load_kn: f64,
    /// Fraction retained in the quasi-permanent service combination.
    quasi_factor: f64,
}

impl Project {
    pub fn structural_columns(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind == FixtureKind::Column)
    }

    fn column(&self, id: u64) -> Option<&Fixture> {
        self.structural_columns().find(|column| column.id == id)
    }

    fn beam_by_id(&self, id: u64) -> Option<&Beam> {
        self.structure.beams.iter().find(|beam| beam.id == id)
    }

    fn resolve_support(&self, support: BeamSupport, visited: &mut HashSet<u64>) -> Option<Point> {
        match support {
            BeamSupport::Column { id } => self.column(id).map(|c| c.center),
            BeamSupport::Beam { beam_id, t } => {
                if !t.is_finite() || !(0.0..=1.0).contains(&t) || !visited.insert(beam_id) {
                    return None;
                }
                let host = self.beam_by_id(beam_id)?;
                let (a, b) = self.beam_endpoints_inner(host, visited)?;
                visited.remove(&beam_id);
                Some(a.lerp(b, t))
            }
        }
    }

    fn beam_endpoints_inner(
        &self,
        beam: &Beam,
        visited: &mut HashSet<u64>,
    ) -> Option<(Point, Point)> {
        let start = self.resolve_support(beam.start_support(), visited)?;
        let end = self.resolve_support(beam.end_support(), visited)?;
        if start.distance(end) < 1e-4 {
            return None;
        }
        Some((start, end))
    }

    /// World-space start and end points of `beam`, whether supported on
    /// columns or on other beams.
    pub fn beam_endpoints(&self, beam: &Beam) -> Option<(Point, Point)> {
        let mut visited = HashSet::from([beam.id]);
        self.beam_endpoints_inner(beam, &mut visited)
    }

    /// Support depth of a beam: 0 when resting only on columns, 1 when resting
    /// on a primary beam, etc.
    fn beam_support_depth(&self, beam: &Beam, visited: &mut HashSet<u64>) -> usize {
        let mut depth = 0;
        for support in [beam.start_support(), beam.end_support()] {
            if let BeamSupport::Beam { beam_id, .. } = support
                && visited.insert(beam_id)
                && let Some(host) = self.beam_by_id(beam_id)
            {
                depth = depth.max(1 + self.beam_support_depth(host, visited));
                visited.remove(&beam_id);
            }
        }
        depth
    }

    /// The beams whose two ends are valid supports (columns or acyclic host beams).
    pub fn valid_beams(&self) -> impl Iterator<Item = &Beam> {
        self.structure
            .beams
            .iter()
            .filter(|beam| self.beam_endpoints(beam).is_some())
    }

    /// Drops beams left without a valid support and slabs left without a room.
    pub fn prune_structure(&mut self) -> usize {
        let before = self.structure.beams.len() + self.structure.slabs.len();
        let valid: HashSet<u64> = self.valid_beams().map(|beam| beam.id).collect();
        self.structure.beams.retain(|beam| valid.contains(&beam.id));
        let rooms: HashSet<u64> = self.rooms.iter().map(|room| room.id).collect();
        self.structure
            .slabs
            .retain(|slab| rooms.contains(&slab.room_id));
        let columns: HashSet<u64> = self.structural_columns().map(|c| c.id).collect();
        let walls: HashSet<u64> = self.walls.iter().map(|w| w.id).collect();
        self.structure
            .wall_foundations
            .retain(|f| walls.contains(&f.wall_id));
        let balance_before = self.structure.balance_beams.len();
        self.structure
            .balance_beams
            .retain(|b| columns.contains(&b.column) && columns.contains(&b.anchor));
        before - self.structure.beams.len() - self.structure.slabs.len()
            + (balance_before - self.structure.balance_beams.len())
    }

    /// Where a column or footing lands for the pointer at `point`: on the
    /// nearest wall junction within 0.5 m, else on the nearest wall's
    /// centre line, turned along the wall (at any angle for diagonal walls).
    pub fn structural_placement_with_angle(&self, point: Point) -> (Point, u8, f64) {
        let mut best: Option<(f64, Point, u8, f64)> = None;
        for wall in &self.walls {
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            // Junctions win over a plain point on the wall.
            for end in [wall.start, wall.end] {
                let d = end.distance(point);
                let score = d - 0.2;
                if d < 0.5 && best.is_none_or(|(b, _, _, _)| score < b) {
                    best = Some((score, end, turns, angle));
                }
            }
            let (on, _) = super::geometry::nearest_point_on_segment(point, wall.start, wall.end);
            let d = on.distance(point);
            if d < 0.5 && best.is_none_or(|(b, _, _, _)| d < b) {
                best = Some((d, on, turns, angle));
            }
        }
        best.map_or((point, 0, 0.0), |(_, at, turns, angle)| (at, turns, angle))
    }

    /// Moves a fixture of this size just inside the lot when part of it would
    /// stand outside: a column or footing on a boundary wall ends up flush
    /// with the lot line instead of being refused (B23). The centre comes
    /// back unchanged when it already fits or there is no lot.
    pub fn fit_inside_lot(
        &self,
        kind: FixtureKind,
        center: Point,
        width: f64,
        depth: f64,
        quarter_turns: u8,
        angle_rad: f64,
    ) -> Point {
        let Some(terrain) = &self.terrain else {
            return center;
        };
        let outline = terrain.outline();
        if outline.len() < 3 {
            return center;
        }
        let mut probe = Fixture {
            id: 0,
            kind,
            center,
            width,
            depth,
            quarter_turns: quarter_turns % 4,
            angle_rad,
            electrical: None,
            structural_load_kn: kind.default_structural_load_kn(),
        };
        let limit = width.max(depth);
        for _ in 0..6 {
            let (low, high) = probe.bounds();
            let outside = [
                low,
                Point::new(high.x, low.y),
                high,
                Point::new(low.x, high.y),
            ]
            .into_iter()
            .find(|corner| !terrain.contains(*corner));
            let Some(corner) = outside else {
                break;
            };
            let nearest = (0..outline.len())
                .map(|i| {
                    super::geometry::nearest_point_on_segment(
                        corner,
                        outline[i],
                        outline[(i + 1) % outline.len()],
                    )
                    .0
                })
                .min_by(|a, b| a.distance(corner).total_cmp(&b.distance(corner)));
            let Some(nearest) = nearest else {
                break;
            };
            let Some(direction) = nearest.minus(corner).normalized() else {
                break;
            };
            let step = nearest.distance(corner) + 0.001;
            probe.center = probe.center.translated(direction.scaled(step));
            if probe.center.distance(center) > limit {
                return center;
            }
        }
        probe.center
    }

    /// Where a column or footing of this size lands for the pointer at
    /// `point`: on the nearest wall junction or centre line, then moved just
    /// inside the lot if it would cross the lot line.
    pub fn structural_placement_fitted(
        &self,
        kind: FixtureKind,
        point: Point,
        width: f64,
        depth: f64,
    ) -> (Point, u8, f64) {
        let (at, turns, angle) = self.structural_placement_with_angle(point);
        (
            self.fit_inside_lot(kind, at, width, depth, turns, angle),
            turns,
            angle,
        )
    }

    /// Same as [`structural_placement_with_angle`], returning position and quarter turns.
    #[cfg(test)]
    pub fn structural_placement(&self, point: Point) -> (Point, u8) {
        let (at, turns, _) = self.structural_placement_with_angle(point);
        (at, turns)
    }

    /// Joins two columns with a beam. Returns its id.
    pub fn add_beam(&mut self, from: u64, to: u64, width: f64, height: f64) -> Result<u64, String> {
        self.add_beam_on_supports(
            BeamSupport::Column { id: from },
            BeamSupport::Column { id: to },
            width,
            height,
        )
    }

    /// Joins two supports (columns or points along existing beams) with a beam.
    pub fn add_beam_on_supports(
        &mut self,
        from: BeamSupport,
        to: BeamSupport,
        width: f64,
        height: f64,
    ) -> Result<u64, String> {
        if from == to {
            return Err("Escolha dois apoios diferentes.".to_owned());
        }
        for support in [from, to] {
            match support {
                BeamSupport::Column { id } => {
                    if self.column(id).is_none() {
                        return Err(format!("O id {id} não é um pilar."));
                    }
                }
                BeamSupport::Beam { beam_id, t } => {
                    if !t.is_finite() || !(0.01..=0.99).contains(&t) {
                        return Err(
                            "A posição do apoio na viga deve ficar entre 0,01 e 0,99.".to_owned()
                        );
                    }
                    if self
                        .beam_by_id(beam_id)
                        .and_then(|b| self.beam_endpoints(b))
                        .is_none()
                    {
                        return Err(format!("O id {beam_id} não é uma viga válida."));
                    }
                }
            }
        }
        validate_section(width, height)?;
        let mut visited = HashSet::new();
        let a = self
            .resolve_support(from, &mut visited)
            .ok_or("Apoio inicial inválido.")?;
        visited.clear();
        let b = self
            .resolve_support(to, &mut visited)
            .ok_or("Apoio final inválido.")?;
        if a.distance(b) < 0.05 {
            return Err("Os apoios da viga estão muito próximos.".to_owned());
        }
        if let (BeamSupport::Column { id: c1 }, BeamSupport::Column { id: c2 }) = (from, to)
            && self.valid_beams().any(|beam| {
                matches!(
                    (beam.start_support(), beam.end_support()),
                    (BeamSupport::Column { id: a }, BeamSupport::Column { id: b })
                        if (a == c1 && b == c2) || (a == c2 && b == c1)
                )
            })
        {
            return Err("Esses pilares já estão ligados por uma viga.".to_owned());
        }
        let id = self.id();
        let from_support = matches!(from, BeamSupport::Beam { .. }).then_some(from);
        let to_support = matches!(to, BeamSupport::Beam { .. }).then_some(to);
        self.structure.beams.push(Beam {
            id,
            from: from.target_id(),
            to: to.target_id(),
            from_support,
            to_support,
            width_m: width,
            height_m: height,
        });
        Ok(id)
    }

    pub fn delete_beam(&mut self, id: u64) -> bool {
        let before = self.structure.beams.len();
        self.structure.beams.retain(|beam| beam.id != id);
        self.structure.beams.len() < before
    }

    pub fn update_beam(
        &mut self,
        id: u64,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Result<(), String> {
        let beam = self
            .structure
            .beams
            .iter_mut()
            .find(|beam| beam.id == id)
            .ok_or("Viga não encontrada.")?;
        let (w, h) = (
            width.unwrap_or(beam.width_m),
            height.unwrap_or(beam.height_m),
        );
        validate_section(w, h)?;
        beam.width_m = w;
        beam.height_m = h;
        Ok(())
    }

    /// Beams touching any of `columns`, including secondary beams transitively
    /// supported on them.
    pub fn beams_of(&self, columns: &[u64]) -> Vec<u64> {
        let mut affected: HashSet<u64> = self
            .valid_beams()
            .filter(|beam| {
                matches!(beam.start_support(), BeamSupport::Column { id } if columns.contains(&id))
                    || matches!(beam.end_support(), BeamSupport::Column { id } if columns.contains(&id))
            })
            .map(|beam| beam.id)
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for beam in self.valid_beams() {
                if affected.contains(&beam.id) {
                    continue;
                }
                let rests_on_affected = [beam.start_support(), beam.end_support()]
                    .into_iter()
                    .any(|s| matches!(s, BeamSupport::Beam { beam_id, .. } if affected.contains(&beam_id)));
                if rests_on_affected {
                    affected.insert(beam.id);
                    changed = true;
                }
            }
        }
        self.valid_beams()
            .filter(|beam| affected.contains(&beam.id))
            .map(|beam| beam.id)
            .collect()
    }

    /// Sets (or adds) the slab over a room.
    pub fn set_slab(
        &mut self,
        room_id: u64,
        kind: SlabKind,
        thickness: Option<f64>,
    ) -> Result<u64, String> {
        let room = self
            .rooms
            .iter()
            .find(|room| room.id == room_id)
            .ok_or("Cômodo não encontrado.")?
            .clone();
        let lx = self.slab_spans(&room).0;
        let thickness = thickness.unwrap_or_else(|| kind.minimum_thickness(lx));
        if !thickness.is_finite() || !(0.05..=0.60).contains(&thickness) {
            return Err("A espessura da laje deve ficar entre 5 e 60 cm.".to_owned());
        }
        if let Some(slab) = self
            .structure
            .slabs
            .iter_mut()
            .find(|s| s.room_id == room_id)
        {
            slab.kind = kind;
            slab.thickness_m = thickness;
            return Ok(slab.id);
        }
        let id = self.id();
        self.structure.slabs.push(Slab {
            id,
            room_id,
            kind,
            thickness_m: thickness,
        });
        Ok(id)
    }

    pub fn remove_slab(&mut self, room_id: u64) -> bool {
        let before = self.structure.slabs.len();
        self.structure.slabs.retain(|slab| slab.room_id != room_id);
        self.structure.slabs.len() < before
    }

    /// The smaller and larger span of the slab over `room` (its outline's
    /// bounding box, axis to axis), metres.
    fn slab_spans(&self, room: &super::model::Room) -> (f64, f64) {
        let Some(outline) = self.room_outline(room) else {
            return (0.0, 0.0);
        };
        let panels = decompose_slab_panels(&outline);
        if panels.is_empty() {
            return (0.0, 0.0);
        }
        let lx = panels.iter().map(|(x, _)| *x).fold(0.0_f64, f64::max);
        let ly = panels.iter().map(|(_, y)| *y).fold(0.0_f64, f64::max);
        (lx, ly)
    }

    /// Column, beam, slab, footing and concentrated-load labels: P1, V1,
    /// L1, S1 and CC1… in reading
    /// order, footings after the column they carry.
    pub fn structure_labels(&self) -> HashMap<u64, String> {
        let mut labels = HashMap::new();
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for (i, column) in columns.iter().enumerate() {
            labels.insert(column.id, format!("P{}", i + 1));
        }
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        let mut next = columns.len() + 1;
        for footing in footings {
            let under = columns
                .iter()
                .position(|column| footing.contains(column.center))
                .map(|i| i + 1);
            let number = under.unwrap_or_else(|| {
                next += 1;
                next - 1
            });
            labels.insert(footing.id, format!("S{number}"));
        }
        for line in self.beam_lines() {
            for span in &line.spans {
                labels.insert(*span, line.label.clone());
            }
        }
        let mut slabs: Vec<(Point, u64)> = self
            .structure
            .slabs
            .iter()
            .filter_map(|slab| {
                let room = self.rooms.iter().find(|room| room.id == slab.room_id)?;
                Some((self.room_label_anchor(room)?.0, slab.id))
            })
            .collect();
        slabs.sort_by(|a, b| reading(a.0, b.0));
        for (i, (_, id)) in slabs.iter().enumerate() {
            labels.insert(*id, format!("L{}", i + 1));
        }
        let mut loads: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| {
                matches!(
                    fixture.kind,
                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                )
            })
            .collect();
        loads.sort_by(|a, b| reading(a.center, b.center));
        for (index, load) in loads.iter().enumerate() {
            labels.insert(load.id, format!("CC{}", index + 1));
        }
        labels
    }

    /// Collinear beams through shared columns, as continuous beams.
    pub fn beam_lines(&self) -> Vec<BeamLine> {
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                let (a, b) = self.beam_endpoints(beam)?;
                Some((*beam, a, b))
            })
            .collect();
        let direction = |a: Point, b: Point| {
            let d = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
            if d.x > 1e-6 || (d.x.abs() <= 1e-6 && d.y > 0.0) {
                d
            } else {
                d.scaled(-1.0)
            }
        };
        // Union-find over beams joined end to end at a shared column in a straight line.
        let mut parent: Vec<usize> = (0..beams.len()).collect();
        fn root(parent: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        // Index beams by their column supports so union-find only checks
        // collinear beams meeting at the same column (O(B) instead of O(B²)).
        let mut by_column: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, (beam, _, _)) in beams.iter().enumerate() {
            for support in [beam.start_support(), beam.end_support()] {
                if let BeamSupport::Column { id } = support {
                    by_column.entry(id).or_default().push(idx);
                }
            }
        }
        let dirs: Vec<Point> = beams.iter().map(|(_, a, b)| direction(*a, *b)).collect();
        for group in by_column.values() {
            for a_idx in 0..group.len() {
                for b_idx in a_idx + 1..group.len() {
                    let (i, j) = (group[a_idx], group[b_idx]);
                    if dirs[i].cross(dirs[j]).abs() < 0.02 {
                        let (ra, rb) = (root(&mut parent, i), root(&mut parent, j));
                        parent[ra] = rb;
                    }
                }
            }
        }
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..beams.len() {
            let r = root(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
        let mut lines: Vec<(u8, f64, f64, BeamLine)> = groups
            .into_values()
            .map(|members| {
                let d = direction(beams[members[0]].1, beams[members[0]].2);
                let along = |p: Point| p.dot(d);
                let mut spans: Vec<&(Beam, Point, Point)> =
                    members.iter().map(|i| &beams[*i]).collect();
                spans.sort_by(|a, b| along(a.1.midpoint(a.2)).total_cmp(&along(b.1.midpoint(b.2))));
                let mut joints: Vec<(f64, BeamSupport)> = Vec::new();
                for (beam, a, b) in &spans {
                    for (support, p) in [(beam.start_support(), *a), (beam.end_support(), *b)] {
                        let already = joints
                            .iter()
                            .any(|(pos, s)| *s == support && (*pos - along(p)).abs() < 1e-3);
                        if !already {
                            joints.push((along(p), support));
                        }
                    }
                }
                joints.sort_by(|a, b| a.0.total_cmp(&b.0));
                let points: Vec<Point> = spans.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
                let (sum_x, sum_y) = points
                    .iter()
                    .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
                let n = points.len().max(1) as f64;
                let min_x = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
                let min_y = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
                let (group, key1, key2) = if d.y.abs() < 0.1 {
                    (0, sum_y / n, min_x)
                } else if d.x.abs() < 0.1 {
                    (1, sum_x / n, min_y)
                } else {
                    (2, min_y, min_x)
                };
                let supports: Vec<BeamSupport> = joints.into_iter().map(|(_, s)| s).collect();
                let joint_ids: Vec<u64> = supports.iter().map(|s| s.target_id()).collect();
                (
                    group,
                    key1,
                    key2,
                    BeamLine {
                        label: String::new(),
                        spans: spans.iter().map(|(beam, _, _)| beam.id).collect(),
                        joints: joint_ids,
                        supports,
                    },
                )
            })
            .collect();
        lines.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(((a.1 / 0.3).round() as i64).cmp(&((b.1 / 0.3).round() as i64)))
                .then(a.2.total_cmp(&b.2))
        });
        lines
            .into_iter()
            .enumerate()
            .map(|(i, (_, _, _, mut line))| {
                line.label = format!("V{}", i + 1);
                line
            })
            .collect()
    }

    /// Lays out a first structure over the walls: a column at every wall
    /// junction and end, more along walls longer than 4.5 m (clear of
    /// doors and windows), a beam along every wall between consecutive
    /// columns, a lattice slab over every room and a footing under every
    /// column, each sized by the pre-sizing. What exists stays.
    pub fn generate_structure(&mut self) -> StructureGeneration {
        let mut report = StructureGeneration::default();
        let max_span = 4.5;
        let near = |project: &Project, p: Point| {
            project
                .structural_columns()
                .any(|column| column.center.distance(p) < 0.3)
        };
        let walls = self.walls.clone();
        // Columns at junctions and ends (oriented along angled walls too).
        for wall in &walls {
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            for end in [wall.start, wall.end] {
                if near(self, end) {
                    continue;
                }
                // On a boundary wall the column stands flush inside the lot.
                let at = self.fit_inside_lot(FixtureKind::Column, end, 0.30, b, turns, angle);
                if near(self, at) {
                    continue;
                }
                match self.add_fixture(FixtureKind::Column, at, 0.30, b, turns) {
                    Some(id) => {
                        if angle.abs() > 1e-9
                            && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                        {
                            f.angle_rad = angle;
                        }
                        report.columns += 1;
                    }
                    None => report.skipped += 1,
                }
            }
        }
        // Intermediate columns on long walls, clear of openings.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            if length <= max_span {
                continue;
            }
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            let blocked: Vec<(f64, f64)> = self
                .openings
                .iter()
                .filter(|opening| opening.wall_id == wall.id)
                .map(|opening| {
                    let s = opening.t * length;
                    let half = opening.width() / 2.0 + 0.2;
                    (s - half, s + half)
                })
                .collect();
            let pieces = (length / max_span).ceil() as usize;
            for k in 1..pieces {
                let target = length * k as f64 / pieces as f64;
                let spot = (0..30)
                    .flat_map(|i| [f64::from(i) * 0.1, -f64::from(i) * 0.1])
                    .map(|delta| target + delta)
                    .find(|s| {
                        *s > 0.5
                            && *s < length - 0.5
                            && !blocked.iter().any(|(a, z)| s > a && s < z)
                    });
                if let Some(s) = spot {
                    let on_axis = wall.start.lerp(wall.end, s / length);
                    let at =
                        self.fit_inside_lot(FixtureKind::Column, on_axis, 0.30, b, turns, angle);
                    if !near(self, at) {
                        match self.add_fixture(FixtureKind::Column, at, 0.30, b, turns) {
                            Some(id) => {
                                if angle.abs() > 1e-9
                                    && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                                {
                                    f.angle_rad = angle;
                                }
                                report.columns += 1;
                            }
                            None => report.skipped += 1,
                        }
                    }
                }
            }
        }
        // Beams along each wall between consecutive columns.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            let mut on: Vec<(f64, u64)> = self
                .structural_columns()
                .filter(|column| {
                    // A column flush with the lot line sits up to half its
                    // section off the wall's centre line.
                    let off = 0.06 + column.width.max(column.depth) / 2.0;
                    super::geometry::distance_to_segment(column.center, wall.start, wall.end) < off
                })
                .map(|column| (column.center.distance(wall.start), column.id))
                .filter(|(s, _)| *s <= length + 0.06)
                .collect();
            on.sort_by(|a, b| a.0.total_cmp(&b.0));
            let b = round_up(wall.thickness().max(0.12), 0.01).min(0.30);
            for pair in on.windows(2) {
                // Columns this close act together: a beam between them
                // would only lift one off its support.
                let span = pair[1].0 - pair[0].0;
                if span < 0.6 {
                    continue;
                }
                let h = round_up(span / 10.0, 0.05).max(0.30);
                if self.add_beam(pair[0].1, pair[1].1, b, h).is_ok() {
                    report.beams += 1;
                }
            }
        }
        // Slabs.
        let rooms: Vec<u64> = self.rooms.iter().map(|room| room.id).collect();
        for room in rooms {
            if !self.structure.slabs.iter().any(|slab| slab.room_id == room)
                && self.set_slab(room, SlabKind::Ribbed, None).is_ok()
            {
                report.slabs += 1;
            }
        }
        // Footings, sized by the loads.
        let columns: Vec<Fixture> = self.structural_columns().copied().collect();
        for column in columns {
            let has = self
                .fixtures
                .iter()
                .any(|f| f.kind == FixtureKind::Footing && f.contains(column.center));
            // A footing under a boundary column stays inside the lot, off
            // centre from its column (a boundary footing, see #144).
            let at = self.fit_inside_lot(
                FixtureKind::Footing,
                column.center,
                0.60,
                0.60,
                column.quarter_turns,
                column.angle_rad,
            );
            if !has
                && let Some(id) =
                    self.add_fixture(FixtureKind::Footing, at, 0.60, 0.60, column.quarter_turns)
            {
                if column.angle_rad.abs() > 1e-9
                    && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                {
                    f.angle_rad = column.angle_rad;
                }
                report.footings += 1;
            }
        }
        self.size_footings();
        report
    }

    /// Resizes every footing to what the soil asks for (never smaller than
    /// 60 cm). Returns how many changed.
    pub fn size_footings(&mut self) -> usize {
        let report = self.structure_report();
        let mut changed = 0;
        for result in report.footings {
            if let Some(footing) = self.fixtures.iter_mut().find(|f| f.id == result.id)
                && ((footing.width - result.needed_width_m).abs() > 1e-6
                    || (footing.depth - result.needed_depth_m).abs() > 1e-6)
            {
                footing.width = result.needed_width_m;
                footing.depth = result.needed_depth_m;
                changed += 1;
            }
        }
        // A footing that grew across the lot line moves back inside it.
        let fitted: Vec<(u64, Point)> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .map(|f| {
                (
                    f.id,
                    self.fit_inside_lot(
                        f.kind,
                        f.center,
                        f.width,
                        f.depth,
                        f.quarter_turns,
                        f.angle_rad,
                    ),
                )
            })
            .collect();
        for (id, center) in fitted {
            if let Some(footing) = self.fixtures.iter_mut().find(|f| f.id == id) {
                footing.center = center;
            }
        }
        changed
    }

    fn floor_snapshot(&self, level: &super::floors::FloorLevel) -> Project {
        let mut snap = Project::default();
        snap.wall_height = level.ceiling_height_m.max(2.0);
        snap.walls = level.data.walls.clone();
        snap.rooms = level.data.rooms.clone();
        snap.openings = level.data.openings.clone();
        snap.fixtures = level.data.fixtures.clone();
        snap.structure.settings = self.structure.settings;
        snap.structure.settings.floors = 1;
        snap.structure.beams = level.data.beams.clone();
        snap.structure.slabs = level.data.slabs.clone();
        snap.structure.roofs = level.data.roofs.clone();
        snap
    }

    /// The load path from the slabs (and any real upper floors above) down to
    /// the soil, and the pre-sizing checks of every element.
    pub fn structure_report(&self) -> StructureReport {
        // When real upper floors with structural elements exist above the
        // active floor, propagate their actual column loads top-down instead
        // of using the scalar `settings.floors` multiplier.
        let active_id = self.floors.active_floor_id;
        let active_elev = self
            .active_floor()
            .map(|level| level.elevation_m)
            .unwrap_or(0.0);
        let mut upper_levels: Vec<&super::floors::FloorLevel> = self
            .floors
            .levels
            .iter()
            .filter(|level| {
                Some(level.id) != active_id
                    && level.elevation_m > active_elev + 1e-4
                    && (!level.data.beams.is_empty()
                        || !level.data.slabs.is_empty()
                        || level
                            .data
                            .fixtures
                            .iter()
                            .any(|f| f.kind == FixtureKind::Column))
            })
            .collect();
        if upper_levels.is_empty() {
            return self.structure_report_with_incoming(&[], false, true);
        }
        // Top-down: highest storey first down to the storey directly above.
        upper_levels.sort_by(|a, b| b.elevation_m.total_cmp(&a.elevation_m));
        let mut incoming: Vec<UpperColumnLoad> = Vec::new();
        let mut inherited_misaligned: Vec<MisalignedColumn> = Vec::new();
        let mut inherited_warnings: Vec<String> = Vec::new();
        for level in upper_levels {
            let snap = self.floor_snapshot(level);
            let rep = snap.structure_report_with_incoming(&incoming, true, false);
            inherited_misaligned.extend(rep.misaligned_columns);
            for w in rep.warnings {
                if w.contains("desalinhado") || w.contains("sem prumada") {
                    inherited_warnings.push(w);
                }
            }
            incoming = rep
                .columns
                .iter()
                .filter_map(|col| {
                    let fix = snap.column(col.id)?;
                    Some(UpperColumnLoad {
                        floor_name: level.name.clone(),
                        label: col.label.clone(),
                        center: fix.center,
                        load_kn: col.load_kn,
                    })
                })
                .collect();
        }
        let mut report = self.structure_report_with_incoming(&incoming, false, false);
        report.misaligned_columns.extend(inherited_misaligned);
        report.warnings.extend(inherited_warnings);
        report
    }

    fn structure_report_with_incoming(
        &self,
        incoming_upper: &[UpperColumnLoad],
        include_own_walls_on_beams: bool,
        use_multiplier: bool,
    ) -> StructureReport {
        let settings = self.structure.settings;
        let labels = self.structure_labels();
        let label = |id: u64| labels.get(&id).cloned().unwrap_or_else(|| "?".to_owned());
        let mut report = StructureReport::default();
        let floors = if use_multiplier {
            f64::from(settings.floors.max(1))
        } else {
            1.0
        };
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                let (a, b) = self.beam_endpoints(beam)?;
                Some((*beam, a, b))
            })
            .collect();
        // Match upper-floor columns against columns on this floor; misaligned
        // columns transfer as point loads onto a beam below or warn if unsupported.
        let mut incoming_column_load: HashMap<u64, f64> = HashMap::new();
        let mut beam_point_loads: HashMap<u64, Vec<BeamPointLoad>> = HashMap::new();
        // Timber roofs (#114): each main-beam end bears on the column under
        // it, or on the concrete beam under it.
        let mut roof_on_column: HashMap<u64, f64> = HashMap::new();
        for roof in &self.structure.roofs {
            let mut result = self.analyse_roof(roof);
            for reaction in &mut result.reactions {
                let column = self
                    .structural_columns()
                    .filter(|c| {
                        c.contains(reaction.point) || c.center.distance(reaction.point) <= 0.35
                    })
                    .min_by(|a, b| {
                        a.center
                            .distance(reaction.point)
                            .total_cmp(&b.center.distance(reaction.point))
                    });
                if let Some(column) = column {
                    *roof_on_column.entry(column.id).or_default() += reaction.load_kn;
                    reaction.support = Some(label(column.id));
                    continue;
                }
                let host = beams
                    .iter()
                    .filter_map(|(beam, a, b)| {
                        let (nearest, t) =
                            super::geometry::nearest_point_on_segment(reaction.point, *a, *b);
                        let dist = nearest.distance(reaction.point);
                        (dist <= 0.25).then_some((dist, beam.id, t))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, beam_id, t)) = host {
                    beam_point_loads
                        .entry(beam_id)
                        .or_default()
                        .push(BeamPointLoad {
                            t: t.clamp(0.02, 0.98),
                            load_kn: reaction.load_kn,
                            quasi_factor: reaction.dead_kn / reaction.load_kn.max(1e-9),
                        });
                    reaction.support = Some(label(beam_id));
                }
            }
            if !result.slope_ok() {
                report.warnings.push(format!(
                    "Telhado {}: inclinação de {:.0} % abaixo dos {:.0} % que a telha pede.",
                    result.label, result.slope_percent, result.min_slope_percent
                ));
            }
            if result.sheet_span_exceeded {
                report.warnings.push(format!(
                    "Telhado {}: as telhas não vencem o espaçamento das terças; aproxime as terças.",
                    result.label
                ));
            }
            for element in result.elements.iter().filter(|e| !e.ok) {
                report.warnings.push(format!(
                    "Telhado {}: nem {}×{} cm resolve as {}; diminua o vão ou o espaçamento, ou use madeira mais forte.",
                    result.label,
                    element.section_cm.0,
                    element.section_cm.1,
                    element.element.label().to_lowercase()
                ));
            }
            report.roofs.push(result);
        }
        // The rest bears on timber posts (#146), directly or through a
        // timber beam along the edge.
        let posts = self.post_effects(&mut report.roofs);
        for result in &report.roofs {
            let loose: Vec<&super::roof::RoofReaction> = result
                .reactions
                .iter()
                .filter(|r| r.support.is_none())
                .collect();
            if !loose.is_empty() {
                report.warnings.push(format!(
                    "Telhado {}: {} apoio(s) de viga sem pilar, mourão nem viga embaixo (até {} kN cada); ponha pilares ou mourões nesses pontos.",
                    result.label,
                    loose.len(),
                    decimal(loose.iter().map(|r| r.load_kn).fold(0.0, f64::max), 1)
                ));
            }
        }
        for post in posts.posts.iter().filter(|p| !p.ok()) {
            report.warnings.push(match post.needed_m {
                Some(size) => format!(
                    "Mourão {}: {} cm não resiste à compressão com flambagem (λ = {:.0}, uso {:.0} %); use {} cm.",
                    post.label,
                    decimal(post.size_m * 100.0, 0),
                    post.slenderness,
                    (post.ratio * 100.0).min(999.0),
                    decimal(size * 100.0, 0)
                ),
                None => format!(
                    "Mourão {}: nem 30 cm resiste; diminua a altura ou a carga.",
                    post.label
                ),
            });
        }
        for beam in posts.eave_beams.iter().filter(|b| !b.ok) {
            report.warnings.push(format!(
                "Viga de madeira {}: nem {}×{} cm vence {} m entre {} e {}; ponha um mourão no meio.",
                beam.label,
                beam.section_cm.0,
                beam.section_cm.1,
                decimal(beam.span_m, 2),
                beam.from_post,
                beam.to_post
            ));
        }
        report.timber_posts = posts.posts;
        report.eave_beams = posts.eave_beams;
        for upper in incoming_upper {
            let aligned = self
                .structural_columns()
                .filter(|col| {
                    col.contains(upper.center) || col.center.distance(upper.center) <= 0.15
                })
                .min_by(|a, b| {
                    a.center
                        .distance(upper.center)
                        .total_cmp(&b.center.distance(upper.center))
                });
            if let Some(col) = aligned {
                *incoming_column_load.entry(col.id).or_default() += upper.load_kn;
            } else {
                // Check if the misaligned column lands on a beam below (transfer girder).
                let host_beam = beams
                    .iter()
                    .filter_map(|(beam, a, b)| {
                        let (nearest, t) =
                            super::geometry::nearest_point_on_segment(upper.center, *a, *b);
                        let dist = nearest.distance(upper.center);
                        (dist <= 0.20).then_some((dist, beam.id, t))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, beam_id, t)) = host_beam {
                    let beam_label = label(beam_id);
                    if upper.load_kn > 0.0 {
                        beam_point_loads
                            .entry(beam_id)
                            .or_default()
                            .push(BeamPointLoad {
                                t: t.clamp(0.02, 0.98),
                                load_kn: upper.load_kn,
                                quasi_factor: 1.0,
                            });
                    }
                    report.misaligned_columns.push(MisalignedColumn {
                        floor_name: upper.floor_name.clone(),
                        column_label: upper.label.clone(),
                        center: upper.center,
                        load_kn: upper.load_kn,
                        transfer_beam: Some(beam_label.clone()),
                    });
                    report.warnings.push(format!(
                        "Pilar {} ({}) desalinhado: nasce sobre {} (viga de transição com {} kN); confira o detalhamento.",
                        upper.label,
                        upper.floor_name,
                        beam_label,
                        decimal(upper.load_kn, 1)
                    ));
                } else {
                    report.misaligned_columns.push(MisalignedColumn {
                        floor_name: upper.floor_name.clone(),
                        column_label: upper.label.clone(),
                        center: upper.center,
                        load_kn: upper.load_kn,
                        transfer_beam: None,
                    });
                    report.warnings.push(format!(
                        "Pilar {} ({}) sem prumada abaixo ({} kN): não há pilar nem viga alinhados no pavimento inferior.",
                        upper.label,
                        upper.floor_name,
                        decimal(upper.load_kn, 1)
                    ));
                }
            }
        }

        // Loads that physically overlap a beam go straight to that beam.
        // The remaining loads are picked up by the slab panel below and
        // distributed to its nearest supported edges in the slab pass.
        let mut concentrated: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| {
                matches!(
                    fixture.kind,
                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                ) && fixture.structural_load_kn.is_some()
            })
            .collect();
        concentrated.sort_by(|a, b| reading(a.center, b.center));
        let concentrated_labels: HashMap<u64, String> = concentrated
            .iter()
            .enumerate()
            .map(|(index, fixture)| (fixture.id, format!("CC{}", index + 1)))
            .collect();
        let mut handled_concentrated = HashSet::new();
        for fixture in &concentrated {
            let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
            if !load_kn.is_finite() || load_kn <= 0.0 {
                report.warnings.push(format!(
                    "{}: carga concentrada inválida; informe um valor positivo em kN.",
                    concentrated_labels[&fixture.id]
                ));
                continue;
            }
            let direct = beams
                .iter()
                .filter_map(|(beam, a, b)| {
                    let (nearest, t) =
                        super::geometry::nearest_point_on_segment(fixture.center, *a, *b);
                    fixture.contains(nearest).then_some((
                        nearest.distance(fixture.center),
                        beam.id,
                        t,
                    ))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, beam_id, t)) = direct {
                beam_point_loads
                    .entry(beam_id)
                    .or_default()
                    .push(BeamPointLoad {
                        t: t.clamp(0.02, 0.98),
                        load_kn,
                        quasi_factor: 1.0,
                    });
                let beam_label = label(beam_id);
                report.concentrated_loads.push(ConcentratedLoadResult {
                    id: fixture.id,
                    label: concentrated_labels[&fixture.id].clone(),
                    kind: fixture.kind,
                    center: fixture.center,
                    load_kn,
                    support: format!("direto na viga {beam_label}"),
                    slab: None,
                    local_moment_knm_m: 0.0,
                    punching_ratio: None,
                    supported: true,
                });
                handled_concentrated.insert(fixture.id);
            }
        }
        // Slab loads shared by the beams along the slab's sides.
        let mut beam_dead: HashMap<u64, f64> = HashMap::new();
        let mut beam_live: HashMap<u64, f64> = HashMap::new();
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                continue;
            };
            let (lx, ly) = self.slab_spans(room);
            let area = self.room_area(room).unwrap_or(lx * ly);
            let dead = slab.kind.self_weight(slab.thickness_m) + settings.finish_kn_m2;
            let live = if settings.per_room_live_loads {
                room_live_load_kn_m2(room.display_name(), settings.live_kn_m2)
            } else {
                settings.live_kn_m2
            };
            let mut supported_segments: Vec<(u64, Point, Point)> = Vec::new();
            let shares = if let Some(outline) = self.room_outline(room) {
                for i in 0..outline.len() {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    let Some(dir) = b.minus(a).normalized() else {
                        continue;
                    };
                    let edge_len = a.distance(b);
                    for (beam, p, q) in &beams {
                        let off = |x: Point| x.minus(a).cross(dir).abs();
                        if off(*p) > 0.12 || off(*q) > 0.12 {
                            continue;
                        }
                        let (s0, s1) = (p.minus(a).dot(dir), q.minus(a).dot(dir));
                        let lo = s0.min(s1).max(0.0);
                        let hi = s0.max(s1).min(edge_len);
                        if hi - lo > 0.05 {
                            supported_segments.push((
                                beam.id,
                                a.translated(dir.scaled(lo)),
                                a.translated(dir.scaled(hi)),
                            ));
                        }
                    }
                }
                yield_line_shares(&outline, &supported_segments)
            } else {
                HashMap::new()
            };
            let total: f64 = shares.values().sum();
            for (id, share) in &shares {
                *beam_dead.entry(*id).or_default() += dead * area * share / total.max(1e-9);
                *beam_live.entry(*id).or_default() += live * area * share / total.max(1e-9);
            }
            let mut point_load_kn = 0.0;
            let mut point_moment_knm_m = 0.0;
            if let Some(outline) = self.room_outline(room) {
                let slab_loads: Vec<_> = concentrated
                    .iter()
                    .copied()
                    .filter(|fixture| {
                        !handled_concentrated.contains(&fixture.id)
                            && super::geometry::polygon_contains_point(&outline, fixture.center)
                    })
                    .collect();
                for fixture in slab_loads {
                    let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
                    if !load_kn.is_finite() || load_kn <= 0.0 {
                        continue;
                    }
                    let effective_width = (fixture.width.min(fixture.depth)
                        + 2.0 * slab.thickness_m)
                        .clamp(0.50, ly.max(0.50));
                    let local_moment = load_kn * lx / (4.0 * effective_width);
                    let punching_ratio = if slab.kind == SlabKind::Solid {
                        let d = (slab.thickness_m - 0.025).max(0.05);
                        let fck = settings.fck_mpa.max(20.0);
                        let f_sd = 1.4 * load_kn;
                        let fcd_kn_m2 = fck / 1.4 * 1000.0;
                        let alpha_v2 = (1.0 - fck / 250.0).max(0.1);
                        let tau_rd2 = 0.27 * alpha_v2 * fcd_kn_m2;
                        let u0 = 2.0 * (fixture.width + fixture.depth).max(0.40);
                        let tau_sd0 = f_sd / (u0 * d);
                        let u1 = 2.0 * (fixture.width + fixture.depth + 2.0 * d);
                        let tau_sd1 = f_sd / (u1 * d);
                        let k_size = (1.0 + (0.20 / d).sqrt()).min(2.0);
                        let tau_rd1 = 0.13 * k_size * (100.0 * 0.002 * fck).cbrt() * 1000.0;
                        Some((tau_sd0 / tau_rd2).max(tau_sd1 / tau_rd1))
                    } else {
                        None
                    };
                    point_load_kn += load_kn;
                    point_moment_knm_m += local_moment;

                    let mut distance_by_beam: HashMap<u64, f64> = HashMap::new();
                    for (beam_id, a, b) in &supported_segments {
                        let distance = super::geometry::distance_to_segment(fixture.center, *a, *b);
                        distance_by_beam
                            .entry(*beam_id)
                            .and_modify(|old| *old = old.min(distance))
                            .or_insert(distance);
                    }
                    let nearest = distance_by_beam
                        .values()
                        .copied()
                        .fold(f64::INFINITY, f64::min);
                    let hosts: Vec<u64> = distance_by_beam
                        .iter()
                        .filter_map(|(beam_id, distance)| {
                            (*distance <= nearest + 0.05).then_some(*beam_id)
                        })
                        .collect();
                    if hosts.is_empty() {
                        report.concentrated_loads.push(ConcentratedLoadResult {
                            id: fixture.id,
                            label: concentrated_labels[&fixture.id].clone(),
                            kind: fixture.kind,
                            center: fixture.center,
                            load_kn,
                            support: "laje sem viga de apoio".to_owned(),
                            slab: Some(label(slab.id)),
                            local_moment_knm_m: local_moment,
                            punching_ratio,
                            supported: false,
                        });
                        report.warnings.push(format!(
                            "{}: {} de {} kN está sobre a laje {} sem viga de apoio.",
                            concentrated_labels[&fixture.id],
                            fixture.kind.label(),
                            decimal(load_kn, 1),
                            label(slab.id)
                        ));
                    } else {
                        let share = load_kn / hosts.len() as f64;
                        let mut host_labels = Vec::new();
                        for beam_id in hosts {
                            let Some((_, a, b)) =
                                beams.iter().find(|(beam, _, _)| beam.id == beam_id)
                            else {
                                continue;
                            };
                            let (_, t) =
                                super::geometry::nearest_point_on_segment(fixture.center, *a, *b);
                            beam_point_loads
                                .entry(beam_id)
                                .or_default()
                                .push(BeamPointLoad {
                                    t: t.clamp(0.02, 0.98),
                                    load_kn: share,
                                    quasi_factor: 1.0,
                                });
                            host_labels.push(label(beam_id));
                        }
                        host_labels.sort();
                        host_labels.dedup();
                        report.concentrated_loads.push(ConcentratedLoadResult {
                            id: fixture.id,
                            label: concentrated_labels[&fixture.id].clone(),
                            kind: fixture.kind,
                            center: fixture.center,
                            load_kn,
                            support: format!(
                                "laje {} → {}",
                                label(slab.id),
                                host_labels.join(", ")
                            ),
                            slab: Some(label(slab.id)),
                            local_moment_knm_m: local_moment,
                            punching_ratio,
                            supported: true,
                        });
                        if slab.kind == SlabKind::Ribbed {
                            report.warnings.push(format!(
                                "{}: carga concentrada sobre laje treliçada; apoie sobre viga ou detalhe faixa maciça e vigotas com o fabricante.",
                                concentrated_labels[&fixture.id]
                            ));
                        } else if punching_ratio.is_some_and(|ratio| ratio > 1.0) {
                            report.warnings.push(format!(
                                "{}: punção local da laje excedida ({:.0}%); aumente a base/espessura ou apoie a carga sobre vigas.",
                                concentrated_labels[&fixture.id],
                                punching_ratio.unwrap_or(0.0) * 100.0
                            ));
                        }
                    }
                    handled_concentrated.insert(fixture.id);
                }
            }
            report.slabs.push(SlabResult {
                id: slab.id,
                label: label(slab.id),
                room: room.display_name().to_owned(),
                kind: slab.kind,
                lx_m: lx,
                ly_m: ly,
                thickness_m: slab.thickness_m,
                thickness_min_m: slab.kind.minimum_thickness(lx),
                load_kn_m2: dead + live,
                point_load_kn,
                point_moment_knm_m,
                supported: total > 0.0,
            });
        }

        for fixture in concentrated {
            if handled_concentrated.contains(&fixture.id) {
                continue;
            }
            let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
            if !load_kn.is_finite() || load_kn <= 0.0 {
                continue;
            }
            report.concentrated_loads.push(ConcentratedLoadResult {
                id: fixture.id,
                label: concentrated_labels[&fixture.id].clone(),
                kind: fixture.kind,
                center: fixture.center,
                load_kn,
                support: "sem laje ou viga abaixo".to_owned(),
                slab: None,
                local_moment_knm_m: 0.0,
                punching_ratio: None,
                supported: false,
            });
            report.warnings.push(format!(
                "{}: {} de {} kN está sem laje ou viga abaixo; reposicione ou projete apoio próprio.",
                concentrated_labels[&fixture.id],
                fixture.kind.label(),
                decimal(load_kn, 1)
            ));
        }

        // Continuous beams, ordered deepest-first so secondary beams resting on
        // other beams transfer their reactions as point loads onto host girders.
        let modulus = settings.modulus_kn_m2();
        let mut column_load: HashMap<u64, f64> = HashMap::new();
        let mut incident: HashMap<u64, Vec<Point>> = HashMap::new();
        let mut ordered_lines = self.beam_lines();
        ordered_lines.sort_by(|a, b| {
            let line_depth = |line: &BeamLine| {
                line.spans
                    .iter()
                    .filter_map(|id| self.beam_by_id(*id))
                    .map(|beam| {
                        let mut v = HashSet::from([beam.id]);
                        self.beam_support_depth(beam, &mut v)
                    })
                    .max()
                    .unwrap_or(0)
            };
            line_depth(b).cmp(&line_depth(a))
        });
        for line in ordered_lines {
            let mut spans = Vec::new();
            let mut stiff = Vec::new();
            let mut span_points: Vec<Vec<(f64, f64)>> = Vec::new();
            let mut span_point_records: Vec<Vec<BeamPointLoad>> = Vec::new();
            for (i, id) in line.spans.iter().enumerate() {
                let Some((beam, b_start, b_end)) = beams.iter().find(|(b, _, _)| b.id == *id)
                else {
                    continue;
                };
                let mut v0 = HashSet::new();
                let a = self.resolve_support(line.supports[i], &mut v0);
                let mut v1 = HashSet::new();
                let b = self.resolve_support(line.supports[i + 1], &mut v1);
                let (Some(a), Some(b)) = (a, b) else {
                    continue;
                };
                let length = a.distance(b).max(0.1);
                for (support, to) in [(line.supports[i], b), (line.supports[i + 1], a)] {
                    if let BeamSupport::Column { id: col_id } = support
                        && let Some(c) = self.column(col_id)
                        && let Some(d) = to.minus(c.center).normalized()
                    {
                        incident.entry(col_id).or_default().push(d);
                    }
                }
                // Masonry walls carried by this beam: either this floor's own
                // walls (on an upper storey) or the floors-1 multiplier when
                // only one floor is drawn.
                let wall_factor = if include_own_walls_on_beams {
                    1.0
                } else if use_multiplier && floors > 1.0 {
                    floors - 1.0
                } else {
                    0.0
                };
                let walls_kn_m: f64 = if wall_factor > 0.0 {
                    self.walls
                        .iter()
                        .filter(|wall| {
                            super::geometry::distance_to_segment(
                                a.midpoint(b),
                                wall.start,
                                wall.end,
                            ) < 0.06
                        })
                        .map(|wall| {
                            // The wall's own top over this span (#145).
                            wall.thickness()
                                * wall.height_at(self.wall_height, a.midpoint(b))
                                * settings.masonry_kn_m3
                        })
                        .fold(0.0, f64::max)
                        * wall_factor
                } else {
                    0.0
                };
                let own = CONCRETE_KN_M3 * beam.width_m * beam.height_m;
                let dead = own + walls_kn_m + beam_dead.get(id).copied().unwrap_or(0.0) / length;
                let live = beam_live.get(id).copied().unwrap_or(0.0) / length;
                let inertia = beam.width_m * beam.height_m.powi(3) / 12.0;
                // Map incoming secondary-beam reactions `(t, P)` along `beam.from -> beam.to`
                // into distance `x` from this span's left joint `a`.
                let reversed = a.distance(*b_end) < a.distance(*b_start);
                let point_records: Vec<BeamPointLoad> = beam_point_loads
                    .get(id)
                    .map(|list| {
                        list.iter()
                            .map(|point| {
                                let t_span = if reversed { 1.0 - point.t } else { point.t };
                                BeamPointLoad {
                                    t: (t_span * length).clamp(0.0, length),
                                    ..*point
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let pts: Vec<(f64, f64)> = point_records
                    .iter()
                    .map(|point| (point.t, point.load_kn))
                    .collect();
                spans.push((beam.id, length, dead, live, beam.width_m, beam.height_m));
                stiff.push((length, dead + live, modulus * inertia));
                span_points.push(pts);
                span_point_records.push(point_records);
            }
            if spans.len() != line.spans.len() {
                continue;
            }
            let moments = continuous_beam_with_point_loads(&stiff, &span_points);
            let mut reactions = vec![0.0; line.joints.len()];
            let mut results = Vec::new();
            let count = spans.len();
            for (i, ((id, length, dead, live, b, h), (left, right))) in
                spans.iter().zip(&moments).enumerate()
            {
                let w = dead + live;
                let pts = &span_points[i];
                let point_records = &span_point_records[i];
                let p_sum: f64 = pts.iter().map(|(_, p)| *p).sum();
                let point_v_left: f64 = pts
                    .iter()
                    .map(|&(a_pos, p)| p * (length - a_pos) / length)
                    .sum();
                let v_left = w * length / 2.0 + point_v_left + (right - left) / length;
                let v_right = v_left - w * length - p_sum;
                reactions[i] += v_left;
                reactions[i + 1] -= v_right;
                let moment_at = |x: f64| {
                    let mut m = left + v_left * x - w * x * x / 2.0;
                    for &(a_pos, p) in pts {
                        if x > a_pos {
                            m -= p * (x - a_pos);
                        }
                    }
                    m
                };
                let span_moment = if pts.is_empty() {
                    let x = (v_left / w.max(1e-9)).clamp(0.0, *length);
                    moment_at(x)
                } else {
                    (0..=100)
                        .map(|k| moment_at(*length * k as f64 / 100.0))
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                // Quasi-permanent load (ψ2 = 0.3), Branson's equivalent inertia
                // with cracking moment Mr and pre-sized CA-50 steel, plus creep
                // factor 1 + 1.32 (NBR 6118 17.3.2.1).
                let w_qp = dead + 0.3 * live;
                let ratio = if w > 0.0 { w_qp / w } else { 1.0 };
                let pts_qp: Vec<(f64, f64)> = point_records
                    .iter()
                    .map(|point| (point.t, point.load_kn * point.quasi_factor))
                    .collect();
                let m_k_max = span_moment.max(0.0).max(left.abs()).max(right.abs());
                let service_ratio = point_records
                    .iter()
                    .map(|point| point.quasi_factor)
                    .fold(ratio, f64::max);
                let m_a_qp = m_k_max * service_ratio;
                let stiffness =
                    effective_beam_stiffness(*b, *h, settings.fck_mpa, modulus, m_k_max, m_a_qp);
                let ei = modulus * stiffness.i_eq_m4;
                let deflection = span_deflection_with_point_loads(
                    *length,
                    w_qp,
                    &pts_qp,
                    left * service_ratio,
                    right * service_ratio,
                    ei,
                ) * (1.0 + 1.32);
                let end_span = i == 0 || i + 1 == count;
                let divisor = if count > 1 && !end_span { 12.0 } else { 10.0 };
                results.push(SpanResult {
                    beam: *id,
                    length_m: *length,
                    width_m: *b,
                    height_m: *h,
                    dead_kn_m: *dead,
                    live_kn_m: *live,
                    moment_left: *left,
                    moment_right: *right,
                    moment_span: span_moment.max(0.0),
                    shear: v_left.abs().max(v_right.abs()),
                    deflection_mm: deflection * 1000.0,
                    limit_mm: length / 250.0 * 1000.0,
                    height_min_m: round_up(length / divisor, 0.05).max(0.25),
                });
            }
            for ((joint, support), reaction) in
                line.joints.iter().zip(&line.supports).zip(&reactions)
            {
                match *support {
                    BeamSupport::Column { id } => {
                        // An upward reaction does not relieve the column below.
                        *column_load.entry(id).or_default() += reaction.max(0.0);
                    }
                    BeamSupport::Beam { beam_id, t } => {
                        if *reaction > 0.0 {
                            beam_point_loads
                                .entry(beam_id)
                                .or_default()
                                .push(BeamPointLoad {
                                    t,
                                    load_kn: *reaction,
                                    // A supporting beam receives a conservative permanent
                                    // reaction until dead/live components are separated.
                                    quasi_factor: 1.0,
                                });
                        }
                    }
                }
                if *reaction < -0.5 {
                    report.warnings.push(format!(
                        "{}: o apoio {} é puxado para cima ({} kN); evite um vão muito curto ao lado de um longo.",
                        line.label,
                        label(*joint),
                        decimal(-reaction, 1)
                    ));
                }
            }
            for span in &results {
                if span.height_m + 1e-6 < span.height_min_m {
                    report.warnings.push(format!(
                        "{}: vão de {} m pede altura de pelo menos {:.0} cm (tem {:.0} cm).",
                        line.label,
                        decimal(span.length_m, 2),
                        span.height_min_m * 100.0,
                        span.height_m * 100.0
                    ));
                }
                if span.deflection_mm > span.limit_mm {
                    report.warnings.push(format!(
                        "{}: flecha de {} mm passa do limite L/250 ({} mm); aumente a altura.",
                        line.label,
                        decimal(span.deflection_mm, 1),
                        decimal(span.limit_mm, 1)
                    ));
                }
                if span.width_m < 0.12 - 1e-9 {
                    report.warnings.push(format!(
                        "{}: largura abaixo de 12 cm (NBR 6118 13.2.2).",
                        line.label
                    ));
                }
            }
            report.beams.push(BeamLineResult {
                label: line.label.clone(),
                joints: line.joints.clone(),
                spans: results,
                reactions,
            });
        }

        // Columns.
        let fcd = settings.fck_mpa / 1.4 / 10.0; // kN/cm²
        let capacity = 0.85 * fcd + 0.015 * 42.0;
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for column in columns {
            let directions = incident.get(&column.id).cloned().unwrap_or_default();
            let mut distinct: Vec<Point> = Vec::new();
            for d in directions {
                if !distinct.iter().any(|e| e.dot(d) > 0.98) {
                    distinct.push(d);
                }
            }
            let position = match distinct.len() {
                0 => ColumnPosition::Isolated,
                1 => ColumnPosition::Corner,
                2 if distinct[0].dot(distinct[1]) < -0.98 => ColumnPosition::Edge,
                2 => ColumnPosition::Corner,
                3 => ColumnPosition::Edge,
                _ => ColumnPosition::Interior,
            };
            // As tall as the tallest wall it stands in (#145).
            let height = self.column_height(column);
            let own = CONCRETE_KN_M3 * column.width * column.depth * height;
            let upper_load_kn = incoming_column_load.get(&column.id).copied().unwrap_or(0.0);
            // The roof (#114) only bears on the top: never multiplied.
            let roof = roof_on_column.get(&column.id).copied().unwrap_or(0.0);
            let load = if use_multiplier {
                (column_load.get(&column.id).copied().unwrap_or(0.0) + own) * floors + roof
            } else {
                column_load.get(&column.id).copied().unwrap_or(0.0) + own + upper_load_kn + roof
            };
            let needed = 1.4 * position.factor() * load / capacity;
            let result = ColumnResult {
                id: column.id,
                label: label(column.id),
                width_m: column.width,
                depth_m: column.depth,
                position,
                load_kn: load,
                upper_load_kn,
                area_needed_cm2: needed,
                area_cm2: column.width * column.depth * 10_000.0,
                height_m: height,
                slenderness: super::wall_heights::slenderness(
                    height,
                    column.width.min(column.depth),
                ),
            };
            if position == ColumnPosition::Isolated {
                report
                    .warnings
                    .push(format!("{}: nenhuma viga chega ao pilar.", result.label));
            } else if !result.slender_ok() {
                report.warnings.push(format!(
                    "{}: esbeltez λ = {:.0} com {} m de altura e {:.0} cm de lado; acima de 90 o pré-dimensionamento não vale: aumente o menor lado para {:.0} cm.",
                    result.label,
                    result.slenderness,
                    decimal(height, 2),
                    column.width.min(column.depth) * 100.0,
                    super::wall_heights::side_for_slenderness(height) * 100.0
                ));
            } else if !result.ok() {
                report.warnings.push(format!(
                    "{}: seção de {} cm² abaixo do necessário ({:.0} cm², mínimo 360 cm² e 14 cm de lado).",
                    result.label,
                    decimal(result.area_cm2, 0),
                    needed.max(360.0)
                ));
            }
            report.columns.push(result);
        }

        // Footings. Balance beams (#144) centre the boundary footings under
        // the lever's reaction and relieve the interior columns.
        // Ground beams (#143) bring the walls' weight down to the footings.
        let ground = self.wall_foundation_effects();
        let balance = self.balance_effects(&report.columns, &ground.at_column);
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        for footing in footings {
            let col_fixture = report
                .columns
                .iter()
                .filter_map(|c| Some((c, self.column(c.id)?)))
                .find(|(_, col)| footing.contains(col.center));
            let column = col_fixture.map(|(c, _)| c);
            let centred = balance.centred_load.get(&footing.id).copied();
            let load = centred.unwrap_or_else(|| {
                column.map_or(0.0, |c| {
                    (c.load_kn + ground.at_column.get(&c.id).copied().unwrap_or(0.0)
                        - balance.relief.get(&c.id).copied().unwrap_or(0.0))
                    .max(0.0)
                })
            });
            let area = 1.05 * load / settings.soil_kpa.max(10.0);
            // Same overhang on all sides of the column.
            let ((cw, cd), (ex, ey)) = col_fixture.map_or(((0.0, 0.0), (0.0, 0.0)), |(_, col)| {
                let dims = if col.quarter_turns % 2 == footing.quarter_turns % 2 {
                    (col.width, col.depth)
                } else {
                    (col.depth, col.width)
                };
                let local = footing.to_local(col.center);
                // The balance beam takes the moment of the offset.
                if centred.is_some() {
                    (dims, (0.0, 0.0))
                } else {
                    (dims, (local.x.abs(), local.y.abs()))
                }
            });
            let diff = cw - cd;
            let depth = ((-diff + (diff * diff + 4.0 * area).sqrt()) / 2.0).max(0.0);
            let needed_depth = round_up(depth, 0.05).max(0.60);
            let needed_width = round_up(depth + diff, 0.05).max(0.60);
            let overhang = ((footing.width - cw).max(footing.depth - cd) / 2.0).max(0.0);
            let height_m = round_up((overhang * 2.0 / 3.0).max(0.25), 0.05);
            let check = check_footing(
                load,
                footing.width,
                footing.depth,
                height_m,
                cw,
                cd,
                ex,
                ey,
                settings.fck_mpa,
            );
            let result = FootingResult {
                id: footing.id,
                label: label(footing.id),
                column: column.map(|c| c.label.clone()),
                width_m: footing.width,
                depth_m: footing.depth,
                load_kn: load,
                needed_width_m: needed_width,
                needed_depth_m: needed_depth,
                height_m,
                eccentricity_m: ex.hypot(ey),
                max_pressure_kpa: check.max_pressure_kpa,
                soil_kpa: settings.soil_kpa,
                punching_ratio: check.punching_ratio,
                overturning_fs: check.overturning_fs,
            };
            if result.column.is_none() {
                report
                    .warnings
                    .push(format!("{}: sapata sem pilar em cima.", result.label));
            } else {
                if result.width_m * result.depth_m + 1e-6
                    < result.needed_width_m * result.needed_depth_m
                {
                    report.warnings.push(format!(
                        "{}: {} m² é pouco para {:.0} kN no solo de {:.0} kPa; use {}.",
                        result.label,
                        decimal(result.width_m * result.depth_m, 2),
                        load,
                        settings.soil_kpa,
                        section_cm(needed_width, needed_depth)
                    ));
                } else if result.max_pressure_kpa > settings.soil_kpa + 1e-3 {
                    report.warnings.push(format!(
                        "{}: excentricidade de {:.0} cm eleva a tensão no solo para {:.0} kPa (limite {:.0} kPa); centralize ou aumente a sapata.",
                        result.label,
                        result.eccentricity_m * 100.0,
                        result.max_pressure_kpa,
                        settings.soil_kpa
                    ));
                }
                if result.overturning_fs < 1.5 {
                    report.warnings.push(format!(
                        "{}: risco de tombamento (FS = {} < 1,5, excentricidade {:.0} cm); centralize a sapata ou use viga de equilíbrio.",
                        result.label,
                        decimal(result.overturning_fs, 2),
                        result.eccentricity_m * 100.0
                    ));
                } else if check.outside_middle_third {
                    report.warnings.push(format!(
                        "{}: carga fora do núcleo central (excentricidade {:.0} cm, descolamento parcial da base).",
                        result.label,
                        result.eccentricity_m * 100.0
                    ));
                }
                if result.punching_ratio > 1.0 + 1e-6 {
                    report.warnings.push(format!(
                        "{}: verificação à punção excedida ({:.0} % do limite NBR 6118); aumente a altura da sapata (h atual {:.0} cm).",
                        result.label,
                        result.punching_ratio * 100.0,
                        result.height_m * 100.0
                    ));
                }
            }
            report.footings.push(result);
        }
        for slab in &report.slabs {
            if !slab.supported {
                report.warnings.push(format!(
                    "{} ({}): nenhuma viga ao longo das bordas da laje.",
                    slab.label, slab.room
                ));
            } else if slab.thickness_m + 1e-6 < slab.thickness_min_m {
                report.warnings.push(format!(
                    "{} ({}): {:.0} cm é pouco para o vão de {} m; use {:.0} cm.",
                    slab.label,
                    slab.room,
                    slab.thickness_m * 100.0,
                    decimal(slab.lx_m, 2),
                    slab.thickness_min_m * 100.0
                ));
            }
        }
        for beam in &balance.results {
            if beam.uplift() {
                report.warnings.push(format!(
                    "{}: o alívio de {} kN levanta o pilar {} ({} kN); aumente o peso no apoio interno ou use sapata corrida.",
                    beam.label,
                    decimal(beam.relief_kn, 1),
                    beam.anchor,
                    decimal(beam.anchor_load_kn, 1)
                ));
            }
            if beam.height_m.is_none() {
                report.warnings.push(format!(
                    "{}: nenhuma seção até 120 cm de altura resiste ao momento de {} kN·m; aproxime o pilar interno ou aumente a largura.",
                    beam.label,
                    decimal(beam.moment_knm, 1)
                ));
            }
        }
        report.balance_beams = balance.results;
        // The survey (#112) may allow less than the pressure adopted.
        if let Some(survey) = &self.structure.soil_survey {
            let widest = report
                .footings
                .iter()
                .map(|f| f.width_m.max(f.depth_m))
                .fold(1.0, f64::max);
            match survey.footing_kpa(widest) {
                Some(kpa) if kpa + 1e-6 < settings.soil_kpa => report.warnings.push(format!(
                    "A sondagem {} indica {:.0} kPa na cota das sapatas ({} m), menos que os {:.0} kPa adotados: use a sondagem na tensão do solo.",
                    survey.name,
                    kpa,
                    decimal(survey.footing_depth_m, 2),
                    settings.soil_kpa
                )),
                None => report.warnings.push(format!(
                    "A sondagem {} não chega à cota das sapatas ({} m).",
                    survey.name,
                    decimal(survey.footing_depth_m, 2)
                )),
                _ => {}
            }
        }
        for result in &ground.results {
            if result.kind == super::wall_foundations::WallFoundationKind::GroundBeam
                && result.height_m.is_none()
            {
                report.warnings.push(format!(
                    "{}: nenhuma seção de baldrame até 80 cm resiste ({} kN·m); acrescente pilares na parede.",
                    result.label,
                    decimal(result.moment_knm, 1)
                ));
            }
            if result.kind == super::wall_foundations::WallFoundationKind::StripFooting
                && result.pressure_kpa > result.soil_kpa + 1e-6
            {
                report.warnings.push(format!(
                    "{}: largura de {:.0} cm é pouca para o solo; use {:.0} cm.",
                    result.label,
                    result.width_m * 100.0,
                    result.needed_width_m * 100.0
                ));
            }
        }
        report.wall_foundations = ground.results;
        report.slabs.sort_by(|a, b| natural(&a.label, &b.label));
        report.beams.sort_by(|a, b| natural(&a.label, &b.label));
        report
    }

    /// Beams and labels as drawn on the formwork plan: the two faces of each
    /// beam between the column faces, and the P, V, L and S labels.
    pub fn structure_marks(&self) -> Vec<PlanMark> {
        let labels = self.structure_labels();
        let mut marks = Vec::new();
        let extent = |column: &Fixture, dir: Point| {
            let (s, c) = column.rotation_rad().sin_cos();
            let u = Point::new(c, s);
            let v = Point::new(-s, c);
            dir.dot(u).abs() * column.width / 2.0 + dir.dot(v).abs() * column.depth / 2.0
        };
        let support_trim = |support: BeamSupport, dir: Point| match support {
            BeamSupport::Column { id } => self.column(id).map_or(0.0, |c| extent(c, dir)),
            BeamSupport::Beam { beam_id, .. } => {
                self.beam_by_id(beam_id).map_or(0.0, |b| b.width_m / 2.0)
            }
        };
        let mut labelled: HashSet<String> = HashSet::new();
        let beam_map: HashMap<u64, Beam> = self.valid_beams().map(|b| (b.id, *b)).collect();
        let lines = self.beam_lines();
        for line in &lines {
            // The longest span carries the line's label.
            let mut longest: Option<(f64, Point, Point, Beam)> = None;
            for id in &line.spans {
                let Some(&beam) = beam_map.get(id) else {
                    continue;
                };
                let Some((pa, pb)) = self.beam_endpoints(&beam) else {
                    continue;
                };
                let Some(dir) = pb.minus(pa).normalized() else {
                    continue;
                };
                let normal = Point::new(-dir.y, dir.x);
                let start = pa.translated(dir.scaled(support_trim(beam.start_support(), dir)));
                let end = pb.translated(dir.scaled(-support_trim(beam.end_support(), dir)));
                for side in [-1.0, 1.0] {
                    let off = normal.scaled(side * beam.width_m / 2.0);
                    marks.push(PlanMark::Line {
                        points: vec![start.translated(off), end.translated(off)],
                        dashed: false,
                        heavy: true,
                    });
                }
                let length = pa.distance(pb);
                if longest.is_none_or(|(l, _, _, _)| length > l) {
                    longest = Some((length, pa, pb, beam));
                }
            }
            if let Some((_, a, b, beam)) = longest
                && labelled.insert(line.label.clone())
            {
                let dir = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
                let mut normal = Point::new(-dir.y, dir.x);
                if normal.y > 0.0 || (normal.y.abs() < 1e-6 && normal.x > 0.0) {
                    normal = normal.scaled(-1.0);
                }
                let text = format!(
                    "{} ({})",
                    line.label,
                    section_cm(beam.width_m, beam.height_m)
                );
                let (w, _) = super::electrical::tag_size(&text);
                let reach = beam.width_m / 2.0 + 0.16 + normal.x.abs() * w / 2.0;
                marks.push(PlanMark::Text {
                    anchor: a.midpoint(b).translated(normal.scaled(reach)),
                    text,
                });
            }
        }
        for column in self.structural_columns() {
            if let Some(text) = labels.get(&column.id) {
                let text = format!("{text} {}", section_cm(column.width, column.depth));
                let (w, _) = super::electrical::tag_size(&text);
                let (hw, hd) = match column.quarter_turns % 2 {
                    0 => (column.width / 2.0, column.depth / 2.0),
                    _ => (column.depth / 2.0, column.width / 2.0),
                };
                marks.push(PlanMark::Text {
                    anchor: column
                        .center
                        .translated(Point::new(hw + w / 2.0 + 0.05, -hd - 0.12)),
                    text,
                });
            }
        }
        for footing in self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
        {
            if let Some(text) = labels.get(&footing.id) {
                let text = format!("{text} {}", section_cm(footing.width, footing.depth));
                let (w, _) = super::electrical::tag_size(&text);
                let (hw, hd) = match footing.quarter_turns % 2 {
                    0 => (footing.width / 2.0, footing.depth / 2.0),
                    _ => (footing.depth / 2.0, footing.width / 2.0),
                };
                marks.push(PlanMark::Text {
                    anchor: footing
                        .center
                        .translated(Point::new(hw - w / 2.0, hd + 0.14)),
                    text,
                });
            }
        }
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                continue;
            };
            let Some((anchor, _, _)) = self.room_label_anchor(room) else {
                continue;
            };
            if let Some(text) = labels.get(&slab.id) {
                marks.push(PlanMark::Text {
                    anchor: anchor.translated(Point::new(0.0, 0.45)),
                    text: format!(
                        "{text} h={:.0} ({})",
                        slab.thickness_m * 100.0,
                        slab.kind.label().to_lowercase()
                    ),
                });
            }
        }
        for load in self.fixtures.iter().filter(|fixture| {
            matches!(
                fixture.kind,
                FixtureKind::WaterTank | FixtureKind::HeavyEquipment
            )
        }) {
            let Some(text) = labels.get(&load.id) else {
                continue;
            };
            let value = load.structural_load_kn.unwrap_or(0.0);
            marks.push(PlanMark::Text {
                anchor: load
                    .center
                    .translated(Point::new(0.0, load.depth / 2.0 + 0.16)),
                text: format!("{text} {} kN", decimal(value, 1)),
            });
        }
        // Balance beams, dashed (below ground).
        let report = self.cached_structure_report();
        marks.extend(self.balance_beam_marks(&report.balance_beams));
        marks.extend(self.wall_foundation_marks(&report.wall_foundations));
        marks.extend(self.roof_marks(&report.roofs));
        marks.extend(self.post_marks(&super::timber_posts::PostEffects {
            posts: report.timber_posts.clone(),
            eave_beams: report.eave_beams.clone(),
        }));
        super::label_layout::resolve_plan_mark_labels(&mut marks, &[]);
        marks
    }
}

fn validate_section(width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite() || !(0.08..=1.0).contains(&width) {
        return Err("A largura da viga deve ficar entre 8 e 100 cm.".to_owned());
    }
    if !height.is_finite() || !(0.15..=2.0).contains(&height) {
        return Err("A altura da viga deve ficar entre 15 e 200 cm.".to_owned());
    }
    Ok(())
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// "V2" before "V10".
fn natural(a: &str, b: &str) -> Ordering {
    let split = |s: &str| {
        let digits: String = s.chars().filter(char::is_ascii_digit).collect();
        let letters: String = s.chars().filter(|c| !c.is_ascii_digit()).collect();
        (letters, digits.parse::<u64>().unwrap_or(0))
    };
    split(a).cmp(&split(b))
}

/// The columns with their section, load and footing, for drawing sheets.
pub fn build_columns_schedule(project: &Project) -> super::schedules::ScheduleTable {
    let report = project.structure_report();
    let rows = report
        .columns
        .iter()
        .map(|column| {
            let footing = report
                .footings
                .iter()
                .find(|f| f.column.as_deref() == Some(column.label.as_str()));
            vec![
                column.label.clone(),
                section_cm(column.width_m, column.depth_m),
                format!("{:.0}", column.load_kn),
                footing.map_or("—".to_owned(), |f| {
                    format!("{} {}", f.label, section_cm(f.width_m, f.depth_m))
                }),
            ]
        })
        .collect();
    let s = project.structure.settings;
    super::schedules::ScheduleTable {
        title: format!(
            "Quadro de pilares e sapatas · fck {:.0} MPa · solo {:.0} kPa",
            s.fck_mpa, s.soil_kpa
        ),
        headers: ["Pilar", "Seção (cm)", "Carga (kN)", "Sapata (cm)"]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
        rows,
        footer: None,
    }
}

/// The internal forces of every beam as diagrams: each continuous beam with
/// its supports, the bending moment diagram (hogging up, sagging down) and
/// the values at supports and spans, plus the deflection of each span.
pub fn beam_diagrams_svg(project: &Project) -> String {
    let report = project.structure_report();
    let width = 420.0;
    let row = 62.0;
    let height = (40.0 + report.beams.len() as f64 * row + 20.0).max(297.0);
    let ink = "#1d2836";
    let blue = "#1565c0";
    let red = "#c62828";
    let font = "font-family=\"Arial, sans-serif\"";
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                anchor: &str,
                color: &str,
                value: &str| {
        svg.push_str(&format!(
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\" text-anchor=\"{anchor}\">{}</text>\n",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        ));
    };
    text(
        &mut svg,
        20.0,
        24.0,
        5.0,
        true,
        "start",
        ink,
        "ESFORÇOS NAS VIGAS · MOMENTO FLETOR, CORTANTE E FLECHA",
    );
    let s = project.structure.settings;
    text(
        &mut svg,
        20.0,
        31.0,
        3.0,
        false,
        "start",
        ink,
        &format!(
            "{} · fck {:.0} MPa · sobrecarga {} kN/m² · revestimento {} kN/m² · {} pavimento(s) · valores característicos",
            project.name,
            s.fck_mpa,
            decimal(s.live_kn_m2, 1),
            decimal(s.finish_kn_m2, 1),
            s.floors.max(1)
        ),
    );
    if report.beams.is_empty() {
        text(
            &mut svg,
            20.0,
            48.0,
            3.6,
            false,
            "start",
            ink,
            "Nenhuma viga ainda: gere a estrutura ou ligue os pilares.",
        );
        svg.push_str("</svg>\n");
        return svg;
    }
    let left = 40.0;
    let usable = width - 40.0 - 120.0;
    for (index, line) in report.beams.iter().enumerate() {
        let top = 44.0 + index as f64 * row;
        let total: f64 = line.spans.iter().map(|s| s.length_m).sum::<f64>().max(0.1);
        let scale = usable / total;
        let axis = top + 22.0;
        text(
            &mut svg,
            20.0,
            axis + 1.2,
            3.4,
            true,
            "start",
            ink,
            &line.label,
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.2}\" y1=\"{axis:.2}\" x2=\"{:.2}\" y2=\"{axis:.2}\" stroke=\"{ink}\" stroke-width=\"0.6\"/>\n",
            left + total * scale
        ));
        let peak = line
            .spans
            .iter()
            .flat_map(|s| {
                [
                    s.moment_left.abs(),
                    s.moment_right.abs(),
                    s.moment_span.abs(),
                ]
            })
            .fold(0.1, f64::max);
        let mscale = 14.0 / peak;
        let mut x0 = left;
        let mut path = format!("M{left:.2} {axis:.2}");
        for (i, span) in line.spans.iter().enumerate() {
            let w = span.dead_kn_m + span.live_kn_m;
            let length = span.length_m;
            let v_left = w * length / 2.0 + (span.moment_right - span.moment_left) / length;
            for k in 0..=24 {
                let x = length * f64::from(k) / 24.0;
                let m = span.moment_left + v_left * x - w * x * x / 2.0;
                // Hogging drawn above the axis, sagging below (tension side).
                path.push_str(&format!(" L{:.2} {:.2}", x0 + x * scale, axis + m * mscale));
            }
            let mid = x0 + length * scale / 2.0;
            text(
                &mut svg,
                mid,
                axis + span.moment_span * mscale + 4.0,
                2.4,
                false,
                "middle",
                blue,
                &format!("M+ {}", decimal(span.moment_span, 1)),
            );
            // Short spans only get their length, so the texts do not meet.
            let caption = if length * scale < 75.0 {
                format!("{} m", decimal(length, 2))
            } else {
                format!(
                    "{} m · {} kN/m · {} · V {} kN · flecha {} mm",
                    decimal(length, 2),
                    decimal(w, 1),
                    section_cm(span.width_m, span.height_m),
                    decimal(span.shear, 1),
                    decimal(span.deflection_mm, 1)
                )
            };
            text(
                &mut svg,
                mid,
                top + 4.0,
                2.4,
                false,
                "middle",
                ink,
                &caption,
            );
            if i == 0 && span.moment_left.abs() > 0.05 {
                text(
                    &mut svg,
                    x0,
                    axis + span.moment_left * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_left, 1),
                );
            }
            if span.moment_right.abs() > 0.05 {
                text(
                    &mut svg,
                    x0 + length * scale,
                    axis + span.moment_right * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_right, 1),
                );
            }
            x0 += length * scale;
        }
        path.push_str(&format!(" L{x0:.2} {axis:.2} Z"));
        svg.push_str(&format!(
            "<path d=\"{path}\" fill=\"{blue}\" fill-opacity=\"0.15\" stroke=\"{blue}\" stroke-width=\"0.35\"/>\n"
        ));
        // Supports with their column label and reaction.
        let labels = project.structure_labels();
        let mut x = left;
        for (j, joint) in line.joints.iter().enumerate() {
            svg.push_str(&format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>\n",
                x,
                axis,
                x - 2.0,
                axis + 3.0,
                x + 2.0,
                axis + 3.0
            ));
            text(
                &mut svg,
                x,
                axis + 7.0,
                2.3,
                true,
                "middle",
                ink,
                labels.get(joint).map_or("?", String::as_str),
            );
            if let Some(r) = line.reactions.get(j) {
                text(
                    &mut svg,
                    x,
                    axis + 10.5,
                    2.2,
                    false,
                    "middle",
                    ink,
                    &format!("R {} kN", decimal(*r, 1)),
                );
            }
            if let Some(span) = line.spans.get(j) {
                x += span.length_m * scale;
            }
        }
        // Summary on the right.
        let sx = width - 112.0;
        let worst = line
            .spans
            .iter()
            .map(|s| s.moment_left.min(s.moment_right))
            .fold(0.0, f64::min);
        let best = line.spans.iter().map(|s| s.moment_span).fold(0.0, f64::max);
        let shear = line.spans.iter().map(|s| s.shear).fold(0.0, f64::max);
        let ok = line
            .spans
            .iter()
            .all(|s| s.deflection_mm <= s.limit_mm && s.height_m + 1e-6 >= s.height_min_m);
        for (k, item) in [
            format!("M− máx: {} kN·m", decimal(worst, 1)),
            format!("M+ máx: {} kN·m", decimal(best, 1)),
            format!("V máx: {} kN", decimal(shear, 1)),
            if ok {
                "Altura e flecha: atendem".to_owned()
            } else {
                "Altura ou flecha: revise".to_owned()
            },
        ]
        .iter()
        .enumerate()
        {
            text(
                &mut svg,
                sx,
                top + 12.0 + k as f64 * 5.0,
                2.7,
                k == 3,
                "start",
                if k == 3 && !ok { red } else { ink },
                item,
            );
        }
    }
    let notes_y = height - 22.0;
    for (i, note) in [
        "Vigas contínuas sobre apoios simples (pilares), carga uniforme por vão: peso próprio, lajes repartidas por charneiras plásticas (45°) e paredes dos pavimentos de cima.",
        "Flecha com carga quase permanente (g + 0,3q), inércia equivalente de Branson (Mr e armadura CA-50) e fluência (1 + 1,32), comparada a L/250 (NBR 6118).",
        "Pré-dimensionamento para estudo: o projeto estrutural final é de um engenheiro habilitado (ART).",
    ]
    .iter()
    .enumerate()
    {
        text(&mut svg, 20.0, notes_y + i as f64 * 4.0, 2.3, false, "start", ink, note);
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
            .unwrap();
        project
    }

    #[test]
    fn a_continuous_beam_matches_the_textbook() {
        // Two equal spans with a uniform load: -wL²/8 over the middle
        // support, 3wL/8 at the ends and 10wL/8 in the middle.
        let (w, l) = (10.0, 4.0);
        let moments = continuous_beam(&[(l, w, 1.0), (l, w, 1.0)]);
        assert!(moments[0].0.abs() < 1e-9);
        assert!((moments[0].1 + w * l * l / 8.0).abs() < 1e-9);
        assert!((moments[1].0 - moments[0].1).abs() < 1e-9);
        // One simply supported span: 5wL⁴/384EI at mid-span.
        let d = span_deflection(l, w, 0.0, 0.0, 1000.0);
        assert!(
            (d - 5.0 * w * l.powi(4) / 384.0 / 1000.0).abs() < 1e-4,
            "{d}"
        );
        assert_eq!(SlabKind::Ribbed.minimum_thickness(3.2), 0.12);
        assert_eq!(SlabKind::Solid.minimum_thickness(4.4), 0.11);
        assert_eq!(natural("V2", "V10"), Ordering::Less);
    }

    #[test]
    fn generating_lays_columns_beams_slabs_and_footings() {
        let mut project = house();
        let wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - w.end.y).abs() < 1e-6 && w.start.y.abs() < 1e-6)
            .unwrap()
            .id;
        project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
        let report = project.generate_structure();
        // Six junctions of the two rooms.
        assert_eq!(report.columns, 6, "{report:?}");
        assert_eq!(report.beams, 7);
        assert_eq!(report.slabs, 2);
        assert_eq!(report.footings, 6);
        // Again, nothing new.
        assert_eq!(project.generate_structure().total(), 0);
        let labels = project.structure_labels();
        let first = project
            .structural_columns()
            .min_by(|a, b| reading(a.center, b.center))
            .unwrap();
        assert_eq!(labels[&first.id], "P1");
        // Two horizontal continuous beams (V1, V2) and three vertical ones.
        let lines = project.beam_lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].label, "V1");
        assert_eq!(lines[0].spans.len(), 2);

        let structure = project.structure_report();
        assert!(
            structure.slabs.iter().all(SlabResult::ok),
            "{:?}",
            structure.slabs
        );
        assert!(structure.columns.iter().all(|c| c.load_kn > 0.0));
        assert!(structure.footings.iter().all(FootingResult::ok));
        // The slab loads all reach the columns: the reactions add up to the
        // slabs, the beams and nothing else (one floor, no walls above).
        let slabs: f64 = project
            .structure
            .slabs
            .iter()
            .map(|slab| {
                let room = project.rooms.iter().find(|r| r.id == slab.room_id).unwrap();
                let result = structure.slabs.iter().find(|r| r.id == slab.id).unwrap();
                result.load_kn_m2 * project.room_area(room).unwrap()
            })
            .sum();
        let beams: f64 = structure
            .beams
            .iter()
            .flat_map(|l| &l.spans)
            .map(|s| CONCRETE_KN_M3 * s.width_m * s.height_m * s.length_m)
            .sum();
        let reactions: f64 = structure.beams.iter().flat_map(|l| &l.reactions).sum();
        assert!(
            ((slabs + beams) - reactions).abs() < 0.01 * reactions,
            "{slabs} + {beams} vs {reactions}"
        );
        assert!(structure.warnings.is_empty(), "{:?}", structure.warnings);
        // The plan shows the beams and every label.
        let marks = project.structure_marks();
        for label in ["P1", "V1", "L1", "S1"] {
            assert!(
                marks
                    .iter()
                    .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with(&format!("{label} ")))),
                "{label}"
            );
        }
        let svg = beam_diagrams_svg(&project);
        assert!(svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok());
        assert!(svg.contains(">V1<"));
    }

    #[test]
    fn concentrated_load_flows_from_slab_to_beams_columns_and_footings() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        let room = project.rooms[0].id;
        project.set_slab(room, SlabKind::Solid, Some(0.12)).unwrap();
        let before = project.structure_report();
        let before_columns: f64 = before.columns.iter().map(|column| column.load_kn).sum();
        let before_moment = before
            .beams
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.moment_span)
            .fold(0.0, f64::max);

        let tank = project
            .add_fixture(FixtureKind::WaterTank, Point::new(2.0, 2.0), 1.20, 1.20, 0)
            .unwrap();
        let load_kn = project
            .fixtures
            .iter()
            .find(|fixture| fixture.id == tank)
            .and_then(|fixture| fixture.structural_load_kn)
            .unwrap();
        let after = project.structure_report();
        let after_columns: f64 = after.columns.iter().map(|column| column.load_kn).sum();
        let after_moment = after
            .beams
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.moment_span)
            .fold(0.0, f64::max);

        assert!((after_columns - before_columns - load_kn).abs() < 1e-6);
        assert!(after_moment > before_moment);
        assert_eq!(after.concentrated_loads.len(), 1);
        let result = &after.concentrated_loads[0];
        assert_eq!(result.label, "CC1");
        assert!(result.supported && result.slab.is_some());
        assert!(result.local_moment_knm_m > 0.0);
        assert!(result.punching_ratio.is_some());
        assert_eq!(after.slabs[0].point_load_kn, load_kn);
        assert!(
            crate::domain::verdict::structure_verdicts(&project, &after)
                .iter()
                .any(|verdict| verdict.element.starts_with("Carga CC1"))
        );
        assert!(
            project.structure_marks().iter().any(
                |mark| matches!(mark, PlanMark::Text { text, .. } if text.starts_with("CC1 "))
            )
        );

        project
            .update_fixture(tank, Some(Point::new(8.0, 8.0)), None, None)
            .unwrap();
        let unsupported = project.structure_report();
        assert!(!unsupported.concentrated_loads[0].supported);
        assert!(
            unsupported
                .warnings
                .iter()
                .any(|warning| warning.contains("sem laje ou viga"))
        );
    }

    #[test]
    fn a_load_directly_on_a_beam_skips_the_slab_local_check() {
        let mut project = house();
        project.generate_structure();
        let beam = project.structure.beams[0];
        let (a, b) = project.beam_endpoints(&beam).unwrap();
        project
            .add_fixture(FixtureKind::HeavyEquipment, a.midpoint(b), 0.40, 0.40, 0)
            .unwrap();
        let report = project.structure_report();
        let load = &report.concentrated_loads[0];
        assert!(load.supported);
        assert!(load.slab.is_none());
        assert!(load.support.contains("direto na viga"));
        assert_eq!(load.punching_ratio, None);
    }

    #[test]
    fn undersized_elements_are_flagged() {
        let mut project = house();
        project.generate_structure();
        let slab = project.structure.slabs[0];
        project
            .set_slab(slab.room_id, SlabKind::Solid, Some(0.06))
            .unwrap();
        let beam = project.structure.beams[0].id;
        project.update_beam(beam, None, Some(0.15)).unwrap();
        project.structure.settings.floors = 6;
        let report = project.structure_report();
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("é pouco para o vão"))
        );
        assert!(report.warnings.iter().any(|w| w.contains("pede altura")));
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("seção") || w.contains("m² é pouco"))
        );
        // Bad sections are refused.
        assert!(project.update_beam(beam, Some(0.02), None).is_err());
        assert!(
            project
                .set_slab(slab.room_id, SlabKind::Solid, Some(2.0))
                .is_err()
        );
    }

    #[test]
    fn each_floor_keeps_its_beams_slabs_and_conduits() {
        // Regression: switching floors left these behind, and the pruning
        // on the next edit deleted them.
        let mut project = house();
        project.generate_structure();
        let (w, d) = FixtureKind::Panel.default_size();
        let panel = project
            .add_fixture(FixtureKind::Panel, Point::new(0.5, 0.2), w, d, 0)
            .unwrap();
        let (w, d) = FixtureKind::CeilingLight.default_size();
        let light = project
            .add_fixture(FixtureKind::CeilingLight, Point::new(2.0, 2.0), w, d, 0)
            .unwrap();
        project
            .add_conduit(panel, light, crate::domain::conduits::ConduitRun::Ceiling)
            .unwrap();
        let (beams, slabs) = (
            project.structure.beams.clone(),
            project.structure.slabs.clone(),
        );
        let conduits = project.conduits.clone();
        let ground = project.ensure_default_floor();
        project.add_floor("Superior", true, false);
        assert!(project.structure.beams.is_empty() && project.conduits.is_empty());
        // An edit upstairs prunes, and must not touch the floor below.
        project.prune_conduits();
        project.prune_structure();
        project.switch_floor(ground).unwrap();
        assert_eq!(project.structure.beams, beams);
        assert_eq!(project.structure.slabs, slabs);
        assert_eq!(project.conduits, conduits);
    }

    #[test]
    fn columns_and_footings_on_a_boundary_wall_stand_flush_inside_the_lot() {
        // B23: a room against the left lot line (x = 0).
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(0.0, 2.0), Point::new(5.0, 6.0), "Gourmet")
            .unwrap();
        // Placing a column on that wall moves it inside instead of refusing it.
        let (at, turns, angle) = project.structural_placement_fitted(
            FixtureKind::Column,
            Point::new(0.05, 4.0),
            0.30,
            0.14,
        );
        assert!(
            project
                .add_fixture(FixtureKind::Column, at, 0.30, 0.14, turns)
                .is_some()
        );
        assert!((at.x - 0.07).abs() < 0.002, "{at:?}");
        let _ = angle;
        // Generating lays every corner column, boundary ones flush, and
        // reports nothing skipped; the beams still find them.
        let mut generated = Project::default();
        generated.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        generated
            .add_room(Point::new(0.0, 2.0), Point::new(4.0, 6.0), "Gourmet")
            .unwrap();
        let report = generated.generate_structure();
        assert_eq!(report.columns, 4, "{report:?}");
        assert_eq!(report.skipped, 0);
        assert_eq!(report.beams, 4);
        let boundary: Vec<_> = generated
            .structural_columns()
            .filter(|c| c.center.x < 1.0)
            .collect();
        assert_eq!(boundary.len(), 2);
        for column in &boundary {
            let (low, _) = column.bounds();
            assert!(low.x >= -1e-9, "inside the lot: {column:?}");
        }
        // Their footings sit inside the lot too, off centre from the column.
        for footing in generated
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
        {
            let (low, _) = footing.bounds();
            assert!(low.x >= -1e-9, "{footing:?}");
        }
        let report = generated.structure_report();
        assert!(
            report.footings.iter().any(|f| f.eccentricity_m > 0.1),
            "boundary footings are eccentric"
        );
    }

    #[test]
    fn beams_go_with_their_columns() {
        let mut project = house();
        project.generate_structure();
        let column = project.structural_columns().next().unwrap().id;
        let attached = project.beams_of(&[column]).len();
        assert!(attached >= 2);
        let before = project.structure.beams.len();
        project.delete_fixture(column);
        assert!(project.beams_of(&[column]).is_empty());
        assert_eq!(project.structure.beams.len(), before - attached);
        // Removed any other way, the pruning on each edit catches it.
        let other = project.structural_columns().next().unwrap().id;
        let left = project.beams_of(&[other]).len();
        project.fixtures.retain(|f| f.id != other);
        assert_eq!(project.prune_structure(), left);
        // A column placed near a junction snaps onto it.
        let (at, _) = project.structural_placement(Point::new(4.2, 0.1));
        assert!(at.distance(Point::new(4.0, 0.0)) < 1e-6, "{at:?}");
    }

    #[test]
    fn branson_deflection_uses_uncracked_inertia_below_mr_and_cracked_as_above_mr() {
        let settings = StructureSettings::default(); // fck = 25 MPa
        let modulus = settings.modulus_kn_m2();
        let (b, h): (f64, f64) = (0.15, 0.40);
        let ic = b * h.powi(3) / 12.0;

        // Light load below cracking moment Mr: beam stays uncracked (I_eq == I_c),
        // unlike the old fixed 0.5 * I_c approximation.
        let uncracked = effective_beam_stiffness(b, h, settings.fck_mpa, modulus, 5.0, 4.0);
        assert!((uncracked.i_eq_m4 - ic).abs() < 1e-12);
        assert!(uncracked.m_r_knm > 10.0);
        assert!(uncracked.as_cm2 >= 0.0015 * b * h * 10_000.0 - 1e-6);

        // Heavy service moment well above Mr: I_eq drops smoothly toward the
        // cracked transformed inertia I_II computed from the pre-sized steel.
        let cracked = effective_beam_stiffness(b, h, settings.fck_mpa, modulus, 60.0, 48.0);
        assert!(cracked.i_eq_m4 < ic);
        assert!(cracked.i_eq_m4 >= cracked.i_cracked_m4 - 1e-12);
        assert!(cracked.as_cm2 > uncracked.as_cm2);
    }

    #[test]
    fn footings_check_eccentric_pressure_overturning_and_punching_shear() {
        // Concentric well-proportioned footing passes all checks.
        let concentric = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.0, 0.0, 25.0);
        assert!(!concentric.outside_middle_third);
        assert!(concentric.overturning_fs.is_infinite());
        assert!(concentric.max_pressure_kpa <= 110.0);
        assert!(concentric.punching_ratio < 1.0);

        // Large eccentricity pushes load outside the middle third, raises peak
        // soil pressure and drops overturning FS below 1.5.
        let eccentric = check_footing(100.0, 1.0, 1.0, 0.35, 0.30, 0.14, 0.36, 0.0, 25.0);
        assert!(eccentric.outside_middle_third);
        assert!(eccentric.overturning_fs < 1.5);
        assert!(eccentric.max_pressure_kpa > 300.0);

        // Heavy load on a shallow footing triggers punching shear failure.
        let punched = check_footing(600.0, 1.8, 1.8, 0.25, 0.30, 0.14, 0.0, 0.0, 25.0);
        assert!(punched.punching_ratio > 1.0);

        // Integration check on a project with an off-centre footing.
        let mut project = house();
        project.generate_structure();
        let footing_id = project
            .fixtures
            .iter()
            .find(|f| f.kind == FixtureKind::Footing)
            .unwrap()
            .id;
        // Shift footing so the column sits near its edge (eccentric).
        if let Some(f) = project.fixtures.iter_mut().find(|f| f.id == footing_id) {
            f.width = 1.0;
            f.depth = 1.0;
            f.center = f.center.translated(Point::new(0.40, 0.0));
        }
        let report = project.structure_report();
        let res = report.footings.iter().find(|f| f.id == footing_id).unwrap();
        assert!(!res.ok());
        assert!(res.overturning_fs < 1.5);
        assert!(report.warnings.iter().any(|w| w.contains("tombamento")));
    }

    #[test]
    fn l_shaped_slab_uses_panel_spans_and_yield_line_load_distribution() {
        // An L-shaped room with 3 m wings inside a 6 × 6 m bounding box must
        // report lx = 3.0 m (wing width), not 6.0 m (bounding box).
        let l_outline = vec![
            Point::new(0.0, 0.0),
            Point::new(6.0, 0.0),
            Point::new(6.0, 3.0),
            Point::new(3.0, 3.0),
            Point::new(3.0, 6.0),
            Point::new(0.0, 6.0),
        ];
        let panels = decompose_slab_panels(&l_outline);
        assert_eq!(panels.len(), 2);
        let lx = panels.iter().map(|(x, _)| *x).fold(0.0_f64, f64::max);
        assert!((lx - 3.0).abs() < 1e-6, "got lx = {lx}");

        // 3 × 6 m rectangular slab with 45° yield lines:
        // short edges take lx²/4 = 2.25 / 18 = 12.5% each,
        // long edges take (lx/2)*(ly - lx/2) = 6.75 / 18 = 37.5% each.
        let rect = vec![
            Point::new(0.0, 0.0),
            Point::new(6.0, 0.0),
            Point::new(6.0, 3.0),
            Point::new(0.0, 3.0),
        ];
        let supported = vec![
            (1, Point::new(0.0, 0.0), Point::new(6.0, 0.0)), // long
            (2, Point::new(6.0, 0.0), Point::new(6.0, 3.0)), // short
            (3, Point::new(6.0, 3.0), Point::new(0.0, 3.0)), // long
            (4, Point::new(0.0, 3.0), Point::new(0.0, 0.0)), // short
        ];
        let shares = yield_line_shares(&rect, &supported);
        assert!((shares[&1] - 0.375).abs() < 1e-6, "long: {}", shares[&1]);
        assert!((shares[&3] - 0.375).abs() < 1e-6, "long: {}", shares[&3]);
        assert!((shares[&2] - 0.125).abs() < 1e-6, "short: {}", shares[&2]);
        assert!((shares[&4] - 0.125).abs() < 1e-6, "short: {}", shares[&4]);
        let sum: f64 = shares.values().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn angled_columns_and_beam_on_beam_supports_transfer_loads_accurately() {
        // 1. Angled wall at 45°: generated columns inherit the 45° angle instead
        // of snapping to quarter turns.
        let mut angled = Project::default();
        angled.set_terrain(Point::new(-2.0, -2.0), Point::new(10.0, 10.0));
        angled
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 3.0))
            .unwrap();
        angled.generate_structure();
        let col = angled.structural_columns().next().unwrap();
        assert!(
            (col.rotation_rad() - std::f64::consts::FRAC_PI_4).abs() < 1e-4,
            "expected 45° column, got {} rad",
            col.rotation_rad()
        );
        // Local +X axis of the column runs along the 45° wall.
        let tip = col.to_world(Point::new(0.15, 0.0));
        let delta = tip.minus(col.center);
        assert!((delta.x - delta.y).abs() < 1e-6);

        // 2. Secondary beam resting on two primary girders transfers its exact
        // self-weight to the girders and down to the four corner columns.
        let mut frame = Project::default();
        frame.set_terrain(Point::new(-2.0, -2.0), Point::new(12.0, 12.0));
        let c1 = frame
            .add_fixture(FixtureKind::Column, Point::new(0.0, 0.0), 0.30, 0.14, 0)
            .unwrap();
        let c2 = frame
            .add_fixture(FixtureKind::Column, Point::new(4.0, 0.0), 0.30, 0.14, 0)
            .unwrap();
        let c3 = frame
            .add_fixture(FixtureKind::Column, Point::new(0.0, 4.0), 0.30, 0.14, 0)
            .unwrap();
        let c4 = frame
            .add_fixture(FixtureKind::Column, Point::new(4.0, 4.0), 0.30, 0.14, 0)
            .unwrap();
        let g1 = frame.add_beam(c1, c2, 0.15, 0.40).unwrap();
        let g2 = frame.add_beam(c3, c4, 0.15, 0.40).unwrap();

        let before = frame.structure_report();
        let g1_moment_before = before
            .beams
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.beam == g1)
            .unwrap()
            .moment_span;

        // Add a secondary beam spanning from midpoint of g1 to midpoint of g2.
        let sec = frame
            .add_beam_on_supports(
                BeamSupport::Beam {
                    beam_id: g1,
                    t: 0.5,
                },
                BeamSupport::Beam {
                    beam_id: g2,
                    t: 0.5,
                },
                0.15,
                0.35,
            )
            .unwrap();
        let after = frame.structure_report();
        let g1_span_after = after
            .beams
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.beam == g1)
            .unwrap();
        let sec_weight = CONCRETE_KN_M3 * 0.15 * 0.35 * 4.0;
        // Point load at mid-span increases primary girder's mid-span moment by P*L/4.
        let expected_extra_moment = (sec_weight / 2.0) * 4.0 / 4.0;
        assert!(
            ((g1_span_after.moment_span - g1_moment_before) - expected_extra_moment).abs() < 0.05,
            "extra moment {} vs expected {}",
            g1_span_after.moment_span - g1_moment_before,
            expected_extra_moment
        );
        // Deleting a column removes its girder AND the secondary beam resting on it.
        let affected = frame.beams_of(&[c1]);
        assert!(affected.contains(&g1) && affected.contains(&sec));
    }

    #[test]
    fn upper_floor_loads_flow_down_to_aligned_columns_and_flag_misaligned_transfers() {
        let mut project = house();
        project.generate_structure();
        let single_floor = project.structure_report();
        let single_total_col_load: f64 = single_floor.columns.iter().map(|c| c.load_kn).sum();

        // Create a real second storey with its own structure aligned over the ground floor.
        let ground = project.ensure_default_floor();
        let _upper = project.add_floor("Superior", true, true);
        project.generate_structure();
        // Switch back to ground floor and verify upper loads accumulate naturally
        // even with `settings.floors == 1`.
        project.switch_floor(ground).unwrap();
        let two_storey = project.structure_report();
        assert!(two_storey.misaligned_columns.is_empty());
        assert!(two_storey.columns.iter().all(|c| c.upper_load_kn > 0.0));
        let two_storey_total: f64 = two_storey.columns.iter().map(|c| c.load_kn).sum();
        assert!(
            two_storey_total > single_total_col_load * 1.8,
            "expected real upper floor load to add up: {two_storey_total} vs {single_total_col_load}"
        );

        // Now add a misaligned column on the upper floor sitting at the midpoint
        // of a ground-floor beam (transfer girder) and another over open space.
        let upper_id = project
            .floors
            .levels
            .iter()
            .find(|l| l.id != ground)
            .unwrap()
            .id;
        project.switch_floor(upper_id).unwrap();
        let on_beam = project
            .add_fixture(FixtureKind::Column, Point::new(2.0, 0.0), 0.30, 0.14, 0)
            .unwrap();
        let floating = project
            .add_fixture(FixtureKind::Column, Point::new(2.0, 2.0), 0.30, 0.14, 0)
            .unwrap();
        let _ = (on_beam, floating);
        project.switch_floor(ground).unwrap();

        let with_transfers = project.structure_report();
        assert_eq!(with_transfers.misaligned_columns.len(), 2);
        assert!(
            with_transfers
                .misaligned_columns
                .iter()
                .any(|m| m.transfer_beam.is_some())
        );
        assert!(
            with_transfers
                .misaligned_columns
                .iter()
                .any(|m| m.transfer_beam.is_none())
        );
        assert!(
            with_transfers
                .warnings
                .iter()
                .any(|w| w.contains("viga de transição"))
        );
        assert!(
            with_transfers
                .warnings
                .iter()
                .any(|w| w.contains("sem prumada abaixo"))
        );
    }
}
