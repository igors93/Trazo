mod canvas;

use std::path::PathBuf;

use eframe::egui::{self, Color32, Key, PointerButton, RichText, Sense};

use crate::{
    domain::{
        geometry::Point,
        history::History,
        model::{OpeningKind, Project, RoomKind},
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

struct WallDrag {
    wall_id: u64,
    last: Point,
}

pub struct FloorPlanApp {
    project: Project,
    history: History,
    file_path: Option<PathBuf>,
    dirty: bool,
    tool: Tool,
    room_kind: RoomKind,
    draft_start: Option<Point>,
    selected_wall: Option<u64>,
    wall_drag: Option<WallDrag>,
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
            room_kind: RoomKind::Bedroom,
            draft_start: None,
            selected_wall: None,
            wall_drag: None,
            opening_drag: None,
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            message: "Clique em dois pontos para desenhar uma parede.".to_owned(),
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.draft_start = None;
        self.wall_drag = None;
        self.message = match tool {
            Tool::Wall => "Clique em dois pontos para desenhar uma parede.",
            Tool::Room => "Escolha o cômodo e clique em dois cantos opostos.",
            Tool::Move => "Arraste uma parede para movê-la. As conexões acompanham.",
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
        self.selected_wall = None;
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
                self.selected_wall = None;
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
            self.selected_wall = None;
            self.message = "Ação desfeita.".to_owned();
        }
    }

    fn redo(&mut self) {
        if self.history.redo(&mut self.project) {
            self.dirty = true;
            self.selected_wall = None;
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

        if self.tool == Tool::Move && response.drag_started_by(PointerButton::Primary) {
            if let Some(point) = world {
                if let Some((id, _)) = self.project.nearest_wall(point, 12.0 / self.scale) {
                    self.record_change(self.project.clone());
                    self.selected_wall = Some(id);
                    self.wall_drag = Some(WallDrag {
                        wall_id: id,
                        last: point.snapped(),
                    });
                }
            }
        }
        if let (Some(drag), Some(point)) = (&mut self.wall_drag, world) {
            if ui.input(|input| input.pointer.primary_down()) {
                let current = point.snapped();
                let delta = Point::new(current.x - drag.last.x, current.y - drag.last.y);
                if delta.x != 0.0 || delta.y != 0.0 {
                    self.project.move_wall(drag.wall_id, delta);
                    self.dirty = true;
                    drag.last = current;
                }
            }
        }
        if ui.input(|input| input.pointer.any_released()) {
            self.wall_drag = None;
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
                            let previous = self.project.clone();
                            let created = if self.tool == Tool::Wall {
                                self.project.add_wall(start, point)
                            } else {
                                self.project.add_room(start, point, self.room_kind)
                            };
                            if created.is_some() {
                                self.record_change(previous);
                                self.message = if self.tool == Tool::Wall {
                                    "Parede criada."
                                } else {
                                    "Cômodo criado."
                                }
                                .to_owned();
                            } else {
                                self.message = "Escolha pontos mais distantes.".to_owned();
                            }
                        } else {
                            self.draft_start = Some(point);
                            self.message = "Agora clique no segundo ponto. Esc cancela.".to_owned();
                        }
                    }
                    Tool::Opening(kind) => self.add_opening_at(point, kind),
                    Tool::Move => {
                        self.selected_wall = self
                            .project
                            .nearest_wall(point, 12.0 / self.scale)
                            .map(|(id, _)| id);
                    }
                }
            }
        }

        let preview = match (self.draft_start, snapped, self.tool) {
            (Some(start), Some(end), Tool::Wall) => Some(Preview::Wall(start, end)),
            (Some(start), Some(end), Tool::Room) => Some(Preview::Room(start, end)),
            _ => None,
        };
        canvas::paint(&painter, view, &self.project, self.selected_wall, preview);
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
                input.modifiers.command && input.key_pressed(Key::Z) && !input.modifiers.shift,
                input.modifiers.command
                    && (input.key_pressed(Key::Y)
                        || (input.modifiers.shift && input.key_pressed(Key::Z))),
                input.key_pressed(Key::Escape),
                input.key_pressed(Key::Delete),
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
            self.message = "Ação cancelada.".to_owned();
        }
        if actions.5 {
            if let Some(id) = self.selected_wall.take() {
                let previous = self.project.clone();
                self.project.delete_wall(id);
                self.record_change(previous);
                self.message = "Parede removida.".to_owned();
            }
        }
    }
}

impl eframe::App for FloorPlanApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.handle_shortcuts(&context);
        egui::Panel::top("top_bar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(23, 36, 56))
                    .inner_margin(egui::Margin::symmetric(17, 11)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("TRAZO")
                            .strong()
                            .size(22.0)
                            .color(Color32::from_rgb(100, 224, 203)),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Plantas baixas simples")
                            .color(Color32::from_rgb(200, 214, 224)),
                    );
                    ui.add_space(28.0);
                    if ui.button("Nova").clicked() {
                        self.new_project();
                    }
                    if ui.button("Abrir").clicked() {
                        self.open();
                    }
                    if ui.button("Salvar").clicked() {
                        self.save();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(self.history.can_undo(), egui::Button::new("Desfazer"))
                        .clicked()
                    {
                        self.undo();
                    }
                    if ui
                        .add_enabled(self.history.can_redo(), egui::Button::new("Refazer"))
                        .clicked()
                    {
                        self.redo();
                    }
                    ui.separator();
                    if ui.button("Exportar PNG").clicked() {
                        self.export_png();
                    }
                    if ui.button("Exportar PDF").clicked() {
                        self.export_pdf();
                    }
                });
            });

        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(190.0)
            .frame(egui::Frame::new().fill(Color32::from_rgb(28, 43, 63)).inner_margin(egui::Margin::same(15)))
            .show(ui, |ui| {
                ui.add_space(14.0);
                ui.label(RichText::new("FERRAMENTAS").strong().size(11.0).color(Color32::from_rgb(147, 175, 188)));
                ui.add_space(11.0);
                for (tool, label) in [
                    (Tool::Wall, "Parede"),
                    (Tool::Room, "Cômodo"),
                    (Tool::Move, "Mover parede"),
                ] {
                    if ui.add_sized([160.0, 38.0], egui::Button::new(label).selected(self.tool == tool)).clicked() {
                        self.set_tool(tool);
                    }
                    ui.add_space(5.0);
                }
                ui.add_space(22.0);
                ui.label(RichText::new("ABERTURAS").strong().size(11.0).color(Color32::from_rgb(147, 175, 188)));
                ui.add_space(10.0);
                for (kind, label) in [(OpeningKind::Door, "Porta"), (OpeningKind::Window, "Janela")] {
                    let response = ui.add_sized([160.0, 40.0], egui::Button::new(label).selected(self.tool == Tool::Opening(kind)).sense(Sense::click_and_drag()));
                    if response.clicked() { self.set_tool(Tool::Opening(kind)); }
                    if response.is_pointer_button_down_on() || response.drag_started() || response.dragged() || response.drag_stopped() {
                        self.opening_drag = Some(kind);
                    }
                    ui.add_space(6.0);
                }
                ui.add_space(10.0);
                ui.label(RichText::new("Arraste para uma parede").size(11.0).color(Color32::from_rgb(176, 193, 201)));
                ui.add_space(27.0);
                ui.label(RichText::new("DICA RÁPIDA").strong().size(11.0).color(Color32::from_rgb(147, 175, 188)));
                ui.add_space(9.0);
                ui.label(RichText::new("Use a roda do mouse para ampliar e o botão do meio para deslocar a planta.").size(12.0).color(Color32::from_rgb(207, 220, 226)));
            });

        egui::Panel::right("details")
            .resizable(false)
            .exact_size(255.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| {
                ui.add_space(13.0);
                ui.label(
                    RichText::new("PROJETO")
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(104, 128, 138)),
                );
                ui.add_space(8.0);
                if ui.text_edit_singleline(&mut self.project.name).changed() {
                    self.dirty = true;
                }
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(15.0);
                ui.label(
                    RichText::new("CÔMODO")
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(104, 128, 138)),
                );
                ui.add_space(8.0);
                egui::ComboBox::from_id_salt("room_kind")
                    .selected_text(self.room_kind.label())
                    .show_ui(ui, |ui| {
                        for kind in RoomKind::ALL {
                            ui.selectable_value(&mut self.room_kind, kind, kind.label());
                        }
                    });
                ui.label(
                    RichText::new("Selecione Cômodo e marque dois cantos opostos.")
                        .size(12.0)
                        .color(Color32::from_rgb(106, 123, 130)),
                );
                ui.add_space(17.0);
                ui.separator();
                ui.add_space(15.0);
                ui.label(
                    RichText::new("RESUMO DA PLANTA")
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(104, 128, 138)),
                );
                ui.add_space(10.0);
                ui.label(format!("{} paredes", self.project.walls.len()));
                ui.label(format!("{} cômodos", self.project.rooms.len()));
                ui.label(format!("{} portas e janelas", self.project.openings.len()));
                ui.add_space(17.0);
                if let Some(id) = self.selected_wall {
                    if let Some(wall) = self.project.walls.iter().find(|wall| wall.id == id) {
                        ui.separator();
                        ui.add_space(13.0);
                        ui.label(
                            RichText::new("PAREDE SELECIONADA")
                                .strong()
                                .size(11.0)
                                .color(Color32::from_rgb(104, 128, 138)),
                        );
                        ui.label(format!("Comprimento: {:.2} m", wall.length()).replace('.', ","));
                        ui.add_space(8.0);
                        if ui.button("Excluir parede").clicked() {
                            let previous = self.project.clone();
                            self.project.delete_wall(id);
                            self.record_change(previous);
                            self.selected_wall = None;
                        }
                    }
                }
                ui.add_space(17.0);
                ui.separator();
                ui.add_space(15.0);
                ui.label(
                    RichText::new("GRADE INTELIGENTE")
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(104, 128, 138)),
                );
                ui.label(
                    RichText::new("Encaixe de 25 cm e alinhamento com extremidades existentes.")
                        .size(12.0)
                        .color(Color32::from_rgb(106, 123, 130)),
                );
            });

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(231, 239, 241))
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.message).color(Color32::from_rgb(64, 89, 99)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Zoom: {:.0}%", self.scale / 60.0 * 100.0));
                        ui.separator();
                        ui.label(if self.dirty {
                            "Alterações não salvas"
                        } else {
                            "Projeto salvo"
                        });
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(248, 250, 251)))
            .show(ui, |ui| self.canvas(ui));
    }
}
