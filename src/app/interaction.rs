use super::{
    CreationState, FloorPlanApp, ItemDrag, Selection, Tool,
    canvas::{
        self, PaintOptions, Preview, ViewTransform,
        overlay::{self, SnapKind},
    },
};
use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::Point,
};
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
        if self.modulation_enabled
            && let Some(start) = start
            && !self.is_wall_end(result)
        {
            result = self.modular_point(start, result);
        }
        result
    }

    fn is_wall_end(&self, point: Point) -> bool {
        self.project
            .walls
            .iter()
            .any(|wall| wall.start.distance(point) < 1e-4 || wall.end.distance(point) < 1e-4)
    }

    /// Rounds what is being drawn to whole half blocks of the active
    /// material: a wall's length, or a room's width and length.
    pub(super) fn modular_point(&self, start: Point, point: Point) -> Point {
        let material = &self.project.masonry;
        match self.tool {
            Tool::Wall => {
                let length = start.distance(point);
                if length < 1e-4 {
                    return point;
                }
                let modular = material.modular_length(length);
                Point::new(
                    start.x + (point.x - start.x) / length * modular,
                    start.y + (point.y - start.y) / length * modular,
                )
            }
            Tool::Room => {
                let side = |from: f32, to: f32| {
                    let span = to - from;
                    from + span.signum() * material.modular_length(span.abs())
                };
                Point::new(side(start.x, point.x), side(start.y, point.y))
            }
            _ => point,
        }
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
            .or_else(|| self.project.fixture_at(point).map(Selection::Fixture))
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
            if self.tool == Tool::Measure {
                return self.measure_snap(point, self.draft_start()).0;
            }
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
                    dimensions: self.show_dimensions,
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

        let creation_tool = matches!(
            self.tool,
            Tool::Wall | Tool::Room | Tool::Terrain | Tool::Measure
        );
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
                        Tool::Measure => start.distance(point) >= 0.03,
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
                Tool::Wall | Tool::Room | Tool::Terrain | Tool::Measure => match self.creation_state {
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
                Tool::Fixture(kind) => {
                    if let Some(raw) = world {
                        self.place_fixture(kind, raw);
                    }
                }
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
        let ghost = match (self.tool, world) {
            (Tool::Fixture(kind), Some(point)) => Some(self.fixture_placement(kind, point)),
            _ => None,
        };
        canvas::paint(
            &painter,
            view,
            &self.project,
            PaintOptions {
                selection: self.selection,
                hover,
                preview,
                show_grid: self.show_grid,
                ghost,
                dimensions: self.show_dimensions,
            },
        );
        self.paint_view_aids(&painter, view);
        if self.tool == Tool::Measure
            && let (Some(start), Some(end)) = (self.draft_start(), snapped)
        {
            canvas::paint_measurement(&painter, view, start, end);
        }

        if let Some(position) = pointer
            && !panning
        {
            let drawing = matches!(
                self.tool,
                Tool::Wall | Tool::Room | Tool::Terrain | Tool::Fixture(_) | Tool::Measure
            ) || opening_target;
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
            if self.tool == Tool::Measure
                && let Some(raw) = world
                && let (point, Some(kind)) = self.measure_snap(raw, self.draft_start())
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

        if response.secondary_clicked() {
            let point = response
                .interact_pointer_pos()
                .filter(|position| on_canvas(*position))
                .map(|position| view.to_world(position));
            self.context_click(point);
        }
        response.context_menu(|ui| self.canvas_context_menu(ui));
    }

    /// A right click: it ends a drawing in progress and selects the element
    /// under the pointer, so the context menu acts on it.
    pub(super) fn context_click(&mut self, point: Option<Point>) {
        if self.draft_start().is_some() {
            self.cancel_creation();
            self.message = "Desenho cancelado.".to_owned();
        }
        if let Some(selection) = point.and_then(|point| self.pick(point)) {
            self.select(selection);
        }
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

    /// Walls, rooms and the terrain move in grid steps when snapping is on,
    /// fixtures in 5 cm steps; openings slide freely along their wall.
    fn drag_position(&self, selection: Selection, point: Point) -> Point {
        match selection {
            _ if !self.snap_enabled => point,
            Selection::Opening(_) => point,
            Selection::Fixture(_) => point.snapped_to(0.05),
            _ => point.snapped_to(0.25),
        }
    }

    /// Where a fixture of `kind` lands for the pointer at `point`. Near a
    /// straight wall it turns its back to the wall and stands against the
    /// wall's face, half a wall thickness off its centre line, so counters
    /// and sinks sit where the bricks end.
    pub(super) fn fixture_placement(&self, kind: FixtureKind, point: Point) -> Fixture {
        let (width, depth) = kind.default_size();
        self.place_against_wall(kind, point, width, depth, self.fixture_turns)
    }

    /// `fixture_placement` for a given size and free rotation.
    pub(super) fn place_against_wall(
        &self,
        kind: FixtureKind,
        point: Point,
        width: f32,
        depth: f32,
        turns: u8,
    ) -> Fixture {
        let step = |value: f32| {
            if self.snap_enabled {
                (value / 0.05).round() * 0.05
            } else {
                value
            }
        };
        let mut fixture = Fixture {
            id: 0,
            kind,
            center: Point::new(step(point.x), step(point.y)),
            width,
            depth,
            quarter_turns: turns % 4,
        };
        let wall = self
            .project
            .nearest_wall(point, 0.8)
            .and_then(|(id, _)| self.project.walls.iter().find(|wall| wall.id == id));
        if let Some(wall) = wall {
            let offset = wall.thickness() / 2.0 + depth / 2.0;
            if (wall.start.y - wall.end.y).abs() < 1e-3 {
                let below = point.y > wall.start.y;
                fixture.quarter_turns = if below { 0 } else { 2 };
                fixture.center = Point::new(
                    step(point.x),
                    wall.start.y + if below { offset } else { -offset },
                );
            } else if (wall.start.x - wall.end.x).abs() < 1e-3 {
                let right = point.x > wall.start.x;
                fixture.quarter_turns = if right { 3 } else { 1 };
                fixture.center = Point::new(
                    wall.start.x + if right { offset } else { -offset },
                    step(point.y),
                );
            }
        }
        fixture
    }

    pub(super) fn place_fixture(&mut self, kind: FixtureKind, point: Point) {
        let placed = self.fixture_placement(kind, point);
        let previous = self.project.clone();
        match self.project.add_fixture(
            kind,
            placed.center,
            placed.width,
            placed.depth,
            placed.quarter_turns,
        ) {
            Some(_) => {
                self.record_change(previous);
                self.message = format!(
                    "{} inserido(a). Clique de novo para outro, ou Esc.",
                    kind.label()
                );
            }
            None => self.message = "O objeto precisa ficar dentro do terreno.".to_owned(),
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
            Selection::Fixture(id) => self.project.move_fixture(id, delta),
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

    /// Where the ruler lands for the pointer at `point`: on the corners and
    /// faces that matter on site (wall corners and faces with their real
    /// thickness, door and window jambs, fixtures, the terrain), else on a
    /// 5 cm grid. F8 keeps it horizontal or vertical.
    pub(super) fn measure_snap(&self, point: Point, start: Option<Point>) -> (Point, Option<SnapKind>) {
        let threshold = (12.0 / self.scale).clamp(0.03, 0.4);
        let mut best: Option<(Point, f32)> = None;
        let mut consider = |candidate: Point| {
            let distance = candidate.distance(point);
            if distance < threshold && best.is_none_or(|(_, closest)| distance < closest) {
                best = Some((candidate, distance));
            }
        };
        let offset = |p: Point, a: (f32, f32), ka: f32, b: (f32, f32), kb: f32| {
            Point::new(p.x + a.0 * ka + b.0 * kb, p.y + a.1 * ka + b.1 * kb)
        };
        let mut faces = Vec::new();
        for wall in &self.project.walls {
            let length = wall.length();
            if length < 1e-4 {
                continue;
            }
            let half = wall.thickness() / 2.0;
            let along = ((wall.end.x - wall.start.x) / length, (wall.end.y - wall.start.y) / length);
            let normal = (-along.1, along.0);
            for (end, outward) in [(wall.start, -1.0), (wall.end, 1.0)] {
                consider(end);
                for side in [-1.0, 1.0] {
                    consider(offset(end, normal, side * half, along, 0.0));
                    consider(offset(end, along, outward * half, normal, side * half));
                }
            }
            // The nearest point on either face, for clear spans.
            let (foot, t) = crate::domain::geometry::nearest_point_on_segment(point, wall.start, wall.end);
            if (0.0..=1.0).contains(&t) {
                let side = (point.x - foot.x) * normal.0 + (point.y - foot.y) * normal.1;
                if side.abs() > 1e-6 {
                    faces.push(offset(foot, normal, side.signum() * half, along, 0.0));
                }
            }
        }
        for opening in &self.project.openings {
            let (Some(center), Some(wall)) = (
                self.opening_center(opening.id),
                self.project.walls.iter().find(|wall| wall.id == opening.wall_id),
            ) else {
                continue;
            };
            let length = wall.length().max(1e-4);
            let along = ((wall.end.x - wall.start.x) / length, (wall.end.y - wall.start.y) / length);
            let normal = (-along.1, along.0);
            for end in [-1.0, 1.0] {
                for side in [-1.0, 0.0, 1.0] {
                    consider(offset(
                        center,
                        along,
                        end * opening.width() / 2.0,
                        normal,
                        side * wall.thickness() / 2.0,
                    ));
                }
            }
        }
        for fixture in &self.project.fixtures {
            let (hw, hd) = (fixture.width / 2.0, fixture.depth / 2.0);
            for (x, y) in [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)] {
                consider(fixture.to_world(Point::new(x, y)));
            }
        }
        if let Some(terrain) = &self.project.terrain {
            for corner in [
                terrain.min,
                Point::new(terrain.max.x, terrain.min.y),
                terrain.max,
                Point::new(terrain.min.x, terrain.max.y),
            ] {
                consider(corner);
            }
        }
        let (mut result, kind) = if let Some((corner, _)) = best {
            (corner, Some(SnapKind::Endpoint))
        } else if let Some(face) = faces
            .into_iter()
            .filter(|face| face.distance(point) < threshold)
            .min_by(|a, b| a.distance(point).total_cmp(&b.distance(point)))
        {
            (face, Some(SnapKind::Face))
        } else if self.snap_enabled {
            (point.snapped_to(0.05), Some(SnapKind::Grid))
        } else {
            (point, None)
        };
        if self.ortho_enabled
            && let Some(start) = start
        {
            if (result.x - start.x).abs() >= (result.y - start.y).abs() {
                result.y = start.y;
            } else {
                result.x = start.x;
            }
        }
        (result, kind)
    }

    fn paint_view_aids(&self, painter: &egui::Painter, view: ViewTransform) {
        for (start, end) in &self.measurements {
            canvas::paint_measurement(painter, view, *start, *end);
        }
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
        app.modulation_enabled = false;
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
    fn right_click_ends_a_draft_and_selects_what_is_under_the_pointer() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();

        app.set_tool(Tool::Wall);
        app.creation_state = CreationState::WaitingSecondPoint {
            start: Point::new(0.0, 2.0),
        };
        app.context_click(Some(Point::new(9.0, 9.0)));
        assert!(app.draft_start().is_none());
        assert_eq!(app.selection, None);
        assert_eq!(app.tool, Tool::Wall);

        app.context_click(Some(Point::new(2.0, 0.02)));
        assert_eq!(app.selection, Some(Selection::Wall(wall)));
        assert_eq!(app.tool, Tool::Move);

        // Right-clicking empty space keeps the selection for the menu.
        app.context_click(Some(Point::new(9.0, 9.0)));
        assert_eq!(app.selection, Some(Selection::Wall(wall)));
    }

    #[test]
    fn fixtures_stand_against_the_wall_face() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        // A horizontal wall along y = 0 and a vertical one along x = 0.
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0));
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(0.0, 6.0));
        let half = crate::domain::geometry::WALL_THICKNESS_METERS / 2.0;

        let below = app.fixture_placement(FixtureKind::KitchenSink, Point::new(3.02, 0.5));
        assert_eq!(below.quarter_turns, 0);
        assert!((below.center.y - (half + 0.3)).abs() < 1e-4);
        assert!((below.center.x - 3.0).abs() < 1e-4);
        // Its back touches the face of the bricks, not the centre line.
        let (low, _) = below.bounds();
        assert!((low.y - half).abs() < 1e-4);

        let beside = app.fixture_placement(FixtureKind::Grill, Point::new(0.4, 3.0));
        assert_eq!(beside.quarter_turns, 3);
        let (low, _) = beside.bounds();
        assert!((low.x - half).abs() < 1e-4);

        // Far from walls the chosen rotation is kept.
        app.fixture_turns = 1;
        let free = app.fixture_placement(FixtureKind::Table, Point::new(3.0, 3.0));
        assert_eq!(free.quarter_turns, 1);

        app.set_tool(Tool::Fixture(FixtureKind::KitchenSink));
        app.place_fixture(FixtureKind::KitchenSink, Point::new(3.0, 0.5));
        assert_eq!(app.project.fixtures.len(), 1);
        assert!(app.history.can_undo());
        assert_eq!(
            app.pick(Point::new(3.0, half + 0.3)),
            Some(Selection::Fixture(app.project.fixtures[0].id))
        );
    }

    #[test]
    fn measuring_snaps_to_faces_and_corners_and_changes_nothing() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
        app.project
            .add_wall(Point::new(0.0, 3.0), Point::new(4.0, 3.0));
        app.set_tool(Tool::Measure);
        let half = crate::domain::geometry::WALL_THICKNESS_METERS / 2.0;

        let (near, kind) = app.measure_snap(Point::new(2.0, half + 0.01), None);
        assert_eq!(kind, Some(SnapKind::Face));
        assert!((near.y - half).abs() < 1e-5);
        let (far, _) = app.measure_snap(Point::new(2.0, 3.0 - half - 0.01), Some(near));
        assert!((far.y - (3.0 - half)).abs() < 1e-5);
        let (corner, kind) = app.measure_snap(Point::new(4.05, half - 0.01), None);
        assert_eq!(kind, Some(SnapKind::Endpoint));
        assert!((corner.x - (4.0 + half)).abs() < 1e-5);

        let before = app.project.clone();
        app.complete_creation(near, far);
        assert_eq!(app.measurements.len(), 1);
        assert_eq!(app.project, before);
        assert!(!app.history.can_undo() && !app.dirty);
        // The clear span between the faces.
        assert!(app.message.contains(&crate::domain::geometry::format_meters(3.0 - 2.0 * half)));
    }

    #[test]
    fn walls_and_rooms_follow_the_block_module() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.snap_enabled = false;
        app.modulation_enabled = true;
        app.project.masonry = crate::domain::masonry::Masonry::presets()
            .into_iter()
            .find(|material| material.name == "Bloco de concreto 14")
            .unwrap();
        app.set_tool(Tool::Wall);
        // A 40 cm module: lengths in 20 cm steps.
        let end = app.snap(Point::new(2.13, 0.0), Some(Point::new(0.0, 0.0)));
        assert!((end.x - 2.2).abs() < 1e-4);
        app.set_tool(Tool::Room);
        let corner = app.snap(Point::new(3.07, 2.31), Some(Point::new(0.0, 0.0)));
        assert!((corner.x - 3.0).abs() < 1e-4 && (corner.y - 2.4).abs() < 1e-4);

        // New walls are built with the material and drawn at its thickness.
        let id = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(2.2, 0.0))
            .unwrap();
        let wall = app.project.walls.iter().find(|wall| wall.id == id).unwrap();
        assert!((wall.thickness() - (0.14 + 0.03)).abs() < 1e-5);
        app.modulation_enabled = false;
        app.set_tool(Tool::Wall);
        let free = app.snap(Point::new(2.13, 0.0), Some(Point::new(0.0, 0.0)));
        assert!((free.x - 2.13).abs() < 1e-4);
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
