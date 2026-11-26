//! Synchronized multi-view tabs and multi-project workspace management.
//!
//! A single [`Project`] (together with its undo [`History`], `file_path`,
//! `dirty` flag, [`PlanLog`] and [`DependencySyncReport`]) is the sole source
//! of truth per open project (`ProjectId`). Multiple [`ViewTab`]s can link to
//! the same `ProjectId`, each with its own visual state (discipline, floor,
//! hidden layer overrides, selection, zoom, pan, measurements and split pane)
//! without duplicating project data in memory.
//!
//! Independent projects still coexist in separate `ProjectId`s: inactive
//! projects live in `self.projects`, while the active project's fields live
//! directly in `FloorPlanApp` so every tool, panel and MCP command operates on
//! the single live model.

mod closing;
mod creation;
mod queries;
mod switching;
#[cfg(test)]
mod synced_tabs_tests;
#[cfg(test)]
mod tests;
mod workspace;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;

use super::plan_sync::PlanLog;
use super::{FloorPlanApp, Selection, Tool};
use crate::domain::dependencies::DependencySyncReport;
use crate::domain::discipline::{Discipline, WorkspacePane};
use crate::domain::geometry::Point;
use crate::domain::history::History;
use crate::domain::model::Project;

/// Runtime identifier of an open shared project (single source of truth).
pub(crate) type ProjectId = u64;

/// Runtime identifier of an open view tab.
pub(crate) type TabId = u64;

/// Which side of a split view holds a tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum TabPane {
    #[default]
    Left,
    Right,
}

impl TabPane {
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    pub(crate) fn to_workspace(self) -> WorkspacePane {
        match self {
            Self::Left => WorkspacePane::Left,
            Self::Right => WorkspacePane::Right,
        }
    }

    pub(crate) fn from_workspace(pane: WorkspacePane) -> Self {
        match pane {
            WorkspacePane::Left => Self::Left,
            WorkspacePane::Right => Self::Right,
        }
    }
}

/// Parked state of an open project that is not currently active.
pub(crate) struct SharedProject {
    pub(crate) id: ProjectId,
    pub(crate) project: Project,
    pub(crate) history: History,
    pub(crate) file_path: Option<PathBuf>,
    pub(crate) dirty: bool,
    pub(crate) recovery_pending: bool,
    pub(crate) plan_log: PlanLog,
    pub(crate) sync_report: DependencySyncReport,
}

impl SharedProject {
    pub(crate) fn holding(
        id: ProjectId,
        project: Project,
        file_path: Option<PathBuf>,
        dirty: bool,
    ) -> Self {
        Self {
            id,
            project,
            history: History::new(),
            file_path,
            dirty,
            recovery_pending: false,
            plan_log: PlanLog::default(),
            sync_report: DependencySyncReport::default(),
        }
    }
}

/// Per-tab visual state referencing a shared [`ProjectId`] without copying data.
pub(crate) struct ViewTab {
    pub(crate) id: TabId,
    pub(crate) project_id: ProjectId,
    pub(crate) title_override: Option<String>,
    pub(crate) discipline: Discipline,
    pub(crate) floor_id: Option<u64>,
    pub(crate) hidden_layers: Vec<u64>,
    pub(crate) selection: Option<Selection>,
    pub(crate) selection_extra: Vec<Selection>,
    pub(crate) scale: f32,
    pub(crate) pan: egui::Vec2,
    pub(crate) fit_pending: bool,
    pub(crate) measurements: Vec<(Point, Point)>,
    pub(crate) constraint_texts: HashMap<u64, String>,
    pub(crate) pane: TabPane,
}

impl ViewTab {
    pub(crate) fn new(id: TabId, project_id: ProjectId, discipline: Discipline) -> Self {
        Self {
            id,
            project_id,
            title_override: None,
            discipline,
            floor_id: None,
            hidden_layers: Vec::new(),
            selection: None,
            selection_extra: Vec::new(),
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            fit_pending: true,
            measurements: Vec::new(),
            constraint_texts: HashMap::new(),
            pane: TabPane::Left,
        }
    }
}

/// What the tab strip and MCP report for one open tab.
#[derive(Clone, Debug)]
pub(crate) struct TabInfo {
    pub(crate) tab_id: TabId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: String,
    pub(crate) discipline: Discipline,
    pub(crate) floor_id: Option<u64>,
    pub(crate) floor_name: String,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
    pub(crate) pane: TabPane,
    /// Displayed on screen right now (either active or the side-by-side tab).
    pub(crate) shown: bool,
    /// True when at least two open tabs share this tab's `project_id`.
    pub(crate) linked: bool,
}

/// Snapshot of a companion split-view tab ready to paint.
pub(crate) struct SidePlanView<'a> {
    pub(crate) base_project: Cow<'a, Project>,
    pub(crate) shown_project: Cow<'a, Project>,
    pub(crate) discipline: Discipline,
    pub(crate) floor_name: String,
    #[allow(dead_code)]
    pub(crate) linked_to_active: bool,
}

pub(crate) fn same_file_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(ca), Ok(cb)) = (std::fs::canonicalize(a), std::fs::canonicalize(b))
        && ca == cb
    {
        return true;
    }
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .replace('/', "\\")
            .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn base_title_of(project: &Project, file_path: Option<&Path>) -> String {
    let name = project.name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    file_path
        .and_then(|path| path.file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Sem título".to_owned())
}

fn format_tab_title(
    project: &Project,
    file_path: Option<&Path>,
    title_override: Option<&str>,
    discipline: Discipline,
    linked: bool,
) -> String {
    if let Some(custom) = title_override.map(str::trim).filter(|s| !s.is_empty()) {
        return custom.to_owned();
    }
    let base = base_title_of(project, file_path);
    if linked || !discipline.is_all() {
        format!("{base} · {}", discipline.label())
    } else {
        base
    }
}

fn adjust_reordered_index(idx: usize, from: usize, to: usize) -> usize {
    if idx == from {
        to
    } else if from < to && idx > from && idx <= to {
        idx - 1
    } else if to < from && idx >= to && idx < from {
        idx + 1
    } else {
        idx
    }
}

fn floor_name_for(project: &Project, floor_id: Option<u64>) -> String {
    if let Some(id) = floor_id
        && let Some(level) = project.floors.levels.iter().find(|l| l.id == id)
    {
        return level.name.clone();
    }
    project
        .active_floor()
        .map(|l| l.name.clone())
        .unwrap_or_else(|| "Térreo".to_owned())
}

impl FloorPlanApp {
    fn alloc_project_id(&mut self) -> ProjectId {
        let id = self.next_project_id;
        self.next_project_id += 1;
        id
    }

    fn alloc_tab_id(&mut self) -> TabId {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }

    /// Moves the active shared project out into a [`SharedProject`].
    fn take_shared_project(&mut self) -> SharedProject {
        SharedProject {
            id: self.active_project_id,
            project: std::mem::take(&mut self.project),
            history: std::mem::replace(&mut self.history, History::new()),
            file_path: self.file_path.take(),
            dirty: std::mem::take(&mut self.dirty),
            recovery_pending: std::mem::take(&mut self.recovery_pending),
            plan_log: std::mem::take(&mut self.plan_log),
            sync_report: std::mem::take(&mut self.sync_report),
        }
    }

    /// Installs `shared` as the active live project.
    fn put_shared_project(&mut self, shared: SharedProject) {
        self.active_project_id = shared.id;
        self.project = shared.project;
        self.history = shared.history;
        self.file_path = shared.file_path;
        self.dirty = shared.dirty;
        self.recovery_pending = shared.recovery_pending;
        self.plan_log = shared.plan_log;
        self.sync_report = shared.sync_report;
    }

    /// Moves the active tab's visual state out into a [`ViewTab`].
    fn take_view_tab(&mut self) -> ViewTab {
        ViewTab {
            id: self.active_tab_id,
            project_id: self.active_project_id,
            title_override: self.active_tab_title.take(),
            discipline: self.discipline,
            floor_id: self.active_tab_floor_id.take(),
            hidden_layers: std::mem::take(&mut self.active_hidden_layers),
            selection: self.selection.take(),
            selection_extra: std::mem::take(&mut self.selection_extra),
            scale: self.scale,
            pan: self.pan,
            fit_pending: self.fit_pending,
            measurements: std::mem::take(&mut self.measurements),
            constraint_texts: std::mem::take(&mut self.constraint_texts),
            pane: self.active_pane,
        }
    }

    /// Installs `view` as the active tab's visual state.
    fn put_view_tab(&mut self, view: ViewTab) {
        self.active_tab_id = view.id;
        self.active_tab_title = view.title_override;
        self.discipline = view.discipline;
        self.active_tab_floor_id = view.floor_id;
        self.active_hidden_layers = view.hidden_layers;
        self.selection = view.selection;
        self.selection_extra = view.selection_extra;
        self.scale = view.scale;
        self.pan = view.pan;
        self.fit_pending = view.fit_pending;
        self.measurements = view.measurements;
        self.constraint_texts = view.constraint_texts;
        self.active_pane = view.pane;
        self.last_canvas_center = None;
        self.prune_selection_for_discipline();
    }

    /// Ensures the active project's current floor matches the active tab's
    /// requested `floor_id` without polluting undo history or dirty status.
    fn align_active_floor_to_tab(&mut self) {
        if let Some(target_floor) = self.active_tab_floor_id {
            if self.project.floors.active_floor_id != Some(target_floor)
                && self
                    .project
                    .floors
                    .levels
                    .iter()
                    .any(|l| l.id == target_floor)
            {
                let _ = self.project.switch_floor(target_floor);
            }
        } else {
            self.active_tab_floor_id = self.project.floors.active_floor_id;
        }
    }

    /// Drops stale selection references in every inactive tab linked to the
    /// active project (called after undo/redo or deletions).
    pub(crate) fn revalidate_linked_tab_selections(&mut self) {
        let pid = self.active_project_id;
        for view in self.tabs.iter_mut().flatten() {
            if view.project_id != pid {
                continue;
            }
            if let Some(sel) = view.selection
                && !self.project.element_exists(sel)
            {
                view.selection = None;
            }
            view.selection_extra
                .retain(|item| self.project.element_exists(*item));
            if view.selection.is_none() && !view.selection_extra.is_empty() {
                view.selection = Some(view.selection_extra.remove(0));
            }
        }
    }

    /// Ends whatever is half done on the plan before another tab shows:
    /// drags are committed, drafts dropped. The selection stays with its tab.
    fn settle_before_switch(&mut self) {
        self.commit_item_drag();
        self.ai_activity.forget_highlight();
        self.cancel_creation();
        self.box_select = None;
        self.item_drag = None;
        self.vertex_drag = None;
        self.drag_initial_project = None;
        self.pending_room = None;
        self.zoom_target = None;
        self.tool = Tool::Move;
    }
}
