//! Opening files into tabs and saving or restoring the workspace: the tabs
//! stored with a project and the per-tab recovery drafts.

use std::path::Path;

use eframe::egui;

use super::{ProjectId, SharedProject, TabPane, ViewTab, same_file_path};
use crate::app::{FloorPlanApp, OpenResult};
use crate::domain::discipline::{Discipline, WorkspaceTab};
use crate::domain::model::Project;
use crate::io::recovery;

impl FloorPlanApp {
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
