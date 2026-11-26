//! Electrical distribution board (NBR 5410): phase balancing, voltage drop,
//! demand factors, service entrance, sub-boards and single-line diagram.

mod calculations;
mod schedule_svg;
pub mod selectivity;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use crate::domain::utility_profiles::UtilityProfile;
pub use calculations::earth_section;
pub use schedule_svg::{build_panel_schedule, single_line_svg};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Supply {
    /// Chosen from the demand.
    #[default]
    Auto,
    /// One phase and neutral.
    Single,
    /// Two phases and neutral.
    Two,
    /// Three phases and neutral.
    Three,
}

impl Supply {
    pub const ALL: [Supply; 4] = [Supply::Auto, Supply::Single, Supply::Two, Supply::Three];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automática",
            Self::Single => "Monofásica",
            Self::Two => "Bifásica",
            Self::Three => "Trifásica",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Single => "monofasica",
            Self::Two => "bifasica",
            Self::Three => "trifasica",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" | "automatica" | "automatico" => Some(Self::Auto),
            "monofasica" | "monofasico" | "mono" => Some(Self::Single),
            "bifasica" | "bifasico" | "bi" => Some(Self::Two),
            "trifasica" | "trifasico" | "tri" => Some(Self::Three),
            _ => None,
        }
    }

    /// Number of phases; 0 while automatic.
    pub fn phases(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Single => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

pub const PHASE_NAMES: [&str; 3] = ["A", "B", "C"];

/// Copper resistivity used for voltage drop, in Ω·mm²/m (1/56, the usual
/// value in Brazilian practice).
pub const COPPER_RESISTIVITY: f64 = 1.0 / 56.0;

/// NBR 5410 6.2.7.1: at most 4 % drop in terminal circuits.
pub const MAX_DROP_PERCENT: f64 = 4.0;

/// The likely maximum load, with the demand factors applied.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    /// Lighting and general-use outlets, installed.
    pub general_va: f64,
    pub general_factor: f64,
    /// Specific-use circuits, installed.
    pub specific_va: f64,
    pub specific_count: usize,
    pub specific_factor: f64,
    pub demand_va: f64,
}

/// The utility connection and what it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServiceEntrance {
    pub supply: Supply,
    /// Demand current on the most loaded phase.
    pub current_a: f64,
    pub breaker_a: Option<f64>,
    pub wire_mm2: Option<f64>,
    pub earth_mm2: Option<f64>,
    pub conduit_mm: Option<u32>,
}

/// NBR 5410: keep sub-board feeder voltage drop at or below 2 % so feeder plus
/// terminal circuits remain within the overall drop budget.
pub const MAX_FEEDER_DROP_PERCENT: f64 = 2.0;

/// Summary and feeder sizing for a secondary distribution board (QDC).
#[derive(Clone, Debug, PartialEq)]
pub struct SubBoardSummary {
    pub panel_id: u64,
    pub label: String,
    pub room: Option<String>,
    pub circuits: Vec<u32>,
    pub installed_va: f64,
    pub demand_va: f64,
    pub current_a: f64,
    pub feeder_length_m: Option<f64>,
    pub feeder_drop_percent: Option<f64>,
    pub breaker_a: Option<f64>,
    pub wire_mm2: Option<f64>,
    pub earth_mm2: Option<f64>,
    pub conduit_mm: Option<u32>,
}

pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// "2,5" for wires, "10" for the larger ones.
pub fn section_label(section: f64) -> String {
    if section.fract().abs() > 1e-9 {
        decimal(section, 1)
    } else {
        format!("{section:.0}")
    }
}

pub(super) fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// "A", "B+C"...
pub fn phase_label(phases: &[u8]) -> String {
    phases
        .iter()
        .map(|p| PHASE_NAMES[usize::from(*p).min(2)])
        .collect::<Vec<_>>()
        .join("+")
}
