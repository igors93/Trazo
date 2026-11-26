//! Opening new tabs: blank, linked to the same project in another
//! discipline, configured views and duplicates.

use eframe::egui;

use super::{SharedProject, TabPane, ViewTab, base_title_of};
use crate::app::FloorPlanApp;
use crate::domain::discipline::Discipline;
use crate::domain::model::Project;

impl FloorPlanApp {
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
}
