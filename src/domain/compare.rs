//! Visual comparison between two versions of a project (#45).
//!
//! Computes geometric and semantic differences across walls, rooms, openings,
//! fixtures and 2D drawings, and exports a color-coded visual diff SVG.

use std::fmt::Write as _;

use super::{
    drawing::Drawing,
    fixtures::Fixture,
    geometry::{Point, format_meters},
    model::{Project, Wall},
};

const POS_TOL: f64 = 0.02;

/// Summary counters for quick inspection in the UI and reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiffCounts {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl DiffCounts {
    pub fn total(self) -> usize {
        self.added + self.removed + self.modified
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomDiffEntry {
    pub name: String,
    pub area_before: Option<f64>,
    pub area_after: Option<f64>,
}

/// Complete comparison result between a baseline project version and the
/// current project.
#[derive(Clone, Debug)]
pub struct ProjectDiff {
    pub baseline_label: String,
    pub walls_added: Vec<Wall>,
    pub walls_removed: Vec<Wall>,
    pub walls_modified: Vec<(Wall, Wall)>,
    pub openings_added: usize,
    pub openings_removed: usize,
    pub openings_modified: usize,
    pub fixtures_added: Vec<Fixture>,
    pub fixtures_removed: Vec<Fixture>,
    pub fixtures_modified: Vec<(Fixture, Fixture)>,
    pub drawings_added: Vec<Drawing>,
    pub drawings_removed: Vec<Drawing>,
    pub rooms_added: Vec<RoomDiffEntry>,
    pub rooms_removed: Vec<RoomDiffEntry>,
    pub rooms_modified: Vec<RoomDiffEntry>,
    pub total_area_before: f64,
    pub total_area_after: f64,
}

impl ProjectDiff {
    pub fn counts(&self) -> DiffCounts {
        DiffCounts {
            added: self.walls_added.len()
                + self.openings_added
                + self.fixtures_added.len()
                + self.drawings_added.len()
                + self.rooms_added.len(),
            removed: self.walls_removed.len()
                + self.openings_removed
                + self.fixtures_removed.len()
                + self.drawings_removed.len()
                + self.rooms_removed.len(),
            modified: self.walls_modified.len()
                + self.openings_modified
                + self.fixtures_modified.len()
                + self.rooms_modified.len(),
        }
    }

    pub fn is_identical(&self) -> bool {
        self.counts().total() == 0
    }
}

fn walls_geometrically_same(a: &Wall, b: &Wall) -> bool {
    let direct = a.start.near(b.start, POS_TOL) && a.end.near(b.end, POS_TOL);
    let flipped = a.start.near(b.end, POS_TOL) && a.end.near(b.start, POS_TOL);
    (direct || flipped) && (a.thickness() - b.thickness()).abs() < 0.005
}

fn fixtures_same(a: &Fixture, b: &Fixture) -> bool {
    a.kind == b.kind
        && a.center.near(b.center, POS_TOL)
        && (a.width - b.width).abs() < POS_TOL
        && (a.depth - b.depth).abs() < POS_TOL
        && a.quarter_turns % 4 == b.quarter_turns % 4
}

fn total_room_area(project: &Project) -> f64 {
    project
        .rooms
        .iter()
        .filter_map(|r| project.room_area(r))
        .sum()
}

/// Computes a structured geometric and semantic diff between `baseline`
/// (older version) and `current` (active version).
pub fn compare_projects(
    baseline: &Project,
    current: &Project,
    baseline_label: impl Into<String>,
) -> ProjectDiff {
    // --- Walls ---
    let mut matched_base_walls = vec![false; baseline.walls.len()];
    let mut matched_cur_walls = vec![false; current.walls.len()];
    let mut walls_modified = Vec::new();

    // 1) Exact geometric matches (even if IDs changed across saves/imports).
    for (ci, cw) in current.walls.iter().enumerate() {
        if let Some(bi) = baseline
            .walls
            .iter()
            .enumerate()
            .position(|(bi, bw)| !matched_base_walls[bi] && walls_geometrically_same(bw, cw))
        {
            matched_base_walls[bi] = true;
            matched_cur_walls[ci] = true;
        }
    }
    // 2) Same ID but moved / resized / different thickness -> modified.
    for (ci, cw) in current.walls.iter().enumerate() {
        if matched_cur_walls[ci] {
            continue;
        }
        if let Some(bi) = baseline
            .walls
            .iter()
            .enumerate()
            .position(|(bi, bw)| !matched_base_walls[bi] && bw.id == cw.id)
        {
            matched_base_walls[bi] = true;
            matched_cur_walls[ci] = true;
            walls_modified.push((baseline.walls[bi].clone(), cw.clone()));
        }
    }
    let walls_added: Vec<Wall> = current
        .walls
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_cur_walls[*i])
        .map(|(_, w)| w.clone())
        .collect();
    let walls_removed: Vec<Wall> = baseline
        .walls
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_base_walls[*i])
        .map(|(_, w)| w.clone())
        .collect();

    // --- Openings ---
    let mut openings_added = 0usize;
    let mut openings_removed = 0usize;
    let mut openings_modified = 0usize;
    let mut matched_base_ops = vec![false; baseline.openings.len()];
    for co in &current.openings {
        if let Some(bi) = baseline
            .openings
            .iter()
            .enumerate()
            .position(|(bi, bo)| !matched_base_ops[bi] && bo.id == co.id)
        {
            matched_base_ops[bi] = true;
            let bo = &baseline.openings[bi];
            if bo.kind != co.kind
                || bo.wall_id != co.wall_id
                || (bo.t - co.t).abs() > 0.005
                || (bo.width() - co.width()).abs() > 0.01
                || bo.flip_side != co.flip_side
                || bo.flip_hinge != co.flip_hinge
            {
                openings_modified += 1;
            }
        } else {
            openings_added += 1;
        }
    }
    for matched in matched_base_ops {
        if !matched {
            openings_removed += 1;
        }
    }

    // --- Fixtures ---
    let mut matched_base_fix = vec![false; baseline.fixtures.len()];
    let mut matched_cur_fix = vec![false; current.fixtures.len()];
    let mut fixtures_modified = Vec::new();
    for (ci, cf) in current.fixtures.iter().enumerate() {
        if let Some(bi) = baseline
            .fixtures
            .iter()
            .enumerate()
            .position(|(bi, bf)| !matched_base_fix[bi] && fixtures_same(bf, cf))
        {
            matched_base_fix[bi] = true;
            matched_cur_fix[ci] = true;
        }
    }
    for (ci, cf) in current.fixtures.iter().enumerate() {
        if matched_cur_fix[ci] {
            continue;
        }
        if let Some(bi) = baseline
            .fixtures
            .iter()
            .enumerate()
            .position(|(bi, bf)| !matched_base_fix[bi] && bf.id == cf.id)
        {
            matched_base_fix[bi] = true;
            matched_cur_fix[ci] = true;
            fixtures_modified.push((baseline.fixtures[bi], *cf));
        }
    }
    let fixtures_added: Vec<Fixture> = current
        .fixtures
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_cur_fix[*i])
        .map(|(_, f)| *f)
        .collect();
    let fixtures_removed: Vec<Fixture> = baseline
        .fixtures
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_base_fix[*i])
        .map(|(_, f)| *f)
        .collect();

    // --- 2D Drawings ---
    let mut matched_base_draw = vec![false; baseline.drawings.len()];
    let mut drawings_added = Vec::new();
    for cd in &current.drawings {
        if let Some(bi) = baseline
            .drawings
            .iter()
            .enumerate()
            .position(|(bi, bd)| !matched_base_draw[bi] && bd.shape == cd.shape)
        {
            matched_base_draw[bi] = true;
        } else {
            drawings_added.push(cd.clone());
        }
    }
    let drawings_removed: Vec<Drawing> = baseline
        .drawings
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_base_draw[*i])
        .map(|(_, d)| d.clone())
        .collect();

    // --- Rooms ---
    let mut matched_base_rooms = vec![false; baseline.rooms.len()];
    let mut rooms_added = Vec::new();
    let mut rooms_modified = Vec::new();
    for cr in &current.rooms {
        let cur_area = current.room_area(cr);
        if let Some(bi) = baseline.rooms.iter().enumerate().position(|(bi, br)| {
            !matched_base_rooms[bi] && (br.id == cr.id || br.name.eq_ignore_ascii_case(&cr.name))
        }) {
            matched_base_rooms[bi] = true;
            let br = &baseline.rooms[bi];
            let base_area = baseline.room_area(br);
            let area_changed = match (base_area, cur_area) {
                (Some(a), Some(b)) => (a - b).abs() > 0.02,
                (None, None) => false,
                _ => true,
            };
            if br.name != cr.name || area_changed {
                rooms_modified.push(RoomDiffEntry {
                    name: if br.name == cr.name {
                        cr.name.clone()
                    } else {
                        format!("{} → {}", br.name, cr.name)
                    },
                    area_before: base_area,
                    area_after: cur_area,
                });
            }
        } else {
            rooms_added.push(RoomDiffEntry {
                name: cr.name.clone(),
                area_before: None,
                area_after: cur_area,
            });
        }
    }
    let rooms_removed: Vec<RoomDiffEntry> = baseline
        .rooms
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_base_rooms[*i])
        .map(|(_, br)| RoomDiffEntry {
            name: br.name.clone(),
            area_before: baseline.room_area(br),
            area_after: None,
        })
        .collect();

    ProjectDiff {
        baseline_label: baseline_label.into(),
        walls_added,
        walls_removed,
        walls_modified,
        openings_added,
        openings_removed,
        openings_modified,
        fixtures_added,
        fixtures_removed,
        fixtures_modified,
        drawings_added,
        drawings_removed,
        rooms_added,
        rooms_removed,
        rooms_modified,
        total_area_before: total_room_area(baseline),
        total_area_after: total_room_area(current),
    }
}

/// Exports a standalone visual diff SVG comparing `baseline` and `current`:
/// unchanged walls in muted slate, removed elements in dashed red, added
/// elements in solid green, and modified elements in amber.
pub fn export_diff_svg(baseline: &Project, current: &Project, diff: &ProjectDiff) -> String {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for bounds in [baseline.bounds(), current.bounds()].into_iter().flatten() {
        min.x = min.x.min(bounds.0.x);
        min.y = min.y.min(bounds.0.y);
        max.x = max.x.max(bounds.1.x);
        max.y = max.y.max(bounds.1.y);
    }
    if !min.x.is_finite() {
        min = Point::new(0.0, 0.0);
        max = Point::new(10.0, 8.0);
    }
    let span_x = (max.x - min.x).max(4.0);
    let span_y = (max.y - min.y).max(4.0);
    let ppm = 80.0_f64.min(2400.0 / span_x).min(2400.0 / span_y);
    let margin = 64.0;
    let width = (span_x * ppm + margin * 2.0).ceil() as u32;
    let height = (span_y * ppm + margin * 2.0 + 48.0).ceil() as u32;
    let pt = |p: Point| -> (f64, f64) {
        (
            margin + (p.x - min.x) * ppm,
            margin + 36.0 + (p.y - min.y) * ppm,
        )
    };

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    let counts = diff.counts();
    let _ = writeln!(
        svg,
        "<text x=\"{margin:.1}\" y=\"28\" font-family=\"Arial, sans-serif\" font-size=\"15\" font-weight=\"700\" fill=\"#1d2836\">Comparação visual: {} → {} (+{} / -{} / ~{})</text>",
        diff.baseline_label
            .replace('&', "&amp;")
            .replace('<', "&lt;"),
        current.name.replace('&', "&amp;").replace('<', "&lt;"),
        counts.added,
        counts.removed,
        counts.modified,
    );
    let _ = writeln!(
        svg,
        "<g font-family=\"Arial, sans-serif\" font-size=\"11\" fill=\"#3b4e63\">\
         <rect x=\"{margin:.1}\" y=\"40\" width=\"12\" height=\"8\" fill=\"#1e9e5a\"/><text x=\"{:.1}\" y=\"48\">Adicionado</text>\
         <rect x=\"{:.1}\" y=\"40\" width=\"12\" height=\"8\" fill=\"#d63b30\"/><text x=\"{:.1}\" y=\"48\">Removido</text>\
         <rect x=\"{:.1}\" y=\"40\" width=\"12\" height=\"8\" fill=\"#d5841b\"/><text x=\"{:.1}\" y=\"48\">Alterado</text>\
         <rect x=\"{:.1}\" y=\"40\" width=\"12\" height=\"8\" fill=\"#cbd5e1\"/><text x=\"{:.1}\" y=\"48\">Sem alteração</text>\
         </g>",
        margin + 18.0,
        margin + 100.0,
        margin + 118.0,
        margin + 195.0,
        margin + 213.0,
        margin + 285.0,
        margin + 303.0,
    );

    // Context: current walls in light muted slate.
    for wall in &current.walls {
        let (x1, y1) = pt(wall.start);
        let (x2, y2) = pt(wall.end);
        let sw = (wall.thickness() * ppm).max(2.0);
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#cbd5e1\" stroke-width=\"{sw:.1}\" stroke-linecap=\"square\" data-diff=\"context\"/>"
        );
    }
    // Removed walls (dashed red).
    for wall in &diff.walls_removed {
        let (x1, y1) = pt(wall.start);
        let (x2, y2) = pt(wall.end);
        let sw = (wall.thickness() * ppm).max(3.0);
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#d63b30\" stroke-width=\"{sw:.1}\" stroke-dasharray=\"8 5\" opacity=\"0.85\" data-diff=\"removed\"/>"
        );
    }
    // Modified walls (previous dashed amber + current solid amber).
    for (old, new) in &diff.walls_modified {
        let (ox1, oy1) = pt(old.start);
        let (ox2, oy2) = pt(old.end);
        let (nx1, ny1) = pt(new.start);
        let (nx2, ny2) = pt(new.end);
        let sw = (new.thickness() * ppm).max(3.0);
        let _ = writeln!(
            svg,
            "<line x1=\"{ox1:.1}\" y1=\"{oy1:.1}\" x2=\"{ox2:.1}\" y2=\"{oy2:.1}\" stroke=\"#d5841b\" stroke-width=\"{:.1}\" stroke-dasharray=\"5 4\" opacity=\"0.55\" data-diff=\"modified-old\"/>",
            sw * 0.8
        );
        let _ = writeln!(
            svg,
            "<line x1=\"{nx1:.1}\" y1=\"{ny1:.1}\" x2=\"{nx2:.1}\" y2=\"{ny2:.1}\" stroke=\"#d5841b\" stroke-width=\"{sw:.1}\" data-diff=\"modified\"/>"
        );
    }
    // Added walls (solid green).
    for wall in &diff.walls_added {
        let (x1, y1) = pt(wall.start);
        let (x2, y2) = pt(wall.end);
        let sw = (wall.thickness() * ppm).max(3.0);
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#1e9e5a\" stroke-width=\"{sw:.1}\" data-diff=\"added\"/>"
        );
    }
    // Fixtures added / removed / modified.
    let write_fix = |svg: &mut String, fix: &Fixture, color: &str, dashed: bool, tag: &str| {
        let (lo, hi) = fix.bounds();
        let (x1, y1) = pt(lo);
        let (x2, y2) = pt(hi);
        let dash = if dashed {
            " stroke-dasharray=\"5 3\""
        } else {
            ""
        };
        let _ = writeln!(
            svg,
            "<rect x=\"{x1:.1}\" y=\"{y1:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{color}\" fill-opacity=\"0.18\" stroke=\"{color}\" stroke-width=\"2\"{dash} data-diff=\"{tag}\"/>",
            (x2 - x1).max(4.0),
            (y2 - y1).max(4.0)
        );
    };
    for f in &diff.fixtures_removed {
        write_fix(&mut svg, f, "#d63b30", true, "removed");
    }
    for (_, f) in &diff.fixtures_modified {
        write_fix(&mut svg, f, "#d5841b", false, "modified");
    }
    for f in &diff.fixtures_added {
        write_fix(&mut svg, f, "#1e9e5a", false, "added");
    }

    let delta = diff.total_area_after - diff.total_area_before;
    let sign = if delta >= 0.0 { "+" } else { "" };
    let _ = writeln!(
        svg,
        "<text x=\"{margin:.1}\" y=\"{}\" font-family=\"Arial, sans-serif\" font-size=\"11\" fill=\"#627489\">Área total: {:.2} m² → {:.2} m² ({sign}{:.2} m²) · Referência de cota: {}</text>",
        height - 12,
        diff.total_area_before,
        diff.total_area_after,
        delta,
        format_meters(1.0),
    );
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    #[test]
    fn identical_projects_report_zero_differences() {
        let mut p = Project::default();
        p.add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let diff = compare_projects(&p, &p, "v1");
        assert!(diff.is_identical());
        assert_eq!(diff.counts().total(), 0);
    }

    #[test]
    fn detects_added_removed_and_modified_walls_fixtures_and_room_areas() {
        let mut v1 = Project::default();
        v1.name = "Projeto v1".to_owned();
        v1.add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let (sw, sd) = FixtureKind::Sofa.default_size();
        let sofa_id = v1
            .add_fixture(FixtureKind::Sofa, Point::new(1.5, 1.5), sw, sd, 0)
            .unwrap();

        let mut v2 = v1.clone();
        v2.name = "Projeto v2".to_owned();
        // Add a new wall.
        v2.add_wall(Point::new(4.0, 0.0), Point::new(7.0, 0.0))
            .unwrap();
        // Remove the sofa and add a table.
        v2.delete_fixture(sofa_id);
        let (tw, td) = FixtureKind::Table.default_size();
        v2.add_fixture(FixtureKind::Table, Point::new(2.0, 1.5), tw, td, 0)
            .unwrap();
        // Move an existing wall slightly.
        let moved_id = v2.walls[0].id;
        v2.walls
            .iter_mut()
            .find(|w| w.id == moved_id)
            .unwrap()
            .end
            .x += 0.5;

        let diff = compare_projects(&v1, &v2, "v1.trazo");
        assert!(!diff.is_identical());
        assert_eq!(diff.walls_added.len(), 1);
        assert_eq!(diff.walls_modified.len(), 1);
        assert_eq!(diff.fixtures_removed.len(), 1);
        assert_eq!(diff.fixtures_added.len(), 1);

        let svg = export_diff_svg(&v1, &v2, &diff);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("data-diff=\"added\""));
        assert!(svg.contains("data-diff=\"removed\""));
        assert!(svg.contains("data-diff=\"modified\""));
    }
}
