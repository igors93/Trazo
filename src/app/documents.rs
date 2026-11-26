//! The document lifecycle: new, open, save, templates, recovery drafts
//! and the confirmation flow before closing unsaved work.

use std::path::{Path, PathBuf};

use eframe::egui;

use super::{FloorPlanApp, OpenResult, PendingAction, Tool, markups};
use crate::io::{project_file, recovery};

impl FloorPlanApp {
    /// A blank plan in a new tab; the current project stays open in its own.
    pub fn request_new_project(&mut self) {
        self.new_tab();
    }

    /// Opens a project in a new tab (or in the active one when it is blank).
    pub fn request_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Projeto ou Modelo Trazo", &["trazo", "trazot", "json"])
            .pick_file()
        {
            let _ = self.open_in_tab(&path);
        }
    }

    pub(crate) fn recover_pending_draft(&mut self) {
        if let Some(draft) = self.pending_recovery.take() {
            let before = std::mem::replace(&mut self.project, draft);
            self.note_plan_change(&before, true);
            self.history.clear();
            self.file_path = None;
            self.dirty = true;
            self.recovery_pending = false;
            self.show_start_page = false;
            self.fit_view_requested();
            self.restore_tab_drafts();
            self.sync_inspector_fields();
            self.sync_masonry_fields();
            self.message = "Rascunho recuperado. Salve o projeto para mantê-lo.".to_owned();
        }
    }

    pub(crate) fn discard_pending_draft(&mut self) {
        self.pending_recovery = None;
        recovery::clear();
        recovery::clear_tabs();
        self.message = "Rascunho automático descartado.".to_owned();
    }

    pub(crate) fn defer_pending_draft(&mut self) {
        self.pending_recovery = None;
        self.message = "Recuperação adiada; o rascunho permanece salvo em disco.".to_owned();
    }

    pub(crate) fn save_as_template(&mut self) -> bool {
        let default_name = format!(
            "{}.trazot",
            if self.project.name.trim().is_empty() {
                "modelo-trazo"
            } else {
                self.project.name.trim()
            }
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Modelo de Projeto Trazo", &["trazot"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return false;
        };
        let mut to_save = self.project.clone();
        to_save.workspace_tabs = self.collect_workspace_tabs(self.active_project_id);
        match project_file::save(&to_save, &path) {
            Ok(()) => {
                if self.user_templates.pick(&path) && !cfg!(test) {
                    let _ = self.user_templates.save();
                }
                self.message = format!("Modelo salvo em {}.", path.display());
                true
            }
            Err(error) => {
                self.message = format!("Não foi possível salvar o modelo: {error}");
                false
            }
        }
    }

    pub fn request_close(&mut self, ctx: &egui::Context) {
        if self.any_tab_dirty() {
            if !self.dirty {
                // Show the tab that needs saving.
                let _ = self.next_unsaved_before_closing();
            }
            self.pending_action = Some(PendingAction::CloseApp);
        } else {
            recovery::clear();
            recovery::clear_tabs();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn execute_pending_action(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::CloseTab => self.close_active_tab(),
                PendingAction::CloseApp => {
                    if !self.next_unsaved_before_closing() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
    }

    pub fn discard_changes_and_proceed(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::CloseTab => self.close_active_tab(),
                PendingAction::CloseApp => {
                    if !self.next_unsaved_before_closing() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
        self.message = "Operação cancelada.".to_owned();
    }

    /// Drops everything in progress on the plan (drafts, drags, the
    /// selection) when another project takes its place.
    pub(super) fn reset_editing_state(&mut self) {
        self.markup_state = markups::MarkupUiState::default();
        self.ai_activity.forget_highlight();
        self.cancel_creation();
        self.clear_selection();
        self.box_select = None;
        self.item_drag = None;
        self.vertex_drag = None;
        self.drag_initial_project = None;
        self.pending_room = None;
    }

    pub(super) fn new_project(&mut self) {
        self.show_start_page = false;
        recovery::clear();
        self.measurements.clear();
        let before = std::mem::take(&mut self.project);
        self.note_plan_change(&before, true);
        self.history.clear();
        self.file_path = None;
        self.dirty = false;
        self.recovery_pending = false;
        self.pending_action = None;
        self.tool = Tool::Move;
        self.reset_editing_state();
        self.pan = egui::Vec2::ZERO;
        self.last_canvas_center = None;
        self.scale = 60.0;
        self.fit_pending = false;
        self.terrain_width_text = "5,00".to_owned();
        self.terrain_height_text = "10,00".to_owned();
        self.sync_masonry_fields();
        self.message = "Nova planta criada.".to_owned();
    }

    pub fn open_file(&mut self, path: &Path) -> OpenResult {
        let is_template = crate::io::templates::is_template_path(path);
        match project_file::load(path) {
            Ok((mut project, warnings)) => {
                let saved_tabs = std::mem::take(&mut project.workspace_tabs);
                if is_template {
                    // Starting from a `.trazot` template always creates a fresh untitled
                    // project with its own project GUID so the template is never overwritten.
                    project.guids.project = crate::domain::guid::Guid::new();
                }
                recovery::clear();
                self.measurements.clear();
                let before = std::mem::replace(&mut self.project, project);
                self.note_plan_change(&before, true);
                self.history.clear();
                self.sync_report = crate::domain::dependencies::DependencySyncReport::default();
                if is_template {
                    self.file_path = None;
                    if self.user_templates.pick(path) && !cfg!(test) {
                        let _ = self.user_templates.save();
                    }
                } else {
                    self.file_path = Some(path.to_path_buf());
                    self.recent_files.record(
                        path,
                        &self.project.name,
                        self.project.walls.len(),
                        self.project.rooms.len(),
                    );
                    if !cfg!(test) {
                        let _ = self.recent_files.save();
                    }
                }
                self.show_start_page = false;
                self.dirty = false;
                self.recovery_pending = false;
                self.pending_action = None;
                self.tool = Tool::Move;
                self.reset_editing_state();
                self.active_tab_title = None;
                self.active_tab_floor_id = self.project.floors.active_floor_id;
                self.active_hidden_layers.clear();
                self.fit_view_requested();
                if !saved_tabs.is_empty() {
                    self.restore_workspace_tabs(saved_tabs);
                }
                self.sync_inspector_fields();
                self.sync_masonry_fields();
                if is_template {
                    self.message = format!(
                        "Novo projeto criado a partir do modelo \"{}\".",
                        path.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "modelo".to_owned())
                    );
                } else if warnings.is_empty() {
                    self.message = "Projeto aberto com sucesso.".to_owned();
                } else {
                    self.message = format!("Projeto aberto com avisos: {}", warnings.join("; "));
                }
                OpenResult::Success
            }
            Err(error) => {
                self.message = format!("Não foi possível abrir o projeto: {error}");
                OpenResult::Error
            }
        }
    }

    pub(super) fn save(&mut self) -> bool {
        let path = match self.file_path.clone() {
            Some(path) => path,
            None => match rfd::FileDialog::new()
                .add_filter("Projeto Trazo", &["trazo", "json"])
                .set_file_name("minha-planta.trazo")
                .save_file()
            {
                Some(path) => path,
                None => return false,
            },
        };
        self.write_project(path)
    }

    /// Saves the shared project (including its linked workspace view tabs) to
    /// `path`, which becomes the current file for every linked tab.
    pub(super) fn write_project(&mut self, path: PathBuf) -> bool {
        let mut to_save = self.project.clone();
        to_save.workspace_tabs = self.collect_workspace_tabs(self.active_project_id);
        match project_file::save(&to_save, &path) {
            Ok(()) => {
                recovery::clear();
                self.recent_files.record(
                    &path,
                    &to_save.name,
                    to_save.walls.len(),
                    to_save.rooms.len(),
                );
                self.thumbnail_cache.remove(&path);
                if !cfg!(test) {
                    let _ = self.recent_files.save();
                }
                self.file_path = Some(path);
                self.dirty = false;
                self.recovery_pending = false;
                self.message = "Projeto salvo com sucesso.".to_owned();
                true
            }
            Err(error) => {
                self.message = format!("Não foi possível salvar: {error}");
                false
            }
        }
    }

    pub(super) fn document_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sem título".to_owned())
    }
}
