//! Earthing layout (ABNT NBR 5410 §6.4) and simplified lightning-protection
//! risk screening (ABNT NBR 5419-2:2015) (#129): grounding rod array sizing via
//! Dwight's equation, BEP & equipotential bonding conductors, automatic plan
//! geometry, NBR 5419-2 `Ad`/`Nd`/`Nl`/`R1` vs. `RT = 10⁻⁵/year` risk check,
//! plan marks, CSV and A3 SVG sheet.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    conduits::PlanMark, distribution::earth_section, geometry::Point, limits, model::Project,
    verdict::Light,
};

/// Maximum building height covered by Trazo's simplified NBR 5419-2 screening, m.
pub const MAX_SPDA_SIMPLIFIED_HEIGHT_M: f64 = 15.0;
/// NBR 5419-2 Table 4 tolerable risk of loss of human life (`RT`), per year.
pub const TOLERABLE_RISK_R1: f64 = 1.0e-5;
/// Default conservative soil resistivity when unmeasured, Ω·m.
pub const DEFAULT_SOIL_RESISTIVITY_OHM_M: f64 = 300.0;
/// Standard Brazilian Copperweld grounding rod length, m.
pub const DEFAULT_ROD_LENGTH_M: f64 = 2.40;
/// Standard Brazilian Copperweld 5/8" rod diameter, mm.
pub const DEFAULT_ROD_DIAMETER_MM: f64 = 16.0;
/// Standard rod-to-rod spacing (≥ 1 × L), m.
pub const DEFAULT_ROD_SPACING_M: f64 = 3.0;
/// Conservative thunderstorm days per year when no city is configured.
const DEFAULT_THUNDERSTORM_DAYS: f64 = 60.0;

/// ABNT NBR 5410 §4.2.2.2 earthing scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EarthingScheme {
    /// Separate neutral (N) and protective earth (PE) from the entrance BEP (Brazilian residential standard).
    #[default]
    TnS,
    /// Local independent grounding electrode; mandates 30 mA RCD on every circuit.
    Tt,
    /// Combined PEN split into N + PE at the main board.
    TnCS,
}

impl EarthingScheme {
    pub const ALL: [Self; 3] = [Self::TnS, Self::Tt, Self::TnCS];

    pub fn label(self) -> &'static str {
        match self {
            Self::TnS => "TN-S (padrão residencial)",
            Self::Tt => "TT (haste independente · DR obrigatório)",
            Self::TnCS => "TN-C-S (PEN separado no QDG)",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::TnS => "TN-S",
            Self::Tt => "TT",
            Self::TnCS => "TN-C-S",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::TnS => "tn_s",
            Self::Tt => "tt",
            Self::TnCS => "tn_c_s",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "tn_s" | "tn-s" | "tns" => Some(Self::TnS),
            "tt" => Some(Self::Tt),
            "tn_c_s" | "tn-c-s" | "tncs" => Some(Self::TnCS),
            _ => None,
        }
    }
}

/// ABNT NBR 5419-2 Table A.1 location factor (`Cd`) for the structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpdaSurroundings {
    /// Surrounded by taller objects (`Cd = 0.25`).
    SurroundedTaller,
    /// Surrounded by objects of similar or smaller height (`Cd = 0.50`, typical urban lot).
    SurroundedSimilar,
    /// Isolated structure with no nearby objects (`Cd = 1.00`, conservative default).
    #[default]
    Isolated,
    /// Isolated structure on a hilltop or ridge (`Cd = 2.00`).
    Hilltop,
}

impl SpdaSurroundings {
    pub const ALL: [Self; 4] = [
        Self::SurroundedTaller,
        Self::SurroundedSimilar,
        Self::Isolated,
        Self::Hilltop,
    ];

    pub fn cd(self) -> f64 {
        match self {
            Self::SurroundedTaller => 0.25,
            Self::SurroundedSimilar => 0.50,
            Self::Isolated => 1.00,
            Self::Hilltop => 2.00,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SurroundedTaller => "Cercada por prédios mais altos (Cd = 0,25)",
            Self::SurroundedSimilar => "Bairro urbano / casas vizinhas (Cd = 0,50)",
            Self::Isolated => "Isolada sem vizinhos altos (Cd = 1,00)",
            Self::Hilltop => "Isolada no topo de morro (Cd = 2,00)",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::SurroundedTaller => "cercada_mais_altos",
            Self::SurroundedSimilar => "urbano_similar",
            Self::Isolated => "isolada",
            Self::Hilltop => "topo_morro",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "cercada_mais_altos" | "predios" | "taller" => Some(Self::SurroundedTaller),
            "urbano_similar" | "urbano" | "similar" => Some(Self::SurroundedSimilar),
            "isolada" | "isolated" => Some(Self::Isolated),
            "topo_morro" | "morro" | "hilltop" => Some(Self::Hilltop),
            _ => None,
        }
    }
}

fn default_resistivity() -> f64 {
    DEFAULT_SOIL_RESISTIVITY_OHM_M
}
fn default_rod_length() -> f64 {
    DEFAULT_ROD_LENGTH_M
}
fn default_rod_diameter() -> f64 {
    DEFAULT_ROD_DIAMETER_MM
}
fn default_rod_spacing() -> f64 {
    DEFAULT_ROD_SPACING_M
}
fn default_true() -> bool {
    true
}

/// Earthing & SPDA settings persisted inside `ElectricalSettings` (`Copy`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EarthingSettings {
    #[serde(default)]
    pub scheme: EarthingScheme,
    /// Soil electrical resistivity ρ, Ω·m (default 300 Ω·m conservative).
    #[serde(default = "default_resistivity")]
    pub soil_resistivity_ohm_m: f64,
    #[serde(default = "default_rod_length")]
    pub rod_length_m: f64,
    #[serde(default = "default_rod_diameter")]
    pub rod_diameter_mm: f64,
    #[serde(default = "default_rod_spacing")]
    pub rod_spacing_m: f64,
    /// Optional fixed number of rods (1..=12); `None` auto-sizes (≥ 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rod_count: Option<usize>,
    #[serde(default)]
    pub surroundings: SpdaSurroundings,
    #[serde(default = "default_true")]
    pub overhead_power_line: bool,
    #[serde(default = "default_true")]
    pub has_dps_at_entrance: bool,
    #[serde(default = "default_true")]
    pub show_on_plan: bool,
}

impl Default for EarthingSettings {
    fn default() -> Self {
        Self {
            scheme: EarthingScheme::TnS,
            soil_resistivity_ohm_m: DEFAULT_SOIL_RESISTIVITY_OHM_M,
            rod_length_m: DEFAULT_ROD_LENGTH_M,
            rod_diameter_mm: DEFAULT_ROD_DIAMETER_MM,
            rod_spacing_m: DEFAULT_ROD_SPACING_M,
            rod_count: None,
            surroundings: SpdaSurroundings::Isolated,
            overhead_power_line: true,
            has_dps_at_entrance: true,
            show_on_plan: true,
        }
    }
}

impl EarthingSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Dwight's formula for one vertical cylindrical rod of length `L` (m) and
/// diameter `d` (m) in homogeneous soil of resistivity `rho` (Ω·m).
pub fn single_rod_resistance_ohm(rho_ohm_m: f64, length_m: f64, diameter_m: f64) -> f64 {
    let l = length_m.max(0.5);
    let d = diameter_m.clamp(0.006, 0.10);
    let factor = (4.0 * l / d).ln() - 1.0;
    (rho_ohm_m.max(10.0) / (2.0 * std::f64::consts::PI * l)) * factor.max(1.0)
}

/// Parallel reduction factor `K_n` for `n` vertical rods in line spaced `s >= L`
/// (accounts for mutual earth-potential interference, IEEE 80 / Tagg).
pub fn rod_array_factor(count: usize) -> f64 {
    match count {
        0 | 1 => 1.00,
        2 => 0.58,
        3 => 0.42,
        4 => 0.33,
        5 => 0.28,
        6 => 0.24,
        n => (1.38 / n as f64).max(0.12),
    }
}

/// Grounding rod position on the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthingRodPoint {
    pub index: usize,
    pub position: Point,
}

/// Complete earthing layout & simplified NBR 5419-2 SPDA risk report (#129).
#[derive(Clone, Debug, PartialEq)]
pub struct EarthingSpdaReport {
    pub settings: EarthingSettings,
    // --- Earthing (NBR 5410 §6.4) ---
    pub single_rod_ohm: f64,
    pub rod_count: usize,
    pub array_resistance_ohm: f64,
    pub target_resistance_ohm: f64,
    pub buried_conductor_mm2: f64,
    pub conduit_conductor_mm2: f64,
    pub bonding_conductor_mm2: f64,
    pub inspection_boxes: usize,
    pub tt_missing_rcd_circuits: usize,
    pub bep_position: Option<Point>,
    pub rods: Vec<EarthingRodPoint>,
    pub earthing_light: Light,
    pub earthing_note: String,
    // --- Simplified SPDA risk check (NBR 5419-2:2015) ---
    pub city_label: String,
    pub thunderstorm_days_year: f64,
    pub flash_density_ng: f64,
    pub building_length_m: f64,
    pub building_width_m: f64,
    pub building_height_m: f64,
    pub collection_area_ad_m2: f64,
    pub annual_direct_flashes_nd: f64,
    pub annual_line_flashes_nl: f64,
    pub risk_r1: f64,
    pub tolerable_risk_rt: f64,
    pub spda_required: bool,
    pub spda_refused_height: bool,
    pub recommended_np: Option<String>,
    pub spda_light: Light,
    pub spda_note: String,
    pub overall_light: Light,
    pub warnings: Vec<String>,
    pub assumptions: Vec<String>,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Computes building bounding box `(min, max)` from walls or rooms.
fn building_footprint(project: &Project) -> Option<(Point, Point)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for w in &project.walls {
        for p in [w.start, w.end] {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    if min_x.is_finite() && max_x > min_x + 0.1 && max_y > min_y + 0.1 {
        Some((Point::new(min_x, min_y), Point::new(max_x, max_y)))
    } else {
        None
    }
}

/// Places BEP near the main panel (or building corner) and `count` grounding
/// rods spaced `spacing_m` along the nearest exterior side inside the lot.
fn layout_earthing_points(
    project: &Project,
    count: usize,
    spacing_m: f64,
) -> (Option<Point>, Vec<EarthingRodPoint>) {
    let Some((bmin, bmax)) = building_footprint(project) else {
        return (None, Vec::new());
    };
    let anchor = project
        .main_panel()
        .map(|p| p.center)
        .unwrap_or_else(|| Point::new(bmin.x, bmin.y));

    // Pick the exterior side closest to QDG, placing BEP & rods 0.60 m outside it.
    let offset = 0.60;
    let d_left = (anchor.x - bmin.x).abs();
    let d_right = (bmax.x - anchor.x).abs();
    let d_top = (anchor.y - bmin.y).abs();
    let d_bottom = (bmax.y - anchor.y).abs();
    let min_d = d_left.min(d_right).min(d_top).min(d_bottom);

    let (mut bep, dir) = if (min_d - d_left).abs() < 1e-6 {
        (
            Point::new(bmin.x - offset, anchor.y.clamp(bmin.y, bmax.y)),
            Point::new(0.0, 1.0),
        )
    } else if (min_d - d_right).abs() < 1e-6 {
        (
            Point::new(bmax.x + offset, anchor.y.clamp(bmin.y, bmax.y)),
            Point::new(0.0, 1.0),
        )
    } else if (min_d - d_top).abs() < 1e-6 {
        (
            Point::new(anchor.x.clamp(bmin.x, bmax.x), bmin.y - offset),
            Point::new(1.0, 0.0),
        )
    } else {
        (
            Point::new(anchor.x.clamp(bmin.x, bmax.x), bmax.y + offset),
            Point::new(1.0, 0.0),
        )
    };

    // Clamp inside terrain bounding box if a lot exists.
    let clamp_lot = |p: Point| -> Point {
        if let Some(terrain) = &project.terrain {
            let (tmin, tmax) = (terrain.min, terrain.max);
            let margin = 0.25;
            if tmax.x - tmin.x > 2.0 * margin && tmax.y - tmin.y > 2.0 * margin {
                return Point::new(
                    p.x.clamp(tmin.x + margin, tmax.x - margin),
                    p.y.clamp(tmin.y + margin, tmax.y - margin),
                );
            }
        }
        p
    };

    bep = clamp_lot(bep);
    let mut rods = Vec::with_capacity(count);
    for i in 0..count {
        let step = (i as f64 + 0.5) * spacing_m;
        let candidate = Point::new(bep.x + dir.x * step, bep.y + dir.y * step);
        let pos = clamp_lot(candidate);
        rods.push(EarthingRodPoint {
            index: i + 1,
            position: pos,
        });
    }
    (Some(bep), rods)
}

/// Computes the earthing layout and simplified NBR 5419-2 SPDA risk check (#129).
pub fn earthing_and_spda_report(project: &Project) -> EarthingSpdaReport {
    let settings = project.electrical.earthing;
    let schedule = project.cached_load_schedule();

    // --- 1. Earthing layout (NBR 5410 §6.4) ---
    let r1 = single_rod_resistance_ohm(
        settings.soil_resistivity_ohm_m,
        settings.rod_length_m,
        settings.rod_diameter_mm / 1000.0,
    );
    let target_ohm = 25.0;
    let rod_count = match settings.rod_count {
        Some(n) => n.clamp(1, 12),
        None => {
            let mut n = 3usize;
            while n < 6 && r1 * rod_array_factor(n) > target_ohm {
                n += 1;
            }
            n
        }
    };
    let array_ohm = r1 * rod_array_factor(rod_count);

    let ent_phase = schedule.entrance.and_then(|e| e.wire_mm2).unwrap_or(10.0);
    let pe_main = earth_section(ent_phase);
    let buried_mm2 = pe_main.max(25.0);
    let conduit_mm2 = pe_main.max(16.0);
    let bonding_mm2 = (pe_main * 0.5).clamp(6.0, 25.0);

    let tt_missing_rcd = if settings.scheme == EarthingScheme::Tt {
        schedule.circuits.iter().filter(|c| !c.needs_rcd).count()
    } else {
        0
    };

    let (bep_position, rods) =
        layout_earthing_points(project, rod_count, settings.rod_spacing_m.max(2.0));

    let earthing_light =
        if (settings.scheme == EarthingScheme::Tt && tt_missing_rcd > 0) || array_ohm > 50.0 {
            Light::Red
        } else if array_ohm > target_ohm {
            Light::Yellow
        } else {
            Light::Green
        };

    let earthing_note = if settings.scheme == EarthingScheme::Tt && tt_missing_rcd > 0 {
        format!(
            "No esquema TT todos os circuitos precisam de DR 30 mA (há {tt_missing_rcd} circuito(s) sem DR): proteja todo o quadro com DR ou use TN-S."
        )
    } else if array_ohm > 50.0 {
        format!(
            "Resistência estimada alta ({} Ω em solo de {:.0} Ω·m com {rod_count} hastes): adicione mais hastes, anel nas fundações ou tratamento químico do solo e meça com terrômetro.",
            decimal(array_ohm, 1),
            settings.soil_resistivity_ohm_m
        )
    } else if array_ohm > target_ohm {
        format!(
            "Resistência estimada de {} Ω com {rod_count} hastes (meta ≤ 25 Ω): considere ampliar a malha ou interligar à ferragem da fundação (NBR 5410 §6.4.1.1).",
            decimal(array_ohm, 1)
        )
    } else {
        format!(
            "Aterramento {} com {rod_count} hastes de {} m (espaçamento {} m) estima {} Ω (≤ 25 Ω), condutor enterrado #{} mm² e BEP.",
            settings.scheme.code(),
            decimal(settings.rod_length_m, 2),
            decimal(settings.rod_spacing_m, 1),
            decimal(array_ohm, 1),
            decimal(buried_mm2, 0)
        )
    };

    // --- 2. Simplified SPDA Risk Check (ABNT NBR 5419-2:2015) ---
    let site = &project.codes.site;
    let td = site
        .thunderstorm_days_year
        .unwrap_or(DEFAULT_THUNDERSTORM_DAYS)
        .clamp(1.0, 200.0);
    let city_label = site
        .location_label()
        .unwrap_or_else(|| "Cidade não informada (Td = 60 dias/ano)".to_owned());
    // Empirical NBR 5419 / IEC 62305 flash density relation: Ng = 0.04 · Td^1.25.
    let ng = 0.04 * td.powf(1.25);

    let (b_len, b_wid) = match building_footprint(project) {
        Some((min, max)) => {
            let dx = (max.x - min.x).max(1.0);
            let dy = (max.y - min.y).max(1.0);
            (dx.max(dy), dx.min(dy))
        }
        None => (10.0, 8.0),
    };
    let storeys = f64::from(limits::storeys(project));
    let max_roof_h = project
        .structure
        .roofs
        .iter()
        .map(|r| r.high_m)
        .fold(0.0, f64::max);
    let b_height = (storeys * project.wall_height).max(max_roof_h).max(2.8);
    let spda_refused_height = b_height > MAX_SPDA_SIMPLIFIED_HEIGHT_M + 1e-6;

    // Equivalent collection area Ad (NBR 5419-2 §A.2.1.1).
    let ad = b_len * b_wid
        + 2.0 * (3.0 * b_height) * (b_len + b_wid)
        + std::f64::consts::PI * (3.0 * b_height).powi(2);
    let cd = settings.surroundings.cd();
    let nd = ng * ad * cd * 1.0e-6;

    // Incoming LV power line flashes Nl (100 m residential exposure, NBR 5419-2 §A.4).
    let al = 40.0 * 100.0;
    let ci = if settings.overhead_power_line {
        1.0
    } else {
        0.5
    };
    let ce = match settings.surroundings {
        SpdaSurroundings::SurroundedTaller => 0.1,
        SpdaSurroundings::SurroundedSimilar => 0.5,
        SpdaSurroundings::Isolated | SpdaSurroundings::Hilltop => 1.0,
    };
    let nl = ng * al * ci * ce * 1.0e-6;

    // Simplified residential loss-of-human-life risk R1 (NBR 5419-2 Annex B/C):
    // Residential masonry/concrete: rf = 1e-3 (low fire risk), rp = 1.0, Lf = 1e-2, Lt = 1e-2.
    let rf = 1.0e-3;
    let lf = 1.0e-2;
    let lt = 1.0e-2;
    let p_spd = if settings.has_dps_at_entrance {
        0.05
    } else {
        1.0
    };
    let ra = nd * 1.0e-2 * lt;
    let rb = nd * 1.0 * rf * lf;
    let ru = nl * 1.0e-2 * p_spd * lt;
    let rv = nl * p_spd * rf * lf;
    let r1 = ra + rb + ru + rv;

    let spda_required = spda_refused_height || r1 > TOLERABLE_RISK_R1;
    let recommended_np = if spda_refused_height {
        Some("Projeto completo NBR 5419 por engenheiro eletricista".to_owned())
    } else if r1 > TOLERABLE_RISK_R1 {
        // Test NP IV (PB=0.2), NP III (0.1), NP II (0.05), NP I (0.02).
        let np = [
            ("NP IV (Classe IV)", 0.20),
            ("NP III (Classe III)", 0.10),
            ("NP II (Classe II)", 0.05),
            ("NP I (Classe I)", 0.02),
        ]
        .iter()
        .find(|(_, pb)| ra * pb + rb * pb + ru + rv <= TOLERABLE_RISK_R1)
        .map_or("NP I + medidas complementares (engenheiro)", |(name, _)| {
            *name
        });
        Some(np.to_owned())
    } else {
        None
    };

    let spda_light = if spda_required {
        Light::Red
    } else if r1 > 0.85 * TOLERABLE_RISK_R1 {
        Light::Yellow
    } else {
        Light::Green
    };

    let spda_note = if spda_refused_height {
        format!(
            "Altura de {} m passa do limite da triagem simplificada ({:.0} m): exige estudo completo de SPDA (NBR 5419-2) por engenheiro eletricista.",
            decimal(b_height, 1),
            MAX_SPDA_SIMPLIFIED_HEIGHT_M
        )
    } else if spda_required {
        format!(
            "Risco simplificado R1 = {:.2e}/ano excede o limite tolerável RT = 1,0×10⁻⁵/ano (NBR 5419-2): exige projeto de SPDA ({}) assinado por engenheiro eletricista.",
            r1,
            recommended_np.as_deref().unwrap_or("NP IV")
        )
    } else if spda_light == Light::Yellow {
        format!(
            "Risco R1 = {:.2e}/ano está próximo do limite 1,0×10⁻⁵/ano: mantenha DPS Classe I+II na entrada e consulte um engenheiro eletricista.",
            r1
        )
    } else {
        format!(
            "Triagem simplificada NBR 5419-2: R1 = {:.2e}/ano ≤ 1,0×10⁻⁵/ano (SPDA estrutural não exigido; manter aterramento e DPS Classe II).",
            r1
        )
    };

    let mut warnings = Vec::new();
    if earthing_light == Light::Red {
        warnings.push(earthing_note.clone());
    }
    if spda_light == Light::Red {
        warnings.push(spda_note.clone());
    }
    if !settings.has_dps_at_entrance {
        warnings.push(
            "Sem DPS na entrada: a NBR 5410 §5.4.2 exige protetor contra surtos em regiões com trovoadas (AQ2/AQ3)."
                .to_owned(),
        );
    }

    let overall_light = earthing_light.max(spda_light);

    let assumptions = vec![
        format!(
            "Aterramento (NBR 5410 §6.4): equação de Dwight para hastes Copperweld Ø{:.0} mm × {} m espaçadas em {} m em solo de {:.0} Ω·m; medição em campo com terrômetro é obrigatória.",
            settings.rod_diameter_mm,
            decimal(settings.rod_length_m, 2),
            decimal(settings.rod_spacing_m, 1),
            settings.soil_resistivity_ohm_m
        ),
        format!(
            "SPDA simplificado (NBR 5419-2:2015): {} · Ng = {} descargas/km²·ano, Ad = {:.0} m², Cd = {} ({}), edificação residencial comum de {} × {} × {} m (limite até 15 m, sem inflamáveis/público).",
            city_label,
            decimal(ng, 2),
            ad,
            decimal(cd, 2),
            settings.surroundings.key(),
            decimal(b_len, 1),
            decimal(b_wid, 1),
            decimal(b_height, 1)
        ),
        "Pré-dimensionamento: a dispensa ou obrigatoriedade legal de SPDA também depende das instruções do Corpo de Bombeiros estadual e exige validação por profissional habilitado (ART/RRT).".to_owned(),
    ];

    EarthingSpdaReport {
        settings,
        single_rod_ohm: r1,
        rod_count,
        array_resistance_ohm: array_ohm,
        target_resistance_ohm: target_ohm,
        buried_conductor_mm2: buried_mm2,
        conduit_conductor_mm2: conduit_mm2,
        bonding_conductor_mm2: bonding_mm2,
        inspection_boxes: rod_count,
        tt_missing_rcd_circuits: tt_missing_rcd,
        bep_position,
        rods,
        earthing_light,
        earthing_note,
        city_label,
        thunderstorm_days_year: td,
        flash_density_ng: ng,
        building_length_m: b_len,
        building_width_m: b_wid,
        building_height_m: b_height,
        collection_area_ad_m2: ad,
        annual_direct_flashes_nd: nd,
        annual_line_flashes_nl: nl,
        risk_r1: r1,
        tolerable_risk_rt: TOLERABLE_RISK_R1,
        spda_required,
        spda_refused_height,
        recommended_np,
        spda_light,
        spda_note,
        overall_light,
        warnings,
        assumptions,
    }
}

/// Plan marks drawing the BEP, grounding rods (`H1..Hn`) and buried earthing bus
/// (*screen equals paper*, #129).
pub fn earthing_plan_marks(project: &Project) -> Vec<PlanMark> {
    if !project.electrical.earthing.show_on_plan {
        return Vec::new();
    }
    if project.main_panel().is_none() {
        return Vec::new();
    }
    let report = earthing_and_spda_report(project);
    let Some(bep) = report.bep_position else {
        return Vec::new();
    };
    let mut marks = Vec::new();

    // Link main panel to BEP and BEP along the rod array.
    let mut bus = Vec::new();
    if let Some(panel) = project.main_panel() {
        bus.push(panel.center);
    }
    bus.push(bep);
    for rod in &report.rods {
        bus.push(rod.position);
    }
    if bus.len() >= 2 {
        marks.push(PlanMark::Line {
            points: bus,
            dashed: true,
            heavy: true,
        });
    }

    // BEP square box + label.
    let s = 0.14;
    marks.push(PlanMark::Line {
        points: vec![
            Point::new(bep.x - s, bep.y - s),
            Point::new(bep.x + s, bep.y - s),
            Point::new(bep.x + s, bep.y + s),
            Point::new(bep.x - s, bep.y + s),
            Point::new(bep.x - s, bep.y - s),
        ],
        dashed: false,
        heavy: true,
    });
    marks.push(PlanMark::Text {
        anchor: Point::new(bep.x, bep.y - 0.28),
        text: "BEP".to_owned(),
    });

    // Grounding rods (circle + ground ticks + H1..Hn label).
    for rod in &report.rods {
        let p = rod.position;
        marks.push(PlanMark::Circle {
            center: p,
            radius: 0.15,
        });
        for (i, half) in [0.10, 0.065, 0.03].iter().enumerate() {
            let y = p.y - 0.03 + i as f64 * 0.04;
            marks.push(PlanMark::Line {
                points: vec![Point::new(p.x - *half, y), Point::new(p.x + *half, y)],
                dashed: false,
                heavy: false,
            });
        }
        marks.push(PlanMark::Text {
            anchor: Point::new(p.x, p.y + 0.30),
            text: format!("H{}", rod.index),
        });
    }

    marks
}

/// Printable A3 landscape SVG sheet of the earthing layout and NBR 5419-2 SPDA screening (#129).
pub fn earthing_spda_sheet_svg(project: &Project) -> String {
    let r = earthing_and_spda_report(project);
    let width = 420.0;
    let height = 297.0;
    let ink = "#1d2836";
    let muted = "#4a5d73";
    let green = "#2e7d32";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"400\" height=\"277\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n"
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                color: &str,
                value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };

    text(
        &mut svg,
        18.0,
        22.0,
        5.0,
        true,
        ink,
        "ATERRAMENTO (NBR 5410) E TRIAGEM DE RISCO SPDA (NBR 5419-2)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!(
            "{} · Esquema {} · {} hastes ({} Ω) · SPDA: {}",
            project.name,
            r.settings.scheme.code(),
            r.rod_count,
            decimal(r.array_resistance_ohm, 1),
            if r.spda_required {
                "Exigido"
            } else {
                "Não exigido na triagem"
            }
        ),
    );

    // Left column: Earthing schematic + specifications.
    let mut y = 42.0;
    text(
        &mut svg,
        18.0,
        y,
        3.6,
        true,
        ink,
        "1. Sistema de aterramento e equipotencialização (NBR 5410 §6.4)",
    );
    y += 6.0;
    let e_color = match r.earthing_light {
        Light::Green => green,
        Light::Yellow => "#b85d00",
        Light::Red => "#c62828",
    };
    text(&mut svg, 18.0, y, 2.7, true, e_color, &r.earthing_note);
    y += 7.0;
    let e_rows = [
        (
            "Esquema de aterramento",
            r.settings.scheme.label().to_owned(),
        ),
        (
            "Resistividade do solo (ρ)",
            format!("{:.0} Ω·m", r.settings.soil_resistivity_ohm_m),
        ),
        (
            "Haste individual (Dwight)",
            format!(
                "Ø{:.0} mm × {} m → R1 = {} Ω",
                r.settings.rod_diameter_mm,
                decimal(r.settings.rod_length_m, 2),
                decimal(r.single_rod_ohm, 1)
            ),
        ),
        (
            "Malha de hastes",
            format!(
                "{} hastes em linha espaçadas em {} m → Ra = {} Ω (meta ≤ 25 Ω)",
                r.rod_count,
                decimal(r.settings.rod_spacing_m, 1),
                decimal(r.array_resistance_ohm, 1)
            ),
        ),
        (
            "Condutor de aterramento",
            format!(
                "#{} mm² cobre nu enterrado (ou #{} mm² isolado em eletroduto)",
                decimal(r.buried_conductor_mm2, 0),
                decimal(r.conduit_conductor_mm2, 0)
            ),
        ),
        (
            "Equipotencialização (BEP)",
            format!(
                "BEP próximo ao QDG + condutores #{} mm² (tubulações, armaduras, PE)",
                decimal(r.bonding_conductor_mm2, 0)
            ),
        ),
        (
            "Caixas de inspeção",
            format!(
                "{} caixa(s) Ø300 mm com tampa (1 por haste)",
                r.inspection_boxes
            ),
        ),
    ];
    for (k, v) in e_rows {
        text(&mut svg, 18.0, y, 2.6, true, muted, k);
        text(&mut svg, 72.0, y, 2.6, false, ink, &v);
        y += 5.5;
    }

    // Schematic diagram of QDG -> BEP -> H1..Hn.
    let sy = y + 8.0;
    let _ = writeln!(
        svg,
        "<rect x=\"22\" y=\"{sy:.1}\" width=\"24\" height=\"12\" fill=\"#f1f5f9\" stroke=\"{ink}\" stroke-width=\"0.4\"/>"
    );
    text(&mut svg, 34.0, sy + 7.2, 2.8, true, ink, "QDG");
    let _ = writeln!(
        svg,
        "<line x1=\"46\" y1=\"{:.1}\" x2=\"64\" y2=\"{:.1}\" stroke=\"{green}\" stroke-width=\"0.7\"/>",
        sy + 6.0,
        sy + 6.0
    );
    let _ = writeln!(
        svg,
        "<rect x=\"64\" y=\"{sy:.1}\" width=\"22\" height=\"12\" fill=\"#e8f5e9\" stroke=\"{green}\" stroke-width=\"0.5\"/>"
    );
    text(&mut svg, 75.0, sy + 7.2, 2.8, true, green, "BEP");
    let shown = r.rod_count.min(6);
    let last_x = 104.0 + (shown.saturating_sub(1)) as f64 * 18.0;
    let _ = writeln!(
        svg,
        "<line x1=\"86\" y1=\"{:.1}\" x2=\"{last_x:.1}\" y2=\"{:.1}\" stroke=\"{green}\" stroke-width=\"0.7\" stroke-dasharray=\"2 1.5\"/>",
        sy + 6.0,
        sy + 6.0
    );
    for i in 0..shown {
        let rx = 104.0 + i as f64 * 18.0;
        let _ = writeln!(
            svg,
            "<line x1=\"{rx:.1}\" y1=\"{:.1}\" x2=\"{rx:.1}\" y2=\"{:.1}\" stroke=\"{green}\" stroke-width=\"0.9\"/>",
            sy + 6.0,
            sy + 26.0
        );
        let _ = writeln!(
            svg,
            "<circle cx=\"{rx:.1}\" cy=\"{:.1}\" r=\"2.2\" fill=\"#ffffff\" stroke=\"{green}\" stroke-width=\"0.4\"/>",
            sy + 6.0
        );
        text(
            &mut svg,
            rx - 2.5,
            sy + 3.0,
            2.2,
            true,
            green,
            &format!("H{}", i + 1),
        );
    }
    text(
        &mut svg,
        104.0,
        sy + 32.0,
        2.3,
        false,
        muted,
        &format!(
            "Hastes Ø{:.0} mm × {} m · espaçamento {} m · cabo #{} mm²",
            r.settings.rod_diameter_mm,
            decimal(r.settings.rod_length_m, 2),
            decimal(r.settings.rod_spacing_m, 1),
            decimal(r.buried_conductor_mm2, 0)
        ),
    );

    // Right column: Simplified NBR 5419-2 SPDA risk check.
    let mut ry = 42.0;
    text(
        &mut svg,
        218.0,
        ry,
        3.6,
        true,
        ink,
        "2. Triagem de risco de descargas atmosféricas (NBR 5419-2)",
    );
    ry += 6.0;
    let s_color = match r.spda_light {
        Light::Green => green,
        Light::Yellow => "#b85d00",
        Light::Red => "#c62828",
    };
    text(&mut svg, 218.0, ry, 2.7, true, s_color, &r.spda_note);
    ry += 7.0;
    let s_rows = [
        (
            "Local / dias de trovoada",
            format!(
                "{} (Td = {:.0} dias/ano)",
                r.city_label, r.thunderstorm_days_year
            ),
        ),
        (
            "Densidade de descargas (Ng)",
            format!("{} descargas / km²·ano", decimal(r.flash_density_ng, 2)),
        ),
        (
            "Dimensões da edificação",
            format!(
                "L = {} m · W = {} m · H = {} m",
                decimal(r.building_length_m, 1),
                decimal(r.building_width_m, 1),
                decimal(r.building_height_m, 1)
            ),
        ),
        (
            "Área de exposição (Ad)",
            format!("{:.0} m²", r.collection_area_ad_m2),
        ),
        (
            "Fator de localização (Cd)",
            r.settings.surroundings.label().to_owned(),
        ),
        (
            "Descargas diretas (Nd)",
            format!("{:.4} / ano", r.annual_direct_flashes_nd),
        ),
        (
            "Descargas na linha (Nl)",
            format!(
                "{:.4} / ano ({})",
                r.annual_line_flashes_nl,
                if r.settings.overhead_power_line {
                    "rede aérea"
                } else {
                    "subterrânea"
                }
            ),
        ),
        (
            "Risco calculado (R1)",
            format!("{:.2e} / ano (tolerável RT = 1,00e-5 / ano)", r.risk_r1),
        ),
        (
            "Recomendação SPDA",
            r.recommended_np
                .clone()
                .unwrap_or_else(|| "Dispensado na triagem · manter DPS Classe II".to_owned()),
        ),
    ];
    for (k, v) in s_rows {
        text(&mut svg, 218.0, ry, 2.6, true, muted, k);
        text(&mut svg, 272.0, ry, 2.6, false, ink, &v);
        ry += 5.8;
    }

    let mut fy = 240.0;
    for note in &r.assumptions {
        text(&mut svg, 18.0, fy, 2.2, false, muted, note);
        fy += 4.2;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Earthing layout and simplified NBR 5419-2 SPDA risk report (#129).
    pub fn earthing_and_spda_report(&self) -> EarthingSpdaReport {
        earthing_and_spda_report(self)
    }

    /// Earthing BEP + grounding rod plan marks (#129).
    pub fn earthing_plan_marks(&self) -> Vec<PlanMark> {
        earthing_plan_marks(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{codes::SiteClimate, fixtures::FixtureKind, samples::SampleProject};

    /// Worked example 1 (Dwight's earthing formula):
    /// ρ = 200 Ω·m, L = 2.40 m, d = 0.016 m (5/8"):
    ///   ln(4 * 2.40 / 0.016) - 1 = ln(600) - 1 = 6.3969 - 1 = 5.3969
    ///   R1 = 200 / (2 * π * 2.40) * 5.3969 = 13.2629 * 5.3969 = 71.58 Ω
    ///   3 rods (K3 = 0.42): R3 = 30.06 Ω -> auto-sizes to 4 rods (K4 = 0.33 -> R4 = 23.62 Ω ≤ 25 Ω)!
    #[test]
    fn worked_example_dwight_earthing_matches_hand_calculation_and_sizes_rods() {
        let r1 = single_rod_resistance_ohm(200.0, 2.40, 0.016);
        assert!((r1 - 71.58).abs() < 0.2, "r1={r1}");

        let mut project = SampleProject::CasaTerrea.build();
        project.electrical.earthing.soil_resistivity_ohm_m = 200.0;
        let (pw, pd) = FixtureKind::Panel.default_size();
        let (rmin, _) = project.room_bounds(&project.rooms[0]).unwrap();
        project
            .add_fixture(
                FixtureKind::Panel,
                Point::new(rmin.x + 0.35, rmin.y + 0.15),
                pw,
                pd,
                0,
            )
            .unwrap();

        let report = project.earthing_and_spda_report();
        assert_eq!(report.rod_count, 4);
        assert!((report.array_resistance_ohm - r1 * 0.33).abs() < 0.1);
        assert!(report.array_resistance_ohm <= 25.0);
        assert_eq!(report.earthing_light, Light::Green);
        assert!(report.bep_position.is_some());
        assert_eq!(report.rods.len(), 4);
        assert!(!project.earthing_plan_marks().is_empty());
    }

    /// Worked example 2 (ABNT NBR 5419-2:2015):
    /// Compare a small 1-storey urban house (Cd = 0.50, DPS installed) vs. an isolated
    /// 3-storey house on a hilltop (Cd = 2.00, high thunderstorm city Campinas Td = 80 days/yr, no DPS).
    #[test]
    fn worked_example_nbr5419_collection_area_and_risk_distinguish_urban_vs_hilltop() {
        let mut urban = Project::default();
        urban.wall_height = 3.0;
        urban.set_terrain(Point::new(0.0, 0.0), Point::new(15.0, 20.0));
        urban
            .add_room(Point::new(2.0, 2.0), Point::new(12.0, 10.0), "Casa")
            .unwrap();
        urban.codes.site = SiteClimate::find_preset("rio_de_janeiro_rj")
            .unwrap()
            .to_site_climate();
        urban.electrical.earthing.surroundings = SpdaSurroundings::SurroundedSimilar;
        urban.electrical.earthing.has_dps_at_entrance = true;

        let r_urban = urban.earthing_and_spda_report();
        // L = 10, W = 8, H = 3.0 -> Ad = 80 + 2*9*18 + π*81 = 80 + 324 + 254.47 = 658.47 m²
        assert!(
            (r_urban.collection_area_ad_m2 - 658.47).abs() < 2.0,
            "Ad={}",
            r_urban.collection_area_ad_m2
        );
        assert!(!r_urban.spda_required);
        assert_eq!(r_urban.spda_light, Light::Green);

        // Now a tall 3-storey house (H = 10.5 m) on a hilltop in Campinas (Td = 80, no DPS).
        let mut hilltop = urban.clone();
        hilltop.wall_height = 3.5;
        hilltop.structure.settings.floors = 3;
        hilltop.codes.site = SiteClimate::find_preset("campinas_sp")
            .unwrap()
            .to_site_climate();
        hilltop.electrical.earthing.surroundings = SpdaSurroundings::Hilltop;
        hilltop.electrical.earthing.has_dps_at_entrance = false;

        let r_hill = hilltop.earthing_and_spda_report();
        assert!(r_hill.collection_area_ad_m2 > r_urban.collection_area_ad_m2 * 4.0);
        assert!(r_hill.risk_r1 > TOLERABLE_RISK_R1);
        assert!(r_hill.spda_required);
        assert_eq!(r_hill.spda_light, Light::Red);
        assert!(r_hill.recommended_np.is_some());

        let svg = earthing_spda_sheet_svg(&hilltop);
        assert!(svg.contains("ATERRAMENTO (NBR 5410)"));
        assert!(svg.contains("</svg>"));
    }
}
