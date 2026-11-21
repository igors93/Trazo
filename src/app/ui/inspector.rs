use eframe::egui::{self, Key, Margin, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, vec2};

use super::super::{FloorPlanApp, Selection, Tool, theme::*};

mod drawing;
mod room_shape;
use super::icons::{self, Icon};
use super::widgets::{
    danger_button, hint, icon_button_sized, panel_header, primary_button, property_label,
    property_value, secondary_button, section,
};
use crate::domain::{
    geometry::{Point, format_meters},
    model::OpeningKind,
};

/// A point in drawing coordinates, with Y pointing up as in CAD.
pub(super) fn format_point(point: Point) -> String {
    format!("X {:.2}  Y {:.2}", point.x + 0.0, -point.y + 0.0).replace('.', ",")
}

fn area(value: f64) -> String {
    // Never "-0,00": a value that rounds to zero is plain zero.
    let value = if value.abs() < 0.005 { 0.0 } else { value };
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
pub(super) fn type_card(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
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
                if self.is_multi_selection() {
                    self.multi_selection_inspector(ui);
                    return;
                }
                match self.selection {
                    Some(Selection::Fixture(id)) => self.fixture_inspector(ui, id),
                    Some(Selection::Drawing(id)) => self.drawing_inspector(ui, id),
                    Some(Selection::Wall(id)) => self.wall_inspector(ui, id),
                    Some(Selection::Room(id)) => self.room_inspector(ui, id),
                    Some(Selection::Opening(id)) => self.opening_inspector(ui, id),
                    Some(Selection::Terrain(_)) => self.terrain_inspector(ui),
                    None if self.tool == Tool::Terrain => self.terrain_inspector(ui),
                    None => self.project_inspector(ui),
                }
                if let Some(selected) = self.selection {
                    self.review_notices_section(ui, selected.id());
                }
                if let Some(entity) = self.selection.and_then(crate::domain::model::Entity::of) {
                    self.constraints_section(ui, entity);
                }
            });
    }

    fn review_notices_section(&self, ui: &mut Ui, id: u64) {
        use crate::domain::dependencies::ReviewSeverity;
        let audit = self.project.cached_dependency_audit();
        let items: Vec<_> = audit
            .review_items
            .iter()
            .filter(|r| r.element_id == Some(id) && r.severity != ReviewSeverity::Info)
            .collect();
        if items.is_empty() {
            return;
        }
        egui::Frame::new()
            .inner_margin(Margin::symmetric(14, 6))
            .show(ui, |ui| {
                for item in items {
                    let color = match item.severity {
                        ReviewSeverity::Critical => DANGER,
                        ReviewSeverity::Warning => egui::Color32::from_rgb(190, 110, 10),
                        ReviewSeverity::Info => MUTED,
                    };
                    hint(
                        ui,
                        Icon::Warning,
                        &format!("{} {}", item.message, item.suggestion),
                        color,
                    );
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
            if fixture.kind.is_electrical() {
                "Instalação elétrica"
            } else if fixture.kind.is_structural() {
                "Estrutura"
            } else {
                "Objeto · equipamento"
            },
        );
        self.electrical_device_section(ui, &fixture);
        let mut submit = false;
        section(ui, "fixture_dimensions", "Dimensões", |ui| {
            grid(ui, "fixture_dimensions_grid", |ui| {
                property_label(ui, "Largura");
                let width = metres_field(ui, &mut self.fixture_width_text);
                ui.end_row();
                property_label(ui, "Profundidade");
                let depth = metres_field(ui, &mut self.fixture_depth_text);
                ui.end_row();
                if fixture.structural_load_kn.is_some() {
                    property_label(ui, "Carga característica");
                    let load = ui
                        .horizontal(|ui| {
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut self.fixture_load_text)
                                    .desired_width(92.0)
                                    .align(egui::Align2::RIGHT_CENTER),
                            );
                            ui.label(RichText::new("kN").color(MUTED));
                            response
                        })
                        .inner;
                    ui.end_row();
                    submit |= load.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                }
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
            &wall
                .masonry
                .as_ref()
                .map(|material| material.label())
                .unwrap_or_else(|| "Parede de 12 cm (sem material definido)".to_owned()),
        );

        let mut submit = false;
        section(ui, "wall_dimensions", "Dimensões", |ui| {
            grid(ui, "wall_dimensions_grid", |ui| {
                property_label(ui, "Comprimento");
                let edit = metres_field(ui, &mut self.wall_length_text);
                submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.end_row();
                property_label(ui, "Espessura");
                property_value(ui, &format_meters(wall.thickness()));
                ui.end_row();
                property_label(ui, "Ângulo");
                let angle = (-(wall.end.y - wall.start.y))
                    .atan2(wall.end.x - wall.start.x)
                    .to_degrees()
                    .rem_euclid(360.0)
                    + 0.0;
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
        let mut use_active = false;
        section(ui, "wall_masonry", "Alvenaria", |ui| {
            grid(ui, "wall_masonry_grid", |ui| {
                if let Some(material) = &wall.masonry {
                    property_label(ui, "Bloco");
                    property_value(
                        ui,
                        &format!(
                            "{:.1} × {:.1} cm, junta {:.1}",
                            material.block_width * 100.0,
                            material.block_length * 100.0,
                            material.joint * 100.0
                        )
                        .replace(".0", "")
                        .replace('.', ","),
                    );
                    ui.end_row();
                    let (whole, cut) = material.course(wall.length());
                    property_label(ui, "1ª fiada");
                    property_value(
                        ui,
                        &if cut > 0.0 {
                            format!("{whole} inteiros + 1 cortado de {:.0} cm", cut * 100.0)
                        } else {
                            format!("{whole} blocos inteiros")
                        },
                    );
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            use_active = secondary_button(ui, "Usar o material atual nesta parede")
                .on_hover_text(self.project.masonry.label())
                .clicked();
        });
        if use_active {
            self.apply_masonry_to_walls(Some(id));
        }
        let mut next_assembly: Option<Option<crate::domain::wall_layers::WallAssembly>> = None;
        section(ui, "wall_multilayer", "Paredes multicamadas", |ui| {
            let current_name = wall
                .assembly
                .as_ref()
                .map(|a| a.name.as_str())
                .unwrap_or("Alvenaria simples (padrão)");
            egui::ComboBox::from_id_salt("wall_assembly_combo")
                .selected_text(current_name)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(wall.assembly.is_none(), "Alvenaria simples (padrão)")
                        .clicked()
                    {
                        next_assembly = Some(None);
                    }
                    for preset in crate::domain::wall_layers::WallAssembly::presets() {
                        let selected = wall
                            .assembly
                            .as_ref()
                            .is_some_and(|a| a.name == preset.name);
                        if ui.selectable_label(selected, &preset.name).clicked() {
                            next_assembly = Some(Some(preset));
                        }
                    }
                });
            if let Some(assembly) = &wall.assembly {
                ui.add_space(4.0);
                for layer in &assembly.layers {
                    ui.label(
                        RichText::new(
                            format!(
                                "• {} ({}) — {:.1} cm",
                                layer.name,
                                layer.role.label(),
                                layer.thickness_m * 100.0
                            )
                            .replace(".0 cm", " cm")
                            .replace('.', ","),
                        )
                        .font(regular(11.5))
                        .color(TEXT_2),
                    );
                }
            }
        });
        if let Some(chosen) = next_assembly {
            let prev = self.project.clone();
            if self.project.set_wall_assembly(id, chosen).is_ok() {
                self.record_change(prev);
            }
        }
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
        let rectangle = self.project.room_is_rectangle(&room);
        type_card(
            ui,
            if rectangle {
                Icon::Room
            } else {
                Icon::RoomPolygon
            },
            room.display_name(),
            if rectangle {
                "Cômodo retangular"
            } else {
                "Cômodo de forma livre"
            },
        );

        let mut submit = false;
        section(ui, "room_identity", "Identificação", |ui| {
            grid(ui, "room_identity_grid", |ui| {
                property_label(ui, "Nome");
                let edit = ui
                    .add(egui::TextEdit::singleline(&mut self.room_name_text).desired_width(150.0));
                submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.end_row();
            });
        });
        let bounds = self.project.room_bounds(&room);
        let room_area = self.project.room_area(&room);
        let outline = self.project.room_outline(&room);
        section(ui, "room_dimensions", "Dimensões", |ui| {
            grid(ui, "room_dimensions_grid", |ui| {
                if !rectangle {
                    room_shape::free_shape_dimensions(ui, room_area, outline.as_deref(), bounds);
                    return;
                }
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
                    property_value(
                        ui,
                        &format_meters(2.0 * ((max.x - min.x) + (max.y - min.y))),
                    );
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
        let current_hatch = room
            .hatch
            .map(|h| h.pattern)
            .unwrap_or(crate::domain::hatch::HatchPattern::None);
        let mut chosen_hatch = None;
        section(ui, "room_hatch", "Hachura e material", |ui| {
            grid(ui, "room_hatch_grid", |ui| {
                property_label(ui, "Padrão");
                egui::ComboBox::from_id_salt(("room_hatch_combo", id))
                    .width(152.0)
                    .selected_text(RichText::new(current_hatch.label()).size(12.0))
                    .show_ui(ui, |ui| {
                        for pattern in crate::domain::hatch::HatchPattern::ALL {
                            if ui
                                .selectable_label(current_hatch == pattern, pattern.label())
                                .clicked()
                            {
                                chosen_hatch = Some(pattern);
                            }
                        }
                    });
                ui.end_row();
            });
        });
        if let Some(pattern) = chosen_hatch {
            let prev = self.project.clone();
            let style = crate::domain::hatch::HatchStyle::for_pattern(pattern);
            if self.project.set_room_hatch(id, Some(style)).is_ok() {
                self.record_change(prev);
                self.message = format!("Hachura atualizada: {}.", pattern.label());
            }
        }
        let mut chosen_finish_preset = None;
        let mut updated_finishes = room.finishes.clone().unwrap_or_default();
        let mut finishes_edited = false;
        section(ui, "room_finishes", "Revestimentos e acabamentos", |ui| {
            egui::ComboBox::from_id_salt(("room_finish_preset", id))
                .width(ui.available_width())
                .selected_text("Aplicar pacote de acabamento...")
                .show_ui(ui, |ui| {
                    for preset in crate::domain::finishes::RoomFinishPreset::ALL {
                        if ui.selectable_label(false, preset.label()).clicked() {
                            chosen_finish_preset = Some(preset);
                        }
                    }
                });
            ui.add_space(6.0);
            grid(ui, "room_finishes_grid", |ui| {
                property_label(ui, "Piso");
                finishes_edited |= ui
                    .add(
                        egui::TextEdit::singleline(&mut updated_finishes.floor)
                            .desired_width(148.0)
                            .hint_text("Porcelanato 60×60"),
                    )
                    .changed();
                ui.end_row();
                property_label(ui, "Parede");
                finishes_edited |= ui
                    .add(
                        egui::TextEdit::singleline(&mut updated_finishes.wall)
                            .desired_width(148.0)
                            .hint_text("Pintura acrílica"),
                    )
                    .changed();
                ui.end_row();
                property_label(ui, "Teto");
                finishes_edited |= ui
                    .add(
                        egui::TextEdit::singleline(&mut updated_finishes.ceiling)
                            .desired_width(148.0)
                            .hint_text("Forro de gesso"),
                    )
                    .changed();
                ui.end_row();
                property_label(ui, "Rodapé");
                finishes_edited |= ui
                    .add(
                        egui::TextEdit::singleline(&mut updated_finishes.baseboard)
                            .desired_width(148.0)
                            .hint_text("Poliestireno"),
                    )
                    .changed();
                ui.end_row();
                property_label(ui, "Alt. rodapé");
                finishes_edited |= ui
                    .add(
                        egui::DragValue::new(&mut updated_finishes.baseboard_height_cm)
                            .range(0.0..=30.0)
                            .speed(0.5)
                            .suffix(" cm"),
                    )
                    .changed();
                ui.end_row();
                property_label(ui, "Notas");
                finishes_edited |= ui
                    .add(
                        egui::TextEdit::singleline(&mut updated_finishes.notes)
                            .desired_width(148.0)
                            .hint_text("Especificações técnicas"),
                    )
                    .changed();
                ui.end_row();
            });
            finishes_edited |= ui
                .checkbox(
                    &mut updated_finishes.waterproofing,
                    "Área molhada (impermeabilização)",
                )
                .changed();
            if let Some(qty) = self.project.room_finish_quantities(&room) {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        format!(
                            "Piso (+10%): {:.2} m² · Paredes: {:.2} m² · Rodapé: {:.1} m",
                            qty.floor_with_waste_m2, qty.wall_net_area_m2, qty.baseboard_linear_m
                        )
                        .replace('.', ","),
                    )
                    .font(regular(11.0))
                    .color(MUTED),
                );
            }
        });
        if let Some(preset) = chosen_finish_preset {
            let prev = self.project.clone();
            if self.project.apply_room_finish_preset(id, preset).is_ok() {
                self.record_change(prev);
                self.message = format!("Acabamentos aplicados: {}.", preset.label());
            }
        } else if finishes_edited
            && self
                .project
                .set_room_finishes(id, Some(updated_finishes))
                .is_ok()
        {
            self.mark_edited_outside_history();
        }
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
            OpeningKind::Door => (Icon::Door, "Porta", opening.effective_door_type().label()),
            OpeningKind::Gate => (
                Icon::Gate,
                "Portão",
                opening.effective_door_type().short_label(),
            ),
            OpeningKind::Window => (
                Icon::Window,
                "Janela",
                opening.effective_window_type().label(),
            ),
            OpeningKind::SaloonDoor => (
                Icon::SaloonDoor,
                "Porta vai-e-vem",
                "Tipo faroeste · 2 folhas, abre para os dois lados",
            ),
            OpeningKind::Gap => (Icon::Gap, "Vão livre", "Trecho aberto da parede, sem porta"),
        };
        type_card(ui, icon, title, subtitle);

        if matches!(opening.kind, OpeningKind::Door | OpeningKind::Gate) {
            let mut chosen_door_type = None;
            let typology = if opening.kind == OpeningKind::Gate {
                "Tipologia do portão"
            } else {
                "Tipologia da porta"
            };
            section(ui, "door_typology", typology, |ui| {
                let current = opening.effective_door_type();
                egui::ComboBox::from_id_salt("door_type_combo")
                    .selected_text(current.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for dt in crate::domain::openings::DoorType::ALL {
                            if ui.selectable_label(current == dt, dt.label()).clicked() {
                                chosen_door_type = Some(dt);
                            }
                        }
                    });
            });
            if let Some(dt) = chosen_door_type {
                let prev = self.project.clone();
                if self.project.set_opening_door_type(id, dt).is_ok() {
                    self.record_change(prev);
                }
            }
        } else if opening.kind == OpeningKind::Window {
            let mut chosen_window_type = None;
            section(ui, "window_typology", "Tipologia da janela", |ui| {
                let current = opening.effective_window_type();
                egui::ComboBox::from_id_salt("window_type_combo")
                    .selected_text(current.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for wt in crate::domain::openings::WindowType::ALL {
                            if ui.selectable_label(current == wt, wt.label()).clicked() {
                                chosen_window_type = Some(wt);
                            }
                        }
                    });
            });
            if let Some(wt) = chosen_window_type {
                let prev = self.project.clone();
                if self.project.set_opening_window_type(id, wt).is_ok() {
                    self.record_change(prev);
                }
            }
        }
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
        let types = crate::domain::annotations::frame_types(&self.project);
        let frame = crate::domain::annotations::frame_of(&types, &opening).cloned();
        let mut heights_submit = false;
        if opening.kind != OpeningKind::Gap {
            section(ui, "opening_frame", "Esquadria", |ui| {
                grid(ui, "opening_frame_grid", |ui| {
                    if let Some(frame) = &frame {
                        property_label(ui, "Tipo");
                        ui.label(
                            RichText::new(frame.tag())
                                .font(strong(13.0))
                                .color(ACCENT_TEXT),
                        );
                        ui.end_row();
                    }
                    property_label(ui, "Altura");
                    let height = metres_field(ui, &mut self.opening_height_text);
                    ui.end_row();
                    let mut sill_lost = false;
                    if opening.kind == OpeningKind::Window {
                        property_label(ui, "Peitoril");
                        sill_lost = metres_field(ui, &mut self.opening_sill_text).lost_focus();
                        ui.end_row();
                    }
                    heights_submit = (height.lost_focus() || sill_lost)
                        && ui.input(|i| i.key_pressed(Key::Enter));
                });
            });
        }

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

        if opening.kind.is_door() {
            let mut flip = (false, false);
            section(ui, "opening_swing", "Abertura da folha", |ui| {
                grid(ui, "opening_swing_grid", |ui| {
                    property_label(ui, "Lado");
                    ui.horizontal(|ui| {
                        property_value(
                            ui,
                            if opening.flip_side {
                                "Invertido"
                            } else {
                                "Padrão"
                            },
                        );
                        flip.0 = secondary_button(ui, "Inverter (I)").clicked();
                    });
                    ui.end_row();
                    if matches!(opening.kind, OpeningKind::Door | OpeningKind::Gate) {
                        property_label(ui, "Dobradiça");
                        ui.horizontal(|ui| {
                            property_value(
                                ui,
                                if opening.flip_hinge {
                                    "Lado do fim"
                                } else {
                                    "Lado do início"
                                },
                            );
                            flip.1 = secondary_button(ui, "Trocar (Shift+I)").clicked();
                        });
                        ui.end_row();
                    }
                });
            });
            if flip.0 || flip.1 {
                self.flip_selected_door(flip.0, flip.1);
            }
        }

        let delete_label = match opening.kind {
            OpeningKind::Door => "Excluir porta",
            OpeningKind::Window => "Excluir janela",
            OpeningKind::SaloonDoor => "Excluir porta",
            OpeningKind::Gap => "Fechar o vão",
            OpeningKind::Gate => "Excluir portão",
        };
        let (apply, delete) = actions(ui, Some("Aplicar"), Some(delete_label));
        if apply || submit {
            self.apply_opening_width(id);
        }
        if apply || heights_submit {
            self.apply_opening_heights(id);
        }
        if delete {
            self.delete_selection();
        }
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

        if self.project.terrain.is_some() {
            let summary = self.project.area_summary();
            section(ui, "terrain_occupation", "Ocupação", |ui| {
                grid(ui, "terrain_occupation_grid", |ui| {
                    property_label(ui, "Área construída");
                    property_value(ui, &area(summary.constructed));
                    ui.end_row();
                    property_label(ui, "Área de projeção");
                    property_value(ui, &area(summary.projection));
                    ui.end_row();
                    property_label(ui, "Taxa de ocupação");
                    property_value(
                        ui,
                        &format!("{:.1} %", summary.projection / summary.terrain * 100.0)
                            .replace('.', ","),
                    );
                    ui.end_row();
                    property_label(ui, "Área livre");
                    property_value(ui, &area((summary.terrain - summary.projection).max(0.0)));
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

        if exists {
            let mut setbacks = self.project.setbacks;
            let mut changed = false;
            section(
                ui,
                "terrain_setbacks",
                "Recuos e limite de implantação",
                |ui| {
                    grid(ui, "terrain_setbacks_grid", |ui| {
                        property_label(ui, "Frontal");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut setbacks.front)
                                    .range(0.0..=1000.0)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            )
                            .changed();
                        ui.end_row();
                        property_label(ui, "Fundos");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut setbacks.rear)
                                    .range(0.0..=1000.0)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            )
                            .changed();
                        ui.end_row();
                        property_label(ui, "Laterais");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut setbacks.side)
                                    .range(0.0..=1000.0)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            )
                            .changed();
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    hint(
                        ui,
                        Icon::Info,
                        "A linha tracejada laranja mostra a faixa edificável e também limita novas paredes.",
                        ACCENT,
                    );
                    if self.project.setbacks.is_active()
                        && !self.project.content_inside_buildable_limit()
                    {
                        ui.add_space(6.0);
                        hint(
                            ui,
                            Icon::Warning,
                            "Há paredes fora do limite de implantação configurado.",
                            WARNING,
                        );
                    }
                },
            );
            if changed {
                let previous = self.project.clone();
                if self.project.set_setbacks(setbacks).is_ok() {
                    self.record_change(previous);
                }
            }
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
                if exists {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if secondary_button(ui, "Dividir verticalmente").clicked() {
                            self.subdivide_terrain(true);
                        }
                        if secondary_button(ui, "Dividir horizontalmente").clicked() {
                            self.subdivide_terrain(false);
                        }
                    });
                    ui.add_space(6.0);
                    hint(
                        ui,
                        Icon::Info,
                        "Selecione o terreno e arraste seus vértices para criar divisas inclinadas.",
                        ACCENT,
                    );
                }
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
        type_card(
            ui,
            Icon::Plan,
            "Planta baixa · Térreo",
            "Propriedades do projeto",
        );

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

        let summary = self.project.area_summary();
        let rooms: Vec<(String, f64)> = self
            .project
            .rooms
            .iter()
            .filter_map(|room| {
                Some((
                    room.display_name().to_owned(),
                    self.project.room_usable_area(room)?,
                ))
            })
            .collect();
        section(ui, "project_areas", "Quadro de áreas", |ui| {
            grid(ui, "project_areas_grid", |ui| {
                for (name, room_area) in &rooms {
                    property_label(ui, name);
                    property_value(ui, &area(*room_area));
                    ui.end_row();
                }
                property_label(ui, "Área útil");
                property_value(ui, &area(summary.usable));
                ui.end_row();
                property_label(ui, "Paredes");
                property_value(ui, &area(summary.walls));
                ui.end_row();
                property_label(ui, "Área construída");
                ui.label(
                    RichText::new(area(summary.constructed))
                        .font(strong(13.0))
                        .color(ACCENT_TEXT),
                );
                ui.end_row();
                property_label(ui, "Área de projeção");
                property_value(ui, &area(summary.projection));
                ui.end_row();
                property_label(ui, "Área do terreno");
                property_value(
                    ui,
                    &self
                        .project
                        .terrain
                        .as_ref()
                        .map_or_else(|| "—".to_owned(), |_| area(summary.terrain)),
                );
                ui.end_row();
                if summary.terrain > 0.0 {
                    property_label(ui, "Ocupação");
                    property_value(
                        ui,
                        &format!("{:.1} %", summary.projection / summary.terrain * 100.0)
                            .replace('.', ","),
                    );
                    ui.end_row();
                }
            });
        });

        let doors = self
            .project
            .openings
            .iter()
            .filter(|o| matches!(o.kind, OpeningKind::Door | OpeningKind::Gate))
            .count();
        let windows = self.project.openings.len() - doors;
        let (wall_area, blocks) = self.project.masonry_estimate();
        let mut apply_all = false;
        let mut height_submit = false;
        section(ui, "project_masonry", "Alvenaria", |ui| {
            grid(ui, "project_masonry_grid", |ui| {
                property_label(ui, "Material atual");
                ui.add(
                    egui::Label::new(RichText::new(self.project.masonry.label()).color(TEXT))
                        .wrap(),
                );
                ui.end_row();
                property_label(ui, "Parede acabada");
                property_value(ui, &format_meters(self.project.masonry.thickness()));
                ui.end_row();
                property_label(ui, "Pé-direito");
                let height = metres_field(ui, &mut self.wall_height_text);
                height_submit = height.lost_focus();
                ui.end_row();
                property_label(ui, "Área de parede");
                property_value(ui, &area(wall_area));
                ui.end_row();
                property_label(ui, "Blocos (estim.)");
                ui.label(
                    RichText::new(format!(
                        "{:.0} + 5% = {:.0}",
                        blocks,
                        (blocks * 1.05).ceil()
                    ))
                    .font(strong(13.0))
                    .color(ACCENT_TEXT),
                );
                ui.end_row();
            });
            ui.add_space(6.0);
            apply_all = secondary_button(ui, "Aplicar o material a todas as paredes").clicked();
            ui.add_space(4.0);
            hint(
                ui,
                Icon::Info,
                "Estimativa: comprimento das paredes × pé-direito, menos portas e janelas. Troque o material na barra de opções da ferramenta Parede.",
                ACCENT,
            );
        });
        if apply_all {
            self.apply_masonry_to_walls(None);
        }
        if height_submit {
            self.apply_wall_height();
        }

        let types = crate::domain::annotations::frame_types(&self.project);
        if !types.is_empty() {
            section(ui, "project_frames", "Quadro de esquadrias", |ui| {
                grid(ui, "project_frames_grid", |ui| {
                    for frame in &types {
                        ui.label(
                            RichText::new(frame.tag())
                                .font(strong(12.5))
                                .color(ACCENT_TEXT),
                        );
                        let sill = if frame.kind == OpeningKind::Window {
                            format!(
                                " · peitoril {}",
                                crate::domain::annotations::centimetres(frame.sill)
                            )
                        } else {
                            String::new()
                        };
                        property_value(
                            ui,
                            &format!("{} · {} un.{sill}", frame.kind.label(), frame.ids.len()),
                        );
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                hint(
                    ui,
                    Icon::Info,
                    "Largura/altura em cm. Esquadrias iguais têm o mesmo código.",
                    ACCENT,
                );
            });
        }

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
