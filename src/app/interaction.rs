use super::{
    CreationState, FloorPlanApp, ItemDrag, Selection, Tool,
    canvas::{
        self, PaintOptions, Preview, ViewTransform,
        overlay::{self, SnapKind},
    },
};
use crate::domain::geometry::Point;
use eframe::egui::{self, Key, PointerButton, Sense};

impl FloorPlanApp {
    fn snap(&self, point: Point, start: Option<Point>) -> Point {
        let mut result = if self.snap_enabled {
            self.magnetic_snap(point, start)
        } else {
            point
        };
        if self.ortho_enabled
            && self.tool == Tool::Wall
            && let Some(start) = start
        {
            if (result.x - start.x).abs() >= (result.y - start.y).abs() {
                result.y = start.y;
            } else {
                result.x = start.x;
            }
        }
        result
    }

    fn magnetic_snap(&self, point: Point, start: Option<Point>) -> Point {
        let mut result = point.snapped_to(0.25);
        let snap_threshold = (12.0 / self.scale).clamp(0.12, 0.40);
        let mut closest = snap_threshold;
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
            if (result.x - start.x).abs() < snap_threshold {
                result.x = start.x;
            }
            if (result.y - start.y).abs() < snap_threshold {
                result.y = start.y;
            }
        }
        result
    }

    fn snap_kind(&self, point: Point) -> Option<SnapKind> {
        if !self.snap_enabled {
            return None;
        }
        let on_endpoint = self.project.walls.iter().any(|wall| {
            wall.start.distance(point) < 1e-4 || wall.end.distance(point) < 1e-4
        });
        Some(if on_endpoint {
            SnapKind::Endpoint
        } else {
            SnapKind::Grid
        })
    }

    /// The element a click at `point` selects: openings first, then walls,
    /// rooms and the terrain.
    fn pick(&self, point: Point) -> Option<Selection> {
        self.project
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
            })
    }

    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        self.prepare_canvas_view(rect);
        let navigation = overlay::navigation_rect(rect);
        // False while a menu, a dialog or a floating button covers the plan:
        // the pointer then belongs to them, not to the drawing below.
        let pointer_free = response.contains_pointer();
        let on_canvas = |position: egui::Pos2| {
            pointer_free && rect.contains(position) && !navigation.contains(position)
        };

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
        let panning = middle_down
            && pointer_free
            && rect.contains(ui.input(|input| input.pointer.hover_pos().unwrap_or_default()));
        if panning {
            self.pan += pointer_delta;
        }
        let view = ViewTransform {
            rect,
            scale: self.scale,
            pan: self.pan,
        };
        let pointer = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|position| on_canvas(*position));
        let world = pointer.map(|position| view.to_world(position));
        self.cursor_world = world;
        let snapped = world.map(|point| {
            let point = self.snap(point, self.draft_start());
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

        if let Some((start, end)) = self.pending_room {
            // Keep the drawn rectangle on screen while the room is named.
            canvas::paint(
                &painter,
                view,
                &self.project,
                PaintOptions {
                    selection: self.selection,
                    preview: Some(Preview::Room(start, end)),
                    show_grid: self.show_grid,
                    ..Default::default()
                },
            );
            self.paint_view_aids(&painter, view);
            self.canvas_navigation(ui, navigation);
            return;
        }

        let hovered_item = if self.item_drag.is_some() {
            None
        } else {
            world.and_then(|point| self.pick(point))
        };

        if self.tool == Tool::Move && hovered_item.is_some() && !panning {
            ui.ctx().set_cursor_icon(if self.item_drag.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        if self.tool == Tool::Move
            && let Some(drag) = self.item_drag
            && drag.moved
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        if self.tool == Tool::Move {
            let pressed = ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    } if on_canvas(*pos) => Some(*pos),
                    _ => None,
                })
            });
            if let Some(position) = pressed {
                self.begin_item_drag(view.to_world(position));
            }
            let primary_down = ui.input(|input| input.pointer.primary_down());
            if primary_down && let Some(point) = world {
                self.drag_item_to(point);
            }
        }

        let creation_tool = matches!(self.tool, Tool::Wall | Tool::Room | Tool::Terrain);
        if creation_tool
            && response.drag_started_by(PointerButton::Primary)
            && self.creation_state == CreationState::Idle
            && let Some(point) = snapped
        {
            self.creation_state = CreationState::Dragging { start: point };
        }

        let mut drag_finished_this_frame = false;

        if ui.input(|input| input.pointer.any_released()) {
            self.commit_item_drag();
            if let Some(kind) = self.opening_drag.take()
                && let Some(point) = world
            {
                self.add_opening_at(point, kind);
            }
            if let CreationState::Dragging { start } = self.creation_state {
                drag_finished_this_frame = true;
                if let Some(point) = snapped {
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
                        self.creation_state = CreationState::Idle;
                    } else {
                        self.creation_state = CreationState::WaitingSecondPoint { start };
                        self.message =
                            "Agora clique no segundo ponto para concluir. Esc cancela.".to_owned();
                    }
                } else {
                    self.creation_state = CreationState::Idle;
                }
            }
        }

        if !drag_finished_this_frame
            && response.clicked_by(PointerButton::Primary)
            && let Some(point) = snapped
        {
            match self.tool {
                Tool::Wall | Tool::Room | Tool::Terrain => match self.creation_state {
                    CreationState::Idle => {
                        self.creation_state = CreationState::WaitingSecondPoint { start: point };
                        self.message =
                            "Agora clique no segundo ponto para concluir. Esc cancela.".to_owned();
                    }
                    CreationState::WaitingSecondPoint { start } => {
                        self.complete_creation(start, point);
                        self.creation_state = CreationState::Idle;
                    }
                    CreationState::Dragging { .. } => {}
                },
                Tool::Opening(kind) => self.add_opening_at(point, kind),
                Tool::Move => {}
            }
        }

        if ui.input(|input| input.key_pressed(Key::Escape)) {
            self.cancel_creation();
        }

        let preview = match (self.draft_start(), snapped, self.tool) {
            (Some(start), Some(end), Tool::Wall) => Some(Preview::Wall(start, end)),
            (Some(start), Some(end), Tool::Room) => Some(Preview::Room(start, end)),
            (Some(start), Some(end), Tool::Terrain) => Some(Preview::Terrain(start, end)),
            _ => None,
        };
        // Pre-highlight what a click would act on: the element to select,
        // or the wall that will receive a door or window.
        let opening_target = matches!(self.tool, Tool::Opening(_)) || self.opening_drag.is_some();
        let hover = if opening_target {
            self.opening_point(world, snapped)
                .and_then(|point| self.project.nearest_wall(point, 0.35))
                .map(|(id, _)| Selection::Wall(id))
        } else if self.tool == Tool::Move {
            hovered_item
        } else {
            None
        }
        .or(self.browser_hover);
        canvas::paint(
            &painter,
            view,
            &self.project,
            PaintOptions {
                selection: self.selection,
                hover,
                preview,
                show_grid: self.show_grid,
            },
        );
        self.paint_view_aids(&painter, view);

        if let Some(position) = pointer
            && !panning
        {
            let drawing = matches!(self.tool, Tool::Wall | Tool::Room | Tool::Terrain)
                || opening_target;
            if drawing {
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
                overlay::paint_crosshair(&painter, position);
            }
            if matches!(self.tool, Tool::Wall | Tool::Room | Tool::Terrain)
                && let Some(point) = snapped
                && let Some(kind) = self.snap_kind(point)
            {
                overlay::paint_snap_marker(&painter, view.to_screen(point), kind);
            }
            if let Some(kind) = self.opening_drag {
                let label = format!("Soltar {} na parede", kind.label().to_lowercase());
                let galley = painter.layout_no_wrap(
                    label,
                    crate::app::theme::regular(12.0),
                    crate::app::theme::ACCENT_TEXT,
                );
                let badge = egui::Rect::from_min_size(
                    position + egui::vec2(16.0, -30.0),
                    galley.size() + egui::vec2(16.0, 8.0),
                );
                painter.rect(
                    badge,
                    5.0,
                    crate::app::theme::PANEL,
                    egui::Stroke::new(1.0, crate::app::theme::ACCENT_RING),
                    egui::StrokeKind::Inside,
                );
                painter.galley(
                    badge.min + egui::vec2(8.0, 4.0),
                    galley,
                    crate::app::theme::ACCENT_TEXT,
                );
            }
        }

        self.canvas_navigation(ui, navigation);
    }

    /// Picks the element at `point` and gets ready to drag it.
    pub(super) fn begin_item_drag(&mut self, point: Point) {
        self.selection = self.pick(point);
        self.item_drag = self.selection.map(|selection| {
            // An opening keeps the spot where it was grabbed instead of
            // jumping so that its centre sits under the pointer.
            let grab_offset = match selection {
                Selection::Opening(id) => self
                    .opening_center(id)
                    .map(|center| Point::new(point.x - center.x, point.y - center.y))
                    .unwrap_or(Point::new(0.0, 0.0)),
                _ => Point::new(0.0, 0.0),
            };
            ItemDrag {
                selection,
                last: self.drag_position(selection, point),
                grab_offset,
                moved: false,
            }
        });
        self.drag_initial_project = self.item_drag.as_ref().map(|_| self.project.clone());
        self.message = Self::selection_message(self.selection).to_owned();
        self.sync_inspector_fields();
    }

    /// Walls, rooms and the terrain move in grid steps when snapping is on;
    /// openings slide freely along their wall.
    fn drag_position(&self, selection: Selection, point: Point) -> Point {
        if self.snap_enabled && !matches!(selection, Selection::Opening(_)) {
            point.snapped_to(0.25)
        } else {
            point
        }
    }

    /// Moves the element being dragged as the pointer reaches `point`.
    /// Nothing changes until the pointer actually moves, so a click only
    /// selects.
    pub(super) fn drag_item_to(&mut self, point: Point) {
        let Some(drag) = self.item_drag else {
            return;
        };
        let current = self.drag_position(drag.selection, point);
        let delta = Point::new(current.x - drag.last.x, current.y - drag.last.y);
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }
        match drag.selection {
            Selection::Wall(id) => self.project.move_wall(id, delta),
            Selection::Room(id) => {
                if !self.project.move_room(id, delta) {
                    self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                }
            }
            Selection::Terrain(id) => self.project.move_terrain(id, delta),
            Selection::Opening(id) => {
                let target = Point::new(
                    point.x - drag.grab_offset.x,
                    point.y - drag.grab_offset.y,
                );
                self.project.move_opening_along_wall(id, target);
            }
        }
        if let Some(drag) = &mut self.item_drag {
            drag.last = current;
            drag.moved = true;
        }
    }

    /// The point a door or window will be placed at: where it is dropped
    /// when dragged from the ribbon, or the snapped click with the tool.
    /// The highlighted wall must be the one that point lands on.
    fn opening_point(&self, world: Option<Point>, snapped: Option<Point>) -> Option<Point> {
        if self.opening_drag.is_some() {
            world
        } else {
            snapped
        }
    }

    fn paint_view_aids(&self, painter: &egui::Painter, view: ViewTransform) {
        self.paint_view_label(painter, view.rect);
        overlay::paint_scale_bar(painter, view);
        overlay::paint_north_arrow(painter, view.rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ortho_keeps_walls_horizontal_or_vertical() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.set_tool(Tool::Wall);
        app.ortho_enabled = true;
        app.snap_enabled = false;
        let start = Point::new(0.0, 0.0);
        let mostly_across = app.snap(Point::new(3.0, 0.9), Some(start));
        assert_eq!(mostly_across, Point::new(3.0, 0.0));
        let mostly_down = app.snap(Point::new(0.7, -2.6), Some(start));
        assert_eq!(mostly_down, Point::new(0.0, -2.6));

        app.ortho_enabled = false;
        assert_eq!(app.snap(Point::new(3.0, 0.9), Some(start)), Point::new(3.0, 0.9));
    }

    #[test]
    fn opening_highlight_uses_the_point_the_opening_will_use() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.set_tool(Tool::Opening(crate::domain::model::OpeningKind::Door));
        let raw = Point::new(1.1, 0.3);
        let snapped = Point::new(1.0, 0.25);
        assert_eq!(app.opening_point(Some(raw), Some(snapped)), Some(snapped));
        app.opening_drag = Some(crate::domain::model::OpeningKind::Door);
        assert_eq!(app.opening_point(Some(raw), Some(snapped)), Some(raw));
    }

    #[test]
    fn clicking_an_opening_off_centre_selects_it_without_moving_it() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
            .unwrap();
        let door = app
            .project
            .add_opening(wall, crate::domain::model::OpeningKind::Door, 0.5)
            .unwrap();
        let before = app.project.clone();

        // Grab the door 0,3 m right of its centre (3, 0) and release.
        let grab = Point::new(3.3, 0.05);
        app.begin_item_drag(grab);
        assert_eq!(app.selection, Some(Selection::Opening(door)));
        app.drag_item_to(grab);
        app.commit_item_drag();
        assert_eq!(app.project, before);
        assert!(!app.dirty && !app.history.can_undo());

        // Dragging 1 m keeps the grab offset: the centre moves 1 m, to 4 m.
        app.begin_item_drag(grab);
        app.drag_item_to(Point::new(4.3, 0.05));
        app.commit_item_drag();
        let center = app.opening_center(door).unwrap();
        assert!((center.x - 4.0).abs() < 1e-3, "{center:?}");
        assert!(app.dirty && app.history.can_undo());
    }

    #[test]
    fn snap_marker_tells_wall_ends_from_grid_points() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
        assert_eq!(app.snap_kind(Point::new(3.0, 0.0)), Some(SnapKind::Endpoint));
        assert_eq!(app.snap_kind(Point::new(1.25, 2.0)), Some(SnapKind::Grid));
        app.snap_enabled = false;
        assert_eq!(app.snap_kind(Point::new(3.0, 0.0)), None);
    }

    #[test]
    fn pick_prefers_openings_then_walls_then_rooms() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala");
        let wall = app.project.rooms[0].wall_ids[0];
        let door = app
            .project
            .add_opening(wall, crate::domain::model::OpeningKind::Door, 0.5)
            .unwrap();
        let door_wall = app.project.walls.iter().find(|w| w.id == wall).unwrap().clone();
        let middle = Point::new(
            (door_wall.start.x + door_wall.end.x) / 2.0,
            (door_wall.start.y + door_wall.end.y) / 2.0,
        );
        assert_eq!(app.pick(middle), Some(Selection::Opening(door)));
        assert!(matches!(app.pick(Point::new(2.0, 2.0)), Some(Selection::Room(_))));
        assert_eq!(app.pick(Point::new(40.0, 40.0)), None);
    }
}
