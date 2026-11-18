use super::super::{FloorPlanApp, Selection, Tool};
use super::icons::{self as toolbar, ToolIcon};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Color32, PointerButton, RichText, Ui};

impl FloorPlanApp {
    pub(super) fn ribbon(&mut self, ui: &mut Ui) {
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
                        "Terreno",
                        ToolIcon::Terrain,
                        self.tool == Tool::Terrain,
                        false,
                    )
                    .clicked()
                    {
                        self.set_tool(Tool::Terrain);
                        self.sync_terrain_fields();
                    }
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
                    "Selecionar",
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
            if ui
                .add_enabled_ui(self.selection.is_some(), |ui| {
                    toolbar::tool_tile(ui, "Excluir", ToolIcon::Delete, false, false)
                })
                .inner
                .on_hover_text("Excluir item selecionado")
                .clicked()
            {
                self.delete_selection();
            }
            if let Some(selection) = self.selection {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("SELEÇÃO")
                            .size(10.0)
                            .color(Color32::from_rgb(117, 136, 143)),
                    );
                    match selection {
                        Selection::Wall(id) => {
                            if let Some(wall) = self.project.walls.iter().find(|wall| wall.id == id)
                            {
                                ui.label(
                                    RichText::new(format!(
                                        "Parede · {}",
                                        format_meters(wall.length())
                                    ))
                                    .strong(),
                                );
                            }
                        }
                        Selection::Room(id) => {
                            if let Some(room) = self.project.rooms.iter().find(|room| room.id == id)
                            {
                                ui.label(RichText::new(room.display_name()).strong());
                            }
                        }
                        Selection::Terrain(_) => {
                            if let Some(terrain) = &self.project.terrain {
                                ui.label(
                                    RichText::new(
                                        format!("Terreno · {:.2} m²", terrain.area())
                                            .replace('.', ","),
                                    )
                                    .strong(),
                                );
                            }
                        }
                    }
                });
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
}
