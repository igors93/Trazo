//! Water, sewage and rain (roadmap section E, milestone M6): the plumbing
//! points of every fixture (#72), cold water (#118), hot water (#119),
//! sewage (#120), the septic system (#121), rainwater (#122), and the
//! settings the plumbing design keeps with the project.
//!
//! The tables come from the plumbing building code pack (NBR 5626, NBR 8160
//! and NBR 10844, `data/codes/plumbing/nbr.json`); the few the pack does not
//! carry are Trazo's, and say so where they are used.

pub mod cold_water;
pub mod hot_water;
pub mod network;
pub mod points;
pub mod rainwater;
pub mod rules;
pub mod septic;
pub mod sewage;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::domain::geometry::Point;

/// How the house heats its water.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heater {
    /// Electric showers: no hot-water pipes, the usual case.
    #[default]
    ElectricShower,
    /// A gas instantaneous heater (*aquecedor de passagem*).
    Gas,
    /// Solar collectors with a hot-water tank (*boiler*).
    Solar,
}

impl Heater {
    pub const ALL: [Heater; 3] = [Heater::ElectricShower, Heater::Gas, Heater::Solar];

    pub fn label(self) -> &'static str {
        match self {
            Self::ElectricShower => "Chuveiro elétrico",
            Self::Gas => "Aquecedor a gás",
            Self::Solar => "Aquecedor solar",
        }
    }

    /// The name tools and files use.
    pub fn key(self) -> &'static str {
        match self {
            Self::ElectricShower => "chuveiro_eletrico",
            Self::Gas => "gas",
            Self::Solar => "solar",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.key() == key.trim())
    }

    /// Whether the house has hot-water pipes to the fixtures.
    pub fn pipes_hot_water(self) -> bool {
        self != Self::ElectricShower
    }
}

fn is_default_heater(heater: &Heater) -> bool {
    *heater == Heater::default()
}

/// The plumbing design's settings, saved with the project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlumbingSettings {
    /// How the water is heated.
    #[serde(default, skip_serializing_if = "is_default_heater")]
    pub heater: Heater,
    /// People living in the house; two per bedroom when not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residents: Option<u32>,
    /// Water a person uses in a day, L.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumption_l_day: Option<f64>,
    /// Days of water the tank keeps (NBR 5626: one to three).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve_days: Option<f64>,
    /// Where the water tank stands, in the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tank_at: Option<Point>,
    /// Height of the tank's base over the floor, m.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tank_base_m: Option<f64>,
    /// Where the gas heater, or the solar hot-water tank, stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heater_at: Option<Point>,
    /// Where the sewage main meets the street sewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sewer_exit: Option<Point>,
    /// Wells and springs on the lot and next to it (the neighbours' too).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wells: Vec<Point>,
    /// The city's rain in a year, mm, for the cistern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annual_rain_mm: Option<f64>,
    /// Septic tank and soakaway, where there is no public sewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub septic: Option<septic::SepticSettings>,
}

impl PlumbingSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
