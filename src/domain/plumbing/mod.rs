//! Water, sewage and rain (roadmap section E, milestone M6): the plumbing
//! points of every fixture (#72), and the settings the plumbing design
//! keeps with the project.
//!
//! The tables come from the plumbing building code pack (NBR 5626, NBR 8160
//! and NBR 10844, `data/codes/plumbing/nbr.json`); the few the pack does not
//! carry are Trazo's, and say so where they are used.

pub mod points;
pub mod rules;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

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
}

impl PlumbingSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
