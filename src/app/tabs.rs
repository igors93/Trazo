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

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;

use super::plan_sync::PlanLog;
use super::{FloorPlanApp, OpenResult, PendingAction, Selection, Tool};
use crate::domain::dependencies::DependencySyncReport;
use crate::domain::discipline::{Discipline, WorkspacePane, WorkspaceTab};
use crate::domain::geometry::Point;
use crate::domain::history::History;
use crate::domain::model::Project;
use crate::io::recovery;

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
    #[allow(dead_code)]
    pub(crate) tab_id: TabId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: String,
    pub(crate) discipline: Discipline,
    #[allow(dead_code)]
    pub(crate) floor_id: Option<u64>,
    #[allow(dead_code)]
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

    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_tab(&self) -> usize {
        self.active_tab
    }

    pub(crate) fn active_project_id(&self) -> ProjectId {
        self.active_project_id
    }

    /// Inactive tab's visual state.
    pub(crate) fn tab_view(&self, index: usize) -> Option<&ViewTab> {
        self.tabs.get(index).and_then(Option::as_ref)
    }

    /// Which shared project owns tab `index`.
    pub(crate) fn tab_project_id(&self, index: usize) -> Option<ProjectId> {
        if index == self.active_tab {
            Some(self.active_project_id)
        } else {
            self.tab_view(index).map(|view| view.project_id)
        }
    }

    /// Borrows the single live [`Project`] for `project_id` (zero copy).
    pub(crate) fn project_by_id(&self, project_id: ProjectId) -> Option<&Project> {
        if project_id == self.active_project_id {
            Some(&self.project)
        } else {
            self.projects.get(&project_id).map(|shared| &shared.project)
        }
    }

    /// Borrows the [`Project`] behind tab `index` (zero copy).
    pub(crate) fn tab_project(&self, index: usize) -> Option<&Project> {
        let pid = self.tab_project_id(index)?;
        self.project_by_id(pid)
    }

    pub(crate) fn project_file_path(&self, project_id: ProjectId) -> Option<&Path> {
        if project_id == self.active_project_id {
            self.file_path.as_deref()
        } else {
            self.projects
                .get(&project_id)
                .and_then(|shared| shared.file_path.as_deref())
        }
    }

    pub(crate) fn project_dirty(&self, project_id: ProjectId) -> bool {
        if project_id == self.active_project_id {
            self.dirty
        } else {
            self.projects
                .get(&project_id)
                .is_some_and(|shared| shared.dirty)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn project_revision(&self, project_id: ProjectId) -> u64 {
        if project_id == self.active_project_id {
            self.plan_log.revision
        } else {
            self.projects
                .get(&project_id)
                .map_or(0, |shared| shared.plan_log.revision)
        }
    }

    /// How many open tabs are linked to `project_id`.
    pub(crate) fn project_tab_count(&self, project_id: ProjectId) -> usize {
        (0..self.tabs.len())
            .filter(|&i| self.tab_project_id(i) == Some(project_id))
            .count()
    }

    /// Distinct open project IDs in tab appearance order.
    #[allow(dead_code)]
    pub(crate) fn open_project_ids(&self) -> Vec<ProjectId> {
        let mut ids = Vec::new();
        for i in 0..self.tabs.len() {
            if let Some(pid) = self.tab_project_id(i)
                && !ids.contains(&pid)
            {
                ids.push(pid);
            }
        }
        ids
    }

    /// The tabs, in order, for the tab strip and MCP inspection.
    pub(crate) fn tab_infos(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, slot)| match slot {
                Some(view) => {
                    let project = self
                        .project_by_id(view.project_id)
                        .expect("every view tab references an open project");
                    let path = self.project_file_path(view.project_id);
                    let linked = self.project_tab_count(view.project_id) > 1;
                    TabInfo {
                        tab_id: view.id,
                        project_id: view.project_id,
                        title: format_tab_title(
                            project,
                            path,
                            view.title_override.as_deref(),
                            view.discipline,
                            linked,
                        ),
                        discipline: view.discipline,
                        floor_id: view.floor_id.or(project.floors.active_floor_id),
                        floor_name: floor_name_for(project, view.floor_id),
                        dirty: self.project_dirty(view.project_id),
                        active: false,
                        pane: view.pane,
                        shown: self.split_tab == Some(index),
                        linked,
                    }
                }
                None => {
                    let linked = self.project_tab_count(self.active_project_id) > 1;
                    TabInfo {
                        tab_id: self.active_tab_id,
                        project_id: self.active_project_id,
                        title: format_tab_title(
                            &self.project,
                            self.file_path.as_deref(),
                            self.active_tab_title.as_deref(),
                            self.discipline,
                            linked,
                        ),
                        discipline: self.discipline,
                        floor_id: self
                            .active_tab_floor_id
                            .or(self.project.floors.active_floor_id),
                        floor_name: floor_name_for(&self.project, self.active_tab_floor_id),
                        dirty: self.dirty,
                        active: index == self.active_tab,
                        pane: self.active_pane,
                        shown: index == self.active_tab,
                        linked,
                    }
                }
            })
            .collect()
    }

    /// Live renderable view of an inactive companion tab in split view.
    ///
    /// When the companion tab shares the active project, it borrows
    /// `&self.project` directly so edits appear simultaneously on both sides.
    pub(crate) fn side_plan_view(&self, index: usize) -> Option<SidePlanView<'_>> {
        let view = self.tab_view(index)?;
        let source = self.project_by_id(view.project_id)?;
        let linked_to_active = view.project_id == self.active_project_id;
        let floor_name = floor_name_for(source, view.floor_id);
        let base_project: Cow<'_, Project> = match view.floor_id {
            Some(fid)
                if source.floors.active_floor_id != Some(fid)
                    && source.floors.levels.iter().any(|l| l.id == fid) =>
            {
                let mut switched = source.clone();
                let _ = switched.switch_floor(fid);
                Cow::Owned(switched)
            }
            _ => Cow::Borrowed(source),
        };
        let shown_project: Cow<'_, Project> = match &base_project {
            Cow::Borrowed(p) => p.for_view(view.discipline, &view.hidden_layers),
            Cow::Owned(p) => Cow::Owned(
                p.for_view(view.discipline, &view.hidden_layers)
                    .into_owned(),
            ),
        };
        Some(SidePlanView {
            base_project,
            shown_project,
            discipline: view.discipline,
            floor_name,
            linked_to_active,
        })
    }

    /// Convenience helper used by tests and legacy callers.
    #[cfg(test)]
    pub(crate) fn side_plan_discipline_project(
        &self,
        index: usize,
    ) -> Option<(Cow<'_, Project>, Discipline)> {
        let side = self.side_plan_view(index)?;
        Some((side.shown_project, side.discipline))
    }

    pub(crate) fn active_pane(&self) -> TabPane {
        self.active_pane
    }

    pub(crate) fn split_ratio(&self) -> f32 {
        self.split_ratio
    }

    pub(crate) fn set_split_ratio(&mut self, ratio: f32) {
        let clamped = ratio.clamp(0.20, 0.80);
        if (self.split_ratio - clamped).abs() > 1e-4 {
            self.split_ratio = clamped;
            self.last_canvas_center = None;
        }
    }

    /// Which pane `index` belongs to while split.
    pub(crate) fn tab_pane(&self, index: usize) -> TabPane {
        if index == self.active_tab {
            self.active_pane
        } else {
            self.tab_view(index)
                .map(|view| view.pane)
                .unwrap_or(TabPane::Left)
        }
    }

    /// Whether any open project has unsaved changes.
    pub(crate) fn any_tab_dirty(&self) -> bool {
        self.dirty || self.projects.values().any(|shared| shared.dirty)
    }

    /// The active tab holds nothing yet: lone tab of an unsaved blank plan.
    pub(crate) fn active_is_blank(&self) -> bool {
        self.project_tab_count(self.active_project_id) == 1
            && !self.dirty
            && self.file_path.is_none()
            && self.project == Project::default()
    }

    /// Shows the tab at `index`.
    ///
    /// When switching between two linked tabs of the same project, the shared
    /// [`Project`], [`History`], `dirty` flag and [`PlanLog`] remain untouched
    /// in place (zero copy).
    pub(crate) fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab || self.tab_view(index).is_none() {
            return;
        }
        self.settle_before_switch();
        let old = self.active_tab;
        let old_pane = self.active_pane;
        let old_pid = self.active_project_id;
        let target_pane = self.tab_pane(index);

        let previous_view = self.take_view_tab();
        self.tabs[old] = Some(previous_view);
        let next_view = self.tabs[index]
            .take()
            .expect("an inactive tab holds its view state");
        let next_pid = next_view.project_id;

        if next_pid != old_pid {
            let parked = self.take_shared_project();
            self.projects.insert(old_pid, parked);
            let target_project = self
                .projects
                .remove(&next_pid)
                .expect("every inactive project_id exists in self.projects");
            self.put_shared_project(target_project);
            // Notify MCP subscribers that the active project changed without
            // mutating the target project's own revision history.
            self.mcp_events.publish(&crate::mcp::events::plan_updated());
        }

        self.put_view_tab(next_view);
        self.active_tab = index;
        self.align_active_floor_to_tab();
        self.revalidate_selection();

        // Side by side: focusing a tab on the other pane keeps the previous
        // active tab visible on its own pane, without jumping sides.
        if self.split_tab.is_some() && (target_pane != old_pane || self.split_tab == Some(index)) {
            self.split_tab = Some(old);
        }

        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        let linked = self.project_tab_count(self.active_project_id) > 1;
        let label = format_tab_title(
            &self.project,
            self.file_path.as_deref(),
            self.active_tab_title.as_deref(),
            self.discipline,
            linked,
        );
        self.message = format!("Aba: {label}.");
    }

    pub(crate) fn next_tab(&mut self, step: isize) {
        let count = self.tabs.len() as isize;
        if count > 1 {
            let next = (self.active_tab as isize + step).rem_euclid(count) as usize;
            self.switch_tab(next);
        }
    }

    /// Opens a brand-new blank independent project in a new tab on the active pane.
    pub(crate) fn new_tab(&mut self) {
        self.new_tab_in_pane(self.active_pane);
    }

    /// Opens a brand-new blank independent project in a new tab on `pane`.
    pub(crate) fn new_tab_in_pane(&mut self, pane: TabPane) {
        let pid = self.alloc_project_id();
        let tid = self.alloc_tab_id();
        self.projects.insert(
            pid,
            SharedProject::holding(pid, Project::default(), None, false),
        );
        let mut view = ViewTab::new(tid, pid, Discipline::All);
        view.pane = if self.split_tab.is_some() {
            pane
        } else {
            TabPane::Left
        };
        self.tabs.push(Some(view));
        let index = self.tabs.len() - 1;
        self.switch_tab(index);
        self.scale = 60.0;
        self.pan = egui::Vec2::ZERO;
        self.fit_pending = false;
        self.terrain_width_text = "5,00".to_owned();
        self.terrain_height_text = "10,00".to_owned();
        self.message = "Nova aba com um projeto em branco.".to_owned();
    }

    /// Opens a synchronized linked view tab for the SAME project behind
    /// `source`, without copying any project data.
    pub(crate) fn new_linked_tab_in_pane(
        &mut self,
        source: usize,
        discipline: Discipline,
        floor_id: Option<u64>,
        title_override: Option<String>,
        pane: TabPane,
    ) -> Option<usize> {
        let pid = self.tab_project_id(source)?;
        let (scale, pan, default_floor) = if source == self.active_tab {
            (
                self.scale,
                self.pan,
                self.active_tab_floor_id
                    .or(self.project.floors.active_floor_id),
            )
        } else {
            let view = self.tab_view(source)?;
            let fallback_floor = self
                .project_by_id(pid)
                .and_then(|p| p.floors.active_floor_id);
            (view.scale, view.pan, view.floor_id.or(fallback_floor))
        };

        let tid = self.alloc_tab_id();
        let mut view = ViewTab::new(tid, pid, discipline);
        view.title_override = title_override
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        view.floor_id = floor_id.or(default_floor);
        view.scale = scale;
        view.pan = pan;
        view.fit_pending = false;
        view.pane = if self.split_tab.is_some() {
            pane
        } else {
            TabPane::Left
        };
        self.tabs.push(Some(view));
        let index = self.tabs.len() - 1;
        self.switch_tab(index);
        self.message = format!(
            "Nova visão vinculada ({}): alterações aqui e nas outras abas compartilham o mesmo projeto.",
            discipline.label()
        );
        Some(index)
    }

    /// Opens a synchronized linked view tab for the active project on the active pane.
    pub(crate) fn new_linked_tab(&mut self, discipline: Discipline) -> usize {
        self.new_linked_tab_in_pane(self.active_tab, discipline, None, None, self.active_pane)
            .unwrap_or(self.active_tab)
    }

    /// Updates the discipline, floor, title or hidden layers of tab `index`.
    pub(crate) fn configure_view_tab(
        &mut self,
        index: usize,
        discipline: Option<Discipline>,
        floor_id: Option<u64>,
        title_override: Option<Option<String>>,
    ) -> Result<(), String> {
        if index >= self.tabs.len() {
            return Err(format!("Aba {} não encontrada.", index + 1));
        }
        let pid = self
            .tab_project_id(index)
            .ok_or_else(|| format!("Aba {} não encontrada.", index + 1))?;
        if let Some(fid) = floor_id {
            let valid = self
                .project_by_id(pid)
                .is_some_and(|p| p.floors.levels.iter().any(|l| l.id == fid));
            if !valid {
                return Err(format!("Pavimento {fid} não encontrado neste projeto."));
            }
        }
        if index == self.active_tab {
            if let Some(d) = discipline {
                self.set_discipline(d);
            }
            if let Some(fid) = floor_id {
                self.active_tab_floor_id = Some(fid);
                self.align_active_floor_to_tab();
                self.clear_selection();
                self.sync_inspector_fields();
            }
            if let Some(title) = title_override {
                self.active_tab_title =
                    title.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
            }
        } else if let Some(view) = self.tabs[index].as_mut() {
            if let Some(d) = discipline {
                view.discipline = d;
            }
            if let Some(fid) = floor_id {
                view.floor_id = Some(fid);
            }
            if let Some(title) = title_override {
                view.title_override = title.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
            }
        }
        Ok(())
    }

    /// Opens an explicit independent copy of the active project in a new tab.
    pub(crate) fn duplicate_tab(&mut self) {
        self.duplicate_tab_in_pane(self.active_tab, self.active_pane);
    }

    /// Copies the project behind `source` into an independent new project tab.
    pub(crate) fn duplicate_tab_in_pane(&mut self, source: usize, pane: TabPane) {
        let Some(source_pid) = self.tab_project_id(source) else {
            return;
        };
        let (mut copy, name, scale, pan, discipline, floor_id) = if source == self.active_tab {
            (
                self.project.clone(),
                base_title_of(&self.project, self.file_path.as_deref()),
                self.scale,
                self.pan,
                self.discipline,
                self.active_tab_floor_id,
            )
        } else if let (Some(project), Some(view)) =
            (self.project_by_id(source_pid), self.tab_view(source))
        {
            (
                project.clone(),
                base_title_of(project, self.project_file_path(source_pid)),
                view.scale,
                view.pan,
                view.discipline,
                view.floor_id,
            )
        } else {
            return;
        };
        copy.name = format!("{name} (cópia)");
        let pid = self.alloc_project_id();
        let tid = self.alloc_tab_id();
        self.projects
            .insert(pid, SharedProject::holding(pid, copy, None, true));
        let mut view = ViewTab::new(tid, pid, discipline);
        view.floor_id = floor_id;
        view.scale = scale;
        view.pan = pan;
        view.fit_pending = false;
        view.pane = if self.split_tab.is_some() {
            pane
        } else {
            TabPane::Left
        };
        self.tabs.push(Some(view));
        let index = self.tabs.len() - 1;
        self.switch_tab(index);
        self.recovery_pending = true;
        self.message = format!("Cópia independente de \"{name}\" aberta numa nova aba.");
    }

    /// Moves tab `index` to `target` pane when viewing side by side.
    pub(crate) fn move_tab_to_pane(&mut self, index: usize, target: TabPane) {
        if self.split_tab.is_none() || index >= self.tabs.len() || self.tab_pane(index) == target {
            return;
        }
        let source_pane = target.opposite();
        if index == self.active_tab {
            self.active_pane = target;
        } else if let Some(view) = self.tabs[index].as_mut() {
            view.pane = target;
        }
        let replacement =
            (0..self.tabs.len()).find(|&i| i != index && self.tab_pane(i) == source_pane);
        match replacement {
            None => {
                self.toggle_side_by_side(None);
            }
            Some(other) => {
                if index == self.active_tab || self.split_tab == Some(index) {
                    self.split_tab = Some(other);
                }
            }
        }
    }

    /// Asks to save ONLY when closing the last open tab of a project with
    /// unsaved changes; closing a secondary linked view never discards data.
    pub(crate) fn request_close_tab(&mut self, index: usize) {
        if index != self.active_tab {
            self.switch_tab(index);
        }
        let last_link = self.project_tab_count(self.active_project_id) <= 1;
        if last_link && self.dirty {
            self.pending_action = Some(PendingAction::CloseTab);
        } else {
            self.close_active_tab();
        }
    }

    /// Closes the active tab without asking.
    ///
    /// If other tabs are still linked to the same project, the shared project
    /// and all its unsaved changes remain alive in them. If this was the last
    /// tab in the window, it turns into a fresh blank project.
    pub(crate) fn close_active_tab(&mut self) {
        self.settle_before_switch();
        let closed_pid = self.active_project_id;
        let still_linked = self.project_tab_count(closed_pid) > 1;

        if self.tabs.len() == 1 {
            recovery::clear();
            let new_pid = self.alloc_project_id();
            let new_tid = self.alloc_tab_id();
            self.put_shared_project(SharedProject::holding(
                new_pid,
                Project::default(),
                None,
                false,
            ));
            self.put_view_tab(ViewTab::new(new_tid, new_pid, Discipline::All));
            self.split_tab = None;
            self.active_pane = TabPane::Left;
            self.fit_pending = false;
            self.mcp_events.publish(&crate::mcp::events::plan_updated());
            self.sync_inspector_fields();
            self.sync_masonry_fields();
            self.sync_drafts();
            self.message = "Aba fechada.".to_owned();
            return;
        }

        let closed = self.active_tab;
        let closed_pane = self.active_pane;
        self.tabs.remove(closed);

        let adjusted_split = match self.split_tab {
            Some(split) if split == closed => None,
            Some(split) if split > closed => Some(split - 1),
            other => other,
        };

        let next_index = if let Some(other_visible) = adjusted_split {
            let same_pane = (0..self.tabs.len()).find(|&i| {
                i != other_visible
                    && self
                        .tab_view(i)
                        .is_some_and(|view| view.pane == closed_pane)
            });
            if let Some(index) = same_pane {
                self.split_tab = Some(other_visible);
                index
            } else {
                self.split_tab = None;
                for view in self.tabs.iter_mut().flatten() {
                    view.pane = TabPane::Left;
                }
                other_visible
            }
        } else {
            self.split_tab = None;
            for view in self.tabs.iter_mut().flatten() {
                view.pane = TabPane::Left;
            }
            closed.min(self.tabs.len() - 1)
        };

        let next_view = self.tabs[next_index]
            .take()
            .expect("an inactive tab holds its view state");
        let next_pid = next_view.project_id;

        if next_pid != closed_pid {
            if still_linked {
                // Other tabs still reference `closed_pid`: keep the shared project alive!
                let parked = self.take_shared_project();
                self.projects.insert(closed_pid, parked);
            } else {
                recovery::clear();
            }
            let target_project = self
                .projects
                .remove(&next_pid)
                .expect("every inactive project_id exists in self.projects");
            self.put_shared_project(target_project);
            self.mcp_events.publish(&crate::mcp::events::plan_updated());
        }

        self.put_view_tab(next_view);
        if self.split_tab.is_none() {
            self.active_pane = TabPane::Left;
        }
        self.active_tab = next_index;
        self.align_active_floor_to_tab();
        self.revalidate_selection();
        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        self.message = if still_linked {
            "Visão fechada (o projeto continua aberto nas outras abas vinculadas).".to_owned()
        } else {
            "Aba fechada.".to_owned()
        };
    }

    /// Finds an already-open project matching `path` (normalized/canonical).
    pub(crate) fn find_open_project_by_path(&self, path: &Path) -> Option<ProjectId> {
        if self
            .file_path
            .as_deref()
            .is_some_and(|open| same_file_path(open, path))
        {
            return Some(self.active_project_id);
        }
        self.projects
            .values()
            .find(|shared| {
                shared
                    .file_path
                    .as_deref()
                    .is_some_and(|open| same_file_path(open, path))
            })
            .map(|shared| shared.id)
    }

    /// Opens `path` without ever creating a second independent instance of a
    /// project that is already open. Restores any saved linked view tabs.
    pub(crate) fn open_in_tab(&mut self, path: &Path) -> OpenResult {
        if let Some(existing_pid) = self.find_open_project_by_path(path)
            && let Some(index) =
                (0..self.tabs.len()).find(|&i| self.tab_project_id(i) == Some(existing_pid))
        {
            self.switch_tab(index);
            self.message =
                "Esse projeto já estava aberto (use 'Nova visão vinculada' para abrir outra disciplina)."
                    .to_owned();
            return OpenResult::Success;
        }

        let created = !self.active_is_blank();
        if created {
            let pid = self.alloc_project_id();
            let tid = self.alloc_tab_id();
            self.projects.insert(
                pid,
                SharedProject::holding(pid, Project::default(), None, false),
            );
            let mut view = ViewTab::new(tid, pid, Discipline::All);
            view.pane = if self.split_tab.is_some() {
                self.active_pane
            } else {
                TabPane::Left
            };
            self.tabs.push(Some(view));
            let index = self.tabs.len() - 1;
            self.switch_tab(index);
        }

        let result = self.open_file(path);
        if matches!(result, OpenResult::Error) && created {
            let message = self.message.clone();
            self.close_active_tab();
            self.message = message;
        }
        result
    }

    /// Collects all linked tabs of `project_id` in order for saving inside the
    /// `.trazo` file's `workspace_tabs`.
    pub(crate) fn collect_workspace_tabs(&self, project_id: ProjectId) -> Vec<WorkspaceTab> {
        let mut out = Vec::new();
        for (index, slot) in self.tabs.iter().enumerate() {
            if self.tab_project_id(index) != Some(project_id) {
                continue;
            }
            match slot {
                None => out.push(WorkspaceTab {
                    title: self.active_tab_title.clone(),
                    discipline: self.discipline,
                    floor_id: self
                        .active_tab_floor_id
                        .or(self.project.floors.active_floor_id),
                    hidden_layers: self.active_hidden_layers.clone(),
                    active: true,
                    split_companion: false,
                    pane: self.active_pane.to_workspace(),
                    scale: Some(self.scale),
                    pan: Some([self.pan.x, self.pan.y]),
                }),
                Some(view) => out.push(WorkspaceTab {
                    title: view.title_override.clone(),
                    discipline: view.discipline,
                    floor_id: view.floor_id,
                    hidden_layers: view.hidden_layers.clone(),
                    active: false,
                    split_companion: self.split_tab == Some(index),
                    pane: view.pane.to_workspace(),
                    scale: Some(view.scale),
                    pan: Some([view.pan.x, view.pan.y]),
                }),
            }
        }
        out
    }

    /// Restores linked view tabs saved inside a loaded project file, binding
    /// every restored tab to the current `self.active_project_id`.
    pub(crate) fn restore_workspace_tabs(&mut self, saved: Vec<WorkspaceTab>) {
        if saved.is_empty() {
            return;
        }
        let pid = self.active_project_id;
        let base_index = self.active_tab;

        let apply_camera = |scale: Option<f32>, pan: Option<[f32; 2]>| -> (f32, egui::Vec2, bool) {
            match (scale, pan) {
                (Some(s), Some([px, py])) if s.is_finite() && s > 1.0 => {
                    (s, egui::vec2(px, py), false)
                }
                _ => (60.0, egui::Vec2::ZERO, true),
            }
        };

        // Configure the first tab in place.
        let first = &saved[0];
        self.active_tab_title = first.title.clone();
        self.discipline = first.discipline;
        self.active_tab_floor_id = first.floor_id.or(self.project.floors.active_floor_id);
        self.active_hidden_layers = first.hidden_layers.clone();
        let (s, p, fit) = apply_camera(first.scale, first.pan);
        self.scale = s;
        self.pan = p;
        self.fit_pending = fit;
        self.align_active_floor_to_tab();

        let mut target_active = first.active.then_some(base_index);
        let mut target_split = first.split_companion.then_some(base_index);
        let mut created_indices = vec![base_index];

        for item in &saved[1..] {
            let tid = self.alloc_tab_id();
            let mut view = ViewTab::new(tid, pid, item.discipline);
            view.title_override = item.title.clone();
            view.floor_id = item.floor_id.or(self.project.floors.active_floor_id);
            view.hidden_layers = item.hidden_layers.clone();
            let (vs, vp, vfit) = apply_camera(item.scale, item.pan);
            view.scale = vs;
            view.pan = vp;
            view.fit_pending = vfit;
            view.pane = TabPane::from_workspace(item.pane);
            self.tabs.push(Some(view));
            let idx = self.tabs.len() - 1;
            created_indices.push(idx);
            if item.active {
                target_active = Some(idx);
            }
            if item.split_companion {
                target_split = Some(idx);
            }
        }

        if let Some(active_idx) = target_active
            && active_idx != self.active_tab
        {
            self.switch_tab(active_idx);
        }
        if let Some(split_idx) = target_split
            && split_idx != self.active_tab
            && created_indices.contains(&split_idx)
        {
            self.toggle_side_by_side(Some(split_idx));
        }
    }

    /// Shows `index` beside the active tab, or hides the side view.
    pub(crate) fn toggle_side_by_side(&mut self, index: Option<usize>) {
        self.last_canvas_center = None;
        let target =
            index.filter(|index| *index != self.active_tab && self.tab_view(*index).is_some());
        match target {
            Some(other) => {
                self.active_pane = TabPane::Left;
                for (i, slot) in self.tabs.iter_mut().enumerate() {
                    if let Some(view) = slot {
                        view.pane = if i == other {
                            TabPane::Right
                        } else {
                            TabPane::Left
                        };
                    }
                }
                self.split_tab = Some(other);
                self.message = "Lado a lado: arraste a divisória para redimensionar; clique no outro lado para editá-lo.".to_owned();
            }
            None => {
                self.split_tab = None;
                self.active_pane = TabPane::Left;
                for view in self.tabs.iter_mut().flatten() {
                    view.pane = TabPane::Left;
                }
                self.message = "Vista única.".to_owned();
            }
        }
    }

    /// Keeps a crash-recovery draft of every distinct open project with unsaved changes.
    pub(crate) fn sync_drafts(&mut self) {
        if self.dirty {
            self.recovery_pending = true;
        } else {
            recovery::clear();
        }
        let waiting: Vec<&Project> = self
            .projects
            .values()
            .filter(|shared| shared.dirty)
            .map(|shared| &shared.project)
            .collect();
        if let Err(error) = recovery::save_tabs(&waiting) {
            self.message = format!("Não foi possível proteger o rascunho das abas: {error}");
        }
    }

    /// After saving or discarding the active project while closing the window:
    /// switches to the next distinct dirty project, or lets the window close.
    pub(crate) fn next_unsaved_before_closing(&mut self) -> bool {
        self.dirty = false;
        self.recovery_pending = false;
        recovery::clear();
        let next = (0..self.tabs.len()).find(|&index| {
            self.tab_project_id(index)
                .is_some_and(|pid| pid != self.active_project_id && self.project_dirty(pid))
        });
        match next {
            Some(index) => {
                self.switch_tab(index);
                self.pending_action = Some(PendingAction::CloseApp);
                true
            }
            None => {
                recovery::clear_tabs();
                false
            }
        }
    }

    /// Reopens projects whose drafts survived a crash.
    pub(crate) fn restore_tab_drafts(&mut self) {
        for project in recovery::load_tabs() {
            let pid = self.alloc_project_id();
            let tid = self.alloc_tab_id();
            self.projects
                .insert(pid, SharedProject::holding(pid, project, None, true));
            self.tabs
                .push(Some(ViewTab::new(tid, pid, Discipline::All)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::FloorPlanApp;

    fn wall(app: &mut FloorPlanApp, y: f64) -> u64 {
        let previous = app.project.clone();
        let id = app
            .project
            .add_wall(Point::new(0.0, y), Point::new(4.0, y))
            .unwrap();
        app.record_change(previous);
        id
    }

    #[test]
    fn each_tab_keeps_its_project_history_selection_and_view() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let first_wall = wall(&mut app, 0.0);
        app.selection = Some(Selection::Wall(first_wall));
        app.scale = 90.0;

        app.new_tab();
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active_tab(), 1);
        assert!(app.project.walls.is_empty() && !app.dirty);
        wall(&mut app, 2.0);
        wall(&mut app, 4.0);

        app.switch_tab(0);
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.selection, Some(Selection::Wall(first_wall)));
        assert_eq!(app.scale, 90.0);
        assert!(app.dirty);
        // Undo stays with its project.
        app.undo();
        assert!(app.project.walls.is_empty());
        app.switch_tab(1);
        assert_eq!(app.project.walls.len(), 2);
        app.undo();
        assert_eq!(app.project.walls.len(), 1);

        let infos = app.tab_infos();
        assert!(infos[1].active && !infos[0].active);
        assert!(app.any_tab_dirty());
    }

    #[test]
    fn linked_tabs_share_single_project_history_and_live_edits() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project.name = "Residência".to_owned();
        app.set_discipline(Discipline::Architecture);
        let w = wall(&mut app, 0.0);

        let elec_idx = app.new_linked_tab(Discipline::Electrical);
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.tab_project_id(0), app.tab_project_id(elec_idx));
        assert!(app.tab_infos()[0].linked && app.tab_infos()[1].linked);

        // Editing in Architecture immediately reflects in Electrical (and live split view).
        app.toggle_side_by_side(Some(0));
        app.switch_tab(0);
        let prev = app.project.clone();
        app.project
            .move_wall_checked(w, Point::new(0.0, 1.5))
            .unwrap();
        app.record_change(prev);

        // Side view reads live shared model!
        let side = app.side_plan_view(elec_idx).unwrap();
        assert!(side.linked_to_active);
        assert_eq!(side.discipline, Discipline::Electrical);
        assert!((side.base_project.walls[0].start.y - 1.5).abs() < 1e-4);

        // Switching to Electrical tab sees the exact same moved wall, and Undo
        // in Electrical reverts it in Architecture too!
        app.switch_tab(elec_idx);
        assert!((app.project.walls[0].start.y - 1.5).abs() < 1e-4);
        app.undo();
        assert!((app.project.walls[0].start.y - 0.0).abs() < 1e-4);
        app.switch_tab(0);
        assert!((app.project.walls[0].start.y - 0.0).abs() < 1e-4);

        // Closing one linked view does NOT prompt or discard unsaved data.
        app.request_close_tab(elec_idx);
        assert_eq!(app.pending_action, None);
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.project.walls.len(), 1);
        assert!(app.dirty);

        // Closing the LAST link to the dirty project DOES prompt.
        app.request_close_tab(0);
        assert_eq!(app.pending_action, Some(PendingAction::CloseTab));
    }

    #[test]
    fn the_new_tab_button_always_opens_a_tab() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        assert!(app.active_is_blank());
        app.new_tab();
        app.new_tab();
        assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    }

    #[test]
    fn copy_in_one_tab_and_paste_in_another() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let id = wall(&mut app, 0.0);
        app.selection = Some(Selection::Wall(id));
        app.copy_selection();
        app.new_tab();
        app.paste();
        assert_eq!(app.project.walls.len(), 1);
    }

    #[test]
    fn a_duplicate_tab_is_an_independent_copy() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project.name = "Casa".to_owned();
        wall(&mut app, 0.0);
        app.duplicate_tab();
        assert_eq!(app.tab_count(), 2);
        assert_ne!(app.tab_project_id(0), app.tab_project_id(1));
        assert_eq!(app.project.name, "Casa (cópia)");
        assert_eq!(app.project.walls.len(), 1);
        assert!(app.dirty && app.file_path.is_none());
        // Side by side with the original, which then trades places on click.
        app.toggle_side_by_side(Some(0));
        assert_eq!(app.split_tab, Some(0));
        app.switch_tab(0);
        assert_eq!(app.split_tab, Some(1));
    }

    #[test]
    fn closing_asks_for_unsaved_tabs_and_keeps_the_rest() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        wall(&mut app, 0.0);
        app.new_tab();
        // A blank tab closes at once.
        app.request_close_tab(1);
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.project.walls.len(), 1);
        // An unsaved one asks first.
        app.request_close_tab(0);
        assert_eq!(app.pending_action, Some(PendingAction::CloseTab));
        app.discard_changes_and_proceed(&context);
        assert_eq!(app.tab_count(), 1);
        assert!(app.project.walls.is_empty() && !app.dirty);

        // Closing the window walks through every unsaved project once.
        wall(&mut app, 0.0);
        app.new_tab();
        assert!(!app.dirty && app.any_tab_dirty());
        app.request_close(&context);
        assert_eq!(app.pending_action, Some(PendingAction::CloseApp));
        assert_eq!(app.active_tab(), 0, "it shows the tab that needs saving");
    }

    #[test]
    fn opening_a_file_uses_a_blank_tab_or_a_new_one() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let folder = crate::io::data_dir().join("tabs-open-test");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("casa.trazo");
        let mut project = Project::default();
        project.name = "Casa".to_owned();
        crate::io::project_file::save(&project, &path).unwrap();

        // The blank first tab takes the file.
        assert!(matches!(app.open_in_tab(&path), OpenResult::Success));
        assert_eq!(app.tab_count(), 1);
        // Opening it again goes back to that tab instead of a second copy.
        app.new_tab();
        assert!(matches!(app.open_in_tab(&path), OpenResult::Success));
        assert_eq!((app.tab_count(), app.active_tab()), (2, 0));
        // A missing file leaves no empty tab behind.
        wall(&mut app, 0.0);
        let tabs = app.tab_count();
        assert!(matches!(
            app.open_in_tab(&folder.join("nada.trazo")),
            OpenResult::Error
        ));
        assert_eq!(app.tab_count(), tabs);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn split_view_keeps_per_pane_tabs_and_clamps_split_ratio() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project.name = "Arquitetura".to_owned();
        app.new_tab();
        app.project.name = "Elétrica".to_owned();

        // Split with tab 0 on the right, active tab 1 on the left.
        app.toggle_side_by_side(Some(0));
        assert_eq!(app.active_pane(), TabPane::Left);
        assert_eq!(app.tab_pane(1), TabPane::Left);
        assert_eq!(app.tab_pane(0), TabPane::Right);

        // Adding a new tab on the right pane keeps it on the right and preserves the left tab.
        app.new_tab_in_pane(TabPane::Right);
        assert_eq!(app.tab_count(), 3);
        assert_eq!(app.active_tab(), 2);
        assert_eq!(app.active_pane(), TabPane::Right);
        assert_eq!(app.split_tab, Some(1));

        // Closing one right tab keeps the other right tab and stays split.
        app.close_active_tab();
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active_pane(), TabPane::Right);
        assert_eq!(app.split_tab, Some(1));

        // Closing the last right tab collapses back to single view cleanly.
        app.close_active_tab();
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.split_tab, None);
        assert_eq!(app.active_pane(), TabPane::Left);

        // Split ratio is safely clamped.
        app.set_split_ratio(0.05);
        assert!((app.split_ratio() - 0.20).abs() < 1e-6);
        app.set_split_ratio(0.95);
        assert!((app.split_ratio() - 0.80).abs() < 1e-6);
        app.set_split_ratio(0.62);
        assert!((app.split_ratio() - 0.62).abs() < 1e-6);
    }

    #[test]
    fn inactive_split_pane_respects_its_tab_discipline() {
        use crate::domain::layers::LayerCategory;

        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.discipline = Discipline::Structure;
        app.new_linked_tab(Discipline::Electrical);
        app.toggle_side_by_side(Some(0));

        let (shown, discipline) = app.side_plan_discipline_project(0).unwrap();
        assert_eq!(discipline, Discipline::Structure);
        assert!(shown.category_visible(LayerCategory::Structure));
        assert!(!shown.category_visible(LayerCategory::Electrical));
        assert!(!shown.category_visible(LayerCategory::Fixtures));
    }
}
