mod canvas;
mod interaction;
mod theme;
mod ui;

use std::path::PathBuf;

use eframe::egui::{self, Key};

use crate::{
    domain::{
        geometry::Point,
        history::History,
        model::{OpeningKind, Project},
    },
    io::{export, project_file},
};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Wall,
    Room,
    Terrain,
    Move,
    Opening(OpeningKind),
}

#[derive(Clone, Copy, PartialEq)]
enum Selection {
    Wall(u64),
    Room(u64),
    Terrain(u64),
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
    fit_pending: bool,
    terrain_width_text: String,
    terrain_height_text: String,
    message: String,
}

impl FloorPlanApp {
    pub fn new(context: &egui::Context) -> Self {
        theme::apply(context);
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
            fit_pending: false,
            terrain_width_text: "5,00".to_owned(),
            terrain_height_text: "10,00".to_owned(),
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
            Tool::Terrain => {
                "Clique em dois cantos ou digite largura e comprimento para criar o terreno."
            }
            Tool::Move => "Clique para selecionar. Arraste parede, cômodo ou terreno para mover.",
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
        self.fit_pending = false;
        self.terrain_width_text = "5,00".to_owned();
        self.terrain_height_text = "10,00".to_owned();
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
                self.sync_terrain_fields();
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
            self.sync_terrain_fields();
            self.message = "Ação desfeita.".to_owned();
        }
    }

    fn redo(&mut self) {
        if self.history.redo(&mut self.project) {
            self.dirty = true;
            self.selection = None;
            self.sync_terrain_fields();
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

    fn create_terrain_from_dimensions(&mut self) {
        let Some((width, height)) = self.terrain_dimensions() else {
            self.message = "Informe medidas válidas de pelo menos 0,25 m.".to_owned();
            return;
        };
        if !width.is_finite() || !height.is_finite() || width < 0.25 || height < 0.25 {
            self.message = "Informe medidas de pelo menos 0,25 m.".to_owned();
            return;
        }
        let previous = self.project.clone();
        let existing = self.project.terrain.as_ref().map(|terrain| terrain.id);
        let id = if let Some(id) = existing {
            self.project.resize_terrain(width, height);
            id
        } else {
            self.project
                .set_terrain(Point::new(0.0, 0.0), Point::new(width, height))
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
        self.history.record(previous);
        self.dirty = true;
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
            }
            self.record_change(previous);
            self.message = match selection {
                Selection::Wall(_) => "Parede removida.",
                Selection::Room(_) => "Cômodo removido.",
                Selection::Terrain(_) => "Terreno removido.",
            }
            .to_owned();
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
