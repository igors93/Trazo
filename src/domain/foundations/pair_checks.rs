//! Lever statics, NBR 6122 pair overturning (`FS ≥ 1.5`) and pair soil
//! pressure verification, plus the boundary strip-footing alternative (#144).
//!
//! Statics of the lever (column loads `P₁` at the boundary column and `N₂` at
//! the interior anchor column, distance `l` between them, footing centre
//! offset `e` from the boundary column along the beam):
//!
//! - reaction on the boundary footing, now centred: `R₁ = P₁ · l / (l − e)`;
//! - relief on the interior column: `ΔP = R₁ − P₁ = P₁ · e / (l − e)`, of
//!   which only half is taken off its footing (NBR 6122 recommendation);
//! - pair overturning safety factor: `FS = N₂ · (l − e) / (P₁ · e) = N₂ / ΔP ≥ 1.5`;
//! - soil pressures of the pair: `σ₁ = 1.05 · R₁ / A₁ ≤ σ_adm` on the boundary
//!   footing and `σ₂ = 1.05 · (N₂ − ΔP/2) / A₂ ≤ σ_adm` on the anchor footing;
//! - bending in the beam: `M = P₁ · e` (hogging, tension at the top), shear
//!   `V = max(ΔP, P₁)` between the boundary column and its centred footing.

use std::collections::HashMap;

use super::sizing::{choose_bars, choose_stirrups, size_balance_beam};
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::structure::ColumnResult;

/// Minimum global overturning safety factor of the boundary–anchor pair (NBR 6122).
pub const PAIR_OVERTURNING_MIN_FS: f64 = 1.5;

/// Strip-footing (*sapata corrida*) alternative along the boundary wall when a
/// balance beam pair has insufficient counterweight (`FS < 1.5`) or excessive
/// soil pressure.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryStripAlternative {
    /// Required continuous strip width along the boundary wall, metres.
    pub width_m: f64,
    /// Resulting contact pressure under the strip footing, kPa.
    pub pressure_kpa: f64,
    /// Allowable soil pressure used, kPa.
    pub soil_kpa: f64,
    /// Whether the strip width stays within practical residential limits (`≤ 1.20 m`).
    pub viable: bool,
    pub note: String,
}

/// Statics, pair soil/overturning verification and pre-sizing of one balance beam.
#[derive(Clone, Debug, PartialEq)]
pub struct BalanceBeamResult {
    pub id: u64,
    pub label: String,
    pub column: String,
    pub anchor: String,
    /// The boundary footing it centres.
    pub footing_id: Option<u64>,
    /// The interior anchor footing of the pair.
    pub anchor_footing_id: Option<u64>,
    pub span_m: f64,
    pub eccentricity_m: f64,
    pub column_load_kn: f64,
    pub reaction_kn: f64,
    pub relief_kn: f64,
    pub anchor_load_kn: f64,
    /// Global overturning safety factor of the pair (`N₂ / ΔP ≥ 1.5`, NBR 6122).
    pub pair_overturning_fs: f64,
    /// Soil pressure under the centred boundary footing (`1.05 · R₁ / A₁`), kPa.
    pub boundary_pressure_kpa: Option<f64>,
    /// Soil pressure under the interior anchor footing (`1.05 · (N₂ − ΔP/2) / A₂`), kPa.
    pub anchor_pressure_kpa: Option<f64>,
    /// Allowable soil pressure adopted, kPa.
    pub soil_kpa: f64,
    pub moment_knm: f64,
    pub shear_kn: f64,
    pub width_m: f64,
    /// Pre-sized height; `None` when even 1.20 m is not enough.
    pub height_m: Option<f64>,
    /// `μ / μ_lim` of the chosen section (`1.0` at the limit).
    pub flexure_ratio: f64,
    /// `τ_wd / τ_Rd2` of the chosen section.
    pub shear_ratio: f64,
    pub top_steel_cm2: f64,
    pub bars: String,
    pub stirrups: String,
    /// Strip-footing alternative proposed when the pair fails overturning or soil checks.
    pub strip_alternative: Option<BoundaryStripAlternative>,
}

impl BalanceBeamResult {
    /// The interior column lifts when the relief exceeds its axial load (`FS < 1.0`).
    pub fn uplift(&self) -> bool {
        self.anchor_load_kn - self.relief_kn < -1e-9
    }

    /// NBR 6122 pair overturning check (`FS = N₂ / ΔP ≥ 1.5`).
    pub fn overturning_ok(&self) -> bool {
        self.pair_overturning_fs >= PAIR_OVERTURNING_MIN_FS - 1e-6
    }

    /// Both footings of the pair stay within allowable soil pressure.
    pub fn pair_soil_ok(&self) -> bool {
        let b_ok = self
            .boundary_pressure_kpa
            .is_none_or(|p| p <= self.soil_kpa + 1e-3);
        let a_ok = self
            .anchor_pressure_kpa
            .is_none_or(|p| p <= self.soil_kpa + 1e-3);
        b_ok && a_ok
    }

    /// Peak soil utilisation ratio across both footings of the pair.
    pub fn pair_soil_ratio(&self) -> f64 {
        let p = self
            .boundary_pressure_kpa
            .unwrap_or(0.0)
            .max(self.anchor_pressure_kpa.unwrap_or(0.0));
        p / self.soil_kpa.max(1e-6)
    }
}

/// Load changes the balance beams make on the footings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BalanceEffects {
    /// Footing id → load it now takes, centred (the boundary footings).
    pub centred_load: HashMap<u64, f64>,
    /// Column id → load taken off its footing (half the relief).
    pub relief: HashMap<u64, f64>,
    pub results: Vec<BalanceBeamResult>,
}

/// Evaluates a continuous boundary strip footing (*sapata corrida*) carrying
/// `column_load_kn` distributed over a tributary boundary length `trib_m`.
fn evaluate_strip_alternative(
    column_load_kn: f64,
    trib_m: f64,
    soil_kpa: f64,
) -> BoundaryStripAlternative {
    let length = trib_m.clamp(2.0, 5.0);
    let q_kn_m = column_load_kn / length;
    let raw_w = (1.05 * q_kn_m / soil_kpa.max(10.0)).max(0.40);
    let width_m = (raw_w / 0.05).ceil() * 0.05;
    let pressure_kpa = 1.05 * q_kn_m / width_m.max(0.40);
    let viable = width_m <= 1.20 + 1e-6 && pressure_kpa <= soil_kpa + 1e-3;
    let note = if viable {
        format!(
            "Alternativa em sapata corrida na divisa: largura {:.0} cm ({:.0} kPa ≤ {:.0} kPa) elimina o levantamento do apoio interno.",
            width_m * 100.0,
            pressure_kpa,
            soil_kpa
        )
    } else {
        format!(
            "Sapata corrida pediria {:.0} cm de largura na divisa: avalie estacas/brocas com viga baldrame.",
            width_m * 100.0
        )
    };
    BoundaryStripAlternative {
        width_m,
        pressure_kpa,
        soil_kpa,
        viable,
        note,
    }
}

impl Project {
    /// Computes balance beams' lever statics, NBR 6122 pair overturning and
    /// soil bearing checks, section sizing and load transfers to the footings.
    pub fn balance_effects(
        &self,
        columns: &[ColumnResult],
        ground: &HashMap<u64, f64>,
    ) -> BalanceEffects {
        let mut effects = BalanceEffects::default();
        let fck = self.structure.settings.fck_mpa;
        let soil_kpa = self.structure.settings.soil_kpa;
        let load_of = |id: u64| columns.iter().find(|c| c.id == id);
        for (index, beam) in self.structure.balance_beams.iter().enumerate() {
            let (Some(column), Some(anchor)) = (
                self.column_by_id(beam.column),
                self.column_by_id(beam.anchor),
            ) else {
                continue;
            };
            let (Some(p1), Some(n2)) = (load_of(beam.column), load_of(beam.anchor)) else {
                continue;
            };
            // The column's load plus the ground beams reaching its footing.
            let p1_load = p1.load_kn + ground.get(&beam.column).copied().unwrap_or(0.0);
            let n2_load = n2.load_kn + ground.get(&beam.anchor).copied().unwrap_or(0.0);
            let footing = self.footing_under(column);
            let anchor_footing = self.footing_under(anchor);
            let axis = anchor.center.minus(column.center);
            let span = axis.length().max(0.1);
            let direction = axis.normalized().unwrap_or(Point::new(1.0, 0.0));
            let e = footing
                .map_or(0.0, |f| f.center.minus(column.center).dot(direction))
                .clamp(0.0, span * 0.9);
            let reaction = p1_load * span / (span - e);
            let relief = reaction - p1_load;
            let pair_overturning_fs = if relief <= 1e-6 {
                f64::INFINITY
            } else {
                n2_load / relief
            };
            let boundary_pressure_kpa =
                footing.map(|f| 1.05 * reaction / (f.width * f.depth).max(1e-6));
            let anchor_net = (n2_load - relief / 2.0).max(0.0);
            let anchor_pressure_kpa =
                anchor_footing.map(|f| 1.05 * anchor_net / (f.width * f.depth).max(1e-6));
            let moment = p1_load * e;
            let shear = relief.max(if e > 1e-6 { p1_load } else { 0.0 });
            let (height, flexure_ratio, shear_ratio, steel) =
                size_balance_beam(beam.width_m, 1.4 * moment, 1.4 * shear, fck);
            let stirrups = choose_stirrups(beam.width_m, height.unwrap_or(1.20), 1.4 * shear, fck);
            if let Some(f) = footing {
                effects.centred_load.insert(f.id, reaction);
            }
            *effects.relief.entry(beam.anchor).or_default() += relief / 2.0;

            let soil_exceeded = boundary_pressure_kpa.is_some_and(|p| p > soil_kpa + 1e-3)
                || anchor_pressure_kpa.is_some_and(|p| p > soil_kpa + 1e-3);
            let strip_alternative = (pair_overturning_fs < PAIR_OVERTURNING_MIN_FS - 1e-6
                || soil_exceeded)
                .then(|| evaluate_strip_alternative(p1_load, span.min(3.5), soil_kpa));

            effects.results.push(BalanceBeamResult {
                id: beam.id,
                label: format!("VA{}", index + 1),
                column: p1.label.clone(),
                anchor: n2.label.clone(),
                footing_id: footing.map(|f| f.id),
                anchor_footing_id: anchor_footing.map(|f| f.id),
                span_m: span,
                eccentricity_m: e,
                column_load_kn: p1_load,
                reaction_kn: reaction,
                relief_kn: relief,
                anchor_load_kn: n2_load,
                pair_overturning_fs,
                boundary_pressure_kpa,
                anchor_pressure_kpa,
                soil_kpa,
                moment_knm: moment,
                shear_kn: shear,
                width_m: beam.width_m,
                height_m: height,
                flexure_ratio,
                shear_ratio,
                top_steel_cm2: steel,
                bars: choose_bars(steel),
                stirrups,
                strip_alternative,
            });
        }
        effects
    }
}
