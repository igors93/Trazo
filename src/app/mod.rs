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
    io::{export, project_file, recovery},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    NewProject,
    OpenProject,
    CloseApp,
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
            snap_enabled: true,
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

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.creation_state = CreationState::Idle;
        self.item_drag = None;
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
            let width = (max.x - min.x).max(0.25);
            let height = (max.y - min.y).max(0.25);
            self.scale = ((rect.width() - 100.0).max(80.0) / width)
                .min((rect.height() - 100.0).max(80.0) / height)
                .clamp(3.0, 240.0);
            self.pan = egui::vec2(
                -(min.x + max.x) * self.scale / 2.0,
                -(min.y + max.y) * self.scale / 2.0,
            );
        } else {
            self.pan = egui::Vec2::ZERO;
            self.scale = 60.0;
        }
    }

    fn prepare_canvas_view(&mut self, rect: egui::Rect) {
        if self.fit_pending {
            self.fit_view(rect);
        } else if let Some(previous_center) = self.last_canvas_center {
            self.pan += previous_center - rect.center();
        }
        self.last_canvas_center = Some(rect.center());
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
        self.dirty = true;
        self.recovery_pending = true;
    }

    fn delete_selection(&mut self) {
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
        let actions = context.input(|input| {
            let pressed = |key: Key| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, .. } if *event_key == key)
                })
            };
            let shortcut = |key: Key, shift: bool| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key: event_key, pressed: true, repeat: false, modifiers, .. }
                        if *event_key == key && (modifiers.command || modifiers.ctrl) && modifiers.shift == shift)
                })
            };
            (
                shortcut(Key::S, false),
                shortcut(Key::O, false),
                shortcut(Key::Z, false) && !editing_text,
                (shortcut(Key::Y, false) || shortcut(Key::Z, true)) && !editing_text,
                pressed(Key::Escape),
                pressed(Key::Delete) && !editing_text,
            )
        });
        if actions.0 {
            self.save();
        }
        if actions.1 {
            self.request_open();
        }
        if actions.2 {
            self.undo();
        }
        if actions.3 {
            self.redo();
        }
        if actions.4 {
            self.cancel_creation();
            self.pending_room = None;
            self.message = "Ação cancelada.".to_owned();
        }
        if actions.5 {
            self.delete_selection();
        }
    }
}

impl eframe::App for FloorPlanApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_layout(ui);
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
