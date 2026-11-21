//! Boundary foundations (#144): balance beams (*vigas alavanca*, *vigas de
//! equilíbrio*) between a boundary column, whose footing has to stay inside
//! the lot and so stands off centre, and an interior column that holds the
//! beam down.
//!
//! Statics of the lever (column loads P₁ at the boundary column and N₂ at the
//! interior one, distance l between them, footing centre e from the boundary
//! column along the beam):
//!
//! - reaction on the boundary footing, now centred: R₁ = P₁ · l / (l − e);
//! - relief on the interior column: ΔP = R₁ − P₁ = P₁ · e / (l − e), of which
//!   only half is taken off its footing (the usual NBR 6122 recommendation);
//! - the interior support must not lift: N₂ − ΔP ≥ 0;
//! - bending in the beam: M = P₁ · e (hogging, tension at the top), shear
//!   V = ΔP from the footing to the interior column.
//!
//! The beam section is pre-sized for Md = 1.4 M and Vd = 1.4 V (CA-50,
//! x/d ≤ 0.45, τwd ≤ τRd2), with the top steel and its bars.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::conduits::PlanMark;
use super::fixtures::{Fixture, FixtureKind};
use super::geometry::Point;
use super::model::Project;
use super::reinforcement::bar_area_cm2;
use super::structure::ColumnResult;

/// A balance beam from a boundary column to an interior one.
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

/// Statics and pre-sizing of one balance beam.
#[derive(Clone, Debug, PartialEq)]
pub struct BalanceBeamResult {
    pub id: u64,
    pub label: String,
    pub column: String,
    pub anchor: String,
    /// The boundary footing it centres.
    pub footing_id: Option<u64>,
    pub span_m: f64,
    pub eccentricity_m: f64,
    pub column_load_kn: f64,
    pub reaction_kn: f64,
    pub relief_kn: f64,
    pub anchor_load_kn: f64,
    pub moment_knm: f64,
    pub shear_kn: f64,
    pub width_m: f64,
    /// Pre-sized height; `None` when even 1.20 m is not enough.
    pub height_m: Option<f64>,
    /// μ / μlim of the chosen section (1.0 at the limit).
    pub flexure_ratio: f64,
    /// τwd / τRd2 of the chosen section.
    pub shear_ratio: f64,
    pub top_steel_cm2: f64,
    pub bars: String,
}

impl BalanceBeamResult {
    /// The interior column lifts when the relief exceeds its load.
    pub fn uplift(&self) -> bool {
        self.anchor_load_kn - self.relief_kn < -1e-9
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

/// Steel limit of the section: x/d ≤ 0.45 with the rectangular block.
const MU_LIMIT: f64 = 0.295;

/// The smallest height (in 5 cm steps, 40 to 120 cm) that carries `md`
/// (kN·m) and `vd` (kN) with width `b`, and the top steel it needs.
pub fn size_balance_beam(b: f64, md: f64, vd: f64, fck_mpa: f64) -> (Option<f64>, f64, f64, f64) {
    let fcd = fck_mpa.max(10.0) / 1.4 * 1000.0; // kN/m²
    let fyd = 500.0 / 1.15 * 1000.0;
    let tau_rd2 = 0.27 * (1.0 - fck_mpa / 250.0) * fcd;
    let mut last = (0.0, 0.0, 0.0);
    for step in 8..=24 {
        let h = f64::from(step) * 0.05;
        let d = h - 0.05;
        let mu = md / (0.85 * fcd * b * d * d);
        let tau = vd / (b * d);
        let xi = ((1.0 - (1.0 - 2.0 * mu.min(MU_LIMIT)).max(0.0).sqrt()) / 0.8).min(0.45);
        let z = d * (1.0 - 0.4 * xi);
        let steel = (md / (z * fyd) * 10_000.0).max(0.0015 * b * h * 10_000.0);
        last = (mu / MU_LIMIT, tau / tau_rd2, steel);
        if mu <= MU_LIMIT && tau <= tau_rd2 {
            return (Some(h), last.0, last.1, last.2);
        }
    }
    (None, last.0, last.1, last.2)
}

/// Top bars for `steel_cm2` (2 to 6 bars of 10 to 20 mm).
fn choose_bars(steel_cm2: f64) -> String {
    for diameter in [10.0, 12.5, 16.0, 20.0] {
        let count = ((steel_cm2 / bar_area_cm2(diameter)).ceil() as u32).max(2);
        if count <= 6 {
            let d = if diameter.fract() == 0.0 {
                format!("{diameter:.0}")
            } else {
                format!("{diameter:.1}").replace('.', ",")
            };
            return format!("{count} Ø{d} (superior)");
        }
    }
    "mais de 6 Ø20: aumente a seção".to_owned()
}

impl Project {
    fn footing_under(&self, column: &Fixture) -> Option<&Fixture> {
        self.fixtures
            .iter()
            .find(|f| f.kind == FixtureKind::Footing && f.contains(column.center))
    }

    fn column_by_id(&self, id: u64) -> Option<&Fixture> {
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

    /// The balance beams' statics, sizing and effect on the footings, from
    /// the column loads of the pre-sizing.
    pub fn balance_effects(&self, columns: &[ColumnResult]) -> BalanceEffects {
        let mut effects = BalanceEffects::default();
        let fck = self.structure.settings.fck_mpa;
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
            let footing = self.footing_under(column);
            let axis = anchor.center.minus(column.center);
            let span = axis.length().max(0.1);
            let direction = axis.normalized().unwrap_or(Point::new(1.0, 0.0));
            let e = footing
                .map_or(0.0, |f| f.center.minus(column.center).dot(direction))
                .clamp(0.0, span * 0.9);
            let reaction = p1.load_kn * span / (span - e);
            let relief = reaction - p1.load_kn;
            let moment = p1.load_kn * e;
            let shear = relief.max(if e > 1e-6 { p1.load_kn } else { 0.0 });
            let (height, flexure_ratio, shear_ratio, steel) =
                size_balance_beam(beam.width_m, 1.4 * moment, 1.4 * shear, fck);
            if let Some(f) = footing {
                effects.centred_load.insert(f.id, reaction);
            }
            *effects.relief.entry(beam.anchor).or_default() += relief / 2.0;
            effects.results.push(BalanceBeamResult {
                id: beam.id,
                label: format!("VA{}", index + 1),
                column: p1.label.clone(),
                anchor: n2.label.clone(),
                footing_id: footing.map(|f| f.id),
                span_m: span,
                eccentricity_m: e,
                column_load_kn: p1.load_kn,
                reaction_kn: reaction,
                relief_kn: relief,
                anchor_load_kn: n2.load_kn,
                moment_knm: moment,
                shear_kn: shear,
                width_m: beam.width_m,
                height_m: height,
                flexure_ratio,
                shear_ratio,
                top_steel_cm2: steel,
                bars: choose_bars(steel),
            });
        }
        effects
    }

    /// Balance beams as drawn on the formwork plan: the two faces from the
    /// boundary footing to the interior column, dashed (below ground), and
    /// the label.
    pub fn balance_beam_marks(&self, results: &[BalanceBeamResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(beam) = self
                .structure
                .balance_beams
                .iter()
                .find(|b| b.id == result.id)
            else {
                continue;
            };
            let (Some(column), Some(anchor)) = (
                self.column_by_id(beam.column),
                self.column_by_id(beam.anchor),
            ) else {
                continue;
            };
            let Some(direction) = anchor.center.minus(column.center).normalized() else {
                continue;
            };
            let normal = Point::new(-direction.y, direction.x);
            for side in [-1.0, 1.0] {
                let off = normal.scaled(side * beam.width_m / 2.0);
                marks.push(PlanMark::Line {
                    points: vec![column.center.translated(off), anchor.center.translated(off)],
                    dashed: true,
                    heavy: true,
                });
            }
            let text = match result.height_m {
                Some(h) => format!(
                    "{} {:.0}×{:.0}",
                    result.label,
                    beam.width_m * 100.0,
                    h * 100.0
                ),
                None => format!("{} (seção insuficiente)", result.label),
            };
            marks.push(PlanMark::Text {
                anchor: column
                    .center
                    .lerp(anchor.center, 0.5)
                    .translated(normal.scaled(beam.width_m / 2.0 + 0.18)),
                text,
            });
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lever_follows_the_textbook_statics() {
        // P1 = 100 kN, l = 4 m, e = 0.30 m: R1 = 100·4/3.7 = 108.1 kN,
        // ΔP = 8.1 kN, M = 30 kN·m.
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let a = project
            .add_fixture(FixtureKind::Column, Point::new(0.10, 5.0), 0.20, 0.20, 0)
            .unwrap();
        let b = project
            .add_fixture(FixtureKind::Column, Point::new(4.10, 5.0), 0.20, 0.20, 0)
            .unwrap();
        project
            .add_fixture(FixtureKind::Footing, Point::new(0.40, 5.0), 0.80, 0.80, 0)
            .unwrap();
        project.add_balance_beam(a, b).unwrap();
        let col = |id: u64, label: &str, load: f64| ColumnResult {
            id,
            label: label.to_owned(),
            width_m: 0.2,
            depth_m: 0.2,
            position: super::super::structure::ColumnPosition::Edge,
            load_kn: load,
            upper_load_kn: 0.0,
            area_needed_cm2: 0.0,
            area_cm2: 400.0,
        };
        let effects = project.balance_effects(&[col(a, "P1", 100.0), col(b, "P2", 150.0)]);
        let r = &effects.results[0];
        assert!((r.eccentricity_m - 0.30).abs() < 1e-9);
        assert!((r.reaction_kn - 100.0 * 4.0 / 3.7).abs() < 1e-9);
        assert!((r.relief_kn - 100.0 * 0.3 / 3.7).abs() < 1e-9);
        assert!((r.moment_knm - 30.0).abs() < 1e-9);
        assert!(!r.uplift());
        assert!(r.height_m.is_some());
        assert!((effects.relief[&b] - r.relief_kn / 2.0).abs() < 1e-9);
        // A light interior column is lifted.
        let lifted = project.balance_effects(&[col(a, "P1", 100.0), col(b, "P2", 5.0)]);
        assert!(lifted.results[0].uplift());
    }

    #[test]
    fn boundary_footings_are_centred_by_balance_beams_to_interior_columns() {
        // A 4 × 4 m room against the left lot line, as in the gourmet area.
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(0.0, 2.0), Point::new(4.0, 6.0), "Gourmet")
            .unwrap();
        project.generate_structure();
        let before = project.structure_report();
        let eccentric: Vec<_> = before
            .footings
            .iter()
            .filter(|f| f.eccentricity_m > 0.05)
            .collect();
        assert_eq!(eccentric.len(), 2, "the two boundary footings");

        let (added, alone) = project.generate_balance_beams();
        assert_eq!((added, alone), (2, 0));
        assert_eq!(project.generate_balance_beams(), (0, 0), "already there");
        let after = project.structure_report();
        assert_eq!(after.balance_beams.len(), 2);
        for beam in &after.balance_beams {
            assert!(!beam.uplift());
            assert!(beam.height_m.is_some());
            let footing = after
                .footings
                .iter()
                .find(|f| Some(f.id) == beam.footing_id)
                .unwrap();
            // Centred under R1: no overturning, the reaction as its load.
            assert!(footing.overturning_fs.is_infinite());
            assert!((footing.load_kn - beam.reaction_kn).abs() < 1e-9);
            assert!(beam.reaction_kn > beam.column_load_kn);
        }
        // The interior footings carry less (half the relief).
        let interior_before: f64 = before
            .footings
            .iter()
            .filter(|f| f.eccentricity_m <= 0.05)
            .map(|f| f.load_kn)
            .sum();
        let interior_after: f64 = after
            .footings
            .iter()
            .filter(|f| {
                !after
                    .balance_beams
                    .iter()
                    .any(|b| b.footing_id == Some(f.id))
            })
            .map(|f| f.load_kn)
            .sum();
        let relief: f64 = after.balance_beams.iter().map(|b| b.relief_kn / 2.0).sum();
        assert!((interior_before - interior_after - relief).abs() < 1e-6);
        // They are drawn, kept across floors and go with their columns.
        let marks = project.structure_marks();
        assert!(
            marks
                .iter()
                .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with("VA1")))
        );
        let ground = project.ensure_default_floor();
        project.add_floor("Superior", true, false);
        assert!(project.structure.balance_beams.is_empty());
        project.switch_floor(ground).unwrap();
        assert_eq!(project.structure.balance_beams.len(), 2);
        let column = project.structure.balance_beams[0].column;
        project.delete_fixture(column);
        assert_eq!(project.structure.balance_beams.len(), 1);
    }

    #[test]
    fn the_section_grows_with_the_moment() {
        let (small, ..) = size_balance_beam(0.20, 1.4 * 20.0, 1.4 * 10.0, 25.0);
        let (large, ..) = size_balance_beam(0.20, 1.4 * 120.0, 1.4 * 40.0, 25.0);
        assert!(small.unwrap() < large.unwrap());
        let (none, flexure, ..) = size_balance_beam(0.20, 1.4 * 2000.0, 1.4 * 50.0, 25.0);
        assert!(none.is_none() && flexure > 1.0);
    }
}
