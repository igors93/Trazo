//! Electrical installation design (ABNT NBR 5410): devices, room load
//! forecast, circuit sizing, automatic placement, plan symbols and load schedule.

mod forecast;
pub mod lighting;
pub mod photovoltaic;
mod placement;
pub mod shopping_list;
mod sizing;
mod symbols;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    model::Project,
    utility_profiles::UtilityOverrides,
};

pub use forecast::{RoomClass, outlet_power_va};
pub use sizing::{BREAKERS, WIRE_CAPACITY, WIRE_CAPACITY_3_LOADED};
pub(super) use symbols::symbol;
pub use symbols::tag_size;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectricalData {
    /// Circuit number in the load schedule, once assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit: Option<u32>,
    /// Apparent power, in volt-amperes.
    #[serde(default)]
    pub power_va: f64,
    /// Mounting height above the floor, in metres (0 for ceiling points).
    #[serde(default)]
    pub height_m: f64,
    /// Optional luminaire luminous flux in lumens (#125, overrides project default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lumens: Option<f64>,
}

/// Project-wide electrical settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectricalSettings {
    /// Phase-to-neutral voltage of general circuits: 127 or 220 V.
    #[serde(default = "default_voltage")]
    pub voltage_v: f64,
    /// Voltage of specific-use circuits (showers, ovens): usually 220 V.
    #[serde(default = "default_specific_voltage")]
    pub specific_voltage_v: f64,
    /// How the utility supplies the building (automatic from the demand).
    #[serde(default)]
    pub supply: crate::domain::distribution::Supply,
    /// Regional utility profile (demand factors & entrance limits).
    #[serde(default)]
    pub utility: crate::domain::distribution::UtilityProfile,
    /// Optional custom overrides for utility tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_rules: Option<UtilityOverrides>,
    /// Room lighting calculation parameters (NBR ISO/CIE 8995-1, #125).
    #[serde(default, skip_serializing_if = "lighting::LightingSettings::is_default")]
    pub lighting: lighting::LightingSettings,
    /// Photovoltaic pre-sizing parameters (NBR 16690 / Lei 14.300, #126).
    #[serde(default, skip_serializing_if = "photovoltaic::PvSettings::is_default")]
    pub pv: photovoltaic::PvSettings,
}

fn default_voltage() -> f64 {
    127.0
}

fn default_specific_voltage() -> f64 {
    220.0
}

impl Default for ElectricalSettings {
    fn default() -> Self {
        Self {
            voltage_v: default_voltage(),
            specific_voltage_v: default_specific_voltage(),
            supply: crate::domain::distribution::Supply::default(),
            utility: crate::domain::distribution::UtilityProfile::default(),
            custom_rules: None,
            lighting: lighting::LightingSettings::default(),
            pv: photovoltaic::PvSettings::default(),
        }
    }
}

impl ElectricalSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// What a device does in the installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceRole {
    /// A light point.
    Lighting,
    /// A general-use outlet (TUG).
    GeneralOutlet,
    /// A specific-use outlet (TUE): shower, oven, air conditioner.
    SpecificOutlet,
    /// A switch: commands lights, draws no power.
    Switch,
    /// The distribution board.
    Panel,
    /// Data, phone or TV: low voltage, not in the load schedule.
    Data,
}

impl FixtureKind {
    pub const ELECTRICAL: [FixtureKind; 11] = [
        FixtureKind::OutletLow,
        FixtureKind::OutletMid,
        FixtureKind::OutletHigh,
        FixtureKind::OutletSpecific,
        FixtureKind::Switch,
        FixtureKind::SwitchTwoWay,
        FixtureKind::SwitchIntermediate,
        FixtureKind::CeilingLight,
        FixtureKind::WallLight,
        FixtureKind::Panel,
        FixtureKind::DataPoint,
    ];

    pub fn is_electrical(self) -> bool {
        self.role().is_some()
    }

    pub fn role(self) -> Option<DeviceRole> {
        Some(match self {
            Self::OutletLow | Self::OutletMid | Self::OutletHigh => DeviceRole::GeneralOutlet,
            Self::OutletSpecific => DeviceRole::SpecificOutlet,
            Self::Switch | Self::SwitchTwoWay | Self::SwitchIntermediate => DeviceRole::Switch,
            Self::CeilingLight | Self::WallLight => DeviceRole::Lighting,
            Self::Panel => DeviceRole::Panel,
            Self::DataPoint => DeviceRole::Data,
            _ => return None,
        })
    }

    /// Whether the device goes on a wall (everything but ceiling points and
    /// structural elements, which stand on the wall's centre line).
    pub fn mounts_on_wall(self) -> bool {
        !self.is_structural()
            && !matches!(
                self,
                Self::CeilingLight
                    | Self::Table
                    | Self::DiningSet
                    | Self::Car
                    | Self::Umbrella
                    | Self::SunLounger
            )
    }

    /// The usual power and height of a new device of this kind.
    pub fn default_electrical(self) -> Option<ElectricalData> {
        let (power_va, height_m) = match self {
            Self::OutletLow => (100.0, 0.30),
            Self::OutletMid => (100.0, 1.20),
            Self::OutletHigh => (100.0, 2.20),
            Self::OutletSpecific => (1500.0, 0.30),
            Self::Switch | Self::SwitchTwoWay | Self::SwitchIntermediate => (0.0, 1.10),
            Self::CeilingLight => (100.0, 0.0),
            Self::WallLight => (60.0, 2.00),
            Self::Panel => (0.0, 1.50),
            Self::DataPoint => (0.0, 0.30),
            _ => return None,
        };
        Some(ElectricalData {
            circuit: None,
            power_va,
            height_m,
            lumens: None,
        })
    }
}

/// The plan symbol of an electrical device, in the fixture's frame: the back
/// (against the wall) at y = -depth/2. Shapes follow common Brazilian

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitKind {
    Lighting,
    GeneralOutlets,
    SpecificOutlet,
    /// Lights and outlets on one circuit, which NBR 5410 asks to separate.
    Mixed,
}

impl CircuitKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::GeneralOutlets => "Tomadas (TUG)",
            Self::SpecificOutlet => "Uso específico (TUE)",
            Self::Mixed => "Misto",
        }
    }

    /// The abbreviation used on the board and on sheets.
    pub fn short(self) -> &'static str {
        match self {
            Self::Lighting => "Ilum.",
            Self::GeneralOutlets => "TUG",
            Self::SpecificOutlet => "TUE",
            Self::Mixed => "Misto",
        }
    }
}

/// One row of the load schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitRow {
    pub number: u32,
    pub kind: CircuitKind,
    pub rooms: Vec<String>,
    pub points: usize,
    pub power_va: f64,
    pub voltage_v: f64,
    pub current_a: f64,
    /// Smallest section that carries the breaker's current, in mm².
    pub wire_mm2: Option<f64>,
    pub breaker_a: Option<f64>,
    /// Whether NBR 5410 5.1.3.2.2 asks for a 30 mA RCD (wet or outdoor areas).
    pub needs_rcd: bool,
    /// Breaker poles: 2 for 220 V between the phases of a 127 V network.
    pub poles: u8,
    /// The phases it is connected to (0 = A, 1 = B, 2 = C).
    pub phases: Vec<u8>,
    /// Cable length to the farthest point, once there is a board.
    pub length_m: Option<f64>,
    /// Voltage drop at the farthest point, in percent.
    pub drop_percent: Option<f64>,
    /// Whether the wire was raised above the breaker's to limit the drop.
    pub wire_by_drop: bool,
    /// Serving board label when the project has sub-boards (`QDG`, `QDC1`, …).
    pub board: Option<String>,
}

/// The circuits, their totals and what needs attention.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadSchedule {
    pub circuits: Vec<CircuitRow>,
    pub total_va: f64,
    pub unassigned: usize,
    pub warnings: Vec<String>,
    /// Installed loads with the demand factors applied.
    pub demand: crate::domain::distribution::Demand,
    /// The supply in use (resolved when automatic).
    pub supply: crate::domain::distribution::Supply,
    /// Installed load on each phase after balancing, in VA.
    pub phase_va: [f64; 3],
    /// Main breaker, feeder and conduit of the utility connection.
    pub entrance: Option<crate::domain::distribution::ServiceEntrance>,
    /// Secondary distribution boards (QDC) fed from the main board.
    pub sub_boards: Vec<crate::domain::distribution::SubBoardSummary>,
}

/// Breaker and wire for a circuit drawing `current_a`: the smallest standard
/// breaker at or above the current, then the smallest section that carries

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationReport {
    pub lights: usize,
    pub outlets: usize,
    pub switches: usize,
    pub specific: usize,
}

impl GenerationReport {
    pub fn total(&self) -> usize {
        self.lights + self.outlets + self.switches + self.specific
    }
}

impl Project {
    /// The fixtures that are electrical devices.
    pub fn electrical_devices(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind.is_electrical())
    }

    fn set_device_power(&mut self, id: u64, power_va: f64) {
        if let Some(device) = self.fixtures.iter_mut().find(|fixture| fixture.id == id)
            && let Some(data) = device.electrical.as_mut()
        {
            data.power_va = power_va;
        }
    }

    /// Changes a device's circuit, power or height; `None` keeps the value.
    pub fn update_electrical(
        &mut self,
        id: u64,
        circuit: Option<Option<u32>>,
        power_va: Option<f64>,
        height_m: Option<f64>,
    ) -> Result<(), String> {
        let device = self
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.id == id && fixture.kind.is_electrical())
            .ok_or("Ponto elétrico não encontrado.")?;
        let data = device.electrical.get_or_insert_with(|| {
            device.kind.default_electrical().unwrap_or(ElectricalData {
                circuit: None,
                power_va: 0.0,
                height_m: 0.0,
                lumens: None,
            })
        });
        if let Some(power) = power_va {
            if !power.is_finite() || !(0.0..=50_000.0).contains(&power) {
                return Err("A potência deve ficar entre 0 e 50000 VA.".to_owned());
            }
            data.power_va = power;
        }
        if let Some(height) = height_m {
            if !height.is_finite() || !(0.0..=6.0).contains(&height) {
                return Err("A altura deve ficar entre 0 e 6 m.".to_owned());
            }
            data.height_m = height;
        }
        if let Some(circuit) = circuit {
            if circuit == Some(0) {
                return Err("Os circuitos são numerados a partir de 1.".to_owned());
            }
            data.circuit = circuit;
        }
        Ok(())
    }

    /// Sets or clears a lighting fixture's luminous flux override, in lumens (#125).
    pub fn set_device_lumens(&mut self, id: u64, lumens: Option<f64>) -> Result<(), String> {
        let device = self
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.id == id && fixture.kind.role() == Some(DeviceRole::Lighting))
            .ok_or("Ponto de iluminação não encontrado.")?;
        if let Some(lm) = lumens
            && (!lm.is_finite() || !(100.0..=50_000.0).contains(&lm))
        {
            return Err("O fluxo luminoso deve ficar entre 100 e 50000 lm.".to_owned());
        }
        let data = device.electrical.get_or_insert_with(|| {
            device.kind.default_electrical().unwrap_or(ElectricalData {
                circuit: None,
                power_va: 100.0,
                height_m: 0.0,
                lumens: None,
            })
        });
        data.lumens = lumens;
        Ok(())
    }
}
