//! Boundary foundations (#144): columns and footings on a lot line (B23),
//! balanced by a balance beam (*viga alavanca*, *viga de equilíbrio*) to an
//! interior footing, or by a boundary strip footing (*sapata corrida*), with
//! NBR 6122 pair overturning and soil bearing checks.
//!
//! Organised into focused submodules:
//! - [`sizing`]: rectangular ULS beam sizing (flexure, shear, bars, stirrups).
//! - [`pair_checks`]: lever statics, pair overturning safety factor (`FS ≥ 1.5`),
//!   soil pressures on both footings of the pair, and boundary strip-footing
//!   alternative when the pair fails.
//! - [`marks`]: formwork and foundation plan marks.

mod marks;
pub mod pair_checks;
pub mod sizing;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use super::fixtures::{Fixture, FixtureKind};
use super::model::Project;

pub use pair_checks::{BalanceBeamResult, PAIR_OVERTURNING_MIN_FS};
pub(super) use sizing::choose_bars;
pub use sizing::size_rectangular_beam;

/// A balance beam from a boundary column to an interior anchor column.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BalanceBeam {
    pub id: u64,
    /// The boundary column, whose footing stands off centre.
    pub column: u64,
    /// The interior column that holds the beam down.
    pub anchor: u64,
    /// Section width, metres.
    pub width_m: f64,
}

impl Project {
    pub(crate) fn footing_under(&self, column: &Fixture) -> Option<&Fixture> {
        self.fixtures
            .iter()
            .find(|f| f.kind == FixtureKind::Footing && f.contains(column.center))
    }

    pub(crate) fn column_by_id(&self, id: u64) -> Option<&Fixture> {
        self.fixtures
            .iter()
            .find(|f| f.id == id && f.kind == FixtureKind::Column)
    }

    /// Joins a boundary column to an interior one with a balance beam.
    pub fn add_balance_beam(&mut self, column: u64, anchor: u64) -> Result<u64, String> {
        if column == anchor {
            return Err("Escolha dois pilares diferentes.".to_owned());
        }
        let boundary = self
            .column_by_id(column)
            .copied()
            .ok_or(format!("O id {column} não é um pilar."))?;
        if self.column_by_id(anchor).is_none() {
            return Err(format!("O id {anchor} não é um pilar."));
        }
        if self.footing_under(&boundary).is_none() {
            return Err("O pilar de divisa precisa de uma sapata embaixo dele.".to_owned());
        }
        if self
            .structure
            .balance_beams
            .iter()
            .any(|b| b.column == column)
        {
            return Err("Esse pilar já tem viga alavanca.".to_owned());
        }
        let id = self.id();
        self.structure.balance_beams.push(BalanceBeam {
            id,
            column,
            anchor,
            width_m: boundary.width.min(boundary.depth).max(0.20),
        });
        Ok(id)
    }

    pub fn delete_balance_beam(&mut self, id: u64) -> bool {
        let before = self.structure.balance_beams.len();
        self.structure.balance_beams.retain(|b| b.id != id);
        self.structure.balance_beams.len() < before
    }

    /// Lays a balance beam from every boundary column whose footing stands
    /// more than 5 cm off centre to the nearest column inward (within 30° of
    /// the footing's offset, 1.5 to 10 m away). Returns how many it added
    /// and how many boundary columns found no partner.
    pub fn generate_balance_beams(&mut self) -> (usize, usize) {
        let columns: Vec<Fixture> = self.structural_columns().copied().collect();
        let mut added = 0;
        let mut alone = 0;
        for column in &columns {
            if self
                .structure
                .balance_beams
                .iter()
                .any(|b| b.column == column.id)
            {
                continue;
            }
            let Some(footing) = self.footing_under(column) else {
                continue;
            };
            let offset = footing.center.minus(column.center);
            if offset.length() <= 0.05 {
                continue;
            }
            let Some(inward) = offset.normalized() else {
                continue;
            };
            let partner = columns
                .iter()
                .filter(|other| other.id != column.id)
                .filter_map(|other| {
                    let to = other.center.minus(column.center);
                    let distance = to.length();
                    let direction = to.normalized()?;
                    (direction.dot(inward) >= 30.0_f64.to_radians().cos()
                        && (1.5..=10.0).contains(&distance))
                    .then_some((distance, other.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match partner {
                Some((_, anchor)) => {
                    if self.add_balance_beam(column.id, anchor).is_ok() {
                        added += 1;
                    }
                }
                None => alone += 1,
            }
        }
        (added, alone)
    }
}
