//! Structurally-shared, memory-bounded undo/redo history (T-04, #84).
//!
//! Instead of deep-cloning every flat vector and metadata blob on each edit,
//! every [`ProjectSnapshot`] stores primary plan collections in persistent
//! 1024-ID [`ChunkedStore`]s (`Arc<Vec<(u64, T)>>`) and secondary project
//! metadata in copy-on-write [`Shared`] handles, sharing unchanged chunks and
//! metadata across snapshots with O(1) `Arc` clones and per-element
//! [`RevisionSnapshot`] tracking.

use std::collections::BTreeSet;

use super::{
    blocks::{BlockDefinition, BlockInstance},
    circulation::CirculationElement,
    conduits::Conduit,
    dimensions::DimensionEntity,
    discipline::WorkspaceTab,
    doc_style::DocStyle,
    drawing::Drawing,
    electrical::ElectricalSettings,
    fixtures::Fixture,
    floors::ProjectFloors,
    guid::GuidRegistry,
    layers::Layer,
    library::LibraryItem,
    masonry::Masonry,
    model::{Constraint, Group, Lot, Opening, Project, Room, Terrain, Wall},
    notes::AnnotationNote,
    schedules::ScheduleConfig,
    setbacks::Setbacks,
    sheet::SheetLayout,
    sheet_set::SheetSet,
    storage::{ChunkedStore, Shared, approximate_value_bytes},
    structure::ProjectStructure,
    underlay::ImageUnderlay,
    views::ProjectViews,
    xref::ExternalReference,
};

/// Maximum number of undo steps retained.
const MAX_HISTORY_STEPS: usize = 100;
/// Soft incremental heap budget for retained undo snapshots (128 MiB).
const MAX_HISTORY_BYTES: usize = 128 << 20;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct ProjectMetadata {
    version: u32,
    name: String,
    terrain: Option<Terrain>,
    lots: Vec<Lot>,
    setbacks: Setbacks,
    masonry: Masonry,
    wall_height: f64,
    layers: Vec<Layer>,
    active_layer_id: Option<u64>,
    sheet: SheetLayout,
    schedules: ScheduleConfig,
    doc_style: DocStyle,
    views: ProjectViews,
    sheet_set: SheetSet,
    groups: Vec<Group>,
    constraints: Vec<Constraint>,
    floors: ProjectFloors,
    library: Vec<LibraryItem>,
    blocks: Vec<BlockDefinition>,
    block_instances: Vec<BlockInstance>,
    structure: ProjectStructure,
    electrical: ElectricalSettings,
    underlays: Vec<ImageUnderlay>,
    xrefs: Vec<ExternalReference>,
    workspace_tabs: Vec<WorkspaceTab>,
    guids: GuidRegistry,
    next_id: u64,
}

impl ProjectMetadata {
    fn from_project(project: &Project) -> Self {
        Self {
            version: project.version,
            name: project.name.clone(),
            terrain: project.terrain.clone(),
            lots: project.lots.clone(),
            setbacks: project.setbacks,
            masonry: project.masonry.clone(),
            wall_height: project.wall_height,
            layers: project.layers.clone(),
            active_layer_id: project.active_layer_id,
            sheet: project.sheet.clone(),
            schedules: project.schedules.clone(),
            doc_style: project.doc_style.clone(),
            views: project.views.clone(),
            sheet_set: project.sheet_set.clone(),
            groups: project.groups.clone(),
            constraints: project.constraints.clone(),
            floors: project.floors.clone(),
            library: project.library.clone(),
            blocks: project.blocks.clone(),
            block_instances: project.block_instances.clone(),
            structure: project.structure.clone(),
            electrical: project.electrical,
            underlays: project.underlays.clone(),
            xrefs: project.xrefs.clone(),
            workspace_tabs: project.workspace_tabs.clone(),
            guids: project.guids.clone(),
            next_id: project.next_id(),
        }
    }

    fn matches_project(&self, project: &Project) -> bool {
        self.version == project.version
            && self.name == project.name
            && self.next_id == project.next_id()
            && self.wall_height == project.wall_height
            && self.active_layer_id == project.active_layer_id
            && self.doc_style == project.doc_style
            && self.setbacks == project.setbacks
            && self.electrical == project.electrical
            && self.terrain == project.terrain
            && self.lots == project.lots
            && self.masonry == project.masonry
            && self.layers == project.layers
            && self.sheet == project.sheet
            && self.schedules == project.schedules
            && self.views == project.views
            && self.sheet_set == project.sheet_set
            && self.groups == project.groups
            && self.constraints == project.constraints
            && self.floors == project.floors
            && self.library == project.library
            && self.blocks == project.blocks
            && self.block_instances == project.block_instances
            && self.structure == project.structure
            && self.underlays == project.underlays
            && self.xrefs == project.xrefs
            && self.workspace_tabs == project.workspace_tabs
            && self.guids == project.guids
    }

    fn approx_bytes(&self) -> usize {
        // Count nested floor data, embedded images and resources without
        // allocating a second serialized copy of potentially large metadata.
        // The multiplier allows for container and allocation overhead.
        std::mem::size_of::<Self>().saturating_add(approximate_value_bytes(self))
    }
}

/// Structurally-shared snapshot of a [`Project`].
#[derive(Clone, Debug)]
pub struct ProjectSnapshot {
    walls: ChunkedStore<Wall>,
    rooms: ChunkedStore<Room>,
    openings: ChunkedStore<Opening>,
    fixtures: ChunkedStore<Fixture>,
    drawings: ChunkedStore<Drawing>,
    dimensions: ChunkedStore<DimensionEntity>,
    notes: ChunkedStore<AnnotationNote>,
    circulations: ChunkedStore<CirculationElement>,
    conduits: ChunkedStore<Conduit>,
    metadata: Shared<ProjectMetadata>,
    incremental_bytes: usize,
}

impl ProjectSnapshot {
    fn retained_bytes(&self) -> usize {
        self.walls.incremental_bytes_over(None)
            + self.rooms.incremental_bytes_over(None)
            + self.openings.incremental_bytes_over(None)
            + self.fixtures.incremental_bytes_over(None)
            + self.drawings.incremental_bytes_over(None)
            + self.dimensions.incremental_bytes_over(None)
            + self.notes.incremental_bytes_over(None)
            + self.circulations.incremental_bytes_over(None)
            + self.conduits.incremental_bytes_over(None)
            + self.metadata.approx_bytes()
    }

    /// Captures `project`, sharing every unchanged 1024-ID chunk and unchanged
    /// metadata block with `base` via O(1) `Arc::clone`.
    pub fn from_project_sharing(project: &Project, base: Option<&Self>) -> Self {
        let walls =
            ChunkedStore::from_slice_sharing(&project.walls, |w| w.id, base.map(|b| &b.walls));
        let rooms =
            ChunkedStore::from_slice_sharing(&project.rooms, |r| r.id, base.map(|b| &b.rooms));
        let openings = ChunkedStore::from_slice_sharing(
            &project.openings,
            |o| o.id,
            base.map(|b| &b.openings),
        );
        let fixtures = ChunkedStore::from_slice_sharing(
            &project.fixtures,
            |f| f.id,
            base.map(|b| &b.fixtures),
        );
        let drawings = ChunkedStore::from_slice_sharing(
            &project.drawings,
            |d| d.id,
            base.map(|b| &b.drawings),
        );
        let dimensions = ChunkedStore::from_slice_sharing(
            &project.dimensions,
            |d| d.id,
            base.map(|b| &b.dimensions),
        );
        let notes =
            ChunkedStore::from_slice_sharing(&project.notes, |n| n.id, base.map(|b| &b.notes));
        let circulations = ChunkedStore::from_slice_sharing(
            &project.circulations,
            |c| c.id,
            base.map(|b| &b.circulations),
        );
        let conduits = ChunkedStore::from_slice_sharing(
            &project.conduits,
            |c| c.id,
            base.map(|b| &b.conduits),
        );

        let (metadata, meta_bytes) = match base {
            Some(b) if b.metadata.matches_project(project) => (b.metadata.clone(), 0),
            _ => {
                let meta = ProjectMetadata::from_project(project);
                let bytes = meta.approx_bytes();
                (Shared::new(meta), bytes)
            }
        };

        let incremental_bytes = walls.incremental_bytes_over(base.map(|b| &b.walls))
            + rooms.incremental_bytes_over(base.map(|b| &b.rooms))
            + openings.incremental_bytes_over(base.map(|b| &b.openings))
            + fixtures.incremental_bytes_over(base.map(|b| &b.fixtures))
            + drawings.incremental_bytes_over(base.map(|b| &b.drawings))
            + dimensions.incremental_bytes_over(base.map(|b| &b.dimensions))
            + notes.incremental_bytes_over(base.map(|b| &b.notes))
            + circulations.incremental_bytes_over(base.map(|b| &b.circulations))
            + conduits.incremental_bytes_over(base.map(|b| &b.conduits))
            + meta_bytes;

        Self {
            walls,
            rooms,
            openings,
            fixtures,
            drawings,
            dimensions,
            notes,
            circulations,
            conduits,
            metadata,
            incremental_bytes,
        }
    }

    /// Reconstructs the exact [`Project`] state from this snapshot.
    pub fn to_project(&self) -> Project {
        let meta = &*self.metadata;
        let mut project = Project::default();
        project.version = meta.version;
        project.name = meta.name.clone();
        project.walls = self.walls.to_vec();
        project.rooms = self.rooms.to_vec();
        project.openings = self.openings.to_vec();
        project.terrain = meta.terrain.clone();
        project.lots = meta.lots.clone();
        project.setbacks = meta.setbacks;
        project.fixtures = self.fixtures.to_vec();
        project.masonry = meta.masonry.clone();
        project.wall_height = meta.wall_height;
        project.drawings = self.drawings.to_vec();
        project.layers = meta.layers.clone();
        project.active_layer_id = meta.active_layer_id;
        project.dimensions = self.dimensions.to_vec();
        project.notes = self.notes.to_vec();
        project.sheet = meta.sheet.clone();
        project.schedules = meta.schedules.clone();
        project.doc_style = meta.doc_style.clone();
        project.views = meta.views.clone();
        project.sheet_set = meta.sheet_set.clone();
        project.groups = meta.groups.clone();
        project.constraints = meta.constraints.clone();
        project.circulations = self.circulations.to_vec();
        project.floors = meta.floors.clone();
        project.library = meta.library.clone();
        project.blocks = meta.blocks.clone();
        project.block_instances = meta.block_instances.clone();
        project.conduits = self.conduits.to_vec();
        project.structure = meta.structure.clone();
        project.electrical = meta.electrical;
        project.underlays = meta.underlays.clone();
        project.xrefs = meta.xrefs.clone();
        project.workspace_tabs = meta.workspace_tabs.clone();
        project.guids = meta.guids.clone();
        project.set_next_id(meta.next_id);
        project
    }

    /// Returns all element IDs that changed between `before` and `self` across
    /// every chunked collection in O(changed chunks).
    #[allow(dead_code)]
    pub fn changed_ids_since(&self, before: &Self) -> BTreeSet<u64> {
        let mut out = BTreeSet::new();
        out.extend(
            self.walls
                .revisions()
                .changed_since(before.walls.revisions()),
        );
        out.extend(
            self.rooms
                .revisions()
                .changed_since(before.rooms.revisions()),
        );
        out.extend(
            self.openings
                .revisions()
                .changed_since(before.openings.revisions()),
        );
        out.extend(
            self.fixtures
                .revisions()
                .changed_since(before.fixtures.revisions()),
        );
        out.extend(
            self.drawings
                .revisions()
                .changed_since(before.drawings.revisions()),
        );
        out.extend(
            self.dimensions
                .revisions()
                .changed_since(before.dimensions.revisions()),
        );
        out.extend(
            self.notes
                .revisions()
                .changed_since(before.notes.revisions()),
        );
        out.extend(
            self.circulations
                .revisions()
                .changed_since(before.circulations.revisions()),
        );
        out.extend(
            self.conduits
                .revisions()
                .changed_since(before.conduits.revisions()),
        );
        out
    }

    /// Number of 1024-ID chunks physically shared (`Arc::ptr_eq`) with `other`.
    #[allow(dead_code)]
    pub fn shared_chunks_with(&self, other: &Self) -> usize {
        self.walls.shared_chunks_with(&other.walls)
            + self.rooms.shared_chunks_with(&other.rooms)
            + self.openings.shared_chunks_with(&other.openings)
            + self.fixtures.shared_chunks_with(&other.fixtures)
            + self.drawings.shared_chunks_with(&other.drawings)
            + self.dimensions.shared_chunks_with(&other.dimensions)
            + self.notes.shared_chunks_with(&other.notes)
            + self.circulations.shared_chunks_with(&other.circulations)
            + self.conduits.shared_chunks_with(&other.conduits)
    }

    #[allow(dead_code)]
    pub fn shares_metadata_with(&self, other: &Self) -> bool {
        self.metadata.same_allocation(&other.metadata)
    }
}

pub struct History {
    undo: Vec<ProjectSnapshot>,
    redo: Vec<ProjectSnapshot>,
    max_steps: usize,
    max_bytes: usize,
    undo_bytes: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_steps: MAX_HISTORY_STEPS,
            max_bytes: MAX_HISTORY_BYTES,
            undo_bytes: 0,
        }
    }

    pub fn record(&mut self, previous: Project) {
        let snap = ProjectSnapshot::from_project_sharing(&previous, self.undo.last());
        self.undo_bytes = self.undo_bytes.saturating_add(snap.incremental_bytes);
        self.undo.push(snap);
        while self.undo.len() > self.max_steps
            || (self.undo.len() > 1 && self.undo_bytes > self.max_bytes)
        {
            let removed = self.undo.remove(0);
            self.undo_bytes = self.undo_bytes.saturating_sub(removed.incremental_bytes);
            // Chunks shared with the evicted base are still retained by the new
            // first snapshot. Charge that base in full before testing the budget.
            if let Some(first) = self.undo.first_mut() {
                self.undo_bytes = self.undo_bytes.saturating_sub(first.incremental_bytes);
                first.incremental_bytes = first.retained_bytes();
                self.undo_bytes = self.undo_bytes.saturating_add(first.incremental_bytes);
            }
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: &mut Project) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(previous.incremental_bytes);
        let current_snap = ProjectSnapshot::from_project_sharing(current, Some(&previous));
        self.redo.push(current_snap);
        *current = previous.to_project();
        true
    }

    pub fn redo(&mut self, current: &mut Project) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let current_snap = ProjectSnapshot::from_project_sharing(current, self.undo.last());
        self.undo_bytes = self
            .undo_bytes
            .saturating_add(current_snap.incremental_bytes);
        self.undo.push(current_snap);
        *current = next.to_project();
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.undo_bytes = 0;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[allow(dead_code)]
    pub fn estimated_bytes(&self) -> usize {
        self.undo_bytes
    }

    #[allow(dead_code)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn evicting_a_snapshot_keeps_shared_base_memory_charged() {
        let mut project = Project::default();
        project.name = "Project metadata".repeat(1000);
        project.add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
        let retained = ProjectSnapshot::from_project_sharing(&project, None).retained_bytes();
        let mut history = History::new();
        history.max_steps = 2;
        for _ in 0..6 {
            history.record(project.clone());
        }
        assert_eq!(history.undo_len(), 2);
        assert_eq!(history.estimated_bytes(), retained);
        history.max_bytes = retained - 1;
        history.record(project.clone());
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.estimated_bytes(), retained);
        assert!(history.undo(&mut project));
        assert!(history.redo(&mut project));
    }

    #[test]
    fn history_accounts_for_embedded_image_payloads_in_metadata() {
        let mut project = Project::default();
        let payload = "a".repeat(65_536);
        project.underlays.push(ImageUnderlay {
            id: 1,
            name: "Fundo".to_owned(),
            source_path: String::new(),
            page: 1,
            pixel_width: 1,
            pixel_height: 1,
            origin: Point::new(0.0, 0.0),
            width_m: 1.0,
            rotation_deg: 0.0,
            opacity: 0.5,
            visible: true,
            locked: false,
            embedded_png_base64: Some(payload),
        });
        let mut history = History::new();
        history.max_bytes = 200_000;
        for i in 0..4 {
            project.name = format!("Revision {i}");
            history.record(project.clone());
        }
        assert_eq!(history.undo_len(), 1);
        assert!(history.estimated_bytes() >= 65_536);
    }

    #[test]
    fn history_structurally_shares_chunks_and_undoes_large_project_fast() {
        // Build a 1,500-wall project spanning 2 full 1024-ID chunks.
        let mut project = Project::default();
        for i in 0..1500usize {
            let row = (i / 50) as f64;
            let col = (i % 50) as f64;
            project.add_wall(
                Point::new(col * 3.0, row * 3.0),
                Point::new(col * 3.0 + 2.5, row * 3.0),
            );
        }
        assert_eq!(project.walls.len(), 1500);

        let mut history = History::new();
        // Record 10 incremental edits touching only the last wall (in chunk 1).
        for step in 1..=10u64 {
            let before = project.clone();
            history.record(before);
            if let Some(last) = project.walls.last_mut() {
                last.end.x += 0.1 * step as f64;
            }
        }

        // Consecutive snapshots must physically share chunk 0 (1024 walls) AND metadata!
        let s0 = &history.undo[0];
        let s1 = &history.undo[1];
        assert!(
            s1.shared_chunks_with(s0) >= 1,
            "unchanged 1024-ID wall chunk must be Arc-shared across undo steps"
        );
        assert!(
            s1.shares_metadata_with(s0),
            "unchanged project metadata must be Arc-shared across undo steps"
        );
        let changed = s1.changed_ids_since(s0);
        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed.iter().next().copied(),
            project.walls.last().map(|w| w.id)
        );

        // Undo and redo round-trip on 1,500 walls must preserve exact equality and stay fast.
        let tip = project.clone();
        let start = Instant::now();
        assert!(history.undo(&mut project));
        assert!(history.redo(&mut project));
        let elapsed = start.elapsed();
        assert_eq!(project, tip);
        assert!(
            elapsed.as_millis() < 50,
            "undo+redo on 1500 walls took {elapsed:?}"
        );
    }

    #[test]
    fn history_respects_memory_budget_eviction() {
        let mut history = History::new();
        history.max_bytes = 2_000;
        let mut project = Project::default();
        for i in 0..20 {
            let x = i as f64 * 2.0;
            project.add_wall(Point::new(x, 0.0), Point::new(x + 1.0, 0.0));
            history.record(project.clone());
        }
        assert!(history.can_undo());
        assert!(
            history.undo_len() < 20,
            "memory budget should evict oldest snapshots"
        );
    }
}
