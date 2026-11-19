use super::{
    CreationState, FloorPlanApp, GripTarget, ItemDrag, Selection, Tool, VertexDrag,
    canvas::{
        self, PaintOptions, Preview, ViewTransform,
        overlay::{self, SnapKind},
    },
    tools::ModifyKind,
};
use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::{POINT_TOLERANCE, Point},
    snap::{SnapResult, find_snap},
    transform::Transform,
};
use eframe::egui::{self, PointerButton, Sense};

impl FloorPlanApp {
    /// Whether typed input is an angle to turn by (turning, with the centre
    /// placed) or the offset distance, rather than a point.
    fn typing_angle_or_offset(&self) -> bool {
        match self.tool {
            Tool::Modify(ModifyKind::Rotate) => self.modify_anchor.is_some(),
            Tool::Modify(ModifyKind::Offset) => true,
            _ => false,
        }
    }

    fn snap(&self, point: Point, start: Option<Point>) -> Point {
        self.snap_detail(point, start)
            .map(|hit| hit.point)
            .unwrap_or(point)
    }

    pub(super) fn snap_detail(&self, point: Point, start: Option<Point>) -> Option<SnapResult> {
        let mut hit = if self.snap_enabled {
            let draft_points: &[Point] = match self.tool {
                Tool::RoomPolygon => &self.polygon_draft,
                Tool::Draw(_) => &self.draw_points,
                _ => &[],
            };
            let threshold = self.screen_metres(12.0).clamp(0.12, 0.40);
            Some(find_snap(
                &self.project,
                draft_points,
                point,
                start,
                threshold,
                self.osnap,
            ))
        } else {
            None
        };

        let mut result = hit.map(|h| h.point).unwrap_or(point);
        let ortho_tool = match self.tool {
            Tool::Wall | Tool::RoomPolygon | Tool::Draw(_) => true,
            Tool::Modify(kind) => kind.uses_selection(),
            _ => false,
        };
        if self.ortho_enabled
            && ortho_tool
            && let Some(start) = start
        {
            let before = result;
            if (result.x - start.x).abs() >= (result.y - start.y).abs() {
                result.y = start.y;
            } else {
                result.x = start.x;
            }
            if !result.near(before, POINT_TOLERANCE)
                && let Some(h) = &mut hit
            {
                h.point = result;
                h.kind = SnapKind::Extension;
                h.guide = Some((start, result));
            }
        }
        let exact_object_snap = hit.is_some_and(|h| {
            matches!(
                h.kind,
                SnapKind::Endpoint
                    | SnapKind::Midpoint
                    | SnapKind::Intersection
                    | SnapKind::Center
                    | SnapKind::Perpendicular
                    | SnapKind::Tangent
            )
        });
        if self.modulation_enabled
            && let Some(start) = start
            && !exact_object_snap
            && !self.is_wall_end(result)
        {
            let modular = self.modular_point(start, result);
            if !modular.near(result, POINT_TOLERANCE) {
                result = modular;
                if let Some(h) = &mut hit {
                    h.point = result;
                    if h.kind == SnapKind::Extension {
                        h.guide = Some((start, result));
                    }
                }
            }
        }
        hit.or_else(|| {
            (!result.near(point, POINT_TOLERANCE)).then_some(SnapResult {
                point: result,
                kind: SnapKind::Extension,
                guide: start.map(|from| (from, result)),
            })
        })
    }

    fn is_wall_end(&self, point: Point) -> bool {
        self.project.walls.iter().any(|wall| {
            wall.start.near(point, POINT_TOLERANCE) || wall.end.near(point, POINT_TOLERANCE)
        })
    }

    /// Rounds what is being drawn to whole half blocks of the active
    /// material: a wall's length, or a room's width and length.
    pub(super) fn modular_point(&self, start: Point, point: Point) -> Point {
        let material = &self.project.masonry;
        match self.tool {
            Tool::Wall | Tool::RoomPolygon => {
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
                let side = |from: f64, to: f64| {
                    let span = to - from;
                    from + span.signum() * material.modular_length(span.abs())
                };
                Point::new(side(start.x, point.x), side(start.y, point.y))
            }
            _ => point,
        }
    }

    #[allow(dead_code)]
    fn snap_kind(&self, point: Point) -> Option<SnapKind> {
        if !self.snap_enabled {
            return None;
        }
        let hit = find_snap(
            &self.project,
            &self.polygon_draft,
            point,
            self.draft_start(),
            POINT_TOLERANCE * 2.0,
            self.osnap,
        );
        Some(if hit.point.near(point, POINT_TOLERANCE * 2.0) {
            hit.kind
        } else {
            SnapKind::Grid
        })
    }

    /// The element a click at `point` selects: openings first, then walls,
    /// rooms and the terrain.
    fn pick(&self, point: Point) -> Option<Selection> {
        self.project
            .nearest_opening(point, self.screen_metres(14.0))
            .map(Selection::Opening)
            .or_else(|| self.project.fixture_at(point).map(Selection::Fixture))
            .or_else(|| {
                self.project
                    .drawing_at(point, self.screen_metres(8.0))
                    .map(Selection::Drawing)
            })
            .or_else(|| {
                self.project
                    .nearest_wall(point, self.screen_metres(12.0))
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
                    - egui::vec2(
                        (before.x * f64::from(self.scale)) as f32,
                        (before.y * f64::from(self.scale)) as f32,
                    );
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
        let mut snap_hit: Option<SnapResult> = None;
        let snapped = world.map(|point| {
            if self.tool == Tool::Measure {
                return self.measure_snap(point, self.draft_start()).0;
            }
            let detail = self.snap_detail(point, self.draft_start());
            let mut result = detail.map(|h| h.point).unwrap_or(point);
            if matches!(self.tool, Tool::Room | Tool::RoomPolygon | Tool::Wall)
                && let Some(terrain) = &self.project.terrain
            {
                let clamped = Point::new(
                    result.x.clamp(terrain.min.x, terrain.max.x),
                    result.y.clamp(terrain.min.y, terrain.max.y),
                );
                result = clamped;
            }
            snap_hit = detail.map(|mut h| {
                if !h.point.near(result, POINT_TOLERANCE) {
                    h.point = result;
                    h.guide = None;
                }
                h
            });
            result
        });
        self.draft_aim = snapped;

        if let Some(corners) = &self.pending_room {
            // Keep the drawn room on screen while it is named.
            canvas::paint(
                &painter,
                view,
                &self.project,
                PaintOptions {
                    selection: self.selection,
                    show_grid: self.show_grid,
                    dimensions: self.show_dimensions,
                    ..Default::default()
                },
            );
            canvas::paint_polygon_draft(&painter, view, corners, None, true);
            self.paint_view_aids(&painter, view);
            self.canvas_navigation(ui, navigation);
            return;
        }

        let hovered_grip = if self.tool == Tool::Move && self.item_drag.is_none() {
            world.and_then(|point| self.grip_at(point))
        } else {
            None
        };
        let hovered_item = if self.item_drag.is_some()
            || self.vertex_drag.is_some()
            || self.box_select.is_some()
        {
            None
        } else {
            world.and_then(|point| self.pick(point))
        };

        if self.tool == Tool::Move && !panning {
            if self.vertex_drag.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if hovered_grip.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if hovered_item.is_some() {
                ui.ctx().set_cursor_icon(if self.item_drag.is_some() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                });
            }
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
                        modifiers,
                    } if on_canvas(*pos) => Some((*pos, modifiers.shift, modifiers.command)),
                    _ => None,
                })
            });
            if let Some((position, shift, alone)) = pressed {
                self.begin_item_drag_with(view.to_world(position), shift, alone);
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
            self.finish_box_select(self.screen_metres(4.0));
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
            self.draft_input.clear();
            match self.tool {
                Tool::Wall
                | Tool::Room
                | Tool::Terrain
                | Tool::Measure
                | Tool::RoomPolygon
                | Tool::Draw(_) => self.apply_canvas_point(point),
                Tool::Modify(kind) if kind.uses_selection() => self.apply_canvas_point(point),
                // Offset, trim and extend act where the element was clicked.
                Tool::Modify(kind) => {
                    if let Some(raw) = world {
                        self.modify_click(kind, raw);
                    }
                }
                Tool::Constrain(kind) => {
                    if let Some(raw) = world {
                        self.constrain_click(kind, raw);
                    }
                }
                Tool::Opening(kind) => self.add_opening_at(point, kind),
                Tool::Fixture(kind) => {
                    if let Some(raw) = world {
                        self.place_fixture(kind, raw);
                    }
                }
                Tool::Move => {}
            }
        }

        if self.tool == Tool::RoomPolygon && response.double_clicked() {
            self.close_polygon_draft();
        }
        if let Tool::Draw(kind) = self.tool
            && response.double_clicked()
        {
            self.finish_drawing(kind, false);
        }

        // An angle to turn by or an offset distance is not a point.
        let numeric_spec = (!self.draft_input.is_empty() && !self.typing_angle_or_offset())
            .then(|| super::tools::numeric_input::parse_numeric_input(&self.draft_input))
            .flatten();
        let numeric_point =
            numeric_spec.and_then(|spec| spec.resolve(self.draft_start(), snapped.or(world)));
        let preview_cursor = numeric_point.or(snapped);

        let preview = match (self.draft_start(), preview_cursor, self.tool) {
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
        } else if let Tool::Modify(kind) = self.tool
            && !kind.uses_selection()
        {
            // The wall or drawing offset, trim or extend would act on.
            self.modify_target
                .or_else(|| world.and_then(|point| self.pick_editable(point)))
        } else if let Tool::Constrain(kind) = self.tool {
            // What the constraint tool has picked, or would pick.
            self.constraint_pick
                .map(Self::constraint_pick_element)
                .or_else(|| {
                    let point = world?;
                    match kind {
                        super::tools::ConstraintTool::Coincident
                        | super::tools::ConstraintTool::Distance => self
                            .pick_vertex(point)
                            .map(|(vertex, _)| vertex.entity.element())
                            .or_else(|| self.pick_line(point).map(|line| line.element())),
                        _ => self.pick_line(point).map(|line| line.element()),
                    }
                })
        } else {
            None
        }
        .or(self.browser_hover);
        let ghost = match (self.tool, world) {
            (Tool::Fixture(kind), Some(point)) => Some(self.fixture_placement(kind, point)),
            _ => None,
        };
        // What the selection box would pick, highlighted as it is drawn.
        let boxed: Vec<Selection> = self
            .box_select
            .filter(|selection_box| {
                selection_box.start.distance(selection_box.end) >= self.screen_metres(4.0)
            })
            .map(|selection_box| {
                self.project.elements_in_box(
                    selection_box.start,
                    selection_box.end,
                    selection_box.crossing(),
                    self.selection_filter,
                )
            })
            .unwrap_or_default();
        canvas::paint(
            &painter,
            view,
            &self.project,
            PaintOptions {
                selection: self.selection,
                extra_selection: &self.selection_extra,
                pending: &boxed,
                hover,
                preview,
                show_grid: self.show_grid,
                ghost,
                dimensions: self.show_dimensions,
                wall_reference: self.wall_reference,
            },
        );
        self.paint_view_aids(&painter, view);
        // Selected groups show their extent and name.
        for group in &self.project.groups {
            if group.members.iter().all(|member| self.is_selected(*member))
                && let Some((low, high)) = self.project.elements_bounds(&group.members)
            {
                overlay::paint_group_frame(
                    &painter,
                    egui::Rect::from_two_pos(view.to_screen(low), view.to_screen(high)),
                    &group.name,
                );
            }
        }
        // The constraints of the selection, or all of them with the tool.
        for (at, text, concerned) in self.constraint_marks() {
            overlay::paint_constraint_badge(&painter, view.to_screen(at), &text, concerned);
        }
        if let Tool::Constrain(kind) = self.tool
            && matches!(
                kind,
                super::tools::ConstraintTool::Coincident | super::tools::ConstraintTool::Distance
            )
            && let Some((_, at)) = world.and_then(|point| self.pick_vertex(point))
        {
            overlay::paint_snap_marker(&painter, view.to_screen(at), SnapKind::Endpoint);
        }
        if let Some(selection_box) = self.box_select
            && selection_box.start.distance(selection_box.end) >= self.screen_metres(4.0)
        {
            overlay::paint_selection_box(
                &painter,
                view.to_screen(selection_box.start),
                view.to_screen(selection_box.end),
                selection_box.crossing(),
            );
        }
        if self.tool == Tool::Move {
            for (pos, target) in self.selection_grips() {
                let active = self.vertex_drag.is_some_and(|drag| drag.target == target)
                    || hovered_grip.is_some_and(|(_, hovered)| hovered == target);
                overlay::paint_vertex_grip(&painter, view.to_screen(pos), active);
            }
        }
        if self.tool == Tool::RoomPolygon && !self.polygon_draft.is_empty() {
            canvas::paint_polygon_draft(&painter, view, &self.polygon_draft, preview_cursor, false);
        }
        if let Tool::Draw(kind) = self.tool {
            let draft = Self::shape_from(kind, &self.draw_points, preview_cursor, false)
                .filter(|_| !self.draw_points.is_empty());
            canvas::paint_draw_draft(&painter, view, draft.as_ref(), &self.draw_points);
        }
        if self.tool == Tool::Measure
            && let (Some(start), Some(end)) = (self.draft_start(), preview_cursor)
        {
            canvas::paint_measurement(&painter, view, start, end);
        }
        let typed_angle = match self.tool {
            Tool::Modify(ModifyKind::Rotate) if !self.draft_input.is_empty() => {
                super::tools::parse_degrees(&self.draft_input)
            }
            _ => None,
        };
        if let Tool::Modify(kind) = self.tool {
            // The selection where it would go, or the offset copy.
            let ghost = match (typed_angle, self.modify_anchor) {
                (Some(degrees), Some(center)) => self
                    .project
                    .fragment(&self.selected())
                    .transformed(&Transform::rotate_degrees(center, degrees))
                    .ok(),
                _ => preview_cursor.and_then(|cursor| self.modify_preview(kind, cursor)),
            };
            if let Some(ghost) = &ghost {
                canvas::paint_fragment_ghost(&painter, view, ghost);
            }
            if let (Some(anchor), Some(cursor)) = (self.modify_anchor, preview_cursor) {
                canvas::paint_modify_guide(
                    &painter,
                    view,
                    anchor,
                    cursor,
                    kind == ModifyKind::Mirror,
                );
            }
        }

        if let Some(position) = pointer
            && !panning
        {
            let drawing = matches!(
                self.tool,
                Tool::Wall
                    | Tool::Room
                    | Tool::RoomPolygon
                    | Tool::Terrain
                    | Tool::Fixture(_)
                    | Tool::Measure
                    | Tool::Draw(_)
                    | Tool::Modify(_)
                    | Tool::Constrain(_)
            ) || opening_target;
            if drawing {
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
                overlay::paint_crosshair(&painter, position);
            }
            let snapping_tool = match self.tool {
                Tool::Wall | Tool::Room | Tool::RoomPolygon | Tool::Terrain | Tool::Draw(_) => true,
                Tool::Modify(kind) => kind.uses_selection(),
                _ => false,
            };
            if snapping_tool && let Some(hit) = snap_hit {
                if let Some((from, to)) = hit.guide {
                    overlay::paint_snap_guide(&painter, view.to_screen(from), view.to_screen(to));
                }
                overlay::paint_snap_marker(&painter, view.to_screen(hit.point), hit.kind);
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

        if !self.draft_input.is_empty() {
            if let Some(target) = numeric_point {
                overlay::paint_snap_marker(&painter, view.to_screen(target), SnapKind::Endpoint);
            }
            let typed_offset = (self.tool == Tool::Modify(ModifyKind::Offset))
                .then(|| {
                    self.draft_input
                        .trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .ok()
                })
                .flatten()
                .filter(|distance| distance.is_finite() && *distance > 0.0);
            let (summary, valid) = match (numeric_spec, numeric_point) {
                _ if self.typing_angle_or_offset() => match (typed_angle, typed_offset) {
                    (Some(degrees), _) => (
                        format!(
                            "Girar {}° · Enter confirma · Esc limpa",
                            format!("{degrees:.1}").replace('.', ",").replace(",0", "")
                        ),
                        true,
                    ),
                    (_, Some(distance)) => (
                        format!(
                            "Distância de deslocamento: {} · Enter confirma",
                            crate::domain::geometry::format_meters(distance)
                        ),
                        true,
                    ),
                    _ => (
                        if self.tool == Tool::Modify(ModifyKind::Offset) {
                            "Distância em metros (ex.: 0,15)".to_owned()
                        } else {
                            "Ângulo em graus (ex.: 45 ou -90)".to_owned()
                        },
                        false,
                    ),
                },
                (Some(spec), Some(_)) => (
                    format!("{} · Enter confirma · Esc limpa", spec.describe()),
                    true,
                ),
                (Some(spec), None) if spec.requires_start() => (
                    "Clique no 1º ponto ou digite X;Y (ex.: 1,5;2,0)".to_owned(),
                    false,
                ),
                _ => (
                    "Ex.: 3,47 · 3,5<45 · <30 · 3,5x4,0 · @2;1 · 1,5;2,0".to_owned(),
                    false,
                ),
            };
            let anchor = numeric_point.map(|pt| view.to_screen(pt)).or(pointer);
            overlay::paint_numeric_input_hud(
                &painter,
                rect,
                anchor,
                &self.draft_input,
                &summary,
                valid,
            );
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
        // "Colar aqui" pastes where the menu was opened.
        self.context_point = point;
        if self.draft_start().is_some() {
            self.cancel_creation();
            self.message = "Desenho cancelado.".to_owned();
        }
        // On an element of a selection of several, the menu acts on all.
        if let Some(selection) = point.and_then(|point| self.pick(point))
            && !(self.is_multi_selection() && self.is_selected(selection))
        {
            self.select(selection);
        }
    }

    /// Editable vertex handles for the current selection in Select/Move mode.
    pub(super) fn selection_grips(&self) -> Vec<(Point, GripTarget)> {
        // Vertices are edited one element at a time.
        if self.is_multi_selection() {
            return Vec::new();
        }
        match self.selection {
            Some(Selection::Wall(id)) => self
                .project
                .walls
                .iter()
                .find(|wall| wall.id == id)
                .map(|wall| {
                    vec![
                        (wall.start, GripTarget::WallCorner(wall.start)),
                        (wall.end, GripTarget::WallCorner(wall.end)),
                    ]
                })
                .unwrap_or_default(),
            Some(Selection::Room(id)) => self
                .project
                .rooms
                .iter()
                .find(|room| room.id == id)
                .and_then(|room| self.project.room_outline(room))
                .map(|corners| {
                    corners
                        .into_iter()
                        .map(|corner| (corner, GripTarget::WallCorner(corner)))
                        .collect()
                })
                .unwrap_or_default(),
            Some(Selection::Drawing(id)) => self
                .project
                .drawing(id)
                .map(|drawing| {
                    crate::domain::model::Project::drawing_handles(&drawing.shape)
                        .into_iter()
                        .enumerate()
                        .map(|(index, pos)| (pos, GripTarget::DrawingHandle { id, index }))
                        .collect()
                })
                .unwrap_or_default(),
            Some(Selection::Terrain(_)) => self
                .project
                .terrain_corners()
                .map(|corners| {
                    corners
                        .into_iter()
                        .enumerate()
                        .map(|(index, pos)| (pos, GripTarget::TerrainCorner(index)))
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn grip_at(&self, point: Point) -> Option<(Point, GripTarget)> {
        let tolerance = self.screen_metres(10.0).clamp(0.08, 0.35);
        self.selection_grips()
            .into_iter()
            .filter(|(pos, _)| pos.distance(point) <= tolerance)
            .min_by(|a, b| a.0.distance(point).total_cmp(&b.0.distance(point)))
    }

    /// Picks the element (or vertex grip of the current selection) at `point`
    /// and gets ready to drag it.
    #[cfg(test)]
    pub(super) fn begin_item_drag(&mut self, point: Point) {
        self.begin_item_drag_with(point, false, false);
    }

    /// A press with the selection tool. On a grip it edits that vertex; on
    /// an element it selects it — with its whole group unless `alone`
    /// (Ctrl) — and gets ready to drag it, with the rest of the selection
    /// when it is part of it; Shift adds it or takes it out. On empty
    /// space, or on a terrain that is not selected, it starts a selection
    /// box.
    pub(super) fn begin_item_drag_with(&mut self, point: Point, shift: bool, alone: bool) {
        self.item_drag = None;
        self.vertex_drag = None;
        self.drag_initial_project = None;
        if !shift && let Some((_, target)) = self.grip_at(point) {
            self.vertex_drag = Some(VertexDrag {
                target,
                moved: false,
            });
            self.drag_initial_project = Some(self.project.clone());
            self.message = "Arraste o vértice para editar a geometria.".to_owned();
            return;
        }
        let item = match self.pick(point) {
            None => {
                self.begin_box_select(point, shift, None);
                return;
            }
            // The terrain is the plan's background: dragging over it draws
            // a selection box, and a click selects it.
            Some(terrain @ Selection::Terrain(_)) if !self.is_selected(terrain) => {
                self.begin_box_select(point, shift, Some(terrain));
                return;
            }
            Some(item) => item,
        };
        // A grouped element brings its group along, the clicked one last
        // (it becomes the primary one).
        let members = if alone {
            vec![item]
        } else {
            let mut members: Vec<Selection> = self
                .project
                .with_whole_groups(&[item])
                .into_iter()
                .filter(|member| *member != item)
                .collect();
            members.push(item);
            members
        };
        if shift {
            if members.len() == 1 {
                self.toggle_selection(item);
            } else {
                let mut all = self.selected();
                if members.iter().all(|member| all.contains(member)) {
                    all.retain(|kept| !members.contains(kept));
                } else {
                    all.extend(members);
                }
                self.set_selection_set(all);
                self.after_selection_change();
            }
            return;
        }
        if alone {
            self.set_selection(Some(item));
        } else if !(self.is_multi_selection() && self.is_selected(item)) {
            self.set_selection_set(members);
        }
        let group = self.is_multi_selection() && self.is_selected(item);
        // An opening keeps the spot where it was grabbed instead of jumping
        // so that its centre sits under the pointer.
        let grab_offset = match item {
            Selection::Opening(id) if !group => self
                .opening_center(id)
                .map(|center| Point::new(point.x - center.x, point.y - center.y))
                .unwrap_or(Point::new(0.0, 0.0)),
            _ => Point::new(0.0, 0.0),
        };
        self.item_drag = Some(ItemDrag {
            selection: item,
            last: self.drag_point(item, group, point),
            grab_offset,
            moved: false,
            group,
        });
        // Locked layers keep their elements in place, alone or in a group.
        if self
            .selected()
            .into_iter()
            .any(|selected| self.selection_is_locked(selected))
        {
            self.item_drag = None;
            self.drag_initial_project = None;
            self.message = "A camada deste elemento está bloqueada.".to_owned();
            self.sync_inspector_fields();
            return;
        }
        self.drag_initial_project = Some(self.project.clone());
        self.after_selection_change();
    }

    /// Walls, rooms and the terrain move in grid steps when snapping is on,
    /// fixtures in 5 cm steps; openings slide freely along their wall.
    fn drag_position(&self, selection: Selection, point: Point) -> Point {
        match selection {
            _ if !self.snap_enabled => point,
            Selection::Opening(_) => point,
            Selection::Fixture(_) | Selection::Drawing(_) => point.snapped_to(0.05),
            _ => point.snapped_to(0.25),
        }
    }

    /// Where the drag stands for the pointer at `point`. Several elements
    /// dragged together move in grid steps when any of them is built (a
    /// wall, a room, the terrain), else in 5 cm steps.
    fn drag_point(&self, grabbed: Selection, group: bool, point: Point) -> Point {
        if !group {
            return self.drag_position(grabbed, point);
        }
        if !self.snap_enabled {
            return point;
        }
        let built = self.selected().iter().any(|item| {
            matches!(
                item,
                Selection::Wall(_) | Selection::Room(_) | Selection::Terrain(_)
            )
        });
        point.snapped_to(if built { 0.25 } else { 0.05 })
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
        width: f64,
        depth: f64,
        turns: u8,
    ) -> Fixture {
        let step = |value: f64| {
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
                if !self.record_change(previous) {
                    return;
                }
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
        if self.box_select.is_some() {
            self.update_box_select(point);
            return;
        }
        if self.project.constraints.is_empty() {
            self.drag_step(point);
            return;
        }
        // Constraints hold while dragging: what the drag does not touch
        // follows, and a step that cannot keep them is not taken.
        let before = self.project.clone();
        let drags = (self.item_drag, self.vertex_drag);
        let corner = match self.vertex_drag {
            Some(VertexDrag {
                target: GripTarget::WallCorner(anchor),
                ..
            }) => self.project.walls.iter().find_map(|wall| {
                if wall
                    .start
                    .near(anchor, crate::domain::geometry::JOIN_TOLERANCE)
                {
                    Some((wall.id, true))
                } else if wall
                    .end
                    .near(anchor, crate::domain::geometry::JOIN_TOLERANCE)
                {
                    Some((wall.id, false))
                } else {
                    None
                }
            }),
            _ => None,
        };
        self.drag_step(point);
        if self.project == before {
            return;
        }
        if let Err(error) = self.project.settle_constraints(&before) {
            self.project = before;
            (self.item_drag, self.vertex_drag) = drags;
            self.message = error;
            return;
        }
        // The dragged corner is wherever the constraints left it.
        if let (Some((id, at_start)), Some(drag)) = (corner, &mut self.vertex_drag)
            && let GripTarget::WallCorner(anchor) = &mut drag.target
            && let Some(wall) = self.project.walls.iter().find(|wall| wall.id == id)
        {
            *anchor = if at_start { wall.start } else { wall.end };
        }
        self.sync_inspector_fields();
    }

    /// One step of a drag, before the constraints are kept.
    fn drag_step(&mut self, point: Point) {
        if self.vertex_drag.is_some() {
            self.drag_vertex_to(point);
            return;
        }
        let Some(drag) = self.item_drag else {
            return;
        };
        let current = self.drag_point(drag.selection, drag.group, point);
        let delta = Point::new(current.x - drag.last.x, current.y - drag.last.y);
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }
        if drag.group {
            if let Err(error) = self.project.move_elements(&self.selected(), delta) {
                self.message = error;
                return;
            }
            if let Some(drag) = &mut self.item_drag {
                drag.last = current;
                drag.moved = true;
            }
            return;
        }
        match drag.selection {
            Selection::Wall(id) => {
                if let Err(error) = self.project.move_wall_checked(id, delta) {
                    self.message = error;
                    return;
                }
            }
            Selection::Room(id) => {
                if !self.project.move_room(id, delta) {
                    self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                }
            }
            Selection::Terrain(id) => self.project.move_terrain(id, delta),
            Selection::Fixture(id) => self.project.move_fixture(id, delta),
            Selection::Drawing(id) => self.project.move_drawing(id, delta),
            Selection::Opening(id) => {
                let target = Point::new(point.x - drag.grab_offset.x, point.y - drag.grab_offset.y);
                self.project.move_opening_along_wall(id, target);
            }
        }
        if let Some(drag) = &mut self.item_drag {
            drag.last = current;
            drag.moved = true;
        }
    }

    fn drag_vertex_to(&mut self, raw_point: Point) {
        let Some(drag) = self.vertex_drag else {
            return;
        };
        let target_point = if self.snap_enabled {
            match drag.target {
                GripTarget::DrawingHandle { .. } => raw_point.snapped_to(0.05),
                GripTarget::WallCorner(_) | GripTarget::TerrainCorner(_) => {
                    self.snap(raw_point, None)
                }
            }
        } else {
            raw_point
        }
        .rounded();

        let outcome = match drag.target {
            GripTarget::WallCorner(anchor) => {
                if anchor.near(target_point, POINT_TOLERANCE) {
                    return;
                }
                self.project.move_wall_corner(anchor, target_point)
            }
            GripTarget::DrawingHandle { id, index } => {
                self.project.move_drawing_handle(id, index, target_point)
            }
            GripTarget::TerrainCorner(corner) => {
                self.project.move_terrain_corner(corner, target_point)
            }
        };

        match outcome {
            Ok(()) => {
                if let Some(active) = &mut self.vertex_drag {
                    if let GripTarget::WallCorner(anchor) = &mut active.target {
                        *anchor = target_point;
                    }
                    active.moved = true;
                }
                self.sync_inspector_fields();
            }
            Err(error) => {
                self.message = error;
            }
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
    pub(super) fn measure_snap(
        &self,
        point: Point,
        start: Option<Point>,
    ) -> (Point, Option<SnapKind>) {
        let threshold = self.screen_metres(12.0).clamp(0.03, 0.4);
        let mut best: Option<(Point, f64)> = None;
        let mut consider = |candidate: Point| {
            let distance = candidate.distance(point);
            if distance < threshold && best.is_none_or(|(_, closest)| distance < closest) {
                best = Some((candidate, distance));
            }
        };
        let offset = |p: Point, a: (f64, f64), ka: f64, b: (f64, f64), kb: f64| {
            Point::new(p.x + a.0 * ka + b.0 * kb, p.y + a.1 * ka + b.1 * kb)
        };
        let mut faces = Vec::new();
        for wall in &self.project.walls {
            let length = wall.length();
            if length < 1e-4 {
                continue;
            }
            let half = wall.thickness() / 2.0;
            let along = (
                (wall.end.x - wall.start.x) / length,
                (wall.end.y - wall.start.y) / length,
            );
            let normal = (-along.1, along.0);
            for (end, outward) in [(wall.start, -1.0), (wall.end, 1.0)] {
                consider(end);
                for side in [-1.0, 1.0] {
                    consider(offset(end, normal, side * half, along, 0.0));
                    consider(offset(end, along, outward * half, normal, side * half));
                }
            }
            // The nearest point on either face, for clear spans.
            let (foot, t) =
                crate::domain::geometry::nearest_point_on_segment(point, wall.start, wall.end);
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
                self.project
                    .walls
                    .iter()
                    .find(|wall| wall.id == opening.wall_id),
            ) else {
                continue;
            };
            let length = wall.length().max(1e-4);
            let along = (
                (wall.end.x - wall.start.x) / length,
                (wall.end.y - wall.start.y) / length,
            );
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
            for corner in terrain.outline() {
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
        assert_eq!(
            app.snap(Point::new(3.0, 0.9), Some(start)),
            Point::new(3.0, 0.9)
        );
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
        assert!(
            app.message
                .contains(&crate::domain::geometry::format_meters(3.0 - 2.0 * half))
        );
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
        assert_eq!(
            app.snap_kind(Point::new(3.0, 0.0)),
            Some(SnapKind::Endpoint)
        );
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
        let door_wall = app
            .project
            .walls
            .iter()
            .find(|w| w.id == wall)
            .unwrap()
            .clone();
        let middle = Point::new(
            (door_wall.start.x + door_wall.end.x) / 2.0,
            (door_wall.start.y + door_wall.end.y) / 2.0,
        );
        assert_eq!(app.pick(middle), Some(Selection::Opening(door)));
        assert!(matches!(
            app.pick(Point::new(2.0, 2.0)),
            Some(Selection::Room(_))
        ));
        assert_eq!(app.pick(Point::new(40.0, 40.0)), None);
    }

    #[test]
    fn dragging_a_selected_room_corner_grip_reshapes_and_records_one_undo_step() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let room_id = app
            .project
            .add_polygon_room(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(4.0, 2.0),
                    Point::new(2.0, 2.0),
                    Point::new(2.0, 4.0),
                    Point::new(0.0, 4.0),
                ],
                "Sala em L",
            )
            .unwrap();
        app.set_tool(Tool::Move);
        app.select(Selection::Room(room_id));
        assert_eq!(app.selection_grips().len(), 6);

        // Grab the inner corner (2, 2) and drag it to (3, 2).
        app.begin_item_drag(Point::new(2.0, 2.0));
        assert!(app.vertex_drag.is_some());
        app.drag_item_to(Point::new(3.0, 2.0));
        app.commit_item_drag();
        let room = app.project.rooms.iter().find(|r| r.id == room_id).unwrap();
        assert!((app.project.room_area(room).unwrap() - 13.0).abs() < 1e-4);

        // One Ctrl+Z restores the original 12 m² L-room.
        app.undo();
        let room = app.project.rooms.iter().find(|r| r.id == room_id).unwrap();
        assert!((app.project.room_area(room).unwrap() - 12.0).abs() < 1e-4);
    }

    #[test]
    fn dragging_a_drawing_handle_and_nudging_a_shared_wall_both_undo_cleanly() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let line_id = app
            .project
            .add_drawing(crate::domain::drawing::Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 0.0),
            })
            .unwrap();
        app.set_tool(Tool::Move);
        app.select(Selection::Drawing(line_id));
        app.begin_item_drag(Point::new(2.0, 0.0));
        app.drag_item_to(Point::new(5.0, 0.0));
        app.commit_item_drag();
        assert!((app.project.drawing(line_id).unwrap().shape.length() - 5.0).abs() < 1e-4);
        app.undo();
        assert!((app.project.drawing(line_id).unwrap().shape.length() - 2.0).abs() < 1e-4);

        // Shared wall nudge updates both rooms and rolls back in one undo step.
        let q1 = app
            .project
            .add_room(Point::new(0.0, 10.0), Point::new(4.0, 13.0), "Q1")
            .unwrap();
        let q2 = app
            .project
            .add_room(Point::new(4.0, 10.0), Point::new(8.0, 13.0), "Q2")
            .unwrap();
        let shared = app
            .project
            .walls
            .iter()
            .find(|w| {
                (w.start.x - 4.0).abs() < 1e-4
                    && (w.end.x - 4.0).abs() < 1e-4
                    && w.start.y.min(w.end.y) >= 9.9
            })
            .unwrap()
            .id;
        app.select(Selection::Wall(shared));
        app.nudge_selection(Point::new(0.5, 0.0));
        let area = |p: &crate::domain::model::Project, id: u64| {
            p.room_area(p.rooms.iter().find(|r| r.id == id).unwrap())
                .unwrap()
        };
        assert!((area(&app.project, q1) - 13.5).abs() < 1e-3);
        assert!((area(&app.project, q2) - 10.5).abs() < 1e-3);
        app.undo();
        assert!((area(&app.project, q1) - 12.0).abs() < 1e-3);
        assert!((area(&app.project, q2) - 12.0).abs() < 1e-3);
    }
}
