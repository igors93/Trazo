//! The modify tools and commands: copy, cut, paste and duplicate; move or
//! copy from a base point; turn about a centre; mirror across an axis;
//! offset, trim and extend walls and drawings.

use crate::app::ui::Glyph;
use crate::app::{FloorPlanApp, Selection, Tool};
use crate::domain::drawing::Shape;
use crate::domain::geometry::{Point, format_meters};
use crate::domain::model::Fragment;
use crate::domain::offset::offset_shape;
use crate::domain::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ModifyKind {
    /// Move the selection from a base point to another.
    Move,
    /// Copy the selection from a base point, as many times as clicked.
    Copy,
    /// Turn the selection about a centre.
    Rotate,
    /// Mirror the selection across an axis.
    Mirror,
    /// A parallel copy of a wall or drawing.
    Offset,
    /// Cut a wall or drawing back to the nearest crossings.
    Trim,
    /// Run an open end up to the next edge.
    Extend,
    /// Cut a wall or drawing at a point, or into equal parts.
    Split,
}

impl ModifyKind {
    pub(in crate::app) const ALL: [ModifyKind; 8] = [
        ModifyKind::Move,
        ModifyKind::Copy,
        ModifyKind::Rotate,
        ModifyKind::Mirror,
        ModifyKind::Offset,
        ModifyKind::Trim,
        ModifyKind::Extend,
        ModifyKind::Split,
    ];

    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            ModifyKind::Move => "Mover",
            ModifyKind::Copy => "Copiar",
            ModifyKind::Rotate => "Girar",
            ModifyKind::Mirror => "Espelhar",
            ModifyKind::Offset => "Deslocar",
            ModifyKind::Trim => "Aparar",
            ModifyKind::Extend => "Estender",
            ModifyKind::Split => "Dividir",
        }
    }

    pub(in crate::app) fn glyph(self) -> Glyph {
        match self {
            ModifyKind::Move => Glyph::Move,
            ModifyKind::Copy => Glyph::Copy,
            ModifyKind::Rotate => Glyph::Rotate,
            ModifyKind::Mirror => Glyph::Mirror,
            ModifyKind::Offset => Glyph::Offset,
            ModifyKind::Trim => Glyph::Trim,
            ModifyKind::Extend => Glyph::Extend,
            ModifyKind::Split => Glyph::Split,
        }
    }

    /// The single key that picks the tool, if it has one.
    pub(in crate::app) fn key(self) -> Option<(eframe::egui::Key, &'static str)> {
        use eframe::egui::Key;
        match self {
            ModifyKind::Offset => Some((Key::O, "O")),
            ModifyKind::Trim => Some((Key::A, "A")),
            ModifyKind::Extend => Some((Key::E, "E")),
            ModifyKind::Split => Some((Key::D, "D")),
            _ => None,
        }
    }

    /// Whether the tool works on the selection (the others pick what to
    /// change by clicking it).
    pub(in crate::app) fn uses_selection(self) -> bool {
        matches!(
            self,
            ModifyKind::Move | ModifyKind::Copy | ModifyKind::Rotate | ModifyKind::Mirror
        )
    }

    /// Whether "keep the original" applies.
    pub(in crate::app) fn can_keep_original(self) -> bool {
        matches!(self, ModifyKind::Rotate | ModifyKind::Mirror)
    }

    pub(in crate::app) fn hint(self) -> &'static str {
        match self {
            ModifyKind::Move => {
                "Clique no ponto base e depois no destino (ou digite 3,47 · @2;1 · 3<45)."
            }
            ModifyKind::Copy => "Clique no ponto base e em cada destino para copiar; Esc encerra.",
            ModifyKind::Rotate => {
                "Clique no centro e depois na direção do ângulo, ou digite o ângulo (45, -90) e Enter."
            }
            ModifyKind::Mirror => {
                "Clique em dois pontos do eixo de espelhamento (F8 trava na horizontal/vertical)."
            }
            ModifyKind::Offset => {
                "Digite a distância (ex.: 0,15) e Enter, clique na parede ou desenho e depois no lado."
            }
            ModifyKind::Trim => {
                "Clique no trecho de parede ou desenho a remover; ele é cortado nos cruzamentos mais próximos."
            }
            ModifyKind::Extend => {
                "Clique perto da ponta a estender; ela vai até a próxima parede, desenho ou divisa."
            }
            ModifyKind::Split => {
                "Clique na parede ou desenho onde dividir, ou escolha dividir em partes iguais na barra de opções."
            }
        }
    }
}

/// An angle typed in degrees: `45`, `-90`, `<30`, `12,5`.
pub(in crate::app) fn parse_degrees(raw: &str) -> Option<f64> {
    let text = raw.trim().trim_start_matches('<').trim().replace(',', ".");
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

impl FloorPlanApp {
    /// Picks a modify tool; those that act on the selection need one.
    pub(in crate::app) fn choose_modify_tool(&mut self, kind: ModifyKind) {
        if kind.uses_selection() && self.selection.is_none() {
            self.message = format!(
                "Selecione primeiro o que quer {} (V) e escolha a ferramenta de novo.",
                kind.label().to_lowercase()
            );
            return;
        }
        self.set_tool(Tool::Modify(kind));
    }

    /// The selection as a fragment, or why there is none.
    fn selection_fragment(&self) -> Result<Fragment, String> {
        if self.selection.is_none() {
            return Err("Selecione algo primeiro.".to_owned());
        }
        let fragment = self.project.fragment(&self.selected());
        if fragment.is_empty() {
            return Err(
                "Nada a copiar: portas e janelas são copiadas com as suas paredes, e o terreno não é copiado."
                    .to_owned(),
            );
        }
        Ok(fragment)
    }

    /// Ctrl+C.
    pub(in crate::app) fn copy_selection(&mut self) -> bool {
        match self.selection_fragment() {
            Ok(fragment) => {
                let count = fragment.len();
                self.clipboard = Some(fragment);
                self.paste_count = 0;
                self.system_clipboard_note =
                    Some(format!("Trazo: {count} elemento(s) da planta copiado(s)"));
                self.message = format!(
                    "{count} elemento(s) copiado(s). Ctrl+V cola no cursor, Ctrl+D duplica."
                );
                true
            }
            Err(error) => {
                self.message = error;
                false
            }
        }
    }

    /// Ctrl+X: copies and deletes, as one undo step.
    pub(in crate::app) fn cut_selection(&mut self) {
        if self.copy_selection() {
            let count = self.clipboard.as_ref().map_or(0, Fragment::len);
            self.delete_selection();
            self.message = format!("{count} elemento(s) recortado(s). Ctrl+V cola no cursor.");
        }
    }

    /// The step pastes and drags follow: the grid when walls come along,
    /// else 5 cm.
    fn paste_step(fragment: &Fragment) -> f64 {
        if fragment.walls.is_empty() {
            0.05
        } else {
            0.25
        }
    }

    /// Ctrl+V: the clipboard centred on the pointer, or a metre down and to
    /// the right of the last paste when the pointer is off the plan.
    pub(in crate::app) fn paste(&mut self) {
        self.paste_at(self.cursor_world);
    }

    /// Pastes the clipboard centred on `at` (kept on the grid steps of
    /// what was copied), or stepped from the original without a point.
    pub(in crate::app) fn paste_at(&mut self, at: Option<Point>) {
        let Some(fragment) = self.clipboard.clone() else {
            self.message = "Nada copiado ainda: selecione e use Ctrl+C.".to_owned();
            return;
        };
        let Some(center) = fragment.center() else {
            return;
        };
        let delta = match at {
            Some(cursor) => {
                let raw = cursor.minus(center);
                if self.snap_enabled {
                    raw.snapped_to(Self::paste_step(&fragment))
                } else {
                    raw
                }
            }
            None => {
                let step = f64::from(self.paste_count + 1);
                Point::new(step, step)
            }
        };
        self.place_copy(&fragment, &Transform::Translate(delta), "colado(s)");
        self.paste_count += 1;
    }

    /// Ctrl+D: a copy of the selection a metre down and to the right.
    pub(in crate::app) fn duplicate_selection(&mut self) {
        match self.selection_fragment() {
            Ok(fragment) => self.place_copy(
                &fragment,
                &Transform::Translate(Point::new(1.0, 1.0)),
                "duplicado(s)",
            ),
            Err(error) => self.message = error,
        }
    }

    /// Adds `fragment` transformed as one undo step and selects the copy.
    fn place_copy(&mut self, fragment: &Fragment, transform: &Transform, done: &str) {
        let previous = self.project.clone();
        let result = fragment
            .transformed(transform)
            .and_then(|copy| self.project.insert_fragment(&copy));
        match result {
            Ok(created) => {
                if self.tool != Tool::Move {
                    self.set_tool(Tool::Move);
                }
                self.record_change(previous);
                let count = fragment.len();
                self.set_selection_set(created);
                self.sync_inspector_fields();
                self.message = format!("{count} elemento(s) {done}.");
            }
            Err(error) => self.message = error,
        }
    }

    /// The centre the quick turns and flips use: the middle of the
    /// selection's box.
    fn selection_center(&self) -> Option<Point> {
        self.project
            .elements_bounds(&self.selected())
            .map(|(low, high)| low.midpoint(high).rounded())
    }

    /// Turns, mirrors or moves the selection in place, or copies it with
    /// `keep`, as one undo step.
    pub(in crate::app) fn transform_selection(
        &mut self,
        transform: &Transform,
        keep: bool,
        done: &str,
    ) {
        let items = self.selected();
        if items.is_empty() {
            self.message = "Selecione algo primeiro.".to_owned();
            return;
        }
        let previous = self.project.clone();
        let result = if keep {
            self.project.copy_elements(&items, transform).map(Some)
        } else {
            self.project
                .transform_elements(&items, transform)
                .map(|()| None)
        };
        match result {
            Ok(created) => {
                if self.project == previous {
                    self.message =
                        "Nada mudou: sem espaço nessa direção dentro do terreno.".to_owned();
                    return;
                }
                self.record_change(previous);
                if let Some(created) = created {
                    self.set_selection_set(created);
                }
                self.sync_inspector_fields();
                self.message = done.to_owned();
            }
            Err(error) => self.message = error,
        }
    }

    /// G / Shift+G and the buttons: a quarter turn about the selection's
    /// centre, clockwise or not.
    pub(in crate::app) fn rotate_selection_quarter(&mut self, clockwise: bool) {
        let Some(center) = self.selection_center() else {
            self.message = "Selecione algo para girar.".to_owned();
            return;
        };
        let degrees = if clockwise { -90.0 } else { 90.0 };
        self.transform_selection(
            &Transform::rotate_degrees(center, degrees),
            false,
            if clockwise {
                "Seleção girada 90° no sentido horário."
            } else {
                "Seleção girada 90° no sentido anti-horário."
            },
        );
    }

    /// H / Shift+H: flips the selection left-right or upside down about
    /// its centre.
    pub(in crate::app) fn mirror_selection(&mut self, left_right: bool) {
        let Some(center) = self.selection_center() else {
            self.message = "Selecione algo para espelhar.".to_owned();
            return;
        };
        let other = if left_right {
            Point::new(center.x, center.y + 1.0)
        } else {
            Point::new(center.x + 1.0, center.y)
        };
        let Ok(axis) = Transform::mirror(center, other) else {
            return;
        };
        self.transform_selection(
            &axis,
            false,
            if left_right {
                "Seleção espelhada na horizontal (esquerda ↔ direita)."
            } else {
                "Seleção espelhada na vertical (cima ↕ baixo)."
            },
        );
    }

    /// The transform a base-point tool makes from `anchor` to `point`.
    pub(in crate::app) fn modify_transform(
        kind: ModifyKind,
        anchor: Point,
        point: Point,
    ) -> Option<Transform> {
        match kind {
            ModifyKind::Move | ModifyKind::Copy => Some(Transform::Translate(point.minus(anchor))),
            ModifyKind::Rotate => (anchor.distance(point) > 1e-6).then(|| Transform::Rotate {
                center: anchor,
                angle: (point.y - anchor.y).atan2(point.x - anchor.x),
            }),
            ModifyKind::Mirror => Transform::mirror(anchor, point).ok(),
            _ => None,
        }
    }

    /// The offset distance typed in the options bar, in metres.
    pub(in crate::app) fn offset_distance(&self) -> Option<f64> {
        self.offset_distance_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|distance| distance.is_finite() && *distance > 0.0 && *distance <= 1000.0)
    }

    /// The wall or drawing under `point` that offset, trim and extend act on.
    pub(in crate::app) fn pick_editable(&self, point: Point) -> Option<Selection> {
        self.project
            .drawing_at(point, self.screen_metres(8.0))
            .map(Selection::Drawing)
            .or_else(|| {
                self.project
                    .nearest_wall(point, self.screen_metres(12.0))
                    .map(|(id, _)| Selection::Wall(id))
            })
    }

    /// A click with a modify tool at `point` (snapped for the base-point
    /// tools, as clicked for offset, trim and extend).
    pub(in crate::app) fn modify_click(&mut self, kind: ModifyKind, point: Point) {
        match kind {
            ModifyKind::Move | ModifyKind::Copy | ModifyKind::Rotate | ModifyKind::Mirror => {
                let Some(anchor) = self.modify_anchor else {
                    self.modify_anchor = Some(point);
                    self.message = match kind {
                        ModifyKind::Rotate => {
                            "Centro marcado. Clique na direção do ângulo ou digite os graus (ex.: 45) e Enter."
                        }
                        ModifyKind::Mirror => "Clique no segundo ponto do eixo.",
                        _ => "Ponto base marcado. Clique no destino ou digite a distância (3,47 · @2;1).",
                    }
                    .to_owned();
                    return;
                };
                let Some(transform) = Self::modify_transform(kind, anchor, point) else {
                    self.message = "Escolha um ponto diferente do primeiro.".to_owned();
                    return;
                };
                self.apply_modify(kind, transform);
            }
            ModifyKind::Offset => self.offset_click(point),
            ModifyKind::Trim | ModifyKind::Extend => {
                let Some(target) = self.pick_editable(point) else {
                    self.message = "Clique sobre uma parede ou um desenho 2D.".to_owned();
                    return;
                };
                let previous = self.project.clone();
                let result = if kind == ModifyKind::Trim {
                    self.project.trim_element(target, point)
                } else {
                    self.project.extend_element(target, point)
                };
                match result {
                    Ok(()) => {
                        self.record_change(previous);
                        self.revalidate_selection();
                        self.message = if kind == ModifyKind::Trim {
                            "Trecho aparado. Clique em outro ou Esc."
                        } else {
                            "Ponta estendida. Clique em outra ou Esc."
                        }
                        .to_owned();
                    }
                    Err(error) => self.message = error,
                }
            }
            ModifyKind::Split => {
                let Some(target) = self.pick_editable(point) else {
                    self.message = "Clique sobre uma parede ou um desenho 2D.".to_owned();
                    return;
                };
                let parts = self.split_parts.max(1) as usize;
                let previous = self.project.clone();
                let result = if parts >= 2 {
                    self.project.divide_element(target, parts)
                } else {
                    self.project.split_element(target, point)
                };
                match result {
                    Ok(pieces) => {
                        self.record_change(previous);
                        self.revalidate_selection();
                        self.message = format!(
                            "Dividido em {} partes. Clique em outro ou Esc.",
                            pieces.len()
                        );
                    }
                    Err(error) => self.message = error,
                }
            }
        }
    }

    /// Applies a base-point tool's `transform` to the selection.
    pub(in crate::app) fn apply_modify(&mut self, kind: ModifyKind, transform: Transform) {
        match kind {
            ModifyKind::Copy => {
                let before = self.selected();
                self.transform_selection(
                    &transform,
                    true,
                    "Cópia feita. Clique em outro destino ou Esc.",
                );
                // Copying goes on from the same base, with the originals.
                self.set_selection_set(before);
            }
            ModifyKind::Move => {
                self.modify_anchor = None;
                self.transform_selection(&transform, false, "Seleção movida.");
            }
            ModifyKind::Rotate | ModifyKind::Mirror => {
                self.modify_anchor = None;
                let keep = self.modify_keep;
                let done = match (kind, keep) {
                    (ModifyKind::Rotate, false) => "Seleção girada.",
                    (ModifyKind::Rotate, true) => "Cópia girada criada.",
                    (_, false) => "Seleção espelhada.",
                    (_, true) => "Cópia espelhada criada.",
                };
                self.transform_selection(&transform, keep, done);
            }
            _ => {}
        }
    }

    /// Offset: the first click picks the wall or drawing, the second the
    /// side.
    fn offset_click(&mut self, point: Point) {
        let Some(distance) = self.offset_distance() else {
            self.message =
                "Informe a distância de deslocamento (ex.: 0,15) na barra de opções.".to_owned();
            return;
        };
        let Some(target) = self.modify_target else {
            match self.pick_editable(point) {
                Some(target) => {
                    self.modify_target = Some(target);
                    self.message = format!(
                        "Clique no lado para onde deslocar {}.",
                        format_meters(distance)
                    );
                }
                None => self.message = "Clique sobre uma parede ou um desenho 2D.".to_owned(),
            }
            return;
        };
        let previous = self.project.clone();
        match self.project.offset_element(target, distance, point) {
            Ok(_) => {
                self.record_change(previous);
                self.modify_target = None;
                self.message = format!(
                    "Cópia paralela criada a {}. Clique em outro elemento ou Esc.",
                    format_meters(distance)
                );
            }
            Err(error) => self.message = error,
        }
    }

    /// Enter on typed input with a modify tool: an angle for turning, a
    /// distance for offset. Returns whether it was taken.
    pub(in crate::app) fn commit_modify_input(&mut self, raw: &str) -> bool {
        match self.tool {
            Tool::Modify(ModifyKind::Rotate) => {
                let Some(center) = self.modify_anchor else {
                    return false;
                };
                let Some(degrees) = parse_degrees(raw) else {
                    self.message = format!("Ângulo \"{raw}\" inválido. Use graus, ex.: 45 ou -90.");
                    return true;
                };
                self.draft_input.clear();
                self.apply_modify(
                    ModifyKind::Rotate,
                    Transform::rotate_degrees(center, degrees),
                );
                true
            }
            Tool::Modify(ModifyKind::Offset) => {
                let value = raw.trim().replace(',', ".").parse::<f64>().ok();
                match value.filter(|v| v.is_finite() && *v > 0.0 && *v <= 1000.0) {
                    Some(distance) => {
                        self.offset_distance_text = format!("{distance:.2}").replace('.', ",");
                        self.draft_input.clear();
                        self.message = format!(
                            "Distância de deslocamento: {}. Clique na parede ou desenho.",
                            format_meters(distance)
                        );
                    }
                    None => {
                        self.message = "Distância inválida: use metros, ex.: 0,15.".to_owned();
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// What a modify tool would make at the pointer, for its preview: the
    /// moved, turned or mirrored selection, or the offset copy.
    pub(in crate::app) fn modify_preview(
        &self,
        kind: ModifyKind,
        cursor: Point,
    ) -> Option<Fragment> {
        if kind == ModifyKind::Offset {
            let target = self.modify_target?;
            let distance = self.offset_distance()?;
            let shape = match target {
                Selection::Drawing(id) => self.project.drawing(id)?.shape.clone(),
                Selection::Wall(id) => {
                    let wall = self.project.walls.iter().find(|w| w.id == id)?;
                    Shape::Line {
                        start: wall.start,
                        end: wall.end,
                    }
                }
                _ => return None,
            };
            let offset = offset_shape(&shape, distance, cursor).ok()?;
            return Some(Fragment {
                drawings: vec![crate::domain::drawing::Drawing {
                    id: 0,
                    shape: offset,
                    layer_id: None,
                }],
                ..Fragment::default()
            });
        }
        let anchor = self.modify_anchor?;
        let transform = Self::modify_transform(kind, anchor, cursor)?;
        self.project
            .fragment(&self.selected())
            .transformed(&transform)
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::element::Element;
    use eframe::egui;

    fn app() -> FloorPlanApp {
        FloorPlanApp::new(&egui::Context::default())
    }

    #[test]
    fn copy_cut_paste_and_duplicate_are_undoable() {
        let mut app = app();
        let room = app
            .project
            .add_room(Point::new(0.0, 0.0), Point::new(3.0, 3.0), "Quarto")
            .unwrap();
        app.set_selection(Some(Selection::Room(room)));
        assert!(app.copy_selection());
        // The pointer at (10, 10): the copy's centre lands there.
        app.cursor_world = Some(Point::new(10.1, 10.1));
        app.paste();
        assert_eq!(app.project.rooms.len(), 2);
        let pasted = match app.selection {
            Some(Selection::Room(id)) => id,
            other => panic!("expected the pasted room, got {other:?}"),
        };
        let bounds = app.project.element_bounds(Element::Room(pasted)).unwrap();
        assert_eq!(bounds.0.midpoint(bounds.1), Point::new(10.0, 10.0));
        app.undo();
        assert_eq!(app.project.rooms.len(), 1);

        // Off the plan, pastes step a metre away each time.
        app.cursor_world = None;
        app.paste();
        app.paste();
        assert_eq!(app.project.rooms.len(), 3);

        app.set_selection(Some(Selection::Room(room)));
        app.duplicate_selection();
        assert_eq!(app.project.rooms.len(), 4);
        app.set_selection(Some(Selection::Room(room)));
        app.cut_selection();
        assert_eq!(app.project.rooms.len(), 3);
        app.undo();
        assert_eq!(app.project.rooms.len(), 4);
    }

    #[test]
    fn base_point_tools_move_copy_rotate_and_mirror() {
        let mut app = app();
        let line = app
            .project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 0.0),
            })
            .unwrap();
        // Without a selection the tools wait for one.
        app.choose_modify_tool(ModifyKind::Move);
        assert_eq!(app.tool, Tool::Move);
        app.set_selection(Some(Selection::Drawing(line)));
        app.choose_modify_tool(ModifyKind::Move);
        assert_eq!(app.tool, Tool::Modify(ModifyKind::Move));
        assert_eq!(app.selection, Some(Selection::Drawing(line)));
        app.apply_canvas_point(Point::new(0.0, 0.0));
        app.draft_input = "@1;1".to_owned();
        app.commit_numeric_input();
        assert_eq!(
            app.project.drawing(line).unwrap().shape,
            Shape::Line {
                start: Point::new(1.0, -1.0),
                end: Point::new(3.0, -1.0)
            }
        );

        // Copy twice from one base point.
        app.choose_modify_tool(ModifyKind::Copy);
        app.apply_canvas_point(Point::new(1.0, -1.0));
        app.apply_canvas_point(Point::new(1.0, 1.0));
        app.apply_canvas_point(Point::new(1.0, 3.0));
        assert_eq!(app.project.drawings.len(), 3);
        assert_eq!(app.selection, Some(Selection::Drawing(line)));

        // Rotate 90° typed, about the line's start.
        app.choose_modify_tool(ModifyKind::Rotate);
        app.apply_canvas_point(Point::new(1.0, -1.0));
        app.draft_input = "90".to_owned();
        app.commit_numeric_input();
        assert_eq!(
            app.project.drawing(line).unwrap().shape,
            Shape::Line {
                start: Point::new(1.0, -1.0),
                end: Point::new(1.0, -3.0)
            }
        );

        // Mirror across x = 0, keeping the original.
        app.choose_modify_tool(ModifyKind::Mirror);
        app.modify_keep = true;
        app.apply_canvas_point(Point::new(0.0, 0.0));
        app.apply_canvas_point(Point::new(0.0, 5.0));
        assert_eq!(app.project.drawings.len(), 4);
        let mirrored = app.project.drawings.last().unwrap();
        assert_eq!(
            mirrored.shape,
            Shape::Line {
                start: Point::new(-1.0, -1.0),
                end: Point::new(-1.0, -3.0)
            }
        );
        // Each of those was one undo step.
        app.undo();
        assert_eq!(app.project.drawings.len(), 3);
    }

    #[test]
    fn quick_turns_and_flips_work_on_the_selection() {
        let mut app = app();
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        app.set_selection(Some(Selection::Wall(wall)));
        app.rotate_selection_quarter(true);
        let turned = &app.project.walls[0];
        assert_eq!(turned.start, Point::new(2.0, -2.0));
        assert_eq!(turned.end, Point::new(2.0, 2.0));
        app.mirror_selection(false);
        let flipped = &app.project.walls[0];
        assert_eq!(flipped.start, Point::new(2.0, 2.0));
        assert_eq!(flipped.end, Point::new(2.0, -2.0));
        assert_eq!(parse_degrees("<12,5"), Some(12.5));
        assert_eq!(parse_degrees("abc"), None);
    }

    #[test]
    fn offset_trim_and_extend_tools_pick_by_clicking() {
        let mut app = app();
        app.set_tool(Tool::Modify(ModifyKind::Offset));
        let line = app
            .project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 0.0),
            })
            .unwrap();
        app.draft_input = "0,5".to_owned();
        app.commit_numeric_input();
        assert_eq!(app.offset_distance(), Some(0.5));
        app.modify_click(ModifyKind::Offset, Point::new(2.0, 0.0));
        assert_eq!(app.modify_target, Some(Selection::Drawing(line)));
        let preview = app.modify_preview(ModifyKind::Offset, Point::new(2.0, 2.0));
        assert_eq!(preview.map(|fragment| fragment.drawings.len()), Some(1));
        app.modify_click(ModifyKind::Offset, Point::new(2.0, 2.0));
        assert_eq!(app.project.drawings.len(), 2);
        assert!(app.modify_target.is_none());

        // A crossing line, then trim the right end back to it.
        app.project
            .add_drawing(Shape::Line {
                start: Point::new(3.0, -1.0),
                end: Point::new(3.0, 1.0),
            })
            .unwrap();
        app.set_tool(Tool::Modify(ModifyKind::Trim));
        app.modify_click(ModifyKind::Trim, Point::new(3.6, 0.0));
        assert_eq!(app.project.drawing(line).unwrap().shape.length(), 3.0);
        app.set_tool(Tool::Modify(ModifyKind::Extend));
        app.modify_click(ModifyKind::Extend, Point::new(9.0, 9.0));
        assert!(app.message.contains("parede ou um desenho"));
        assert!(app.history.can_undo());
    }
}
