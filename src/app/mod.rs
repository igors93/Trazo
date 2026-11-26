mod ai_activity;
pub mod app_icon;
mod canvas;
mod creation;
mod documents;
mod edit_commands;
mod exports;
mod interaction;
mod lifecycle;
mod markups;
mod mcp_tools;
pub(crate) mod pacing;
mod plan_sync;
mod properties;
mod selection;
mod selection_rules;
mod shortcuts;
mod tabs;
mod terrain;
#[cfg(test)]
mod tests;
mod theme;
mod tools;
mod ui;
#[cfg(test)]
mod ui_tests;
mod undo;
mod viewport;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui;

use crate::{
    domain::{
        fixtures::FixtureKind,
        geometry::Point,
        history::History,
        model::{OpeningKind, Project},
    },
    io::settings::Settings,
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
    Markups,
    Sheet,
    Print,
    Schedules,
    Templates,
    Views,
    SheetSet,
    Circulation,
    Floors,
    Library,
    Ergonomics,
    Electrical,
    Structure,
    Safety,
    Quantities,
    Underlay,
    Xref,
    Compare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    /// Closing the active tab, which has unsaved changes.
    CloseTab,
    CloseApp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RibbonTab {
    Architecture,
    Edit,
    Interiors,
    Electrical,
    Structure,
    Drafting,
    Markups,
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
    /// Review shapes; None selects and moves existing marks.
    Markup(Option<crate::domain::markups::MarkupKind>),
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
    markup_state: markups::MarkupUiState,
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
    print_state: ui::PrintDialogState,
    underlay_state: ui::UnderlayUiState,
    xref_manager: crate::io::xref::XrefManager,
    compare_state: ui::CompareUiState,
    background: crate::domain::background::BackgroundAnalyzer,
    background_wake_ready: bool,
    new_layer_name: String,
    new_note_text: String,
    new_revision_text: String,
    release_reviewer: String,
    release_registration: String,
    release_date: String,
    library_search: String,
    library_category_filter: Option<crate::domain::library::LibraryCategory>,
    new_library_name: String,
    new_library_width: f64,
    new_library_depth: f64,
    new_library_base: Option<FixtureKind>,
    personal_library: crate::io::personal_library::PersonalLibrary,
    library_tab: u8,
    save_item_to_personal: bool,
    new_block_name: String,
    new_block_category: String,
    new_block_attr_key: String,
    new_block_attr_default: String,
    new_block_attrs: Vec<crate::domain::blocks::BlockAttribute>,
    save_block_to_personal: bool,
    block_insert_overrides: Vec<(String, String)>,
    block_insert_target: Option<crate::domain::blocks::BlockDefinition>,
    show_ergonomics: bool,
    ergonomics_min_passage: f64,
    /// The discipline shown on the plan: everything, or one of them with
    /// the architecture faded behind it (#96).
    discipline: crate::domain::discipline::Discipline,
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
    /// Library model selected for pointer placement, including its real size.
    fixture_preset: Option<crate::domain::library::LibraryItem>,
    fixture_width_text: String,
    fixture_depth_text: String,
    fixture_load_text: String,
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
    /// Limits continuous overlay repaints without delaying input-driven frames.
    frame_pacer: pacing::FramePacer,
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
    /// Every open view tab; the active one is `None`, its visual state lives
    /// in the fields below and its shared project lives in `self.project` (see `tabs.rs`).
    tabs: Vec<Option<tabs::ViewTab>>,
    active_tab: usize,
    active_tab_id: tabs::TabId,
    active_tab_title: Option<String>,
    active_tab_floor_id: Option<u64>,
    active_hidden_layers: Vec<u64>,
    /// Shared project ID of the active tab; all other open projects wait here.
    active_project_id: tabs::ProjectId,
    projects: std::collections::HashMap<tabs::ProjectId, tabs::SharedProject>,
    next_project_id: tabs::ProjectId,
    next_tab_id: tabs::TabId,
    /// Which side holds the active tab when viewing side by side.
    active_pane: tabs::TabPane,
    /// The tab shown beside the active one, with the same view.
    split_tab: Option<usize>,
    /// Horizontal fraction (`0.20..=0.80`) given to the left side when split.
    split_ratio: f32,
    /// Last cross-discipline dependency sync report for the active project.
    sync_report: crate::domain::dependencies::DependencySyncReport,
    /// Start Page (*Página Inicial*, T-06) visibility flag.
    show_start_page: bool,
    /// Pending autosave draft awaiting user choice (*Recuperar / Descartar / Adiar*).
    pending_recovery: Option<Project>,
    /// User-registered `.trazot` project templates.
    user_templates: crate::io::templates::UserTemplates,
    /// Recently opened/saved project files shown as cards on the Start Page.
    recent_files: crate::io::templates::RecentFiles,
    /// Lazily decoded PNG thumbnails extracted from recent `.trazo` ZIP files.
    thumbnail_cache: std::collections::HashMap<PathBuf, Option<egui::TextureHandle>>,
    /// Quick command palette (`Ctrl+K`) state.
    show_command_palette: bool,
    command_palette_query: String,
    command_palette_selected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenResult {
    Success,
    Error,
}

impl eframe::App for FloorPlanApp {
    /// Runs before every frame and also while the window is minimized, so
    /// an assistant's tool calls are answered even then.
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.background_wake_ready {
            let wake_ctx = context.clone();
            self.background.set_wake(move || wake_ctx.request_repaint());
            self.background_wake_ready = true;
        }
        self.process_mcp_calls();
        self.poll_update_check();
        self.publish_plan_changes();
        self.schedule_background_analysis();
        self.save_recovery_if_pending();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Closes the public tunnel and revokes its secret token with the app.
        self.tunnel = None;
        self.tunnel_token.clear();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        self.show_layout(ui);
        // A refused edit's reason wins over a message written after it.
        if let Some(notice) = self.edit_notice.take() {
            self.message = notice;
        }
        self.persist_settings();
        self.save_recovery_if_pending();
        let frame_delay = self
            .frame_pacer
            .record_frame(Instant::now(), frame_start.elapsed());
        let load_path_animating = self.project.structure.overlay.load_path
            && self
                .project
                .category_visible(crate::domain::layers::LayerCategory::Structure)
            && (!self.project.structure.beams.is_empty()
                || !self.project.structure.slabs.is_empty());
        if !self.show_start_page && (self.ai_activity.animating() || load_path_animating) {
            ui.ctx().request_repaint_after(frame_delay);
        }
    }
}
