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
use super::geometry::{Point, distance_to_segment, nearest_point_on_segment};
use super::model::Project;
use super::structure::{CONCRETE_KN_M3, continuous_beam_with_end_moments};

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
    /// Ground beam: receiving ground-beam ids and transferred reactions (kN).
    pub beam_reactions: Vec<(u64, f64)>,
    /// A manually selected ground beam without two connected supports.
    pub unsupported: bool,
}

/// What the wall foundations add to the footings, and their results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallFoundationEffects {
    /// Column id → ground beam reactions arriving at its footing, kN.
    pub at_column: HashMap<u64, f64>,
    pub results: Vec<WallFoundationResult>,
}

#[derive(Clone, Copy, Debug)]
enum GroundSupport {
    Column(u64),
    Beam { id: u64, station_m: f64 },
}

#[derive(Clone, Copy, Debug)]
struct GroundStation {
    at_m: f64,
    support: GroundSupport,
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

    fn ground_stations(
        &self,
        wall: &super::model::Wall,
        resolved: &HashMap<u64, (usize, Vec<GroundStation>)>,
    ) -> Vec<GroundStation> {
        let mut stations: Vec<GroundStation> = self
            .columns_on_wall(wall)
            .into_iter()
            .map(|(at_m, id, _)| GroundStation {
                at_m,
                support: GroundSupport::Column(id),
            })
            .collect();
        let Some(direction) = wall.end.minus(wall.start).normalized() else {
            return stations;
        };
        for (at_m, endpoint) in [(0.0, wall.start), (wall.length(), wall.end)] {
            if stations
                .iter()
                .any(|station| (station.at_m - at_m).abs() < 0.05)
            {
                continue;
            }
            let mut candidates = self
                .structure
                .wall_foundations
                .iter()
                .filter(|foundation| {
                    foundation.kind == WallFoundationKind::GroundBeam
                        && foundation.wall_id != wall.id
                })
                .filter_map(|foundation| {
                    let (rank, supports) = resolved.get(&foundation.id)?;
                    let other = self.walls.iter().find(|w| w.id == foundation.wall_id)?;
                    let other_dir = other.end.minus(other.start).normalized()?;
                    if direction.dot(other_dir).abs() >= 0.95
                        || distance_to_segment(endpoint, other.start, other.end) > 0.08
                    {
                        return None;
                    }
                    let (nearest, _) = nearest_point_on_segment(endpoint, other.start, other.end);
                    let station_m = nearest.distance(other.start);
                    let first = supports.first()?.at_m;
                    let last = supports.last()?.at_m;
                    if station_m < first - 1e-9 || station_m > last + 1e-9 {
                        return None;
                    }
                    Some((*rank, foundation.id, station_m))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            if let Some((_, id, station_m)) = candidates.first() {
                stations.push(GroundStation {
                    at_m,
                    support: GroundSupport::Beam {
                        id: *id,
                        station_m: *station_m,
                    },
                });
            }
        }
        stations.sort_by(|a, b| {
            a.at_m.total_cmp(&b.at_m).then_with(|| {
                let priority = |s: &GroundStation| match s.support {
                    GroundSupport::Column(_) => 0,
                    GroundSupport::Beam { .. } => 1,
                };
                priority(a).cmp(&priority(b))
            })
        });
        stations.dedup_by(|a, b| (a.at_m - b.at_m).abs() < 0.05);
        stations
    }

    fn resolved_ground_beams(&self) -> HashMap<u64, (usize, Vec<GroundStation>)> {
        let mut resolved = HashMap::new();
        let beams: Vec<_> = self
            .structure
            .wall_foundations
            .iter()
            .filter(|f| f.kind == WallFoundationKind::GroundBeam)
            .collect();
        for foundation in &beams {
            if let Some(wall) = self.walls.iter().find(|w| w.id == foundation.wall_id) {
                let stations = self
                    .columns_on_wall(wall)
                    .into_iter()
                    .map(|(at_m, id, _)| GroundStation {
                        at_m,
                        support: GroundSupport::Column(id),
                    })
                    .collect::<Vec<_>>();
                if stations.len() >= 2 {
                    resolved.insert(foundation.id, (0, stations));
                }
            }
        }
        loop {
            let mut added = 0;
            for foundation in &beams {
                if resolved.contains_key(&foundation.id) {
                    continue;
                }
                let Some(wall) = self.walls.iter().find(|w| w.id == foundation.wall_id) else {
                    continue;
                };
                let stations = self.ground_stations(wall, &resolved);
                if stations.len() >= 2 {
                    let rank = stations
                        .iter()
                        .filter_map(|station| match station.support {
                            GroundSupport::Beam { id, .. } => resolved.get(&id).map(|(r, _)| r + 1),
                            GroundSupport::Column(_) => None,
                        })
                        .max()
                        .unwrap_or(0);
                    resolved.insert(foundation.id, (rank, stations));
                    added += 1;
                }
            }
            if added == 0 {
                break;
            }
        }
        resolved
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
        let mut remaining = Vec::new();
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
            if !supported {
                remaining.push(wall);
                continue;
            }
            let id = self.id();
            self.structure.wall_foundations.push(WallFoundation {
                id,
                wall_id: wall.id,
                kind: WallFoundationKind::GroundBeam,
                width_m: None,
            });
            beams += 1;
        }
        // Extend the supported network from column-backed beams. A secondary
        // beam is promoted only when both wall ends reach a column or an
        // already resolved beam; cycles remain on strip footings.
        loop {
            let mut promoted = Vec::new();
            let resolved = self.resolved_ground_beams();
            remaining.retain(|wall| {
                let supports = self.ground_stations(wall, &resolved);
                let supported = supports.len() >= 2
                    && supports.first().is_some_and(|s| s.at_m <= 0.30)
                    && supports
                        .last()
                        .is_some_and(|s| s.at_m >= wall.length() - 0.30);
                if supported {
                    promoted.push(*wall);
                    false
                } else {
                    true
                }
            });
            if promoted.is_empty() {
                break;
            }
            // Re-evaluate in wall order so newly promoted beams are available
            // to the next level of the support network.
            for wall in promoted {
                let id = self.id();
                self.structure.wall_foundations.push(WallFoundation {
                    id,
                    wall_id: wall.id,
                    kind: WallFoundationKind::GroundBeam,
                    width_m: None,
                });
                beams += 1;
            }
        }
        for wall in remaining {
            let id = self.id();
            self.structure.wall_foundations.push(WallFoundation {
                id,
                wall_id: wall.id,
                kind: WallFoundationKind::StripFooting,
                width_m: None,
            });
            strips += 1;
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
        let (mut beam_no, mut strip_no) = (0, 0);
        let resolved = self.resolved_ground_beams();
        let mut incoming: HashMap<u64, Vec<(f64, f64)>> = HashMap::new();
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
        let mut labels = HashMap::new();
        for foundation in &ordered {
            let label = if foundation.kind == WallFoundationKind::GroundBeam
                && resolved.contains_key(&foundation.id)
            {
                beam_no += 1;
                format!("VB{beam_no}")
            } else {
                strip_no += 1;
                format!("SC{strip_no}")
            };
            labels.insert(foundation.id, label);
        }
        // Solve dependents first, while preserving spatial labels above.
        ordered.sort_by(|a, b| {
            let rank = |f: &WallFoundation| resolved.get(&f.id).map_or(0, |(r, _)| *r);
            rank(b).cmp(&rank(a))
        });
        for foundation in ordered {
            let Some(wall) = self.walls.iter().find(|w| w.id == foundation.wall_id) else {
                continue;
            };
            let length = wall.length().max(0.1);
            // Each wall's own (possibly sloped) top, #145.
            let net = self.wall_net_area(wall);
            let wall_q = s.masonry_kn_m3 * wall.thickness() * net / length;
            let mut result = WallFoundationResult {
                id: foundation.id,
                wall_id: wall.id,
                label: labels.get(&foundation.id).cloned().unwrap_or_default(),
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
                beam_reactions: Vec::new(),
                unsupported: false,
            };
            let on = resolved.get(&foundation.id).map(|(_, stations)| stations);
            match (foundation.kind, on) {
                (WallFoundationKind::GroundBeam, Some(on)) if on.len() >= 2 => {
                    let b = foundation
                        .width_m
                        .unwrap_or_else(|| round_up(wall.thickness().max(0.15), 0.05));
                    // Iterate self-weight with the sized height h (up to 3 passes) so
                    // taller ground beams (60–80 cm) carry their true dead load.
                    let mut h_guess = 0.40_f64;
                    let mut q = wall_q + CONCRETE_KN_M3 * b * h_guess;
                    let mut reactions = vec![0.0; on.len()];
                    let (mut moment, mut shear) = (0.0_f64, 0.0_f64);
                    let (mut h, mut flexure, mut shear_ratio, mut steel) = (None, 0.0, 0.0, 0.0);
                    let mut support_loads = vec![0.0; on.len()];
                    let mut span_loads = vec![Vec::new(); on.len() - 1];
                    for (at_m, load_kn) in incoming.remove(&foundation.id).unwrap_or_default() {
                        if let Some((i, _)) = on
                            .iter()
                            .enumerate()
                            .find(|(_, station)| (station.at_m - at_m).abs() <= 0.05)
                        {
                            support_loads[i] += load_kn;
                        } else if let Some((i, pair)) = on
                            .windows(2)
                            .enumerate()
                            .find(|(_, pair)| at_m > pair[0].at_m && at_m < pair[1].at_m)
                        {
                            span_loads[i].push((at_m - pair[0].at_m, load_kn));
                        } else {
                            // A junction within numerical tolerance of an end
                            // support still transfers its full vertical force.
                            let last = on.len() - 1;
                            let nearest = if at_m < on[0].at_m { 0 } else { last };
                            support_loads[nearest] += load_kn;
                        }
                    }
                    for loads in &mut span_loads {
                        loads.sort_by(|a, b| a.0.total_cmp(&b.0));
                    }
                    for _ in 0..3 {
                        q = wall_q + CONCRETE_KN_M3 * b * h_guess;
                        let spans: Vec<(f64, f64, f64)> = on
                            .windows(2)
                            .map(|p| ((p[1].at_m - p[0].at_m).max(0.1), q, 1.0))
                            .collect();
                        let before = on[0].at_m;
                        let after = (length - on[on.len() - 1].at_m).max(0.0);
                        let overhang_moments =
                            (-q * before * before / 2.0, -q * after * after / 2.0);
                        let ends =
                            continuous_beam_with_end_moments(&spans, &span_loads, overhang_moments);
                        reactions.clone_from(&support_loads);
                        moment = 0.0;
                        shear = 0.0;
                        for (i, ((l, w, _), (left, right))) in spans.iter().zip(&ends).enumerate() {
                            let pts = &span_loads[i];
                            let point_sum: f64 = pts.iter().map(|(_, p)| p).sum();
                            let point_left: f64 = pts.iter().map(|(a, p)| p * (l - a) / l).sum();
                            let v_left = w * l / 2.0 + point_left + (right - left) / l;
                            let v_right = v_left - w * l - point_sum;
                            reactions[i] += v_left;
                            reactions[i + 1] -= v_right;
                            let moment_at = |x: f64| {
                                left + v_left * x
                                    - w * x * x / 2.0
                                    - pts.iter().map(|(a, p)| p * (x - a).max(0.0)).sum::<f64>()
                            };
                            let mut start = 0.0;
                            let mut p_before = 0.0;
                            for (end, p_at_end) in pts
                                .iter()
                                .map(|(a, p)| (*a, *p))
                                .chain(std::iter::once((*l, 0.0)))
                            {
                                let zero_shear =
                                    ((v_left - p_before) / w.max(1e-9)).clamp(start, end);
                                moment = moment
                                    .max(moment_at(start).abs())
                                    .max(moment_at(end).abs())
                                    .max(moment_at(zero_shear).abs());
                                shear = shear
                                    .max((v_left - w * start - p_before).abs())
                                    .max((v_left - w * end - p_before).abs());
                                p_before += p_at_end;
                                start = end;
                            }
                            shear = shear.max(v_right.abs());
                        }
                        // Wall beyond the end columns hangs on them as a cantilever.
                        reactions[0] += q * before;
                        let last = reactions.len() - 1;
                        reactions[last] += q * after;
                        shear = shear.max(q * before).max(q * after);
                        let sized = size_rectangular_beam(
                            b,
                            1.4 * moment,
                            1.4 * shear,
                            s.fck_mpa,
                            0.30,
                            0.80,
                        );
                        h = sized.0;
                        flexure = sized.1;
                        shear_ratio = sized.2;
                        steel = sized.3;
                        let next_h = h.unwrap_or(0.80);
                        if (next_h - h_guess).abs() < 1e-6 {
                            break;
                        }
                        h_guess = next_h;
                    }
                    result.load_kn_m = q;
                    result.moment_knm = moment;
                    result.shear_kn = shear;
                    result.width_m = b;
                    result.height_m = h;
                    result.flexure_ratio = flexure;
                    result.shear_ratio = shear_ratio;
                    result.steel_cm2 = steel;
                    result.bars = choose_bars(steel).replace("(superior)", "(em cima e embaixo)");
                    for (station, reaction) in on.iter().zip(&reactions) {
                        match station.support {
                            GroundSupport::Column(id) => {
                                *effects.at_column.entry(id).or_default() += reaction;
                                result.reactions.push((id, *reaction));
                            }
                            GroundSupport::Beam { id, station_m } => {
                                incoming.entry(id).or_default().push((station_m, *reaction));
                                result.beam_reactions.push((id, *reaction));
                            }
                        }
                    }
                }
                _ => {
                    result.unsupported = foundation.kind == WallFoundationKind::GroundBeam;
                    result.kind = WallFoundationKind::StripFooting;
                    // Exact closed-form strip width (25 cm footing thickness):
                    // σ = (wall_q + 25·B·0.25) / B <= soil_kpa => B >= wall_q / (soil_kpa - 6.25).
                    let soil = s.soil_kpa.max(10.0);
                    let strip_self_kpa = CONCRETE_KN_M3 * 0.25;
                    let width = if soil > strip_self_kpa + 1.0 {
                        round_up(1.05 * wall_q / (soil - strip_self_kpa), 0.05).max(0.40)
                    } else {
                        round_up(1.05 * wall_q / soil, 0.05).max(0.40)
                    };
                    let used = foundation.width_m.unwrap_or(width);
                    let q = wall_q + CONCRETE_KN_M3 * used * 0.25;
                    result.load_kn_m = q;
                    result.needed_width_m = width;
                    result.width_m = used;
                    result.pressure_kpa =
                        (1.05 * wall_q + CONCRETE_KN_M3 * used * 0.25) / used.max(0.01);
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
        let h_actual = beam.height_m.unwrap();
        assert!((beam.load_kn_m - wall_q - 25.0 * beam.width_m * h_actual).abs() < 1e-9);
        // Drawn on the formwork plan.
        let marks = project.structure_marks();
        assert!(
            marks
                .iter()
                .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with("VB1")))
        );
    }

    #[test]
    fn ground_beam_overhang_reactions_follow_force_and_moment_equilibrium() {
        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0))
            .unwrap();
        let left = project
            .add_fixture(FixtureKind::Column, Point::new(1.0, 0.0), 0.20, 0.20, 0)
            .unwrap();
        let right = project
            .add_fixture(FixtureKind::Column, Point::new(5.0, 0.0), 0.20, 0.20, 0)
            .unwrap();
        project
            .set_wall_foundation(wall, WallFoundationKind::GroundBeam, None)
            .unwrap();
        let effects = project.wall_foundation_effects();
        let beam = &effects.results[0];
        let q = beam.load_kn_m;
        assert_eq!(beam.reactions.len(), 2);
        assert_eq!(beam.reactions[0].0, left);
        assert_eq!(beam.reactions[1].0, right);
        assert!((beam.reactions[0].1 - 3.125 * q).abs() < 1e-6);
        assert!((beam.reactions[1].1 - 1.875 * q).abs() < 1e-6);
        assert!((beam.reactions.iter().map(|(_, r)| r).sum::<f64>() - 5.0 * q).abs() < 1e-6);
        assert!((beam.moment_knm - 1.7578125 * q).abs() < 1e-6);
    }

    #[test]
    fn excessive_overhang_reports_uplift_instead_of_hiding_the_reaction() {
        use crate::domain::verdict::{Light, structure_verdicts};

        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.5, 0.0))
            .unwrap();
        project
            .add_fixture(FixtureKind::Column, Point::new(2.5, 0.0), 0.20, 0.20, 0)
            .unwrap();
        let right = project
            .add_fixture(FixtureKind::Column, Point::new(3.5, 0.0), 0.20, 0.20, 0)
            .unwrap();
        project
            .set_wall_foundation(wall, WallFoundationKind::GroundBeam, None)
            .unwrap();
        let effects = project.wall_foundation_effects();
        let beam = &effects.results[0];
        assert!((beam.reactions[1].1 + 2.625 * beam.load_kn_m).abs() < 1e-6);
        assert!((effects.at_column[&right] - beam.reactions[1].1).abs() < 1e-6);

        let report = project.structure_report();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("ascendente"))
        );
        let verdicts = structure_verdicts(&project, &report);
        assert!(verdicts.iter().any(|verdict| {
            verdict.element.contains(&beam.label)
                && verdict
                    .checks
                    .iter()
                    .any(|check| check.light == Light::Red && check.name == "Apoio")
        }));
    }

    #[test]
    fn secondary_ground_beams_route_loads_through_a_beam_chain_to_columns() {
        let mut project = Project::default();
        let primary_wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
            .unwrap();
        let secondary_wall = project
            .add_wall(Point::new(3.0, 0.0), Point::new(3.0, 4.0))
            .unwrap();
        let tertiary_wall = project
            .add_wall(Point::new(3.0, 2.0), Point::new(5.0, 2.0))
            .unwrap();
        for point in [
            Point::new(0.0, 0.0),
            Point::new(6.0, 0.0),
            Point::new(3.0, 4.0),
            Point::new(5.0, 2.0),
        ] {
            project
                .add_fixture(FixtureKind::Column, point, 0.20, 0.20, 0)
                .unwrap();
        }
        project
            .set_wall_foundation(primary_wall, WallFoundationKind::GroundBeam, None)
            .unwrap();
        let baseline_moment = project.wall_foundation_effects().results[0].moment_knm;
        assert_eq!(project.generate_wall_foundations(), (2, 0));
        let effects = project.wall_foundation_effects();
        let primary = effects
            .results
            .iter()
            .find(|r| r.wall_id == primary_wall)
            .unwrap();
        let secondary = effects
            .results
            .iter()
            .find(|r| r.wall_id == secondary_wall)
            .unwrap();
        let tertiary = effects
            .results
            .iter()
            .find(|r| r.wall_id == tertiary_wall)
            .unwrap();
        assert_eq!(tertiary.beam_reactions.len(), 1);
        assert_eq!(tertiary.beam_reactions[0].0, secondary.id);
        assert_eq!(secondary.beam_reactions.len(), 1);
        assert_eq!(secondary.beam_reactions[0].0, primary.id);
        assert_eq!(primary.reactions.len(), 2);
        assert!(primary.moment_knm > baseline_moment);
        let total_wall_and_beams: f64 = effects
            .results
            .iter()
            .map(|result| result.load_kn_m * result.length_m)
            .sum();
        let total_columns: f64 = effects.at_column.values().sum();
        assert!((total_columns - total_wall_and_beams).abs() < 1e-6);
    }

    #[test]
    fn unsupported_ground_beam_falls_back_to_strip_with_a_failed_support_check() {
        use crate::domain::verdict::{Light, structure_verdicts};

        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        project
            .set_wall_foundation(wall, WallFoundationKind::GroundBeam, None)
            .unwrap();
        let report = project.structure_report();
        let result = &report.wall_foundations[0];
        assert_eq!(result.kind, WallFoundationKind::StripFooting);
        assert!(result.unsupported);
        assert!(structure_verdicts(&project, &report).iter().any(|verdict| {
            verdict.element.contains(&result.label)
                && verdict
                    .checks
                    .iter()
                    .any(|c| c.light == Light::Red && c.name == "Apoios")
        }));
    }

    #[test]
    fn a_beam_cycle_without_a_grounded_path_stays_on_strip_footings() {
        let mut project = Project::default();
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(0.0, 4.0))
            .unwrap();
        for point in [Point::new(4.0, 0.0), Point::new(0.0, 4.0)] {
            project
                .add_fixture(FixtureKind::Column, point, 0.20, 0.20, 0)
                .unwrap();
        }
        assert_eq!(project.generate_wall_foundations(), (0, 2));
        assert!(
            project
                .wall_foundation_effects()
                .results
                .iter()
                .all(|result| result.kind == WallFoundationKind::StripFooting)
        );
    }
}
