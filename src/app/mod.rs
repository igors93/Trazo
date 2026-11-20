mod ai_activity;
pub mod app_icon;
mod canvas;
mod interaction;
mod mcp_tools;
mod plan_sync;
mod selection;
mod tabs;
mod theme;
mod tools;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Key};

use crate::{
    domain::{
        fixtures::FixtureKind,
        geometry::Point,
        history::History,
        model::{OpeningKind, Project},
    },
    io::{
        export, project_file, recovery,
        settings::{self, Settings},
    },
    mcp::{self, server::ToolCall},
};

/// The state of the MCP server that lets AI assistants use Trazo.
#[derive(Clone, Debug, PartialEq)]
pub enum McpStatus {
    Off,
    Listening(u16),
    Failed(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DocDialog {
    #[default]
    None,
    Layers,
    Dimensions,
    Notes,
    Sheet,
    Schedules,
    Templates,
    Views,
    SheetSet,
    Circulation,
    Floors,
    Library,
    Ergonomics,
    Electrical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    /// Closing the active tab, which has unsaved changes.
    CloseTab,
    CloseApp,
}

/// How far an arrow key moves the selection: one grid step.
const NUDGE_STEP: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RibbonTab {
    Architecture,
    Edit,
    Interiors,
    Electrical,
    Drafting,
    View,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tool {
    Wall,
    Room,
    /// A room of any shape, drawn corner by corner.
    RoomPolygon,
    Terrain,
    Move,
    Opening(OpeningKind),
    /// Places counters, sinks, stoves, bathroom fittings...
    Fixture(FixtureKind),
    /// Measures between two points without touching the project.
    Measure,
    /// Draws lines, polylines, rectangles, arcs, circles and splines.
    Draw(tools::DrawKind),
    /// Copy, move, turn, mirror, offset, trim and extend.
    Modify(tools::ModifyKind),
    /// Keeps lines parallel or perpendicular, points together or distances fixed.
    Constrain(tools::ConstraintTool),
}

/// A selected element: the domain's reference to it.
use crate::domain::element::Element as Selection;

#[derive(Clone, Copy)]
struct ItemDrag {
    selection: Selection,
    last: Point,
    /// Where an opening was grabbed, relative to its centre.
    grab_offset: Point,
    moved: bool,
    /// The whole selection moves, not only the grabbed element.
    group: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum GripTarget {
    WallCorner(Point),
    DrawingHandle { id: u64, index: usize },
    TerrainCorner(usize),
}

#[derive(Clone, Copy)]
struct VertexDrag {
    target: GripTarget,
    moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CreationState {
    Idle,
    WaitingSecondPoint { start: Point },
    Dragging { start: Point },
}

pub struct FloorPlanApp {
    project: Project,
    history: History,
    file_path: Option<PathBuf>,
    dirty: bool,
    recovery_pending: bool,
    pending_action: Option<PendingAction>,
    tool: Tool,
    creation_state: CreationState,
    /// The primary selected element: the one the properties panel shows.
    selection: Option<Selection>,
    /// The other selected elements when several are (see `selection`).
    selection_extra: Vec<Selection>,
    /// Which kinds a selection box or "select all" picks.
    selection_filter: crate::domain::element::SelectionFilter,
    /// The selection box being drawn.
    box_select: Option<selection::BoxSelect>,
    item_drag: Option<ItemDrag>,
    vertex_drag: Option<VertexDrag>,
    drag_initial_project: Option<Project>,
    /// The corners of a room waiting for its name.
    pending_room: Option<Vec<Point>>,
    /// Corners of the free-shape room being drawn.
    polygon_draft: Vec<Point>,
    /// Points clicked with a drawing tool.
    draw_points: Vec<Point>,
    /// Direct numeric input typed while drawing (e.g. `3,47`, `3,5<45`, `3x4`, `1,5;2,0`).
    draft_input: String,
    /// Last snapped canvas point, used as aiming direction for numeric input.
    draft_aim: Option<Point>,
    drawing_radius_text: String,
    room_name: String,
    room_dialog_focus: bool,
    opening_drag: Option<OpeningKind>,
    scale: f32,
    pan: egui::Vec2,
    last_canvas_center: Option<egui::Pos2>,
    fit_pending: bool,
    snap_enabled: bool,
    osnap: crate::domain::snap::OsnapSettings,
    wall_reference: crate::domain::wall_reference::WallReference,
    terrain_width_text: String,
    terrain_height_text: String,
    wall_length_text: String,
    room_name_text: String,
    room_width_text: String,
    room_height_text: String,
    opening_width_text: String,
    message: String,
    ribbon_tab: RibbonTab,
    ribbon_collapsed: bool,
    show_grid: bool,
    ortho_enabled: bool,
    show_browser: bool,
    show_properties: bool,
    show_help: bool,
    show_report_dialog: bool,
    report_summary: String,
    report_include_log: bool,
    show_updates_dialog: bool,
    check_updates_on_start: bool,
    update_status: crate::io::updates::UpdateStatus,
    update_receiver: Option<mpsc::Receiver<crate::io::updates::UpdateStatus>>,
    active_doc_dialog: DocDialog,
    new_layer_name: String,
    new_note_text: String,
    new_revision_text: String,
    library_search: String,
    library_category_filter: Option<crate::domain::library::LibraryCategory>,
    new_library_name: String,
    new_library_width: f64,
    new_library_depth: f64,
    new_library_base: Option<FixtureKind>,
    show_ergonomics: bool,
    ergonomics_min_passage: f64,
    /// Fades architecture and furniture so the electrical design stands out.
    electrical_only: bool,
    browser_filter: String,
    browser_hover: Option<Selection>,
    cursor_world: Option<Point>,
    zoom_target: Option<(Point, Point)>,
    window_title: String,
    /// The interface preferences as last written to disk.
    saved_settings: Settings,
    /// Tools run by an AI assistant through MCP in this session.
    mcp_actions: u32,
    /// Tool calls from the MCP server, run on the UI thread.
    mcp_calls: Option<mpsc::Receiver<ToolCall>>,
    mcp_status: McpStatus,
    show_mcp_dialog: bool,
    /// The outcome of the last action in the "Conectar uma IA" window.
    mcp_feedback: Option<(bool, String)>,
    /// The public tunnel for cloud assistants, while it runs.
    tunnel: Option<mcp::tunnel::Tunnel>,
    /// Secret token required on requests coming through a public tunnel.
    tunnel_token: mcp::server::TunnelToken,
    /// Rotation of the next fixture placed away from walls.
    fixture_turns: u8,
    fixture_width_text: String,
    fixture_depth_text: String,
    /// Distances measured with the ruler in this session (never saved).
    measurements: Vec<(Point, Point)>,
    /// Walls and rooms follow the block module while drawn.
    modulation_enabled: bool,
    /// Dimension chains (cotas) around the building.
    show_dimensions: bool,
    masonry_width_text: String,
    masonry_length_text: String,
    masonry_joint_text: String,
    masonry_plaster_text: String,
    wall_height_text: String,
    opening_height_text: String,
    opening_sill_text: String,
    /// What Ctrl+C or Ctrl+X took, ready for Ctrl+V.
    clipboard: Option<crate::domain::model::Fragment>,
    /// Text to leave on the system clipboard after a copy, so Ctrl+V
    /// reaches the app (egui only reports it when the clipboard is not empty).
    system_clipboard_note: Option<String>,
    /// Pastes since the last copy, to step pastes made without the pointer.
    paste_count: u32,
    /// Where the plan's context menu was last opened.
    context_point: Option<Point>,
    /// The first point of a modify tool: base point, centre or first point
    /// of the mirror axis.
    modify_anchor: Option<Point>,
    /// The wall or drawing picked for an offset.
    modify_target: Option<Selection>,
    /// Turning and mirroring make copies, keeping the originals.
    modify_keep: bool,
    offset_distance_text: String,
    /// Dividing: 1 cuts where clicked, 2 or more into equal parts.
    split_parts: u32,
    /// The "Repetir" dialog, while open.
    repeat_form: Option<tools::RepeatForm>,
    /// The name of the selected group, as typed in the properties panel.
    group_name_text: String,
    /// Why the last edit was refused, shown again at the end of the frame
    /// over any message written after it.
    edit_notice: Option<String>,
    /// What the connected assistant did in this session, shown on the plan.
    ai_activity: ai_activity::AiActivity,
    /// The view follows what the assistant changes.
    follow_ai: bool,
    /// Where the plan was drawn last frame.
    last_canvas_rect: Option<egui::Rect>,
    /// Inside an AI batch: edits are recorded as one undo step at its end.
    batch_depth: u32,
    /// Feeds the event streams of connected assistants.
    mcp_events: mcp::events::EventHub,
    /// The plan's revisions and who changed what.
    plan_log: plan_sync::PlanLog,
    /// The assistant's tool running now, if any.
    mcp_running: Option<String>,
    /// The first pick of a constraint tool.
    constraint_pick: Option<tools::ConstraintPick>,
    /// The distances of the constraints, as typed in the properties panel.
    constraint_texts: std::collections::HashMap<u64, String>,
    /// Every open tab; the active one is `None`, its project lives in the
    /// fields above (see `tabs.rs`).
    tabs: Vec<Option<tabs::Document>>,
    active_tab: usize,
    /// Which side holds the active tab when viewing side by side.
    active_pane: tabs::TabPane,
    /// The tab shown beside the active one, with the same view.
    split_tab: Option<usize>,
    /// Horizontal fraction (`0.20..=0.80`) given to the left side when split.
    split_ratio: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenResult {
    Success,
    Error,
}

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
        let restored = recovered.is_some();
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
            project: recovered.unwrap_or_default(),
            history: History::new(),
            file_path: None,
            dirty: restored,
            recovery_pending: false,
            pending_action: None,
            tool: Tool::Move,
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
            new_layer_name: String::new(),
            new_note_text: String::new(),
            new_revision_text: String::new(),
            library_search: String::new(),
            library_category_filter: None,
            new_library_name: String::new(),
            new_library_width: 1.20,
            new_library_depth: 0.60,
            new_library_base: Some(FixtureKind::Counter),
            show_ergonomics: false,
            electrical_only: false,
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
            fixture_width_text: String::new(),
            fixture_depth_text: String::new(),
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
            active_pane: tabs::TabPane::Left,
            split_tab: None,
            split_ratio: 0.5,
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

    pub(super) fn start_update_check(&mut self, context: &egui::Context) {
        let waker = context.clone();
        self.update_status = crate::io::updates::UpdateStatus::Checking;
        self.update_receiver = Some(crate::io::updates::spawn_check(
            crate::io::crash_log::app_version(),
            move || waker.request_repaint(),
        ));
    }

    fn poll_update_check(&mut self) {
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
    fn start_mcp(
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
    fn open_new_window(&mut self) {
        self.message = match crate::io::instance::open_new_window() {
            Ok(()) => {
                "Nova janela aberta: cada janela edita um projeto, com servidor de IA próprio."
                    .to_owned()
            }
            Err(error) => error,
        };
    }

    /// Runs the tool calls the MCP server has queued, answering each.
    fn process_mcp_calls(&mut self) {
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

    fn save_recovery_if_pending(&mut self) {
        if self.recovery_pending {
            match recovery::save(&self.project) {
                Ok(()) => self.recovery_pending = false,
                Err(error) => {
                    self.message = format!("Não foi possível proteger o rascunho: {error}")
                }
            }
        }
    }

    fn current_settings(&self) -> Settings {
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
    fn persist_settings(&mut self) {
        let current = self.current_settings();
        if current != self.saved_settings {
            if !cfg!(test) {
                let _ = settings::save(&current);
            }
            self.saved_settings = current;
        }
    }

    #[allow(dead_code)]
    pub fn creation_state(&self) -> CreationState {
        self.creation_state
    }

    pub fn draft_start(&self) -> Option<Point> {
        match self.creation_state {
            // A free-shape room continues from its last corner.
            // A modify tool measures from its base point.
            CreationState::Idle => self
                .polygon_draft
                .last()
                .or(self.draw_points.last())
                .copied()
                .or(self.modify_anchor),
            CreationState::WaitingSecondPoint { start } => Some(start),
            CreationState::Dragging { start } => Some(start),
        }
    }

    pub fn cancel_creation(&mut self) {
        self.creation_state = CreationState::Idle;
        self.polygon_draft.clear();
        self.draw_points.clear();
        self.draft_input.clear();
        self.modify_anchor = None;
        self.modify_target = None;
        self.constraint_pick = None;
    }

    /// Ends an element drag in progress, keeping what it moved as one undo
    /// step. Whatever interrupts a drag (a tool key, Delete) goes through
    /// here, so a move is never left outside the history and unsaved.
    fn commit_item_drag(&mut self) {
        // A selection box cut short selects nothing.
        self.box_select = None;
        let drag = self.item_drag.take();
        let vertex = self.vertex_drag.take();
        let moved = drag.is_some_and(|d| d.moved) || vertex.is_some_and(|v| v.moved);
        if let Some(initial) = self.drag_initial_project.take()
            && moved
        {
            self.record_change(initial);
            self.sync_inspector_fields();
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_item_drag();
        self.tool = tool;
        self.cancel_creation();
        // Moving, copying, turning and mirroring act on the selection.
        if !matches!(tool, Tool::Modify(kind) if kind.uses_selection()) {
            self.clear_selection();
        }
        self.message = match tool {
            Tool::Wall => "Clique em dois pontos para desenhar uma parede.",
            Tool::Room => "Clique em dois cantos opostos e dê um nome ao cômodo.",
            Tool::RoomPolygon => {
                "Clique em cada canto do cômodo (em L, U ou livre); feche no primeiro canto ou com Enter."
            }
            Tool::Terrain => {
                "Clique em dois cantos ou digite largura e comprimento para criar o terreno."
            }
            Tool::Move => "Clique para selecionar. Arraste parede, cômodo ou terreno para mover.",
            Tool::Opening(OpeningKind::Door) => "Clique em uma parede para colocar uma porta.",
            Tool::Opening(OpeningKind::Gate) => {
                "Clique no muro ou na parede da frente para colocar o portão (de correr, 3,00 m)."
            }
            Tool::Opening(OpeningKind::Window) => "Clique em uma parede para colocar uma janela.",
            Tool::Opening(OpeningKind::SaloonDoor) => {
                "Clique em uma parede para colocar uma porta vai-e-vem."
            }
            Tool::Opening(OpeningKind::Gap) => {
                "Clique em uma parede para abrir um vão livre; ajuste a largura no painel."
            }
            Tool::Fixture(_) => {
                "Clique para posicionar. Perto de uma parede o objeto encosta nela; G gira."
            }
            Tool::Draw(kind) => kind.hint(),
            Tool::Modify(kind) => kind.hint(),
            Tool::Constrain(kind) => kind.hint(),
            Tool::Measure => {
                "Clique em dois pontos para medir. Encaixa em cantos, faces e batentes; nada é alterado."
            }
        }
        .to_owned();
    }

    /// A blank plan in a new tab; the current project stays open in its own.
    pub fn request_new_project(&mut self) {
        self.new_tab();
    }

    /// Opens a project in a new tab (or in the active one when it is blank).
    pub fn request_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Projeto Trazo", &["trazo", "json"])
            .pick_file()
        {
            let _ = self.open_in_tab(&path);
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
    fn reset_editing_state(&mut self) {
        self.ai_activity.forget_highlight();
        self.cancel_creation();
        self.clear_selection();
        self.box_select = None;
        self.item_drag = None;
        self.vertex_drag = None;
        self.drag_initial_project = None;
        self.pending_room = None;
    }

    fn new_project(&mut self) {
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
        match project_file::load(path) {
            Ok((project, warnings)) => {
                recovery::clear();
                self.measurements.clear();
                let before = std::mem::replace(&mut self.project, project);
                self.note_plan_change(&before, true);
                self.history.clear();
                self.file_path = Some(path.to_path_buf());
                self.dirty = false;
                self.recovery_pending = false;
                self.pending_action = None;
                self.tool = Tool::Move;
                self.reset_editing_state();
                self.sync_inspector_fields();
                self.fit_view_requested();
                if warnings.is_empty() {
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

    fn save(&mut self) -> bool {
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

    /// Saves the project to `path`, which becomes the current file.
    fn write_project(&mut self, path: PathBuf) -> bool {
        match project_file::save(&self.project, &path) {
            Ok(()) => {
                recovery::clear();
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

    fn complete_creation(&mut self, start: Point, end: Point) {
        match self.tool {
            Tool::Terrain => {
                let previous = self.project.clone();
                if let Some(id) = self.project.set_terrain(start, end) {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.set_selection(Some(Selection::Terrain(id)));
                    if let Some(terrain) = &self.project.terrain {
                        self.terrain_width_text =
                            format!("{:.2}", terrain.width()).replace('.', ",");
                        self.terrain_height_text =
                            format!("{:.2}", terrain.height()).replace('.', ",");
                    }
                    self.fit_view_requested();
                    self.message =
                        "Terreno criado. Ajuste as medidas à direita se desejar.".to_owned();
                } else {
                    self.message = "Dimensões do terreno muito pequenas. Mínimo 0,25 m.".to_owned();
                }
            }
            Tool::Wall => {
                let (wall_start, wall_end) = self.wall_reference.offset_segment(
                    start,
                    end,
                    self.project.masonry.thickness(),
                );
                let previous = self.project.clone();
                if self.project.add_wall(wall_start, wall_end).is_some() {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.message = if self.wall_reference
                        == crate::domain::wall_reference::WallReference::Center
                    {
                        "Parede criada.".to_owned()
                    } else {
                        format!(
                            "Parede criada ({}).",
                            self.wall_reference.label().to_lowercase()
                        )
                    };
                } else {
                    self.message =
                        "A parede deve caber dentro do terreno e ter pelo menos 0,25 m.".to_owned();
                }
            }
            Tool::Room => {
                if (start.x - end.x).abs() >= 0.5 && (start.y - end.y).abs() >= 0.5 {
                    let Some((min, max)) = self.wall_reference.offset_rect(
                        start,
                        end,
                        self.project.masonry.thickness(),
                    ) else {
                        self.message =
                            "Dimensões do cômodo muito pequenas para a referência escolhida."
                                .to_owned();
                        return;
                    };
                    self.pending_room = Some(vec![
                        min,
                        Point::new(max.x, min.y),
                        max,
                        Point::new(min.x, max.y),
                    ]);
                    self.room_name.clear();
                    self.room_dialog_focus = true;
                    self.message = "Digite o nome do cômodo para concluir.".to_owned();
                } else {
                    self.message = "Dimensões do cômodo muito pequenas. Mínimo 0,50 m.".to_owned();
                }
            }
            Tool::Measure => {
                self.measurements.push((start, end));
                let (dx, dy) = ((end.x - start.x).abs(), (end.y - start.y).abs());
                self.message = format!(
                    "Medida: {}{}. Meça de novo ou Esc para limpar.",
                    crate::domain::geometry::format_meters(start.distance(end)),
                    if dx > 0.005 && dy > 0.005 {
                        format!(" (Δx {:.2} · Δy {:.2})", dx, dy).replace('.', ",")
                    } else {
                        String::new()
                    }
                );
            }
            _ => {}
        }
    }

    fn export_sheet_pdf(&mut self) {
        let default_name = format!(
            "prancha-{}-{}.pdf",
            self.project.sheet.size.short_name().to_lowercase(),
            self.project.sheet.scale.label().replace(':', "-")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Prancha PDF em escala", &["pdf"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::export_scaled_pdf(&self.project, &path) {
            Ok(()) => format!(
                "Prancha {} em escala {} exportada em PDF.",
                self.project.sheet.size.short_name(),
                self.project.sheet.scale.label()
            ),
            Err(error) => format!("Não foi possível exportar a prancha PDF: {error}"),
        };
    }

    fn export_sheet_svg(&mut self) {
        let default_name = format!(
            "prancha-{}-{}.svg",
            self.project.sheet.size.short_name().to_lowercase(),
            self.project.sheet.scale.label().replace(':', "-")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Prancha SVG em escala", &["svg"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::export_scaled_svg(&self.project, &path) {
            Ok(()) => format!(
                "Prancha {} em escala {} exportada em SVG.",
                self.project.sheet.size.short_name(),
                self.project.sheet.scale.label()
            ),
            Err(error) => format!("Não foi possível exportar a prancha SVG: {error}"),
        };
    }

    fn export_png(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagem PNG", &["png"])
            .set_file_name("planta-baixa.png")
            .save_file()
        else {
            return;
        };
        self.message = match export::png(&self.project, &path) {
            Ok(()) => "Imagem PNG exportada.".to_owned(),
            Err(error) => format!("Não foi possível exportar PNG: {error}"),
        };
    }

    fn export_pdf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name("planta-baixa.pdf")
            .save_file()
        else {
            return;
        };
        self.message = match export::pdf(&self.project, &path) {
            Ok(()) => "PDF exportado.".to_owned(),
            Err(error) => format!("Não foi possível exportar PDF: {error}"),
        };
    }

    fn export_svg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho SVG editável", &["svg"])
            .set_file_name("planta-baixa.svg")
            .save_file()
        else {
            return;
        };
        self.message = match export::svg(&self.project, &path) {
            Ok(()) => "SVG editável exportado.".to_owned(),
            Err(error) => format!("Não foi possível exportar SVG: {error}"),
        };
    }

    fn export_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planilha CSV", &["csv"])
            .set_file_name("quantitativos.csv")
            .save_file()
        else {
            return;
        };
        self.message = match export::csv(&self.project, &path) {
            Ok(()) => "CSV de áreas, esquadrias e quantitativos exportado.".to_owned(),
            Err(error) => format!("Não foi possível exportar CSV: {error}"),
        };
    }

    fn export_dxf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DXF", &["dxf"])
            .set_file_name("planta-baixa.dxf")
            .save_file()
        else {
            return;
        };
        self.message = match crate::io::dxf::export(&self.project, &path) {
            Ok(()) => "Desenho DXF exportado (camadas, cotas e unidades em metros).".to_owned(),
            Err(error) => format!("Não foi possível exportar DXF: {error}"),
        };
    }

    fn import_dxf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DXF", &["dxf"])
            .pick_file()
        else {
            return;
        };
        let previous = self.project.clone();
        match crate::io::dxf::import_file(&mut self.project, &path) {
            Ok(report) => {
                if report.total_imported() > 0 || report.layers > 0 {
                    self.record_change(previous);
                    self.fit_view_requested();
                }
                self.message = report.summary_pt();
            }
            Err(error) => {
                self.project = previous;
                self.message = format!("Não foi possível importar DXF: {error}");
            }
        }
    }

    /// Drops from the selection whatever the project no longer has (after
    /// an undo, say); the last element left becomes the primary one.
    pub fn revalidate_selection(&mut self) {
        let kept: Vec<Selection> = self
            .selected()
            .into_iter()
            .filter(|item| self.project.element_exists(*item))
            .collect();
        self.set_selection_set(kept);
    }

    fn undo(&mut self) {
        let before = self.project.clone();
        if self.history.undo(&mut self.project) {
            self.note_plan_change(&before, false);
            self.dirty = true;
            self.recovery_pending = true;
            self.revalidate_selection();
            self.item_drag = None;
            self.drag_initial_project = None;
            self.creation_state = CreationState::Idle;
            self.pending_room = None;
            self.sync_inspector_fields();
            self.message = "Ação desfeita.".to_owned();
        }
    }

    fn redo(&mut self) {
        let before = self.project.clone();
        if self.history.redo(&mut self.project) {
            self.note_plan_change(&before, false);
            self.dirty = true;
            self.recovery_pending = true;
            self.revalidate_selection();
            self.item_drag = None;
            self.drag_initial_project = None;
            self.creation_state = CreationState::Idle;
            self.pending_room = None;
            self.sync_inspector_fields();
            self.message = "Ação refeita.".to_owned();
        }
    }

    fn fit_view_requested(&mut self) {
        self.fit_pending = true;
    }

    fn fit_view(&mut self, rect: egui::Rect) {
        self.fit_pending = false;
        if let Some((min, max)) = self.project.bounds() {
            self.fit_bounds(rect, min, max, 240.0);
        } else {
            self.pan = egui::Vec2::ZERO;
            self.scale = 60.0;
        }
    }

    /// Centres the box `min`–`max` in `rect` at the largest scale that shows
    /// it whole, never above `max_scale`.
    fn fit_bounds(&mut self, rect: egui::Rect, min: Point, max: Point, max_scale: f32) {
        let width = (max.x - min.x).max(0.25) as f32;
        let height = (max.y - min.y).max(0.25) as f32;
        self.scale = ((rect.width() - 100.0).max(80.0) / width)
            .min((rect.height() - 100.0).max(80.0) / height)
            .clamp(3.0, max_scale);
        let scale = f64::from(self.scale);
        self.pan = egui::vec2(
            (-(min.x + max.x) * scale / 2.0) as f32,
            (-(min.y + max.y) * scale / 2.0) as f32,
        );
    }

    /// A distance on screen, in metres at the current zoom.
    fn screen_metres(&self, pixels: f32) -> f64 {
        f64::from(pixels) / f64::from(self.scale)
    }

    fn prepare_canvas_view(&mut self, rect: egui::Rect) {
        if self.fit_pending {
            self.fit_view(rect);
        } else if let Some((min, max)) = self.zoom_target.take() {
            self.fit_bounds(rect, min, max, 120.0);
        } else if let Some(previous_center) = self.last_canvas_center {
            self.pan += previous_center - rect.center();
        }
        self.last_canvas_center = Some(rect.center());
        self.last_canvas_rect = Some(rect);
    }

    /// Zooms by `factor` around the centre of the view.
    fn zoom_by(&mut self, factor: f32) {
        let scale = (self.scale * factor).clamp(3.0, 240.0);
        self.pan *= scale / self.scale;
        self.scale = scale;
    }

    fn zoom_percent(&self) -> f32 {
        self.scale / 60.0 * 100.0
    }

    /// The box an element occupies, for zooming to it.
    fn selection_bounds(&self, selection: Selection) -> Option<(Point, Point)> {
        let span = |points: &[Point]| {
            let mut min = points[0];
            let mut max = points[0];
            for point in points {
                min = Point::new(min.x.min(point.x), min.y.min(point.y));
                max = Point::new(max.x.max(point.x), max.y.max(point.y));
            }
            (
                Point::new(min.x - 0.75, min.y - 0.75),
                Point::new(max.x + 0.75, max.y + 0.75),
            )
        };
        match selection {
            Selection::Wall(id) => {
                let wall = self.project.walls.iter().find(|wall| wall.id == id)?;
                Some(span(&[wall.start, wall.end]))
            }
            Selection::Room(id) => {
                let room = self.project.rooms.iter().find(|room| room.id == id)?;
                let (min, max) = self.project.room_bounds(room)?;
                Some(span(&[min, max]))
            }
            Selection::Terrain(_) => {
                let terrain = self.project.terrain.as_ref()?;
                Some(span(&[terrain.min, terrain.max]))
            }
            Selection::Fixture(id) => {
                let fixture = self.project.fixtures.iter().find(|f| f.id == id)?;
                let (low, high) = fixture.bounds();
                Some(span(&[low, high]))
            }
            Selection::Drawing(id) => {
                let (low, high) = self.project.drawing(id)?.shape.bounds();
                Some(span(&[low, high]))
            }
            Selection::Opening(id) => {
                let opening = self.project.openings.iter().find(|o| o.id == id)?;
                let center = self.opening_center(id)?;
                let half = opening.width() / 2.0 + 1.0;
                Some(span(&[
                    Point::new(center.x - half, center.y - half),
                    Point::new(center.x + half, center.y + half),
                ]))
            }
        }
    }

    /// The middle of an opening, on its wall's centre line.
    fn opening_center(&self, id: u64) -> Option<Point> {
        let opening = self.project.openings.iter().find(|o| o.id == id)?;
        let wall = self
            .project
            .walls
            .iter()
            .find(|wall| wall.id == opening.wall_id)?;
        Some(Point::new(
            wall.start.x + (wall.end.x - wall.start.x) * opening.t,
            wall.start.y + (wall.end.y - wall.start.y) * opening.t,
        ))
    }

    fn selection_message(selection: Option<Selection>) -> &'static str {
        match selection {
            Some(Selection::Opening(_)) => {
                "Abertura selecionada. Arraste ao longo da parede ou altere a largura no painel."
            }
            Some(Selection::Wall(_)) => {
                "Parede selecionada. Arraste para mover ou altere o comprimento no painel."
            }
            Some(Selection::Room(_)) => {
                "Cômodo selecionado. Arraste para mover ou altere as medidas no painel."
            }
            Some(Selection::Terrain(_)) => {
                "Terreno selecionado. Ajuste as medidas ou arraste para mover."
            }
            Some(Selection::Fixture(_)) => {
                "Objeto selecionado. Arraste para mover, G para girar ou ajuste as medidas no painel."
            }
            Some(Selection::Drawing(_)) => {
                "Desenho selecionado. Arraste ou use as setas para mover; Delete exclui."
            }
            None => "Clique em um elemento para selecionar.",
        }
    }

    /// Selects an element from outside the canvas (the project browser),
    /// switching to the selection tool.
    fn select(&mut self, selection: Selection) {
        if self.tool != Tool::Move {
            self.set_tool(Tool::Move);
        }
        self.cancel_creation();
        self.set_selection(Some(selection));
        self.sync_inspector_fields();
        self.message = Self::selection_message(self.selection).to_owned();
    }

    /// Zooms to `selection`, or to everything selected when it is part of
    /// a selection of several elements.
    fn zoom_to_selection(&mut self, selection: Selection) {
        let bounds = if self.is_multi_selection() && self.is_selected(selection) {
            self.project
                .elements_bounds(&self.selected())
                .map(|(low, high)| {
                    (
                        Point::new(low.x - 0.75, low.y - 0.75),
                        Point::new(high.x + 0.75, high.y + 0.75),
                    )
                })
        } else {
            self.selection_bounds(selection)
        };
        if let Some(bounds) = bounds {
            self.zoom_target = Some(bounds);
        }
    }

    /// Moves the selected element by `delta` metres as one undo step,
    /// within the terrain like a drag. Doors and windows slide along their
    /// wall, so only the part of `delta` along the wall counts.
    fn selection_is_locked(&self, selection: Selection) -> bool {
        use crate::domain::layers::LayerCategory;
        match selection {
            Selection::Terrain(_) => self.project.category_locked(LayerCategory::Terrain),
            Selection::Room(_) => self.project.category_locked(LayerCategory::Rooms),
            Selection::Opening(_) => self.project.category_locked(LayerCategory::Openings),
            Selection::Fixture(id) => self
                .project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .is_some_and(|fixture| {
                    self.project
                        .category_locked(crate::domain::model::Project::fixture_category(fixture))
                }),
            Selection::Wall(id) => self
                .project
                .walls
                .iter()
                .find(|w| w.id == id)
                .is_some_and(|w| self.project.is_wall_locked(w)),
            Selection::Drawing(id) => self
                .project
                .drawing(id)
                .is_some_and(|d| self.project.is_drawing_locked(d)),
        }
    }

    fn nudge_selection(&mut self, delta: Point) {
        let Some(selection) = self.selection else {
            return;
        };
        if self
            .selected()
            .into_iter()
            .any(|selected| self.selection_is_locked(selected))
        {
            self.message = "A camada deste elemento está bloqueada.".to_owned();
            return;
        }
        if self.item_drag.is_some() || self.draft_start().is_some() {
            return;
        }
        let previous = self.project.clone();
        if self.is_multi_selection() {
            if let Err(error) = self.project.move_elements(&self.selected(), delta) {
                self.message = error;
                return;
            }
            if self.project == previous {
                self.message =
                    "Não há espaço para mover a seleção nessa direção dentro do terreno."
                        .to_owned();
                return;
            }
            if !self.record_change(previous) {
                return;
            }
            self.sync_inspector_fields();
            self.message = format!(
                "{} movidos {}. Setas: 0,25 m · Shift+setas: 1 m.",
                self.selection_summary(),
                crate::domain::geometry::format_meters(delta.x.abs().max(delta.y.abs()))
            );
            return;
        }
        match selection {
            Selection::Wall(id) => {
                if let Err(error) = self.project.move_wall_checked(id, delta) {
                    self.message = error;
                    return;
                }
            }
            Selection::Room(id) => {
                if !self.project.move_room(id, delta) {
                    self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                    return;
                }
            }
            Selection::Terrain(id) => self.project.move_terrain(id, delta),
            Selection::Fixture(id) => self.project.move_fixture(id, delta),
            Selection::Drawing(id) => self.project.move_drawing(id, delta),
            Selection::Opening(id) => {
                if let Some(center) = self.opening_center(id) {
                    self.project.move_opening_along_wall(
                        id,
                        Point::new(center.x + delta.x, center.y + delta.y),
                    );
                }
            }
        }
        if self.project == previous {
            self.message = match selection {
                Selection::Opening(_) => {
                    "A abertura só se move ao longo da parede, e não há espaço nessa direção."
                }
                _ => "Não há espaço para mover nessa direção dentro do terreno.",
            }
            .to_owned();
            return;
        }
        if !self.record_change(previous) {
            return;
        }
        self.sync_inspector_fields();
        self.message = format!(
            "Seleção movida {}. Setas: 0,25 m · Shift+setas: 1 m.",
            crate::domain::geometry::format_meters(delta.x.abs().max(delta.y.abs()))
        );
    }

    /// G: turns the selected fixture, or the next one to be placed; any
    /// other selection turns 90° clockwise about its centre.
    fn rotate_fixture_or_next(&mut self) {
        if matches!(self.tool, Tool::Fixture(_)) {
            self.fixture_turns = (self.fixture_turns + 1) % 4;
            self.message = format!(
                "Próximo objeto girado para {}° (longe das paredes).",
                self.fixture_turns as u32 * 90
            );
        } else if let Some(Selection::Fixture(id)) = self.selection
            && !self.is_multi_selection()
        {
            let previous = self.project.clone();
            match self.project.rotate_fixture(id) {
                Ok(()) => {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.sync_inspector_fields();
                    self.message = "Objeto girado 90°.".to_owned();
                }
                Err(error) => self.message = error,
            }
        } else if self.selection.is_some() {
            self.rotate_selection_quarter(true);
        }
    }

    /// I / Shift+I: the selected door opens to the other side of its wall
    /// or turns on the other jamb.
    fn flip_selected_door(&mut self, side: bool, hinge: bool) {
        let Some(Selection::Opening(id)) = self.selection else {
            self.message = "Selecione uma porta para inverter.".to_owned();
            return;
        };
        let is_door = self
            .project
            .openings
            .iter()
            .any(|opening| opening.id == id && opening.kind.is_door());
        if !is_door {
            self.message = "Só portas mudam de lado.".to_owned();
            return;
        }
        let previous = self.project.clone();
        self.project.flip_opening(id, side, hinge);
        if !self.record_change(previous) {
            return;
        }
        self.message = match (side, hinge) {
            (true, false) => "Porta abrindo para o outro lado da parede.",
            (false, true) => "Dobradiça da porta trocada de lado.",
            _ => "Porta invertida.",
        }
        .to_owned();
    }

    /// The material fields show the active brick or block, in centimetres.
    fn sync_masonry_fields(&mut self) {
        let cm = |value: f64| {
            let value = value * 100.0;
            if (value - value.round()).abs() < 0.05 {
                format!("{value:.0}")
            } else {
                format!("{value:.1}").replace('.', ",")
            }
        };
        let material = &self.project.masonry;
        self.masonry_width_text = cm(material.block_width);
        self.masonry_length_text = cm(material.block_length);
        self.masonry_joint_text = cm(material.joint);
        self.masonry_plaster_text = cm(material.plaster);
        self.wall_height_text = format!("{:.2}", self.project.wall_height).replace('.', ",");
    }

    /// Makes `material` the one new walls are built with (one undo step).
    fn set_masonry(&mut self, material: crate::domain::masonry::Masonry) {
        if !material.valid() {
            self.message =
                "Medidas de bloco inválidas: largura de 4 a 50 cm, comprimento de 5 a 100 cm, junta até 5 cm e reboco até 8 cm."
                    .to_owned();
            self.sync_masonry_fields();
            return;
        }
        if material == self.project.masonry {
            return;
        }
        let previous = self.project.clone();
        self.project.masonry = material;
        if !self.record_change(previous) {
            return;
        }
        self.sync_masonry_fields();
        self.message = format!(
            "Novas paredes: {} (parede acabada de {}).",
            self.project.masonry.label(),
            crate::domain::geometry::format_meters(self.project.masonry.thickness())
        );
    }

    /// Reads the brick or block typed in centimetres.
    fn apply_masonry_fields(&mut self) {
        let parse = |value: &str| {
            value
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .map(|cm| cm / 100.0)
        };
        let (Some(width), Some(length), Some(joint), Some(plaster)) = (
            parse(&self.masonry_width_text),
            parse(&self.masonry_length_text),
            parse(&self.masonry_joint_text),
            parse(&self.masonry_plaster_text),
        ) else {
            self.message = "Informe as medidas do bloco em centímetros (ex.: 14 e 39).".to_owned();
            return;
        };
        let mut material = self.project.masonry.clone();
        let changed = (material.block_width - width).abs() > 1e-5
            || (material.block_length - length).abs() > 1e-5;
        material.block_width = width;
        material.block_length = length;
        material.joint = joint;
        material.plaster = plaster;
        if changed {
            material.name = "Personalizado".to_owned();
        }
        self.set_masonry(material);
    }

    /// Builds the selected wall, or every wall, with the active material.
    fn apply_masonry_to_walls(&mut self, only: Option<u64>) {
        let material = self.project.masonry.clone();
        let previous = self.project.clone();
        match only {
            Some(id) => {
                self.project.set_wall_masonry(id, &material);
            }
            None => self.project.set_all_walls_masonry(&material),
        }
        if !self.record_change(previous) {
            return;
        }
        self.message = format!(
            "{} construída(s) com {}.",
            if only.is_some() {
                "Parede"
            } else {
                "Todas as paredes"
            },
            material.label()
        );
    }

    fn apply_wall_height(&mut self) {
        let Some(height) = self
            .wall_height_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|height| (1.5..=10.0).contains(height))
        else {
            self.message = "Pé-direito deve estar entre 1,50 e 10,00 m.".to_owned();
            return;
        };
        if (height - self.project.wall_height).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        self.project.wall_height = height;
        self.record_change(previous);
    }

    /// Applies the height and sill typed in the properties panel.
    fn apply_opening_heights(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(opening) = self.project.openings.iter().find(|o| o.id == id).cloned() else {
            return;
        };
        let height = parse(&self.opening_height_text).filter(|height| {
            opening
                .height()
                .is_none_or(|current| (current - height).abs() > 1e-4)
        });
        let sill = if opening.kind == OpeningKind::Window {
            parse(&self.opening_sill_text).filter(|sill| (opening.sill() - sill).abs() > 1e-4)
        } else {
            None
        };
        if height.is_none() && sill.is_none() {
            return;
        }
        let previous = self.project.clone();
        match self.project.set_opening_heights(id, height, sill) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
            }
            Err(error) => self.message = error,
        }
    }

    /// Applies the width and depth typed in the properties panel.
    fn apply_fixture_size(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (Some(width), Some(depth)) = (
            parse(&self.fixture_width_text),
            parse(&self.fixture_depth_text),
        ) else {
            self.message = "Informe largura e profundidade válidas em metros.".to_owned();
            return;
        };
        let previous = self.project.clone();
        match self
            .project
            .update_fixture(id, None, Some((width, depth)), None)
        {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message = format!("Objeto com {width:.2} × {depth:.2} m.").replace('.', ",");
            }
            Err(error) => self.message = error,
        }
    }

    fn toggle_snap(&mut self) {
        self.snap_enabled = !self.snap_enabled;
        self.message = if self.snap_enabled {
            "Encaixe ativado: grade de 0,25 m e extremidades de paredes."
        } else {
            "Encaixe desativado."
        }
        .to_owned();
    }

    fn toggle_ortho(&mut self) {
        self.ortho_enabled = !self.ortho_enabled;
        self.message = if self.ortho_enabled {
            "Modo orto ativado: paredes apenas horizontais ou verticais."
        } else {
            "Modo orto desativado."
        }
        .to_owned();
    }

    fn toggle_grid(&mut self) {
        self.show_grid = !self.show_grid;
    }

    fn document_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sem título".to_owned())
    }

    fn create_terrain_from_dimensions(&mut self) {
        let Some((width, height)) = self.terrain_dimensions() else {
            self.message = "Informe medidas válidas de pelo menos 0,25 m.".to_owned();
            return;
        };
        if !width.is_finite() || !height.is_finite() || width < 0.25 || height < 0.25 {
            self.message = "Informe medidas de pelo menos 0,25 m.".to_owned();
            return;
        }
        if let Some(terrain) = &self.project.terrain
            && (terrain.width() - width).abs() < 1e-4
            && (terrain.height() - height).abs() < 1e-4
        {
            return;
        }
        let previous = self.project.clone();
        let existing = self.project.terrain.as_ref().map(|terrain| terrain.id);
        let id = if let Some(id) = existing {
            self.project.resize_terrain(width, height);
            id
        } else {
            let origin = self
                .project
                .content_bounds()
                .map(|(min, max)| {
                    Point::new(
                        (min.x + max.x - width) / 2.0,
                        (min.y + max.y - height) / 2.0,
                    )
                })
                .unwrap_or(Point::new(0.0, 0.0));
            self.project
                .set_terrain(origin, Point::new(origin.x + width, origin.y + height))
                .unwrap()
        };
        if !self.record_change(previous) {
            return;
        }
        self.set_selection(Some(Selection::Terrain(id)));
        self.fit_view_requested();
        self.message = format!(
            "Terreno {}: {:.2} m².",
            if existing.is_some() {
                "atualizado"
            } else {
                "criado"
            },
            width * height
        )
        .replace('.', ",");
        if !self.project.content_inside_terrain() {
            self.message
                .push_str(" Há paredes fora do terreno; use Centralizar planta.");
        }
    }

    fn center_content_on_terrain(&mut self) {
        let previous = self.project.clone();
        if self.project.center_content_on_terrain() {
            if !self.record_change(previous) {
                return;
            }
            self.fit_view_requested();
            self.message = "Planta centralizada no terreno.".to_owned();
        } else {
            self.message = "A planta precisa caber no terreno para ser centralizada.".to_owned();
        }
    }

    fn detect_enclosed_rooms(&mut self) {
        let previous = self.project.clone();
        let count = self.project.create_rooms_from_enclosed_walls();
        if count > 0 {
            self.record_change(previous);
            self.message = format!(
                "{} {} detectado{} automaticamente.",
                count,
                if count == 1 { "ambiente" } else { "ambientes" },
                if count == 1 { "" } else { "s" }
            );
        } else {
            self.message = "Nenhum novo ambiente fechado foi encontrado.".to_owned();
        }
    }

    fn subdivide_terrain(&mut self, vertical: bool) {
        let Some(terrain) = &self.project.terrain else {
            self.message = "Defina o terreno antes de desmembrar.".to_owned();
            return;
        };
        let margin = terrain.width().max(terrain.height()) + 1.0;
        let (a, b) = if vertical {
            let x = (terrain.min.x + terrain.max.x) / 2.0;
            (
                Point::new(x, terrain.min.y - margin),
                Point::new(x, terrain.max.y + margin),
            )
        } else {
            let y = (terrain.min.y + terrain.max.y) / 2.0;
            (
                Point::new(terrain.min.x - margin, y),
                Point::new(terrain.max.x + margin, y),
            )
        };
        let previous = self.project.clone();
        match self.project.subdivide_terrain(a, b) {
            Ok(_) => {
                self.record_change(previous);
                self.message =
                    "Terreno desmembrado em dois lotes com áreas conservadas.".to_owned();
            }
            Err(error) => self.message = error,
        }
    }

    fn terrain_dimensions(&self) -> Option<(f64, f64)> {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (width, height) = (
            parse(&self.terrain_width_text)?,
            parse(&self.terrain_height_text)?,
        );
        (width.is_finite()
            && height.is_finite()
            && width >= 0.25
            && height >= 0.25
            && width <= 10000.0
            && height <= 10000.0)
            .then_some((width, height))
    }

    /// Records `previous` as the undo step of the edit just made, once the
    /// constraints have been kept (moving what the edit did not touch).
    /// Returns false when the constraints refused the edit, or undid it,
    /// leaving the project as it was; the message then says why, and the
    /// caller must not announce a change.
    fn record_change(&mut self, previous: Project) -> bool {
        // Deleted elements leave their groups (a group of one goes).
        self.project.prune_groups();
        let edited = previous != self.project;
        if let Err(error) = self.project.settle_constraints(&previous) {
            self.project = previous;
            self.message = error;
            self.edit_notice = Some(self.message.clone());
            return false;
        }
        if previous == self.project {
            if edited {
                self.message = "As restrições mantiveram a geometria como estava; remova a restrição para fazer essa mudança.".to_owned();
                self.edit_notice = Some(self.message.clone());
                return false;
            }
            return true;
        }
        self.note_plan_change(&previous, false);
        // An AI batch records itself as one step when it ends.
        if self.batch_depth == 0 {
            self.history.record(previous);
        }
        self.mark_edited_outside_history();
        true
    }

    /// Marks the project as changed and due for the crash-recovery draft,
    /// for continuous form edits that are not individual undo steps (such as
    /// typing the project name or technical-sheet metadata).
    fn mark_edited_outside_history(&mut self) {
        self.dirty = true;
        self.recovery_pending = true;
    }

    fn delete_selection(&mut self) {
        self.commit_item_drag();
        if self
            .selected()
            .into_iter()
            .any(|selected| self.selection_is_locked(selected))
        {
            self.message = "A camada deste elemento está bloqueada.".to_owned();
            return;
        }
        if self.is_multi_selection() {
            let items = self.selected();
            let summary = self.selection_summary();
            let previous = self.project.clone();
            self.project.delete_elements(&items);
            self.clear_selection();
            if !self.record_change(previous) {
                return;
            }
            self.message = format!("{summary} removidos.");
            self.sync_inspector_fields();
            return;
        }
        if let Some(selection) = self.selection.take() {
            let previous = self.project.clone();
            match selection {
                Selection::Wall(id) => self.project.delete_wall(id),
                Selection::Room(id) => {
                    self.project.delete_room(id);
                }
                Selection::Terrain(id) => {
                    self.project.delete_terrain(id);
                }
                Selection::Opening(id) => {
                    self.project.delete_opening(id);
                }
                Selection::Fixture(id) => {
                    self.project.delete_fixture(id);
                }
                Selection::Drawing(id) => {
                    self.project.delete_drawing(id);
                }
            }
            if !self.record_change(previous) {
                return;
            }
            self.message = match selection {
                Selection::Wall(_) => "Parede removida.",
                Selection::Room(_) => "Cômodo removido.",
                Selection::Terrain(_) => "Terreno removido.",
                Selection::Opening(_) => "Abertura removida.",
                Selection::Fixture(_) => "Objeto removido.",
                Selection::Drawing(_) => "Desenho removido.",
            }
            .to_owned();
            self.sync_inspector_fields();
        }
    }

    pub fn sync_inspector_fields(&mut self) {
        self.sync_terrain_fields();
        self.sync_masonry_fields();
        if let Some(group) = self
            .selected_group()
            .and_then(|id| self.project.groups.iter().find(|group| group.id == id))
        {
            self.group_name_text = group.name.clone();
        }
        self.sync_constraint_fields();
        match self.selection {
            Some(Selection::Fixture(id)) => {
                if let Some(fixture) = self.project.fixtures.iter().find(|f| f.id == id) {
                    self.fixture_width_text = format!("{:.2}", fixture.width).replace('.', ",");
                    self.fixture_depth_text = format!("{:.2}", fixture.depth).replace('.', ",");
                }
            }
            Some(Selection::Wall(id)) => {
                if let Some(wall) = self.project.walls.iter().find(|w| w.id == id) {
                    self.wall_length_text = format!("{:.2}", wall.length()).replace('.', ",");
                }
            }
            Some(Selection::Room(id)) => {
                if let Some(room) = self.project.rooms.iter().find(|r| r.id == id) {
                    self.room_name_text = room.display_name().to_string();
                    if let Some((min, max)) = self.project.room_bounds(room) {
                        self.room_width_text = format!("{:.2}", max.x - min.x).replace('.', ",");
                        self.room_height_text = format!("{:.2}", max.y - min.y).replace('.', ",");
                    }
                }
            }
            Some(Selection::Opening(id)) => {
                if let Some(op) = self.project.openings.iter().find(|o| o.id == id) {
                    self.opening_width_text = format!("{:.2}", op.width()).replace('.', ",");
                    self.opening_height_text = op
                        .height()
                        .map(|height| format!("{height:.2}").replace('.', ","))
                        .unwrap_or_default();
                    self.opening_sill_text = format!("{:.2}", op.sill()).replace('.', ",");
                }
            }
            Some(Selection::Drawing(id)) => self.sync_drawing_fields(id),
            _ => {}
        }
    }

    pub fn apply_wall_length(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(new_length) = parse(&self.wall_length_text) else {
            self.message = "Informe uma medida válida em metros (ex: 3,50).".to_owned();
            return;
        };
        let Some(wall) = self.project.walls.iter().find(|w| w.id == id) else {
            self.message = "Parede não encontrada.".to_owned();
            return;
        };
        if (wall.length() - new_length).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        match self.project.resize_wall_length(id, new_length) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message = format!("Comprimento da parede alterado para {:.2} m.", new_length)
                    .replace('.', ",");
            }
            Err(err) => {
                self.message = err;
            }
        }
    }

    pub fn apply_room_dimensions(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (Some(new_w), Some(new_h)) =
            (parse(&self.room_width_text), parse(&self.room_height_text))
        else {
            self.message = "Informe largura e comprimento válidos em metros.".to_owned();
            return;
        };

        let Some(room) = self.project.rooms.iter().find(|r| r.id == id).cloned() else {
            self.message = "Cômodo não encontrado.".to_owned();
            return;
        };

        let name_changed = room.name != self.room_name_text.trim();
        // A free-shape room has no width and length to type.
        let dimensions_changed = if !self.project.room_is_rectangle(&room) {
            false
        } else if let Some((min, max)) = self.project.room_bounds(&room) {
            let current_w = max.x - min.x;
            let current_h = max.y - min.y;
            (current_w - new_w).abs() > 1e-4 || (current_h - new_h).abs() > 1e-4
        } else {
            true
        };

        if !name_changed && !dimensions_changed {
            return;
        }

        let mut tx = self.project.clone();
        if name_changed && let Err(err) = tx.rename_room(id, &self.room_name_text) {
            self.message = err;
            return;
        }
        if dimensions_changed && let Err(err) = tx.resize_room_dimensions(id, new_w, new_h) {
            self.message = err;
            return;
        }

        let previous = self.project.clone();
        self.project = tx;
        if !self.record_change(previous) {
            return;
        }
        self.sync_inspector_fields();
        if name_changed && dimensions_changed {
            self.message = format!(
                "Nome e medidas do cômodo alterados para {:.2} × {:.2} m.",
                new_w, new_h
            )
            .replace('.', ",");
        } else if name_changed {
            self.message = format!(
                "Nome do cômodo alterado para \"{}\".",
                self.room_name_text.trim()
            );
        } else {
            self.message = format!(
                "Medidas do cômodo alteradas para {:.2} × {:.2} m.",
                new_w, new_h
            )
            .replace('.', ",");
        }
    }

    pub fn apply_opening_width(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(new_w) = parse(&self.opening_width_text) else {
            self.message = "Informe uma largura válida em metros.".to_owned();
            return;
        };
        let Some(op) = self.project.openings.iter().find(|o| o.id == id) else {
            self.message = "Abertura não encontrada.".to_owned();
            return;
        };
        if (op.width() - new_w).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        match self.project.update_opening_width(id, new_w) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message =
                    format!("Largura da abertura alterada para {:.2} m.", new_w).replace('.', ",");
            }
            Err(err) => {
                self.message = err;
            }
        }
    }

    fn add_opening_at(&mut self, point: Point, kind: OpeningKind) {
        if let Some((wall_id, t)) = self.project.nearest_wall(point, 0.35) {
            let previous = self.project.clone();
            if let Some(id) = self.project.add_opening(wall_id, kind, t) {
                // Doors open towards the side of the wall that was clicked.
                self.project.swing_towards(id, point);
                if !self.record_change(previous) {
                    return;
                }
                self.message = if kind.is_door() {
                    format!(
                        "{} adicionada, abrindo para o lado clicado. I inverte o lado; Shift+I, a dobradiça.",
                        kind.label()
                    )
                } else {
                    format!("{} adicionada à parede.", kind.label())
                };
            } else {
                self.message = "Sem espaço suficiente nesta parede.".to_owned();
            }
        } else {
            self.message = "Solte o elemento sobre uma parede.".to_owned();
        }
    }

    pub(super) fn supports_numeric_input(&self) -> bool {
        matches!(
            self.tool,
            Tool::Wall
                | Tool::Room
                | Tool::RoomPolygon
                | Tool::Terrain
                | Tool::Measure
                | Tool::Draw(_)
                | Tool::Modify(
                    tools::ModifyKind::Move
                        | tools::ModifyKind::Copy
                        | tools::ModifyKind::Rotate
                        | tools::ModifyKind::Mirror
                        | tools::ModifyKind::Offset
                )
        )
    }

    pub(super) fn apply_canvas_point(&mut self, point: Point) {
        match self.tool {
            Tool::Wall | Tool::Room | Tool::Terrain | Tool::Measure => match self.creation_state {
                CreationState::Idle => {
                    self.creation_state = CreationState::WaitingSecondPoint { start: point };
                    self.message =
                        "Clique no segundo ponto ou digite a medida (ex.: 3,47 · 3,5<45 · 3x4) e Enter."
                            .to_owned();
                }
                CreationState::WaitingSecondPoint { start } | CreationState::Dragging { start } => {
                    self.complete_creation(start, point);
                    self.creation_state = CreationState::Idle;
                }
            },
            Tool::RoomPolygon => self.add_polygon_point(point),
            Tool::Draw(kind) => self.add_draw_point(kind, point),
            Tool::Modify(kind) => self.modify_click(kind, point),
            _ => {}
        }
    }

    fn commit_numeric_input(&mut self) {
        let raw = self.draft_input.trim().to_owned();
        // An angle to turn by, or the offset distance.
        if self.commit_modify_input(&raw) {
            return;
        }
        let Some(spec) = tools::numeric_input::parse_numeric_input(&raw) else {
            self.message = format!(
                "Entrada \"{raw}\" inválida. Use ex.: 3,47 · 3,5<45 · 3,5x4,0 · @2;1 · 1,5;2,0"
            );
            return;
        };
        let start = self.draft_start();
        let aim = self.draft_aim.or(self.cursor_world);
        let Some(point) = spec.resolve(start, aim) else {
            self.message =
                "Defina o primeiro ponto (ou digite coordenadas X;Y) antes da distância ou ângulo."
                    .to_owned();
            return;
        };
        self.draft_input.clear();
        self.apply_canvas_point(point);
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        // Canvas and toolbar widgets can hold focus without editing text.
        let editing_text = context.text_edit_focused();
        // While a dialog is open, editing commands must not act on the
        // project behind it; the dialog handles its own keys.
        let modal_open = self.pending_room.is_some()
            || self.pending_action.is_some()
            || self.show_help
            || self.show_mcp_dialog
            || self.active_doc_dialog != DocDialog::None
            || self.repeat_form.is_some();
        let blocking_dialog = self.pending_room.is_some()
            || self.pending_action.is_some()
            || self.repeat_form.is_some();

        // Capture direct CAD numeric input typed while a drawing tool is active.
        if !editing_text && !modal_open && self.supports_numeric_input() {
            let typed: Vec<char> = context.input(|input| {
                if input.modifiers.ctrl || input.modifiers.command || input.modifiers.alt {
                    return Vec::new();
                }
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::Text(text) => Some(text.chars().collect::<Vec<_>>()),
                        _ => None,
                    })
                    .flatten()
                    .collect()
            });
            for ch in typed {
                if self.draft_input.chars().count() < 32
                    && tools::numeric_input::accepts_char(ch, !self.draft_input.is_empty())
                {
                    self.draft_input.push(ch);
                }
            }
        }
        let typing_numeric = !self.draft_input.is_empty();

        let actions = context.input(|input| {
            let pressed = |key: Key| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, .. } if *event_key == key)
                })
            };
            let plain = |key: Key| {
                !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                            if *event_key == key && modifiers.is_none())
                    })
            };
            let key_unmodified = |key: Key, allow_repeat: bool| {
                !editing_text
                    && !modal_open
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat, modifiers, .. }
                            if *event_key == key && modifiers.is_none() && (allow_repeat || !repeat))
                    })
            };
            let shortcut = |key: Key, shift: bool| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                        if *event_key == key && (modifiers.command || modifiers.ctrl) && modifiers.shift == shift)
                })
            };
            // Arrows nudge the selection: 0,25 m, or 1 m with Shift. Held
            // keys repeat, as in other editors.
            let nudge = [
                (Key::ArrowLeft, -1.0, 0.0),
                (Key::ArrowRight, 1.0, 0.0),
                (Key::ArrowUp, 0.0, -1.0),
                (Key::ArrowDown, 0.0, 1.0),
            ]
            .into_iter()
            .find_map(|(key, dx, dy)| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key: event_key,
                        pressed: true,
                        modifiers,
                        ..
                    } if *event_key == key
                        && !editing_text
                        && !modal_open
                        && !modifiers.ctrl
                        && !modifiers.command
                        && !modifiers.alt =>
                    {
                        let step = if modifiers.shift { 1.0 } else { NUDGE_STEP };
                        Some(Point::new(dx * step, dy * step))
                    }
                    _ => None,
                })
            });
            let tool = [
                (Key::V, Tool::Move),
                (Key::T, Tool::Terrain),
                (Key::P, Tool::Wall),
                (Key::C, Tool::Room),
                (Key::L, Tool::RoomPolygon),
                (Key::R, Tool::Opening(OpeningKind::Door)),
                (Key::J, Tool::Opening(OpeningKind::Window)),
                (Key::M, Tool::Measure),
            ]
            .into_iter()
            .chain(tools::ModifyKind::ALL.into_iter().filter_map(|kind| {
                kind.key().map(|(key, _)| (key, Tool::Modify(kind)))
            }))
            .find_map(|(key, tool)| plain(key).then_some(tool))
            .or_else(|| {
                (!editing_text && !modal_open && !typing_numeric)
                    .then(|| Self::drafting_shortcut(input))
                    .flatten()
            });
            let free = !modal_open;
            // Ctrl+C, Ctrl+X and Ctrl+V reach the app as clipboard events.
            let clipboard_event = |accept: fn(&egui::Event) -> bool| {
                !editing_text && free && input.events.iter().any(accept)
            };
            let shifted = |key: Key| {
                !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                            if *event_key == key && modifiers.shift && !modifiers.ctrl && !modifiers.command && !modifiers.alt)
                    })
            };
            ShortcutActions {
                copy: clipboard_event(|event| matches!(event, egui::Event::Copy))
                    || (shortcut(Key::C, false) && !editing_text && free),
                cut: clipboard_event(|event| matches!(event, egui::Event::Cut))
                    || (shortcut(Key::X, false) && !editing_text && free),
                paste: clipboard_event(|event| {
                    matches!(event, egui::Event::Paste(_) | egui::Event::PasteImage(_))
                }) || (shortcut(Key::V, false) && !editing_text && free),
                duplicate: shortcut(Key::D, false) && !editing_text && free,
                group: shortcut(Key::G, false) && !editing_text && free,
                ungroup: shortcut(Key::G, true) && !editing_text && free,
                join: plain(Key::U),
                rotate_back: shifted(Key::G),
                mirror_left_right: plain(Key::H),
                mirror_up_down: shifted(Key::H),
                save: shortcut(Key::S, false) && free,
                open: shortcut(Key::O, false) && free,
                new: (shortcut(Key::N, false) || shortcut(Key::T, false)) && free,
                new_window: shortcut(Key::N, true) && free,
                close_tab: shortcut(Key::W, false) && free,
                tab_step: if free && shortcut(Key::Tab, false) {
                    1
                } else if free && shortcut(Key::Tab, true) {
                    -1
                } else {
                    0
                },
                undo: shortcut(Key::Z, false) && !editing_text && free,
                redo: (shortcut(Key::Y, false) || shortcut(Key::Z, true)) && !editing_text && free,
                select_all: shortcut(Key::A, false) && !editing_text && free,
                escape: pressed(Key::Escape)
                    && self.pending_action.is_none()
                    && self.repeat_form.is_none(),
                delete: pressed(Key::Delete) && !editing_text && !typing_numeric && free,
                tool,
                nudge,
                rotate: plain(Key::G),
                flip_side: plain(Key::I),
                flip_hinge: !editing_text
                    && !modal_open
                    && !typing_numeric
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: Key::I, pressed: true, repeat: false, modifiers, .. }
                            if modifiers.shift && !modifiers.ctrl && !modifiers.command && !modifiers.alt)
                    }),
                fit: plain(Key::F),
                finish_polygon: plain(Key::Enter),
                undo_point: plain(Key::Backspace),
                numeric_commit: typing_numeric && key_unmodified(Key::Enter, false),
                numeric_backspace: typing_numeric && key_unmodified(Key::Backspace, true),
                cycle_wall_reference: matches!(
                    self.tool,
                    Tool::Wall | Tool::Room | Tool::RoomPolygon
                ) && key_unmodified(Key::Tab, false),
                snap: pressed(Key::F3) && free,
                grid: pressed(Key::F7) && free,
                ortho: pressed(Key::F8) && free,
                help: pressed(Key::F1) && !blocking_dialog,
            }
        });
        if actions.save {
            self.save();
        }
        if actions.open {
            self.request_open();
        }
        if actions.new_window {
            self.open_new_window();
        }
        if actions.new {
            self.request_new_project();
        }
        if actions.close_tab {
            self.request_close_tab(self.active_tab);
        }
        if actions.tab_step != 0 {
            self.next_tab(actions.tab_step);
        }
        if actions.undo {
            self.undo();
        }
        if actions.redo {
            self.redo();
        }
        if actions.select_all {
            self.select_all();
        }
        if actions.copy {
            self.copy_selection();
        }
        if actions.cut {
            self.cut_selection();
        }
        if actions.paste {
            self.paste();
        }
        if actions.group {
            self.group_selection();
        }
        if actions.ungroup {
            self.ungroup_selection();
        }
        if actions.join {
            self.join_selection();
        }
        if actions.duplicate {
            self.duplicate_selection();
        }
        // Ctrl+V only reaches the app while the system clipboard holds
        // something, so a copy leaves a note there.
        if let Some(note) = self.system_clipboard_note.take() {
            context.copy_text(note);
        }
        if actions.escape {
            if self.show_help || self.show_mcp_dialog || self.active_doc_dialog != DocDialog::None {
                self.show_help = false;
                self.show_mcp_dialog = false;
                self.active_doc_dialog = DocDialog::None;
                self.mcp_feedback = None;
            } else if self.box_select.take().is_some() {
                self.message = "Seleção por janela cancelada.".to_owned();
            } else if !self.draft_input.is_empty() {
                self.draft_input.clear();
                self.message = "Entrada numérica limpa.".to_owned();
            } else if self.tool == Tool::Measure
                && self.draft_start().is_none()
                && !self.measurements.is_empty()
            {
                self.measurements.clear();
                self.message = "Medidas apagadas.".to_owned();
            } else if self.draft_start().is_none()
                && self.pending_room.is_none()
                && self.selection.is_some()
            {
                self.clear_selection();
                self.message = "Seleção limpa.".to_owned();
            } else {
                self.cancel_creation();
                self.pending_room = None;
                self.message = "Ação cancelada.".to_owned();
            }
        }
        if actions.numeric_backspace {
            self.draft_input.pop();
        }
        if actions.numeric_commit {
            self.commit_numeric_input();
        }
        if actions.cycle_wall_reference {
            self.wall_reference = self.wall_reference.next();
            self.message = format!(
                "Referência da parede: {} (Tab alterna).",
                self.wall_reference.label()
            );
        }
        if actions.delete {
            self.delete_selection();
        }
        if let Some(delta) = actions.nudge {
            self.nudge_selection(delta);
        }
        if actions.rotate {
            self.rotate_fixture_or_next();
        }
        if actions.rotate_back {
            self.rotate_selection_quarter(false);
        }
        if actions.mirror_left_right || actions.mirror_up_down {
            self.mirror_selection(actions.mirror_left_right);
        }
        if actions.flip_side || actions.flip_hinge {
            self.flip_selected_door(actions.flip_side, actions.flip_hinge);
        }
        if let Some(tool) = actions.tool {
            self.set_tool(tool);
            if tool == Tool::Terrain {
                self.sync_terrain_fields();
            }
        }
        if actions.fit {
            self.fit_view_requested();
        }
        if let Tool::Draw(kind) = self.tool {
            if actions.finish_polygon {
                self.finish_drawing(kind, false);
            }
            if actions.undo_point {
                self.undo_draw_point();
            }
        }
        if self.tool == Tool::RoomPolygon {
            if actions.finish_polygon {
                self.close_polygon_draft();
            }
            if actions.undo_point {
                self.undo_polygon_point();
            }
        }
        if actions.snap {
            self.toggle_snap();
        }
        if actions.grid {
            self.toggle_grid();
        }
        if actions.ortho {
            self.toggle_ortho();
        }
        if actions.help {
            self.show_help = !self.show_help;
        }
    }
}

struct ShortcutActions {
    save: bool,
    open: bool,
    new: bool,
    new_window: bool,
    close_tab: bool,
    tab_step: isize,
    undo: bool,
    redo: bool,
    select_all: bool,
    copy: bool,
    cut: bool,
    paste: bool,
    duplicate: bool,
    group: bool,
    ungroup: bool,
    join: bool,
    rotate_back: bool,
    mirror_left_right: bool,
    mirror_up_down: bool,
    escape: bool,
    delete: bool,
    tool: Option<Tool>,
    nudge: Option<Point>,
    rotate: bool,
    flip_side: bool,
    flip_hinge: bool,
    fit: bool,
    finish_polygon: bool,
    undo_point: bool,
    numeric_commit: bool,
    numeric_backspace: bool,
    cycle_wall_reference: bool,
    snap: bool,
    grid: bool,
    ortho: bool,
    help: bool,
}

impl FloorPlanApp {
    /// Keeps the window title in step with the document and its state.
    fn update_window_title(&mut self, context: &egui::Context) {
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
}

impl eframe::App for FloorPlanApp {
    /// Runs before every frame and also while the window is minimized, so
    /// an assistant's tool calls are answered even then.
    fn logic(&mut self, _context: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_mcp_calls();
        self.poll_update_check();
        self.publish_plan_changes();
        self.save_recovery_if_pending();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Closes the public tunnel and revokes its secret token with the app.
        self.tunnel = None;
        self.tunnel_token.clear();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_layout(ui);
        // A refused edit's reason wins over a message written after it.
        if let Some(notice) = self.edit_notice.take() {
            self.message = notice;
        }
        self.persist_settings();
        self.save_recovery_if_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(
        app: &mut FloorPlanApp,
        context: &egui::Context,
        key: Key,
        modifiers: egui::Modifiers,
    ) {
        // Press and release, so a later press of the same key is not a repeat.
        let event = |pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        context.begin_pass(egui::RawInput {
            events: vec![event(true), event(false)],
            ..Default::default()
        });
        app.handle_shortcuts(context);
        context.end_pass().textures_delta.clear();
    }

    #[test]
    fn shortcuts_do_not_edit_the_project_behind_a_dialog() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let previous = app.project.clone();
        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        app.record_change(previous);
        app.selection = Some(Selection::Wall(id));
        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };

        // "Save changes?" is open: Delete, Esc and Ctrl+Z leave everything alone.
        app.pending_action = Some(PendingAction::CloseTab);
        press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
        press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
        press(&mut app, &context, Key::Z, ctrl);
        press(&mut app, &context, Key::P, egui::Modifiers::NONE);
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.selection, Some(Selection::Wall(id)));
        assert_eq!(app.tool, Tool::Move);
        assert_eq!(app.pending_action, Some(PendingAction::CloseTab));

        // The help is open: editing keys wait, Esc closes it.
        app.pending_action = None;
        app.show_help = true;
        press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
        press(&mut app, &context, Key::Z, ctrl);
        assert_eq!(app.project.walls.len(), 1);
        press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
        assert!(!app.show_help);
        assert_eq!(app.selection, Some(Selection::Wall(id)));

        // With no dialog, the same keys work again.
        press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
        assert!(app.project.walls.is_empty());
        press(&mut app, &context, Key::Z, ctrl);
        assert_eq!(app.project.walls.len(), 1);
    }

    #[test]
    fn renaming_the_project_is_protected_by_the_recovery_draft() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        assert!(!app.dirty && !app.recovery_pending);
        app.project.name = "Casa de praia".to_owned();
        app.mark_edited_outside_history();
        assert!(app.dirty);
        assert!(app.recovery_pending);
        assert!(!app.history.can_undo());
    }

    /// Starts dragging wall `id` and moves it 2 m to the right.
    fn drag_wall(app: &mut FloorPlanApp, id: u64) {
        app.selection = Some(Selection::Wall(id));
        app.drag_initial_project = Some(app.project.clone());
        app.item_drag = Some(ItemDrag {
            selection: Selection::Wall(id),
            last: Point::new(2.0, 0.0),
            grab_offset: Point::new(0.0, 0.0),
            moved: true,
            group: false,
        });
        app.project.move_wall(id, Point::new(2.0, 0.0));
    }

    #[test]
    fn switching_tools_mid_drag_keeps_the_move_undoable_and_unsaved() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        drag_wall(&mut app, id);
        press(&mut app, &context, Key::P, egui::Modifiers::NONE);
        assert_eq!(app.tool, Tool::Wall);
        assert!(app.dirty && app.recovery_pending);
        assert!(app.drag_initial_project.is_none() && app.item_drag.is_none());
        app.undo();
        assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
    }

    #[test]
    fn deleting_mid_drag_undoes_the_delete_before_the_move() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        drag_wall(&mut app, id);
        app.delete_selection();
        assert!(app.project.walls.is_empty());
        app.undo();
        assert_eq!(app.project.walls[0].start, Point::new(2.0, 0.0));
        app.undo();
        assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
    }

    #[test]
    fn arrow_keys_nudge_the_selection_as_undoable_steps() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        app.selection = Some(Selection::Wall(id));

        press(&mut app, &context, Key::ArrowRight, egui::Modifiers::NONE);
        assert_eq!(app.project.walls[0].start, Point::new(0.25, 0.0));
        press(&mut app, &context, Key::ArrowDown, egui::Modifiers::SHIFT);
        assert_eq!(app.project.walls[0].start, Point::new(0.25, 1.0));
        assert!(app.dirty);

        app.undo();
        assert_eq!(app.project.walls[0].start, Point::new(0.25, 0.0));
        app.undo();
        assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));

        // Nothing selected, or Ctrl held: the arrows do nothing.
        press(&mut app, &context, Key::ArrowRight, egui::Modifiers::CTRL);
        app.selection = None;
        press(&mut app, &context, Key::ArrowRight, egui::Modifiers::NONE);
        assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
    }

    #[test]
    fn arrows_slide_openings_along_their_wall_only() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
            .unwrap();
        let door = app
            .project
            .add_opening(wall, OpeningKind::Door, 0.5)
            .unwrap();
        app.selection = Some(Selection::Opening(door));

        app.nudge_selection(Point::new(1.0, 0.0));
        assert!((app.opening_center(door).unwrap().x - 4.0).abs() < 1e-3);
        let before = app.project.clone();
        app.nudge_selection(Point::new(0.0, 1.0));
        assert_eq!(app.project, before);
        assert!(app.message.contains("ao longo da parede"));
    }

    #[test]
    fn nudging_stops_at_the_terrain_edge() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .set_terrain(Point::new(0.0, 0.0), Point::new(5.0, 5.0));
        let id = app
            .project
            .add_wall(Point::new(0.0, 1.0), Point::new(3.0, 1.0))
            .unwrap();
        app.selection = Some(Selection::Wall(id));
        app.history.clear();
        app.dirty = false;
        app.nudge_selection(Point::new(-0.25, 0.0));
        assert_eq!(app.project.walls[0].start, Point::new(0.0, 1.0));
        assert!(!app.dirty && !app.history.can_undo());
        assert!(app.message.contains("terreno"));
    }

    #[test]
    fn interface_preferences_are_tracked_for_saving() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        assert_eq!(app.current_settings(), Settings::default());
        app.toggle_grid();
        app.toggle_ortho();
        app.ribbon_collapsed = true;
        assert_ne!(app.current_settings(), app.saved_settings);
        app.persist_settings();
        assert_eq!(app.saved_settings, app.current_settings());
        assert!(!app.saved_settings.show_grid && app.saved_settings.ortho_enabled);
    }

    #[test]
    fn doors_open_towards_the_click_and_flip_with_i() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
        // The leaf swings along (-dy, dx) = (0, 1) by default: below the wall.
        app.add_opening_at(Point::new(2.0, 0.3), OpeningKind::Door);
        assert!(!app.project.openings[0].flip_side);
        app.add_opening_at(Point::new(3.2, -0.3), OpeningKind::Window);
        app.project.openings.clear();
        app.add_opening_at(Point::new(2.0, -0.3), OpeningKind::Door);
        let door = app.project.openings[0].id;
        assert!(app.project.openings[0].flip_side);

        app.selection = Some(Selection::Opening(door));
        press(&mut app, &context, Key::I, egui::Modifiers::NONE);
        assert!(!app.project.openings[0].flip_side);
        press(&mut app, &context, Key::I, egui::Modifiers::SHIFT);
        assert!(app.project.openings[0].flip_hinge);
        app.undo();
        assert!(!app.project.openings[0].flip_hinge);
    }

    #[test]
    fn letter_keys_pick_tools_and_escape_clears_the_selection() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        press(&mut app, &context, Key::P, egui::Modifiers::NONE);
        assert_eq!(app.tool, Tool::Wall);
        press(&mut app, &context, Key::J, egui::Modifiers::NONE);
        assert_eq!(app.tool, Tool::Opening(OpeningKind::Window));
        press(&mut app, &context, Key::V, egui::Modifiers::NONE);
        assert_eq!(app.tool, Tool::Move);

        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        app.selection = Some(Selection::Wall(id));
        press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
        assert_eq!(app.selection, None);
    }

    #[test]
    fn ctrl_z_undoes_and_ctrl_y_restores_the_project() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let previous = app.project.clone();
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
        app.record_change(previous);
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        for (key, expected_walls) in [(Key::Z, 0), (Key::Y, 1)] {
            context.begin_pass(egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            });
            app.handle_shortcuts(&context);
            assert_eq!(app.project.walls.len(), expected_walls);
            context.end_pass().textures_delta.clear();
        }
    }

    #[test]
    fn fit_view_shows_a_fifty_meter_plot() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .set_terrain(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0));
        app.fit_view(rect);
        let view = canvas::ViewTransform {
            rect,
            scale: app.scale,
            pan: app.pan,
        };
        let a = view.to_screen(Point::new(0.0, 0.0));
        let b = view.to_screen(Point::new(50.0, 50.0));
        assert!(rect.contains(a) && rect.contains(b));
        assert!(app.scale > 3.0);
    }

    #[test]
    fn terrain_dimension_input_accepts_portuguese_decimals() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.terrain_width_text = "5,5".to_owned();
        app.terrain_height_text = "10".to_owned();
        assert_eq!(app.terrain_dimensions(), Some((5.5, 10.0)));
        app.terrain_width_text = "0".to_owned();
        assert_eq!(app.terrain_dimensions(), None);
    }

    #[test]
    fn terrain_created_after_room_contains_the_room() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .add_room(Point::new(-20.0, 2.0), Point::new(-10.0, 10.0), "Garagem");
        app.terrain_width_text = "10".to_owned();
        app.terrain_height_text = "50".to_owned();
        app.create_terrain_from_dimensions();
        assert!(app.project.content_inside_terrain());
    }

    #[test]
    fn opening_inspector_does_not_shift_the_plan_on_screen() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(740.0, 700.0));
        app.prepare_canvas_view(wide);
        let before = canvas::ViewTransform {
            rect: wide,
            scale: app.scale,
            pan: app.pan,
        }
        .to_screen(Point::new(5.0, 10.0));
        app.prepare_canvas_view(narrow);
        let after = canvas::ViewTransform {
            rect: narrow,
            scale: app.scale,
            pan: app.pan,
        }
        .to_screen(Point::new(5.0, 10.0));
        assert_eq!(before, after);
    }

    #[test]
    fn initial_tool_is_selection_move() {
        let context = egui::Context::default();
        let app = FloorPlanApp::new(&context);
        assert_eq!(app.tool, Tool::Move);
    }

    #[test]
    fn a_new_plan_opens_in_a_tab_and_keeps_the_unsaved_one() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let prev = app.project.clone();
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        app.record_change(prev);
        assert!(app.dirty);

        // Nothing is asked and nothing is lost: the new plan gets a tab.
        app.request_new_project();
        assert_eq!(app.pending_action, None);
        assert_eq!(app.tab_count(), 2);
        assert!(app.project.walls.is_empty() && !app.dirty);

        app.switch_tab(0);
        assert_eq!(app.project.walls.len(), 1);
        assert!(app.dirty);
    }

    #[test]
    fn request_close_on_clean_project_closes_and_on_dirty_asks_confirmation() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        assert!(!app.dirty);

        app.request_close(&context);
        assert_eq!(app.pending_action, None);

        let prev = app.project.clone();
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        app.record_change(prev);
        assert!(app.dirty);

        app.request_close(&context);
        assert_eq!(app.pending_action, Some(PendingAction::CloseApp));

        app.cancel_pending_action();
        assert_eq!(app.pending_action, None);
        assert!(app.dirty);
    }

    #[test]
    fn failed_or_cancelled_open_preserves_dirty_state_and_project() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let prev = app.project.clone();
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        app.record_change(prev);
        assert!(app.dirty);

        let invalid_path =
            std::env::temp_dir().join(format!("trazo-invalid-{}.json", std::process::id()));
        std::fs::write(&invalid_path, "not a valid json").unwrap();

        let result = app.open_file(&invalid_path);
        assert_eq!(result, OpenResult::Error);
        assert!(app.dirty);
        assert_eq!(app.project.walls.len(), 1);
        let _ = std::fs::remove_file(invalid_path);
    }

    #[test]
    fn room_rename_preserves_walls_openings_and_works_with_shared_walls() {
        let mut project = Project::default();
        let r1 = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let _r2 = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
            .unwrap();
        let r1_wall = project.rooms[0].wall_ids[0];
        let op_id = project
            .add_opening(r1_wall, OpeningKind::Door, 0.5)
            .unwrap();

        assert!(project.rename_room(r1, "Estar").unwrap());
        assert_eq!(project.rooms[0].display_name(), "Estar");
        assert_eq!(project.walls.len(), 7);
        assert_eq!(project.openings.len(), 1);
        assert_eq!(project.openings[0].id, op_id);
    }

    #[test]
    fn room_resize_preserves_openings_and_wall_ids() {
        let mut project = Project::default();
        let r_id = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Suíte")
            .unwrap();
        let bottom_wall_id = project.rooms[0].wall_ids[0];
        let door_id = project
            .add_opening(bottom_wall_id, OpeningKind::Door, 0.5)
            .unwrap();
        let orig_wall_ids = project.rooms[0].wall_ids.clone();

        assert!(project.resize_room_dimensions(r_id, 6.0, 5.0).is_ok());

        assert_eq!(project.rooms[0].wall_ids, orig_wall_ids);
        assert_eq!(project.openings.len(), 1);
        assert_eq!(project.openings[0].id, door_id);
        assert_eq!(project.openings[0].wall_id, bottom_wall_id);

        let res = project.resize_room_dimensions(r_id, 0.80, 5.0);
        assert!(res.is_err());
        let (min, max) = project.room_bounds(&project.rooms[0]).unwrap();
        assert!((max.x - min.x - 6.0).abs() < 1e-3);
    }

    #[test]
    fn creation_flow_creates_elements_correctly() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);

        app.set_tool(Tool::Wall);
        app.complete_creation(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
        assert_eq!(app.project.walls.len(), 1);

        app.set_tool(Tool::Room);
        app.complete_creation(Point::new(0.0, 0.0), Point::new(3.0, 3.0));
        assert!(app.pending_room.is_some());
    }

    #[test]
    fn wall_numerical_resize_updates_length() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let w_id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        app.selection = Some(Selection::Wall(w_id));
        app.sync_inspector_fields();
        assert_eq!(app.wall_length_text, "4,00");

        app.wall_length_text = "6,50".to_owned();
        app.apply_wall_length(w_id);
        assert!((app.project.walls[0].length() - 6.5).abs() < 1e-3);
        assert!(app.dirty);

        // Undo reverts the length change
        app.undo();
        assert!((app.project.walls[0].length() - 4.0).abs() < 1e-3);
    }

    #[test]
    fn room_numerical_resize_updates_dimensions() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let r_id = app
            .project
            .add_room(Point::new(0.0, 0.0), Point::new(3.0, 4.0), "Sala")
            .unwrap();
        app.selection = Some(Selection::Room(r_id));
        app.sync_inspector_fields();
        assert_eq!(app.room_width_text, "3,00");
        assert_eq!(app.room_height_text, "4,00");

        app.room_name_text = "Living".to_owned();
        app.room_width_text = "5,00".to_owned();
        app.room_height_text = "6,00".to_owned();
        app.apply_room_dimensions(r_id);

        let room = &app.project.rooms[0];
        assert_eq!(room.display_name(), "Living");
        assert!((app.project.room_area(room).unwrap() - 30.0).abs() < 1e-2);

        // Undo restores previous dimensions
        app.undo();
        let room = &app.project.rooms[0];
        assert_eq!(room.display_name(), "Sala");
        assert!((app.project.room_area(room).unwrap() - 12.0).abs() < 1e-2);
    }

    #[test]
    fn opening_selection_width_update_and_deletion() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let w_id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0))
            .unwrap();
        let op_id = app
            .project
            .add_opening(w_id, OpeningKind::Door, 0.5)
            .unwrap();

        app.selection = Some(Selection::Opening(op_id));
        app.sync_inspector_fields();
        assert_eq!(app.opening_width_text, "0,90");

        // Custom width
        app.opening_width_text = "1,20".to_owned();
        app.apply_opening_width(op_id);
        assert_eq!(app.project.openings[0].width(), 1.20);

        // Delete opening
        app.delete_selection();
        assert_eq!(app.project.openings.len(), 0);
        assert_eq!(app.selection, None);

        // Undo restores opening
        app.undo();
        assert_eq!(app.project.openings.len(), 1);
        assert_eq!(app.project.openings[0].width(), 1.20);
    }

    #[test]
    fn undo_revalidates_selection_when_item_no_longer_exists() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let prev = app.project.clone();
        let w_id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        app.record_change(prev);

        app.selection = Some(Selection::Wall(w_id));
        assert!(app.selection.is_some());

        // Undo wall creation: wall disappears, selection must be cleared
        app.undo();
        assert_eq!(app.project.walls.len(), 0);
        assert_eq!(app.selection, None);

        // Redo restores wall
        app.redo();
        assert_eq!(app.project.walls.len(), 1);
    }

    #[test]
    fn no_op_and_validation_errors_do_not_pollute_history_or_dirty() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let w_id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        app.dirty = false;
        app.history.clear();

        // 1. Same wall length: no-op, dirty stays false, history remains empty
        app.wall_length_text = "4,00".to_owned();
        app.apply_wall_length(w_id);
        assert!(!app.dirty);
        assert!(!app.history.can_undo());

        // 2. Invalid wall length: error, dirty stays false, history remains empty
        app.wall_length_text = "0,10".to_owned(); // below 0.25m min
        app.apply_wall_length(w_id);
        assert!(!app.dirty);
        assert!(!app.history.can_undo());
        assert!(app.message.contains("O comprimento deve estar entre"));

        // 3. Opening width tests
        let op_id = app
            .project
            .add_opening(w_id, OpeningKind::Door, 0.5)
            .unwrap();
        app.dirty = false;
        app.history.clear();

        // Same opening width
        app.opening_width_text = "0,90".to_owned();
        app.apply_opening_width(op_id);
        assert!(!app.dirty);
        assert!(!app.history.can_undo());

        // Invalid opening width (below min)
        app.opening_width_text = "0,20".to_owned();
        app.apply_opening_width(op_id);
        assert!(!app.dirty);
        assert!(!app.history.can_undo());
        assert!(app.message.contains("Valor fora dos limites permitidos"));

        // 4. Room dimensions no-op
        let r_id = app
            .project
            .add_room(Point::new(10.0, 10.0), Point::new(14.0, 14.0), "Quarto")
            .unwrap();
        app.dirty = false;
        app.history.clear();

        app.room_name_text = "Quarto".to_owned();
        app.room_width_text = "4,00".to_owned();
        app.room_height_text = "4,00".to_owned();
        app.apply_room_dimensions(r_id);
        assert!(!app.dirty);
        assert!(!app.history.can_undo());
    }

    #[test]
    fn creation_state_flow_two_clicks_drag_esc_and_tool_switch() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);

        // Initial state
        assert_eq!(app.creation_state(), CreationState::Idle);
        assert!(app.draft_start().is_none());

        // 1. Tool switch resets creation state
        app.set_tool(Tool::Wall);
        app.creation_state = CreationState::WaitingSecondPoint {
            start: Point::new(1.0, 1.0),
        };
        assert_eq!(app.draft_start(), Some(Point::new(1.0, 1.0)));

        app.set_tool(Tool::Room);
        assert_eq!(app.creation_state(), CreationState::Idle);
        assert!(app.draft_start().is_none());

        // 2. Escape cancels creation
        app.set_tool(Tool::Wall);
        app.creation_state = CreationState::WaitingSecondPoint {
            start: Point::new(2.0, 2.0),
        };
        app.cancel_creation();
        assert_eq!(app.creation_state(), CreationState::Idle);
        assert!(app.draft_start().is_none());

        // 3. Two-click completion creates wall
        app.set_tool(Tool::Wall);
        let start = Point::new(0.0, 0.0);
        let end = Point::new(5.0, 0.0);
        app.creation_state = CreationState::WaitingSecondPoint { start };
        app.complete_creation(start, end);
        app.creation_state = CreationState::Idle;
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.creation_state(), CreationState::Idle);

        // 4. Drag completion creates wall
        app.creation_state = CreationState::Dragging {
            start: Point::new(0.0, 0.0),
        };
        let drag_end = Point::new(0.0, 4.0);
        app.complete_creation(Point::new(0.0, 0.0), drag_end);
        app.creation_state = CreationState::Idle;
        assert_eq!(app.project.walls.len(), 2);
        assert_eq!(app.creation_state(), CreationState::Idle);
    }

    #[test]
    fn typing_distance_and_polar_during_drawing_creates_exact_walls() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.set_tool(Tool::Wall);

        // First point via absolute coordinates `1;2` (Y up, as the status
        // bar shows it: 2 m above the origin is -2 in plan coordinates).
        app.draft_input = "1;2".to_owned();
        app.commit_numeric_input();
        assert_eq!(app.draft_start(), Some(Point::new(1.0, -2.0)));
        assert!(app.draft_input.is_empty());

        // Second point via distance `3,47` aiming right.
        app.draft_aim = Some(Point::new(5.0, -2.0));
        app.draft_input = "3,47".to_owned();
        app.commit_numeric_input();
        assert_eq!(app.project.walls.len(), 1);
        assert!((app.project.walls[0].length() - 3.47).abs() < 1e-4);

        // Escape clears typed input before cancelling the draft.
        app.apply_canvas_point(Point::new(0.0, 0.0));
        app.draft_input = "4,5<45".to_owned();
        press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
        assert!(app.draft_input.is_empty());
        assert_eq!(app.draft_start(), Some(Point::new(0.0, 0.0)));
    }

    #[test]
    fn wall_reference_shifts_walls_and_rooms_and_tab_cycles_it() {
        use crate::domain::wall_reference::WallReference;

        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let half = app.project.masonry.thickness() * 0.5;
        app.set_tool(Tool::Wall);
        assert_eq!(app.wall_reference, WallReference::Center);

        // Tab cycles Center -> External -> Internal -> Center.
        press(&mut app, &context, Key::Tab, egui::Modifiers::NONE);
        assert_eq!(app.wall_reference, WallReference::External);

        app.complete_creation(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
        assert_eq!(app.project.walls.len(), 1);
        assert!((app.project.walls[0].start.y - half).abs() < 1e-4);
        assert!((app.project.walls[0].end.y - half).abs() < 1e-4);

        press(&mut app, &context, Key::Tab, egui::Modifiers::NONE);
        assert_eq!(app.wall_reference, WallReference::Internal);

        // Room referenced by Internal expands centreline by half thickness on each side.
        app.set_tool(Tool::Room);
        app.complete_creation(Point::new(1.0, 1.0), Point::new(4.0, 5.0));
        let corners = app.pending_room.as_ref().expect("pending room corners");
        assert!((corners[0].x - (1.0 - half)).abs() < 1e-4);
        assert!((corners[2].x - (4.0 + half)).abs() < 1e-4);
        assert!((corners[0].y - (1.0 - half)).abs() < 1e-4);
        assert!((corners[2].y - (5.0 + half)).abs() < 1e-4);
    }

    #[test]
    fn startup_with_file_argument_loads_project() {
        let mut project = Project::default();
        project.name = "Projeto por duplo clique".to_owned();
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("trazo-startup-open-{}.trazo", std::process::id()));
        project_file::save(&project, &path).unwrap();

        let context = egui::Context::default();
        let app = FloorPlanApp::new_with_file(&context, Some(path.clone()));
        assert_eq!(app.project.name, "Projeto por duplo clique");
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.file_path.as_deref(), Some(path.as_path()));
        assert!(!app.dirty);

        let _ = std::fs::remove_file(&path);
    }
}
