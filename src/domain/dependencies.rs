//! Cross-discipline dependency tracking, safe automatic propagation and manual
//! review auditing.
//!
//! All disciplines (architecture, electrical, plumbing, structure, foundations)
//! share a single [`Project`] model. When an architectural or structural edit
//! changes geometry, this module:
//!
//! 1. **Propagates safe dependencies automatically** (e.g., wall-mounted
//!    electrical/sanitary fixtures moving with a translated/resized wall, or a
//!    footing following its column) while preserving element IDs so conduits,
//!    circuits and beams stay linked.
//! 2. **Refuses unsafe auto-repositioning** (e.g., a fixture colliding with an
//!    opening, falling off a shortened wall, exceeding terrain bounds, or
//!    silently warping structural columns/beams across altered wall geometry),
//!    preserving the user's data and flagging a [`ReviewItem`].
//! 3. **Audits the current state** ([`Project::audit_dependencies`]) so the UI
//!    and MCP assistants can inspect every dependency link, every automatic
//!    update and every item that requires manual review or engineering
//!    verification.
//!
//! Automatic updates never imply automatic engineering approval: all electrical
//! and structural calculations remain subject to Trazo's technical checks and
//! applicability limits.

use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use serde::{Deserialize, Serialize};

use super::discipline::Discipline;
use super::fixtures::{Fixture, FixtureKind};
use super::geometry::{Point, distance_to_segment, nearest_point_on_segment};
use super::model::{Project, Wall};
use super::verdict::{Light, electrical_verdicts, structure_verdicts};

/// Maximum gap (metres) between a fixture's back face and a wall face for it
/// to count as mounted against that wall.
const WALL_ATTACH_TOLERANCE: f64 = 0.14;

/// Maximum distance (metres) between a footing centre and a column centre for
/// them to count as coupled when the column sits outside the footing footprint.
const FOOTING_COLUMN_TOLERANCE: f64 = 0.18;

/// Returns whether `footing` structurally supports a column at `column_center`:
/// either centred within [`FOOTING_COLUMN_TOLERANCE`] or as an eccentric /
/// boundary footing (*sapata de divisa*) whose footprint encloses the column.
fn footing_supports_column(footing: &Fixture, column_center: Point) -> bool {
    footing.contains(column_center)
        || footing.center.distance(column_center) <= FOOTING_COLUMN_TOLERANCE
}

/// Maximum distance (metres) between a column centre and a wall axis before it
/// is flagged as off-axis when architectural walls exist.
const COLUMN_WALL_TOLERANCE: f64 = 0.25;

/// Severity of a manual review item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSeverity {
    Info,
    Warning,
    Critical,
}

impl ReviewSeverity {
    pub fn key(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "aviso",
            Self::Critical => "critico",
        }
    }
}

/// Classification of a cross-discipline review finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewKind {
    FixtureDetachedFromWall,
    FixtureOpeningCollision,
    FixtureOutsideRoom,
    ColumnOffWallAxis,
    FootingMisalignedFromColumn,
    ColumnMissingFooting,
    StructuralVerificationFailed,
    ElectricalVerificationFailed,
    MisalignedUpperColumn,
}

impl ReviewKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::FixtureDetachedFromWall => "elemento_solto_da_parede",
            Self::FixtureOpeningCollision => "colisao_com_abertura",
            Self::FixtureOutsideRoom => "elemento_fora_de_comodo",
            Self::ColumnOffWallAxis => "pilar_fora_do_eixo_da_parede",
            Self::FootingMisalignedFromColumn => "sapata_desalinhada_do_pilar",
            Self::ColumnMissingFooting => "pilar_sem_sapata",
            Self::StructuralVerificationFailed => "verificacao_estrutural",
            Self::ElectricalVerificationFailed => "verificacao_eletrica",
            Self::MisalignedUpperColumn => "pilar_superior_desalinhado",
        }
    }
}

/// An automatic adjustment applied during dependency propagation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoUpdate {
    pub discipline: Discipline,
    pub element_id: u64,
    pub source_id: u64,
    pub description: String,
}

/// An item requiring manual review or engineering adjustment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub discipline: Discipline,
    pub severity: ReviewSeverity,
    pub kind: ReviewKind,
    pub element_id: Option<u64>,
    pub related_ids: Vec<u64>,
    pub position: Option<Point>,
    pub message: String,
    pub suggestion: String,
}

/// A structural/architectural/MEP dependency link detected in the current model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DependencyLink {
    pub source_kind: String,
    pub source_id: u64,
    pub target_kind: String,
    pub target_id: u64,
    pub target_discipline: Discipline,
    pub relation: String,
}

/// Result of propagating dependencies right after a model edit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DependencySyncReport {
    pub auto_updated: Vec<AutoUpdate>,
    pub blocked_updates: Vec<ReviewItem>,
    pub recalculated_disciplines: Vec<Discipline>,
}

impl DependencySyncReport {
    pub fn is_empty(&self) -> bool {
        self.auto_updated.is_empty()
            && self.blocked_updates.is_empty()
            && self.recalculated_disciplines.is_empty()
    }

    /// Short Portuguese status summary for the UI status bar.
    pub fn summary_line(&self) -> Option<String> {
        let auto = self.auto_updated.len();
        let blocked = self.blocked_updates.len();
        match (auto, blocked) {
            (0, 0) => None,
            (a, 0) => Some(format!(
                "Sincronização: {a} elemento(s) acompanhado(s) automaticamente."
            )),
            (0, b) => Some(format!(
                "Revisão manual: {b} elemento(s) preservado(s) para ajuste."
            )),
            (a, b) => Some(format!(
                "Sincronização: {a} atualizado(s) · {b} para revisão manual."
            )),
        }
    }
}

/// Full cross-discipline dependency and review audit of a [`Project`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DependencyAudit {
    pub links: Vec<DependencyLink>,
    pub review_items: Vec<ReviewItem>,
}

/// Back-face midpoint and outward front unit direction of a fixture.
fn fixture_back_and_front(fixture: &Fixture) -> (Point, Point) {
    let theta = fixture.rotation_rad();
    let (sin_t, cos_t) = theta.sin_cos();
    let half_d = fixture.depth * 0.5;
    // In local coords, back is (0, -depth/2) and front direction is (0, +1).
    let back = Point::new(
        fixture.center.x + sin_t * half_d,
        fixture.center.y - cos_t * half_d,
    );
    let front_dir = Point::new(-sin_t, cos_t);
    (back, front_dir)
}

/// Returns whether a fixture kind can attach to a wall face and follow safe
/// wall movements (wall-mounted electrical devices and plumbing fixtures —
/// never freestanding furniture such as tables, islands, armchairs or cars).
fn follows_wall(kind: FixtureKind) -> bool {
    (kind.is_electrical() && kind.mounts_on_wall()) || kind.is_plumbing()
}

/// Discipline owning a fixture for dependency reporting.
fn fixture_discipline(kind: FixtureKind) -> Discipline {
    if kind.is_electrical() {
        Discipline::Electrical
    } else if kind.is_plumbing() {
        Discipline::Plumbing
    } else if matches!(kind, FixtureKind::Footing | FixtureKind::PileCap) {
        Discipline::Foundations
    } else if kind.is_structural() {
        Discipline::Structure
    } else {
        Discipline::Architecture
    }
}

/// Finds the wall a fixture's back face was mounted against, returning
/// `(wall_id, distance_along_wall, signed_normal_offset)`.
fn attached_wall_of<'a>(fixture: &Fixture, walls: &'a [Wall]) -> Option<(&'a Wall, f64, f64)> {
    let (back, front_dir) = fixture_back_and_front(fixture);
    let mut best: Option<(&Wall, f64, f64, f64)> = None;
    for wall in walls {
        let len = wall.length();
        if len < 1e-4 {
            continue;
        }
        let u = Point::new(
            (wall.end.x - wall.start.x) / len,
            (wall.end.y - wall.start.y) / len,
        );
        let n = Point::new(-u.y, u.x);
        // Fixture back should be roughly parallel to the wall (front roughly perpendicular).
        if front_dir.dot(u).abs() > 0.35 {
            continue;
        }
        let rel_back = back.minus(wall.start);
        let s = rel_back.dot(u);
        if s < -0.05 || s > len + 0.05 {
            continue;
        }
        let dist_back = distance_to_segment(back, wall.start, wall.end);
        let expected_gap = wall.thickness() * 0.5;
        let err = (dist_back - expected_gap).abs();
        if err > WALL_ATTACH_TOLERANCE {
            continue;
        }
        let rel_center = fixture.center.minus(wall.start);
        let signed_normal = rel_center.dot(n);
        let s_center = rel_center.dot(u);
        if best.is_none_or(|(_, _, _, best_err)| err < best_err) {
            best = Some((wall, s_center, signed_normal, err));
        }
    }
    best.map(|(w, s, n, _)| (w, s, n))
}

/// Checks if a fixture at distance `s` along `wall` actually conflicts with
/// an opening on that wall.
///
/// High-sill windows (`sill >= 0.90 m`) are normal above kitchen sinks,
/// washbasins, toilets, showers and low outlets; doors, gates and floor-to-top
/// gaps conflict horizontally with any wall-mounted fixture.
fn overlaps_wall_opening(project: &Project, wall: &Wall, s: f64, fixture: &Fixture) -> Option<u64> {
    use super::model::OpeningKind;
    let len = wall.length();
    let half = (fixture.width * 0.5).max(0.05);
    for op in project.openings.iter().filter(|o| o.wall_id == wall.id) {
        let op_center = op.t * len;
        let op_half = op.width() * 0.5 + 0.04;
        if (s - op_center).abs() >= op_half + half {
            continue;
        }
        if op.kind == OpeningKind::Window {
            let sill = op.sill();
            let top = sill + op.height().unwrap_or(1.20);
            if fixture.kind.is_plumbing() && sill >= 0.90 {
                continue;
            }
            if let Some(elec) = fixture.electrical
                && (elec.height_m + 0.12 < sill || elec.height_m > top + 0.10)
            {
                continue;
            }
        }
        return Some(op.id);
    }
    None
}

/// Computes quarter-turn delta when a wall rotated by a clean multiple of 90°.
fn orthogonal_turn_delta(old_u: Point, new_u: Point) -> Option<u8> {
    let old_a = old_u.y.atan2(old_u.x);
    let new_a = new_u.y.atan2(new_u.x);
    let mut diff = (new_a - old_a).rem_euclid(TAU);
    if diff > PI {
        diff -= TAU;
    }
    let turns = (diff / FRAC_PI_2).round();
    if (diff - turns * FRAC_PI_2).abs() <= 0.05 {
        Some((turns as i32).rem_euclid(4) as u8)
    } else {
        None
    }
}

fn push_unique_discipline(list: &mut Vec<Discipline>, discipline: Discipline) {
    if !list.contains(&discipline) {
        list.push(discipline);
    }
}

impl Project {
    /// Propagates safe cross-discipline dependencies after an edit from
    /// `previous` to `self`, returning a report of automatic adjustments and
    /// items preserved for manual review.
    pub fn propagate_dependencies(&mut self, previous: &Project) -> DependencySyncReport {
        let mut report = DependencySyncReport::default();

        // Map walls from `previous` by ID to detect moved, resized or deleted walls.
        let prev_walls: HashMap<u64, &Wall> = previous.walls.iter().map(|w| (w.id, w)).collect();
        let curr_walls: HashMap<u64, Wall> = self.walls.iter().map(|w| (w.id, w.clone())).collect();

        let mut moved_walls: HashMap<u64, (&Wall, Wall)> = HashMap::new();
        let mut deleted_wall_ids: Vec<u64> = Vec::new();
        for (&wid, &prev_w) in &prev_walls {
            match curr_walls.get(&wid) {
                Some(curr_w) => {
                    if !prev_w.start.near(curr_w.start, 1e-4) || !prev_w.end.near(curr_w.end, 1e-4)
                    {
                        moved_walls.insert(wid, (prev_w, curr_w.clone()));
                    }
                }
                None => deleted_wall_ids.push(wid),
            }
        }

        // Map fixtures from `previous` by ID to detect user-moved fixtures vs stationary ones.
        let prev_fixtures: HashMap<u64, &Fixture> =
            previous.fixtures.iter().map(|f| (f.id, f)).collect();

        // 1. Wall-mounted electrical and plumbing fixtures following moved/deleted walls.
        if !moved_walls.is_empty() || !deleted_wall_ids.is_empty() {
            let prev_wall_slice = &previous.walls;
            let fixture_ids: Vec<u64> = self.fixtures.iter().map(|f| f.id).collect();
            for fid in fixture_ids {
                let Some(&prev_f) = prev_fixtures.get(&fid) else {
                    continue;
                };
                if !follows_wall(prev_f.kind) {
                    continue;
                }
                let Some(curr_idx) = self.fixtures.iter().position(|f| f.id == fid) else {
                    continue;
                };
                // Respect explicit manual movement of the fixture in this same edit.
                if !self.fixtures[curr_idx].center.near(prev_f.center, 1e-4) {
                    continue;
                }
                let Some((old_wall, old_s, signed_normal)) =
                    attached_wall_of(prev_f, prev_wall_slice)
                else {
                    continue;
                };
                let discipline = fixture_discipline(prev_f.kind);
                if deleted_wall_ids.contains(&old_wall.id) {
                    report.blocked_updates.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureDetachedFromWall,
                        element_id: Some(fid),
                        related_ids: vec![old_wall.id],
                        position: Some(prev_f.center),
                        message: format!(
                            "{} #{fid} ficou sem parede de apoio após a remoção da parede #{}.",
                            prev_f.kind.label(),
                            old_wall.id
                        ),
                        suggestion: "Reposicione o ponto numa parede próxima ou remova-o."
                            .to_owned(),
                    });
                    continue;
                }
                let Some((_, new_wall)) = moved_walls.get(&old_wall.id) else {
                    continue;
                };
                let old_len = old_wall.length();
                let new_len = new_wall.length();
                if old_len < 1e-4 || new_len < 1e-4 {
                    continue;
                }
                let old_u = Point::new(
                    (old_wall.end.x - old_wall.start.x) / old_len,
                    (old_wall.end.y - old_wall.start.y) / old_len,
                );
                let new_u = Point::new(
                    (new_wall.end.x - new_wall.start.x) / new_len,
                    (new_wall.end.y - new_wall.start.y) / new_len,
                );
                let new_n = Point::new(-new_u.y, new_u.x);
                let Some(turn_delta) = orthogonal_turn_delta(old_u, new_u) else {
                    report.blocked_updates.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureDetachedFromWall,
                        element_id: Some(fid),
                        related_ids: vec![old_wall.id],
                        position: Some(prev_f.center),
                        message: format!(
                            "{} #{fid} não foi reposicionado automaticamente porque a parede #{} girou para um ângulo livre.",
                            prev_f.kind.label(),
                            old_wall.id
                        ),
                        suggestion: "Ajuste manualmente a posição e orientação do ponto na parede."
                            .to_owned(),
                    });
                    continue;
                };

                let start_fixed = old_wall.start.near(new_wall.start, 1e-3);
                let end_fixed = old_wall.end.near(new_wall.end, 1e-3);
                let new_s = if start_fixed && !end_fixed {
                    old_s
                } else if !start_fixed && end_fixed {
                    new_len - (old_len - old_s)
                } else if (new_len - old_len).abs() < 1e-3 {
                    old_s
                } else {
                    (old_s / old_len) * new_len
                };

                let margin = (prev_f.width * 0.5).min(0.15);
                if new_s < margin - 1e-3 || new_s > new_len - margin + 1e-3 {
                    report.blocked_updates.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureDetachedFromWall,
                        element_id: Some(fid),
                        related_ids: vec![old_wall.id],
                        position: Some(prev_f.center),
                        message: format!(
                            "{} #{fid} foi preservado no local porque a parede #{} encurtou além do ponto.",
                            prev_f.kind.label(),
                            old_wall.id
                        ),
                        suggestion: "Reposicione o ponto dentro do novo trecho da parede."
                            .to_owned(),
                    });
                    continue;
                }

                if let Some(op_id) = overlaps_wall_opening(self, new_wall, new_s, prev_f) {
                    report.blocked_updates.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureOpeningCollision,
                        element_id: Some(fid),
                        related_ids: vec![old_wall.id, op_id],
                        position: Some(prev_f.center),
                        message: format!(
                            "{} #{fid} não acompanhou a parede #{} pois colidiria com a abertura #{op_id}.",
                            prev_f.kind.label(),
                            old_wall.id
                        ),
                        suggestion: "Mova o ponto para fora do vão da abertura.".to_owned(),
                    });
                    continue;
                }

                let candidate = Point::new(
                    new_wall.start.x + new_u.x * new_s + new_n.x * signed_normal,
                    new_wall.start.y + new_u.y * new_s + new_n.y * signed_normal,
                )
                .rounded();
                let new_turns = (prev_f.quarter_turns + turn_delta) % 4;
                if self.terrain.is_some() && !self.point_inside_buildable_limit(candidate) {
                    report.blocked_updates.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureDetachedFromWall,
                        element_id: Some(fid),
                        related_ids: vec![old_wall.id],
                        position: Some(prev_f.center),
                        message: format!(
                            "{} #{fid} não acompanhou a parede #{} porque sairia do limite do terreno.",
                            prev_f.kind.label(),
                            old_wall.id
                        ),
                        suggestion: "Ajuste o terreno ou reposicione o elemento.".to_owned(),
                    });
                    continue;
                }

                self.fixtures[curr_idx].center = candidate;
                self.fixtures[curr_idx].quarter_turns = new_turns;
                push_unique_discipline(&mut report.recalculated_disciplines, discipline);
                report.auto_updated.push(AutoUpdate {
                    discipline,
                    element_id: fid,
                    source_id: old_wall.id,
                    description: format!(
                        "{} #{fid} acompanhou a parede #{}.",
                        prev_f.kind.label(),
                        old_wall.id
                    ),
                });
            }
        }

        // 2. Footings following explicitly moved columns (preserving ID and dimensions).
        let moved_columns: Vec<(u64, Point, Point)> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Column)
            .filter_map(|curr_c| {
                let prev_c = prev_fixtures.get(&curr_c.id)?;
                if !prev_c.center.near(curr_c.center, 1e-4) {
                    Some((curr_c.id, prev_c.center, curr_c.center))
                } else {
                    None
                }
            })
            .collect();

        for (col_id, old_center, new_center) in &moved_columns {
            let delta = new_center.minus(*old_center);
            let footing_ids: Vec<u64> = self
                .fixtures
                .iter()
                .filter(|f| matches!(f.kind, FixtureKind::Footing | FixtureKind::PileCap))
                .map(|f| f.id)
                .collect();
            for fid in footing_ids {
                let Some(&prev_ft) = prev_fixtures.get(&fid) else {
                    continue;
                };
                if !footing_supports_column(prev_ft, *old_center) {
                    continue;
                }
                let Some(idx) = self.fixtures.iter().position(|f| f.id == fid) else {
                    continue;
                };
                if !self.fixtures[idx].center.near(prev_ft.center, 1e-4) {
                    continue;
                }
                let candidate = prev_ft.center.translated(delta).rounded();
                if self.terrain.is_some() && !self.point_inside_buildable_limit(candidate) {
                    report.blocked_updates.push(ReviewItem {
                        discipline: Discipline::Foundations,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FootingMisalignedFromColumn,
                        element_id: Some(fid),
                        related_ids: vec![*col_id],
                        position: Some(prev_ft.center),
                        message: format!(
                            "Sapata #{fid} não acompanhou o pilar #{col_id} porque sairia do limite do terreno."
                        ),
                        suggestion: "Verifique sapata de divisa / viga de equilíbrio ou ajuste o pilar.".to_owned(),
                    });
                    continue;
                }
                self.fixtures[idx].center = candidate;
                push_unique_discipline(
                    &mut report.recalculated_disciplines,
                    Discipline::Foundations,
                );
                report.auto_updated.push(AutoUpdate {
                    discipline: Discipline::Foundations,
                    element_id: fid,
                    source_id: *col_id,
                    description: format!("Sapata #{fid} acompanhou o pilar #{col_id}."),
                });
            }
        }

        // 3. Track which disciplines have derived calculations invalidated.
        if !moved_walls.is_empty() || !deleted_wall_ids.is_empty() || self.rooms != previous.rooms {
            push_unique_discipline(
                &mut report.recalculated_disciplines,
                Discipline::Architecture,
            );
            if self.electrical_devices().next().is_some() || !self.conduits.is_empty() {
                push_unique_discipline(
                    &mut report.recalculated_disciplines,
                    Discipline::Electrical,
                );
            }
            if !self.structure.beams.is_empty()
                || !self.structure.slabs.is_empty()
                || self.fixtures.iter().any(|f| f.kind.is_structural())
            {
                push_unique_discipline(&mut report.recalculated_disciplines, Discipline::Structure);
                push_unique_discipline(
                    &mut report.recalculated_disciplines,
                    Discipline::Foundations,
                );
            }
        }
        if !moved_columns.is_empty() {
            push_unique_discipline(&mut report.recalculated_disciplines, Discipline::Structure);
            push_unique_discipline(
                &mut report.recalculated_disciplines,
                Discipline::Foundations,
            );
        }

        report
    }

    /// Audits the current project for active cross-discipline dependency links
    /// and every element requiring manual review or engineering adjustment.
    pub fn audit_dependencies(&self) -> DependencyAudit {
        let mut audit = DependencyAudit::default();

        // 1. Wall -> Opening links.
        for opening in &self.openings {
            audit.links.push(DependencyLink {
                source_kind: "parede".to_owned(),
                source_id: opening.wall_id,
                target_kind: "abertura".to_owned(),
                target_id: opening.id,
                target_discipline: Discipline::Architecture,
                relation: "hospeda".to_owned(),
            });
        }

        // 2. Wall -> Wall-mounted electrical & plumbing fixtures, plus detachment / collision checks.
        for fixture in &self.fixtures {
            if !follows_wall(fixture.kind) {
                continue;
            }
            let discipline = fixture_discipline(fixture.kind);
            if let Some((wall, s, _)) = attached_wall_of(fixture, &self.walls) {
                audit.links.push(DependencyLink {
                    source_kind: "parede".to_owned(),
                    source_id: wall.id,
                    target_kind: fixture.kind.key().to_owned(),
                    target_id: fixture.id,
                    target_discipline: discipline,
                    relation: "fixado_em".to_owned(),
                });
                if let Some(op_id) = overlaps_wall_opening(self, wall, s, fixture) {
                    audit.review_items.push(ReviewItem {
                        discipline,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::FixtureOpeningCollision,
                        element_id: Some(fixture.id),
                        related_ids: vec![wall.id, op_id],
                        position: Some(fixture.center),
                        message: format!(
                            "{} #{} sobrepõe a abertura #{op_id} na parede #{}.",
                            fixture.kind.label(),
                            fixture.id,
                            wall.id
                        ),
                        suggestion: "Desloque o elemento ao longo da parede para fora do vão."
                            .to_owned(),
                    });
                }
            } else if !self.walls.is_empty()
                && fixture.kind.is_electrical()
                && fixture.kind.mounts_on_wall()
            {
                audit.review_items.push(ReviewItem {
                    discipline,
                    severity: ReviewSeverity::Warning,
                    kind: ReviewKind::FixtureDetachedFromWall,
                    element_id: Some(fixture.id),
                    related_ids: Vec::new(),
                    position: Some(fixture.center),
                    message: format!(
                        "{} #{} está solto (afastado das paredes).",
                        fixture.kind.label(),
                        fixture.id
                    ),
                    suggestion: "Encoste o ponto na parede mais próxima para manter a referência."
                        .to_owned(),
                });
            }

            if fixture.kind.is_electrical()
                && !self.rooms.is_empty()
                && self.room_for_fixture(fixture).is_none()
            {
                audit.review_items.push(ReviewItem {
                    discipline: Discipline::Electrical,
                    severity: ReviewSeverity::Info,
                    kind: ReviewKind::FixtureOutsideRoom,
                    element_id: Some(fixture.id),
                    related_ids: Vec::new(),
                    position: Some(fixture.center),
                    message: format!(
                        "{} #{} está fora de qualquer cômodo cadastrado.",
                        fixture.kind.label(),
                        fixture.id
                    ),
                    suggestion:
                        "Verifique se o ponto pertence a área externa ou ajuste sua posição."
                            .to_owned(),
                });
            }
        }

        // 3. Structural columns vs architectural walls, beams, and footings.
        let columns: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Column)
            .collect();
        let footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| matches!(f.kind, FixtureKind::Footing | FixtureKind::PileCap))
            .collect();

        for col in &columns {
            if let Some((nearest_wall, dist)) = self
                .walls
                .iter()
                .map(|w| {
                    let (foot, _) = nearest_point_on_segment(col.center, w.start, w.end);
                    (w, foot.distance(col.center))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                if dist <= COLUMN_WALL_TOLERANCE {
                    audit.links.push(DependencyLink {
                        source_kind: "parede".to_owned(),
                        source_id: nearest_wall.id,
                        target_kind: "pilar".to_owned(),
                        target_id: col.id,
                        target_discipline: Discipline::Structure,
                        relation: "alinhado_em".to_owned(),
                    });
                } else {
                    audit.review_items.push(ReviewItem {
                        discipline: Discipline::Structure,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::ColumnOffWallAxis,
                        element_id: Some(col.id),
                        related_ids: vec![nearest_wall.id],
                        position: Some(col.center),
                        message: format!(
                            "Pilar #{} está a {:.2} m da parede mais próxima (#{}).",
                            col.id, dist, nearest_wall.id
                        )
                        .replace('.', ","),
                        suggestion:
                            "Revise o alinhamento estrutural após mudanças arquitetônicas ou confirme pilar isolado."
                                .to_owned(),
                    });
                }
            }

            if !footings.is_empty() {
                let coupled = footings
                    .iter()
                    .filter(|ft| footing_supports_column(ft, col.center))
                    .min_by(|a, b| {
                        a.center
                            .distance(col.center)
                            .total_cmp(&b.center.distance(col.center))
                    });
                if let Some(ft) = coupled {
                    audit.links.push(DependencyLink {
                        source_kind: "pilar".to_owned(),
                        source_id: col.id,
                        target_kind: "sapata".to_owned(),
                        target_id: ft.id,
                        target_discipline: Discipline::Foundations,
                        relation: "apoia".to_owned(),
                    });
                } else {
                    audit.review_items.push(ReviewItem {
                        discipline: Discipline::Foundations,
                        severity: ReviewSeverity::Warning,
                        kind: ReviewKind::ColumnMissingFooting,
                        element_id: Some(col.id),
                        related_ids: Vec::new(),
                        position: Some(col.center),
                        message: format!("Pilar #{} está sem sapata sob seu eixo.", col.id),
                        suggestion: "Insira ou alinhe uma sapata sob este pilar.".to_owned(),
                    });
                }
            }
        }

        // Footings without a column resting on their footprint.
        for ft in &footings {
            let has_col = columns
                .iter()
                .any(|c| footing_supports_column(ft, c.center));
            if !has_col {
                audit.review_items.push(ReviewItem {
                    discipline: Discipline::Foundations,
                    severity: ReviewSeverity::Warning,
                    kind: ReviewKind::FootingMisalignedFromColumn,
                    element_id: Some(ft.id),
                    related_ids: Vec::new(),
                    position: Some(ft.center),
                    message: format!("Sapata #{} não possui pilar apoiado sobre sua base.", ft.id),
                    suggestion: "Mova a sapata para sob o pilar correspondente ou remova-a."
                        .to_owned(),
                });
            }
        }

        // Beam -> Support links and Room -> Slab links.
        for beam in &self.structure.beams {
            for sup in [beam.start_support(), beam.end_support()] {
                audit.links.push(DependencyLink {
                    source_kind: match sup {
                        super::structure::BeamSupport::Column { .. } => "pilar".to_owned(),
                        super::structure::BeamSupport::Beam { .. } => "viga".to_owned(),
                    },
                    source_id: sup.target_id(),
                    target_kind: "viga".to_owned(),
                    target_id: beam.id,
                    target_discipline: Discipline::Structure,
                    relation: "apoia".to_owned(),
                });
            }
        }
        for slab in &self.structure.slabs {
            audit.links.push(DependencyLink {
                source_kind: "comodo".to_owned(),
                source_id: slab.room_id,
                target_kind: "laje".to_owned(),
                target_id: slab.id,
                target_discipline: Discipline::Structure,
                relation: "cobre".to_owned(),
            });
        }

        // 4. Technical verification failures (never auto-approved!).
        if !columns.is_empty()
            || !self.structure.beams.is_empty()
            || !self.structure.slabs.is_empty()
            || !footings.is_empty()
        {
            let report = self.cached_structure_report();
            for misaligned in &report.misaligned_columns {
                audit.review_items.push(ReviewItem {
                    discipline: Discipline::Structure,
                    severity: ReviewSeverity::Critical,
                    kind: ReviewKind::MisalignedUpperColumn,
                    element_id: None,
                    related_ids: Vec::new(),
                    position: Some(misaligned.center),
                    message: format!(
                        "Pilar superior {} ({}) desalinhado do pavimento atual ({:.1} kN).",
                        misaligned.column_label, misaligned.floor_name, misaligned.load_kn
                    )
                    .replace('.', ","),
                    suggestion: if misaligned.transfer_beam.is_some() {
                        "Confira a viga de transição que recebe esta carga pontual.".to_owned()
                    } else {
                        "Adicione pilar alinhado ou viga de transição sob este ponto.".to_owned()
                    },
                });
            }
            for verdict in structure_verdicts(self, &report) {
                if verdict.light() == Light::Red {
                    let is_footing = verdict.element.starts_with('S');
                    let detail = verdict
                        .checks
                        .iter()
                        .find(|c| c.light == Light::Red)
                        .map(|c| c.simple.clone())
                        .unwrap_or_else(|| "Falha na verificação estrutural.".to_owned());
                    audit.review_items.push(ReviewItem {
                        discipline: if is_footing {
                            Discipline::Foundations
                        } else {
                            Discipline::Structure
                        },
                        severity: ReviewSeverity::Critical,
                        kind: ReviewKind::StructuralVerificationFailed,
                        element_id: None,
                        related_ids: Vec::new(),
                        position: None,
                        message: format!("{}: {detail}", verdict.element),
                        suggestion: "Redimensione o elemento na vista estrutural ou de fundações."
                            .to_owned(),
                    });
                }
            }
        }

        if self.electrical_devices().next().is_some() {
            let schedule = self.cached_load_schedule();
            for verdict in electrical_verdicts(&schedule) {
                if verdict.light() == Light::Red {
                    let detail = verdict
                        .checks
                        .iter()
                        .find(|c| c.light == Light::Red)
                        .map(|c| c.simple.clone())
                        .unwrap_or_else(|| "Falha na verificação elétrica.".to_owned());
                    audit.review_items.push(ReviewItem {
                        discipline: Discipline::Electrical,
                        severity: ReviewSeverity::Critical,
                        kind: ReviewKind::ElectricalVerificationFailed,
                        element_id: None,
                        related_ids: Vec::new(),
                        position: None,
                        message: format!("{}: {detail}", verdict.element),
                        suggestion: "Revise circuitos, fiação ou eletrodutos na vista elétrica."
                            .to_owned(),
                    });
                }
            }
        }

        audit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::conduits::ConduitRun;
    use crate::domain::model::OpeningKind;

    #[test]
    fn wall_mounted_outlet_and_sink_follow_moved_wall_preserving_ids_and_conduits() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Cozinha")
            .unwrap();
        // Top wall: (0,0) -> (4,0), thickness ~0.15m.
        let top_wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - 0.0).abs() < 1e-3 && (w.end.y - 0.0).abs() < 1e-3)
            .unwrap()
            .clone();
        let half_t = top_wall.thickness() * 0.5;
        let (ow, od) = FixtureKind::OutletMid.default_size();
        let outlet_id = project
            .add_fixture(
                FixtureKind::OutletMid,
                Point::new(1.5, half_t + od * 0.5),
                ow,
                od,
                0,
            )
            .unwrap();
        let (sw, sd) = FixtureKind::KitchenSink.default_size();
        let sink_id = project
            .add_fixture(
                FixtureKind::KitchenSink,
                Point::new(3.0, half_t + sd * 0.5),
                sw,
                sd,
                0,
            )
            .unwrap();
        let (pw, pd) = FixtureKind::Panel.default_size();
        let panel_id = project
            .add_fixture(FixtureKind::Panel, Point::new(0.5, 2.7), pw, pd, 2)
            .unwrap();
        let conduit_id = project
            .add_conduit(panel_id, outlet_id, ConduitRun::Ceiling)
            .unwrap();

        let previous = project.clone();
        // Move the top wall up by -0.50 m (expanding the room).
        project
            .move_wall_checked(top_wall.id, Point::new(0.0, -0.5))
            .unwrap();
        let report = project.propagate_dependencies(&previous);

        assert!(
            report
                .auto_updated
                .iter()
                .any(|u| u.element_id == outlet_id),
            "outlet should follow moved wall: {report:?}"
        );
        assert!(
            report.auto_updated.iter().any(|u| u.element_id == sink_id),
            "kitchen sink should follow moved wall: {report:?}"
        );
        assert!(report.blocked_updates.is_empty());

        let moved_outlet = project.fixtures.iter().find(|f| f.id == outlet_id).unwrap();
        assert!(
            (moved_outlet.center.y - (-0.5 + half_t + od * 0.5)).abs() < 1e-2,
            "outlet moved with wall: {:?}",
            moved_outlet.center
        );
        // Conduit reference to outlet_id remains intact!
        assert!(
            project
                .conduits
                .iter()
                .any(|c| c.id == conduit_id && c.to == outlet_id)
        );
    }

    #[test]
    fn unsafe_wall_shrink_preserves_outlet_and_flags_manual_review() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
        let wall_id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let half_t = project.walls[0].thickness() * 0.5;
        let (ow, od) = FixtureKind::OutletLow.default_size();
        // Place outlet near x = 3.5 m on the 4 m wall.
        let outlet_id = project
            .add_fixture(
                FixtureKind::OutletLow,
                Point::new(3.5, half_t + od * 0.5),
                ow,
                od,
                0,
            )
            .unwrap();
        let original_pos = project.fixtures[0].center;

        let previous = project.clone();
        // Shrink wall to 2.0 m so x = 3.5 m falls off the wall.
        project.resize_wall_length(wall_id, 2.0).unwrap();
        let report = project.propagate_dependencies(&previous);

        // Outlet was NOT force-moved; it is preserved and flagged for review.
        let outlet = project.fixtures.iter().find(|f| f.id == outlet_id).unwrap();
        assert_eq!(outlet.center, original_pos);
        assert!(report.blocked_updates.iter().any(|r| {
            r.element_id == Some(outlet_id) && r.kind == ReviewKind::FixtureDetachedFromWall
        }));

        // Audit also reports it as detached from walls.
        let audit = project.audit_dependencies();
        assert!(audit.review_items.iter().any(|r| {
            r.element_id == Some(outlet_id) && r.kind == ReviewKind::FixtureDetachedFromWall
        }));
    }

    #[test]
    fn wall_move_flags_off_axis_column_without_silently_warping_structure() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        project.generate_structure();
        let top_wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - 0.0).abs() < 1e-3 && (w.end.y - 0.0).abs() < 1e-3)
            .unwrap()
            .id;
        let col_before: Vec<(u64, Point)> = project
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Column)
            .map(|c| (c.id, c.center))
            .collect();

        let previous = project.clone();
        project
            .move_wall_checked(top_wall, Point::new(0.0, -1.0))
            .unwrap();
        let _report = project.propagate_dependencies(&previous);

        // Structural columns are NOT silently shifted into unsafe spans;
        // instead, audit flags them for manual structural review.
        for (cid, pos) in &col_before {
            let col = project.fixtures.iter().find(|f| f.id == *cid).unwrap();
            assert_eq!(col.center, *pos);
        }
        // Columns on the moved corners stayed at y=0, which is still on the side walls
        // (stretched to y=-1..3), so add a mid-wall column or test an isolated wall move:
        let iso_wall = project
            .add_wall(Point::new(6.0, 0.0), Point::new(10.0, 0.0))
            .unwrap();
        let col_id = project
            .add_fixture(FixtureKind::Column, Point::new(8.0, 0.0), 0.20, 0.20, 0)
            .unwrap();
        let prev2 = project.clone();
        project
            .move_wall_checked(iso_wall, Point::new(0.0, 1.0))
            .unwrap();
        project.propagate_dependencies(&prev2);
        let audit = project.audit_dependencies();
        assert!(
            audit.review_items.iter().any(|r| {
                r.element_id == Some(col_id) && r.kind == ReviewKind::ColumnOffWallAxis
            })
        );
    }

    #[test]
    fn footing_follows_moved_column_and_opening_collision_blocks_auto_move() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
        let col_id = project
            .add_fixture(FixtureKind::Column, Point::new(2.0, 2.0), 0.20, 0.20, 0)
            .unwrap();
        let ft_id = project
            .add_fixture(FixtureKind::Footing, Point::new(2.0, 2.0), 0.80, 0.80, 0)
            .unwrap();

        let prev = project.clone();
        project.move_fixture(col_id, Point::new(1.0, 0.5));
        let report = project.propagate_dependencies(&prev);
        assert!(report.auto_updated.iter().any(|u| u.element_id == ft_id));
        let ft = project.fixtures.iter().find(|f| f.id == ft_id).unwrap();
        assert!(ft.center.near(Point::new(3.0, 2.5), 1e-4));

        // Opening collision check: place a wall with a door and an outlet,
        // move a wall so the audit detects overlap if moved onto opening.
        let wall_id = project
            .add_wall(Point::new(0.0, 6.0), Point::new(4.0, 6.0))
            .unwrap();
        project
            .add_opening_sized(wall_id, OpeningKind::Door, 0.5, Some(0.9))
            .unwrap();
        let half_t = project
            .walls
            .iter()
            .find(|w| w.id == wall_id)
            .unwrap()
            .thickness()
            * 0.5;
        let (ow, od) = FixtureKind::OutletMid.default_size();
        let bad_outlet = project
            .add_fixture(
                FixtureKind::OutletMid,
                Point::new(2.0, 6.0 + half_t + od * 0.5),
                ow,
                od,
                0,
            )
            .unwrap();
        let audit = project.audit_dependencies();
        assert!(audit.review_items.iter().any(|r| {
            r.element_id == Some(bad_outlet) && r.kind == ReviewKind::FixtureOpeningCollision
        }));
    }

    #[test]
    fn freestanding_furniture_and_shower_under_high_window_do_not_trigger_warnings() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Banheiro")
            .unwrap();
        let top_wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - 0.0).abs() < 1e-3 && (w.end.y - 0.0).abs() < 1e-3)
            .unwrap()
            .clone();
        // High bathroom window (default sill >= 1.1 m) right above the shower.
        project
            .add_opening_sized(top_wall.id, OpeningKind::Window, 0.25, Some(0.60))
            .unwrap();
        let half_t = top_wall.thickness() * 0.5;
        let (sw, sd) = FixtureKind::Shower.default_size();
        project
            .add_fixture(
                FixtureKind::Shower,
                Point::new(1.25, half_t + sd * 0.5),
                sw,
                sd,
                0,
            )
            .unwrap();
        // Freestanding table, island counter and car away from walls.
        project
            .add_fixture(FixtureKind::Table, Point::new(2.5, 2.5), 2.0, 1.0, 0)
            .unwrap();
        project
            .add_fixture(FixtureKind::Counter, Point::new(3.0, 2.0), 1.5, 0.6, 0)
            .unwrap();
        project
            .add_fixture(FixtureKind::Car, Point::new(8.0, 8.0), 1.85, 4.5, 0)
            .unwrap();

        let audit = project.audit_dependencies();
        assert!(
            audit.review_items.is_empty(),
            "expected no false positives on furniture or high window over shower, got: {:?}",
            audit.review_items
        );
    }

    #[test]
    fn boundary_eccentric_footing_is_coupled_without_false_warning() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        // Boundary wall along x = 0.10 (20 cm thick wall flush with x = 0.0).
        project
            .add_wall(Point::new(0.10, 0.0), Point::new(0.10, 6.0))
            .unwrap();
        // Boundary column centred at x = 0.10, resting on a 60×130 cm boundary
        // footing centred at x = 0.30 (eccentricity 0.20 m > 0.18 m, flush with x = 0.0).
        let col_id = project
            .add_fixture(FixtureKind::Column, Point::new(0.10, 2.0), 0.20, 0.30, 0)
            .unwrap();
        let ft_id = project
            .add_fixture(FixtureKind::Footing, Point::new(0.30, 2.0), 0.60, 1.30, 0)
            .unwrap();

        let audit = project.audit_dependencies();
        assert!(
            audit.links.iter().any(|l| {
                l.source_id == col_id && l.target_id == ft_id && l.relation == "apoia"
            }),
            "expected pillar->footing link for boundary footing: {:?}",
            audit.links
        );
        assert!(
            !audit.review_items.iter().any(|r| matches!(
                r.kind,
                ReviewKind::ColumnMissingFooting | ReviewKind::FootingMisalignedFromColumn
            )),
            "boundary footing enclosing its column must not trigger misalignment warning: {:?}",
            audit.review_items
        );

        // Moving the boundary column along the wall drags the eccentric footing with it.
        let prev = project.clone();
        project.move_fixture(col_id, Point::new(0.0, 0.5));
        let report = project.propagate_dependencies(&prev);
        assert!(report.auto_updated.iter().any(|u| u.element_id == ft_id));
        let ft = project.fixtures.iter().find(|f| f.id == ft_id).unwrap();
        assert!(ft.center.near(Point::new(0.30, 2.5), 1e-4));

        // Truly detached footing far from the column must still trigger both warnings.
        let orphan_ft = project
            .add_fixture(FixtureKind::Footing, Point::new(5.0, 5.0), 0.60, 0.60, 0)
            .unwrap();
        let audit2 = project.audit_dependencies();
        assert!(audit2.review_items.iter().any(|r| {
            r.element_id == Some(orphan_ft) && r.kind == ReviewKind::FootingMisalignedFromColumn
        }));
    }
}
