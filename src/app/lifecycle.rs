//! Starting the editor and keeping its background services running:
//! the constructor, the update check, the MCP server, recovery drafts,
//! interface settings, the window title and background analysis.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui;

use super::{
    CreationState, DocDialog, FloorPlanApp, McpStatus, RibbonTab, Tool, ai_activity, markups,
    pacing, plan_sync, tabs, theme, ui,
};
use crate::{
    domain::{fixtures::FixtureKind, history::History, model::Project},
    io::{
        recovery,
        settings::{self, Settings},
    },
    mcp::{self, server::ToolCall},
};

impl FloorPlanApp {
    pub fn new(context: &egui::Context) -> Self {
        Self::new_with_file(context, None)
    }

    pub fn new_with_file(context: &egui::Context, initial_file: Option<PathBuf>) -> Self {
        theme::apply(context);
        let recovered = if cfg!(test) || initial_file.is_some() {
            None
        } else {
            recovery::load()
        };
        let show_start_page = !cfg!(test) && initial_file.is_none();
        let (initial_project, pending_recovery, restored) = if show_start_page {
            (Project::default(), recovered, false)
        } else {
            let has = recovered.is_some();
            (recovered.unwrap_or_default(), None, has)
        };
        let user_templates = if cfg!(test) {
            crate::io::templates::UserTemplates::default()
        } else {
            crate::io::templates::UserTemplates::load()
        };
        let recent_files = if cfg!(test) {
            crate::io::templates::RecentFiles::default()
        } else {
            crate::io::templates::RecentFiles::load()
        };
        let preferences = if cfg!(test) {
            Settings::default()
        } else {
            settings::load()
        };
        let tunnel_token = mcp::server::TunnelToken::new();
        let mcp_events = mcp::events::EventHub::new();
        let (mcp_calls, mcp_status) = if cfg!(test) {
            (None, McpStatus::Off)
        } else {
            Self::start_mcp(context, tunnel_token.clone(), mcp_events.clone())
        };
        let mut app = Self {
            project: initial_project,
            history: History::new(),
            file_path: None,
            dirty: restored,
            recovery_pending: false,
            pending_action: None,
            tool: Tool::Move,
            markup_state: markups::MarkupUiState::default(),
            creation_state: CreationState::Idle,
            selection: None,
            selection_extra: Vec::new(),
            selection_filter: preferences.selection_filter,
            box_select: None,
            item_drag: None,
            vertex_drag: None,
            drag_initial_project: None,
            pending_room: None,
            polygon_draft: Vec::new(),
            draw_points: Vec::new(),
            draft_input: String::new(),
            draft_aim: None,
            drawing_radius_text: String::new(),
            room_name: String::new(),
            room_dialog_focus: false,
            opening_drag: None,
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            last_canvas_center: None,
            fit_pending: restored,
            snap_enabled: preferences.snap_enabled,
            osnap: preferences.osnap,
            wall_reference: preferences.wall_reference,
            terrain_width_text: "5,00".to_owned(),
            terrain_height_text: "10,00".to_owned(),
            wall_length_text: String::new(),
            room_name_text: String::new(),
            room_width_text: String::new(),
            room_height_text: String::new(),
            opening_width_text: String::new(),
            message: if restored {
                "Rascunho recuperado. Salve o projeto para mantê-lo.".to_owned()
            } else {
                "Selecione uma ferramenta ou clique em um elemento para começar.".to_owned()
            },
            ribbon_tab: RibbonTab::Architecture,
            ribbon_collapsed: preferences.ribbon_collapsed,
            show_grid: preferences.show_grid,
            ortho_enabled: preferences.ortho_enabled,
            show_browser: preferences.show_browser,
            show_properties: preferences.show_properties,
            show_help: false,
            show_report_dialog: false,
            report_summary: String::new(),
            report_include_log: true,
            show_updates_dialog: false,
            check_updates_on_start: preferences.check_updates,
            update_status: crate::io::updates::UpdateStatus::Idle,
            update_receiver: None,
            active_doc_dialog: DocDialog::None,
            print_state: ui::PrintDialogState::default(),
            underlay_state: ui::UnderlayUiState::default(),
            xref_manager: crate::io::xref::XrefManager::default(),
            compare_state: ui::CompareUiState::default(),
            background: crate::domain::background::BackgroundAnalyzer::new(),
            background_wake_ready: false,
            new_layer_name: String::new(),
            new_note_text: String::new(),
            new_revision_text: String::new(),
            release_reviewer: String::new(),
            release_registration: String::new(),
            release_date: String::new(),
            library_search: String::new(),
            library_category_filter: None,
            new_library_name: String::new(),
            new_library_width: 1.20,
            new_library_depth: 0.60,
            new_library_base: Some(FixtureKind::Counter),
            personal_library: crate::io::personal_library::load(),
            library_tab: 0,
            save_item_to_personal: true,
            new_block_name: String::new(),
            new_block_category: "Blocos".to_owned(),
            new_block_attr_key: "CODIGO".to_owned(),
            new_block_attr_default: String::new(),
            new_block_attrs: vec![crate::domain::blocks::BlockAttribute::new(
                "CODIGO", "Código", "BL-01",
            )],
            save_block_to_personal: true,
            block_insert_overrides: Vec::new(),
            block_insert_target: None,
            show_ergonomics: false,
            discipline: Default::default(),
            ergonomics_min_passage: 0.70,
            browser_filter: String::new(),
            browser_hover: None,
            cursor_world: None,
            zoom_target: None,
            window_title: String::new(),
            saved_settings: preferences,
            mcp_actions: 0,
            mcp_calls,
            mcp_status,
            show_mcp_dialog: false,
            mcp_feedback: None,
            tunnel: None,
            tunnel_token,
            fixture_turns: 0,
            fixture_preset: None,
            fixture_width_text: String::new(),
            fixture_depth_text: String::new(),
            fixture_load_text: String::new(),
            measurements: Vec::new(),
            modulation_enabled: preferences.modulation,
            show_dimensions: preferences.dimensions,
            masonry_width_text: String::new(),
            masonry_length_text: String::new(),
            masonry_joint_text: String::new(),
            masonry_plaster_text: String::new(),
            wall_height_text: String::new(),
            opening_height_text: String::new(),
            opening_sill_text: String::new(),
            clipboard: None,
            system_clipboard_note: None,
            paste_count: 0,
            context_point: None,
            modify_anchor: None,
            modify_target: None,
            modify_keep: false,
            offset_distance_text: "0,15".to_owned(),
            split_parts: 1,
            repeat_form: None,
            group_name_text: String::new(),
            edit_notice: None,
            ai_activity: ai_activity::AiActivity::default(),
            frame_pacer: pacing::FramePacer::default(),
            follow_ai: preferences.follow_ai,
            last_canvas_rect: None,
            batch_depth: 0,
            mcp_events,
            plan_log: plan_sync::PlanLog::default(),
            mcp_running: None,
            constraint_pick: None,
            constraint_texts: std::collections::HashMap::new(),
            tabs: vec![None],
            active_tab: 0,
            active_tab_id: 1,
            active_tab_title: None,
            active_tab_floor_id: None,
            active_hidden_layers: Vec::new(),
            active_project_id: 1,
            projects: std::collections::HashMap::new(),
            next_project_id: 2,
            next_tab_id: 2,
            active_pane: tabs::TabPane::Left,
            split_tab: None,
            split_ratio: 0.5,
            sync_report: crate::domain::dependencies::DependencySyncReport::default(),
            show_start_page,
            pending_recovery,
            user_templates,
            recent_files,
            thumbnail_cache: std::collections::HashMap::new(),
            show_command_palette: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
        };
        if restored {
            app.restore_tab_drafts();
        }
        app.sync_masonry_fields();
        if let Some(path) = initial_file {
            app.open_file(&path);
        }
        if !cfg!(test) && app.check_updates_on_start {
            app.start_update_check(context);
        }
        app
    }

    pub(crate) fn start_update_check(&mut self, context: &egui::Context) {
        let waker = context.clone();
        self.update_status = crate::io::updates::UpdateStatus::Checking;
        self.update_receiver = Some(crate::io::updates::spawn_check(
            crate::io::crash_log::app_version(),
            move || waker.request_repaint(),
        ));
    }

    pub(super) fn poll_update_check(&mut self) {
        let Some(receiver) = &self.update_receiver else {
            return;
        };
        let status = match receiver.try_recv() {
            Ok(status) => status,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => crate::io::updates::UpdateStatus::Failed(
                "A verificação de atualizações foi interrompida.".to_owned(),
            ),
        };
        if let crate::io::updates::UpdateStatus::Available(ref release) = status {
            self.message = format!(
                "Nova versão disponível: Trazo {} (Ajuda → Verificar atualizações).",
                release.tag
            );
        }
        self.update_status = status;
        self.update_receiver = None;
    }

    /// Starts the MCP server. A busy port (another Trazo window, say) only
    /// leaves MCP off; the editor works as usual.
    pub(super) fn start_mcp(
        context: &egui::Context,
        tunnel_token: mcp::server::TunnelToken,
        events: mcp::events::EventHub,
    ) -> (Option<mpsc::Receiver<ToolCall>>, McpStatus) {
        let (sender, receiver) = mpsc::channel();
        let waker = context.clone();
        // Each window has its own port (7799, 7800, ...), so an assistant can
        // work on several projects at once.
        let port = crate::io::instance::mcp_port(mcp::port());
        match mcp::server::start_with_token(port, sender, tunnel_token, events, move || {
            waker.request_repaint()
        }) {
            Ok(port) => (Some(receiver), McpStatus::Listening(port)),
            Err(error) => (None, McpStatus::Failed(error)),
        }
    }

    /// Opens another Trazo window for a second project.
    pub(super) fn open_new_window(&mut self) {
        self.message = match crate::io::instance::open_new_window() {
            Ok(()) => {
                "Nova janela aberta: cada janela edita um projeto, com servidor de IA próprio."
                    .to_owned()
            }
            Err(error) => error,
        };
    }

    /// Runs the tool calls the MCP server has queued, answering each.
    pub(super) fn process_mcp_calls(&mut self) {
        let Some(calls) = &self.mcp_calls else {
            return;
        };
        let pending: Vec<ToolCall> = calls.try_iter().collect();
        for call in pending {
            if Instant::now() > call.deadline {
                // The client gave up waiting; running it now would surprise it.
                continue;
            }
            let result = self.run_mcp_tool(&call.name, &call.arguments).to_result();
            let _ = call.reply.send(result);
        }
    }

    pub(super) fn save_recovery_if_pending(&mut self) {
        if self.recovery_pending {
            match recovery::save(&self.project) {
                Ok(()) => self.recovery_pending = false,
                Err(error) => {
                    self.message = format!("Não foi possível proteger o rascunho: {error}")
                }
            }
        }
    }

    pub(super) fn current_settings(&self) -> Settings {
        Settings {
            show_grid: self.show_grid,
            snap_enabled: self.snap_enabled,
            osnap: self.osnap,
            wall_reference: self.wall_reference,
            ortho_enabled: self.ortho_enabled,
            show_browser: self.show_browser,
            show_properties: self.show_properties,
            ribbon_collapsed: self.ribbon_collapsed,
            modulation: self.modulation_enabled,
            dimensions: self.show_dimensions,
            selection_filter: self.selection_filter,
            follow_ai: self.follow_ai,
            check_updates: self.check_updates_on_start,
        }
    }

    /// Writes the interface preferences when they change. It is best
    /// effort: preferences are a convenience, so a failing disk is neither
    /// reported nor retried every frame.
    pub(super) fn persist_settings(&mut self) {
        let current = self.current_settings();
        if current != self.saved_settings {
            if !cfg!(test) {
                let _ = settings::save(&current);
            }
            self.saved_settings = current;
        }
    }

    /// Keeps the window title in step with the document and its state.
    pub(super) fn update_window_title(&mut self, context: &egui::Context) {
        let title = format!(
            "{}{} — Trazo",
            if self.dirty { "• " } else { "" },
            self.document_name()
        );
        if title != self.window_title {
            context.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Pre-warms structural and electrical caches in the background thread
    /// whenever relevant elements exist, so dialogs and plan overlays stay responsive (#108).
    pub(super) fn schedule_background_analysis(&self) {
        if !self.project.structure.beams.is_empty()
            || !self.project.structure.slabs.is_empty()
            || self.project.fixtures.iter().any(|f| f.kind.is_structural())
        {
            self.background.schedule_structure(&self.project);
        }
        if !self.project.conduits.is_empty()
            || self.project.fixtures.iter().any(|f| f.kind.is_electrical())
        {
            self.background.schedule_electrical(&self.project);
        }
    }
}
