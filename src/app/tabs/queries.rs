//! Read-only questions about the open tabs and projects: counts, titles,
//! panes, export views and the side-by-side plan.

use std::borrow::Cow;
use std::path::Path;

use super::{ProjectId, SidePlanView, TabInfo, TabPane, ViewTab, floor_name_for, format_tab_title};
use crate::app::FloorPlanApp;
use crate::domain::model::Project;

impl FloorPlanApp {
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
            .filter_map(|(index, slot)| match slot {
                Some(view) => {
                    let project = self.project_by_id(view.project_id)?;
                    let path = self.project_file_path(view.project_id);
                    let linked = self.project_tab_count(view.project_id) > 1;
                    Some(TabInfo {
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
                    })
                }
                None => {
                    let linked = self.project_tab_count(self.active_project_id) > 1;
                    Some(TabInfo {
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
                    })
                }
            })
            .collect()
    }

    /// Export view matching the currently active tab (discipline, floor,
    /// per-tab hidden layers and display title).
    pub(crate) fn active_export_view(&self) -> crate::io::export::ExportView {
        let linked = self.project_tab_count(self.active_project_id) > 1;
        let mut title = format_tab_title(
            &self.project,
            self.file_path.as_deref(),
            self.active_tab_title.as_deref(),
            self.discipline,
            linked,
        );
        if self.project.floors.levels.len() > 1 {
            let floor = floor_name_for(&self.project, self.active_tab_floor_id);
            if !title.contains(&floor) {
                title.push_str(&format!(" · {floor}"));
            }
        }
        crate::io::export::ExportView {
            title,
            discipline: self.discipline,
            floor_id: self
                .active_tab_floor_id
                .or(self.project.floors.active_floor_id),
            hidden_layers: self.active_hidden_layers.clone(),
        }
    }

    /// Ordered export views for every open tab belonging to the active project,
    /// preserving the exact tab-strip order so multi-page PDF exports emit one
    /// page per tab exactly as seen on screen.
    pub(crate) fn project_tab_export_views(&self) -> Vec<crate::io::export::ExportView> {
        let linked = self.project_tab_count(self.active_project_id) > 1;
        let multi_floor = self.project.floors.levels.len() > 1;
        let mut views = Vec::new();
        for slot in &self.tabs {
            match slot {
                Some(view) if view.project_id == self.active_project_id => {
                    let mut title = format_tab_title(
                        &self.project,
                        self.file_path.as_deref(),
                        view.title_override.as_deref(),
                        view.discipline,
                        linked,
                    );
                    if multi_floor {
                        let floor = floor_name_for(&self.project, view.floor_id);
                        if !title.contains(&floor) {
                            title.push_str(&format!(" · {floor}"));
                        }
                    }
                    views.push(crate::io::export::ExportView {
                        title,
                        discipline: view.discipline,
                        floor_id: view.floor_id.or(self.project.floors.active_floor_id),
                        hidden_layers: view.hidden_layers.clone(),
                    });
                }
                Some(_) => {}
                None => views.push(self.active_export_view()),
            }
        }
        if views.is_empty() {
            views.push(self.active_export_view());
        }
        views
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
    ) -> Option<(Cow<'_, Project>, crate::domain::discipline::Discipline)> {
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
}
