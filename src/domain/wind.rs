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

use std::collections::HashMap;

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
    /// (NBR 6123, Table 1 & Table 2: b, Fr and p by category I–V and class A/B/C,
    /// with constant S2 for z ≤ 5 m).
    pub fn s2(&self, z: f64, size: f64) -> f64 {
        let class = if size > 50.0 {
            2u8 // Class C
        } else if size > 20.0 {
            1u8 // Class B
        } else {
            0u8 // Class A
        };
        let (b, p) = match (self.category, class) {
            (1, 0) => (1.10, 0.06),
            (1, 1) => (1.11, 0.065),
            (1, _) => (1.12, 0.07),
            (2, 0) => (1.00, 0.085),
            (2, 1) => (1.00, 0.09),
            (2, _) => (1.00, 0.10),
            (4, 0) => (0.86, 0.12),
            (4, 1) => (0.85, 0.125),
            (4, _) => (0.84, 0.135),
            (5, 0) => (0.74, 0.15),
            (5, 1) => (0.73, 0.16),
            (5, _) => (0.71, 0.175),
            (_, 0) => (0.94, 0.10),
            (_, 1) => (0.94, 0.105),
            (_, _) => (0.93, 0.115),
        };
        let fr = match class {
            0 => 1.00,
            1 => 0.98,
            _ => 0.95,
        };
        // NBR 6123 Table 2: S2 is constant below z = 5 m.
        b * fr * (z.max(5.0) / 10.0).powf(p)
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
    pub bracing: Vec<RoofBracingResult>,
    pub global: Vec<GlobalStabilityResult>,
}

/// Design shear that must reach the ground through the roof bracing path.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofBracingResult {
    pub roof_id: u64,
    pub label: String,
    pub demand_kn: f64,
    pub capacity_kn: Option<f64>,
    pub path_ok: bool,
}

impl RoofBracingResult {
    pub fn ok(&self) -> bool {
        self.path_ok
            && self
                .capacity_kn
                .is_some_and(|capacity| capacity + 1e-9 >= self.demand_kn)
    }
}

/// Conservative whole-structure screening in one horizontal direction.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalStabilityResult {
    pub direction: &'static str,
    pub height_m: f64,
    pub facade_m: f64,
    pub wind_kn: f64,
    pub overturning_knm: f64,
    pub gravity_kn: f64,
    pub roof_uplift_kn: f64,
    pub stabilizing_knm: f64,
    pub overturning_ratio: f64,
    pub alpha: f64,
    pub alpha_limit: f64,
    pub drift_mm: f64,
    pub drift_limit_mm: f64,
    pub supports_ok: bool,
    pub bracing_ok: bool,
}

impl GlobalStabilityResult {
    pub fn ok(&self) -> bool {
        self.supports_ok
            && self.bracing_ok
            && self.overturning_ratio >= 1.0 - 1e-9
            && self.alpha <= self.alpha_limit + 1e-9
            && self.drift_mm <= self.drift_limit_mm + 1e-9
    }
}

/// μ, steel per face (cm²) of a section `b` × `h` under `md` (kN·m).
fn bending(b: f64, h: f64, md: f64, fck: f64) -> (f64, f64) {
    let fcd = fck.max(10.0) / 1.4 * 1000.0;
    let fyd = 500.0 / 1.15 * 1000.0;
    // Match `size_rectangular_beam` effective depth d = h - 0.05 m.
    let d = (h - 0.05).max(0.05);
    let mu = md / (0.85 * fcd * b * d * d);
    let xi = ((1.0 - (1.0 - 2.0 * mu.min(0.295)).max(0.0).sqrt()) / 0.8).min(0.45);
    let z = d * (1.0 - 0.4 * xi);
    (mu / 0.295, md / (z * fyd) * 10_000.0)
}

impl Project {
    fn global_wind_stability(
        &self,
        report: &StructureReport,
        wind: &WindReport,
    ) -> Vec<GlobalStabilityResult> {
        if wind.columns.is_empty() && report.roofs.is_empty() {
            return Vec::new();
        }
        let mut points: Vec<Point> = self
            .walls
            .iter()
            .flat_map(|wall| [wall.start, wall.end])
            .collect();
        for roof in &self.structure.roofs {
            points.extend([roof.min, roof.max]);
        }
        let Some(first) = points.first().copied() else {
            return Vec::new();
        };
        let (lo, hi) = points.into_iter().fold((first, first), |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
        let height = self
            .walls
            .iter()
            .map(|wall| {
                let (a, b) = wall.heights(self.wall_height);
                a.max(b)
            })
            .chain(self.structure.roofs.iter().map(|roof| roof.high_m))
            .fold(self.wall_height, f64::max);
        let mut supports = Vec::new();
        let mut gravity = Vec::new();
        let mut uplift = Vec::new();
        let mut supports_ok = !report.footings.is_empty()
            || !report.timber_posts.is_empty()
            || report.wall_foundations.iter().any(|f| {
                f.kind == super::wall_foundations::WallFoundationKind::StripFooting
                    && !f.unsupported
            });
        for footing in &report.footings {
            if let Some(fixture) = self.fixtures.iter().find(|f| f.id == footing.id) {
                supports.push((fixture.center, footing.width_m / 2.0, footing.depth_m / 2.0));
                gravity.push((
                    fixture.center,
                    25.0 * footing.width_m * footing.depth_m * footing.height_m,
                ));
            }
        }
        for post in &report.timber_posts {
            if let Some(fixture) = self.fixtures.iter().find(|f| f.id == post.id) {
                supports.push((fixture.center, post.base_m / 2.0, post.base_m / 2.0));
                gravity.push((
                    fixture.center,
                    24.0 * post.base_m * post.base_m * post.base_depth_m,
                ));
            }
        }
        for strip in report.wall_foundations.iter().filter(|f| {
            f.kind == super::wall_foundations::WallFoundationKind::StripFooting && !f.unsupported
        }) {
            if let Some(wall) = self.walls.iter().find(|w| w.id == strip.wall_id) {
                supports.push((wall.start, strip.width_m / 2.0, strip.width_m / 2.0));
                supports.push((wall.end, strip.width_m / 2.0, strip.width_m / 2.0));
                gravity.push((
                    wall.start.midpoint(wall.end),
                    25.0 * strip.width_m * 0.25 * strip.length_m,
                ));
            }
        }
        for wall in &self.walls {
            let founded = report
                .wall_foundations
                .iter()
                .any(|f| f.wall_id == wall.id && !f.unsupported)
                || [wall.start, wall.end].iter().all(|end| {
                    report.footings.iter().any(|f| {
                        self.fixtures
                            .iter()
                            .find(|fix| fix.id == f.id)
                            .is_some_and(|fix| fix.center.distance(*end) < 0.35)
                    })
                });
            supports_ok &= founded;
            if founded {
                gravity.push((
                    wall.start.midpoint(wall.end),
                    self.structure.settings.masonry_kn_m3
                        * wall.thickness()
                        * self.wall_net_area(wall),
                ));
            }
        }
        for column in &report.columns {
            let Some(fixture) = self.structural_columns().find(|f| f.id == column.id) else {
                continue;
            };
            supports_ok &= report
                .footings
                .iter()
                .any(|f| f.column.as_deref() == Some(column.label.as_str()));
            gravity.push((
                fixture.center,
                25.0 * fixture.width * fixture.depth * column.height_m,
            ));
        }
        for roof in &report.roofs {
            if let Some(model) = self.structure.roofs.iter().find(|r| r.id == roof.id) {
                supports_ok &= roof.unsupported() == 0;
                let area = (model.max.x - model.min.x) * (model.max.y - model.min.y);
                let center = model.min.midpoint(model.max);
                gravity.push((center, roof.dead_kn_m2 * area));
                if let Some(effect) = wind.roofs.iter().find(|effect| effect.id == roof.id) {
                    uplift.push((center, effect.uplift_kpa * area));
                }
            }
        }
        if supports.is_empty() {
            supports_ok = false;
        }
        let gravity_kn: f64 = gravity.iter().map(|(_, load)| load).sum();
        let roof_uplift_kn: f64 = uplift.iter().map(|(_, load)| load).sum();
        let nk: f64 = report
            .columns
            .iter()
            .map(|column| column.load_kn.max(0.0))
            .sum();
        let n_storeys = self.structure.settings.floors.clamp(1, 3) as f64;
        let alpha_limit = 0.2 + 0.1 * n_storeys;
        let bracing_ok = wind.bracing.iter().all(RoofBracingResult::ok);
        let mut rows = Vec::new();
        for (direction, along_x, positive) in [
            ("+X", true, true),
            ("−X", true, false),
            ("+Y", false, true),
            ("−Y", false, false),
        ] {
            let facade = if along_x { hi.y - lo.y } else { hi.x - lo.x };
            if facade < 0.1 {
                continue;
            }
            let q = self
                .structure
                .wind
                .pressure_kpa(height, self.building_size());
            let force = WALL_CP * q * facade * height;
            let moment = force * height / 2.0;
            let edge = supports
                .iter()
                .map(|(point, half_x, half_y)| {
                    let center = if along_x { point.x } else { point.y };
                    let half = if along_x { *half_x } else { *half_y };
                    if positive {
                        center + half
                    } else {
                        center - half
                    }
                })
                .reduce(|a, b| if positive { a.max(b) } else { a.min(b) })
                .unwrap_or(0.0);
            let downward_moment = gravity
                .iter()
                .map(|(point, load)| {
                    let at = if along_x { point.x } else { point.y };
                    0.9 * load * (if positive { edge - at } else { at - edge })
                })
                .sum::<f64>();
            let uplift_moment = uplift
                .iter()
                .map(|(point, load)| {
                    let at = if along_x { point.x } else { point.y };
                    1.4 * load * (if positive { edge - at } else { at - edge }).max(0.0)
                })
                .sum::<f64>();
            let stabilizing = downward_moment - uplift_moment;
            let ei: f64 = report
                .columns
                .iter()
                .filter_map(|column| {
                    if column.height_m < height - 0.30 {
                        return None;
                    }
                    let fix = self.structural_columns().find(|f| f.id == column.id)?;
                    let (s, c) = fix.rotation_rad().sin_cos();
                    let i_local_x = fix.depth * fix.width.powi(3) / 12.0;
                    let i_local_y = fix.width * fix.depth.powi(3) / 12.0;
                    let i = if along_x {
                        i_local_x * c * c + i_local_y * s * s
                    } else {
                        i_local_x * s * s + i_local_y * c * c
                    };
                    Some(
                        0.4 * self.structure.settings.modulus_kn_m2()
                            * i
                            * (height / column.height_m).powi(3),
                    )
                })
                .sum();
            let alpha = if ei > 1e-9 {
                height * (nk / ei).sqrt()
            } else {
                f64::INFINITY
            };
            let drift_mm = if ei > 1e-9 {
                force * height.powi(3) / (3.0 * ei) * 1000.0
            } else {
                f64::INFINITY
            };
            rows.push(GlobalStabilityResult {
                direction,
                height_m: height,
                facade_m: facade,
                wind_kn: force,
                overturning_knm: moment,
                gravity_kn,
                roof_uplift_kn,
                stabilizing_knm: stabilizing,
                overturning_ratio: stabilizing / (1.4 * moment).max(1e-9),
                alpha,
                alpha_limit,
                drift_mm,
                drift_limit_mm: height / 850.0 * 1000.0,
                supports_ok,
                bracing_ok,
            });
        }
        rows
    }

    /// Sets the site's wind.
    pub fn set_wind(&mut self, wind: WindSettings) -> Result<(), String> {
        wind.check()?;
        self.structure.wind = wind;
        Ok(())
    }

    /// Largest plan dimension of the building, metres.
    pub(crate) fn building_size(&self) -> f64 {
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
        let mut roof_columns: HashMap<u64, Vec<usize>> = HashMap::new();
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
            let roof_id = report
                .roofs
                .iter()
                .find(|r| {
                    r.reactions
                        .iter()
                        .any(|x| x.support.as_deref() == Some(result.label.as_str()))
                })
                .map(|r| r.id);
            let braced = roof_id.is_some();
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
            if let Some(id) = roof_id {
                roof_columns
                    .entry(id)
                    .or_default()
                    .push(out.columns.len() - 1);
            }
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
            // NBR 6123 §6.2.5: open side Cpi = +0.8; closed building uniform permeability Cpi = +0.2.
            let cpi = if open { OPEN_CPI } else { 0.2 };
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
        for roof in &self.structure.roofs {
            let Some(roof_result) = report.roofs.iter().find(|result| result.id == roof.id) else {
                continue;
            };
            let columns = roof_columns.get(&roof.id).map(Vec::as_slice).unwrap_or(&[]);
            // A simply supported wind-loaded column sends 3/8 of its
            // horizontal load to the top. The roof's own vertical face adds
            // a conservative projected-area load to that same path.
            let wall_shear: f64 = columns
                .iter()
                .map(|&index| {
                    let col = &out.columns[index];
                    3.0 / 8.0 * WALL_CP * col.q_kpa * col.tributary_m * col.height_m
                })
                .sum();
            let roof_face = WALL_CP
                * wind.pressure_kpa(roof.high_m, size.max(roof.eaves_m()))
                * (roof.high_m - roof.low_m)
                * roof.eaves_m();
            let demand = 1.4 * (wall_shear + roof_face);
            let grounded: Vec<_> = roof_result
                .reactions
                .iter()
                .filter(|reaction| {
                    reaction.support.as_ref().is_some_and(|label| {
                        report
                            .footings
                            .iter()
                            .any(|footing| footing.column.as_ref() == Some(label))
                    })
                })
                .map(|reaction| reaction.point)
                .collect();
            let path_ok = grounded
                .iter()
                .any(|a| grounded.iter().any(|b| a.distance(*b) > 0.5));
            let bracing = RoofBracingResult {
                roof_id: roof.id,
                label: roof_result.label.clone(),
                demand_kn: demand,
                capacity_kn: roof.lateral_bracing_kn,
                path_ok,
            };
            if !bracing.ok() {
                for &index in columns {
                    let col = &mut out.columns[index];
                    let w = WALL_CP * col.q_kpa * col.tributary_m;
                    col.braced = false;
                    col.moment_knm = w * col.height_m * col.height_m / 2.0;
                    let md = 1.4 * col.moment_knm;
                    let shear = w * col.height_m;
                    (col.ratio, col.steel_cm2) = bending(col.width_m, col.depth_m, md, fck);
                    col.needed_depth_m = size_rectangular_beam(
                        col.width_m.max(0.12),
                        md,
                        1.4 * shear,
                        fck,
                        0.15,
                        0.80,
                    )
                    .0;
                }
            }
            out.bracing.push(bracing);
        }
        out.global = self.global_wind_stability(report, &out);
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
        // Lower down, less wind; NBR 6123 Table 2 keeps S2 constant for z <= 5 m.
        assert!(wind.pressure_kpa(4.3, 15.0) < q);
        assert!((wind.s2(2.6, 15.0) - wind.s2(5.0, 15.0)).abs() < 1e-12);
        // Larger buildings (class B > 20 m, class C > 50 m) reduce S2 slightly.
        assert!(wind.s2(10.0, 30.0) < wind.s2(10.0, 15.0));
        assert!(wind.s2(10.0, 60.0) < wind.s2(10.0, 30.0));
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
            project
                .add_fixture(FixtureKind::Footing, Point::new(0.07, y), 0.80, 0.80, 0)
                .unwrap();
        }
        for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
            project
                .add_fixture(FixtureKind::TimberPost, Point::new(4.93, y), 0.15, 0.15, 0)
                .unwrap();
        }
        if with_roof {
            let mut roof = Roof::new(
                Point::new(0.0, 0.0),
                Point::new(5.0, 14.75),
                RoofSide::West,
                4.30,
                2.60,
            );
            roof.lateral_bracing_kn = Some(1000.0);
            project.add_roof(roof).unwrap();
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

    #[test]
    fn roof_bearing_alone_cannot_brace_the_wall_or_pass_global_stability() {
        let mut project = gourmet(true);
        project.structure.roofs[0].lateral_bracing_kn = None;
        let report = project.structure_report();
        let wind = &report.wind;
        assert!(!wind.bracing[0].ok());
        assert!(wind.bracing[0].path_ok);
        assert!(wind.columns.iter().all(|column| !column.braced));
        assert_eq!(wind.global.len(), 4);
        assert!(
            wind.global
                .iter()
                .all(|global| !global.bracing_ok && !global.ok())
        );
        let baseline = wind.global.iter().find(|g| g.direction == "+X").unwrap();
        let original_force = baseline.wind_kn;
        project.structure.wind.v0_ms = 45.0;
        let stronger = project.structure_report();
        let stronger = stronger
            .wind
            .global
            .iter()
            .find(|g| g.direction == "+X")
            .unwrap();
        assert!(stronger.wind_kn > original_force);
        assert!(stronger.drift_mm > baseline.drift_mm);
        assert!(stronger.overturning_knm > baseline.overturning_knm);
    }

    #[test]
    fn roof_bracing_needs_two_grounded_supports_and_enough_capacity() {
        let mut project = gourmet(true);
        let report = project.structure_report();
        assert!(report.wind.bracing[0].ok());
        assert!(report.wind.columns.iter().all(|column| column.braced));
        project.structure.roofs[0].lateral_bracing_kn = Some(1.0);
        let weak = project.structure_report();
        assert!(!weak.wind.bracing[0].ok());
        assert!(weak.wind.columns.iter().all(|column| !column.braced));
        project.structure.roofs[0].lateral_bracing_kn = Some(1000.0);
        project
            .fixtures
            .retain(|fixture| fixture.kind != FixtureKind::Footing);
        let ungrounded = project.structure_report();
        assert!(!ungrounded.wind.bracing[0].path_ok);
        assert!(ungrounded.wind.columns.iter().all(|column| !column.braced));
    }

    #[test]
    fn a_grounded_stiff_low_rise_passes_the_global_screening() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(8.0, 8.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        for x in [0.0, 4.0] {
            for y in [0.0, 2.0, 4.0] {
                project
                    .add_fixture(FixtureKind::Column, Point::new(x, y), 0.80, 0.80, 0)
                    .unwrap();
                project
                    .add_fixture(FixtureKind::Footing, Point::new(x, y), 1.50, 1.50, 0)
                    .unwrap();
            }
        }
        let mut roof = Roof::new(
            Point::new(0.0, 0.0),
            Point::new(4.0, 4.0),
            RoofSide::West,
            3.0,
            2.5,
        );
        roof.lateral_bracing_kn = Some(1000.0);
        project.add_roof(roof).unwrap();
        let wind = project.structure_report().wind;
        assert_eq!(wind.global.len(), 4);
        assert!(wind.bracing[0].ok());
        assert!(
            wind.global.iter().all(GlobalStabilityResult::ok),
            "{:?}",
            wind.global
        );
        project.fixtures.retain(|fixture| {
            fixture.kind != FixtureKind::Footing
                || fixture.center.distance(Point::new(4.0, 4.0)) > 0.1
        });
        let unsupported = project.structure_report().wind;
        assert!(
            unsupported
                .global
                .iter()
                .all(|direction| !direction.supports_ok && !direction.ok())
        );
    }
}
