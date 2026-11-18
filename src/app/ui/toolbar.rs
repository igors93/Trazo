use super::super::{FloorPlanApp, Selection, Tool};
use super::icons::{self as toolbar, ToolIcon};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Color32, PointerButton, RichText, Ui};

impl FloorPlanApp {
    pub(super) fn ribbon(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Grupo 1: Selecionar e Criação
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if toolbar::tool_tile(
                        ui,
                        "Selecionar",
                        ToolIcon::Move,
                        self.tool == Tool::Move,
                        false,
                    )
                    .on_hover_text("Selecionar e mover elementos na planta (V)")
                    .clicked()
                    {
                        self.set_tool(Tool::Move);
                    }
                    if toolbar::tool_tile(
                        ui,
                        "Terreno",
                        ToolIcon::Terrain,
                        self.tool == Tool::Terrain,
                        false,
                    )
                    .on_hover_text("Criar ou dimensionar terreno (clique e arraste ou 2 cliques)")
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
                    .on_hover_text("Desenhar parede (clique e arraste ou 2 cliques)")
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
                    .on_hover_text("Criar cômodo retangular (clique e arraste ou 2 cantos)")
                    .clicked()
                    {
                        self.set_tool(Tool::Room);
                    }
                });
                ui.label(
                    RichText::new("DESENHO")
                        .size(9.5)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Grupo 2: Aberturas
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    self.opening_tile(ui, OpeningKind::Door, "Porta", ToolIcon::Door);
                    self.opening_tile(ui, OpeningKind::Window, "Janela", ToolIcon::Window);
                });
                ui.label(
                    RichText::new("ABERTURAS")
                        .size(9.5)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Grupo 3: Ações contextuais
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(self.selection.is_some(), |ui| {
                            toolbar::tool_tile(ui, "Excluir", ToolIcon::Delete, false, false)
                        })
                        .inner
                        .on_hover_text("Excluir item selecionado (Delete)")
                        .clicked()
                    {
                        self.delete_selection();
                    }

                    // Botão de alternar encaixe
                    let snap_label = if self.snap_enabled {
                        "Encaixe: Sim"
                    } else {
                        "Encaixe: Não"
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(snap_label).size(11.0).color(
                                if self.snap_enabled {
                                    Color32::from_rgb(20, 122, 112)
                                } else {
                                    Color32::from_rgb(120, 135, 140)
                                },
                            ))
                            .min_size(egui::vec2(60.0, 46.0))
                            .fill(if self.snap_enabled {
                                Color32::from_rgb(228, 245, 241)
                            } else {
                                Color32::from_rgb(243, 246, 247)
                            }),
                        )
                        .on_hover_text(
                            "Ativar ou desativar encaixe magnético na grade e pontos de conexão",
                        )
                        .clicked()
                    {
                        self.snap_enabled = !self.snap_enabled;
                    }
                });
                ui.label(
                    RichText::new("AÇÕES")
                        .size(9.5)
                        .color(Color32::from_rgb(117, 136, 143)),
                );
            });

            if let Some(selection) = self.selection {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("SELEÇÃO ATIVA")
                            .size(9.5)
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
                        Selection::Opening(id) => {
                            if let Some(opening) = self.project.openings.iter().find(|o| o.id == id)
                            {
                                let name = match opening.kind {
                                    OpeningKind::Door => "Porta",
                                    OpeningKind::Window => "Janela",
                                };
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        name,
                                        format_meters(opening.width())
                                    ))
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
        let tooltip = match kind {
            OpeningKind::Door => "Inserir porta na parede (clique ou arraste)",
            OpeningKind::Window => "Inserir janela na parede (clique ou arraste)",
        };
        let response = toolbar::tool_tile(ui, label, icon, self.tool == Tool::Opening(kind), true)
            .on_hover_text(tooltip);
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
