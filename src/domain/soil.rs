//! Soil from a survey and the choice of foundation (#112).
//!
//! A standard penetration test (SPT, *sondagem a percussão*, NBR 6484)
//! gives one blow count N per metre and the soil at that depth. From it:
//!
//! - **Allowable pressure for footings:** σadm = 20 · N̄ kPa (the Brazilian
//!   rule σadm = N̄ / 50 MPa, valid for 5 ≤ N̄ ≤ 20), with N̄ the mean blow
//!   count in the pressure bulb, from the footing base D down to D + 2B;
//!   capped at 400 kPa.
//! - **Bored piles (*estaca broca*):** Décourt-Quaresma, with the factors of
//!   bored piles: tip Rp = α · C · Np · Ap, shaft Rl = β · 10 · (Nl/3 + 1) ·
//!   U · L kPa; allowable = min((Rp + Rl)/2, Rp/4 + Rl/1.3), the tip taking
//!   at most 20 % of it (NBR 6122 for bored piles), and at most 5 MPa on
//!   the concrete. Hand-dug brocas: 20 to 30 cm, 3 to 6 m long, above the
//!   water table.
//!
//! The foundation study compares, for the real loads of the structure,
//! isolated and strip footings, a raft (*radier*) and bored piles under
//! ground beams, and recommends the viable option that takes the least
//! concrete. It is a first study: the foundation design belongs to a
//! licensed engineer, with a survey on the site.

use serde::{Deserialize, Serialize};

use super::model::Project;
use super::wall_foundations::WallFoundationKind;

/// Soil groups of the Décourt-Quaresma method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilKind {
    #[default]
    Clay,
    ClayeySilt,
    SandySilt,
    Sand,
}

impl SoilKind {
    pub const ALL: [Self; 4] = [Self::Clay, Self::ClayeySilt, Self::SandySilt, Self::Sand];

    pub fn label(self) -> &'static str {
        match self {
            Self::Clay => "Argila",
            Self::ClayeySilt => "Silte argiloso",
            Self::SandySilt => "Silte arenoso",
            Self::Sand => "Areia",
        }
    }

    /// The name used by the MCP tools.
    pub fn key(self) -> &'static str {
        match self {
            Self::Clay => "argila",
            Self::ClayeySilt => "silte_argiloso",
            Self::SandySilt => "silte_arenoso",
            Self::Sand => "areia",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase().replace(' ', "_");
        Self::ALL.into_iter().find(|kind| kind.key() == raw)
    }

    /// Tip coefficient C of Décourt-Quaresma, kPa.
    fn tip_c_kpa(self) -> f64 {
        match self {
            Self::Clay => 120.0,
            Self::ClayeySilt => 200.0,
            Self::SandySilt => 250.0,
            Self::Sand => 400.0,
        }
    }

    /// α (tip) and β (shaft) of Décourt (1996) for bored piles.
    fn bored_factors(self) -> (f64, f64) {
        match self {
            Self::Clay => (0.85, 0.80),
            Self::ClayeySilt | Self::SandySilt => (0.60, 0.65),
            Self::Sand => (0.50, 0.50),
        }
    }
}

/// One SPT reading: the blow count N at a depth, and the soil there.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SptReading {
    pub depth_m: f64,
    pub n: u32,
    #[serde(default)]
    pub soil: SoilKind,
}

fn default_footing_depth() -> f64 {
    1.0
}

/// A soil survey: one borehole with a reading per metre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoilSurvey {
    /// Borehole name, such as "SP-01".
    #[serde(default)]
    pub name: String,
    /// Readings sorted by depth.
    pub readings: Vec<SptReading>,
    /// Depth of the water table, metres; `None` when not found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_table_m: Option<f64>,
    /// Depth of the footings' base, metres.
    #[serde(default = "default_footing_depth")]
    pub footing_depth_m: f64,
}

/// Blow counts above this are taken as this (Décourt-Quaresma).
const MAX_N: f64 = 50.0;
/// Pressure cap for footings from the SPT rule, kPa.
const MAX_FOOTING_KPA: f64 = 400.0;
/// Below this mean blow count footings are not advised (soft soil).
pub const MIN_N_FOOTINGS: f64 = 5.0;
/// Below this mean blow count even a raft is not advised.
pub const MIN_N_RAFT: f64 = 3.0;
/// Longest hand-dug bored pile, metres.
pub const MAX_BROCA_M: f64 = 6.0;
/// Shortest bored pile, metres: below the surface layer that swells,
/// shrinks or collapses with the seasons.
pub const MIN_BROCA_M: f64 = 3.0;
/// Concrete stress allowed in a bored pile, kPa.
const PILE_CONCRETE_KPA: f64 = 5_000.0;
/// Most piles under one column before a heavier pile is needed.
const MAX_PILES_PER_COLUMN: u32 = 3;

impl SoilSurvey {
    /// A checked survey: depths from 0.5 to 40 m, increasing; N up to 100.
    pub fn new(
        name: String,
        mut readings: Vec<SptReading>,
        water_table_m: Option<f64>,
        footing_depth_m: f64,
    ) -> Result<Self, String> {
        if readings.is_empty() {
            return Err("A sondagem precisa de pelo menos uma leitura.".to_owned());
        }
        readings.sort_by(|a, b| a.depth_m.total_cmp(&b.depth_m));
        for pair in readings.windows(2) {
            if pair[1].depth_m - pair[0].depth_m < 0.2 {
                return Err(format!(
                    "Duas leituras na mesma profundidade ({:.2} m).",
                    pair[1].depth_m
                )
                .replace('.', ","));
            }
        }
        for reading in &readings {
            if !reading.depth_m.is_finite() || !(0.5..=40.0).contains(&reading.depth_m) {
                return Err("As profundidades devem ficar entre 0,5 e 40 m.".to_owned());
            }
            if reading.n > 100 {
                return Err("O N do SPT vai de 0 a 100 golpes.".to_owned());
            }
        }
        if let Some(water) = water_table_m
            && (!water.is_finite() || !(0.0..=40.0).contains(&water))
        {
            return Err("O nível d'água deve ficar entre 0 e 40 m.".to_owned());
        }
        if !footing_depth_m.is_finite() || !(0.4..=3.0).contains(&footing_depth_m) {
            return Err("A cota da sapata deve ficar entre 0,40 e 3,00 m.".to_owned());
        }
        Ok(Self {
            name,
            readings,
            water_table_m,
            footing_depth_m,
        })
    }

    pub fn deepest_m(&self) -> f64 {
        self.readings.last().map_or(0.0, |r| r.depth_m)
    }

    /// Mean blow count of the readings between `from` and `to`; when none
    /// falls there, the first reading below `from`. `None` when the survey
    /// stops above `from`.
    pub fn mean_n(&self, from: f64, to: f64) -> Option<f64> {
        let inside: Vec<f64> = self
            .readings
            .iter()
            .filter(|r| r.depth_m >= from - 1e-6 && r.depth_m <= to + 1e-6)
            .map(|r| f64::from(r.n).min(MAX_N))
            .collect();
        if inside.is_empty() {
            return self
                .readings
                .iter()
                .find(|r| r.depth_m >= from - 1e-6)
                .map(|r| f64::from(r.n).min(MAX_N));
        }
        Some(inside.iter().sum::<f64>() / inside.len() as f64)
    }

    /// Mean blow count under a footing `width` wide at the footing depth.
    pub fn footing_n(&self, width: f64) -> Option<f64> {
        let base = self.footing_depth_m;
        self.mean_n(base, base + 2.0 * width.max(0.6))
    }

    /// Allowable pressure for a footing `width` wide, kPa.
    pub fn footing_kpa(&self, width: f64) -> Option<f64> {
        self.footing_n(width)
            .map(|n| (20.0 * n).min(MAX_FOOTING_KPA))
    }

    /// The reading nearest `depth`.
    fn reading_at(&self, depth: f64) -> Option<&SptReading> {
        self.readings.iter().min_by(|a, b| {
            (a.depth_m - depth)
                .abs()
                .total_cmp(&(b.depth_m - depth).abs())
        })
    }

    /// Capacity of a bored pile `diameter` wide and `length` long, or
    /// `None` when the survey does not reach its tip.
    pub fn pile_capacity(&self, diameter: f64, length: f64) -> Option<PileCapacity> {
        if self.deepest_m() + 1e-6 < length {
            return None;
        }
        let tip_n = self.mean_n(length - 1.0, length + 1.0)?;
        let mut shaft: Vec<&SptReading> = self
            .readings
            .iter()
            .filter(|r| r.depth_m < length - 1.0 - 1e-6)
            .collect();
        if shaft.is_empty() {
            shaft = self
                .readings
                .iter()
                .filter(|r| r.depth_m <= length + 1e-6)
                .collect();
        }
        let shaft_n = shaft
            .iter()
            .map(|r| f64::from(r.n).clamp(3.0, MAX_N))
            .sum::<f64>()
            / shaft.len().max(1) as f64;
        let beta =
            shaft.iter().map(|r| r.soil.bored_factors().1).sum::<f64>() / shaft.len().max(1) as f64;
        let tip_soil = self.reading_at(length)?.soil;
        let alpha = tip_soil.bored_factors().0;
        let area = std::f64::consts::PI * diameter * diameter / 4.0;
        let perimeter = std::f64::consts::PI * diameter;
        let tip_kn = alpha * tip_soil.tip_c_kpa() * tip_n * area;
        let shaft_kn = beta * 10.0 * (shaft_n / 3.0 + 1.0) * perimeter * length;
        let geotechnical = ((tip_kn + shaft_kn) / 2.0)
            .min(tip_kn / 4.0 + shaft_kn / 1.3)
            // Bored piles: the tip takes at most 20 % of the allowable load.
            .min(1.25 * shaft_kn / 2.0);
        let structural_kn = PILE_CONCRETE_KPA * area;
        Some(PileCapacity {
            diameter_m: diameter,
            length_m: length,
            tip_n,
            shaft_n,
            tip_kn,
            shaft_kn,
            structural_kn,
            allowable_kn: geotechnical.min(structural_kn),
        })
    }

    /// Longest hand-dug pile this survey allows: 6 m, above the water
    /// table and within the readings.
    pub fn max_broca_m(&self) -> f64 {
        let mut limit = MAX_BROCA_M.min(self.deepest_m());
        if let Some(water) = self.water_table_m {
            limit = limit.min(water);
        }
        limit.floor()
    }
}

/// Bearing capacity of one bored pile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PileCapacity {
    pub diameter_m: f64,
    pub length_m: f64,
    pub tip_n: f64,
    pub shaft_n: f64,
    /// Ultimate tip and shaft resistance, kN.
    pub tip_kn: f64,
    pub shaft_kn: f64,
    /// Limit of the concrete, kN.
    pub structural_kn: f64,
    /// Allowable load, kN.
    pub allowable_kn: f64,
}

/// A kind of foundation the study compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationOption {
    Footings,
    Raft,
    BoredPiles,
}

impl FoundationOption {
    pub fn label(self) -> &'static str {
        match self {
            Self::Footings => "Sapatas",
            Self::Raft => "Radier",
            Self::BoredPiles => "Brocas com baldrame",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Footings => "sapatas",
            Self::Raft => "radier",
            Self::BoredPiles => "brocas",
        }
    }
}

/// One option of the study.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionResult {
    pub option: FoundationOption,
    pub viable: bool,
    /// What it takes, in a line.
    pub summary: String,
    /// Why it does not work, or what to watch.
    pub notes: Vec<String>,
    /// Concrete it takes, m³ (pile caps included).
    pub concrete_m3: f64,
    /// Allowable soil pressure used, kPa.
    pub soil_kpa: Option<f64>,
}

/// The foundation options compared for the structure's loads.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundationStudy {
    pub survey: bool,
    /// Column loads, ground beams included, kN.
    pub column_loads_kn: Vec<f64>,
    /// Walls on strip footings: (kN/m, length m).
    pub strip_loads: Vec<(f64, f64)>,
    pub total_load_kn: f64,
    /// Bounding rectangle of the walls and columns, m².
    pub footprint_m2: f64,
    pub options: Vec<OptionResult>,
    pub recommended: Option<FoundationOption>,
    /// The bored pile chosen, and how many.
    pub pile: Option<PileCapacity>,
    pub piles: u32,
    /// Allowable pressure for footings from the survey, kPa.
    pub survey_footing_kpa: Option<f64>,
}

/// Concrete in a pile cap for `n` piles of diameter `d`, m³.
fn cap_m3(n: u32, d: f64) -> f64 {
    let side = (d + 0.30).max(0.45);
    match n {
        0 => 0.0,
        1 => side * side * 0.40,
        2 => side * (3.0 * d + 0.30 + side) * 0.45,
        _ => (3.0 * d + 0.60).powi(2) * 0.50,
    }
}

impl Project {
    /// Replaces (or removes) the soil survey.
    pub fn set_soil_survey(&mut self, survey: Option<SoilSurvey>) {
        self.structure.soil_survey = survey;
    }

    /// Takes the survey's allowable pressure for 1 m footings as the soil
    /// pressure of the structure, rounded down to 10 kPa. Returns it.
    pub fn apply_survey_to_soil(&mut self) -> Option<f64> {
        let kpa = self.structure.soil_survey.as_ref()?.footing_kpa(1.0)?;
        let kpa = ((kpa / 10.0).floor() * 10.0).clamp(50.0, 600.0);
        self.structure.settings.soil_kpa = kpa;
        Some(kpa)
    }

    /// Compares footings, a raft and bored piles for the structure's loads.
    pub fn foundation_study(&self) -> FoundationStudy {
        let report = self.cached_structure_report();
        let ground = self.wall_foundation_effects();
        let column_loads_kn: Vec<f64> = report
            .columns
            .iter()
            .map(|c| c.load_kn + ground.at_column.get(&c.id).copied().unwrap_or(0.0))
            .collect();
        let strip_loads: Vec<(f64, f64)> = ground
            .results
            .iter()
            .filter(|r| r.kind == WallFoundationKind::StripFooting)
            .map(|r| (r.load_kn_m, r.length_m))
            .collect();
        let total_load_kn = column_loads_kn.iter().sum::<f64>()
            + strip_loads.iter().map(|(q, l)| q * l).sum::<f64>();
        let footprint_m2 = self.foundation_footprint_m2();
        let survey = self.structure.soil_survey.as_ref();
        let mut study = FoundationStudy {
            survey: survey.is_some(),
            column_loads_kn,
            strip_loads,
            total_load_kn,
            footprint_m2,
            options: Vec::new(),
            recommended: None,
            pile: None,
            piles: 0,
            survey_footing_kpa: survey.and_then(|s| s.footing_kpa(1.0)),
        };
        if study.column_loads_kn.is_empty() && study.strip_loads.is_empty() {
            return study;
        }
        study.options.push(self.footing_option(&study, survey));
        study.options.push(self.raft_option(&study, survey));
        let (piles, pile, count) = pile_option(&study, survey);
        study.pile = pile;
        study.piles = count;
        study.options.push(piles);
        study.recommended = study
            .options
            .iter()
            .filter(|o| o.viable)
            .min_by(|a, b| a.concrete_m3.total_cmp(&b.concrete_m3))
            .map(|o| o.option);
        study
    }

    /// Bounding rectangle of the walls and the columns, m².
    fn foundation_footprint_m2(&self) -> f64 {
        let points = self
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .chain(self.structural_columns().map(|c| c.center));
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        for p in points {
            bounds = Some(match bounds {
                None => (p.x, p.y, p.x, p.y),
                Some((x0, y0, x1, y1)) => (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
            });
        }
        bounds.map_or(0.0, |(x0, y0, x1, y1)| (x1 - x0) * (y1 - y0))
    }

    fn footing_option(&self, study: &FoundationStudy, survey: Option<&SoilSurvey>) -> OptionResult {
        let settings = self.structure.settings;
        let mut notes = Vec::new();
        // First with 1 m footings, then with the widest one found.
        let pressure = |width: f64| match survey {
            Some(s) => s.footing_kpa(width),
            None => Some(settings.soil_kpa),
        };
        let size = |kpa: f64| {
            let widths: Vec<f64> = study
                .column_loads_kn
                .iter()
                .map(|n| (1.05 * n / kpa).sqrt().max(0.60))
                .collect();
            let strips: Vec<f64> = study
                .strip_loads
                .iter()
                .map(|(q, _)| (1.05 * q / kpa).max(0.40))
                .collect();
            (widths, strips)
        };
        let Some(first) = pressure(1.0) else {
            return OptionResult {
                option: FoundationOption::Footings,
                viable: false,
                summary: "A sondagem não chega à cota das sapatas.".to_owned(),
                notes,
                concrete_m3: 0.0,
                soil_kpa: None,
            };
        };
        let (widths, _) = size(first.max(10.0));
        let widest = widths.iter().copied().fold(1.0, f64::max);
        let kpa = pressure(widest).unwrap_or(first).max(10.0);
        let (widths, strips) = size(kpa);
        let area: f64 = widths.iter().map(|b| b * b).sum::<f64>()
            + strips
                .iter()
                .zip(&study.strip_loads)
                .map(|(b, (_, l))| b * l)
                .sum::<f64>();
        let concrete: f64 = widths
            .iter()
            .map(|b| b * b * ((b - 0.20) / 3.0).max(0.25))
            .sum::<f64>()
            + strips
                .iter()
                .zip(&study.strip_loads)
                .map(|(b, (_, l))| b * 0.25 * l)
                .sum::<f64>();
        let share = area / study.footprint_m2.max(1e-6);
        let n_bar = survey
            .and_then(|s| s.footing_n(widest))
            .unwrap_or(kpa / 20.0);
        let mut viable = true;
        if n_bar < MIN_N_FOOTINGS - 1e-9 {
            viable = false;
            notes.push(format!(
                "Solo fraco na cota da sapata (N̄ = {:.0}, abaixo de 5): risco de recalque.",
                n_bar
            ));
        }
        if share > 0.5 {
            viable = false;
            notes.push(format!(
                "As sapatas cobririam {:.0} % da projeção: um radier sai mais simples.",
                share * 100.0
            ));
        }
        if let Some(s) = survey
            && let Some(water) = s.water_table_m
            && water < s.footing_depth_m + 0.3
        {
            notes.push(format!(
                "Lençol a {} m: a escavação das sapatas vai pegar água (rebaixamento).",
                decimal(water, 2)
            ));
        }
        OptionResult {
            option: FoundationOption::Footings,
            viable,
            summary: format!(
                "{} sapata(s) isolada(s){} com σadm {:.0} kPa a {} m; {} m² ({:.0} % da projeção), a maior com {:.0} cm.",
                widths.len(),
                if strips.is_empty() {
                    String::new()
                } else {
                    format!(" e {} sapata(s) corrida(s)", strips.len())
                },
                kpa,
                decimal(survey.map_or(1.0, |s| s.footing_depth_m), 2),
                decimal(area, 1),
                share * 100.0,
                widths.iter().copied().fold(0.0, f64::max) * 100.0
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: Some(kpa),
        }
    }

    fn raft_option(&self, study: &FoundationStudy, survey: Option<&SoilSurvey>) -> OptionResult {
        let settings = self.structure.settings;
        let area = study.footprint_m2;
        let mut notes = Vec::new();
        const THICKNESS: f64 = 0.15;
        let pressure = 1.05 * study.total_load_kn / area.max(1e-6) + 25.0 * THICKNESS;
        let (n_bar, kpa) = match survey {
            Some(s) => {
                let n = s.mean_n(0.5, 3.0);
                (n, n.map(|n| (20.0 * n).min(MAX_FOOTING_KPA)))
            }
            None => (Some(settings.soil_kpa / 20.0), Some(settings.soil_kpa)),
        };
        let mut viable = area >= 4.0;
        if area < 4.0 {
            notes.push("Projeção pequena demais para um radier.".to_owned());
        }
        let Some(kpa) = kpa else {
            return OptionResult {
                option: FoundationOption::Raft,
                viable: false,
                summary: "A sondagem não cobre os primeiros metros.".to_owned(),
                notes,
                concrete_m3: 0.0,
                soil_kpa: None,
            };
        };
        if let Some(n) = n_bar
            && n < MIN_N_RAFT - 1e-9
        {
            viable = false;
            notes.push(format!(
                "Argila muito mole (N̄ = {n:.0}): mesmo um radier recalca; consulte um engenheiro de fundações."
            ));
        }
        if pressure > kpa + 1e-9 {
            viable = false;
            notes.push(format!(
                "Pressão de {pressure:.0} kPa acima dos {kpa:.0} kPa do solo."
            ));
        }
        let perimeter = {
            // Perimeter of the bounding rectangle with this area, as a square.
            4.0 * area.sqrt()
        };
        let concrete = area * THICKNESS + perimeter * 0.20 * 0.40;
        OptionResult {
            option: FoundationOption::Raft,
            viable,
            summary: format!(
                "Laje de {:.0} cm sobre {} m² com vigas de borda; pressão média {:.0} kPa para σadm {:.0} kPa.",
                THICKNESS * 100.0,
                decimal(area, 1),
                pressure,
                kpa
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: Some(kpa),
        }
    }
}

/// The bored-pile option: the hand-dug pile (20 to 30 cm, 3 to 6 m) that
/// takes the least concrete with at most three piles per column, piles
/// under the strip walls at most 2.5 m apart.
fn pile_option(
    study: &FoundationStudy,
    survey: Option<&SoilSurvey>,
) -> (OptionResult, Option<PileCapacity>, u32) {
    let fail = |summary: String, notes: Vec<String>| {
        (
            OptionResult {
                option: FoundationOption::BoredPiles,
                viable: false,
                summary,
                notes,
                concrete_m3: 0.0,
                soil_kpa: None,
            },
            None,
            0,
        )
    };
    let Some(survey) = survey else {
        return fail(
            "Sem sondagem: informe o SPT para dimensionar as brocas.".to_owned(),
            Vec::new(),
        );
    };
    let max_length = survey.max_broca_m();
    if max_length < MIN_BROCA_M {
        return fail(
            format!(
                "Brocas precisam de pelo menos 3 m acima do lençol e dentro da sondagem (até {} m).",
                decimal(max_length, 1)
            ),
            Vec::new(),
        );
    }
    let mut best: Option<(f64, PileCapacity, u32)> = None;
    for diameter in [0.20, 0.25, 0.30] {
        let mut length = MIN_BROCA_M;
        while length <= max_length + 1e-9 {
            if let Some(pile) = survey.pile_capacity(diameter, length)
                && pile.allowable_kn > 1.0
            {
                let area = std::f64::consts::PI * diameter * diameter / 4.0;
                let mut fits = true;
                let mut count = 0;
                let mut concrete = 0.0;
                for load in &study.column_loads_kn {
                    let n = ((load / pile.allowable_kn) - 1e-9).ceil().max(1.0) as u32;
                    if n > MAX_PILES_PER_COLUMN {
                        fits = false;
                        break;
                    }
                    count += n;
                    concrete += f64::from(n) * area * length + cap_m3(n, diameter);
                }
                for (q, l) in &study.strip_loads {
                    let spacing = (pile.allowable_kn / q.max(1e-6)).min(2.5);
                    if spacing < 3.0 * diameter {
                        fits = false;
                        break;
                    }
                    let n = (l / spacing - 1e-9).ceil() as u32 + 1;
                    count += n;
                    // The ground beam over them: 15 × 30 cm.
                    concrete += f64::from(n) * area * length + 0.15 * 0.30 * l;
                }
                if fits && best.as_ref().is_none_or(|(c, _, _)| concrete < *c - 1e-9) {
                    best = Some((concrete, pile, count));
                }
            }
            length += 1.0;
        }
    }
    let Some((concrete, pile, count)) = best else {
        return fail(
            format!(
                "Nenhuma broca de 20 a 30 cm com até {} m resolve com até 3 por pilar: estaca mecanizada (hélice contínua, escavada) — consulte um engenheiro.",
                decimal(max_length, 0)
            ),
            Vec::new(),
        );
    };
    let mut notes =
        vec!["Cada pilar fica num bloco sobre as brocas, ligado pelas vigas baldrame.".to_owned()];
    if survey.water_table_m.is_some() {
        notes.push("Brocas só acima do lençol; o comprimento já respeita isso.".to_owned());
    }
    (
        OptionResult {
            option: FoundationOption::BoredPiles,
            viable: true,
            summary: format!(
                "{count} broca(s) de Ø {:.0} cm × {} m, {:.0} kN cada (ponta {:.0} kN, atrito {:.0} kN na ruptura).",
                pile.diameter_m * 100.0,
                decimal(pile.length_m, 0),
                pile.allowable_kn,
                pile.tip_kn,
                pile.shaft_kn
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: None,
        },
        Some(pile),
        count,
    )
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    fn reading(depth: f64, n: u32, soil: SoilKind) -> SptReading {
        SptReading {
            depth_m: depth,
            n,
            soil,
        }
    }

    fn survey(ns: &[u32], soil: SoilKind) -> SoilSurvey {
        let readings = ns
            .iter()
            .enumerate()
            .map(|(i, n)| reading(i as f64 + 1.0, *n, soil))
            .collect();
        SoilSurvey::new("SP-01".to_owned(), readings, None, 1.0).unwrap()
    }

    #[test]
    fn the_footing_pressure_follows_the_mean_blow_count_in_the_bulb() {
        let s = survey(&[6, 8, 10, 12, 30], SoilKind::Clay);
        // 1 m wide at 1 m: readings from 1 to 3 m → N̄ = 8 → 160 kPa.
        assert!((s.footing_kpa(1.0).unwrap() - 160.0).abs() < 1e-9);
        // Capped at 400 kPa.
        let hard = survey(&[40, 40, 40], SoilKind::Sand);
        assert!((hard.footing_kpa(1.0).unwrap() - 400.0).abs() < 1e-9);
        // A survey that stops above the base gives nothing.
        let short =
            SoilSurvey::new("x".into(), vec![reading(0.5, 5, SoilKind::Clay)], None, 1.0).unwrap();
        assert!(short.footing_kpa(1.0).is_none());
    }

    #[test]
    fn decourt_quaresma_matches_a_hand_calculation() {
        // Clay, N = 4, 6, 8, 10, 12, 14 from 1 to 6 m; Ø 25 cm × 4 m.
        let s = survey(&[4, 6, 8, 10, 12, 14], SoilKind::Clay);
        let pile = s.pile_capacity(0.25, 4.0).unwrap();
        // Tip: N at 3, 4 and 5 m → 10; Rp = 0.85 × 120 × 10 × 0.0491 = 50.1 kN.
        let area = std::f64::consts::PI * 0.25 * 0.25 / 4.0;
        assert!((pile.tip_kn - 0.85 * 120.0 * 10.0 * area).abs() < 1e-9);
        // Shaft: N at 1 and 2 m → 5; Rl = 0.8 × 10 × (5/3 + 1) × π·0.25 × 4.
        let shaft = 0.8 * 10.0 * (5.0 / 3.0 + 1.0) * std::f64::consts::PI * 0.25 * 4.0;
        assert!((pile.shaft_kn - shaft).abs() < 1e-9);
        let expected = ((pile.tip_kn + shaft) / 2.0)
            .min(pile.tip_kn / 4.0 + shaft / 1.3)
            .min(1.25 * shaft / 2.0);
        assert!((pile.allowable_kn - expected).abs() < 1e-9);
        // Too long for the survey.
        assert!(s.pile_capacity(0.25, 7.0).is_none());
    }

    #[test]
    fn brocas_stay_above_the_water_table_and_within_six_metres() {
        let mut s = survey(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 5], SoilKind::Clay);
        assert!((s.max_broca_m() - 6.0).abs() < 1e-9);
        s.water_table_m = Some(3.4);
        assert!((s.max_broca_m() - 3.0).abs() < 1e-9);
        assert!(SoilSurvey::new("x".into(), vec![], None, 1.0).is_err());
        assert!(
            SoilSurvey::new(
                "x".into(),
                vec![
                    reading(1.0, 5, SoilKind::Clay),
                    reading(1.0, 6, SoilKind::Clay)
                ],
                None,
                1.0
            )
            .is_err()
        );
    }

    fn small_house() -> Project {
        let mut project = Project::default();
        project.wall_height = 2.8;
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        for i in 0..4 {
            project.add_wall(corners[i], corners[(i + 1) % 4]).unwrap();
        }
        project.generate_structure();
        assert!(project.structural_columns().count() >= 4);
        project
    }

    #[test]
    fn good_soil_recommends_footings_and_soft_soil_rules_them_out() {
        let mut project = small_house();
        project.set_soil_survey(Some(survey(&[10, 12, 14, 16, 18, 20], SoilKind::Sand)));
        let study = project.foundation_study();
        assert!(study.total_load_kn > 0.0);
        let footings = &study.options[0];
        assert!(footings.viable, "{footings:?}");
        assert_eq!(study.recommended, Some(FoundationOption::Footings));

        project.set_soil_survey(Some(survey(&[2, 3, 4, 12, 15, 18], SoilKind::Clay)));
        let study = project.foundation_study();
        assert!(!study.options[0].viable);
        assert!(study.options[0].notes[0].contains("Solo fraco"));
        let piles = &study.options[2];
        assert!(piles.viable, "{piles:?}");
        assert!(study.pile.is_some() && study.piles >= 4);
        assert!(study.recommended.is_some());
    }

    #[test]
    fn the_survey_sets_the_soil_pressure_of_the_structure() {
        let mut project = small_house();
        project.set_soil_survey(Some(survey(&[6, 7, 8, 9], SoilKind::Clay)));
        // N̄ from 1 to 3 m = 7 → 140 kPa.
        assert_eq!(project.apply_survey_to_soil(), Some(140.0));
        assert!((project.structure.settings.soil_kpa - 140.0).abs() < 1e-9);
    }
}
