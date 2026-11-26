//! Septic tank and soakaway (*tanque séptico e sumidouro*) where there is no
//! public sewer (NBR 7229:1993, NBR 13969:1997).
//!
//! - Useful volume `V = 1000 + N·(C·T + K·Lf)` litres, with the daily
//!   contribution `C` and fresh sludge `Lf` of Table 1, the detention time `T`
//!   of Table 2 and the sludge accumulation rate `K` of Table 3.
//! - Useful depth from Table 4; prismatic tanks with length/width 2 to 4 and
//!   width at least 0.80 m; cylindrical ones with `1.10 m ≤ D ≤ 2·H`.
//! - Least distances: 1.50 m from buildings, lot limits, soakaways and the
//!   water service pipe; 3.0 m from trees and the public water network;
//!   15 m from wells and water bodies.
//! - Soakaway: infiltration area `A = Q / TAS`, counting the wall below the
//!   inlet and the bottom (`π·D²/4 + π·D·H`); bottom at least 1.50 m above
//!   the highest water table; 20 m from wells. The absorption rate `TAS`
//!   comes from a percolation test (NBR 13969 Annex A); without one Trazo
//!   uses the presumed soil's value (or 20 L/m²·dia, the low end for clays)
//!   and says the test decides.

use serde::{Deserialize, Serialize};

use crate::domain::geometry::{Point, distance_to_segment};
use crate::domain::model::Project;

use super::cold_water::{PEOPLE_PER_BEDROOM, bedroom_count};

/// Fresh sludge per person per day for dwellings, L (Table 1).
const FRESH_SLUDGE_L: f64 = 1.0;
/// Least distance of the tank (and, in Trazo, of the soakaway) to
/// buildings, lot limits and each other, m.
pub const MIN_DISTANCE_M: f64 = 1.50;
/// What Trazo asks of a soakaway near foundations in collapsible soil, m.
pub const COLLAPSIBLE_DISTANCE_M: f64 = 3.0;
/// Absorption rate without a test or a presumed soil, L/m²·dia.
pub const FALLBACK_ABSORPTION_L_M2_DAY: f64 = 20.0;
/// Depth of the inlet pipe below the ground, m.
const INLET_DEPTH_M: f64 = 0.60;
/// Wall of a precast ring or a brick soakaway, m.
const RING_WALL_M: f64 = 0.10;
/// Deepest soakaway Trazo sizes without asking for another solution, m.
pub const MAX_SOAKAWAY_DEPTH_M: f64 = 6.0;
/// Bottom of the soakaway above the water table, m.
pub const WATER_TABLE_CLEARANCE_M: f64 = 1.50;

/// The house's standard (NBR 7229 Table 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SewageStandard {
    High,
    #[default]
    Medium,
    Low,
}

impl SewageStandard {
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Low];

    pub fn key(self) -> &'static str {
        match self {
            Self::High => "alto",
            Self::Medium => "medio",
            Self::Low => "baixo",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "padrão alto",
            Self::Medium => "padrão médio",
            Self::Low => "padrão baixo",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase().replace('é', "e");
        Self::ALL.into_iter().find(|s| s.key() == raw)
    }

    /// Daily sewage per person, L (Table 1).
    pub fn per_person_l(self) -> f64 {
        match self {
            Self::High => 160.0,
            Self::Medium => 130.0,
            Self::Low => 100.0,
        }
    }
}

/// The coldest month's mean temperature (NBR 7229 Table 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColdestMonth {
    /// t ≤ 10 °C.
    Cold,
    /// 10 < t ≤ 20 °C.
    #[default]
    Mild,
    /// t > 20 °C.
    Warm,
}

impl ColdestMonth {
    pub const ALL: [Self; 3] = [Self::Cold, Self::Mild, Self::Warm];

    pub fn key(self) -> &'static str {
        match self {
            Self::Cold => "frio",
            Self::Mild => "ameno",
            Self::Warm => "quente",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cold => "até 10 °C",
            Self::Mild => "de 10 a 20 °C",
            Self::Warm => "acima de 20 °C",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase();
        Self::ALL.into_iter().find(|c| c.key() == raw)
    }
}

fn default_diameter() -> f64 {
    1.50
}

fn default_years() -> u32 {
    1
}

/// The septic system's settings, saved with the plumbing design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SepticSettings {
    /// Centre of the tank in the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tank_at: Option<Point>,
    /// Centres of the soakaways in the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub soakaways_at: Vec<Point>,
    #[serde(default = "default_diameter")]
    pub soakaway_diameter_m: f64,
    #[serde(default)]
    pub standard: SewageStandard,
    /// Years between cleanings (1 to 5).
    #[serde(default = "default_years")]
    pub cleaning_years: u32,
    #[serde(default)]
    pub coldest_month: ColdestMonth,
    /// People beyond the residents (frequent guests of the leisure area).
    #[serde(default)]
    pub extra_people: u32,
    /// Absorption rate measured on site (percolation test), L/m²·dia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tested_absorption_l_m2_day: Option<f64>,
}

impl Default for SepticSettings {
    fn default() -> Self {
        Self {
            tank_at: None,
            soakaways_at: Vec::new(),
            soakaway_diameter_m: default_diameter(),
            standard: SewageStandard::default(),
            cleaning_years: default_years(),
            coldest_month: ColdestMonth::default(),
            extra_people: 0,
            tested_absorption_l_m2_day: None,
        }
    }
}

impl SepticSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=5).contains(&self.cleaning_years) {
            return Err(
                "O intervalo de limpeza deve ficar entre 1 e 5 anos (NBR 7229).".to_owned(),
            );
        }
        if !self.soakaway_diameter_m.is_finite()
            || !(0.80..=3.0).contains(&self.soakaway_diameter_m)
        {
            return Err("O diâmetro do sumidouro deve ficar entre 0,80 e 3,00 m.".to_owned());
        }
        if let Some(tas) = self.tested_absorption_l_m2_day
            && (!tas.is_finite() || !(5.0..=200.0).contains(&tas))
        {
            return Err(
                "A taxa de infiltração medida deve ficar entre 5 e 200 L/m²·dia.".to_owned(),
            );
        }
        if self.extra_people > 200 {
            return Err("Pessoas extras: no máximo 200.".to_owned());
        }
        if self.soakaways_at.len() > 6 {
            return Err("No máximo 6 sumidouros.".to_owned());
        }
        Ok(())
    }
}

/// Where the absorption rate comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbsorptionSource {
    Tested,
    PresumedSoil,
    Fallback,
}

/// One distance rule, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct SepticDistance {
    /// "tanque" or "sumidouro 1".
    pub element: String,
    /// "parede 12", "divisa do lote", "sumidouro 2", "tanque".
    pub to: String,
    pub distance_m: f64,
    pub needed_m: f64,
    /// The norm's own minimum (not Trazo's extra care).
    pub normative_m: f64,
}

impl SepticDistance {
    pub fn ok(&self) -> bool {
        self.distance_m + 1e-6 >= self.normative_m
    }

    pub fn comfortable(&self) -> bool {
        self.distance_m + 1e-6 >= self.needed_m
    }
}

/// The septic system, sized.
#[derive(Clone, Debug, PartialEq)]
pub struct SepticDesign {
    pub standard: SewageStandard,
    pub coldest_month: ColdestMonth,
    pub cleaning_years: u32,
    pub residents: u32,
    pub people: u32,
    pub per_person_l: f64,
    pub daily_l: f64,
    pub detention_days: f64,
    pub sludge_k_days: f64,
    pub volume_l: f64,
    pub depth_m: f64,
    pub max_depth_m: f64,
    /// Prismatic tank inside: length × width, m.
    pub length_m: f64,
    pub width_m: f64,
    /// Or a cylindrical one (precast rings) this deep and wide, m; the
    /// distances are measured from its outer face.
    pub cylinder_depth_m: f64,
    pub diameter_m: f64,
    pub absorption_l_m2_day: f64,
    pub absorption_source: AbsorptionSource,
    pub soakaway_count: u32,
    pub soakaway_diameter_m: f64,
    pub area_needed_m2: f64,
    /// Useful height below the inlet of each soakaway, m.
    pub soakaway_height_m: f64,
    /// Total excavation of each soakaway (inlet depth + useful height), m.
    pub soakaway_depth_m: f64,
    pub water_table_m: Option<f64>,
    pub collapsible: bool,
    pub distances: Vec<SepticDistance>,
    pub placed: bool,
    pub notes: Vec<String>,
}

/// Detention time `T` in days for a daily contribution (Table 2).
pub fn detention_days(daily_l: f64) -> f64 {
    match daily_l {
        d if d <= 1500.0 => 1.00,
        d if d <= 3000.0 => 0.92,
        d if d <= 4500.0 => 0.83,
        d if d <= 6000.0 => 0.75,
        d if d <= 7500.0 => 0.67,
        d if d <= 9000.0 => 0.58,
        _ => 0.50,
    }
}

/// Sludge accumulation rate `K` in days (Table 3).
pub fn sludge_rate_days(years: u32, coldest: ColdestMonth) -> f64 {
    let years = years.clamp(1, 5);
    let base = match coldest {
        ColdestMonth::Cold => 94.0,
        ColdestMonth::Mild => 65.0,
        ColdestMonth::Warm => 57.0,
    };
    base + 40.0 * f64::from(years - 1)
}

/// Useful depth range for a volume in litres (Table 4).
pub fn depth_range_m(volume_l: f64) -> (f64, f64) {
    if volume_l <= 6000.0 {
        (1.20, 2.20)
    } else if volume_l <= 10_000.0 {
        (1.50, 2.50)
    } else {
        (1.80, 2.80)
    }
}

fn round_up(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

/// The septic system, or `None` when the project has none configured.
pub fn septic_design(project: &Project) -> Option<SepticDesign> {
    let settings = project.plumbing.septic.as_ref()?;
    let residents = project
        .plumbing
        .residents
        .filter(|n| *n > 0)
        .unwrap_or((PEOPLE_PER_BEDROOM * bedroom_count(project)).max(PEOPLE_PER_BEDROOM));
    let people = residents + settings.extra_people;
    let n = f64::from(people);
    let per_person_l = settings.standard.per_person_l();
    let daily_l = n * per_person_l;
    let detention = detention_days(daily_l);
    let k = sludge_rate_days(settings.cleaning_years, settings.coldest_month);
    let volume_l = 1000.0 + n * (per_person_l * detention + k * FRESH_SLUDGE_L);
    let (depth_min, depth_max) = depth_range_m(volume_l);
    let depth = depth_min;
    let plan_area = volume_l / 1000.0 / depth;
    let width = round_up((plan_area / 2.0).sqrt().max(0.80), 0.05);
    let length = round_up((plan_area / width).max(2.0 * width), 0.05);
    // Precast rings, the usual small tank: as deep as Table 4 allows, for
    // the smallest footprint (1.10 m ≤ D ≤ 2·H).
    let cylinder_depth = depth_max;
    let diameter = round_up(
        (4.0 * volume_l / 1000.0 / cylinder_depth / std::f64::consts::PI)
            .sqrt()
            .max(1.10),
        0.05,
    );

    let presumed = project
        .structure
        .presumed_soil
        .as_ref()
        .map(crate::domain::soil::PresumedSoil::analysis);
    let (absorption, source) = match (settings.tested_absorption_l_m2_day, &presumed) {
        (Some(tested), _) => (tested, AbsorptionSource::Tested),
        (None, Some(analysis)) => (analysis.absorption_l_m2_day, AbsorptionSource::PresumedSoil),
        (None, None) => (FALLBACK_ABSORPTION_L_M2_DAY, AbsorptionSource::Fallback),
    };
    let count = settings.soakaways_at.len().max(1) as u32;
    let area_needed = daily_l / absorption.max(1.0);
    let d = settings.soakaway_diameter_m;
    let per_soakaway = area_needed / f64::from(count);
    let bottom = std::f64::consts::PI * d * d / 4.0;
    let height = round_up(
        ((per_soakaway - bottom) / (std::f64::consts::PI * d)).max(0.5),
        0.10,
    );
    let soakaway_depth = INLET_DEPTH_M + height;
    let water_table = project
        .structure
        .soil_survey
        .as_ref()
        .and_then(crate::domain::soil::SoilSurvey::governing_water_table_m);
    let collapsible = presumed.as_ref().is_some_and(|a| a.collapsible);

    // Distances, measured from the edges.
    let mut distances = Vec::new();
    let pool_walls = project.pool_wall_ids();
    let boundary: Vec<Point> = project
        .terrain
        .as_ref()
        .map(crate::domain::model::Terrain::outline)
        .unwrap_or_default();
    // Measured from the outer face of the ring tank (10 cm walls).
    let tank_radius = diameter / 2.0 + RING_WALL_M;
    let elements: Vec<(String, Point, f64, bool)> = settings
        .tank_at
        .map(|at| ("tanque".to_owned(), at, tank_radius, false))
        .into_iter()
        .chain(settings.soakaways_at.iter().enumerate().map(|(i, at)| {
            (
                format!("sumidouro {}", i + 1),
                *at,
                d / 2.0 + RING_WALL_M,
                true,
            )
        }))
        .collect();
    for (name, at, radius, soakaway) in &elements {
        // Building walls: every wall that is not an open side, a pool wall
        // or on the lot boundary (the boundary has its own rule).
        let nearest_wall = project
            .walls
            .iter()
            .filter(|w| !pool_walls.contains(&w.id) && !project.wall_is_open_side(w))
            .filter(|w| {
                boundary.len() < 3
                    || !(0..boundary.len()).any(|i| {
                        let (a, b) = (boundary[i], boundary[(i + 1) % boundary.len()]);
                        distance_to_segment(w.start, a, b) < 0.30
                            && distance_to_segment(w.end, a, b) < 0.30
                    })
            })
            .map(|w| {
                (
                    w.id,
                    distance_to_segment(*at, w.start, w.end) - w.thickness() / 2.0 - radius,
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((wall, distance)) = nearest_wall {
            let needed = if *soakaway && collapsible {
                COLLAPSIBLE_DISTANCE_M
            } else {
                MIN_DISTANCE_M
            };
            distances.push(SepticDistance {
                element: name.clone(),
                to: format!("construção (parede {wall})"),
                distance_m: distance.max(0.0),
                needed_m: needed,
                normative_m: MIN_DISTANCE_M,
            });
        }
        if boundary.len() >= 3 {
            let distance = (0..boundary.len())
                .map(|i| distance_to_segment(*at, boundary[i], boundary[(i + 1) % boundary.len()]))
                .fold(f64::INFINITY, f64::min)
                - radius;
            distances.push(SepticDistance {
                element: name.clone(),
                to: "divisa do lote".to_owned(),
                distance_m: distance.max(0.0),
                needed_m: MIN_DISTANCE_M,
                normative_m: MIN_DISTANCE_M,
            });
        }
    }
    for (i, a) in elements.iter().enumerate() {
        for b in elements.iter().skip(i + 1) {
            let distance = a.1.distance(b.1) - a.2 - b.2;
            distances.push(SepticDistance {
                element: a.0.clone(),
                to: b.0.clone(),
                distance_m: distance.max(0.0),
                needed_m: MIN_DISTANCE_M,
                normative_m: MIN_DISTANCE_M,
            });
        }
    }
    let placed = settings.tank_at.is_some() && !settings.soakaways_at.is_empty();

    let mut notes = vec![
        format!(
            "Volume útil (NBR 7229): V = 1000 + N·(C·T + K·Lf) = 1000 + {people}·({}·{} + {}·1) = {} L.",
            decimal(per_person_l, 0),
            decimal(detention, 2),
            decimal(k, 0),
            decimal(volume_l, 0)
        ),
        "Tampas de inspeção acessíveis e limpeza por caminhão limpa-fossa no intervalo escolhido; nunca entre no tanque: os gases matam.".to_owned(),
        "Sumidouro com paredes de tijolo ou anéis furados, fundo com 50 cm de brita e tampa; 20 m de distância de qualquer poço de água (NBR 13969).".to_owned(),
        "Quando a rede de esgoto chegar à rua, ligue a casa nela e desative a fossa.".to_owned(),
    ];
    match source {
        AbsorptionSource::Tested => {}
        AbsorptionSource::PresumedSoil => notes.push(format!(
            "Taxa de infiltração estimada pelo solo provável ({} L/m²·dia): faça o teste de infiltração no lugar do sumidouro (NBR 13969, Anexo A) antes de cavar.",
            decimal(absorption, 0)
        )),
        AbsorptionSource::Fallback => notes.push(format!(
            "Sem teste de infiltração nem solo informado, o Trazo usa {} L/m²·dia, o lado seguro das argilas: o teste no terreno (NBR 13969, Anexo A) decide o tamanho.",
            decimal(absorption, 0)
        )),
    }
    if collapsible {
        notes.push(format!(
            "Solo colapsível: o sumidouro molha o solo o tempo todo. Mantenha-o a pelo menos {} m das fundações (o Trazo pede mais que os {} m da norma) e longe da piscina.",
            decimal(COLLAPSIBLE_DISTANCE_M, 1),
            decimal(MIN_DISTANCE_M, 1)
        ));
    }
    if water_table.is_none() {
        notes.push(format!(
            "Confirme a profundidade do lençol freático: o fundo do sumidouro ({} m) precisa ficar {} m acima dele.",
            decimal(soakaway_depth, 1),
            decimal(WATER_TABLE_CLEARANCE_M, 1)
        ));
    }

    Some(SepticDesign {
        standard: settings.standard,
        coldest_month: settings.coldest_month,
        cleaning_years: settings.cleaning_years,
        residents,
        people,
        per_person_l,
        daily_l,
        detention_days: detention,
        sludge_k_days: k,
        volume_l,
        depth_m: depth,
        max_depth_m: depth_max,
        length_m: length,
        width_m: width,
        cylinder_depth_m: cylinder_depth,
        diameter_m: diameter,
        absorption_l_m2_day: absorption,
        absorption_source: source,
        soakaway_count: count,
        soakaway_diameter_m: d,
        area_needed_m2: area_needed,
        soakaway_height_m: height,
        soakaway_depth_m: soakaway_depth,
        water_table_m: water_table,
        collapsible,
        distances,
        placed,
        notes,
    })
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_match_nbr_7229() {
        assert!((detention_days(1500.0) - 1.0).abs() < 1e-9);
        assert!((detention_days(1501.0) - 0.92).abs() < 1e-9);
        assert!((detention_days(9001.0) - 0.50).abs() < 1e-9);
        assert!((sludge_rate_days(1, ColdestMonth::Warm) - 57.0).abs() < 1e-9);
        assert!((sludge_rate_days(1, ColdestMonth::Mild) - 65.0).abs() < 1e-9);
        assert!((sludge_rate_days(5, ColdestMonth::Cold) - 254.0).abs() < 1e-9);
        assert!((sludge_rate_days(3, ColdestMonth::Mild) - 145.0).abs() < 1e-9);
        assert_eq!(depth_range_m(6000.0), (1.20, 2.20));
        assert_eq!(depth_range_m(8000.0), (1.50, 2.50));
        assert_eq!(depth_range_m(12_000.0), (1.80, 2.80));
    }

    fn house(residents: u32) -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 30.0));
        project.plumbing.residents = Some(residents);
        project.plumbing.septic = Some(SepticSettings::default());
        project
    }

    #[test]
    fn eight_people_of_medium_standard_need_about_three_thousand_litres() {
        // N = 8, C = 130, daily = 1040 L → T = 1; K = 65 (1 year, 10–20 °C).
        // V = 1000 + 8·(130·1 + 65·1) = 2560 L.
        let design = septic_design(&house(8)).unwrap();
        assert!((design.volume_l - 2560.0).abs() < 1e-9);
        assert!((design.depth_m - 1.20).abs() < 1e-9);
        assert!(design.width_m >= 0.80 && design.length_m >= 2.0 * design.width_m - 1e-9);
        assert!(design.length_m * design.width_m * design.depth_m >= 2.56 - 1e-9);
        // Without a test or a soil: 20 L/m²·dia → 52 m² of infiltration.
        assert_eq!(design.absorption_source, AbsorptionSource::Fallback);
        assert!((design.area_needed_m2 - 52.0).abs() < 1e-9);
        assert!(!design.placed);
    }

    #[test]
    fn a_tested_absorption_rate_sizes_the_soakaway_and_distances_are_measured() {
        let mut project = house(8);
        let settings = project.plumbing.septic.as_mut().unwrap();
        settings.tested_absorption_l_m2_day = Some(40.0);
        settings.tank_at = Some(Point::new(3.0, 27.5));
        settings.soakaways_at = vec![Point::new(7.0, 27.5)];
        let design = septic_design(&project).unwrap();
        assert_eq!(design.absorption_source, AbsorptionSource::Tested);
        // 1040 / 40 = 26 m²; D = 1.5: 26 = 1.767 + 4.712·H → H = 5.14 → 5.2 m.
        assert!((design.area_needed_m2 - 26.0).abs() < 1e-9);
        assert!((design.soakaway_height_m - 5.2).abs() < 1e-9);
        assert!(design.placed);
        let boundary = design
            .distances
            .iter()
            .find(|d| d.element == "tanque" && d.to == "divisa do lote")
            .unwrap();
        // The tank is 2.5 m from the back boundary minus its half length.
        assert!(boundary.distance_m < 2.5);
        assert!(design.distances.iter().any(|d| d.to == "sumidouro 1"));
    }
}
