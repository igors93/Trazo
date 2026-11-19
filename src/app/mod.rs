mod canvas;
mod interaction;
mod theme;
mod ui;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key};

use crate::{
    domain::{
        geometry::Point,
        history::History,
        model::{OpeningKind, Project},
    },
    io::{
        export, project_file, recovery,
        settings::{self, Settings},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    NewProject,
    OpenProject,
    CloseApp,
}

/// How far an arrow key moves the selection: one grid step.
const NUDGE_STEP: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RibbonTab {
    Architecture,
    View,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tool {
    Wall,
    Room,
    Terrain,
    Move,
    Opening(OpeningKind),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Selection {
    Wall(u64),
    Room(u64),
    Terrain(u64),
    Opening(u64),
}

#[derive(Clone, Copy)]
struct ItemDrag {
    selection: Selection,
    last: Point,
    /// Where an opening was grabbed, relative to its centre.
    grab_offset: Point,
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
    selection: Option<Selection>,
    item_drag: Option<ItemDrag>,
    drag_initial_project: Option<Project>,
    pending_room: Option<(Point, Point)>,
    room_name: String,
    room_dialog_focus: bool,
    opening_drag: Option<OpeningKind>,
    scale: f32,
    pan: egui::Vec2,
    last_canvas_center: Option<egui::Pos2>,
    fit_pending: bool,
    snap_enabled: bool,
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
    browser_filter: String,
    browser_hover: Option<Selection>,
    cursor_world: Option<Point>,
    zoom_target: Option<(Point, Point)>,
    window_title: String,
    /// The interface preferences as last written to disk.
    saved_settings: Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenResult {
    Success,
    Cancelled,
    Error,
}

impl FloorPlanApp {
    pub fn new(context: &egui::Context) -> Self {
        theme::apply(context);
        let recovered = if cfg!(test) { None } else { recovery::load() };
        let restored = recovered.is_some();
        let preferences = if cfg!(test) {
            Settings::default()
        } else {
            settings::load()
        };
        Self {
            project: recovered.unwrap_or_default(),
            history: History::new(),
            file_path: None,
            dirty: restored,
            recovery_pending: false,
            pending_action: None,
            tool: Tool::Move,
            creation_state: CreationState::Idle,
            selection: None,
            item_drag: None,
            drag_initial_project: None,
            pending_room: None,
            room_name: String::new(),
            room_dialog_focus: false,
            opening_drag: None,
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            last_canvas_center: None,
            fit_pending: restored,
            snap_enabled: preferences.snap_enabled,
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
            browser_filter: String::new(),
            browser_hover: None,
            cursor_world: None,
            zoom_target: None,
            window_title: String::new(),
            saved_settings: preferences,
        }
    }

    fn current_settings(&self) -> Settings {
        Settings {
            show_grid: self.show_grid,
            snap_enabled: self.snap_enabled,
            ortho_enabled: self.ortho_enabled,
            show_browser: self.show_browser,
            show_properties: self.show_properties,
            ribbon_collapsed: self.ribbon_collapsed,
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
            CreationState::Idle => None,
            CreationState::WaitingSecondPoint { start } => Some(start),
            CreationState::Dragging { start } => Some(start),
        }
    }

    pub fn cancel_creation(&mut self) {
        self.creation_state = CreationState::Idle;
    }

    /// Ends an element drag in progress, keeping what it moved as one undo
    /// step. Whatever interrupts a drag (a tool key, Delete) goes through
    /// here, so a move is never left outside the history and unsaved.
    fn commit_item_drag(&mut self) {
        let drag = self.item_drag.take();
        if let Some(initial) = self.drag_initial_project.take()
            && drag.is_some_and(|drag| drag.moved)
        {
            self.record_change(initial);
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_item_drag();
        self.tool = tool;
        self.creation_state = CreationState::Idle;
        self.selection = None;
        self.message = match tool {
            Tool::Wall => "Clique em dois pontos para desenhar uma parede.",
            Tool::Room => "Clique em dois cantos opostos e dê um nome ao cômodo.",
            Tool::Terrain => {
                "Clique em dois cantos ou digite largura e comprimento para criar o terreno."
            }
            Tool::Move => "Clique para selecionar. Arraste parede, cômodo ou terreno para mover.",
            Tool::Opening(OpeningKind::Door) => "Clique em uma parede para colocar uma porta.",
            Tool::Opening(OpeningKind::Window) => "Clique em uma parede para colocar uma janela.",
        }
        .to_owned();
    }

    pub fn request_new_project(&mut self) {
        if self.dirty {
            self.pending_action = Some(PendingAction::NewProject);
        } else {
            self.new_project();
        }
    }

    pub fn request_open(&mut self) {
        if self.dirty {
            self.pending_action = Some(PendingAction::OpenProject);
        } else {
            self.open();
        }
    }

    pub fn request_close(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.pending_action = Some(PendingAction::CloseApp);
        } else {
            recovery::clear();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn execute_pending_action(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::NewProject => self.new_project(),
                PendingAction::OpenProject => {
                    let _ = self.open();
                }
                PendingAction::CloseApp => {
                    recovery::clear();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    pub fn discard_changes_and_proceed(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action {
            match action {
                PendingAction::NewProject => {
                    self.pending_action = None;
                    self.new_project();
                }
                PendingAction::OpenProject => match self.open() {
                    OpenResult::Success => {
                        self.pending_action = None;
                    }
                    OpenResult::Cancelled => {
                        self.pending_action = None;
                        self.message = "Abertura cancelada. Projeto atual mantido.".to_owned();
                    }
                    OpenResult::Error => {
                        self.pending_action = None;
                    }
                },
                PendingAction::CloseApp => {
                    self.pending_action = None;
                    recovery::clear();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
        self.message = "Operação cancelada.".to_owned();
    }

    fn new_project(&mut self) {
        recovery::clear();
        self.project = Project::default();
        self.history.clear();
        self.file_path = None;
        self.dirty = false;
        self.recovery_pending = false;
        self.pending_action = None;
        self.tool = Tool::Move;
        self.creation_state = CreationState::Idle;
        self.selection = None;
        self.item_drag = None;
        self.drag_initial_project = None;
        self.pending_room = None;
        self.pan = egui::Vec2::ZERO;
        self.last_canvas_center = None;
        self.scale = 60.0;
        self.fit_pending = false;
        self.terrain_width_text = "5,00".to_owned();
        self.terrain_height_text = "10,00".to_owned();
        self.message = "Nova planta criada.".to_owned();
    }

    pub fn open(&mut self) -> OpenResult {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projeto Trazo", &["json"])
            .pick_file()
        else {
            return OpenResult::Cancelled;
        };
        self.open_file(&path)
    }

    pub fn open_file(&mut self, path: &Path) -> OpenResult {
        match project_file::load(path) {
            Ok((project, warnings)) => {
                recovery::clear();
                self.project = project;
                self.history.clear();
                self.file_path = Some(path.to_path_buf());
                self.dirty = false;
                self.recovery_pending = false;
                self.pending_action = None;
                self.tool = Tool::Move;
                self.creation_state = CreationState::Idle;
                self.selection = None;
                self.item_drag = None;
                self.drag_initial_project = None;
                self.pending_room = None;
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
                .add_filter("Projeto Trazo", &["json"])
                .set_file_name("minha-planta.trazo.json")
                .save_file()
            {
                Some(path) => path,
                None => return false,
            },
        };
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
                    self.record_change(previous);
                    self.selection = Some(Selection::Terrain(id));
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
                let previous = self.project.clone();
                if self.project.add_wall(start, end).is_some() {
                    self.record_change(previous);
                    self.message = "Parede criada.".to_owned();
                } else {
                    self.message =
                        "A parede deve caber dentro do terreno e ter pelo menos 0,25 m.".to_owned();
                }
            }
            Tool::Room => {
                if (start.x - end.x).abs() >= 0.5 && (start.y - end.y).abs() >= 0.5 {
                    self.pending_room = Some((start, end));
                    self.room_name.clear();
                    self.room_dialog_focus = true;
                    self.message = "Digite o nome do cômodo para concluir.".to_owned();
                } else {
                    self.message = "Dimensões do cômodo muito pequenas. Mínimo 0,50 m.".to_owned();
                }
            }
            _ => {}
        }
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

    pub fn revalidate_selection(&mut self) {
        let valid = match self.selection {
            Some(Selection::Wall(id)) => self.project.walls.iter().any(|w| w.id == id),
            Some(Selection::Room(id)) => self.project.rooms.iter().any(|r| r.id == id),
            Some(Selection::Opening(id)) => self.project.openings.iter().any(|o| o.id == id),
            Some(Selection::Terrain(_)) => self.project.terrain.is_some(),
            None => false,
        };
        if !valid {
            self.selection = None;
        }
    }

    fn undo(&mut self) {
        if self.history.undo(&mut self.project) {
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
        if self.history.redo(&mut self.project) {
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
        let width = (max.x - min.x).max(0.25);
        let height = (max.y - min.y).max(0.25);
        self.scale = ((rect.width() - 100.0).max(80.0) / width)
            .min((rect.height() - 100.0).max(80.0) / height)
            .clamp(3.0, max_scale);
        self.pan = egui::vec2(
            -(min.x + max.x) * self.scale / 2.0,
            -(min.y + max.y) * self.scale / 2.0,
        );
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
        self.selection = Some(selection);
        self.sync_inspector_fields();
        self.message = Self::selection_message(self.selection).to_owned();
    }

    fn zoom_to_selection(&mut self, selection: Selection) {
        if let Some(bounds) = self.selection_bounds(selection) {
            self.zoom_target = Some(bounds);
        }
    }

    /// Moves the selected element by `delta` metres as one undo step,
    /// within the terrain like a drag. Doors and windows slide along their
    /// wall, so only the part of `delta` along the wall counts.
    fn nudge_selection(&mut self, delta: Point) {
        let Some(selection) = self.selection else {
            return;
        };
        if self.item_drag.is_some() || self.draft_start().is_some() {
            return;
        }
        let previous = self.project.clone();
        match selection {
            Selection::Wall(id) => self.project.move_wall(id, delta),
            Selection::Room(id) => {
                if !self.project.move_room(id, delta) {
                    self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                    return;
                }
            }
            Selection::Terrain(id) => self.project.move_terrain(id, delta),
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
        self.record_change(previous);
        self.sync_inspector_fields();
        self.message = format!(
            "Seleção movida {}. Setas: 0,25 m · Shift+setas: 1 m.",
            crate::domain::geometry::format_meters(delta.x.abs().max(delta.y.abs()))
        );
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
        self.record_change(previous);
        self.selection = Some(Selection::Terrain(id));
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
            self.record_change(previous);
            self.fit_view_requested();
            self.message = "Planta centralizada no terreno.".to_owned();
        } else {
            self.message = "A planta precisa caber no terreno para ser centralizada.".to_owned();
        }
    }

    fn terrain_dimensions(&self) -> Option<(f32, f32)> {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f32>().ok();
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

    fn record_change(&mut self, previous: Project) {
        if previous == self.project {
            return;
        }
        self.history.record(previous);
        self.mark_edited_outside_history();
    }

    /// Marks the project as changed and due for the crash-recovery draft,
    /// for edits that are not undo steps (such as typing the project name).
    fn mark_edited_outside_history(&mut self) {
        self.dirty = true;
        self.recovery_pending = true;
    }

    fn delete_selection(&mut self) {
        self.commit_item_drag();
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
            }
            self.record_change(previous);
            self.message = match selection {
                Selection::Wall(_) => "Parede removida.",
                Selection::Room(_) => "Cômodo removido.",
                Selection::Terrain(_) => "Terreno removido.",
                Selection::Opening(_) => "Abertura removida.",
            }
            .to_owned();
            self.sync_inspector_fields();
        }
    }

    pub fn sync_inspector_fields(&mut self) {
        self.sync_terrain_fields();
        match self.selection {
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
                }
            }
            _ => {}
        }
    }

    pub fn apply_wall_length(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f32>().ok();
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
                self.record_change(previous);
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
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f32>().ok();
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
        let dimensions_changed = if let Some((min, max)) = self.project.room_bounds(&room) {
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
        self.record_change(previous);
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
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f32>().ok();
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
                self.record_change(previous);
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
            if self.project.add_opening(wall_id, kind, t).is_some() {
                self.record_change(previous);
                self.message = format!("{} adicionada à parede.", kind.label());
            } else {
                self.message = "Sem espaço suficiente nesta parede.".to_owned();
            }
        } else {
            self.message = "Solte o elemento sobre uma parede.".to_owned();
        }
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        // Canvas and toolbar widgets can hold focus without editing text.
        let editing_text = context.text_edit_focused();
        // While a dialog is open, editing commands must not act on the
        // project behind it; the dialog handles its own keys.
        let modal_open =
            self.pending_room.is_some() || self.pending_action.is_some() || self.show_help;
        let blocking_dialog = self.pending_room.is_some() || self.pending_action.is_some();
        let actions = context.input(|input| {
            let pressed = |key: Key| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, .. } if *event_key == key)
                })
            };
            let plain = |key: Key| {
                !editing_text
                    && !modal_open
                    && input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                            if *event_key == key && modifiers.is_none())
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
                (Key::R, Tool::Opening(OpeningKind::Door)),
                (Key::J, Tool::Opening(OpeningKind::Window)),
            ]
            .into_iter()
            .find_map(|(key, tool)| plain(key).then_some(tool));
            let free = !modal_open;
            ShortcutActions {
                save: shortcut(Key::S, false) && free,
                open: shortcut(Key::O, false) && free,
                new: shortcut(Key::N, false) && free,
                undo: shortcut(Key::Z, false) && !editing_text && free,
                redo: (shortcut(Key::Y, false) || shortcut(Key::Z, true)) && !editing_text && free,
                escape: pressed(Key::Escape) && self.pending_action.is_none(),
                delete: pressed(Key::Delete) && !editing_text && free,
                tool,
                nudge,
                fit: plain(Key::F),
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
        if actions.new {
            self.request_new_project();
        }
        if actions.undo {
            self.undo();
        }
        if actions.redo {
            self.redo();
        }
        if actions.escape {
            if self.show_help {
                self.show_help = false;
            } else if self.draft_start().is_none()
                && self.pending_room.is_none()
                && self.selection.is_some()
            {
                self.selection = None;
                self.message = "Seleção limpa.".to_owned();
            } else {
                self.cancel_creation();
                self.pending_room = None;
                self.message = "Ação cancelada.".to_owned();
            }
        }
        if actions.delete {
            self.delete_selection();
        }
        if let Some(delta) = actions.nudge {
            self.nudge_selection(delta);
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
    undo: bool,
    redo: bool,
    escape: bool,
    delete: bool,
    tool: Option<Tool>,
    nudge: Option<Point>,
    fit: bool,
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_layout(ui);
        self.persist_settings();
        if self.recovery_pending {
            match recovery::save(&self.project) {
                Ok(()) => self.recovery_pending = false,
                Err(error) => {
                    self.message = format!("Não foi possível proteger o rascunho: {error}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut FloorPlanApp, context: &egui::Context, key: Key, modifiers: egui::Modifiers) {
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
        app.pending_action = Some(PendingAction::NewProject);
        press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
        press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
        press(&mut app, &context, Key::Z, ctrl);
        press(&mut app, &context, Key::P, egui::Modifiers::NONE);
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.selection, Some(Selection::Wall(id)));
        assert_eq!(app.tool, Tool::Move);
        assert_eq!(app.pending_action, Some(PendingAction::NewProject));

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
        let door = app.project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
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
    fn dirty_project_requests_confirmation_before_new_project() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let prev = app.project.clone();
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        app.record_change(prev);
        assert!(app.dirty);

        app.request_new_project();
        assert_eq!(app.pending_action, Some(PendingAction::NewProject));
        // Wall was not discarded
        assert_eq!(app.project.walls.len(), 1);

        app.cancel_pending_action();
        assert_eq!(app.pending_action, None);
        assert_eq!(app.project.walls.len(), 1);
        assert!(app.dirty);

        app.request_new_project();
        app.discard_changes_and_proceed(&context);
        assert_eq!(app.pending_action, None);
        assert_eq!(app.project.walls.len(), 0);
        assert!(!app.dirty);
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
}
