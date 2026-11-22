//! Preliminary reinforcement pre-sizing, NBR 6118:2023 bond/anchorage/lap
//! lengths, bar schedule (tabela de aço), CA-50 diameter summary and A3
//! schematic detailing sheet (#110, B12).

mod bond;
mod csv;
mod design;
mod sizing;
mod svg;

#[cfg(test)]
mod tests;

use crate::domain::structure::SlabKind;

pub use bond::{BondZone, bar_area_cm2, bar_mass_kg_m, lap_splice_m};
pub use csv::steel_schedule_csv;
pub use design::build_reinforcement_report;
pub use svg::reinforcement_detail_svg;

pub const STEEL_DENSITY_KG_M3: f64 = 7_850.0;
pub const STEEL_FYK_MPA: f64 = 500.0;
pub const COMMERCIAL_BAR_LENGTH_M: f64 = 12.0;
pub const STEEL_WASTE_FACTOR: f64 = 1.10;

pub(super) const LONGITUDINAL_DIAMETERS_MM: [f64; 5] = [10.0, 12.5, 16.0, 20.0, 25.0];
pub(super) const MESH_DIAMETERS_MM: [f64; 4] = [6.3, 8.0, 10.0, 12.5];
pub(super) const STIRRUP_DIAMETERS_MM: [f64; 3] = [6.3, 8.0, 10.0];
pub(super) const STANDARD_SPACINGS_CM: [f64; 6] = [20.0, 15.0, 12.5, 10.0, 7.5, 5.0];

pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

pub(super) fn diameter_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        decimal(value, 1)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarSet {
    pub count: u32,
    pub diameter_mm: f64,
    pub required_cm2: f64,
    pub provided_cm2: f64,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
    pub fits_section: bool,
}

impl BarSet {
    pub fn notation(&self) -> String {
        format!("{} Ø{}", self.count, diameter_text(self.diameter_mm))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransverseBars {
    pub diameter_mm: f64,
    pub spacing_cm: f64,
    pub count: u32,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
    pub shear_capacity_ok: bool,
}

impl TransverseBars {
    pub fn notation(&self) -> String {
        format!(
            "Ø{} c/{} cm",
            diameter_text(self.diameter_mm),
            decimal(
                self.spacing_cm,
                if self.spacing_cm.fract() == 0.0 { 0 } else { 1 }
            )
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshBars {
    pub diameter_mm: f64,
    pub spacing_cm: f64,
    pub required_cm2_m: f64,
    pub provided_cm2_m: f64,
    pub count: u32,
    pub length_each_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
}

impl MeshBars {
    pub fn notation(&self) -> String {
        format!(
            "Ø{} c/{} cm",
            diameter_text(self.diameter_mm),
            decimal(
                self.spacing_cm,
                if self.spacing_cm.fract() == 0.0 { 0 } else { 1 }
            )
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamReinforcement {
    pub beam_id: u64,
    pub label: String,
    pub span_index: usize,
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    pub bottom: BarSet,
    pub top_left: BarSet,
    pub top_right: BarSet,
    pub stirrups: TransverseBars,
    /// Required hooked anchorage length (metres) for bottom (good bond) and
    /// top (poor bond when h ≥ 30 cm) longitudinal bars (NBR 6118 §9.4.2.5).
    pub bottom_anchorage_m: f64,
    pub top_anchorage_m: f64,
    pub flexural_domain_ok: bool,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnReinforcement {
    pub column_id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    pub longitudinal: BarSet,
    pub ties: TransverseBars,
    /// Preliminary lap/starter splice length `l_0` in metres (NBR 6118 §9.5.2).
    pub lap_length_m: f64,
    pub steel_ratio: f64,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlabReinforcement {
    pub slab_id: u64,
    pub label: String,
    pub room: String,
    pub kind: SlabKind,
    pub main: MeshBars,
    pub distribution: MeshBars,
    pub domain_ok: bool,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootingReinforcement {
    pub footing_id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    pub bars_x: MeshBars,
    pub bars_y: MeshBars,
    pub mass_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SteelScheduleRow {
    pub mark: String,
    pub element: String,
    pub position: String,
    pub shape: String,
    pub diameter_mm: f64,
    pub quantity: u32,
    pub unit_length_m: f64,
    pub total_length_m: f64,
    pub mass_kg: f64,
}

/// Consolidated bar-schedule summary grouped by commercial CA-50 diameter.
#[derive(Clone, Debug, PartialEq)]
pub struct SteelDiameterSummary {
    pub diameter_mm: f64,
    pub total_length_m: f64,
    pub bars_12m: u32,
    pub mass_kg: f64,
    pub mass_with_waste_kg: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReinforcementReport {
    pub beams: Vec<BeamReinforcement>,
    pub columns: Vec<ColumnReinforcement>,
    pub slabs: Vec<SlabReinforcement>,
    pub footings: Vec<FootingReinforcement>,
    pub schedule: Vec<SteelScheduleRow>,
    pub by_diameter: Vec<SteelDiameterSummary>,
    pub warnings: Vec<String>,
}

impl ReinforcementReport {
    pub fn total_mass_kg(&self) -> f64 {
        self.schedule.iter().map(|row| row.mass_kg).sum()
    }

    pub fn total_mass_with_waste_kg(&self) -> f64 {
        self.total_mass_kg() * STEEL_WASTE_FACTOR
    }

    pub fn total_bars_12m(&self) -> u32 {
        self.by_diameter.iter().map(|row| row.bars_12m).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.beams.is_empty()
            && self.columns.is_empty()
            && self.slabs.is_empty()
            && self.footings.is_empty()
    }
}
