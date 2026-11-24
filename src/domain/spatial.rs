//! Uniform XY grid spatial index and incremental validation (T-05, #62).
//!
//! Tiles plan elements (`Wall`, `Opening`, `Fixture`, `Drawing`, `Room`) into
//! 4 m × 4 m grid cells so picking, object snapping (OSNAP), dependent-element
//! lookup and incremental validation query only local candidates instead of
//! scanning the entire building.

use std::collections::{HashMap, HashSet};

use super::{
    element::Element,
    geometry::{JOIN_TOLERANCE, Point},
    model::{ChangeSet, Project, Wall},
};

/// Grid cell width in metres (4 m balances residential rooms and multi-storey plans).
const CELL_SIZE_M: f64 = 4.0;
/// Maximum tiles a single oversized element may populate before going into `everywhere`.
const MAX_TILES_PER_ITEM: i64 = 1024;
/// Global safety cap on total cell references to guard against hostile coordinates.
const MAX_TOTAL_REFS: usize = 500_000;

#[inline]
fn cell_coord(value: f64) -> Option<i64> {
    if !value.is_finite() || value.abs() > 1e9 {
        return None;
    }
    Some((value / CELL_SIZE_M).floor() as i64)
}

fn cell_range(min: Point, max: Point) -> Option<((i64, i64), (i64, i64))> {
    let x0 = cell_coord(min.x.min(max.x))?;
    let y0 = cell_coord(min.y.min(max.y))?;
    let x1 = cell_coord(min.x.max(max.x))?;
    let y1 = cell_coord(min.y.max(max.y))?;
    let span_x = x1.saturating_sub(x0).saturating_add(1);
    let span_y = y1.saturating_sub(y0).saturating_add(1);
    if span_x.saturating_mul(span_y) > MAX_TILES_PER_ITEM {
        return None;
    }
    Some(((x0, y0), (x1, y1)))
}

fn insert_box(
    tiles: &mut HashMap<(i64, i64), Vec<usize>>,
    everywhere: &mut Vec<usize>,
    total_refs: &mut usize,
    index: usize,
    min: Point,
    max: Point,
) {
    if *total_refs < MAX_TOTAL_REFS
        && let Some(((x0, y0), (x1, y1))) = cell_range(min, max)
    {
        for gx in x0..=x1 {
            for gy in y0..=y1 {
                tiles.entry((gx, gy)).or_default().push(index);
                *total_refs += 1;
            }
        }
    } else {
        everywhere.push(index);
    }
}

fn query_box(
    tiles: &HashMap<(i64, i64), Vec<usize>>,
    everywhere: &[usize],
    min: Point,
    max: Point,
) -> Vec<usize> {
    let mut out: Vec<usize> = everywhere.to_vec();
    if let Some(((x0, y0), (x1, y1))) = cell_range(min, max) {
        for gx in x0..=x1 {
            for gy in y0..=y1 {
                if let Some(bucket) = tiles.get(&(gx, gy)) {
                    out.extend_from_slice(bucket);
                }
            }
        }
    } else {
        for bucket in tiles.values() {
            out.extend_from_slice(bucket);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Read-only XY spatial grid index derived from one [`Project`] state and cached
/// in [`crate::domain::cache`] by geometry fingerprint.
#[derive(Debug, Default)]
pub struct SpatialIndex {
    wall_tiles: HashMap<(i64, i64), Vec<usize>>,
    walls_everywhere: Vec<usize>,
    fixture_tiles: HashMap<(i64, i64), Vec<usize>>,
    fixtures_everywhere: Vec<usize>,
    drawing_tiles: HashMap<(i64, i64), Vec<usize>>,
    drawings_everywhere: Vec<usize>,
    room_tiles: HashMap<(i64, i64), Vec<usize>>,
    rooms_everywhere: Vec<usize>,
    wall_openings: HashMap<u64, Vec<usize>>,
    wall_rooms: HashMap<u64, Vec<usize>>,
    wall_index_by_id: HashMap<u64, usize>,
}

impl SpatialIndex {
    /// Builds a fresh spatial index over `project`.
    pub fn build(project: &Project) -> Self {
        let mut index = Self::default();
        let mut total_refs = 0usize;

        for (i, wall) in project.walls.iter().enumerate() {
            index.wall_index_by_id.insert(wall.id, i);
            let pad = wall.thickness() * 0.5 + JOIN_TOLERANCE;
            let min = Point::new(
                wall.start.x.min(wall.end.x) - pad,
                wall.start.y.min(wall.end.y) - pad,
            );
            let max = Point::new(
                wall.start.x.max(wall.end.x) + pad,
                wall.start.y.max(wall.end.y) + pad,
            );
            insert_box(
                &mut index.wall_tiles,
                &mut index.walls_everywhere,
                &mut total_refs,
                i,
                min,
                max,
            );
        }

        for (i, opening) in project.openings.iter().enumerate() {
            index
                .wall_openings
                .entry(opening.wall_id)
                .or_default()
                .push(i);
        }

        for (i, fixture) in project.fixtures.iter().enumerate() {
            let (min, max) = fixture.bounds();
            insert_box(
                &mut index.fixture_tiles,
                &mut index.fixtures_everywhere,
                &mut total_refs,
                i,
                min,
                max,
            );
        }

        for (i, drawing) in project.drawings.iter().enumerate() {
            let (min, max) = drawing.shape.bounds();
            insert_box(
                &mut index.drawing_tiles,
                &mut index.drawings_everywhere,
                &mut total_refs,
                i,
                min,
                max,
            );
        }

        for (i, room) in project.rooms.iter().enumerate() {
            for &wid in &room.wall_ids {
                index.wall_rooms.entry(wid).or_default().push(i);
            }
            if let Some((min, max)) = project.room_bounds(room) {
                insert_box(
                    &mut index.room_tiles,
                    &mut index.rooms_everywhere,
                    &mut total_refs,
                    i,
                    min,
                    max,
                );
            } else {
                index.rooms_everywhere.push(i);
            }
        }

        index
    }

    /// Wall indices whose bounding boxes intersect `[min, max]`, sorted & deduplicated.
    pub fn wall_candidates_in_box(&self, min: Point, max: Point) -> Vec<usize> {
        query_box(&self.wall_tiles, &self.walls_everywhere, min, max)
    }

    /// Wall indices within `radius` metres of `point`, sorted & deduplicated.
    pub fn wall_candidates_near(&self, point: Point, radius: f64) -> Vec<usize> {
        let r = radius.abs();
        self.wall_candidates_in_box(
            Point::new(point.x - r, point.y - r),
            Point::new(point.x + r, point.y + r),
        )
    }

    /// Opening indices hosted on walls within `radius` metres of `point`.
    #[allow(dead_code)]
    pub fn opening_candidates_near(&self, point: Point, radius: f64, walls: &[Wall]) -> Vec<usize> {
        let mut out = Vec::new();
        for wall_idx in self.wall_candidates_near(point, radius) {
            if let Some(wall) = walls.get(wall_idx)
                && let Some(ops) = self.wall_openings.get(&wall.id)
            {
                out.extend_from_slice(ops);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Fixture indices within `radius` metres of `point`, sorted & deduplicated.
    pub fn fixture_candidates_near(&self, point: Point, radius: f64) -> Vec<usize> {
        let r = radius.abs();
        query_box(
            &self.fixture_tiles,
            &self.fixtures_everywhere,
            Point::new(point.x - r, point.y - r),
            Point::new(point.x + r, point.y + r),
        )
    }

    /// Drawing indices within `radius` metres of `point`, sorted & deduplicated.
    pub fn drawing_candidates_near(&self, point: Point, radius: f64) -> Vec<usize> {
        let r = radius.abs();
        query_box(
            &self.drawing_tiles,
            &self.drawings_everywhere,
            Point::new(point.x - r, point.y - r),
            Point::new(point.x + r, point.y + r),
        )
    }

    /// Room indices whose bounding boxes cover `point`, sorted & deduplicated.
    pub fn room_candidates_at(&self, point: Point) -> Vec<usize> {
        query_box(&self.room_tiles, &self.rooms_everywhere, point, point)
    }

    /// Room indices whose bounding boxes intersect `[min, max]`.
    pub fn room_candidates_in_box(&self, min: Point, max: Point) -> Vec<usize> {
        query_box(&self.room_tiles, &self.rooms_everywhere, min, max)
    }

    /// Drawing indices whose bounding boxes intersect `[min, max]`.
    pub fn drawing_candidates_in_box(&self, min: Point, max: Point) -> Vec<usize> {
        query_box(&self.drawing_tiles, &self.drawings_everywhere, min, max)
    }

    pub fn wall_index(&self, wall_id: u64) -> Option<usize> {
        self.wall_index_by_id.get(&wall_id).copied()
    }

    pub fn openings_on_wall(&self, wall_id: u64) -> &[usize] {
        self.wall_openings.get(&wall_id).map_or(&[], Vec::as_slice)
    }

    pub fn rooms_sharing_wall(&self, wall_id: u64) -> &[usize] {
        self.wall_rooms.get(&wall_id).map_or(&[], Vec::as_slice)
    }
}

/// Summary of an incremental validation pass restricted to a [`ChangeSet`] and
/// its immediate topological/spatial dependents.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IncrementalValidationReport {
    pub checked_walls: usize,
    pub checked_openings: usize,
    pub checked_rooms: usize,
    pub checked_fixtures: usize,
    pub checked_drawings: usize,
    pub issues: Vec<String>,
}

impl Project {
    /// Incrementally validates only the elements touched in `changes` plus
    /// their direct dependents (openings hosted on touched walls and rooms
    /// referencing touched walls), avoiding O(N) full-model scans per edit.
    #[allow(dead_code)]
    pub fn validate_incremental(&self, changes: &ChangeSet) -> IncrementalValidationReport {
        let mut report = IncrementalValidationReport::default();
        if changes.is_empty() {
            return report;
        }
        let spatial = self.cached_spatial_index();
        let mut wall_ids: HashSet<u64> = HashSet::new();
        let mut opening_ids: HashSet<u64> = HashSet::new();
        let mut room_indices: HashSet<usize> = HashSet::new();
        let mut fixture_ids: HashSet<u64> = HashSet::new();
        let mut drawing_ids: HashSet<u64> = HashSet::new();

        for element in changes.touched() {
            match element {
                Element::Wall(id) => {
                    wall_ids.insert(id);
                }
                Element::Opening(id) => {
                    opening_ids.insert(id);
                }
                Element::Room(id) => {
                    if let Some(idx) = self.rooms.iter().position(|r| r.id == id) {
                        room_indices.insert(idx);
                    }
                }
                Element::Fixture(id) => {
                    fixture_ids.insert(id);
                }
                Element::Drawing(id) => {
                    drawing_ids.insert(id);
                }
                Element::Terrain(_) => {}
            }
        }

        // Expand touched walls to their hosted openings and dependent rooms.
        for &wid in &wall_ids {
            for &op_idx in spatial.openings_on_wall(wid) {
                if let Some(op) = self.openings.get(op_idx) {
                    opening_ids.insert(op.id);
                }
            }
            for &rm_idx in spatial.rooms_sharing_wall(wid) {
                room_indices.insert(rm_idx);
            }
        }

        for wid in wall_ids {
            if let Some(idx) = spatial.wall_index(wid)
                && let Some(wall) = self.walls.get(idx)
            {
                report.checked_walls += 1;
                if !wall.start.x.is_finite()
                    || !wall.start.y.is_finite()
                    || !wall.end.x.is_finite()
                    || !wall.end.y.is_finite()
                {
                    report
                        .issues
                        .push(format!("Parede #{wid} possui coordenadas não finitas."));
                } else if wall.length() < 0.05 {
                    report
                        .issues
                        .push(format!("Parede #{wid} possui comprimento menor que 5 cm."));
                }
            }
        }

        for oid in opening_ids {
            if let Some(op) = self.openings.iter().find(|o| o.id == oid) {
                report.checked_openings += 1;
                match spatial
                    .wall_index(op.wall_id)
                    .and_then(|i| self.walls.get(i))
                {
                    None => report.issues.push(format!(
                        "Abertura #{oid} referencia parede inexistente #{}.",
                        op.wall_id
                    )),
                    Some(wall) => {
                        if !(0.0..=1.0).contains(&op.t) || op.width() > wall.length() + 1e-3 {
                            report.issues.push(format!(
                                "Abertura #{oid} excede os limites da parede #{}.",
                                wall.id
                            ));
                        }
                    }
                }
            }
        }

        for ridx in room_indices {
            if let Some(room) = self.rooms.get(ridx) {
                report.checked_rooms += 1;
                if room.wall_ids.len() < 3
                    || room
                        .wall_ids
                        .iter()
                        .any(|wid| spatial.wall_index(*wid).is_none())
                {
                    report.issues.push(format!(
                        "Cômodo \"{}\" possui paredes ausentes ou incompletas.",
                        room.display_name()
                    ));
                }
            }
        }

        for fid in fixture_ids {
            if let Some(f) = self.fixtures.iter().find(|f| f.id == fid) {
                report.checked_fixtures += 1;
                if !f.center.x.is_finite()
                    || !f.center.y.is_finite()
                    || f.width <= 0.0
                    || f.depth <= 0.0
                {
                    report.issues.push(format!(
                        "Objeto #{fid} possui dimensões ou posição inválidas."
                    ));
                }
            }
        }

        for did in drawing_ids {
            if let Some(d) = self.drawing(did) {
                report.checked_drawings += 1;
                if let Err(err) = d.shape.validate() {
                    report.issues.push(format!("Desenho #{did}: {err}"));
                }
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::domain::{
        discipline::Discipline,
        fixtures::FixtureKind,
        model::OpeningKind,
        snap::{OsnapSettings, find_snap},
    };

    /// Builds a synthetic 5-storey building with 500 walls on the active floor
    /// (plus 4 upper floors stored in `project.floors`) for scale verification.
    fn make_500_wall_5_storey_project() -> Project {
        let mut project = Project::default();
        // Create 10×25 grid of 4m×3m bays → 500 walls on the active floor.
        for row in 0..10usize {
            for col in 0..25usize {
                let x0 = col as f64 * 4.0;
                let y0 = row as f64 * 3.0;
                let w1 = project.add_wall(Point::new(x0, y0), Point::new(x0 + 4.0, y0));
                let _w2 = project.add_wall(Point::new(x0, y0), Point::new(x0, y0 + 3.0));
                if row % 2 == 0
                    && col % 5 == 0
                    && let Some(wid) = w1
                {
                    project.add_opening(wid, OpeningKind::Door, 0.5);
                    project.add_fixture(
                        FixtureKind::Column,
                        Point::new(x0 + 0.15, y0 + 0.15),
                        0.20,
                        0.20,
                        0,
                    );
                }
            }
        }
        assert!(project.walls.len() >= 500);
        // Add 4 upper floor levels copying the layout (5 storeys × 500 walls = 2,500 walls total).
        for storey in 1..5usize {
            project.add_floor(&format!("{}º Pavimento", storey + 1), true, true);
        }
        assert_eq!(project.floors.levels.len(), 5);
        assert!(project.walls.len() >= 500);
        project
    }

    #[test]
    fn spatial_index_matches_linear_scan_and_meets_scale_targets() {
        let mut project = make_500_wall_5_storey_project();
        let spatial = project.cached_spatial_index();

        // 1. Accuracy: spatial wall query near a known wall returns that wall.
        let sample_wall = &project.walls[250];
        let mid = sample_wall.start.midpoint(sample_wall.end);
        let candidates = spatial.wall_candidates_near(mid, 0.25);
        assert!(candidates.contains(&250));
        assert!(
            candidates.len() < project.walls.len() / 5,
            "spatial index must prune far walls (got {} of {})",
            candidates.len(),
            project.walls.len()
        );

        // 2. Picking & OSNAP speed on 500-wall plan: warm cache, average over 20 queries < 2 ms each.
        let settings = OsnapSettings::default();
        let start_snap = Instant::now();
        for k in 0..20usize {
            let p = Point::new((k as f64) * 4.0 + 0.05, 6.02);
            let res = find_snap(&project, &[], p, None, 0.25, settings);
            assert!(res.point.x.is_finite() && res.point.y.is_finite());
            let _ = project.fixture_at_in_discipline(p, Discipline::All);
            let _ = project.room_at(p);
            let _ = project.drawing_at(p, 0.2);
        }
        let per_query = start_snap.elapsed() / 20;
        assert!(
            per_query.as_millis() < 20,
            "spatial snap + pick took {per_query:?} per query on 500 walls"
        );

        // 3. Edit + changes_since + incremental validation on 500-wall 5-storey plan < 50 ms.
        let before = project.clone();
        let target_wall_id = project.walls[123].id;
        let t_edit = Instant::now();
        project.walls[123].end.x += 0.20;
        let changes = project.changes_since(&before);
        let report = project.validate_incremental(&changes);
        let edit_elapsed = t_edit.elapsed();

        assert_eq!(changes.changed, vec![Element::Wall(target_wall_id)]);
        assert_eq!(report.checked_walls, 1);
        assert!(report.issues.is_empty());
        assert!(
            edit_elapsed.as_millis() < 50,
            "incremental edit + diff + validation took {edit_elapsed:?} (> 50 ms)"
        );
    }
}
