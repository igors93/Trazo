//! Timber posts (*mourões*) and the timber beam over them (#146).
//!
//! A roof beam end with no concrete column or beam under it bears on a
//! timber post within 35 cm, or on a timber beam along the roof edge that
//! spans from post to post (simply supported, point loads shared by
//! statics). Every post is checked in compression with buckling and gets a
//! concrete base:
//!
//! - **Slenderness:** λ = L0 / i, L0 the post height (braced at the top by
//!   the roof), at most 140 (NBR 7190).
//! - **Short posts** (λ ≤ 40): σNd = Nd / A ≤ fc0,d.
//! - **Medium and slender posts:** σNd / fc0,d + σMd / fc0,d ≤ 1 with
//!   Md = Nd · ed, ed = (ei + ea + ec) · FE / (FE − Nd), ei = h/30,
//!   ea = L0/300, FE = π² Ec0,ef I / L0²; above λ = 80 the creep
//!   eccentricity ec = (ei + ea) · (exp(φ Ngq / (FE − Ngq)) − 1), φ = 0.8,
//!   Ngq = Ngk + 0.2 Nqk (NBR 7190:1997, 7.5).
//! - **Base:** a concrete block B × B with B = √(1.05 N / σadm), at least
//!   40 cm, in 5 cm steps; the timber stays off the ground on a steel
//!   anchor.
//!
//! Wind acts as a distributed lateral load on the exposed post. Its design
//! bending utilisation is added to the axial/buckling utilisation.

use std::collections::HashMap;

use super::conduits::PlanMark;
use super::fixtures::{Fixture, FixtureKind};
use super::geometry::Point;
use super::model::Project;
use super::roof::{RoofResult, TimberClass, TimberSpecies, timber_design};
use super::wind::WALL_CP;

/// Most slender post NBR 7190 allows.
pub const MAX_POST_SLENDERNESS: f64 = 140.0;

/// A timber post checked.
#[derive(Clone, Debug, PartialEq)]
pub struct TimberPostResult {
    pub id: u64,
    pub label: String,
    /// Diameter (round) or smallest side (square), metres.
    pub size_m: f64,
    pub round: bool,
    pub height_m: f64,
    /// Characteristic load and its permanent part, kN.
    pub load_kn: f64,
    pub dead_kn: f64,
    pub species: TimberSpecies,
    pub timber_class: TimberClass,
    pub treated: bool,
    pub slenderness: f64,
    /// Axial buckling and lateral wind contributions, then their interaction.
    pub axial_ratio: f64,
    pub wind_ratio: f64,
    pub wind_kpa: f64,
    pub wind_force_kn: f64,
    pub wind_moment_knm: f64,
    pub ratio: f64,
    /// Smallest size that passes, metres (`None` when even 30 cm fails).
    pub needed_m: Option<f64>,
    /// Side of the concrete base and the pressure under it.
    pub base_m: f64,
    pub base_depth_m: f64,
    pub base_kpa: f64,
    /// No-tension/kern check under the wind moment, and design anchor tension.
    pub base_stability_ratio: f64,
    pub anchor_kn: f64,
    /// Allowable soil pressure, kPa.
    pub soil_kpa: f64,
}

impl TimberPostResult {
    pub fn ok(&self) -> bool {
        self.treated
            && self.ratio <= 1.0 + 1e-9
            && self.slenderness <= MAX_POST_SLENDERNESS + 1e-9
            && self.base_kpa <= self.soil_kpa + 1e-9
            && self.base_stability_ratio <= 1.0 + 1e-9
    }
}

/// Square concrete block kept fully in compression under vertical load and
/// wind moment. Returns side, depth, maximum soil pressure and kern ratio.
fn size_post_base(load_kn: f64, moment_knm: f64, soil_kpa: f64) -> (f64, f64, f64, f64) {
    const CONCRETE_KN_M3: f64 = 24.0;
    let mut last = (2.0, 2.0, f64::INFINITY, f64::INFINITY);
    for step in 8..=40 {
        let side = f64::from(step) * 0.05;
        let depth = side.max(0.60);
        let vertical = 1.05 * load_kn + CONCRETE_KN_M3 * side * side * depth;
        let stability = 6.0 * moment_knm / (vertical * side).max(1e-9);
        let qmax = vertical / (side * side) * (1.0 + stability);
        last = (side, depth, qmax, stability);
        if qmax <= soil_kpa && stability <= 1.0 {
            return last;
        }
    }
    last
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostCheck {
    pub slenderness: f64,
    pub axial_ratio: f64,
    pub wind_ratio: f64,
    pub combined_ratio: f64,
    pub wind_force_kn: f64,
    pub wind_moment_knm: f64,
}

/// A timber beam from post to post along a roof edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EaveBeamResult {
    pub label: String,
    pub species: TimberSpecies,
    pub timber_class: TimberClass,
    pub treated: bool,
    pub from: Point,
    pub to: Point,
    pub from_post: String,
    pub to_post: String,
    pub span_m: f64,
    /// Roof beam ends it carries: (distance from `from`, kN).
    pub loads: Vec<(f64, f64)>,
    pub section_cm: (f64, f64),
    pub ok: bool,
    pub bending_ratio: f64,
    pub shear_ratio: f64,
    pub deflection_ratio: f64,
}

/// The posts and the timber beams of the structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PostEffects {
    pub posts: Vec<TimberPostResult>,
    pub eave_beams: Vec<EaveBeamResult>,
}

/// Area, inertia and section modulus of a post `size` across.
fn post_section(round: bool, size: f64) -> (f64, f64, f64) {
    if round {
        let pi = std::f64::consts::PI;
        (
            pi * size * size / 4.0,
            pi * size.powi(4) / 64.0,
            pi * size.powi(3) / 32.0,
        )
    } else {
        (size * size, size.powi(4) / 12.0, size.powi(3) / 6.0)
    }
}

/// Slenderness and utilisation of a post in compression, buckling and wind.
pub fn post_check(
    timber: TimberClass,
    round: bool,
    size: f64,
    height: f64,
    dead_kn: f64,
    live_kn: f64,
    wind_kpa: f64,
) -> PostCheck {
    let (fc0d, _, e_ef, _) = timber_design(timber);
    let (area, inertia, modulus) = post_section(round, size);
    let l0 = height.max(0.1);
    let lambda = l0 / (inertia / area).sqrt();
    let nd = 1.4 * (dead_kn + live_kn);
    let sigma_n = nd / area;
    let wind_force = wind_kpa.max(0.0) * size * l0;
    let wind_moment = wind_kpa.max(0.0) * size * l0 * l0 / 2.0;
    let wind_ratio = 1.4 * wind_moment / modulus / fc0d;
    let axial_ratio = if lambda > MAX_POST_SLENDERNESS + 1e-9 {
        f64::INFINITY
    } else if lambda <= 40.0 {
        sigma_n / fc0d
    } else {
        let fe = std::f64::consts::PI.powi(2) * e_ef * inertia / (l0 * l0);
        if nd >= fe {
            f64::INFINITY
        } else {
            let ei = size / 30.0;
            let ea = l0 / 300.0;
            let ec = if lambda > 80.0 {
                let ngq = dead_kn + 0.2 * live_kn;
                if ngq >= fe {
                    f64::INFINITY
                } else {
                    (ei + ea) * ((0.8 * ngq / (fe - ngq)).exp() - 1.0)
                }
            } else {
                0.0
            };
            if ec.is_infinite() {
                f64::INFINITY
            } else {
                let ed = (ei + ea + ec) * fe / (fe - nd);
                let moment = nd * ed;
                sigma_n / fc0d + moment / modulus / fc0d
            }
        }
    };
    PostCheck {
        slenderness: lambda,
        axial_ratio,
        wind_ratio,
        combined_ratio: axial_ratio + wind_ratio,
        wind_force_kn: wind_force,
        wind_moment_knm: wind_moment,
    }
}

/// Sizes a simply supported timber beam of span `span` under point loads
/// `(a, P)` (characteristic kN) and its own weight.
fn size_eave_beam(
    timber: TimberClass,
    span: f64,
    loads: &[(f64, f64)],
) -> ((f64, f64), bool, f64, f64, f64) {
    const CATALOG: [(f64, f64); 7] = [
        (6.0, 12.0),
        (6.0, 16.0),
        (8.0, 16.0),
        (8.0, 20.0),
        (10.0, 20.0),
        (10.0, 25.0),
        (12.0, 30.0),
    ];
    let (fc0d, fv0d, e_ef, gamma) = timber_design(timber);
    let mut last = (
        (0.0, 0.0),
        false,
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
    );
    for (b_cm, h_cm) in CATALOG {
        let (b, h) = (b_cm / 100.0, h_cm / 100.0);
        let w = gamma * b * h;
        let r0: f64 = loads
            .iter()
            .map(|(a, p)| p * (span - a) / span)
            .sum::<f64>()
            + w * span / 2.0;
        let r1: f64 = loads.iter().map(|(a, p)| p * a / span).sum::<f64>() + w * span / 2.0;
        // The largest moment is under a load or at midspan.
        let moment_at = |x: f64| {
            r0 * x
                - loads
                    .iter()
                    .filter(|(a, _)| *a < x)
                    .map(|(a, p)| p * (x - a))
                    .sum::<f64>()
                - w * x * x / 2.0
        };
        let moment = loads
            .iter()
            .map(|(a, _)| moment_at(*a))
            .chain([moment_at(span / 2.0)])
            .fold(0.0, f64::max);
        let inertia = b * h.powi(3) / 12.0;
        let bending = 1.4 * moment / (b * h * h / 6.0) / fc0d;
        let shear = 1.5 * 1.4 * r0.max(r1) / (b * h) / fv0d;
        // Each load's largest deflection, added (conservative), and the own weight.
        let deflection = loads
            .iter()
            .map(|(a, p)| {
                let bb = a.min(span - a);
                p * bb * (span * span - bb * bb).powf(1.5)
                    / (9.0 * 3f64.sqrt() * span * e_ef * inertia)
            })
            .sum::<f64>()
            + 5.0 * w * span.powi(4) / (384.0 * e_ef * inertia);
        let deflection_ratio = deflection / (span / 200.0);
        let ok = bending <= 1.0 + 1e-9 && shear <= 1.0 + 1e-9 && deflection_ratio <= 1.0 + 1e-9;
        last = ((b_cm, h_cm), ok, bending, shear, deflection_ratio);
        if ok {
            break;
        }
    }
    last
}

impl Project {
    pub fn timber_posts(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::TimberPost)
    }

    /// M1, M2… in reading order.
    pub fn post_labels(&self) -> HashMap<u64, String> {
        let mut posts: Vec<&Fixture> = self.timber_posts().collect();
        posts.sort_by(|a, b| {
            ((a.center.y / 0.3).round() as i64)
                .cmp(&((b.center.y / 0.3).round() as i64))
                .then(a.center.x.total_cmp(&b.center.x))
        });
        posts
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id, format!("M{}", i + 1)))
            .collect()
    }

    /// Height of a post: the roof over it, else the storey height.
    pub fn post_height(&self, post: &Fixture) -> f64 {
        self.structure
            .roofs
            .iter()
            .filter(|r| {
                let p = post.center;
                p.x >= r.min.x - 0.35
                    && p.x <= r.max.x + 0.35
                    && p.y >= r.min.y - 0.35
                    && p.y <= r.max.y + 0.35
            })
            .map(|r| r.height_at(post.center))
            .fold(None, |best: Option<f64>, h| {
                Some(best.map_or(h, |b| b.max(h)))
            })
            .unwrap_or(self.wall_height)
    }

    /// Bears the roof beam ends left without support on the posts, directly
    /// or through a timber beam along the edge, and checks every post.
    pub fn post_effects(&self, roofs: &mut [RoofResult]) -> PostEffects {
        let labels = self.post_labels();
        let species = self.structure.post_species;
        let timber = species.class();
        let mut loads: HashMap<u64, (f64, f64)> = HashMap::new();
        let mut eave_beams = Vec::new();
        for result in roofs.iter_mut() {
            let Some(roof) = self.structure.roofs.iter().find(|r| r.id == result.id) else {
                continue;
            };
            for reaction in result.reactions.iter_mut().filter(|r| r.support.is_none()) {
                let post = self
                    .timber_posts()
                    .filter(|p| {
                        p.contains(reaction.point) || p.center.distance(reaction.point) <= 0.35
                    })
                    .min_by(|a, b| {
                        a.center
                            .distance(reaction.point)
                            .total_cmp(&b.center.distance(reaction.point))
                    });
                if let Some(post) = post {
                    let entry = loads.entry(post.id).or_default();
                    entry.0 += reaction.dead_kn;
                    entry.1 += reaction.load_kn;
                    reaction.support = labels.get(&post.id).cloned();
                }
            }
            for high in [true, false] {
                let mut edge: Vec<(f64, &Fixture)> = self
                    .timber_posts()
                    .filter(|p| roof.edge_of(p.center, 0.35) == Some(high))
                    .map(|p| (roof.along(p.center), p))
                    .collect();
                edge.sort_by(|a, b| a.0.total_cmp(&b.0));
                for pair in edge.windows(2) {
                    let ((a0, p0), (a1, p1)) = (pair[0], pair[1]);
                    let span = a1 - a0;
                    if span < 0.3 {
                        continue;
                    }
                    let label = format!("VM{}", eave_beams.len() + 1);
                    let mut point_loads = Vec::new();
                    let mut dead_loads = Vec::new();
                    for reaction in result
                        .reactions
                        .iter_mut()
                        .filter(|r| r.support.is_none() && r.high == high)
                    {
                        let at = roof.along(reaction.point);
                        if at > a0 + 1e-6 && at < a1 - 1e-6 {
                            point_loads.push((at - a0, reaction.load_kn));
                            dead_loads.push(reaction.dead_kn);
                            reaction.support = Some(label.clone());
                        }
                    }
                    let (section, ok, bending, shear, deflection) =
                        size_eave_beam(timber, span, &point_loads);
                    let (_, _, _, gamma) = timber_design(timber);
                    let own = gamma * section.0 * section.1 / 10_000.0 * span / 2.0;
                    for ((a, p), g) in point_loads.iter().zip(&dead_loads) {
                        let share1 = a / span;
                        for (post, share) in [(p0, 1.0 - share1), (p1, share1)] {
                            let entry = loads.entry(post.id).or_default();
                            entry.0 += g * share;
                            entry.1 += p * share;
                        }
                    }
                    for post in [p0, p1] {
                        let entry = loads.entry(post.id).or_default();
                        entry.0 += own;
                        entry.1 += own;
                    }
                    eave_beams.push(EaveBeamResult {
                        label,
                        species,
                        timber_class: timber,
                        treated: self.structure.post_treated,
                        from: p0.center,
                        to: p1.center,
                        from_post: labels.get(&p0.id).cloned().unwrap_or_default(),
                        to_post: labels.get(&p1.id).cloned().unwrap_or_default(),
                        span_m: span,
                        loads: point_loads,
                        section_cm: section,
                        ok,
                        bending_ratio: bending,
                        shear_ratio: shear,
                        deflection_ratio: deflection,
                    });
                }
            }
        }
        let soil = self.structure.settings.soil_kpa.max(10.0);
        let mut posts: Vec<TimberPostResult> = self
            .timber_posts()
            .map(|post| {
                let (dead, total) = loads.get(&post.id).copied().unwrap_or((0.0, 0.0));
                let round = (post.width - post.depth).abs() < 1e-6;
                let size = post.width.min(post.depth);
                let height = self.post_height(post);
                let (_, _, _, gamma) = timber_design(timber);
                let own = gamma * post_section(round, size).0 * height;
                let (dead, total) = (dead + own, total + own);
                let live = total - dead;
                let plan_size = self
                    .structure
                    .roofs
                    .iter()
                    .filter(|roof| {
                        post.center.x >= roof.min.x - 0.35
                            && post.center.x <= roof.max.x + 0.35
                            && post.center.y >= roof.min.y - 0.35
                            && post.center.y <= roof.max.y + 0.35
                    })
                    .map(|roof| roof.eaves_m().max(roof.run_m()))
                    .fold(10.0_f64, f64::max);
                let wind_kpa = WALL_CP * self.structure.wind.pressure_kpa(height, plan_size);
                let check = post_check(timber, round, size, height, dead, live, wind_kpa);
                let needed = (12..=30).map(|cm| f64::from(cm) / 100.0).find(|s| {
                    post_check(timber, round, *s, height, dead, live, wind_kpa).combined_ratio
                        <= 1.0
                });
                let (base, base_depth, base_kpa, base_stability) =
                    size_post_base(total, check.wind_moment_knm, soil);
                let anchor = 1.4 * check.wind_moment_knm / (0.8 * size).max(0.05);
                TimberPostResult {
                    id: post.id,
                    label: labels.get(&post.id).cloned().unwrap_or_default(),
                    size_m: size,
                    round,
                    height_m: height,
                    load_kn: total,
                    dead_kn: dead,
                    species,
                    timber_class: timber,
                    treated: self.structure.post_treated,
                    slenderness: check.slenderness,
                    axial_ratio: check.axial_ratio,
                    wind_ratio: check.wind_ratio,
                    wind_kpa,
                    wind_force_kn: check.wind_force_kn,
                    wind_moment_knm: check.wind_moment_knm,
                    ratio: check.combined_ratio,
                    needed_m: needed,
                    base_m: base,
                    base_depth_m: base_depth,
                    base_kpa,
                    base_stability_ratio: base_stability,
                    anchor_kn: anchor,
                    soil_kpa: soil,
                }
            })
            .collect();
        posts.sort_by(|a, b| {
            let n = |l: &str| l.trim_start_matches('M').parse::<u32>().unwrap_or(0);
            n(&a.label).cmp(&n(&b.label))
        });
        PostEffects { posts, eave_beams }
    }

    /// The posts' bases and the timber beams drawn on the plan.
    /// Timber post marks filtered by view scope:
    /// - `include_bases`: dashed foundation footing square around each post (`Fundações`).
    /// - `include_eave_beams`: overhead timber eave beams `VM#` (`Estrutura`).
    pub fn post_marks_filtered(
        &self,
        effects: &PostEffects,
        include_bases: bool,
        include_eave_beams: bool,
    ) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in &effects.posts {
            let Some(post) = self.timber_posts().find(|p| p.id == result.id) else {
                continue;
            };
            let h = result.base_m / 2.0;
            let c = post.center;
            if include_bases {
                marks.push(PlanMark::Line {
                    points: vec![
                        Point::new(c.x - h, c.y - h),
                        Point::new(c.x + h, c.y - h),
                        Point::new(c.x + h, c.y + h),
                        Point::new(c.x - h, c.y + h),
                        Point::new(c.x - h, c.y - h),
                    ],
                    dashed: true,
                    heavy: false,
                });
            }
            let label_offset = if include_bases {
                h + 0.18
            } else {
                result.size_m / 2.0 + 0.16
            };
            marks.push(PlanMark::Text {
                anchor: Point::new(c.x, c.y + label_offset),
                text: format!(
                    "{} {}{:.0}",
                    result.label,
                    if result.round { "Ø" } else { "□" },
                    result.size_m * 100.0
                ),
            });
        }
        if include_eave_beams {
            for beam in &effects.eave_beams {
                marks.push(PlanMark::Line {
                    points: vec![beam.from, beam.to],
                    dashed: false,
                    heavy: true,
                });
                marks.push(PlanMark::Text {
                    anchor: beam
                        .from
                        .lerp(beam.to, 0.5)
                        .translated(Point::new(0.25, 0.0)),
                    text: format!(
                        "{} {:.0}×{:.0}",
                        beam.label, beam.section_cm.0, beam.section_cm.1
                    ),
                });
            }
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::roof::{Roof, RoofSide};

    #[test]
    fn treatment_defaults_true_and_old_class_projects_migrate() {
        assert!(Project::default().structure.post_treated);
        let structure: crate::domain::structure::ProjectStructure =
            serde_json::from_str(r#"{"post_timber":"c40"}"#).unwrap();
        assert_eq!(structure.post_species, TimberSpecies::ClassC40);
        assert!(structure.post_treated);
    }

    #[test]
    fn a_short_post_is_checked_in_plain_compression() {
        // Ø 20 cm, 1.5 m: λ = 1.5 / 0.05 = 30.
        let check = post_check(TimberClass::C30, true, 0.20, 1.5, 20.0, 5.0, 0.0);
        assert!((check.slenderness - 30.0).abs() < 1e-9);
        let area = std::f64::consts::PI * 0.04 / 4.0;
        let fc0d = timber_design(TimberClass::C30).0;
        assert!((check.combined_ratio - 1.4 * 25.0 / area / fc0d).abs() < 1e-9);
    }

    #[test]
    fn a_slender_post_pays_for_buckling_and_creep() {
        // Ø 12 cm, 2.6 m: λ = 2.6 / 0.03 = 86.7, above 80.
        let check = post_check(TimberClass::C30, true, 0.12, 2.6, 6.0, 2.0, 0.0);
        assert!(
            check.slenderness > 80.0 && check.slenderness < 90.0,
            "{}",
            check.slenderness
        );
        let area = std::f64::consts::PI * 0.0144 / 4.0;
        let plain = 1.4 * 8.0 / area / timber_design(TimberClass::C30).0;
        assert!(
            check.combined_ratio > plain * 1.2,
            "{} vs {plain}",
            check.combined_ratio
        );
        // Too slender: λ > 140.
        assert!(
            post_check(TimberClass::C30, true, 0.08, 3.0, 1.0, 0.0, 0.0)
                .combined_ratio
                .is_infinite()
        );
    }

    #[test]
    fn lateral_wind_adds_bending_to_the_axial_post_check() {
        let calm = post_check(TimberClass::C30, false, 0.15, 3.0, 8.0, 2.0, 0.0);
        let windy = post_check(TimberClass::C30, false, 0.15, 3.0, 8.0, 2.0, 0.80);
        assert!(windy.wind_force_kn > 0.0 && windy.wind_moment_knm > 0.0);
        assert!(windy.combined_ratio > calm.combined_ratio);
        assert!((windy.combined_ratio - windy.axial_ratio - windy.wind_ratio).abs() < 1e-12);
    }

    #[test]
    fn the_post_base_resists_soil_pressure_and_wind_overturning() {
        let (side, depth, pressure, stability) = size_post_base(8.0, 0.8, 150.0);
        assert!(side >= 0.40 && depth >= 0.60);
        assert!(pressure <= 150.0, "{pressure}");
        assert!(stability <= 1.0, "{stability}");
        let (_, _, _, windier) = size_post_base(8.0, 1.6, 150.0);
        assert!(windier <= 1.0);
    }

    #[test]
    fn the_gourmet_posts_carry_the_low_edge_of_the_roof() {
        let mut project = Project::default();
        for y in [0.07, 3.0, 6.0, 9.0, 12.0, 14.68] {
            project
                .add_fixture(FixtureKind::Column, Point::new(0.07, y), 0.14, 0.30, 0)
                .unwrap();
        }
        // Posts every 3 m on the pool side, one of them missing at 9 m.
        for y in [0.07, 3.0, 6.0, 12.0, 14.68] {
            project
                .add_fixture(FixtureKind::TimberPost, Point::new(4.93, y), 0.15, 0.15, 0)
                .unwrap();
        }
        project
            .add_roof(Roof::new(
                Point::new(0.0, 0.0),
                Point::new(5.0, 14.75),
                RoofSide::West,
                4.30,
                2.60,
            ))
            .unwrap();
        let report = project.structure_report();
        let roof = &report.roofs[0];
        assert_eq!(roof.unsupported(), 0, "{:#?}", roof.reactions);
        // The beam end at 9 m rides on a timber beam from M3 to M4.
        assert_eq!(report.eave_beams.len(), 4);
        let carrying = report
            .eave_beams
            .iter()
            .find(|b| !b.loads.is_empty())
            .unwrap();
        assert!((carrying.span_m - 6.0).abs() < 1e-6);
        assert!(carrying.ok, "{carrying:?}");
        assert_eq!(report.timber_posts.len(), 5);
        let low: f64 = roof
            .reactions
            .iter()
            .filter(|r| !r.high)
            .map(|r| r.load_kn)
            .sum();
        let posts: f64 = report.timber_posts.iter().map(|p| p.load_kn).sum();
        // Posts take the low edge plus their own and the timber beams' weight.
        assert!(posts > low && posts < low * 1.25, "{posts} vs {low}");
        for post in &report.timber_posts {
            assert!((post.height_m - 2.6).abs() < 0.03, "{}", post.height_m);
            assert!(post.base_m >= 0.40);
        }
    }
}
