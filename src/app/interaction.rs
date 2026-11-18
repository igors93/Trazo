use super::{
    FloorPlanApp, ItemDrag, Selection, Tool,
    canvas::{self, Preview, ViewTransform},
};
use crate::domain::geometry::Point;
use eframe::egui::{self, Color32, PointerButton, Sense};

impl FloorPlanApp {
    fn snap(&self, point: Point, start: Option<Point>) -> Point {
        if !self.snap_enabled {
            return point;
        }
        let mut result = point.snapped_to(self.snap_step());
        let mut closest = (10.0 / self.scale).clamp(0.18, 2.0);
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

    fn snap_step(&self) -> f32 {
        if self.scale < 8.0 {
            5.0
        } else if self.scale < 20.0 {
            1.0
        } else {
            0.25
        }
    }

    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        self.prepare_canvas_view(rect);
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.0
                && let Some(position) = ui.input(|input| input.pointer.hover_pos())
            {
                let before = ViewTransform {
                    rect,
                    scale: self.scale,
                    pan: self.pan,
                }
                .to_world(position);
                self.scale = (self.scale * (scroll * 0.0015).exp()).clamp(3.0, 240.0);
                self.pan = position
                    - rect.center()
                    - egui::vec2(before.x * self.scale, before.y * self.scale);
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
        let snapped = world.map(|point| {
            let point = self.snap(point, self.draft_start);
            if matches!(self.tool, Tool::Room | Tool::Wall)
                && let Some(terrain) = &self.project.terrain
            {
                return Point::new(
                    point.x.clamp(terrain.min.x, terrain.max.x),
                    point.y.clamp(terrain.min.y, terrain.max.y),
                );
            }
            point
        });

        if self.pending_room.is_some() {
            canvas::paint(&painter, view, &self.project, self.selection, None);
            return;
        }

        if self.tool == Tool::Move
            && let Some(point) = world
            && (self
                .project
                .nearest_opening(point, 14.0 / self.scale)
                .is_some()
                || self
                    .project
                    .nearest_wall(point, 12.0 / self.scale)
                    .is_some()
                || self.project.room_at(point).is_some()
                || self
                    .project
                    .terrain
                    .as_ref()
                    .is_some_and(|t| t.contains(point)))
        {
            ui.ctx().set_cursor_icon(if self.item_drag.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
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
                    .nearest_opening(point, 14.0 / self.scale)
                    .map(Selection::Opening)
                    .or_else(|| {
                        self.project
                            .nearest_wall(point, 12.0 / self.scale)
                            .map(|(id, _)| Selection::Wall(id))
                    })
                    .or_else(|| self.project.room_at(point).map(Selection::Room))
                    .or_else(|| {
                        self.project
                            .terrain
                            .as_ref()
                            .filter(|terrain| terrain.contains(point))
                            .map(|terrain| Selection::Terrain(terrain.id))
                    });
                self.item_drag = self.selection.map(|selection| ItemDrag {
                    selection,
                    last: point.snapped_to(0.25),
                    moved: false,
                });
                self.drag_initial_project = self.item_drag.as_ref().map(|_| self.project.clone());
                self.message = match self.selection {
                    Some(Selection::Opening(_)) => {
                        "Abertura selecionada. Arraste ao longo da parede ou altere a largura no painel."
                    }
                    Some(Selection::Wall(_)) => {
                        "Parede selecionada. Arraste para mover ou altere o comprimento no painel."
                    }
                    Some(Selection::Room(_)) => {
                        "Cômodo selecionado. Arraste para mover ou altere as medidas no painel."
                    }
                    Some(Selection::Terrain(_)) => {
                        "Terreno selecionado. Ajuste as medidas ou arraste para mover."
                    }
                    None => "Clique em um elemento para selecionar.",
                }
                .to_owned();
                self.sync_inspector_fields();
            }
            let primary_down = ui.input(|input| input.pointer.primary_down());
            if primary_down && let (Some(point), Some(drag)) = (world, self.item_drag) {
                let current = point.snapped_to(0.25);
                let delta = Point::new(current.x - drag.last.x, current.y - drag.last.y);
                if delta.x != 0.0
                    || delta.y != 0.0
                    || matches!(drag.selection, Selection::Opening(_))
                {
                    match drag.selection {
                        Selection::Wall(id) => self.project.move_wall(id, delta),
                        Selection::Room(id) => {
                            if !self.project.move_room(id, delta) {
                                self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                            }
                        }
                        Selection::Terrain(id) => self.project.move_terrain(id, delta),
                        Selection::Opening(id) => {
                            self.project.move_opening_along_wall(id, point);
                        }
                    }
                    if let Some(drag) = &mut self.item_drag {
                        drag.last = current;
                        drag.moved = true;
                    }
                }
            }
        }

        let creation_tool = matches!(self.tool, Tool::Wall | Tool::Room | Tool::Terrain);
        if creation_tool {
            let primary_pressed = response.drag_started_by(PointerButton::Primary)
                || ui.input(|input| {
                    input.events.iter().any(|event| match event {
                        egui::Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed: true,
                            ..
                        } => rect.contains(*pos),
                        _ => false,
                    })
                });
            if primary_pressed
                && self.draft_start.is_none()
                && let Some(point) = snapped
            {
                self.draft_start = Some(point);
                self.creation_is_drag = true;
            }
        }

        if ui.input(|input| input.pointer.any_released()) {
            if let Some(initial) = self.drag_initial_project.take() {
                if let Some(drag) = self.item_drag.take()
                    && drag.moved
                    && initial != self.project
                {
                    self.record_change(initial);
                }
            } else {
                self.item_drag = None;
            }
            if let Some(kind) = self.opening_drag.take()
                && let Some(point) = world
            {
                self.add_opening_at(point, kind);
            }
            if self.creation_is_drag {
                self.creation_is_drag = false;
                if let (Some(start), Some(point)) = (self.draft_start, snapped) {
                    let has_dragged = match self.tool {
                        Tool::Wall => start.distance(point) >= 0.25,
                        Tool::Room => {
                            (start.x - point.x).abs() >= 0.4 && (start.y - point.y).abs() >= 0.4
                        }
                        Tool::Terrain => {
                            (start.x - point.x).abs() >= 0.25 && (start.y - point.y).abs() >= 0.25
                        }
                        _ => false,
                    };
                    if has_dragged {
                        self.complete_creation(start, point);
                        self.draft_start = None;
                    } else {
                        self.message =
                            "Agora clique no segundo ponto para concluir. Esc cancela.".to_owned();
                    }
                }
            }
        }

        if response.clicked_by(PointerButton::Primary)
            && let Some(point) = snapped
        {
            match self.tool {
                Tool::Wall | Tool::Room | Tool::Terrain => {
                    if !self.creation_is_drag {
                        if let Some(start) = self.draft_start.take() {
                            self.complete_creation(start, point);
                        } else {
                            self.draft_start = Some(point);
                            self.message =
                                "Agora clique no segundo ponto para concluir. Esc cancela."
                                    .to_owned();
                        }
                    }
                }
                Tool::Opening(kind) => self.add_opening_at(point, kind),
                Tool::Move => {}
            }
        }

        let preview = match (self.draft_start, snapped, self.tool) {
            (Some(start), Some(end), Tool::Wall) => Some(Preview::Wall(start, end)),
            (Some(start), Some(end), Tool::Room) => Some(Preview::Room(start, end)),
            (Some(start), Some(end), Tool::Terrain) => Some(Preview::Terrain(start, end)),
            _ => None,
        };
        canvas::paint(&painter, view, &self.project, self.selection, preview);
        if let Some(position) = pointer {
            if matches!(self.tool, Tool::Wall | Tool::Room | Tool::Terrain)
                && let Some(point) = snapped
            {
                painter.circle_filled(view.to_screen(point), 4.5, Color32::from_rgb(28, 169, 151));
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
}
