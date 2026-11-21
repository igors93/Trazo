//! Wall foundations (#143): every wall needs something carrying it to the
//! soil. Where the wall runs between columns it gets a ground beam
//! (*viga baldrame*) resting on their footings; where it does not (an
//! interior wall such as a bathroom partition) it gets a strip footing
//! (*sapata corrida*).
//!
//! - Wall load: q = γ · t · (net wall area) / L, kN/m (openings discounted),
//!   plus the foundation's own weight.
//! - Ground beam: a continuous beam over the columns on the wall's line,
//!   solved like the floor beams; its reactions are added to the footings
//!   under those columns. Sized for Md = 1.4 M, Vd = 1.4 V (30 to 80 cm).
//! - Strip footing: width B = 1.05 · q / σadm, at least 40 cm, in 5 cm steps.
//!
//! Only the ground floor has wall foundations; walls whose openings take
//! 90 % or more of their length (an open side) are left out.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::conduits::PlanMark;
use super::foundations::{choose_bars, size_rectangular_beam};
use super::geometry::Point;
use super::model::Project;
use super::structure::{CONCRETE_KN_M3, continuous_beam_with_point_loads};

/// How a wall reaches the soil.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallFoundationKind {
    /// A ground beam on the footings of the wall's columns.
    #[default]
    GroundBeam,
    /// A continuous strip footing under the wall.
    StripFooting,
}

impl WallFoundationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::GroundBeam => "Viga baldrame",
            Self::StripFooting => "Sapata corrida",
        }
    }
}

/// The foundation chosen for one wall.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallFoundation {
    pub id: u64,
    pub wall_id: u64,
    pub kind: WallFoundationKind,
    /// Beam width or strip width, metres; `None` sizes it automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_m: Option<f64>,
}

/// One wall foundation, analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct WallFoundationResult {
    pub id: u64,
    pub wall_id: u64,
    pub label: String,
    pub kind: WallFoundationKind,
    pub length_m: f64,
    /// Wall plus foundation load, kN/m.
    pub load_kn_m: f64,
    /// Ground beam: largest moment (kN·m) and shear (kN).
    pub moment_knm: f64,
    pub shear_kn: f64,
    pub width_m: f64,
    /// Ground beam height (None when 80 cm is not enough).
    pub height_m: Option<f64>,
    pub flexure_ratio: f64,
    pub shear_ratio: f64,
    pub steel_cm2: f64,
    pub bars: String,
    /// Strip footing: width needed and soil pressure under the width used.
    pub needed_width_m: f64,
    pub soil_kpa: f64,
    pub pressure_kpa: f64,
    /// Ground beam: column ids it rests on, with the reaction on each (kN).
    pub reactions: Vec<(u64, f64)>,
}

/// What the wall foundations add to the footings, and their results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallFoundationEffects {
    /// Column id → ground beam reactions arriving at its footing, kN.
    pub at_column: HashMap<u64, f64>,
    pub results: Vec<WallFoundationResult>,
}

fn round_up(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

impl Project {
    /// Whether the active floor is the ground floor (the only one with wall
    /// foundations).
    fn on_ground_floor(&self) -> bool {
        let Some(active) = self.active_floor() else {
            return true;
        };
        !self
            .floors
            .levels
            .iter()
            .any(|level| level.elevation_m < active.elevation_m - 1e-4)
    }

    /// Columns standing on a wall's line, with their position along it.
    fn columns_on_wall(&self, wall: &super::model::Wall) -> Vec<(f64, u64, Point)> {
        let length = wall.length();
        let mut on: Vec<(f64, u64, Point)> = self
            .structural_columns()
            .filter(|c| {
                let off = 0.06 + c.width.max(c.depth) / 2.0;
                super::geometry::distance_to_segment(c.center, wall.start, wall.end) < off
            })
            .map(|c| {
                let (point, _) =
                    super::geometry::nearest_point_on_segment(c.center, wall.start, wall.end);
                (point.distance(wall.start), c.id, point)
            })
            .filter(|(s, _, _)| *s <= length + 0.06)
            .collect();
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        on.dedup_by(|a, b| (a.0 - b.0).abs() < 0.05);
        on
    }

    fn wall_is_open(&self, wall: &super::model::Wall) -> bool {
        let open: f64 = self
            .openings
            .iter()
            .filter(|o| o.wall_id == wall.id)
            .map(|o| o.width())
            .sum();
        open >= 0.9 * wall.length()
    }

    /// Gives every ground-floor wall without a foundation one: a ground
    /// beam when there is a column within 30 cm of each end, a strip footing
    /// otherwise. Open sides are left out. Returns (ground beams, strips).
    pub fn generate_wall_foundations(&mut self) -> (usize, usize) {
        if !self.on_ground_floor() {
            return (0, 0);
        }
        let mut beams = 0;
        let mut strips = 0;
        let walls = self.walls.clone();
        for wall in &walls {
            if self
                .structure
                .wall_foundations
                .iter()
                .any(|f| f.wall_id == wall.id)
                || self.wall_is_open(wall)
            {
                continue;
            }
            let length = wall.length();
            let on = self.columns_on_wall(wall);
            let supported = on.len() >= 2
                && on.first().is_some_and(|(s, _, _)| *s <= 0.30)
                && on.last().is_some_and(|(s, _, _)| *s >= length - 0.30);
            let kind = if supported {
                beams += 1;
                WallFoundationKind::GroundBeam
            } else {
                strips += 1;
                WallFoundationKind::StripFooting
            };
            let id = self.id();
            self.structure.wall_foundations.push(WallFoundation {
                id,
                wall_id: wall.id,
                kind,
                width_m: None,
            });
        }
        (beams, strips)
    }

    pub fn set_wall_foundation(
        &mut self,
        wall_id: u64,
        kind: WallFoundationKind,
        width_m: Option<f64>,
    ) -> Result<u64, String> {
        if !self.walls.iter().any(|w| w.id == wall_id) {
            return Err("Parede não encontrada.".to_owned());
        }
        if let Some(w) = width_m
            && (!w.is_finite() || !(0.10..=3.0).contains(&w))
        {
            return Err("A largura deve ficar entre 10 cm e 3 m.".to_owned());
        }
        if let Some(f) = self
            .structure
            .wall_foundations
            .iter_mut()
            .find(|f| f.wall_id == wall_id)
        {
            f.kind = kind;
            f.width_m = width_m;
            return Ok(f.id);
        }
        let id = self.id();
        self.structure.wall_foundations.push(WallFoundation {
            id,
            wall_id,
            kind,
            width_m,
        });
        Ok(id)
    }

    pub fn remove_wall_foundations(&mut self) -> usize {
        let count = self.structure.wall_foundations.len();
        self.structure.wall_foundations.clear();
        count
    }

    /// The wall foundations analysed, and the loads their ground beams add
    /// to the footings.
    pub fn wall_foundation_effects(&self) -> WallFoundationEffects {
        let mut effects = WallFoundationEffects::default();
        if !self.on_ground_floor() {
            return effects;
        }
        let s = self.structure.settings;
        let height = self.wall_height.max(0.5);
        let (mut beam_no, mut strip_no) = (0, 0);
        let mut ordered: Vec<&WallFoundation> = self.structure.wall_foundations.iter().collect();
        ordered.sort_by(|a, b| {
            let mid = |f: &WallFoundation| {
                self.walls
                    .iter()
                    .find(|w| w.id == f.wall_id)
                    .map_or(Point::new(0.0, 0.0), |w| w.start.midpoint(w.end))
            };
            let (ma, mb) = (mid(a), mid(b));
            ((ma.y / 0.3).round() as i64)
                .cmp(&((mb.y / 0.3).round() as i64))
                .then(ma.x.total_cmp(&mb.x))
        });
        for foundation in ordered {
            let Some(wall) = self.walls.iter().find(|w| w.id == foundation.wall_id) else {
                continue;
            };
            let length = wall.length().max(0.1);
            let openings: f64 = self
                .openings
                .iter()
                .filter(|o| o.wall_id == wall.id)
                .map(|o| o.width() * o.height().unwrap_or(height).min(height))
                .sum();
            let net = (length * height - openings).max(0.0);
            let wall_q = s.masonry_kn_m3 * wall.thickness() * net / length;
            let mut result = WallFoundationResult {
                id: foundation.id,
                wall_id: wall.id,
                label: String::new(),
                kind: foundation.kind,
                length_m: length,
                load_kn_m: wall_q,
                moment_knm: 0.0,
                shear_kn: 0.0,
                width_m: 0.0,
                height_m: None,
                flexure_ratio: 0.0,
                shear_ratio: 0.0,
                steel_cm2: 0.0,
                bars: String::new(),
                needed_width_m: 0.0,
                soil_kpa: s.soil_kpa,
                pressure_kpa: 0.0,
                reactions: Vec::new(),
            };
            let on = self.columns_on_wall(wall);
            match foundation.kind {
                WallFoundationKind::GroundBeam if on.len() >= 2 => {
                    beam_no += 1;
                    result.label = format!("VB{beam_no}");
                    let b = foundation
                        .width_m
                        .unwrap_or_else(|| round_up(wall.thickness().max(0.15), 0.05));
                    // Own weight with a 40 cm first guess.
                    let q = wall_q + CONCRETE_KN_M3 * b * 0.40;
                    let spans: Vec<(f64, f64, f64)> = on
                        .windows(2)
                        .map(|p| ((p[1].0 - p[0].0).max(0.1), q, 1.0))
                        .collect();
                    let none = vec![Vec::new(); spans.len()];
                    let ends = continuous_beam_with_point_loads(&spans, &none);
                    let mut reactions = vec![0.0; on.len()];
                    let (mut moment, mut shear) = (0.0_f64, 0.0_f64);
                    for (i, ((l, w, _), (left, right))) in spans.iter().zip(&ends).enumerate() {
                        let v_left = w * l / 2.0 + (right - left) / l;
                        let v_right = v_left - w * l;
                        reactions[i] += v_left;
                        reactions[i + 1] -= v_right;
                        let x = (v_left / w.max(1e-9)).clamp(0.0, *l);
                        let span = left + v_left * x - w * x * x / 2.0;
                        moment = moment.max(span.abs()).max(left.abs()).max(right.abs());
                        shear = shear.max(v_left.abs()).max(v_right.abs());
                    }
                    // Wall beyond the end columns hangs on them as a cantilever.
                    let before = on[0].0;
                    let after = (length - on[on.len() - 1].0).max(0.0);
                    reactions[0] += q * before;
                    let last = reactions.len() - 1;
                    reactions[last] += q * after;
                    moment = moment
                        .max(q * before * before / 2.0)
                        .max(q * after * after / 2.0);
                    let (h, flexure, shear_ratio, steel) =
                        size_rectangular_beam(b, 1.4 * moment, 1.4 * shear, s.fck_mpa, 0.30, 0.80);
                    result.load_kn_m = q;
                    result.moment_knm = moment;
                    result.shear_kn = shear;
                    result.width_m = b;
                    result.height_m = h;
                    result.flexure_ratio = flexure;
                    result.shear_ratio = shear_ratio;
                    result.steel_cm2 = steel;
                    result.bars = choose_bars(steel).replace("(superior)", "(em cima e embaixo)");
                    for ((_, column, _), reaction) in on.iter().zip(&reactions) {
                        *effects.at_column.entry(*column).or_default() += reaction.max(0.0);
                        result.reactions.push((*column, *reaction));
                    }
                }
                _ => {
                    strip_no += 1;
                    result.label = format!("SC{strip_no}");
                    result.kind = WallFoundationKind::StripFooting;
                    // Own weight of a 25 cm-thick strip, then B = 1.05 q / σ.
                    let mut width = 0.40;
                    for _ in 0..5 {
                        let q = wall_q + CONCRETE_KN_M3 * width * 0.25;
                        width = round_up(1.05 * q / s.soil_kpa.max(10.0), 0.05).max(0.40);
                    }
                    let used = foundation.width_m.unwrap_or(width);
                    let q = wall_q + CONCRETE_KN_M3 * used * 0.25;
                    result.load_kn_m = q;
                    result.needed_width_m = width;
                    result.width_m = used;
                    result.pressure_kpa = 1.05 * q / used.max(0.01);
                }
            }
            effects.results.push(result);
        }
        effects
    }

    /// Ground beams (dashed, below ground) and strip footings (dashed
    /// outline) on the formwork plan, with their labels.
    pub fn wall_foundation_marks(&self, results: &[WallFoundationResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(wall) = self.walls.iter().find(|w| w.id == result.wall_id) else {
                continue;
            };
            let Some(direction) = wall.end.minus(wall.start).normalized() else {
                continue;
            };
            let normal = Point::new(-direction.y, direction.x);
            let half = result.width_m / 2.0;
            for side in [-1.0, 1.0] {
                let off = normal.scaled(side * half);
                marks.push(PlanMark::Line {
                    points: vec![wall.start.translated(off), wall.end.translated(off)],
                    dashed: true,
                    heavy: result.kind == WallFoundationKind::GroundBeam,
                });
            }
            let text = match (result.kind, result.height_m) {
                (WallFoundationKind::GroundBeam, Some(h)) => format!(
                    "{} {:.0}×{:.0}",
                    result.label,
                    result.width_m * 100.0,
                    h * 100.0
                ),
                (WallFoundationKind::GroundBeam, None) => {
                    format!("{} (seção insuficiente)", result.label)
                }
                (WallFoundationKind::StripFooting, _) => {
                    format!("{} B={:.0}", result.label, result.width_m * 100.0)
                }
            };
            marks.push(PlanMark::Text {
                anchor: wall
                    .start
                    .midpoint(wall.end)
                    .translated(normal.scaled(-(half + 0.22))),
                text,
            });
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Banheiro")
            .unwrap();
        project
    }

    #[test]
    fn walls_between_columns_get_ground_beams_and_the_others_strip_footings() {
        let mut project = house();
        // Columns only on the outer corners: the shared wall at x = 4 has
        // none at its ends.
        for (x, y) in [(0.0, 0.0), (7.0, 0.0), (7.0, 4.0), (0.0, 4.0)] {
            project
                .add_fixture(FixtureKind::Column, Point::new(x, y), 0.20, 0.20, 0)
                .unwrap();
        }
        let (beams, strips) = project.generate_wall_foundations();
        // The 4 m left and 4 m right walls have columns at both ends; the
        // split top and bottom walls (0–4 and 4–7) have a column at one end
        // only, like the shared wall: strips.
        assert_eq!(
            (beams, strips),
            (2, 5),
            "{:?}",
            project.structure.wall_foundations
        );
        assert_eq!(project.generate_wall_foundations(), (0, 0), "nothing new");
        let effects = project.wall_foundation_effects();
        for strip in effects
            .results
            .iter()
            .filter(|r| r.kind == WallFoundationKind::StripFooting)
        {
            assert!(strip.width_m >= 0.40);
            assert!(strip.pressure_kpa <= strip.soil_kpa + 1e-6, "{strip:?}");
        }
        for beam in effects
            .results
            .iter()
            .filter(|r| r.kind == WallFoundationKind::GroundBeam)
        {
            assert!(beam.height_m.is_some());
            // A single span: reactions qL/2 each.
            let total: f64 = beam.reactions.iter().map(|(_, r)| r).sum();
            assert!((total - beam.load_kn_m * beam.length_m).abs() < 1e-6);
        }
    }

    #[test]
    fn ground_beam_reactions_reach_the_footings() {
        let mut project = house();
        project.generate_structure();
        let before: f64 = project
            .structure_report()
            .footings
            .iter()
            .map(|f| f.load_kn)
            .sum();
        let (beams, _) = project.generate_wall_foundations();
        assert!(beams > 0);
        let effects = project.wall_foundation_effects();
        let added: f64 = effects.at_column.values().sum();
        let report = project.structure_report();
        let after: f64 = report.footings.iter().map(|f| f.load_kn).sum();
        assert!(
            (after - before - added).abs() < 1e-6,
            "{after} − {before} vs {added}"
        );
        assert_eq!(report.wall_foundations.len(), effects.results.len());
        // The wall load: 13 kN/m³ × wall thickness × 2.8 m (no openings).
        let beam = report
            .wall_foundations
            .iter()
            .find(|r| r.kind == WallFoundationKind::GroundBeam)
            .unwrap();
        let wall = project.walls.iter().find(|w| w.id == beam.wall_id).unwrap();
        let wall_q = 13.0 * wall.thickness() * 2.8;
        assert!((beam.load_kn_m - wall_q - 25.0 * beam.width_m * 0.40).abs() < 1e-9);
        // Drawn on the formwork plan.
        let marks = project.structure_marks();
        assert!(
            marks
                .iter()
                .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with("VB1")))
        );
    }
}
