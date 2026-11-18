use eframe::egui::{self, Align, Color32, PointerButton, RichText, Ui};

use crate::domain::{geometry::format_meters, model::{OpeningKind, RoomKind}};

use super::{
    FloorPlanApp, Tool,
    toolbar::{self, ToolIcon},
};

impl FloorPlanApp {
    pub(super) fn show_layout(&mut self, ui: &mut Ui) {
        let context = ui.ctx().clone();
        self.handle_shortcuts(&context);

        egui::Panel::top("menu_bar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(248, 250, 251))
                    .inner_margin(egui::Margin::symmetric(15, 5)),
            )
            .show(ui, |ui| self.menu_bar(ui));

        egui::Panel::top("tool_ribbon")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(239, 244, 245))
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| self.ribbon(ui));

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(231, 239, 241))
                    .inner_margin(egui::Margin::symmetric(16, 7)),
            )
            .show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(248, 250, 251)))
            .show(ui, |ui| self.canvas(ui));
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Trazo")
                    .strong()
                    .size(16.0)
                    .color(Color32::from_rgb(19, 122, 111)),
            );
            ui.add_space(12.0);
            ui.menu_button("Arquivo", |ui| {
                if ui.button("Nova planta").clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button("Abrir...                 Ctrl+O").clicked() {
                    self.open();
                    ui.close();
                }
                if ui.button("Salvar                  Ctrl+S").clicked() {
                    self.save();
                    ui.close();
                }
                ui.separator();
                if ui.button("Exportar imagem PNG...").clicked() {
                    self.export_png();
                    ui.close();
                }
                if ui.button("Exportar PDF...").clicked() {
                    self.export_pdf();
                    ui.close();
                }
            });
            ui.menu_button("Editar", |ui| {
                if ui
                    .add_enabled(
                        self.history.can_undo(),
                        egui::Button::new("Desfazer            Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.history.can_redo(),
                        egui::Button::new("Refazer            Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.selected_wall.is_some(),
                        egui::Button::new("Excluir parede     Delete"),
                    )
                    .clicked()
                {
                    self.delete_selected_wall();
                    ui.close();
                }
            });
            ui.menu_button("Exibir", |ui| {
                if ui.button("Centralizar planta").clicked() {
                    self.fit_view_requested();
                    ui.close();
                }
                if ui.button("Ampliar").clicked() {
                    self.scale = (self.scale * 1.2).min(180.0);
                    ui.close();
                }
                if ui.button("Reduzir").clicked() {
                    self.scale = (self.scale / 1.2).max(25.0);
                    ui.close();
                }
            });
            ui.menu_button("Ajuda", |ui| {
                ui.label("Parede e cômodo: clique em dois pontos.");
                ui.label("Porta e janela: arraste ou clique na parede.");
                ui.label("Roda do mouse: zoom. Botão do meio: mover a vista.");
                ui.label("Esc: cancelar desenho.");
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if self.dirty { "Não salvo" } else { "Salvo" })
                        .size(11.0)
                        .color(Color32::from_rgb(105, 123, 130)),
                );
                if ui
                    .add(egui::TextEdit::singleline(&mut self.project.name).desired_width(175.0))
                    .changed()
                {
                    self.dirty = true;
                }
            });
        });
    }

    fn ribbon(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("INÍCIO")
                .strong()
                .size(10.0)
                .color(Color32::from_rgb(29, 134, 122)),
        );
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if toolbar::tool_tile(
                        ui,
                        "Parede",
                        ToolIcon::Wall,
                        self.tool == Tool::Wall,
                        false,
                    )
                    .clicked()
                    {
                        self.set_tool(Tool::Wall);
                    }
                    if toolbar::tool_tile(
                        ui,
                        "Cômodo",
                        ToolIcon::Room,
                        self.tool == Tool::Room,
                        false,
                    )
                    .clicked()
                    {
                        self.set_tool(Tool::Room);
                    }
                    ui.vertical(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Tipo de cômodo")
                                .size(11.0)
                                .color(Color32::from_rgb(91, 112, 121)),
                        );
                        let mut selected = false;
                        egui::ComboBox::from_id_salt("room_kind")
                            .width(112.0)
                            .selected_text(self.room_kind.label())
                            .show_ui(ui, |ui| {
                                for kind in RoomKind::ALL {
                                    if ui.selectable_value(&mut self.room_kind, kind, kind.label()).clicked() {
                                        selected = true;
                                    }
                                }
                            });
                        if selected {
                            self.set_tool(Tool::Room);
                        }
                    });
                });
                ui.label(
                    RichText::new("CRIAR")
                        .size(10.0)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    self.opening_tile(ui, OpeningKind::Door, "Porta", ToolIcon::Door);
                    self.opening_tile(ui, OpeningKind::Window, "Janela", ToolIcon::Window);
                });
                ui.label(
                    RichText::new("ABERTURAS")
                        .size(10.0)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);
            ui.vertical(|ui| {
                if toolbar::tool_tile(
                    ui,
                    "Mover",
                    ToolIcon::Move,
                    self.tool == Tool::Move,
                    false,
                )
                .clicked()
                {
                    self.set_tool(Tool::Move);
                }
                ui.label(
                    RichText::new("AJUSTAR")
                        .size(10.0)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });
            if let Some(id) = self.selected_wall {
                if let Some(wall) = self.project.walls.iter().find(|wall| wall.id == id) {
                    let length = wall.length();
                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("PAREDE SELECIONADA")
                                .size(10.0)
                                .color(Color32::from_rgb(117, 136, 143)),
                        );
                        ui.label(RichText::new(format_meters(length)).strong().size(14.0));
                        if ui.small_button("Excluir").clicked() {
                            self.delete_selected_wall();
                        }
                    });
                }
            }
        });
    }

    fn opening_tile(&mut self, ui: &mut Ui, kind: OpeningKind, label: &str, icon: ToolIcon) {
        let response = toolbar::tool_tile(ui, label, icon, self.tool == Tool::Opening(kind), true);
        let pressed_on_tile = ui.input(|input| {
            input.events.iter().any(|event| {
                matches!(event, egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if response.rect.contains(*pos))
            })
        });
        if response.clicked() {
            self.set_tool(Tool::Opening(kind));
        }
        if pressed_on_tile
            || response.is_pointer_button_down_on()
            || response.drag_started()
            || response.dragged()
            || response.drag_stopped()
        {
            self.opening_drag = Some(kind);
        }
    }

    fn status_bar(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.message).color(Color32::from_rgb(64, 89, 99)));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(format!("Zoom {:.0}%", self.scale / 60.0 * 100.0));
                ui.separator();
                ui.label(format!(
                    "{} paredes | {} cômodos | {} aberturas",
                    self.project.walls.len(),
                    self.project.rooms.len(),
                    self.project.openings.len()
                ));
            });
        });
    }

    pub(super) fn delete_selected_wall(&mut self) {
        if let Some(id) = self.selected_wall.take() {
            let previous = self.project.clone();
            self.project.delete_wall(id);
            self.record_change(previous);
            self.message = "Parede removida.".to_owned();
        }
    }
}
