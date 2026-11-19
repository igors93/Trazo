use eframe::egui::{self, Key, Margin, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, vec2};

use super::super::{FloorPlanApp, Selection, Tool, theme::*};
use super::icons::{self, Icon};
use super::widgets::{
    danger_button, hint, icon_button_sized, panel_header, primary_button, property_label,
    property_value, secondary_button, section,
};
use crate::domain::{
    geometry::{Point, WALL_THICKNESS_METERS, format_meters},
    model::OpeningKind,
};

/// A point in drawing coordinates, with Y pointing up as in CAD.
pub(super) fn format_point(point: Point) -> String {
    format!("X {:.2}  Y {:.2}", point.x + 0.0, -point.y + 0.0).replace('.', ",")
}

fn area(value: f32) -> String {
    format!("{value:.2} m²").replace('.', ",")
}

fn grid(ui: &mut Ui, id: &str, body: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .min_col_width(96.0)
        .show(ui, body);
}

/// A text field for metres, aligned right, with its unit.
fn metres_field(ui: &mut Ui, text: &mut String) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let response = ui.add(
            egui::TextEdit::singleline(text)
                .desired_width(92.0)
                .align(egui::Align2::RIGHT_CENTER),
        );
        ui.label(RichText::new("m").color(MUTED));
        response
    })
    .inner
}

/// The card at the top of the panel that says what is selected.
fn type_card(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
    egui::Frame::new()
        .inner_margin(Margin {
            left: 12,
            right: 12,
            top: 2,
            bottom: 10,
        })
        .show(ui, |ui| {
            egui::Frame::new()
                .fill(PANEL_ALT)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(8)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (tile, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                        ui.painter().rect_filled(tile, 8, ACCENT_SOFT);
                        icons::paint(
                            ui.painter(),
                            Rect::from_center_size(tile.center(), vec2(22.0, 22.0)),
                            icon,
                            ACCENT_TEXT,
                            ACCENT,
                        );
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.add(
                                egui::Label::new(
                                    RichText::new(title).font(strong(14.0)).color(TEXT),
                                )
                                .truncate(),
                            );
                            ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
                        });
                    });
                });
        });
}

/// The buttons that close a properties form.
fn actions(ui: &mut Ui, apply: Option<&str>, delete: Option<&str>) -> (bool, bool) {
    let mut result = (false, false);
    egui::Frame::new()
        .inner_margin(Margin {
            left: 14,
            right: 12,
            top: 12,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(label) = apply {
                    result.0 = primary_button(ui, label).clicked();
                }
                if let Some(label) = delete {
                    result.1 = danger_button(ui, label).clicked();
                }
            });
        });
    result
}

impl FloorPlanApp {
    pub(super) fn show_inspector(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
        let header = panel_header(ui, "Propriedades", "");
        let close = Rect::from_center_size(
            Pos2::new(header.right() - 20.0, header.center().y),
            vec2(24.0, 24.0),
        );
        if ui
            .put(close, |ui: &mut Ui| {
                icon_button_sized(ui, Icon::Close, "Ocultar propriedades", false, 22.0, 13.0)
            })
            .clicked()
        {
            self.show_properties = false;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                match self.selection {
                    Some(Selection::Fixture(id)) => self.fixture_inspector(ui, id),
                    Some(Selection::Wall(id)) => self.wall_inspector(ui, id),
                    Some(Selection::Room(id)) => self.room_inspector(ui, id),
                    Some(Selection::Opening(id)) => self.opening_inspector(ui, id),
                    Some(Selection::Terrain(_)) => self.terrain_inspector(ui),
                    None if self.tool == Tool::Terrain => self.terrain_inspector(ui),
                    None => self.project_inspector(ui),
                }
            });
    }

    fn missing(ui: &mut Ui, text: &str) {
        egui::Frame::new()
            .inner_margin(Margin::same(14))
            .show(ui, |ui| hint(ui, Icon::Warning, text, DANGER));
    }

    fn fixture_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(fixture) = self.project.fixtures.iter().find(|f| f.id == id).copied() else {
            Self::missing(ui, "Objeto não encontrado.");
            return;
        };
        type_card(
            ui,
            Icon::Fixture(fixture.kind),
            fixture.kind.label(),
            "Objeto · equipamento",
        );
        let mut submit = false;
        section(ui, "fixture_dimensions", "Dimensões", |ui| {
            grid(ui, "fixture_dimensions_grid", |ui| {
                property_label(ui, "Largura");
                let width = metres_field(ui, &mut self.fixture_width_text);
                ui.end_row();
                property_label(ui, "Profundidade");
                let depth = metres_field(ui, &mut self.fixture_depth_text);
                ui.end_row();
                submit |= (width.lost_focus() || depth.lost_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter));
            });
        });
        let mut rotate = false;
        section(ui, "fixture_position", "Posição", |ui| {
            grid(ui, "fixture_position_grid", |ui| {
                property_label(ui, "Centro");
                property_value(ui, &format_point(fixture.center));
                ui.end_row();
                property_label(ui, "Rotação");
                ui.horizontal(|ui| {
                    property_value(ui, &format!("{}°", fixture.quarter_turns as u32 * 90));
                    rotate = secondary_button(ui, "Girar 90° (G)").clicked();
                });
                ui.end_row();
            });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Arraste para mover (passos de 5 cm com encaixe) ou use as setas.",
                ACCENT,
            );
        });
        if rotate {
            self.rotate_fixture_or_next();
        }
        let (apply, delete) = actions(ui, Some("Aplicar"), Some("Excluir objeto"));
        if apply || submit {
            self.apply_fixture_size(id);
        }
        if delete {
            self.delete_selection();
        }
    }

    fn wall_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(wall) = self.project.walls.iter().find(|w| w.id == id).cloned() else {
            Self::missing(ui, "Parede não encontrada.");
            return;
        };
        type_card(
            ui,
            Icon::Wall,
            "Parede",
            &format!(
                "Parede básica · {:.0} cm",
                WALL_THICKNESS_METERS * 100.0
            ),
        );

        let mut submit = false;
        section(ui, "wall_dimensions", "Dimensões", |ui| {
            grid(ui, "wall_dimensions_grid", |ui| {
                property_label(ui, "Comprimento");
                let edit = metres_field(ui, &mut self.wall_length_text);
                submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.end_row();
                property_label(ui, "Espessura");
                property_value(ui, &format_meters(WALL_THICKNESS_METERS));
                ui.end_row();
                property_label(ui, "Ângulo");
                let angle = (-(wall.end.y - wall.start.y))
                    .atan2(wall.end.x - wall.start.x)
                    .to_degrees()
                    .rem_euclid(360.0);
                property_value(ui, &format!("{angle:.1}°").replace('.', ","));
                ui.end_row();
            });
        });
        section(ui, "wall_location", "Localização", |ui| {
            grid(ui, "wall_location_grid", |ui| {
                property_label(ui, "Início");
                property_value(ui, &format_point(wall.start));
                ui.end_row();
                property_label(ui, "Fim");
                property_value(ui, &format_point(wall.end));
                ui.end_row();
            });
        });
        let rooms: Vec<String> = self
            .project
            .rooms
            .iter()
            .filter(|room| room.wall_ids.contains(&id))
            .map(|room| room.display_name().to_owned())
            .collect();
        let openings = self
            .project
            .openings
            .iter()
            .filter(|opening| opening.wall_id == id)
            .count();
        section(ui, "wall_relations", "Relações", |ui| {
            grid(ui, "wall_relations_grid", |ui| {
                property_label(ui, "Aberturas");
                property_value(ui, &openings.to_string());
                ui.end_row();
                property_label(ui, "Cômodos");
                property_value(
                    ui,
                    &if rooms.is_empty() {
                        "Parede avulsa".to_owned()
                    } else {
                        rooms.join(", ")
                    },
                );
                ui.end_row();
            });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "O comprimento muda na direção da parede, respeitando os limites do terreno.",
                ACCENT,
            );
        });

        let (apply, delete) = actions(ui, Some("Aplicar"), Some("Excluir parede"));
        if apply || submit {
            self.apply_wall_length(id);
        }
        if delete {
            self.delete_selection();
        }
    }

    fn room_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(room) = self.project.rooms.iter().find(|r| r.id == id).cloned() else {
            Self::missing(ui, "Cômodo não encontrado.");
            return;
        };
        type_card(ui, Icon::Room, room.display_name(), "Cômodo retangular");

        let mut submit = false;
        section(ui, "room_identity", "Identificação", |ui| {
            grid(ui, "room_identity_grid", |ui| {
                property_label(ui, "Nome");
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.room_name_text).desired_width(150.0),
                );
                submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.end_row();
            });
        });
        let bounds = self.project.room_bounds(&room);
        let room_area = self.project.room_area(&room);
        section(ui, "room_dimensions", "Dimensões", |ui| {
            grid(ui, "room_dimensions_grid", |ui| {
                property_label(ui, "Largura");
                let width = metres_field(ui, &mut self.room_width_text);
                ui.end_row();
                property_label(ui, "Comprimento");
                let height = metres_field(ui, &mut self.room_height_text);
                ui.end_row();
                submit |= (width.lost_focus() || height.lost_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter));
                property_label(ui, "Área");
                ui.label(
                    RichText::new(room_area.map(area).unwrap_or_else(|| "—".to_owned()))
                        .font(strong(13.0))
                        .color(ACCENT_TEXT),
                );
                ui.end_row();
                if let Some((min, max)) = bounds {
                    property_label(ui, "Perímetro");
                    property_value(ui, &format_meters(2.0 * ((max.x - min.x) + (max.y - min.y))));
                    ui.end_row();
                }
            });
        });
        let doors = self
            .project
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door && room.wall_ids.contains(&o.wall_id))
            .count();
        let windows = self
            .project
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Window && room.wall_ids.contains(&o.wall_id))
            .count();
        section(ui, "room_relations", "Relações", |ui| {
            grid(ui, "room_relations_grid", |ui| {
                property_label(ui, "Paredes");
                property_value(ui, &room.wall_ids.len().to_string());
                ui.end_row();
                property_label(ui, "Portas");
                property_value(ui, &doors.to_string());
                ui.end_row();
                property_label(ui, "Janelas");
                property_value(ui, &windows.to_string());
                ui.end_row();
            });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Alterar as medidas move as paredes do cômodo, preservando portas e janelas.",
                ACCENT,
            );
        });

        let (apply, delete) = actions(ui, Some("Aplicar"), Some("Excluir cômodo"));
        if apply || submit {
            self.apply_room_dimensions(id);
        }
        if delete {
            self.delete_selection();
        }
    }

    fn opening_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(opening) = self.project.openings.iter().find(|o| o.id == id).cloned() else {
            Self::missing(ui, "Abertura não encontrada.");
            return;
        };
        let (icon, title, subtitle) = match opening.kind {
            OpeningKind::Door => (Icon::Door, "Porta", "Porta de abrir · 1 folha"),
            OpeningKind::Window => (Icon::Window, "Janela", "Janela em parede"),
            OpeningKind::SaloonDoor => (
                Icon::SaloonDoor,
                "Porta vai-e-vem",
                "Tipo faroeste · 2 folhas, abre para os dois lados",
            ),
        };
        type_card(ui, icon, title, subtitle);
        let wall_length = self
            .project
            .walls
            .iter()
            .find(|w| w.id == opening.wall_id)
            .map(|w| w.length())
            .unwrap_or(0.0);

        let mut submit = false;
        section(ui, "opening_dimensions", "Dimensões", |ui| {
            grid(ui, "opening_dimensions_grid", |ui| {
                property_label(ui, "Largura");
                let edit = metres_field(ui, &mut self.opening_width_text);
                submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.end_row();
                property_label(ui, "Limites");
                property_value(
                    ui,
                    &format!(
                        "{} a {}",
                        format_meters(opening.kind.min_width()),
                        format_meters(opening.kind.max_width())
                    ),
                );
                ui.end_row();
            });
        });
        section(ui, "opening_position", "Posição", |ui| {
            grid(ui, "opening_position_grid", |ui| {
                property_label(ui, "Na parede");
                property_value(ui, &format!("{:.0} %", opening.t * 100.0));
                ui.end_row();
                property_label(ui, "Do início");
                property_value(ui, &format_meters(opening.t * wall_length));
                ui.end_row();
                property_label(ui, "Parede");
                property_value(ui, &format_meters(wall_length));
                ui.end_row();
            });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Arraste a abertura com o mouse para deslizá-la ao longo da parede.",
                ACCENT,
            );
        });

        let delete_label = match opening.kind {
            OpeningKind::Door => "Excluir porta",
            OpeningKind::Window => "Excluir janela",
            OpeningKind::SaloonDoor => "Excluir porta",
        };
        let (apply, delete) = actions(ui, Some("Aplicar"), Some(delete_label));
        if apply || submit {
            self.apply_opening_width(id);
        }
        if delete {
            self.delete_selection();
        }
    }

    fn built_area(&self) -> f32 {
        self.project
            .rooms
            .iter()
            .filter_map(|room| self.project.room_area(room))
            .sum()
    }

    fn terrain_inspector(&mut self, ui: &mut Ui) {
        let exists = self.project.terrain.is_some();
        type_card(
            ui,
            Icon::Terrain,
            "Terreno",
            if exists {
                "Lote do projeto"
            } else {
                "Novo terreno com medidas exatas"
            },
        );

        let mut submit = false;
        section(ui, "terrain_dimensions", "Dimensões", |ui| {
            grid(ui, "terrain_dimensions_grid", |ui| {
                property_label(ui, "Largura");
                let width = metres_field(ui, &mut self.terrain_width_text);
                ui.end_row();
                property_label(ui, "Comprimento");
                let height = metres_field(ui, &mut self.terrain_height_text);
                ui.end_row();
                submit |= (width.lost_focus() || height.lost_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter));
                property_label(ui, "Área");
                match self.terrain_dimensions() {
                    Some((w, h)) => {
                        ui.label(
                            RichText::new(area(w * h))
                                .font(strong(13.0))
                                .color(ACCENT_TEXT),
                        );
                    }
                    None => {
                        ui.label(RichText::new("Valores inválidos").color(DANGER));
                    }
                }
                ui.end_row();
            });
            if self.terrain_dimensions().is_none() {
                ui.add_space(6.0);
                hint(
                    ui,
                    Icon::Warning,
                    "Informe valores entre 0,25 e 10.000 m.",
                    DANGER,
                );
            }
        });

        if let Some(terrain) = &self.project.terrain {
            let built = self.built_area();
            let lot = terrain.area();
            section(ui, "terrain_occupation", "Ocupação", |ui| {
                grid(ui, "terrain_occupation_grid", |ui| {
                    property_label(ui, "Área construída");
                    property_value(ui, &area(built));
                    ui.end_row();
                    property_label(ui, "Taxa de ocupação");
                    property_value(
                        ui,
                        &format!("{:.1} %", built / lot * 100.0).replace('.', ","),
                    );
                    ui.end_row();
                    property_label(ui, "Área livre");
                    property_value(ui, &area((lot - built).max(0.0)));
                    ui.end_row();
                });
                if !self.project.walls.is_empty() && !self.project.content_inside_terrain() {
                    ui.add_space(6.0);
                    hint(
                        ui,
                        Icon::Warning,
                        "Há paredes ou cômodos fora do terreno. Use Centralizar planta.",
                        WARNING,
                    );
                }
            });
        }

        let valid = self.terrain_dimensions().is_some();
        let selected = matches!(self.selection, Some(Selection::Terrain(_)));
        egui::Frame::new()
            .inner_margin(Margin {
                left: 14,
                right: 12,
                top: 12,
                bottom: 12,
            })
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let label = if exists { "Aplicar medidas" } else { "Criar terreno" };
                    if ui
                        .add_enabled_ui(valid, |ui| primary_button(ui, label))
                        .inner
                        .clicked()
                    {
                        submit = true;
                    }
                    if exists && !self.project.walls.is_empty() {
                        let can_center = self.project.content_fits_terrain();
                        if ui
                            .add_enabled_ui(can_center, |ui| {
                                secondary_button(ui, "Centralizar planta")
                            })
                            .inner
                            .clicked()
                        {
                            self.center_content_on_terrain();
                        }
                    }
                    if selected && danger_button(ui, "Excluir terreno").clicked() {
                        self.delete_selection();
                    }
                });
                if !exists {
                    ui.add_space(10.0);
                    hint(
                        ui,
                        Icon::Info,
                        "Você também pode desenhar o terreno na grade: arraste ou clique em dois cantos.",
                        ACCENT,
                    );
                }
            });
        if submit && valid {
            self.create_terrain_from_dimensions();
        }
    }

    fn project_inspector(&mut self, ui: &mut Ui) {
        type_card(ui, Icon::Plan, "Planta baixa · Térreo", "Propriedades do projeto");

        section(ui, "project_identity", "Identificação", |ui| {
            grid(ui, "project_identity_grid", |ui| {
                property_label(ui, "Nome");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.project.name)
                            .hint_text("Nome do projeto")
                            .desired_width(150.0),
                    )
                    .changed()
                {
                    self.mark_edited_outside_history();
                }
                ui.end_row();
                property_label(ui, "Arquivo");
                ui.add(
                    egui::Label::new(RichText::new(self.document_name()).color(TEXT)).truncate(),
                );
                ui.end_row();
                property_label(ui, "Situação");
                if self.dirty {
                    ui.label(RichText::new("Alterações não salvas").color(WARNING));
                } else {
                    ui.label(RichText::new("Salvo").color(SUCCESS));
                }
                ui.end_row();
            });
        });

        let built = self.built_area();
        let rooms: Vec<(String, f32)> = self
            .project
            .rooms
            .iter()
            .filter_map(|room| {
                Some((
                    room.display_name().to_owned(),
                    self.project.room_area(room)?,
                ))
            })
            .collect();
        let terrain_area = self.project.terrain.as_ref().map(|terrain| terrain.area());
        section(ui, "project_areas", "Quadro de áreas", |ui| {
            grid(ui, "project_areas_grid", |ui| {
                for (name, room_area) in &rooms {
                    property_label(ui, name);
                    property_value(ui, &area(*room_area));
                    ui.end_row();
                }
                property_label(ui, "Área construída");
                ui.label(RichText::new(area(built)).font(strong(13.0)).color(ACCENT_TEXT));
                ui.end_row();
                property_label(ui, "Terreno");
                property_value(
                    ui,
                    &terrain_area.map(area).unwrap_or_else(|| "—".to_owned()),
                );
                ui.end_row();
                if let Some(lot) = terrain_area {
                    property_label(ui, "Ocupação");
                    property_value(
                        ui,
                        &format!("{:.1} %", built / lot * 100.0).replace('.', ","),
                    );
                    ui.end_row();
                }
            });
        });

        let doors = self
            .project
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .count();
        let windows = self.project.openings.len() - doors;
        section(ui, "project_elements", "Elementos", |ui| {
            grid(ui, "project_elements_grid", |ui| {
                for (label, count) in [
                    ("Paredes", self.project.walls.len()),
                    ("Cômodos", self.project.rooms.len()),
                    ("Portas", doors),
                    ("Janelas", windows),
                ] {
                    property_label(ui, label);
                    property_value(ui, &count.to_string());
                    ui.end_row();
                }
            });
        });

        section(ui, "project_view", "Vista", |ui| {
            grid(ui, "project_view_grid", |ui| {
                property_label(ui, "Zoom");
                property_value(ui, &format!("{:.0} %", self.zoom_percent()));
                ui.end_row();
                property_label(ui, "Unidade");
                property_value(ui, "Metros (m)");
                ui.end_row();
                property_label(ui, "Grade");
                let mut grid_on = self.show_grid;
                if ui.checkbox(&mut grid_on, "Visível (F7)").changed() {
                    self.toggle_grid();
                }
                ui.end_row();
                property_label(ui, "Encaixe");
                let mut snap = self.snap_enabled;
                if ui.checkbox(&mut snap, "Ativo (F3)").changed() {
                    self.toggle_snap();
                }
                ui.end_row();
                property_label(ui, "Orto");
                let mut ortho = self.ortho_enabled;
                if ui.checkbox(&mut ortho, "Ativo (F8)").changed() {
                    self.toggle_ortho();
                }
                ui.end_row();
            });
        });
        ui.add_space(12.0);
    }

    pub(in crate::app) fn sync_terrain_fields(&mut self) {
        if let Some(terrain) = &self.project.terrain {
            self.terrain_width_text = format!("{:.2}", terrain.width()).replace('.', ",");
            self.terrain_height_text = format!("{:.2}", terrain.height()).replace('.', ",");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_read_with_y_up_and_decimal_commas() {
        assert_eq!(format_point(Point::new(1.5, 0.0)), "X 1,50  Y 0,00");
        assert_eq!(format_point(Point::new(0.0, 2.25)), "X 0,00  Y -2,25");
    }
}
