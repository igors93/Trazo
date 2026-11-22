//! Structural data model: beams, slabs, soil/occupancy classes and settings.

use serde::{Deserialize, Serialize};

use crate::domain::fixtures::FixtureKind;

use super::{CONCRETE_KN_M3, round_up};

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
    /// 2D plane-frame analysis (#94): monolithic rigid beam-column joints (`true`)
    /// or pinned column-top joints (`false`).
    #[serde(default = "default_true")]
    pub frame_rigid_joints: bool,
    /// 2D plane-frame analysis (#94): fixed column bases (`true`) or pinned bases (`false`).
    #[serde(default = "default_true")]
    pub frame_fixed_bases: bool,
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
            frame_rigid_joints: true,
            frame_fixed_bases: true,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStructure {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<Beam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slabs: Vec<Slab>,
    #[serde(default, skip_serializing_if = "StructureSettings::is_default")]
    pub settings: StructureSettings,
    /// Balance beams from boundary columns to interior ones (#144).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balance_beams: Vec<crate::domain::foundations::BalanceBeam>,
    /// Ground beams and strip footings under the walls (#143).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wall_foundations: Vec<crate::domain::wall_foundations::WallFoundation>,
    /// The site's soil survey (SPT), shared by every floor (#112).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soil_survey: Option<crate::domain::soil::SoilSurvey>,
    /// Timber roofs over this floor (#114).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roofs: Vec<crate::domain::roof::Roof>,
    /// Species (or certified strength class) of timber posts and eave beams (#146).
    #[serde(
        default,
        alias = "post_timber",
        skip_serializing_if = "is_default_timber_species"
    )]
    pub post_species: crate::domain::roof::TimberSpecies,
    /// Exterior timber must be preservative-treated even when naturally durable.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub post_treated: bool,
    /// The site's wind (#147).
    #[serde(default, skip_serializing_if = "is_default_wind")]
    pub wind: crate::domain::wind::WindSettings,
}

impl Default for ProjectStructure {
    fn default() -> Self {
        Self {
            beams: Vec::new(),
            slabs: Vec::new(),
            settings: StructureSettings::default(),
            balance_beams: Vec::new(),
            wall_foundations: Vec::new(),
            soil_survey: None,
            roofs: Vec::new(),
            post_species: crate::domain::roof::TimberSpecies::default(),
            post_treated: true,
            wind: crate::domain::wind::WindSettings::default(),
        }
    }
}

fn is_default_wind(wind: &crate::domain::wind::WindSettings) -> bool {
    *wind == crate::domain::wind::WindSettings::default()
}

fn is_default_timber_species(species: &crate::domain::roof::TimberSpecies) -> bool {
    *species == crate::domain::roof::TimberSpecies::default()
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
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
            && is_default_timber_species(&self.post_species)
            && self.post_treated
            && is_default_wind(&self.wind)
    }
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
