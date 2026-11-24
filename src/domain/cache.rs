//! Content-fingerprinted caches for derived electrical and structural views so
//! panning, zooming and redrawing reuse the last computation instead of
//! recomputing marks, schedules and tags on every frame (B04).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use super::conduits::PlanMark;
use super::electrical::LoadSchedule;
use super::fixtures::Fixture;
use super::geometry::Point;
use super::model::{Project, Room, Wall};
use super::reinforcement::{ReinforcementReport, build_reinforcement_report};
use super::structure::{Beam, Slab, StructureReport};

const CACHE_SLOTS: usize = 8;

struct RingCache<T> {
    entries: Vec<(u64, Arc<T>)>,
}

impl<T> RingCache<T> {
    fn new() -> Self {
        Self {
            entries: Vec::with_capacity(CACHE_SLOTS),
        }
    }

    fn get_or_insert(&mut self, key: u64, compute: impl FnOnce() -> T) -> Arc<T> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            let entry = self.entries.remove(pos);
            let value = Arc::clone(&entry.1);
            self.entries.push(entry);
            return value;
        }
        let value = Arc::new(compute());
        if self.entries.len() >= CACHE_SLOTS {
            self.entries.remove(0);
        }
        self.entries.push((key, Arc::clone(&value)));
        value
    }
}

fn hash_f64(h: &mut DefaultHasher, v: f64) {
    let bits = if v == 0.0 { 0u64 } else { v.to_bits() };
    bits.hash(h);
}

fn hash_point(h: &mut DefaultHasher, p: Point) {
    hash_f64(h, p.x);
    hash_f64(h, p.y);
}

fn hash_wall(h: &mut DefaultHasher, wall: &Wall) {
    wall.id.hash(h);
    hash_point(h, wall.start);
    hash_point(h, wall.end);
    hash_f64(h, wall.thickness());
    hash_f64(h, wall.height_m.unwrap_or(-1.0));
    hash_f64(h, wall.height_end_m.unwrap_or(-1.0));
    if let Some(masonry) = &wall.masonry {
        true.hash(h);
        masonry.name.hash(h);
        for value in [
            masonry.block_width,
            masonry.block_length,
            masonry.block_height,
            masonry.joint,
            masonry.plaster,
        ] {
            hash_f64(h, value);
        }
    } else {
        false.hash(h);
    }
}

fn hash_room(h: &mut DefaultHasher, room: &Room) {
    room.id.hash(h);
    room.name.hash(h);
    room.wall_ids.hash(h);
}

fn hash_fixture(h: &mut DefaultHasher, f: &Fixture) {
    f.id.hash(h);
    f.kind.hash(h);
    hash_point(h, f.center);
    hash_f64(h, f.width);
    hash_f64(h, f.depth);
    f.quarter_turns.hash(h);
    hash_f64(h, f.angle_rad);
    if let Some(e) = &f.electrical {
        true.hash(h);
        e.circuit.hash(h);
        hash_f64(h, e.power_va);
        hash_f64(h, e.height_m);
    } else {
        false.hash(h);
    }
    if let Some(load) = f.structural_load_kn {
        true.hash(h);
        hash_f64(h, load);
    } else {
        false.hash(h);
    }
}

fn hash_beam(h: &mut DefaultHasher, b: &Beam) {
    b.id.hash(h);
    b.from.hash(h);
    b.to.hash(h);
    if let Some(s) = b.from_support {
        true.hash(h);
        match s {
            super::structure::BeamSupport::Column { id } => {
                0u8.hash(h);
                id.hash(h);
            }
            super::structure::BeamSupport::Beam { beam_id, t } => {
                1u8.hash(h);
                beam_id.hash(h);
                hash_f64(h, t);
            }
        }
    } else {
        false.hash(h);
    }
    if let Some(s) = b.to_support {
        true.hash(h);
        match s {
            super::structure::BeamSupport::Column { id } => {
                0u8.hash(h);
                id.hash(h);
            }
            super::structure::BeamSupport::Beam { beam_id, t } => {
                1u8.hash(h);
                beam_id.hash(h);
                hash_f64(h, t);
            }
        }
    } else {
        false.hash(h);
    }
    hash_f64(h, b.width_m);
    hash_f64(h, b.height_m);
}

fn hash_slab(h: &mut DefaultHasher, s: &Slab) {
    s.id.hash(h);
    s.room_id.hash(h);
    (s.kind as u8).hash(h);
    hash_f64(h, s.thickness_m);
}

fn electrical_fingerprint(project: &Project) -> u64 {
    let mut h = DefaultHasher::new();
    hash_f64(&mut h, project.wall_height);
    hash_f64(&mut h, project.electrical.voltage_v);
    hash_f64(&mut h, project.electrical.specific_voltage_v);
    (project.electrical.supply as u8).hash(&mut h);
    (project.electrical.utility as u8).hash(&mut h);
    if let Some(o) = project.electrical.custom_rules {
        true.hash(&mut h);
        for opt in [
            o.single_phase_max_kva,
            o.two_phase_max_kva,
            o.max_bt_kva,
            o.min_breaker_a,
            o.min_wire_mm2,
            o.general_demand_scale,
            o.specific_demand_scale,
        ] {
            opt.is_some().hash(&mut h);
            if let Some(v) = opt {
                hash_f64(&mut h, v);
            }
        }
    } else {
        false.hash(&mut h);
    }
    project.walls.len().hash(&mut h);
    for wall in &project.walls {
        hash_wall(&mut h, wall);
    }
    project.rooms.len().hash(&mut h);
    for room in &project.rooms {
        hash_room(&mut h, room);
    }
    project.fixtures.len().hash(&mut h);
    for fixture in &project.fixtures {
        if fixture.kind.is_electrical() {
            hash_fixture(&mut h, fixture);
        }
    }
    project.conduits.len().hash(&mut h);
    for conduit in &project.conduits {
        conduit.id.hash(&mut h);
        conduit.from.hash(&mut h);
        conduit.to.hash(&mut h);
        (conduit.run as u8).hash(&mut h);
    }
    h.finish()
}

fn structure_fingerprint(project: &Project) -> u64 {
    let mut h = DefaultHasher::new();
    hash_f64(&mut h, project.wall_height);
    let s = project.structure.settings;
    hash_f64(&mut h, s.fck_mpa);
    hash_f64(&mut h, s.soil_kpa);
    s.floors.hash(&mut h);
    hash_f64(&mut h, s.live_kn_m2);
    hash_f64(&mut h, s.finish_kn_m2);
    hash_f64(&mut h, s.masonry_kn_m3);
    s.per_room_live_loads.hash(&mut h);
    (s.occupancy as u8).hash(&mut h);
    s.frame_rigid_joints.hash(&mut h);
    s.frame_fixed_bases.hash(&mut h);
    project.walls.len().hash(&mut h);
    for wall in &project.walls {
        hash_wall(&mut h, wall);
    }
    project.rooms.len().hash(&mut h);
    for room in &project.rooms {
        hash_room(&mut h, room);
    }
    project.fixtures.len().hash(&mut h);
    for fixture in &project.fixtures {
        if fixture.kind.is_structural() {
            hash_fixture(&mut h, fixture);
        }
    }
    project.structure.beams.len().hash(&mut h);
    for beam in &project.structure.beams {
        hash_beam(&mut h, beam);
    }
    project.structure.slabs.len().hash(&mut h);
    for slab in &project.structure.slabs {
        hash_slab(&mut h, slab);
    }
    project.structure.wall_foundations.len().hash(&mut h);
    for f in &project.structure.wall_foundations {
        f.id.hash(&mut h);
        f.wall_id.hash(&mut h);
        (f.kind as u8).hash(&mut h);
        hash_f64(&mut h, f.width_m.unwrap_or(-1.0));
    }
    for opening in &project.openings {
        opening.id.hash(&mut h);
        opening.wall_id.hash(&mut h);
        (opening.kind as u8).hash(&mut h);
        hash_f64(&mut h, opening.t);
        hash_f64(&mut h, opening.width());
        hash_f64(&mut h, opening.height().unwrap_or(-1.0));
        hash_f64(&mut h, opening.sill());
    }
    if let Some(survey) = &project.structure.soil_survey {
        survey.readings.len().hash(&mut h);
        for r in &survey.readings {
            hash_f64(&mut h, r.depth_m);
            r.n.hash(&mut h);
            (r.soil as u8).hash(&mut h);
        }
        hash_f64(&mut h, survey.water_table_m.unwrap_or(-1.0));
        hash_f64(&mut h, survey.footing_depth_m);
        survey.name.hash(&mut h);
    }
    (project.structure.post_species as u8).hash(&mut h);
    project.structure.post_treated.hash(&mut h);
    let wind = project.structure.wind;
    hash_f64(&mut h, wind.v0_ms);
    wind.category.hash(&mut h);
    hash_f64(&mut h, wind.s1);
    hash_f64(&mut h, wind.s3);
    project.structure.roofs.len().hash(&mut h);
    for roof in &project.structure.roofs {
        roof.id.hash(&mut h);
        hash_point(&mut h, roof.min);
        hash_point(&mut h, roof.max);
        (roof.high_side as u8).hash(&mut h);
        (roof.tile as u8).hash(&mut h);
        (roof.timber as u8).hash(&mut h);
        roof.exposed.hash(&mut h);
        for v in [
            roof.high_m,
            roof.low_m,
            roof.beam_spacing_m,
            roof.purlin_spacing_m,
            roof.rafter_spacing_m,
        ] {
            hash_f64(&mut h, v);
        }
        if let Some((width, height)) = roof.beam_section_cm {
            hash_f64(&mut h, width);
            hash_f64(&mut h, height);
        }
        roof.extra_beam_positions_m.len().hash(&mut h);
        for position in &roof.extra_beam_positions_m {
            hash_f64(&mut h, *position);
        }
    }
    project.structure.balance_beams.len().hash(&mut h);
    for beam in &project.structure.balance_beams {
        beam.id.hash(&mut h);
        beam.column.hash(&mut h);
        beam.anchor.hash(&mut h);
        hash_f64(&mut h, beam.width_m);
    }
    // Include upper floors in the fingerprint so multi-storey load-path
    // caches invalidate when an upper storey changes.
    project.floors.active_floor_id.hash(&mut h);
    project.floors.levels.len().hash(&mut h);
    for level in &project.floors.levels {
        level.id.hash(&mut h);
        level.name.hash(&mut h);
        hash_f64(&mut h, level.elevation_m);
        hash_f64(&mut h, level.ceiling_height_m);
        if Some(level.id) != project.floors.active_floor_id {
            level.data.walls.len().hash(&mut h);
            for wall in &level.data.walls {
                hash_wall(&mut h, wall);
            }
            level.data.rooms.len().hash(&mut h);
            for room in &level.data.rooms {
                hash_room(&mut h, room);
            }
            for fixture in &level.data.fixtures {
                if fixture.kind.is_structural() {
                    hash_fixture(&mut h, fixture);
                }
            }
            for beam in &level.data.beams {
                hash_beam(&mut h, beam);
            }
            for slab in &level.data.slabs {
                hash_slab(&mut h, slab);
            }
        }
    }
    h.finish()
}

type CacheSlot<T> = OnceLock<Mutex<RingCache<T>>>;
type ElectricalTag = (Point, String);

impl Project {
    /// Cached [`Project::load_schedule`], reused across frames until electrical
    /// inputs change.
    pub fn cached_load_schedule(&self) -> Arc<LoadSchedule> {
        static CACHE: CacheSlot<LoadSchedule> = OnceLock::new();
        let key = electrical_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.load_schedule())
    }

    /// Cached [`Project::conduit_marks`], reused across frames until electrical
    /// inputs change.
    pub fn cached_conduit_marks(&self) -> Arc<Vec<PlanMark>> {
        static CACHE: CacheSlot<Vec<PlanMark>> = OnceLock::new();
        let key = electrical_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.conduit_marks())
    }

    /// Cached [`Project::electrical_tags`], reused across frames until
    /// electrical inputs change.
    pub fn cached_electrical_tags(&self) -> Arc<Vec<ElectricalTag>> {
        static CACHE: CacheSlot<Vec<ElectricalTag>> = OnceLock::new();
        let key = electrical_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.electrical_tags())
    }

    /// Cached [`Project::structure_marks`], reused across frames until
    /// structural inputs change.
    pub fn cached_structure_marks(&self) -> Arc<Vec<PlanMark>> {
        static CACHE: CacheSlot<Vec<PlanMark>> = OnceLock::new();
        let key = structure_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.structure_marks())
    }

    /// Cached [`Project::superstructure_marks`] for the **Estrutura** tab
    /// (`Planta de Forma`: columns, upper beams, slabs, loads, roofs, timber).
    pub fn cached_superstructure_marks(&self) -> Arc<Vec<PlanMark>> {
        static CACHE: CacheSlot<Vec<PlanMark>> = OnceLock::new();
        let key = structure_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.superstructure_marks())
    }

    /// Cached [`Project::foundation_marks`] for the **Fundações** tab
    /// (`Planta de Fundações`: columns, footings, balance beams, baldrames,
    /// strip footings and timber post bases).
    pub fn cached_foundation_marks(&self) -> Arc<Vec<PlanMark>> {
        static CACHE: CacheSlot<Vec<PlanMark>> = OnceLock::new();
        let key = structure_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.foundation_marks())
    }

    /// Cached [`Project::structure_report`], reused across frames until
    /// structural inputs change.
    pub fn cached_structure_report(&self) -> Arc<StructureReport> {
        static CACHE: CacheSlot<StructureReport> = OnceLock::new();
        let key = structure_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.structure_report())
    }

    /// Cached reinforcement proposal and steel schedule, derived from the
    /// same structural fingerprint as the analysis report.
    pub fn cached_reinforcement_report(&self) -> Arc<ReinforcementReport> {
        static CACHE: CacheSlot<ReinforcementReport> = OnceLock::new();
        let key = structure_fingerprint(self);
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || {
            let structure = self.structure_report();
            build_reinforcement_report(self, &structure)
        })
    }

    /// Cached [`Project::audit_dependencies`], reused across frames until
    /// geometry, openings, electrical or structural inputs change.
    pub fn cached_dependency_audit(&self) -> Arc<super::dependencies::DependencyAudit> {
        static CACHE: CacheSlot<super::dependencies::DependencyAudit> = OnceLock::new();
        let mut h = DefaultHasher::new();
        electrical_fingerprint(self).hash(&mut h);
        structure_fingerprint(self).hash(&mut h);
        self.openings.len().hash(&mut h);
        for op in &self.openings {
            op.id.hash(&mut h);
            op.wall_id.hash(&mut h);
            hash_f64(&mut h, op.t);
            hash_f64(&mut h, op.width());
        }
        self.fixtures.len().hash(&mut h);
        for f in &self.fixtures {
            if f.kind.is_plumbing() {
                hash_fixture(&mut h, f);
            }
        }
        let key = h.finish();
        let cache = CACHE.get_or_init(|| Mutex::new(RingCache::new()));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert(key, || self.audit_dependencies())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    #[test]
    fn derived_caches_reuse_results_across_frames_and_invalidate_on_edit() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        project.generate_structure();
        project.generate_electrical_points();
        let (pw, pd) = FixtureKind::Panel.default_size();
        project
            .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
            .unwrap();
        project.assign_circuits();
        project.generate_conduits().unwrap();

        // Repeated calls across frames return the exact cached Arc without recomputing.
        let s1 = project.cached_load_schedule();
        let s2 = project.cached_load_schedule();
        assert!(Arc::ptr_eq(&s1, &s2));

        let c1 = project.cached_conduit_marks();
        let c2 = project.cached_conduit_marks();
        assert!(Arc::ptr_eq(&c1, &c2));

        let t1 = project.cached_electrical_tags();
        let t2 = project.cached_electrical_tags();
        assert!(Arc::ptr_eq(&t1, &t2));

        let m1 = project.cached_structure_marks();
        let m2 = project.cached_structure_marks();
        assert!(Arc::ptr_eq(&m1, &m2));

        let r1 = project.cached_structure_report();
        let r2 = project.cached_structure_report();
        assert!(Arc::ptr_eq(&r1, &r2));

        let a1 = project.cached_reinforcement_report();
        let a2 = project.cached_reinforcement_report();
        assert!(Arc::ptr_eq(&a1, &a2));

        // Mutating an electrical setting invalidates electrical caches while
        // leaving structural caches warm.
        project.electrical.voltage_v = 220.0;
        let s3 = project.cached_load_schedule();
        assert!(!Arc::ptr_eq(&s1, &s3));
        let m3 = project.cached_structure_marks();
        assert!(Arc::ptr_eq(&m1, &m3));

        let tank = project
            .add_fixture(FixtureKind::WaterTank, Point::new(2.0, 1.5), 1.20, 1.20, 0)
            .unwrap();
        let loaded = project.cached_structure_report();
        assert!(!Arc::ptr_eq(&r1, &loaded));
        project
            .fixtures
            .iter_mut()
            .find(|fixture| fixture.id == tank)
            .unwrap()
            .structural_load_kn = Some(15.0);
        let changed_load = project.cached_structure_report();
        assert!(!Arc::ptr_eq(&loaded, &changed_load));

        // Mutating a structural parameter invalidates structural caches.
        project.structure.settings.live_kn_m2 = 3.0;
        let r3 = project.cached_structure_report();
        assert!(!Arc::ptr_eq(&r1, &r3));
        let a3 = project.cached_reinforcement_report();
        assert!(!Arc::ptr_eq(&a1, &a3));
    }
}
