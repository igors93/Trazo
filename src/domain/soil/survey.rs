//! Single- and multi-borehole SPT soil survey (ABNT NBR 6484).
//!
//! Each borehole stores one blow count `N` per metre, its soil group, and
//! optional water-table depth. When a site has several boreholes (`SP-01`,
//! `SP-02`, ...), queries return the **conservative governing envelope**
//! across boreholes: lowest mean `N` in the pressure bulb, shallowest water
//! table, and minimum deepest explored depth (NBR 6122 §5.2).

use serde::{Deserialize, Serialize};

use super::SoilKind;

/// Blow counts above this are capped (Décourt-Quaresma).
pub(crate) const MAX_N: f64 = 50.0;
/// Allowable pressure cap for footings from the SPT rule, kPa.
pub const MAX_FOOTING_KPA: f64 = 400.0;
/// Below this mean blow count footings are not advised (soft soil).
pub const MIN_N_FOOTINGS: f64 = 5.0;
/// Below this mean blow count even a raft is not advised.
pub const MIN_N_RAFT: f64 = 3.0;

/// One SPT reading: the blow count `N` at a depth, and the soil there.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SptReading {
    pub depth_m: f64,
    pub n: u32,
    #[serde(default)]
    pub soil: SoilKind,
}

/// One SPT borehole (*furo de sondagem*, e.g. `SP-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Borehole {
    #[serde(default)]
    pub name: String,
    pub readings: Vec<SptReading>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_table_m: Option<f64>,
}

impl Borehole {
    /// Validates and sorts readings by increasing depth (0.5–40 m, `N` 0–100).
    pub fn new(
        name: String,
        mut readings: Vec<SptReading>,
        water_table_m: Option<f64>,
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
        Ok(Self {
            name: if name.trim().is_empty() {
                "SP-01".to_owned()
            } else {
                name.trim().to_owned()
            },
            readings,
            water_table_m,
        })
    }

    pub fn deepest_m(&self) -> f64 {
        self.readings.last().map_or(0.0, |r| r.depth_m)
    }

    /// Mean blow count of the readings between `from` and `to`; when none
    /// falls there, the first reading below `from`. `None` when the borehole
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

    /// The reading nearest `depth`.
    pub(crate) fn reading_at(&self, depth: f64) -> Option<&SptReading> {
        self.readings.iter().min_by(|a, b| {
            (a.depth_m - depth)
                .abs()
                .total_cmp(&(b.depth_m - depth).abs())
        })
    }

    /// Representative (most conservative) soil in `[from, to]`.
    pub fn dominant_soil(&self, from: f64, to: f64) -> SoilKind {
        let mut inside = self
            .readings
            .iter()
            .filter(|r| r.depth_m >= from - 1e-6 && r.depth_m <= to + 1e-6)
            .peekable();
        if inside.peek().is_none() {
            return self
                .reading_at(from)
                .map_or(SoilKind::Clay, |reading| reading.soil);
        }
        // Choose the soil with lowest elastic modulus (most compressible / conservative).
        inside
            .min_by(|a, b| {
                a.soil
                    .modulus_mpa_per_n()
                    .total_cmp(&b.soil.modulus_mpa_per_n())
            })
            .map_or(SoilKind::Clay, |reading| reading.soil)
    }
}

fn default_footing_depth() -> f64 {
    1.0
}

/// Site SPT survey holding one primary borehole (`name`, `readings`,
/// `water_table_m`) and any number of additional boreholes (`extra_boreholes`).
///
/// The flat primary fields preserve 100% JSON compatibility with existing
/// `.trazo.json` project files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoilSurvey {
    /// Primary borehole name, such as `"SP-01"`.
    #[serde(default)]
    pub name: String,
    /// Primary borehole readings sorted by depth.
    pub readings: Vec<SptReading>,
    /// Primary borehole water-table depth, metres; `None` when not found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_table_m: Option<f64>,
    /// Depth of the footings' base, metres.
    #[serde(default = "default_footing_depth")]
    pub footing_depth_m: f64,
    /// Additional boreholes on the same site (`SP-02`, `SP-03`, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_boreholes: Vec<Borehole>,
}

impl SoilSurvey {
    /// Creates a checked single-borehole survey.
    pub fn new(
        name: String,
        readings: Vec<SptReading>,
        water_table_m: Option<f64>,
        footing_depth_m: f64,
    ) -> Result<Self, String> {
        let primary = Borehole::new(name, readings, water_table_m)?;
        Self::from_boreholes(vec![primary], footing_depth_m)
    }

    /// Creates a checked survey from one or more boreholes (`SP-01`, `SP-02`, ...).
    pub fn from_boreholes(boreholes: Vec<Borehole>, footing_depth_m: f64) -> Result<Self, String> {
        if boreholes.is_empty() {
            return Err("A sondagem precisa de pelo menos um furo.".to_owned());
        }
        if !footing_depth_m.is_finite() || !(0.4..=3.0).contains(&footing_depth_m) {
            return Err("A cota da sapata deve ficar entre 0,40 e 3,00 m.".to_owned());
        }
        let mut validated = Vec::with_capacity(boreholes.len());
        for (idx, b) in boreholes.into_iter().enumerate() {
            let name = if b.name.trim().is_empty() {
                format!("SP-{:02}", idx + 1)
            } else {
                b.name
            };
            validated.push(Borehole::new(name, b.readings, b.water_table_m)?);
        }
        let primary = validated.remove(0);
        Ok(Self {
            name: primary.name,
            readings: primary.readings,
            water_table_m: primary.water_table_m,
            footing_depth_m,
            extra_boreholes: validated,
        })
    }

    /// All boreholes on the site (primary first, then additional boreholes).
    pub fn boreholes(&self) -> Vec<Borehole> {
        let mut all = Vec::with_capacity(1 + self.extra_boreholes.len());
        all.push(Borehole {
            name: if self.name.trim().is_empty() {
                "SP-01".to_owned()
            } else {
                self.name.clone()
            },
            readings: self.readings.clone(),
            water_table_m: self.water_table_m,
        });
        all.extend(self.extra_boreholes.iter().cloned());
        all
    }

    pub fn borehole_count(&self) -> usize {
        1 + self.extra_boreholes.len()
    }

    /// Shallowest explored depth across all boreholes (conservative for piles).
    pub fn deepest_m(&self) -> f64 {
        self.boreholes()
            .into_iter()
            .map(|b| b.deepest_m())
            .fold(f64::INFINITY, f64::min)
            .min(40.0)
    }

    /// Shallowest water table found across all boreholes (conservative).
    pub fn governing_water_table_m(&self) -> Option<f64> {
        self.boreholes()
            .into_iter()
            .filter_map(|b| b.water_table_m)
            .min_by(f64::total_cmp)
    }

    /// Conservative minimum mean blow count across all boreholes in `[from, to]`.
    pub fn mean_n(&self, from: f64, to: f64) -> Option<f64> {
        self.governing_mean_n(from, to).map(|(n, _, _)| n)
    }

    /// Returns `(mean_n, dominant_soil, borehole_name)` of the weakest borehole
    /// in `[from, to]`. Returns `None` if any borehole fails to reach `from`.
    pub fn governing_mean_n(&self, from: f64, to: f64) -> Option<(f64, SoilKind, String)> {
        let mut weakest: Option<(f64, SoilKind, String)> = None;
        for b in self.boreholes() {
            let n = b.mean_n(from, to)?;
            let soil = b.dominant_soil(from, to);
            if weakest.as_ref().is_none_or(|(old, _, _)| n < *old) {
                weakest = Some((n, soil, b.name));
            }
        }
        weakest
    }

    /// Mean blow count under a footing `width` wide at `footing_depth_m`
    /// (weakest borehole envelope).
    pub fn footing_n(&self, width: f64) -> Option<f64> {
        let base = self.footing_depth_m;
        self.mean_n(base, base + 2.0 * width.max(0.6))
    }

    /// Weakest borehole `(n_bar, soil, borehole_name)` in the footing pressure bulb.
    pub fn governing_footing_bulb(&self, width: f64) -> Option<(f64, SoilKind, String)> {
        let base = self.footing_depth_m;
        self.governing_mean_n(base, base + 2.0 * width.max(0.6))
    }

    /// Allowable pressure for a footing `width` wide, kPa (`σ_adm = 20·N̄ ≤ 400 kPa`).
    pub fn footing_kpa(&self, width: f64) -> Option<f64> {
        self.footing_n(width)
            .map(|n| (20.0 * n).min(MAX_FOOTING_KPA))
    }
}
