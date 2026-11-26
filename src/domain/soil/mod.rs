//! Soil from a survey (SPT) or soil-type table, elastic settlement estimation,
//! and comparative foundation study (#112).
//!
//! Organised into focused submodules:
//! - [`survey`]: single- and multi-borehole SPT data (NBR 6484) with
//!   conservative governing-envelope queries across boreholes.
//! - [`settlement`]: immediate elastic and differential settlement estimates
//!   for footings, rafts and piles (NBR 6122 / Teixeira & Godoy).
//! - [`piles`]: Décourt-Quaresma capacity and sizing for hand-dug bored piles
//!   (*brocas*, 3–6 m above water table) and mechanized deep piles (*hélice
//!   contínua*, 6–20 m, allowed below the water table).
//! - [`study`]: comparative study across footings, raft (*radier*), brocas and
//!   deep piles for the structure's real loads.
//! - [`presumed`]: the presumed profile from a qualitative description when
//!   there is no survey yet; it only ever makes the calculations safer.

pub mod piles;
pub mod presumed;
pub mod settlement;
pub mod study;
pub mod survey;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use piles::PileCapacity;
pub use presumed::{Likelihood, PresumedLayer, PresumedSoil, PresumedSoilType};
pub use settlement::{MAX_FOOTING_SETTLEMENT_MM, estimate_footing_settlement_mm};
pub use study::FoundationStudy;
pub use survey::{Borehole, MAX_FOOTING_KPA, SoilSurvey, SptReading};

/// Soil groups of the Décourt-Quaresma and elastic-settlement correlations.
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

    /// Tip coefficient `C` of Décourt-Quaresma, kPa.
    pub(crate) fn tip_c_kpa(self) -> f64 {
        match self {
            Self::Clay => 120.0,
            Self::ClayeySilt => 200.0,
            Self::SandySilt => 250.0,
            Self::Sand => 400.0,
        }
    }

    /// `α` (tip) and `β` (shaft) of Décourt (1996) for hand-dug bored piles (*brocas*).
    pub(crate) fn bored_factors(self) -> (f64, f64) {
        match self {
            Self::Clay => (0.85, 0.80),
            Self::ClayeySilt | Self::SandySilt => (0.60, 0.65),
            Self::Sand => (0.50, 0.50),
        }
    }

    /// `α` (tip) and `β` (shaft) of Décourt (1996) for continuous flight auger
    /// (*hélice contínua*) deep piles.
    pub(crate) fn cfa_factors(self) -> (f64, f64) {
        let _ = self;
        (0.30, 1.00)
    }

    /// Empirical elastic deformation modulus per SPT blow (`E_s / N`, MPa),
    /// calibrated to Brazilian practice (Teixeira & Godoy).
    pub(crate) fn modulus_mpa_per_n(self) -> f64 {
        match self {
            Self::Clay => 2.5,
            Self::ClayeySilt => 3.0,
            Self::SandySilt => 4.0,
            Self::Sand => 5.0,
        }
    }

    /// Typical drained/effective Poisson's ratio `ν`.
    pub(crate) fn poisson_ratio(self) -> f64 {
        match self {
            Self::Clay => 0.40,
            Self::ClayeySilt => 0.35,
            Self::SandySilt => 0.32,
            Self::Sand => 0.30,
        }
    }
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}
