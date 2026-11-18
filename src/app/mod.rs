mod canvas;
mod layout;
mod toolbar;

use std::path::PathBuf;

use eframe::egui::{self, Color32, Key, PointerButton, Sense};

use crate::{
    domain::{
        geometry::Point,
        history::History,
        model::{OpeningKind, Project},
    },
    io::{export, project_file},
};

use canvas::{Preview, ViewTransform};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Wall,
    Room,
    Move,
    Opening(OpeningKind),
}

#[derive(Clone, Copy, PartialEq)]
enum Selection {
    Wall(u64),
    Room(u64),
}

#[derive(Clone, Copy)]
struct ItemDrag {
    selection: Selection,
    last: Point,
    moved: bool,
}

pub struct FloorPlanApp {
    project: Project,
    history: History,
    file_path: Option<PathBuf>,
    dirty: bool,
    tool: Tool,
    draft_start: Option<Point>,
    selection: Option<Selection>,
    item_drag: Option<ItemDrag>,
    pending_room: Option<(Point, Point)>,
    room_name: String,
    room_dialog_focus: bool,
    opening_drag: Option<OpeningKind>,
    scale: f32,
    pan: egui::Vec2,
    message: String,
}

impl FloorPlanApp {
    pub fn new(context: &egui::Context) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.widgets.active.bg_fill = Color32::from_rgb(33, 164, 150);
        visuals.selection.bg_fill = Color32::from_rgb(31, 177, 159);
        context.set_visuals(visuals);
        Self {
            project: Project::default(),
            history: History::new(),
            file_path: None,
            dirty: false,
            tool: Tool::Wall,
            draft_start: None,
            selection: None,
            item_drag: None,
            pending_room: None,
            room_name: String::new(),
            room_dialog_focus: false,
            opening_drag: None,
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            message: "Clique em dois pontos para desenhar uma parede.".to_owned(),
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.draft_start = None;
        self.item_drag = None;
        self.selection = None;
        self.message = match tool {
            Tool::Wall => "Clique em dois pontos para desenhar uma parede.",
            Tool::Room => "Clique em dois cantos opostos e dê um nome ao cômodo.",
            Tool::Move => {
                "Clique para selecionar. Arraste uma parede ou o interior de um cômodo para mover."
            }
            Tool::Opening(OpeningKind::Door) => "Clique em uma parede para colocar uma porta.",
            Tool::Opening(OpeningKind::Window) => "Clique em uma parede para colocar uma janela.",
        }
        .to_owned();
    }

    fn new_project(&mut self) {
        self.project = Project::default();
        self.history.clear();
        self.file_path = None;
        self.dirty = false;
        self.draft_start = None;
        self.selection = None;
        self.pending_room = None;
        self.pan = egui::Vec2::ZERO;
        self.scale = 60.0;
        self.message = "Nova planta criada.".to_owned();
    }

    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projeto Trazo", &["json"])
            .pick_file()
        else {
            return;
        };
        match project_file::load(&path) {
            Ok(project) => {
                self.project = project;
                self.history.clear();
                self.file_path = Some(path);
                self.dirty = false;
                self.draft_start = None;
                self.selection = None;
                self.pending_room = None;
                self.fit_view_requested();
                self.message = "Projeto aberto com sucesso.".to_owned();
            }
            Err(error) => self.message = format!("Não foi possível abrir: {error}"),
        }
    }

    fn save(&mut self) {
        let path = match self.file_path.clone() {
            Some(path) => path,
            None => match rfd::FileDialog::new()
                .add_filter("Projeto Trazo", &["json"])
                .set_file_name("minha-planta.trazo.json")
                .save_file()
            {
                Some(path) => path,
                None => return,
            },
        };
        match project_file::save(&self.project, &path) {
            Ok(()) => {
                self.file_path = Some(path);
                self.dirty = false;
                self.message = "Projeto salvo com sucesso.".to_owned();
            }
            Err(error) => self.message = format!("Não foi possível salvar: {error}"),
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

    fn undo(&mut self) {
        if self.history.undo(&mut self.project) {
            self.dirty = true;
            self.selection = None;
            self.message = "Ação desfeita.".to_owned();
        }
    }

    fn redo(&mut self) {
        if self.history.redo(&mut self.project) {
            self.dirty = true;
            self.selection = None;
            self.message = "Ação refeita.".to_owned();
        }
    }

    fn fit_view_requested(&mut self) {
        if let Some((min, max)) = self.project.bounds() {
            self.pan = egui::vec2(
                -(min.x + max.x) * self.scale / 2.0,
                -(min.y + max.y) * self.scale / 2.0,
            );
        } else {
            self.pan = egui::Vec2::ZERO;
        }
    }

    fn record_change(&mut self, previous: Project) {
        self.history.record(previous);
        self.dirty = true;
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

    fn snap(&self, point: Point, start: Option<Point>) -> Point {
        let mut result = point.snapped();
        let mut closest = 0.18;
        for wall in &self.project.walls {
            for endpoint in [wall.start, wall.end] {
                let distance = point.distance(endpoint);
                if distance < closest {
                    closest = distance;
                    result = endpoint;
                }
            }
        }
        if let Some(start) = start {
            if (result.x - start.x).abs() < 0.18 {
                result.x = start.x;
            }
            if (result.y - start.y).abs() < 0.18 {
                result.y = start.y;
            }
        }
        result
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                if let Some(position) = ui.input(|input| input.pointer.hover_pos()) {
                    let before = ViewTransform {
                        rect,
                        scale: self.scale,
                        pan: self.pan,
                    }
                    .to_world(position);
                    self.scale = (self.scale * (scroll * 0.0015).exp()).clamp(25.0, 180.0);
                    self.pan = position
                        - rect.center()
                        - egui::vec2(before.x * self.scale, before.y * self.scale);
                }
            }
        }
        let (middle_down, pointer_delta) =
            ui.input(|input| (input.pointer.middle_down(), input.pointer.delta()));
        if middle_down
            && rect.contains(ui.input(|input| input.pointer.hover_pos().unwrap_or_default()))
        {
            self.pan += pointer_delta;
        }
        let view = ViewTransform {
            rect,
            scale: self.scale,
            pan: self.pan,
        };
        let pointer = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|position| rect.contains(*position));
        let world = pointer.map(|position| view.to_world(position));
        let snapped = world.map(|point| self.snap(point, self.draft_start));

        if self.pending_room.is_some() {
            canvas::paint(&painter, view, &self.project, self.selection, None);
            return;
        }

        if self.tool == Tool::Move {
            if let Some(point) = world {
                if self
                    .project
                    .nearest_wall(point, 12.0 / self.scale)
                    .is_some()
                    || self.project.room_at(point).is_some()
                {
                    ui.ctx().set_cursor_icon(if self.item_drag.is_some() {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
            }
        }

        if self.tool == Tool::Move {
            let pressed = ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    } if rect.contains(*pos) => Some(*pos),
                    _ => None,
                })
            });
            if let Some(position) = pressed {
                let point = view.to_world(position);
                self.selection = self
                    .project
                    .nearest_wall(point, 12.0 / self.scale)
                    .map(|(id, _)| Selection::Wall(id))
                    .or_else(|| self.project.room_at(point).map(Selection::Room));
                self.item_drag = self.selection.map(|selection| ItemDrag {
                    selection,
                    last: point.snapped(),
                    moved: false,
                });
                self.message = match self.selection {
                    Some(Selection::Wall(_)) => {
                        "Parede selecionada. Arraste para mover ou clique em Excluir."
                    }
                    Some(Selection::Room(_)) => {
                        "Cômodo selecionado. Arraste para mover ou clique em Excluir."
                    }
                    None => "Clique em uma parede ou dentro de um cômodo para selecionar.",
                }
                .to_owned();
            }
            let primary_down = ui.input(|input| input.pointer.primary_down());
            if primary_down {
                if let (Some(point), Some(drag)) = (world, self.item_drag) {
                    let current = point.snapped();
                    let delta = Point::new(current.x - drag.last.x, current.y - drag.last.y);
                    if delta.x != 0.0 || delta.y != 0.0 {
                        if !drag.moved {
                            self.record_change(self.project.clone());
                        }
                        match drag.selection {
                            Selection::Wall(id) => self.project.move_wall(id, delta),
                            Selection::Room(id) => self.project.move_room(id, delta),
                        }
                        if let Some(drag) = &mut self.item_drag {
                            drag.last = current;
                            drag.moved = true;
                        }
                    }
                }
            }
        }
        if ui.input(|input| input.pointer.any_released()) {
            self.item_drag = None;
            if let Some(kind) = self.opening_drag.take() {
                if let Some(point) = world {
                    self.add_opening_at(point, kind);
                }
            }
        }

        if response.clicked_by(PointerButton::Primary) {
            if let Some(point) = snapped {
                match self.tool {
                    Tool::Wall | Tool::Room => {
                        if let Some(start) = self.draft_start.take() {
                            if self.tool == Tool::Wall {
                                let previous = self.project.clone();
                                if self.project.add_wall(start, point).is_some() {
                                    self.record_change(previous);
                                    self.message = "Parede criada.".to_owned();
                                } else {
                                    self.message = "Escolha pontos mais distantes.".to_owned();
                                }
                            } else if (start.x - point.x).abs() >= 0.5
                                && (start.y - point.y).abs() >= 0.5
                            {
                                self.pending_room = Some((start, point));
                                self.room_name.clear();
                                self.room_dialog_focus = true;
                                self.message = "Digite o nome do cômodo para concluir.".to_owned();
                            } else {
                                self.message = "Escolha pontos mais distantes.".to_owned();
                            }
                        } else {
                            self.draft_start = Some(point);
                            self.message = "Agora clique no segundo ponto. Esc cancela.".to_owned();
                        }
                    }
                    Tool::Opening(kind) => self.add_opening_at(point, kind),
                    Tool::Move => {}
                }
            }
        }

        let preview = match (self.draft_start, snapped, self.tool) {
            (Some(start), Some(end), Tool::Wall) => Some(Preview::Wall(start, end)),
            (Some(start), Some(end), Tool::Room) => Some(Preview::Room(start, end)),
            _ => None,
        };
        canvas::paint(&painter, view, &self.project, self.selection, preview);
        if let Some(position) = pointer {
            if self.tool == Tool::Wall || self.tool == Tool::Room {
                if let Some(point) = snapped {
                    painter.circle_filled(
                        view.to_screen(point),
                        4.5,
                        Color32::from_rgb(28, 169, 151),
                    );
                }
            }
            if let Some(kind) = self.opening_drag {
                painter.text(
                    position + egui::vec2(15.0, -15.0),
                    egui::Align2::LEFT_CENTER,
                    kind.label(),
                    egui::FontId::proportional(14.0),
                    Color32::from_rgb(25, 121, 116),
                );
            }
        }
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        let actions = context.input(|input| {
            (
                input.modifiers.command && input.key_pressed(Key::S),
                input.modifiers.command && input.key_pressed(Key::O),
                input.modifiers.command
                    && input.key_pressed(Key::Z)
                    && !input.modifiers.shift
                    && !context.egui_wants_keyboard_input(),
                input.modifiers.command
                    && (input.key_pressed(Key::Y)
                        || (input.modifiers.shift && input.key_pressed(Key::Z)))
                    && !context.egui_wants_keyboard_input(),
                input.key_pressed(Key::Escape),
                input.key_pressed(Key::Delete) && !context.egui_wants_keyboard_input(),
            )
        });
        if actions.0 {
            self.save();
        }
        if actions.1 {
            self.open();
        }
        if actions.2 {
            self.undo();
        }
        if actions.3 {
            self.redo();
        }
        if actions.4 {
            self.draft_start = None;
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
    }
}
