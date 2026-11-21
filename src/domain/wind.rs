//! Wind (#147): the pressure of NBR 6123 on the tall walls and on the
//! roofs, for open-sided buildings such as a gourmet area.
//!
//! - **Pressure:** q = 0.613 · Vk² (N/m²), Vk = V0 · S1 · S2 · S3, with
//!   S2 = b · Fr · (z / 10)^p for the terrain category and the building
//!   class (A below 20 m, B from 20 to 50 m) (NBR 6123, 5.3).
//! - **Tall walls:** a column standing in a wall taller than the storey
//!   takes the wind on its share of the wall, with a net coefficient of
//!   1.3 (walls of open-sided buildings and free-standing walls). Where a
//!   roof beam bears on it the roof props its top (the roof then needs
//!   bracing down to the gable walls), M = w H² / 8; otherwise it stands as
//!   a cantilever from its footing, M = w H² / 2. The column is checked in
//!   bending across the wall (Md = 1.4 M, x/d ≤ 0.45).
//! - **Roofs:** suction Cpe = −1.0 over the roof (conservative for one
//!   slope) and internal pressure Cpi = +0.8 when a side is open (a
//!   dominant opening facing the wind, NBR 6123, 6.2.5), 0 otherwise. The
//!   net uplift 1.4 · (1.0 + Cpi) · q − 0.9 · g, when positive, is the
//!   anchorage each roof beam end needs.

use serde::{Deserialize, Serialize};

use super::foundations::size_rectangular_beam;
use super::geometry::{Point, distance_to_segment};
use super::model::Project;
use super::structure::StructureReport;

fn default_v0() -> f64 {
    35.0
}
fn default_category() -> u8 {
    3
}
fn one() -> f64 {
    1.0
}

/// The site's wind (NBR 6123).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindSettings {
    /// Basic speed from the isopleth map, m/s (30 to 50 in Brazil).
    #[serde(default = "default_v0")]
    pub v0_ms: f64,
    /// Terrain category, I (open sea) to V (dense city centre).
    #[serde(default = "default_category")]
    pub category: u8,
    /// Topography and statistical factors.
    #[serde(default = "one")]
    pub s1: f64,
    #[serde(default = "one")]
    pub s3: f64,
}

impl Default for WindSettings {
    fn default() -> Self {
        Self {
            v0_ms: default_v0(),
            category: default_category(),
            s1: 1.0,
            s3: 1.0,
        }
    }
}

impl WindSettings {
    pub fn check(&self) -> Result<(), String> {
        if !(25.0..=55.0).contains(&self.v0_ms) {
            return Err(
                "V0 deve ficar entre 25 e 55 m/s (mapa de isopletas da NBR 6123).".to_owned(),
            );
        }
        if !(1..=5).contains(&self.category) {
            return Err("A categoria do terreno vai de I a V (1 a 5).".to_owned());
        }
        if !(0.5..=1.5).contains(&self.s1) || !(0.8..=1.2).contains(&self.s3) {
            return Err("S1 vai de 0,5 a 1,5 e S3 de 0,8 a 1,2.".to_owned());
        }
        Ok(())
    }

    /// S2 at height `z` for a building whose largest dimension is `size`
    /// (NBR 6123, table 1: b, Fr and p by category and class).
    pub fn s2(&self, z: f64, size: f64) -> f64 {
        let class_b = size >= 20.0;
        let (b, p) = match (self.category, class_b) {
            (1, false) => (1.10, 0.06),
            (1, true) => (1.10, 0.065),
            (2, false) => (1.00, 0.085),
            (2, true) => (1.00, 0.09),
            (4, false) => (0.86, 0.12),
            (4, true) => (0.86, 0.125),
            (5, false) => (0.74, 0.15),
            (5, true) => (0.74, 0.16),
            (_, false) => (0.94, 0.10),
            (_, true) => (0.94, 0.105),
        };
        let fr = if class_b { 0.98 } else { 1.0 };
        b * fr * (z.max(1.0) / 10.0).powf(p)
    }

    /// Dynamic pressure at height `z`, kPa.
    pub fn pressure_kpa(&self, z: f64, size: f64) -> f64 {
        let vk = self.v0_ms * self.s1 * self.s2(z, size) * self.s3;
        0.613 * vk * vk / 1000.0
    }
}

/// Net pressure coefficient on a wall of an open-sided building.
pub const WALL_CP: f64 = 1.3;
/// Suction on a one-slope roof (conservative).
pub const ROOF_CPE: f64 = -1.0;
/// Internal pressure with a dominant opening facing the wind.
pub const OPEN_CPI: f64 = 0.8;

/// A column of a tall wall under the wind.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnWindResult {
    pub id: u64,
    pub label: String,
    pub height_m: f64,
    pub tributary_m: f64,
    pub q_kpa: f64,
    /// Propped at the top by a roof (else a cantilever).
    pub braced: bool,
    /// Characteristic moment at the critical section, kN·m.
    pub moment_knm: f64,
    /// Section across and along the wall, metres.
    pub depth_m: f64,
    pub width_m: f64,
    /// Bending utilisation (μ over 0.295).
    pub ratio: f64,
    /// Steel on each face, cm².
    pub steel_cm2: f64,
    /// Smallest depth across the wall that passes, metres.
    pub needed_depth_m: Option<f64>,
}

/// A roof under the wind's suction.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofWindResult {
    pub id: u64,
    pub label: String,
    pub q_kpa: f64,
    pub open: bool,
    /// Characteristic uplift and the permanent load, kN/m².
    pub uplift_kpa: f64,
    pub dead_kpa: f64,
    /// 1.4 · uplift − 0.9 · dead, kN/m² (positive: the roof lifts).
    pub net_design_kpa: f64,
    /// Anchorage each beam end needs, design kN (largest).
    pub anchor_kn: f64,
}

impl RoofWindResult {
    pub fn lifts(&self) -> bool {
        self.net_design_kpa > 1e-9
    }
}

/// The wind on the structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindReport {
    pub columns: Vec<ColumnWindResult>,
    pub roofs: Vec<RoofWindResult>,
}

/// μ, steel per face (cm²) of a section `b` × `h` under `md` (kN·m).
fn bending(b: f64, h: f64, md: f64, fck: f64) -> (f64, f64) {
    let fcd = fck.max(10.0) / 1.4 * 1000.0;
    let fyd = 500.0 / 1.15 * 1000.0;
    let d = (h - 0.04).max(0.05);
    let mu = md / (0.85 * fcd * b * d * d);
    let xi = ((1.0 - (1.0 - 2.0 * mu.min(0.295)).max(0.0).sqrt()) / 0.8).min(0.45);
    let z = d * (1.0 - 0.4 * xi);
    (mu / 0.295, md / (z * fyd) * 10_000.0)
}

impl Project {
    /// Sets the site's wind.
    pub fn set_wind(&mut self, wind: WindSettings) -> Result<(), String> {
        wind.check()?;
        self.structure.wind = wind;
        Ok(())
    }

    /// Largest plan dimension of the building, metres.
    fn building_size(&self) -> f64 {
        let mut points = self.walls.iter().flat_map(|w| [w.start, w.end]);
        let Some(first) = points.next() else {
            return 10.0;
        };
        let (lo, hi) = points.fold((first, first), |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
        (hi.x - lo.x).max(hi.y - lo.y)
    }

    /// The wind on the columns of tall walls and on the roofs.
    pub fn wind_effects(&self, report: &StructureReport) -> WindReport {
        let wind = self.structure.wind;
        let size = self.building_size();
        let storey = self.wall_height;
        let fck = self.structure.settings.fck_mpa;
        let mut out = WindReport::default();
        for result in &report.columns {
            let Some(column) = self.structural_columns().find(|c| c.id == result.id) else {
                continue;
            };
            let height = self.column_height(column);
            if height <= storey + 0.2 {
                continue;
            }
            let reach = column.width.max(column.depth) / 2.0 + 0.06;
            // The tallest wall it stands in.
            let Some(wall) = self
                .walls
                .iter()
                .filter(|w| distance_to_segment(column.center, w.start, w.end) < reach)
                .max_by(|a, b| {
                    a.height_at(storey, column.center)
                        .total_cmp(&b.height_at(storey, column.center))
                })
            else {
                continue;
            };
            let Some(u) = wall.end.minus(wall.start).normalized() else {
                continue;
            };
            let n = Point::new(-u.y, u.x);
            let (s, c) = column.rotation_rad().sin_cos();
            let (cu, cv) = (Point::new(c, s), Point::new(-s, c));
            let across = n.dot(cu).abs() * column.width + n.dot(cv).abs() * column.depth;
            let along = u.dot(cu).abs() * column.width + u.dot(cv).abs() * column.depth;
            // Its share of the wall, between the neighbours on the wall.
            let length = wall.length();
            let at = column.center.minus(wall.start).dot(u).clamp(0.0, length);
            let mut stations: Vec<f64> = self
                .structural_columns()
                .filter(|o| {
                    distance_to_segment(o.center, wall.start, wall.end)
                        < o.width.max(o.depth) / 2.0 + 0.06
                })
                .map(|o| o.center.minus(wall.start).dot(u).clamp(0.0, length))
                .collect();
            stations.sort_by(f64::total_cmp);
            let before = stations
                .iter()
                .copied()
                .filter(|s| *s < at - 1e-6)
                .fold(None, |_, s| Some(s));
            let after = stations.iter().copied().find(|s| *s > at + 1e-6);
            let left = before.map_or(0.0, |b| (at + b) / 2.0);
            let right = after.map_or(length, |a| (at + a) / 2.0);
            let tributary = (right - left).max(0.1);
            let q = wind.pressure_kpa(height, size);
            let w = WALL_CP * q * tributary;
            let braced = report.roofs.iter().any(|r| {
                r.reactions
                    .iter()
                    .any(|x| x.support.as_deref() == Some(result.label.as_str()))
            });
            let moment = if braced {
                w * height * height / 8.0
            } else {
                w * height * height / 2.0
            };
            let shear = if braced { 5.0 / 8.0 } else { 1.0 } * w * height;
            let md = 1.4 * moment;
            let (ratio, steel) = bending(along, across, md, fck);
            let needed = size_rectangular_beam(along.max(0.12), md, 1.4 * shear, fck, 0.15, 0.80).0;
            out.columns.push(ColumnWindResult {
                id: column.id,
                label: result.label.clone(),
                height_m: height,
                tributary_m: tributary,
                q_kpa: q,
                braced,
                moment_knm: moment,
                depth_m: across,
                width_m: along,
                ratio,
                steel_cm2: steel,
                needed_depth_m: needed,
            });
        }
        for result in &report.roofs {
            let Some(roof) = self.structure.roofs.iter().find(|r| r.id == result.id) else {
                continue;
            };
            let open = result.reactions.iter().any(|r| {
                r.support
                    .as_deref()
                    .is_none_or(|s| s.starts_with('M') || s.starts_with("VM"))
            });
            let q = wind.pressure_kpa(roof.high_m, size.max(roof.eaves_m()));
            let cpi = if open { OPEN_CPI } else { 0.0 };
            let uplift = (-ROOF_CPE + cpi) * q;
            let dead = result.dead_kn_m2;
            let net = 1.4 * uplift - 0.9 * dead;
            let per_area = result.dead_kn_m2 + result.live_kn_m2;
            let anchor = if net > 0.0 {
                result
                    .reactions
                    .iter()
                    .map(|r| net * r.load_kn / per_area.max(1e-9))
                    .fold(0.0, f64::max)
            } else {
                0.0
            };
            out.roofs.push(RoofWindResult {
                id: result.id,
                label: result.label.clone(),
                q_kpa: q,
                open,
                uplift_kpa: uplift,
                dead_kpa: dead,
                net_design_kpa: net,
                anchor_kn: anchor,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::roof::{Roof, RoofSide};

    #[test]
    fn the_pressure_follows_nbr_6123() {
        let wind = WindSettings::default();
        // Category III, class A, z = 10 m: S2 = 0.94; Vk = 32.9 m/s.
        assert!((wind.s2(10.0, 15.0) - 0.94).abs() < 1e-9);
        let q = wind.pressure_kpa(10.0, 15.0);
        assert!((q - 0.613 * (35.0f64 * 0.94).powi(2) / 1000.0).abs() < 1e-12);
        // Lower down, less wind; larger buildings (class B), a bit less too.
        assert!(wind.pressure_kpa(4.3, 15.0) < q);
        assert!(wind.s2(10.0, 30.0) < wind.s2(10.0, 15.0));
        let mut bad = wind;
        bad.v0_ms = 70.0;
        assert!(bad.check().is_err());
    }

    fn gourmet(with_roof: bool) -> Project {
        let mut project = Project::default();
        project.wall_height = 2.8;
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(0.0, 14.75))
            .unwrap();
        project.set_wall_height(wall, Some(4.3), None).unwrap();
        for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
            project
                .add_fixture(FixtureKind::Column, Point::new(0.07, y), 0.30, 0.14, 1)
                .unwrap();
        }
        for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
            project
                .add_fixture(FixtureKind::TimberPost, Point::new(4.93, y), 0.15, 0.15, 0)
                .unwrap();
        }
        if with_roof {
            project
                .add_roof(Roof::new(
                    Point::new(0.0, 0.0),
                    Point::new(5.0, 14.75),
                    RoofSide::West,
                    4.30,
                    2.60,
                ))
                .unwrap();
        }
        project
    }

    #[test]
    fn a_tall_wall_column_is_propped_by_the_roof_or_stands_as_a_cantilever() {
        let free = gourmet(false);
        let report = free.structure_report();
        let free_wind = free.wind_effects(&report);
        assert_eq!(free_wind.columns.len(), 6);
        let middle = free_wind
            .columns
            .iter()
            .find(|c| (c.tributary_m - 3.0).abs() < 1e-6)
            .unwrap();
        assert!(!middle.braced);
        // 14 cm across the wall cannot hold a 4.3 m cantilever.
        assert!((middle.depth_m - 0.14).abs() < 1e-6, "{}", middle.depth_m);
        assert!(middle.ratio > 1.0, "{middle:?}");
        let roofed = gourmet(true);
        let report = roofed.structure_report();
        let wind = roofed.wind_effects(&report);
        let propped = wind
            .columns
            .iter()
            .find(|c| (c.tributary_m - 3.0).abs() < 1e-6)
            .unwrap();
        assert!(propped.braced);
        assert!((propped.moment_knm * 4.0 - middle.moment_knm).abs() < 1e-9);
        // The open side lets the wind lift the roof.
        let roof = &wind.roofs[0];
        assert!(roof.open);
        assert!((roof.uplift_kpa - 1.8 * roof.q_kpa).abs() < 1e-12);
    }
}
