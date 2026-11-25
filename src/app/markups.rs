//! Review markup interaction, deliberately independent of architectural editing.

use super::{
    FloorPlanApp, Tool,
    canvas::{self, PaintOptions, ViewTransform},
};
use crate::domain::{
    geometry::Point,
    markups::{MAX_POINTS, Markup, MarkupKind},
};
use eframe::egui::{self, Painter, Response, Stroke, Ui};

type MarkupContext = (u64, crate::domain::guid::Guid, Option<u64>);

pub(super) struct MarkupUiState {
    pub color: [u8; 3],
    pub comment: String,
    pub selected: Option<u64>,
    pub edit: Option<Markup>,
    pub search: String,
    pub context_elements: Vec<crate::domain::element::Element>,
    owner: Option<MarkupContext>,
    points: Vec<Point>,
    drag: Option<(u64, Point, Option<usize>)>,
}

impl Default for MarkupUiState {
    fn default() -> Self {
        Self {
            color: [220, 65, 65],
            comment: String::new(),
            selected: None,
            edit: None,
            search: String::new(),
            context_elements: Vec::new(),
            owner: None,
            points: Vec::new(),
            drag: None,
        }
    }
}
impl MarkupUiState {
    pub fn cancel_draft(&mut self) {
        self.points.clear();
        self.drag = None;
    }
}

impl FloorPlanApp {
    fn markup_context(&self) -> MarkupContext {
        (
            self.active_project_id(),
            self.project.guids.project,
            self.project.floors.active_floor_id,
        )
    }

    /// Numeric element IDs may coincide in different documents. A pending
    /// comment must never be applied to the new document's unrelated mark.
    pub(super) fn sync_markup_context(&mut self) {
        let owner = self.markup_context();
        if self.markup_state.owner != Some(owner) {
            self.markup_state.cancel_draft();
            self.markup_state.selected = None;
            self.markup_state.edit = None;
            self.markup_state.context_elements.clear();
            self.markup_state.comment.clear();
            self.markup_state.search.clear();
            self.markup_state.owner = Some(owner);
        }
    }

    pub(super) fn current_markup_selection(&self) -> Option<u64> {
        self.markup_state.selected.filter(|id| {
            self.markup_state.owner == Some(self.markup_context())
                && self.project.markups.iter().any(|m| m.id == *id)
        })
    }

    pub(super) fn insert_markup(&mut self, kind: MarkupKind, points: Vec<Point>) {
        let before = self.project.clone();
        match self.project.add_markup(Markup {
            id: 0,
            kind,
            points,
            color: self.markup_state.color,
            comment: self.markup_state.comment.clone(),
            resolved: false,
            linked_elements: vec![],
        }) {
            Ok(id) => {
                self.markup_state.owner = Some(self.markup_context());
                self.project.markup_settings.visible = true;
                self.markup_state.selected = Some(id);
                self.record_change(before);
                self.message = format!(
                    "Marcação M{id} criada. Abra Lista e comentários para acrescentar detalhes."
                );
            }
            Err(error) => self.message = error,
        }
    }
    pub(super) fn mark_selection(&mut self) {
        let elements: Vec<_> = self
            .markup_state
            .context_elements
            .iter()
            .copied()
            .filter(|e| self.project.element_exists(*e))
            .collect();
        let Some((a, b)) = self.project.elements_bounds(&elements) else {
            self.message = "Selecione elementos da planta antes de abrir Marcações.".into();
            return;
        };
        let before = self.project.clone();
        let m = Markup {
            id: 0,
            kind: MarkupKind::Rectangle,
            points: vec![
                a.translated(Point::new(-0.15, -0.15)),
                b.translated(Point::new(0.15, 0.15)),
            ],
            color: self.markup_state.color,
            comment: self.markup_state.comment.clone(),
            resolved: false,
            linked_elements: elements,
        };
        match self.project.add_markup(m) {
            Ok(id) => {
                self.markup_state.owner = Some(self.markup_context());
                self.project.markup_settings.visible = true;
                self.markup_state.selected = Some(id);
                self.record_change(before);
                self.message = format!("Marcação M{id} vinculada à seleção.");
            }
            Err(e) => self.message = e,
        }
    }
    pub(super) fn markup_canvas(
        &mut self,
        ui: &mut Ui,
        response: &Response,
        painter: &Painter,
        view: ViewTransform,
        world: Option<Point>,
    ) {
        self.sync_markup_context();
        let Tool::Markup(kind) = self.tool else {
            return;
        };
        if !self
            .project
            .markups
            .iter()
            .any(|m| Some(m.id) == self.markup_state.selected)
        {
            self.markup_state.selected = None;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
            self.markup_state.cancel_draft();
        }
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let released = ui.input(|i| i.pointer.primary_released());
        let down = ui.input(|i| i.pointer.primary_down());
        if kind.is_none() {
            if pressed && let Some(p) = world {
                let handle = self
                    .project
                    .markups
                    .iter()
                    .find(|m| {
                        Some(m.id) == self.markup_state.selected
                            && self.project.markup_shown(m)
                            && m.kind != MarkupKind::Freehand
                    })
                    .and_then(|m| {
                        m.points
                            .iter()
                            .position(|v| v.distance(p) < view.metres(9.0))
                            .map(|i| (m.id, i))
                    });
                let selected = handle.map(|(id, _)| id).or_else(|| {
                    self.project
                        .markups
                        .iter()
                        .rev()
                        .find(|m| self.project.markup_shown(m) && m.hit(p, view.metres(8.0)))
                        .map(|m| m.id)
                });
                self.markup_state.selected = selected;
                self.markup_state.edit = None;
                self.markup_state.drag = selected.map(|id| (id, p, handle.map(|(_, index)| index)));
            }
            if released
                && let Some((id, start, handle)) = self.markup_state.drag.take()
                && let Some(p) = world
                && start.distance(p) > view.metres(3.0)
            {
                let before = self.project.clone();
                if let Some(mut m) = self.project.markups.iter().find(|m| m.id == id).cloned() {
                    if let Some(index) = handle {
                        m.points[index] = p;
                    } else {
                        m.points
                            .iter_mut()
                            .for_each(|v| *v = v.translated(p.minus(start)));
                    }
                    match self.project.update_markup(m) {
                        Ok(()) => {
                            self.record_change(before);
                        }
                        Err(e) => self.message = e,
                    }
                }
            }
            if !ui.ctx().egui_wants_keyboard_input()
                && ui.input(|i| i.key_pressed(egui::Key::Delete))
                && let Some(id) = self.markup_state.selected.take()
            {
                let before = self.project.clone();
                if self.project.delete_markup(id) {
                    self.record_change(before);
                }
            }
        } else if let Some(kind) = kind {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if kind == MarkupKind::Freehand {
                if pressed && let Some(p) = world {
                    self.markup_state.points = vec![p];
                }
                if down
                    && let Some(p) = world
                    && self.markup_state.points.len() < MAX_POINTS
                    && self
                        .markup_state
                        .points
                        .last()
                        .is_some_and(|last| last.distance(p) > view.metres(2.0))
                {
                    self.markup_state.points.push(p);
                }
                if released && !self.markup_state.points.is_empty() {
                    if let Some(p) = world
                        && self.markup_state.points.len() < MAX_POINTS
                        && self
                            .markup_state
                            .points
                            .last()
                            .is_some_and(|last| last.distance(p) > 0.001)
                    {
                        self.markup_state.points.push(p);
                    }
                    let points = std::mem::take(&mut self.markup_state.points);
                    self.insert_markup(kind, points);
                }
            } else {
                if response.drag_started()
                    && self.markup_state.points.is_empty()
                    && let Some(pos) = ui.input(|i| i.pointer.press_origin())
                {
                    self.markup_state.points.push(view.to_world(pos));
                }
                if response.drag_stopped()
                    && let Some(start) = self.markup_state.points.pop()
                    && let Some(end) = world
                {
                    self.insert_markup(kind, vec![start, end]);
                } else if response.clicked()
                    && let Some(p) = world
                {
                    if let Some(start) = self.markup_state.points.pop() {
                        self.insert_markup(kind, vec![start, p]);
                    } else {
                        self.markup_state.points.push(p);
                    }
                }
            }
        }
        let shown = self
            .project
            .for_view(self.discipline, &self.active_hidden_layers);
        let textures = self.underlay_state.texture_pairs();
        canvas::paint(
            painter,
            view,
            &shown,
            PaintOptions {
                show_grid: self.show_grid,
                dimensions: self.show_dimensions,
                underlay_textures: &textures,
                ..Default::default()
            },
        );
        drop(shown);
        self.paint_xrefs_overlay(painter, view);
        self.paint_compare_overlay(painter, view);
        // Marks are painted here as well as on ordinary plan views; canvas::paint
        // already drew them, so this pass only adds selection and previews.
        if let Some(m) = self
            .project
            .markups
            .iter()
            .find(|m| Some(m.id) == self.markup_state.selected)
            && self.project.markup_shown(m)
        {
            let (a, b) = m.bounds();
            painter.rect_stroke(
                egui::Rect::from_two_pos(view.to_screen(a), view.to_screen(b)).expand(5.0),
                3.0,
                Stroke::new(1.0, super::theme::SELECTION),
                egui::StrokeKind::Outside,
            );
            if kind.is_none() && m.kind != MarkupKind::Freehand {
                for p in &m.points {
                    painter.rect_filled(
                        egui::Rect::from_center_size(view.to_screen(*p), egui::vec2(7.0, 7.0)),
                        1.0,
                        super::theme::SELECTION,
                    );
                }
            }
        }
        if let Some(kind) = kind
            && !self.markup_state.points.is_empty()
            && let Some(end) = world
        {
            let mut points = self.markup_state.points.clone();
            if kind != MarkupKind::Freehand {
                points.truncate(1);
                points.push(end);
            }
            let m = Markup {
                id: 0,
                kind,
                points,
                color: self.markup_state.color,
                comment: String::new(),
                resolved: false,
                linked_elements: vec![],
            };
            if m.validate().is_ok() {
                canvas::paint_markup(painter, view, &m, false);
            }
        }
        if let Some((id, start, handle)) = self.markup_state.drag
            && down
            && let Some(end) = world
            && let Some(mut m) = self.project.markups.iter().find(|m| m.id == id).cloned()
        {
            if let Some(index) = handle {
                m.points[index] = end;
            } else {
                m.points
                    .iter_mut()
                    .for_each(|p| *p = p.translated(end.minus(start)));
            }
            canvas::paint_markup(painter, view, &m, false);
        }
        if !down
            && let Some(p) = world
            && let Some(m) = self
                .project
                .markups
                .iter()
                .rev()
                .find(|m| self.project.markup_shown(m) && m.hit(p, view.metres(8.0)))
        {
            response.clone().on_hover_text(format!(
                "{} · {}\n{}\n{}",
                m.label(),
                m.kind.label(),
                if m.resolved { "Resolvida" } else { "Pendente" },
                if m.comment.is_empty() {
                    "Sem comentário"
                } else {
                    &m.comment
                }
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{element::Element, model::Project};
    fn frame(app: &mut FloorPlanApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
            },
        );
        out.textures_delta.clear();
    }
    fn pointer(app: &mut FloorPlanApp, ctx: &egui::Context, pos: egui::Pos2, down: Option<bool>) {
        let mut events = vec![egui::Event::PointerMoved(pos)];
        if let Some(pressed) = down {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        frame(app, ctx, events);
    }
    fn click(app: &mut FloorPlanApp, ctx: &egui::Context, pos: egui::Pos2) {
        pointer(app, ctx, pos, Some(true));
        pointer(app, ctx, pos, Some(false));
    }

    #[test]
    fn pointer_creates_moves_resizes_and_cancels_marks_without_editing_walls() {
        let ctx = egui::Context::default();
        let mut app = FloorPlanApp::new(&ctx);
        app.project = Project::default();
        app.fit_pending = false;
        app.scale = 40.0;
        app.project
            .add_wall(Point::new(-3.0, -2.0), Point::new(3.0, -2.0))
            .unwrap();
        let walls = app.project.walls.clone();
        app.set_tool(Tool::Markup(Some(MarkupKind::Rectangle)));
        frame(&mut app, &ctx, vec![]);
        let a = egui::pos2(300.0, 250.0);
        let b = egui::pos2(460.0, 350.0);
        pointer(&mut app, &ctx, a, None);
        click(&mut app, &ctx, a);
        click(&mut app, &ctx, b);
        assert_eq!(app.project.markups.len(), 1);
        let original = app.project.markups[0].clone();
        let view = ViewTransform {
            rect: app.last_canvas_rect.unwrap(),
            scale: app.scale,
            pan: app.pan,
        };
        assert!(original.points[0].near(view.to_world(a), 1e-6));
        app.set_tool(Tool::Markup(None));
        let center = egui::pos2(380.0, 300.0);
        pointer(&mut app, &ctx, center, Some(true));
        pointer(&mut app, &ctx, center + egui::vec2(40.0, 20.0), None);
        pointer(&mut app, &ctx, center + egui::vec2(40.0, 20.0), Some(false));
        assert!(
            app.project.markups[0].points[0]
                .near(original.points[0].translated(Point::new(1.0, 0.5)), 1e-6)
        );
        let corner = view.to_screen(app.project.markups[0].points[1]);
        pointer(&mut app, &ctx, corner, Some(true));
        pointer(&mut app, &ctx, corner + egui::vec2(40.0, 0.0), None);
        pointer(&mut app, &ctx, corner + egui::vec2(40.0, 0.0), Some(false));
        assert!(
            app.project.markups[0].points[1]
                .near(original.points[1].translated(Point::new(2.0, 0.5)), 1e-6)
        );
        app.set_tool(Tool::Markup(Some(MarkupKind::Circle)));
        click(&mut app, &ctx, a);
        assert_eq!(app.markup_state.points.len(), 1);
        app.cancel_creation();
        assert!(app.markup_state.points.is_empty());
        app.set_tool(Tool::Markup(Some(MarkupKind::Freehand)));
        pointer(&mut app, &ctx, a, Some(true));
        pointer(&mut app, &ctx, a + egui::vec2(30.0, 10.0), None);
        pointer(&mut app, &ctx, a + egui::vec2(60.0, 5.0), None);
        pointer(&mut app, &ctx, a + egui::vec2(65.0, 10.0), Some(false));
        assert_eq!(app.project.markups.len(), 2);
        assert_eq!(app.project.walls, walls);
    }

    #[test]
    fn marking_the_previous_selection_keeps_explicit_element_references() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = Project::default();
        let id = app
            .project
            .add_wall(Point::ORIGIN, Point::new(4.0, 0.0))
            .unwrap();
        app.set_selection(Some(Element::Wall(id)));
        app.set_tool(Tool::Markup(None));
        app.mark_selection();
        assert_eq!(app.project.markups.len(), 1);
        assert_eq!(
            app.project.markups[0].linked_elements,
            vec![Element::Wall(id)]
        );
        app.undo();
        assert!(app.project.markups.is_empty());
        assert_eq!(app.project.walls.len(), 1);
    }

    #[test]
    fn pending_comments_and_selection_do_not_cross_projects_with_reused_numeric_ids() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = Project::default();
        app.set_tool(Tool::Markup(None));
        app.insert_markup(
            MarkupKind::Rectangle,
            vec![Point::ORIGIN, Point::new(2.0, 2.0)],
        );
        let original = app.project.markups[0].clone();
        app.markup_state.edit = Some(original.clone());
        app.markup_state.edit.as_mut().unwrap().comment =
            "Unfinished comment from another project".into();
        let mut other = Project::default();
        let mut mark = original.clone();
        mark.comment = "Other project's reminder".into();
        let id = other.add_markup(mark).unwrap();
        assert_eq!(original.id, id);
        assert_ne!(app.project.guids.project, other.guids.project);
        app.project = other;
        let output = app.run_mcp_tool("obter_planta", &serde_json::json!({}));
        assert!(!output.is_error);
        assert!(output.structured.unwrap()["marcacao_selecionada"].is_null());
        assert!(app.markup_state.edit.is_none());
        assert!(app.markup_state.selected.is_none());
        assert_eq!(app.project.markups[0].comment, "Other project's reminder");
    }
}
