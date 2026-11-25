//! Structural system: columns and footings (fixtures), beams between columns
//! and slabs over rooms, plus pre-sizing by the usual Brazilian engineering
//! rules (NBR 6118, NBR 6120, NBR 6122) and continuous-beam analysis.

mod beam_solver;
mod footing_check;
mod layout;
mod marks;
mod model;
mod output;
mod report;
mod results;
mod slab_geometry;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;

use crate::domain::geometry::Point;

/// Unit weight of reinforced concrete, kN/m³ (NBR 6120).
pub const CONCRETE_KN_M3: f64 = 25.0;

#[cfg(test)]
pub use beam_solver::{continuous_beam, span_deflection};
pub use beam_solver::{
    continuous_beam_with_end_moments, continuous_beam_with_point_loads, effective_beam_stiffness,
    span_deflection_with_point_loads,
};
pub use footing_check::check_footing;
pub use marks::StructureMarksScope;
pub(super) use marks::symbol;
pub use model::{
    Beam, BeamSupport, OccupancyClass, ProjectStructure, Slab, SlabKind, SoilClass,
    StructureSettings, room_live_load_kn_m2,
};
pub use output::{beam_diagrams_svg, build_columns_schedule};
pub use results::{
    BeamLine, BeamLineResult, ColumnPosition, ColumnResult, ConcentratedLoadResult, FootingResult,
    MisalignedColumn, PileCapResult, SlabResult, SpanPointLoad, SpanResult, StructureGeneration,
    StructureReport, beam_moment_at, beam_shear_at,
};
pub use slab_geometry::{decompose_slab_panels, yield_line_shares};

pub(crate) fn round_up(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

/// "14×30" from metres.
pub fn section_cm(b: f64, h: f64) -> String {
    format!("{:.0}×{:.0}", b * 100.0, h * 100.0)
}

/// Reading order on the plan: top to bottom, then left to right.
pub(crate) fn reading(a: Point, b: Point) -> Ordering {
    let row = |p: Point| (p.y / 0.3).round() as i64;
    row(a).cmp(&row(b)).then(a.x.total_cmp(&b.x))
}

pub(crate) fn validate_section(width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite() || !(0.08..=1.0).contains(&width) {
        return Err("A largura da viga deve ficar entre 8 e 100 cm.".to_owned());
    }
    if !height.is_finite() || !(0.15..=2.0).contains(&height) {
        return Err("A altura da viga deve ficar entre 15 e 200 cm.".to_owned());
    }
    Ok(())
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// "V2" before "V10".
pub(crate) fn natural(a: &str, b: &str) -> Ordering {
    let split = |s: &str| {
        let digits: String = s.chars().filter(char::is_ascii_digit).collect();
        let letters: String = s.chars().filter(|c| !c.is_ascii_digit()).collect();
        (letters, digits.parse::<u64>().unwrap_or(0))
    };
    split(a).cmp(&split(b))
}
