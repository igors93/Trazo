//! Structural element layout, CRUD, boundary-lot placement, beam-line grouping,
//! automatic generation and footing sizing.

use std::collections::{HashMap, HashSet};

use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::Point,
    model::Project,
};

use super::{
    Beam, BeamLine, BeamSupport, Slab, SlabKind, StructureGeneration, decompose_slab_panels,
    reading, round_up, slab_geometry::wall_orientation, validate_section,
};

impl Project {
    pub fn structural_columns(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind == FixtureKind::Column)
    }

    pub(super) fn column(&self, id: u64) -> Option<&Fixture> {
        self.structural_columns().find(|column| column.id == id)
    }

    pub(super) fn beam_by_id(&self, id: u64) -> Option<&Beam> {
        self.structure.beams.iter().find(|beam| beam.id == id)
    }

    pub(super) fn resolve_support(
        &self,
        support: BeamSupport,
        visited: &mut HashSet<u64>,
    ) -> Option<Point> {
        match support {
            BeamSupport::Column { id } => self.column(id).map(|c| c.center),
            BeamSupport::Beam { beam_id, t } => {
                if !t.is_finite() || !(0.0..=1.0).contains(&t) || !visited.insert(beam_id) {
                    return None;
                }
                let host = self.beam_by_id(beam_id)?;
                let (a, b) = self.beam_endpoints_inner(host, visited)?;
                visited.remove(&beam_id);
                Some(a.lerp(b, t))
            }
        }
    }

    fn beam_endpoints_inner(
        &self,
        beam: &Beam,
        visited: &mut HashSet<u64>,
    ) -> Option<(Point, Point)> {
        let start = self.resolve_support(beam.start_support(), visited)?;
        let end = self.resolve_support(beam.end_support(), visited)?;
        if start.distance(end) < 1e-4 {
            return None;
        }
        Some((start, end))
    }

    /// World-space start and end points of `beam`, whether supported on
    /// columns or on other beams.
    pub fn beam_endpoints(&self, beam: &Beam) -> Option<(Point, Point)> {
        let mut visited = HashSet::from([beam.id]);
        self.beam_endpoints_inner(beam, &mut visited)
    }

    /// Support depth of a beam: 0 when resting only on columns, 1 when resting
    /// on a primary beam, etc.
    pub(super) fn beam_support_depth(&self, beam: &Beam, visited: &mut HashSet<u64>) -> usize {
        let mut depth = 0;
        for support in [beam.start_support(), beam.end_support()] {
            if let BeamSupport::Beam { beam_id, .. } = support
                && visited.insert(beam_id)
                && let Some(host) = self.beam_by_id(beam_id)
            {
                depth = depth.max(1 + self.beam_support_depth(host, visited));
                visited.remove(&beam_id);
            }
        }
        depth
    }

    /// The beams whose two ends are valid supports (columns or acyclic host beams).
    pub fn valid_beams(&self) -> impl Iterator<Item = &Beam> {
        self.structure
            .beams
            .iter()
            .filter(|beam| self.beam_endpoints(beam).is_some())
    }

    /// Drops beams left without a valid support and slabs left without a room.
    pub fn prune_structure(&mut self) -> usize {
        let before = self.structure.beams.len() + self.structure.slabs.len();
        let valid: HashSet<u64> = self.valid_beams().map(|beam| beam.id).collect();
        self.structure.beams.retain(|beam| valid.contains(&beam.id));
        let rooms: HashSet<u64> = self.rooms.iter().map(|room| room.id).collect();
        self.structure
            .slabs
            .retain(|slab| rooms.contains(&slab.room_id));
        let columns: HashSet<u64> = self.structural_columns().map(|c| c.id).collect();
        let walls: HashSet<u64> = self.walls.iter().map(|w| w.id).collect();
        self.structure
            .wall_foundations
            .retain(|f| walls.contains(&f.wall_id));
        let balance_before = self.structure.balance_beams.len();
        self.structure
            .balance_beams
            .retain(|b| columns.contains(&b.column) && columns.contains(&b.anchor));
        before - self.structure.beams.len() - self.structure.slabs.len()
            + (balance_before - self.structure.balance_beams.len())
    }

    /// Where a column or footing lands for the pointer at `point`: on the
    /// nearest wall junction within 0.5 m, else on the nearest wall's
    /// centre line, turned along the wall (at any angle for diagonal walls).
    pub fn structural_placement_with_angle(&self, point: Point) -> (Point, u8, f64) {
        let mut best: Option<(f64, Point, u8, f64)> = None;
        for wall in &self.walls {
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            // Junctions win over a plain point on the wall.
            for end in [wall.start, wall.end] {
                let d = end.distance(point);
                let score = d - 0.2;
                if d < 0.5 && best.is_none_or(|(b, _, _, _)| score < b) {
                    best = Some((score, end, turns, angle));
                }
            }
            let (on, _) =
                crate::domain::geometry::nearest_point_on_segment(point, wall.start, wall.end);
            let d = on.distance(point);
            if d < 0.5 && best.is_none_or(|(b, _, _, _)| d < b) {
                best = Some((d, on, turns, angle));
            }
        }
        best.map_or((point, 0, 0.0), |(_, at, turns, angle)| (at, turns, angle))
    }

    /// Moves a fixture of this size just inside the lot when part of it would
    /// stand outside: a column or footing on a boundary wall ends up flush
    /// with the lot line instead of being refused (B23). The centre comes
    /// back unchanged when it already fits or there is no lot.
    pub fn fit_inside_lot(
        &self,
        kind: FixtureKind,
        center: Point,
        width: f64,
        depth: f64,
        quarter_turns: u8,
        angle_rad: f64,
    ) -> Point {
        let Some(terrain) = &self.terrain else {
            return center;
        };
        let outline = terrain.outline();
        if outline.len() < 3 {
            return center;
        }
        let mut probe = Fixture {
            id: 0,
            kind,
            center,
            width,
            depth,
            quarter_turns: quarter_turns % 4,
            angle_rad,
            electrical: None,
            structural_load_kn: kind.default_structural_load_kn(),
        };
        let limit = width.max(depth);
        for _ in 0..6 {
            let (low, high) = probe.bounds();
            let outside = [
                low,
                Point::new(high.x, low.y),
                high,
                Point::new(low.x, high.y),
            ]
            .into_iter()
            .find(|corner| !terrain.contains(*corner));
            let Some(corner) = outside else {
                break;
            };
            let nearest = (0..outline.len())
                .map(|i| {
                    crate::domain::geometry::nearest_point_on_segment(
                        corner,
                        outline[i],
                        outline[(i + 1) % outline.len()],
                    )
                    .0
                })
                .min_by(|a, b| a.distance(corner).total_cmp(&b.distance(corner)));
            let Some(nearest) = nearest else {
                break;
            };
            let Some(direction) = nearest.minus(corner).normalized() else {
                break;
            };
            let step = nearest.distance(corner) + 0.001;
            probe.center = probe.center.translated(direction.scaled(step));
            if probe.center.distance(center) > limit {
                return center;
            }
        }
        probe.center
    }

    /// Where a column or footing of this size lands for the pointer at
    /// `point`: on the nearest wall junction or centre line, then moved just
    /// inside the lot if it would cross the lot line.
    pub fn structural_placement_fitted(
        &self,
        kind: FixtureKind,
        point: Point,
        width: f64,
        depth: f64,
    ) -> (Point, u8, f64) {
        let (at, turns, angle) = self.structural_placement_with_angle(point);
        (
            self.fit_inside_lot(kind, at, width, depth, turns, angle),
            turns,
            angle,
        )
    }

    /// Same as [`structural_placement_with_angle`], returning position and quarter turns.
    #[cfg(test)]
    pub fn structural_placement(&self, point: Point) -> (Point, u8) {
        let (at, turns, _) = self.structural_placement_with_angle(point);
        (at, turns)
    }

    /// Joins two columns with a beam. Returns its id.
    pub fn add_beam(&mut self, from: u64, to: u64, width: f64, height: f64) -> Result<u64, String> {
        self.add_beam_on_supports(
            BeamSupport::Column { id: from },
            BeamSupport::Column { id: to },
            width,
            height,
        )
    }

    /// Joins two supports (columns or points along existing beams) with a beam.
    pub fn add_beam_on_supports(
        &mut self,
        from: BeamSupport,
        to: BeamSupport,
        width: f64,
        height: f64,
    ) -> Result<u64, String> {
        if from == to {
            return Err("Escolha dois apoios diferentes.".to_owned());
        }
        for support in [from, to] {
            match support {
                BeamSupport::Column { id } => {
                    if self.column(id).is_none() {
                        return Err(format!("O id {id} não é um pilar."));
                    }
                }
                BeamSupport::Beam { beam_id, t } => {
                    if !t.is_finite() || !(0.01..=0.99).contains(&t) {
                        return Err(
                            "A posição do apoio na viga deve ficar entre 0,01 e 0,99.".to_owned()
                        );
                    }
                    if self
                        .beam_by_id(beam_id)
                        .and_then(|b| self.beam_endpoints(b))
                        .is_none()
                    {
                        return Err(format!("O id {beam_id} não é uma viga válida."));
                    }
                }
            }
        }
        validate_section(width, height)?;
        let mut visited = HashSet::new();
        let a = self
            .resolve_support(from, &mut visited)
            .ok_or("Apoio inicial inválido.")?;
        visited.clear();
        let b = self
            .resolve_support(to, &mut visited)
            .ok_or("Apoio final inválido.")?;
        if a.distance(b) < 0.05 {
            return Err("Os apoios da viga estão muito próximos.".to_owned());
        }
        if let (BeamSupport::Column { id: c1 }, BeamSupport::Column { id: c2 }) = (from, to)
            && self.valid_beams().any(|beam| {
                matches!(
                    (beam.start_support(), beam.end_support()),
                    (BeamSupport::Column { id: a }, BeamSupport::Column { id: b })
                        if (a == c1 && b == c2) || (a == c2 && b == c1)
                )
            })
        {
            return Err("Esses pilares já estão ligados por uma viga.".to_owned());
        }
        let id = self.id();
        let from_support = matches!(from, BeamSupport::Beam { .. }).then_some(from);
        let to_support = matches!(to, BeamSupport::Beam { .. }).then_some(to);
        self.structure.beams.push(Beam {
            id,
            from: from.target_id(),
            to: to.target_id(),
            from_support,
            to_support,
            width_m: width,
            height_m: height,
        });
        Ok(id)
    }

    pub fn delete_beam(&mut self, id: u64) -> bool {
        let before = self.structure.beams.len();
        self.structure.beams.retain(|beam| beam.id != id);
        self.structure.beams.len() < before
    }

    pub fn update_beam(
        &mut self,
        id: u64,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Result<(), String> {
        let beam = self
            .structure
            .beams
            .iter_mut()
            .find(|beam| beam.id == id)
            .ok_or("Viga não encontrada.")?;
        let (w, h) = (
            width.unwrap_or(beam.width_m),
            height.unwrap_or(beam.height_m),
        );
        validate_section(w, h)?;
        beam.width_m = w;
        beam.height_m = h;
        Ok(())
    }

    /// Beams touching any of `columns`, including secondary beams transitively
    /// supported on them.
    pub fn beams_of(&self, columns: &[u64]) -> Vec<u64> {
        let mut affected: HashSet<u64> = self
            .valid_beams()
            .filter(|beam| {
                matches!(beam.start_support(), BeamSupport::Column { id } if columns.contains(&id))
                    || matches!(beam.end_support(), BeamSupport::Column { id } if columns.contains(&id))
            })
            .map(|beam| beam.id)
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for beam in self.valid_beams() {
                if affected.contains(&beam.id) {
                    continue;
                }
                let rests_on_affected = [beam.start_support(), beam.end_support()]
                    .into_iter()
                    .any(|s| matches!(s, BeamSupport::Beam { beam_id, .. } if affected.contains(&beam_id)));
                if rests_on_affected {
                    affected.insert(beam.id);
                    changed = true;
                }
            }
        }
        self.valid_beams()
            .filter(|beam| affected.contains(&beam.id))
            .map(|beam| beam.id)
            .collect()
    }

    /// Sets (or adds) the slab over a room.
    pub fn set_slab(
        &mut self,
        room_id: u64,
        kind: SlabKind,
        thickness: Option<f64>,
    ) -> Result<u64, String> {
        let room = self
            .rooms
            .iter()
            .find(|room| room.id == room_id)
            .ok_or("Cômodo não encontrado.")?
            .clone();
        let lx = self.slab_spans(&room).0;
        let thickness = thickness.unwrap_or_else(|| kind.minimum_thickness(lx));
        if !thickness.is_finite() || !(0.05..=0.60).contains(&thickness) {
            return Err("A espessura da laje deve ficar entre 5 e 60 cm.".to_owned());
        }
        if let Some(slab) = self
            .structure
            .slabs
            .iter_mut()
            .find(|s| s.room_id == room_id)
        {
            slab.kind = kind;
            slab.thickness_m = thickness;
            return Ok(slab.id);
        }
        let id = self.id();
        self.structure.slabs.push(Slab {
            id,
            room_id,
            kind,
            thickness_m: thickness,
        });
        Ok(id)
    }

    pub fn remove_slab(&mut self, room_id: u64) -> bool {
        let before = self.structure.slabs.len();
        self.structure.slabs.retain(|slab| slab.room_id != room_id);
        self.structure.slabs.len() < before
    }

    /// The smaller and larger span of the slab over `room` (its outline's
    /// bounding box, axis to axis), metres.
    pub(super) fn slab_spans(&self, room: &crate::domain::model::Room) -> (f64, f64) {
        let Some(outline) = self.room_outline(room) else {
            return (0.0, 0.0);
        };
        let panels = decompose_slab_panels(&outline);
        if panels.is_empty() {
            return (0.0, 0.0);
        }
        let lx = panels.iter().map(|(x, _)| *x).fold(0.0_f64, f64::max);
        let ly = panels.iter().map(|(_, y)| *y).fold(0.0_f64, f64::max);
        (lx, ly)
    }

    /// Column, beam, slab, footing and concentrated-load labels: P1, V1,
    /// L1, S1 and CC1… in reading
    /// order, footings after the column they carry.
    pub fn structure_labels(&self) -> HashMap<u64, String> {
        let mut labels = HashMap::new();
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for (i, column) in columns.iter().enumerate() {
            labels.insert(column.id, format!("P{}", i + 1));
        }
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        let mut next = columns.len() + 1;
        for footing in footings {
            let under = columns
                .iter()
                .position(|column| footing.contains(column.center))
                .map(|i| i + 1);
            let number = under.unwrap_or_else(|| {
                next += 1;
                next - 1
            });
            labels.insert(footing.id, format!("S{number}"));
        }
        for line in self.beam_lines() {
            for span in &line.spans {
                labels.insert(*span, line.label.clone());
            }
        }
        let mut slabs: Vec<(Point, u64)> = self
            .structure
            .slabs
            .iter()
            .filter_map(|slab| {
                let room = self.rooms.iter().find(|room| room.id == slab.room_id)?;
                Some((self.room_label_anchor(room)?.0, slab.id))
            })
            .collect();
        slabs.sort_by(|a, b| reading(a.0, b.0));
        for (i, (_, id)) in slabs.iter().enumerate() {
            labels.insert(*id, format!("L{}", i + 1));
        }
        let mut loads: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| {
                matches!(
                    fixture.kind,
                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                )
            })
            .collect();
        loads.sort_by(|a, b| reading(a.center, b.center));
        for (index, load) in loads.iter().enumerate() {
            labels.insert(load.id, format!("CC{}", index + 1));
        }
        labels
    }

    /// Collinear beams through shared columns, as continuous beams.
    pub fn beam_lines(&self) -> Vec<BeamLine> {
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                let (a, b) = self.beam_endpoints(beam)?;
                Some((*beam, a, b))
            })
            .collect();
        let direction = |a: Point, b: Point| {
            let d = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
            if d.x > 1e-6 || (d.x.abs() <= 1e-6 && d.y > 0.0) {
                d
            } else {
                d.scaled(-1.0)
            }
        };
        // Union-find over beams joined end to end at a shared column in a straight line.
        let mut parent: Vec<usize> = (0..beams.len()).collect();
        fn root(parent: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        // Index beams by their column supports so union-find only checks
        // collinear beams meeting at the same column (O(B) instead of O(B²)).
        let mut by_column: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, (beam, _, _)) in beams.iter().enumerate() {
            for support in [beam.start_support(), beam.end_support()] {
                if let BeamSupport::Column { id } = support {
                    by_column.entry(id).or_default().push(idx);
                }
            }
        }
        let dirs: Vec<Point> = beams.iter().map(|(_, a, b)| direction(*a, *b)).collect();
        for group in by_column.values() {
            for a_idx in 0..group.len() {
                for b_idx in a_idx + 1..group.len() {
                    let (i, j) = (group[a_idx], group[b_idx]);
                    if dirs[i].cross(dirs[j]).abs() < 0.02 {
                        let (ra, rb) = (root(&mut parent, i), root(&mut parent, j));
                        parent[ra] = rb;
                    }
                }
            }
        }
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..beams.len() {
            let r = root(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
        let mut lines: Vec<(u8, f64, f64, BeamLine)> = groups
            .into_values()
            .map(|members| {
                let d = direction(beams[members[0]].1, beams[members[0]].2);
                let along = |p: Point| p.dot(d);
                let mut spans: Vec<&(Beam, Point, Point)> =
                    members.iter().map(|i| &beams[*i]).collect();
                spans.sort_by(|a, b| along(a.1.midpoint(a.2)).total_cmp(&along(b.1.midpoint(b.2))));
                let mut joints: Vec<(f64, BeamSupport)> = Vec::new();
                for (beam, a, b) in &spans {
                    for (support, p) in [(beam.start_support(), *a), (beam.end_support(), *b)] {
                        let already = joints
                            .iter()
                            .any(|(pos, s)| *s == support && (*pos - along(p)).abs() < 1e-3);
                        if !already {
                            joints.push((along(p), support));
                        }
                    }
                }
                joints.sort_by(|a, b| a.0.total_cmp(&b.0));
                let points: Vec<Point> = spans.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
                let (sum_x, sum_y) = points
                    .iter()
                    .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
                let n = points.len().max(1) as f64;
                let min_x = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
                let min_y = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
                let (group, key1, key2) = if d.y.abs() < 0.1 {
                    (0, sum_y / n, min_x)
                } else if d.x.abs() < 0.1 {
                    (1, sum_x / n, min_y)
                } else {
                    (2, min_y, min_x)
                };
                let supports: Vec<BeamSupport> = joints.into_iter().map(|(_, s)| s).collect();
                let joint_ids: Vec<u64> = supports.iter().map(|s| s.target_id()).collect();
                (
                    group,
                    key1,
                    key2,
                    BeamLine {
                        label: String::new(),
                        spans: spans.iter().map(|(beam, _, _)| beam.id).collect(),
                        joints: joint_ids,
                        supports,
                    },
                )
            })
            .collect();
        lines.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(((a.1 / 0.3).round() as i64).cmp(&((b.1 / 0.3).round() as i64)))
                .then(a.2.total_cmp(&b.2))
        });
        lines
            .into_iter()
            .enumerate()
            .map(|(i, (_, _, _, mut line))| {
                line.label = format!("V{}", i + 1);
                line
            })
            .collect()
    }

    /// Lays out a first structure over the walls: a column at every wall
    /// junction and end, more along walls longer than 4.5 m (clear of
    /// doors and windows), a beam along every wall between consecutive
    /// columns, a lattice slab over every room and a footing under every
    /// column, each sized by the pre-sizing. What exists stays.
    pub fn generate_structure(&mut self) -> StructureGeneration {
        let mut report = StructureGeneration::default();
        let max_span = 4.5;
        let near = |project: &Project, p: Point| {
            project
                .structural_columns()
                .any(|column| column.center.distance(p) < 0.3)
        };
        let walls = self.walls.clone();
        // Columns at junctions and ends (oriented along angled walls too).
        for wall in &walls {
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            for end in [wall.start, wall.end] {
                if near(self, end) {
                    continue;
                }
                // On a boundary wall the column stands flush inside the lot.
                let at = self.fit_inside_lot(FixtureKind::Column, end, 0.30, b, turns, angle);
                if near(self, at) {
                    continue;
                }
                match self.add_fixture(FixtureKind::Column, at, 0.30, b, turns) {
                    Some(id) => {
                        if angle.abs() > 1e-9
                            && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                        {
                            f.angle_rad = angle;
                        }
                        report.columns += 1;
                    }
                    None => report.skipped += 1,
                }
            }
        }
        // Intermediate columns on long walls, clear of openings.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            if length <= max_span {
                continue;
            }
            let along = wall.end.minus(wall.start);
            let (turns, angle) = wall_orientation(along);
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            let blocked: Vec<(f64, f64)> = self
                .openings
                .iter()
                .filter(|opening| opening.wall_id == wall.id)
                .map(|opening| {
                    let s = opening.t * length;
                    let half = opening.width() / 2.0 + 0.2;
                    (s - half, s + half)
                })
                .collect();
            let pieces = (length / max_span).ceil() as usize;
            for k in 1..pieces {
                let target = length * k as f64 / pieces as f64;
                let spot = (0..30)
                    .flat_map(|i| [f64::from(i) * 0.1, -f64::from(i) * 0.1])
                    .map(|delta| target + delta)
                    .find(|s| {
                        *s > 0.5
                            && *s < length - 0.5
                            && !blocked.iter().any(|(a, z)| s > a && s < z)
                    });
                if let Some(s) = spot {
                    let on_axis = wall.start.lerp(wall.end, s / length);
                    let at =
                        self.fit_inside_lot(FixtureKind::Column, on_axis, 0.30, b, turns, angle);
                    if !near(self, at) {
                        match self.add_fixture(FixtureKind::Column, at, 0.30, b, turns) {
                            Some(id) => {
                                if angle.abs() > 1e-9
                                    && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                                {
                                    f.angle_rad = angle;
                                }
                                report.columns += 1;
                            }
                            None => report.skipped += 1,
                        }
                    }
                }
            }
        }
        // Beams along each wall between consecutive columns.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            let mut on: Vec<(f64, u64)> = self
                .structural_columns()
                .filter(|column| {
                    // A column flush with the lot line sits up to half its
                    // section off the wall's centre line.
                    let off = 0.06 + column.width.max(column.depth) / 2.0;
                    crate::domain::geometry::distance_to_segment(
                        column.center,
                        wall.start,
                        wall.end,
                    ) < off
                })
                .map(|column| (column.center.distance(wall.start), column.id))
                .filter(|(s, _)| *s <= length + 0.06)
                .collect();
            on.sort_by(|a, b| a.0.total_cmp(&b.0));
            let b = round_up(wall.thickness().max(0.12), 0.01).min(0.30);
            for pair in on.windows(2) {
                // Columns this close act together: a beam between them
                // would only lift one off its support.
                let span = pair[1].0 - pair[0].0;
                if span < 0.6 {
                    continue;
                }
                let h = round_up(span / 10.0, 0.05).max(0.30);
                if self.add_beam(pair[0].1, pair[1].1, b, h).is_ok() {
                    report.beams += 1;
                }
            }
        }
        // Slabs.
        let rooms: Vec<u64> = self.rooms.iter().map(|room| room.id).collect();
        for room in rooms {
            if !self.structure.slabs.iter().any(|slab| slab.room_id == room)
                && self.set_slab(room, SlabKind::Ribbed, None).is_ok()
            {
                report.slabs += 1;
            }
        }
        // Footings, sized by the loads.
        let columns: Vec<Fixture> = self.structural_columns().copied().collect();
        for column in columns {
            let has = self
                .fixtures
                .iter()
                .any(|f| f.kind == FixtureKind::Footing && f.contains(column.center));
            // A footing under a boundary column stays inside the lot, off
            // centre from its column (a boundary footing, see #144).
            let at = self.fit_inside_lot(
                FixtureKind::Footing,
                column.center,
                0.60,
                0.60,
                column.quarter_turns,
                column.angle_rad,
            );
            if !has
                && let Some(id) =
                    self.add_fixture(FixtureKind::Footing, at, 0.60, 0.60, column.quarter_turns)
            {
                if column.angle_rad.abs() > 1e-9
                    && let Some(f) = self.fixtures.iter_mut().find(|f| f.id == id)
                {
                    f.angle_rad = column.angle_rad;
                }
                report.footings += 1;
            }
        }
        self.size_footings();
        report
    }

    /// Resizes every footing to what the soil asks for (never smaller than
    /// 60 cm). Returns how many changed.
    pub fn size_footings(&mut self) -> usize {
        let report = self.structure_report();
        let mut changed = 0;
        for result in report.footings {
            if let Some(footing) = self.fixtures.iter_mut().find(|f| f.id == result.id)
                && ((footing.width - result.needed_width_m).abs() > 1e-6
                    || (footing.depth - result.needed_depth_m).abs() > 1e-6)
            {
                footing.width = result.needed_width_m;
                footing.depth = result.needed_depth_m;
                changed += 1;
            }
        }
        // A footing that grew across the lot line moves back inside it.
        let fitted: Vec<(u64, Point)> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .map(|f| {
                (
                    f.id,
                    self.fit_inside_lot(
                        f.kind,
                        f.center,
                        f.width,
                        f.depth,
                        f.quarter_turns,
                        f.angle_rad,
                    ),
                )
            })
            .collect();
        for (id, center) in fitted {
            if let Some(footing) = self.fixtures.iter_mut().find(|f| f.id == id) {
                footing.center = center;
            }
        }
        changed
    }
}
