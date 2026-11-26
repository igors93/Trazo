//! Creating elements on the canvas: the active tool, the two-click
//! creation flow, library placements, openings and typed numeric input.

use super::{CreationState, DocDialog, FloorPlanApp, Selection, Tool, tools};
use crate::domain::{fixtures::FixtureKind, geometry::Point, model::OpeningKind};

impl FloorPlanApp {
    #[allow(dead_code)]
    pub fn creation_state(&self) -> CreationState {
        self.creation_state
    }

    pub fn draft_start(&self) -> Option<Point> {
        match self.creation_state {
            // A free-shape room continues from its last corner.
            // A modify tool measures from its base point.
            CreationState::Idle => self
                .polygon_draft
                .last()
                .or(self.draw_points.last())
                .copied()
                .or(self.modify_anchor),
            CreationState::WaitingSecondPoint { start } => Some(start),
            CreationState::Dragging { start } => Some(start),
        }
    }

    pub fn cancel_creation(&mut self) {
        self.markup_state.cancel_draft();
        self.creation_state = CreationState::Idle;
        self.polygon_draft.clear();
        self.draw_points.clear();
        self.draft_input.clear();
        self.modify_anchor = None;
        self.modify_target = None;
        self.constraint_pick = None;
    }

    /// Ends an element drag in progress, keeping what it moved as one undo
    /// step. Whatever interrupts a drag (a tool key, Delete) goes through
    /// here, so a move is never left outside the history and unsaved.
    pub(super) fn commit_item_drag(&mut self) {
        // A selection box cut short selects nothing.
        self.box_select = None;
        let drag = self.item_drag.take();
        let vertex = self.vertex_drag.take();
        let moved = drag.is_some_and(|d| d.moved) || vertex.is_some_and(|v| v.moved);
        if let Some(initial) = self.drag_initial_project.take()
            && moved
        {
            self.record_change(initial);
            self.sync_inspector_fields();
        }
    }

    pub(super) fn set_tool(&mut self, tool: Tool) {
        self.sync_markup_context();
        if matches!(tool, Tool::Markup(_)) && !matches!(self.tool, Tool::Markup(_)) {
            self.markup_state.context_elements = self.selected();
        }
        self.commit_item_drag();
        self.tool = tool;
        self.fixture_preset = None;
        self.cancel_creation();
        self.markup_state.cancel_draft();
        // Moving, copying, turning and mirroring act on the selection.
        if !matches!(tool, Tool::Modify(kind) if kind.uses_selection()) {
            self.clear_selection();
        }
        self.message = match tool {
            Tool::Wall => "Clique em dois pontos para desenhar uma parede.",
            Tool::Room => "Clique em dois cantos opostos e dê um nome ao cômodo.",
            Tool::RoomPolygon => {
                "Clique em cada canto do cômodo (em L, U ou livre); feche no primeiro canto ou com Enter."
            }
            Tool::Terrain => {
                "Clique em dois cantos ou digite largura e comprimento para criar o terreno."
            }
            Tool::Move => "Clique para selecionar. Arraste parede, cômodo ou terreno para mover.",
            Tool::Opening(OpeningKind::Door) => "Clique em uma parede para colocar uma porta.",
            Tool::Opening(OpeningKind::Gate) => {
                "Clique no muro ou na parede da frente para colocar o portão (de correr, 3,00 m)."
            }
            Tool::Opening(OpeningKind::Window) => "Clique em uma parede para colocar uma janela.",
            Tool::Opening(OpeningKind::SaloonDoor) => {
                "Clique em uma parede para colocar uma porta vai-e-vem."
            }
            Tool::Opening(OpeningKind::Gap) => {
                "Clique em uma parede para abrir um vão livre; ajuste a largura no painel."
            }
            Tool::Fixture(kind) if !kind.mounts_on_wall() => {
                "Clique para escolher o ponto de inserção; G gira o objeto."
            }
            Tool::Fixture(_) => {
                "Clique para posicionar. Perto de uma parede o objeto encosta nela; G gira."
            }
            Tool::Draw(kind) => kind.hint(),
            Tool::Markup(None) => "Clique em uma marcação para selecionar; arraste para mover. Abra Lista e comentários para editar.",
            Tool::Markup(Some(crate::domain::markups::MarkupKind::Freehand)) => "Arraste sobre a planta para desenhar uma marcação. Esc cancela.",
            Tool::Markup(Some(_)) => "Clique em dois pontos ou arraste para criar uma marcação. Esc cancela.",
            Tool::Modify(kind) => kind.hint(),
            Tool::Constrain(kind) => kind.hint(),
            Tool::Measure => {
                "Clique em dois pontos para medir. Encaixa em cantos, faces e batentes; nada é alterado."
            }
        }
        .to_owned();
    }

    pub(super) fn fixture_tool_size(&self, kind: FixtureKind) -> (f64, f64) {
        self.fixture_preset
            .as_ref()
            .filter(|item| item.base_kind == Some(kind))
            .map(|item| (item.width_m, item.depth_m))
            .unwrap_or_else(|| kind.default_size())
    }

    pub(super) fn start_library_placement(&mut self, item: crate::domain::library::LibraryItem) {
        let Some(kind) = item.base_kind else {
            return;
        };
        if !crate::domain::fixtures::valid_size(kind, item.width_m, item.depth_m) {
            self.message = "O modelo da biblioteca tem dimensões inválidas.".to_owned();
            return;
        }
        self.set_tool(Tool::Fixture(kind));
        let dimensions = format!("{:.2} × {:.2} m", item.width_m, item.depth_m).replace('.', ",");
        self.message = format!(
            "{} · {dimensions}. Clique para posicionar; G gira.",
            item.name
        );
        self.fixture_preset = Some(item);
        self.active_doc_dialog = DocDialog::None;
    }

    pub(super) fn complete_creation(&mut self, start: Point, end: Point) {
        if matches!(self.tool, Tool::Terrain | Tool::Wall | Tool::Room)
            && !self.discipline.allows_architecture_edit()
        {
            self.message = self.architecture_edit_blocked_message();
            return;
        }
        match self.tool {
            Tool::Terrain => {
                let previous = self.project.clone();
                if let Some(id) = self.project.set_terrain(start, end) {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.set_selection(Some(Selection::Terrain(id)));
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
                let (wall_start, wall_end) = self.wall_reference.offset_segment(
                    start,
                    end,
                    self.project.masonry.thickness(),
                );
                let previous = self.project.clone();
                if self.project.add_wall(wall_start, wall_end).is_some() {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.message = if self.wall_reference
                        == crate::domain::wall_reference::WallReference::Center
                    {
                        "Parede criada.".to_owned()
                    } else {
                        format!(
                            "Parede criada ({}).",
                            self.wall_reference.label().to_lowercase()
                        )
                    };
                } else {
                    self.message =
                        "A parede deve caber dentro do terreno e ter pelo menos 0,25 m.".to_owned();
                }
            }
            Tool::Room => {
                if (start.x - end.x).abs() >= 0.5 && (start.y - end.y).abs() >= 0.5 {
                    let Some((min, max)) = self.wall_reference.offset_rect(
                        start,
                        end,
                        self.project.masonry.thickness(),
                    ) else {
                        self.message =
                            "Dimensões do cômodo muito pequenas para a referência escolhida."
                                .to_owned();
                        return;
                    };
                    self.pending_room = Some(vec![
                        min,
                        Point::new(max.x, min.y),
                        max,
                        Point::new(min.x, max.y),
                    ]);
                    self.room_name.clear();
                    self.room_dialog_focus = true;
                    self.message = "Digite o nome do cômodo para concluir.".to_owned();
                } else {
                    self.message = "Dimensões do cômodo muito pequenas. Mínimo 0,50 m.".to_owned();
                }
            }
            Tool::Measure => {
                self.measurements.push((start, end));
                let (dx, dy) = ((end.x - start.x).abs(), (end.y - start.y).abs());
                self.message = format!(
                    "Medida: {}{}. Meça de novo ou Esc para limpar.",
                    crate::domain::geometry::format_meters(start.distance(end)),
                    if dx > 0.005 && dy > 0.005 {
                        format!(" (Δx {:.2} · Δy {:.2})", dx, dy).replace('.', ",")
                    } else {
                        String::new()
                    }
                );
            }
            _ => {}
        }
    }

    pub(super) fn add_opening_at(&mut self, point: Point, kind: OpeningKind) {
        if !self.discipline.allows_architecture_edit() {
            self.message = self.architecture_edit_blocked_message();
            return;
        }
        if let Some((wall_id, t)) = self.project.nearest_wall(point, 0.35) {
            let previous = self.project.clone();
            if let Some(id) = self.project.add_opening(wall_id, kind, t) {
                // Doors open towards the side of the wall that was clicked.
                self.project.swing_towards(id, point);
                if !self.record_change(previous) {
                    return;
                }
                self.message = if kind.is_door() {
                    format!(
                        "{} adicionada, abrindo para o lado clicado. I inverte o lado; Shift+I, a dobradiça.",
                        kind.label()
                    )
                } else {
                    format!("{} adicionada à parede.", kind.label())
                };
            } else {
                self.message = "Sem espaço suficiente nesta parede.".to_owned();
            }
        } else {
            self.message = "Solte o elemento sobre uma parede.".to_owned();
        }
    }

    pub(crate) fn supports_numeric_input(&self) -> bool {
        matches!(
            self.tool,
            Tool::Wall
                | Tool::Room
                | Tool::RoomPolygon
                | Tool::Terrain
                | Tool::Measure
                | Tool::Draw(_)
                | Tool::Modify(
                    tools::ModifyKind::Move
                        | tools::ModifyKind::Copy
                        | tools::ModifyKind::Rotate
                        | tools::ModifyKind::Mirror
                        | tools::ModifyKind::Offset
                )
        )
    }

    pub(crate) fn apply_canvas_point(&mut self, point: Point) {
        if matches!(
            self.tool,
            Tool::Wall | Tool::Room | Tool::Terrain | Tool::RoomPolygon | Tool::Draw(_)
        ) && !self.discipline.allows_architecture_edit()
        {
            self.creation_state = CreationState::Idle;
            self.message = self.architecture_edit_blocked_message();
            return;
        }
        match self.tool {
            Tool::Wall | Tool::Room | Tool::Terrain | Tool::Measure => match self.creation_state {
                CreationState::Idle => {
                    self.creation_state = CreationState::WaitingSecondPoint { start: point };
                    self.message =
                        "Clique no segundo ponto ou digite a medida (ex.: 3,47 · 3,5<45 · 3x4) e Enter."
                            .to_owned();
                }
                CreationState::WaitingSecondPoint { start } | CreationState::Dragging { start } => {
                    self.complete_creation(start, point);
                    self.creation_state = CreationState::Idle;
                }
            },
            Tool::RoomPolygon => self.add_polygon_point(point),
            Tool::Draw(kind) => self.add_draw_point(kind, point),
            Tool::Modify(kind) => self.modify_click(kind, point),
            _ => {}
        }
    }

    pub(super) fn commit_numeric_input(&mut self) {
        let raw = self.draft_input.trim().to_owned();
        // An angle to turn by, or the offset distance.
        if self.commit_modify_input(&raw) {
            return;
        }
        let Some(spec) = tools::numeric_input::parse_numeric_input(&raw) else {
            self.message = format!(
                "Entrada \"{raw}\" inválida. Use ex.: 3,47 · 3,5<45 · 3,5x4,0 · @2;1 · 1,5;2,0"
            );
            return;
        };
        let start = self.draft_start();
        let aim = self.draft_aim.or(self.cursor_world);
        let Some(point) = spec.resolve(start, aim) else {
            self.message =
                "Defina o primeiro ponto (ou digite coordenadas X;Y) antes da distância ou ângulo."
                    .to_owned();
            return;
        };
        self.draft_input.clear();
        self.apply_canvas_point(point);
    }
}
