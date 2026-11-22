//! Comparative foundation study across spread footings, stiff raft (*radier*),
//! hand-dug bored piles (*brocas*) and deep mechanized piles (*hélice contínua*).

use super::piles::{PileCapacity, broca_option, deep_pile_option};
use super::settlement::{
    MAX_DIFFERENTIAL_SETTLEMENT_MM, MAX_FOOTING_SETTLEMENT_MM, MAX_RAFT_SETTLEMENT_MM,
    differential_settlement_mm, estimate_footing_settlement_mm, estimate_raft_settlement_mm,
};
use super::survey::{MAX_FOOTING_KPA, MIN_N_FOOTINGS, MIN_N_RAFT, SoilSurvey};
use super::{SoilKind, decimal};
use crate::domain::model::Project;
use crate::domain::wall_foundations::WallFoundationKind;

/// A kind of foundation the study compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationOption {
    Footings,
    Raft,
    BoredPiles,
    DeepPiles,
}

impl FoundationOption {
    pub fn label(self) -> &'static str {
        match self {
            Self::Footings => "Sapatas",
            Self::Raft => "Radier",
            Self::BoredPiles => "Brocas com baldrame",
            Self::DeepPiles => "Estacas profundas (hélice contínua)",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Footings => "sapatas",
            Self::Raft => "radier",
            Self::BoredPiles => "brocas",
            Self::DeepPiles => "estacas_profundas",
        }
    }
}

/// One evaluated foundation option in the comparative study.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionResult {
    pub option: FoundationOption,
    pub viable: bool,
    /// What it takes, in one concise line.
    pub summary: String,
    /// Why it does not work, or what to watch.
    pub notes: Vec<String>,
    /// Concrete volume estimate, m³ (pile caps / edge beams included).
    pub concrete_m3: f64,
    /// Allowable soil pressure used, kPa (for shallow options).
    pub soil_kpa: Option<f64>,
    /// Estimated maximum elastic settlement, mm.
    pub settlement_mm: Option<f64>,
    /// Estimated differential settlement between supports, mm (footings).
    pub differential_settlement_mm: Option<f64>,
}

/// The foundation options compared for the structure's real loads.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundationStudy {
    pub survey: bool,
    /// Number of SPT boreholes considered (`0` when using soil-type table).
    pub borehole_count: usize,
    /// Name of the weakest governing borehole for shallow foundations.
    pub governing_borehole: Option<String>,
    /// Column loads, ground beams included, kN.
    pub column_loads_kn: Vec<f64>,
    /// Walls on strip footings: `(kN/m, length m)`.
    pub strip_loads: Vec<(f64, f64)>,
    pub total_load_kn: f64,
    /// Bounding rectangle of the walls and columns, m².
    pub footprint_m2: f64,
    pub options: Vec<OptionResult>,
    pub recommended: Option<FoundationOption>,
    /// The hand-dug bored pile chosen, and how many.
    pub pile: Option<PileCapacity>,
    pub piles: u32,
    /// The deep CFA pile chosen, and how many.
    pub deep_pile: Option<PileCapacity>,
    pub deep_piles: u32,
    /// Allowable pressure for 1 m footings from the survey, kPa.
    pub survey_footing_kpa: Option<f64>,
}

impl Project {
    /// Replaces (or removes) the site's SPT soil survey.
    pub fn set_soil_survey(&mut self, survey: Option<SoilSurvey>) {
        self.structure.soil_survey = survey;
    }

    /// Takes the survey's allowable pressure for 1 m footings as the soil
    /// pressure of the structure, rounded down to 10 kPa. Returns it.
    pub fn apply_survey_to_soil(&mut self) -> Option<f64> {
        let kpa = self.structure.soil_survey.as_ref()?.footing_kpa(1.0)?;
        // Never silently clamp weak soil UP to 50 kPa (preserve real low bearing capacity).
        let kpa = ((kpa / 10.0).floor() * 10.0).clamp(10.0, 600.0);
        self.structure.settings.soil_kpa = kpa;
        Some(kpa)
    }

    /// Compares footings, a raft, hand-dug brocas and deep CFA piles for the
    /// structure's real loads.
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
        let governing_borehole = survey
            .and_then(|s| s.governing_footing_bulb(1.0))
            .map(|(_, _, name)| name);
        let mut study = FoundationStudy {
            survey: survey.is_some(),
            borehole_count: survey.map_or(0, SoilSurvey::borehole_count),
            governing_borehole,
            column_loads_kn,
            strip_loads,
            total_load_kn,
            footprint_m2,
            options: Vec::new(),
            recommended: None,
            pile: None,
            piles: 0,
            deep_pile: None,
            deep_piles: 0,
            survey_footing_kpa: survey.and_then(|s| s.footing_kpa(1.0)),
        };
        if study.column_loads_kn.is_empty() && study.strip_loads.is_empty() {
            return study;
        }
        study.options.push(self.footing_option(&study, survey));
        study.options.push(self.raft_option(&study, survey));
        let (brocas, pile, count) = broca_option(&study, survey);
        study.pile = pile;
        study.piles = count;
        study.options.push(brocas);
        let (deep, deep_pile, deep_count) = deep_pile_option(&study, survey);
        study.deep_pile = deep_pile;
        study.deep_piles = deep_count;
        study.options.push(deep);
        study.recommended = study
            .options
            .iter()
            .filter(|o| o.viable)
            .min_by(|a, b| a.concrete_m3.total_cmp(&b.concrete_m3))
            .map(|o| o.option);
        study
    }

    /// Bounding rectangle of the walls and columns, m².
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
                settlement_mm: None,
                differential_settlement_mm: None,
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
        let (n_bar, soil_kind, gov_name) =
            match survey.and_then(|s| s.governing_footing_bulb(widest)) {
                Some((n, kind, name)) => (n, kind, Some(name)),
                None => (kpa / 20.0, SoilKind::Clay, None),
            };
        // Settlement per footing under working pressure (1.05·N / B²).
        let settlements: Vec<f64> = study
            .column_loads_kn
            .iter()
            .zip(&widths)
            .map(|(load, b)| {
                let q = 1.05 * load / (b * b).max(1e-6);
                estimate_footing_settlement_mm(q, *b, *b, soil_kind, n_bar)
            })
            .collect();
        let max_s = settlements.iter().copied().fold(0.0, f64::max);
        let min_s = settlements
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .min(max_s);
        let diff_s = differential_settlement_mm(max_s, min_s);

        let mut viable = true;
        if n_bar < MIN_N_FOOTINGS - 1e-9 {
            viable = false;
            notes.push(format!(
                "Solo fraco na cota da sapata (N̄ = {:.0}, abaixo de 5): risco de recalque excessivo.",
                n_bar
            ));
        }
        if max_s > MAX_FOOTING_SETTLEMENT_MM + 1e-6 {
            viable = false;
            notes.push(format!(
                "Recalque estimado de {} mm excede o limite de {:.0} mm para sapatas isoladas.",
                decimal(max_s, 1),
                MAX_FOOTING_SETTLEMENT_MM
            ));
        }
        if diff_s > MAX_DIFFERENTIAL_SETTLEMENT_MM + 1e-6 {
            viable = false;
            notes.push(format!(
                "Recalque diferencial estimado de {} mm excede {:.0} mm: avalie radier ou estacas.",
                decimal(diff_s, 1),
                MAX_DIFFERENTIAL_SETTLEMENT_MM
            ));
        }
        if share > 0.5 {
            viable = false;
            notes.push(format!(
                "As sapatas cobririam {:.0} % da projeção: um radier sai mais simples.",
                share * 100.0
            ));
        }
        if let Some(s) = survey {
            if s.borehole_count() > 1
                && let Some(name) = gov_name
            {
                notes.push(format!("Tensão e recalque governados pelo furo {name}."));
            }
            if let Some(water) = s.governing_water_table_m()
                && water < s.footing_depth_m + 0.3
            {
                notes.push(format!(
                    "Lençol a {} m: a escavação das sapatas vai pegar água (rebaixamento).",
                    decimal(water, 2)
                ));
            }
        }
        OptionResult {
            option: FoundationOption::Footings,
            viable,
            summary: format!(
                "{} sapata(s) isolada(s){} com σadm {:.0} kPa a {} m; {} m² ({:.0} % da projeção), maior {:.0} cm, recalque est. {} mm (dif. {} mm).",
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
                widths.iter().copied().fold(0.0, f64::max) * 100.0,
                decimal(max_s, 1),
                decimal(diff_s, 1),
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: Some(kpa),
            settlement_mm: Some(max_s),
            differential_settlement_mm: Some(diff_s),
        }
    }

    fn raft_option(&self, study: &FoundationStudy, survey: Option<&SoilSurvey>) -> OptionResult {
        let settings = self.structure.settings;
        let area = study.footprint_m2;
        let mut notes = Vec::new();
        const THICKNESS: f64 = 0.15;
        let pressure = 1.05 * study.total_load_kn / area.max(1e-6) + 25.0 * THICKNESS;
        // Raft pressure bulb: check BOTH near-surface bearing zone [0.5, 3.0] m
        // and deep compressible bulb [0.5, 1.5·B_eq] m, taking the weaker (lower N̄)
        // so neither soft top clay nor soft deep strata are masked by firm layers.
        let raft_bulb_depth = (1.5 * area.max(4.0).sqrt()).max(3.0);
        let (n_bar, soil_kind, kpa) = match survey {
            Some(s) => {
                let shallow = s.governing_mean_n(0.5, 3.0);
                let to_depth = raft_bulb_depth.min(s.deepest_m().max(3.0));
                let deep = s.governing_mean_n(0.5, to_depth);
                let governing = match (shallow, deep) {
                    (Some(a), Some(b)) => {
                        if a.0 <= b.0 {
                            Some(a)
                        } else {
                            Some(b)
                        }
                    }
                    (a, b) => a.or(b),
                };
                match governing {
                    Some((n, kind, _)) => (Some(n), kind, Some((20.0 * n).min(MAX_FOOTING_KPA))),
                    None => (None, SoilKind::Clay, None),
                }
            }
            None => (
                Some(settings.soil_kpa / 20.0),
                SoilKind::Clay,
                Some(settings.soil_kpa),
            ),
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
                settlement_mm: None,
                differential_settlement_mm: None,
            };
        };
        let s_raft = estimate_raft_settlement_mm(pressure, area, soil_kind, n_bar.unwrap_or(5.0));
        if let Some(n) = n_bar
            && n < MIN_N_RAFT - 1e-9
        {
            viable = false;
            notes.push(format!(
                "Solo muito mole (N̄ = {n:.0}): mesmo um radier recalca; prefira estacas profundas."
            ));
        }
        if pressure > kpa + 1e-9 {
            viable = false;
            notes.push(format!(
                "Pressão de {pressure:.0} kPa acima dos {kpa:.0} kPa do solo."
            ));
        }
        if s_raft > MAX_RAFT_SETTLEMENT_MM + 1e-6 {
            viable = false;
            notes.push(format!(
                "Recalque estimado de {} mm excede o limite de {:.0} mm para radier.",
                decimal(s_raft, 1),
                MAX_RAFT_SETTLEMENT_MM
            ));
        }
        let perimeter = 4.0 * area.sqrt();
        let concrete = area * THICKNESS + perimeter * 0.20 * 0.40;
        OptionResult {
            option: FoundationOption::Raft,
            viable,
            summary: format!(
                "Laje de {:.0} cm sobre {} m² com vigas de borda; pressão média {:.0} kPa (σadm {:.0} kPa), recalque est. {} mm.",
                THICKNESS * 100.0,
                decimal(area, 1),
                pressure,
                kpa,
                decimal(s_raft, 1),
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: Some(kpa),
            settlement_mm: Some(s_raft),
            differential_settlement_mm: None,
        }
    }
}
