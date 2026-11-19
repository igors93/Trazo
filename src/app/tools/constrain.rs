//! The constraint tools: click two lines (walls or drawn lines) to keep
//! them parallel or perpendicular, two points to keep them together, or two
//! points or two parallel lines to fix the distance between them (a line
//! clicked twice fixes its length).

use crate::app::ui::Glyph;
use crate::app::{FloorPlanApp, Selection, Tool};
use crate::domain::drawing::Shape;
use crate::domain::geometry::{Point, format_meters};
use crate::domain::model::{ConstraintKind, Entity, Vertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ConstraintTool {
    Parallel,
    Perpendicular,
    Coincident,
    Distance,
}

/// What the first click of a constraint tool picked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum ConstraintPick {
    Line(Entity),
    Point(Vertex),
}

impl ConstraintTool {
    pub(in crate::app) const ALL: [ConstraintTool; 4] = [
        ConstraintTool::Parallel,
        ConstraintTool::Perpendicular,
        ConstraintTool::Coincident,
        ConstraintTool::Distance,
    ];

    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            ConstraintTool::Parallel => "Paralelas",
            ConstraintTool::Perpendicular => "Perpendiculares",
            ConstraintTool::Coincident => "Coincidentes",
            ConstraintTool::Distance => "Distância fixa",
        }
    }

    pub(in crate::app) fn glyph(self) -> Glyph {
        match self {
            ConstraintTool::Parallel => Glyph::Parallel,
            ConstraintTool::Perpendicular => Glyph::Perpendicular,
            ConstraintTool::Coincident => Glyph::Coincident,
            ConstraintTool::Distance => Glyph::FixedDistance,
        }
    }

    pub(in crate::app) fn hint(self) -> &'static str {
        match self {
            ConstraintTool::Parallel => {
                "Clique na linha de referência e depois na que deve ficar paralela a ela (paredes ou linhas)."
            }
            ConstraintTool::Perpendicular => {
                "Clique na linha de referência e depois na que deve ficar perpendicular a ela."
            }
            ConstraintTool::Coincident => {
                "Clique em dois pontos (pontas de paredes ou de desenhos): o segundo vai até o primeiro e passa a segui-lo."
            }
            ConstraintTool::Distance => {
                "Clique em dois pontos, em duas linhas paralelas, ou duas vezes na mesma linha para fixar o comprimento."
            }
        }
    }
}

impl FloorPlanApp {
    /// The straight line under `point`: a drawn line or a wall.
    pub(in crate::app) fn pick_line(&self, point: Point) -> Option<Entity> {
        let drawn = self
            .project
            .drawing_at(point, self.screen_metres(8.0))
            .filter(|id| {
                matches!(
                    self.project.drawing(*id).map(|d| &d.shape),
                    Some(Shape::Line { .. })
                )
            })
            .map(Entity::Drawing);
        drawn.or_else(|| {
            self.project
                .nearest_wall(point, self.screen_metres(12.0))
                .map(|(id, _)| Entity::Wall(id))
        })
    }

    /// The constrainable point nearest `point`, within reach.
    pub(in crate::app) fn pick_vertex(&self, point: Point) -> Option<(Vertex, Point)> {
        let reach = self.screen_metres(10.0).clamp(0.05, 0.4);
        self.project
            .walls
            .iter()
            .map(|wall| Entity::Wall(wall.id))
            .chain(self.project.drawings.iter().map(|d| Entity::Drawing(d.id)))
            .flat_map(|entity| self.project.entity_vertices(entity))
            .filter(|(_, at)| at.distance(point) <= reach)
            .min_by(|a, b| a.1.distance(point).total_cmp(&b.1.distance(point)))
    }

    /// What a click of `tool` at `point` picks.
    fn constraint_pick_at(&self, tool: ConstraintTool, point: Point) -> Option<ConstraintPick> {
        match tool {
            ConstraintTool::Parallel | ConstraintTool::Perpendicular => {
                self.pick_line(point).map(ConstraintPick::Line)
            }
            ConstraintTool::Coincident => self
                .pick_vertex(point)
                .map(|(vertex, _)| ConstraintPick::Point(vertex)),
            ConstraintTool::Distance => self
                .pick_vertex(point)
                .map(|(vertex, _)| ConstraintPick::Point(vertex))
                .or_else(|| self.pick_line(point).map(ConstraintPick::Line)),
        }
    }

    /// The element a pick highlights.
    pub(in crate::app) fn constraint_pick_element(pick: ConstraintPick) -> Selection {
        match pick {
            ConstraintPick::Line(entity) => entity.element(),
            ConstraintPick::Point(vertex) => vertex.entity.element(),
        }
    }

    /// What `tool` makes of the two picks, with the distances as they are.
    fn constraint_from(
        &self,
        tool: ConstraintTool,
        first: ConstraintPick,
        second: ConstraintPick,
    ) -> Result<ConstraintKind, String> {
        use ConstraintPick::{Line, Point as At};
        match (tool, first, second) {
            (ConstraintTool::Parallel, Line(a), Line(b)) => Ok(ConstraintKind::Parallel { a, b }),
            (ConstraintTool::Perpendicular, Line(a), Line(b)) => {
                Ok(ConstraintKind::Perpendicular { a, b })
            }
            (ConstraintTool::Coincident, At(a), At(b)) => Ok(ConstraintKind::Coincident { a, b }),
            (ConstraintTool::Distance, At(a), At(b)) => {
                let (pa, pb) = (
                    self.project
                        .vertex_point(a)
                        .ok_or("Ponto não encontrado.")?,
                    self.project
                        .vertex_point(b)
                        .ok_or("Ponto não encontrado.")?,
                );
                Ok(ConstraintKind::PointDistance {
                    a,
                    b,
                    distance: pa.distance(pb),
                })
            }
            // The same line twice: its length.
            (ConstraintTool::Distance, Line(a), Line(b)) if a == b => {
                let (start, end) = self.project.entity_line(a).ok_or("Linha não encontrada.")?;
                Ok(ConstraintKind::PointDistance {
                    a: Vertex {
                        entity: a,
                        index: 0,
                    },
                    b: Vertex {
                        entity: a,
                        index: 1,
                    },
                    distance: start.distance(end),
                })
            }
            (ConstraintTool::Distance, Line(a), Line(b)) => {
                let (a0, a1) = self.project.entity_line(a).ok_or("Linha não encontrada.")?;
                let (b0, b1) = self.project.entity_line(b).ok_or("Linha não encontrada.")?;
                let along = a1.minus(a0).normalized().ok_or("Linha sem comprimento.")?;
                let normal = Point::new(-along.y, along.x);
                Ok(ConstraintKind::LineDistance {
                    a,
                    b,
                    distance: b0.midpoint(b1).minus(a0).dot(normal).abs(),
                })
            }
            (ConstraintTool::Distance, _, _) => {
                Err("Escolha dois pontos, ou duas linhas paralelas.".to_owned())
            }
            _ => Err("Escolha dois elementos do mesmo tipo.".to_owned()),
        }
    }

    /// A click with a constraint tool: the first picks the reference, the
    /// second makes the constraint.
    pub(in crate::app) fn constrain_click(&mut self, tool: ConstraintTool, point: Point) {
        let Some(pick) = self.constraint_pick_at(tool, point) else {
            self.message = match tool {
                ConstraintTool::Coincident => "Clique perto da ponta de uma parede ou desenho.",
                ConstraintTool::Distance => "Clique num ponto (ponta) ou numa linha.",
                _ => "Clique numa parede ou numa linha desenhada.",
            }
            .to_owned();
            return;
        };
        let Some(first) = self.constraint_pick else {
            self.constraint_pick = Some(pick);
            self.message = match tool {
                ConstraintTool::Coincident => "Agora clique no ponto que vai até ele.",
                ConstraintTool::Distance => {
                    "Agora o segundo ponto, ou a segunda linha (a mesma linha fixa o comprimento)."
                }
                _ => "Agora clique na linha que vai se ajustar a ela.",
            }
            .to_owned();
            return;
        };
        self.constraint_pick = None;
        match self.constraint_from(tool, first, pick) {
            Ok(kind) => self.create_constraint(kind),
            Err(error) => self.message = error,
        }
    }

    fn create_constraint(&mut self, kind: ConstraintKind) {
        let previous = self.project.clone();
        match self.project.add_constraint(kind) {
            Ok(_) => {
                if !self.record_change(previous) {
                    return;
                }
                self.revalidate_selection();
                self.sync_inspector_fields();
                self.message = format!(
                    "Restrição criada: {}. Ela é mantida nas próximas edições.",
                    kind.label().to_lowercase()
                );
            }
            Err(error) => self.message = error,
        }
    }

    /// Removes one constraint (the geometry stays).
    pub(in crate::app) fn remove_constraint(&mut self, id: u64) {
        let previous = self.project.clone();
        if self.project.remove_constraint(id) {
            if !self.record_change(previous) {
                return;
            }
            self.sync_inspector_fields();
            self.message = "Restrição removida.".to_owned();
        }
    }

    /// Removes every constraint on the selected walls and drawings.
    pub(in crate::app) fn remove_selection_constraints(&mut self) {
        let entities: Vec<Entity> = self.selected().into_iter().filter_map(Entity::of).collect();
        let previous = self.project.clone();
        self.project.constraints.retain(|constraint| {
            !constraint
                .kind
                .entities()
                .iter()
                .any(|e| entities.contains(e))
        });
        let removed = previous.constraints.len() - self.project.constraints.len();
        if removed == 0 {
            self.message = "A seleção não tem restrições.".to_owned();
            return;
        }
        if !self.record_change(previous) {
            return;
        }
        self.sync_inspector_fields();
        self.message = format!("{removed} restrição(ões) removida(s).");
    }

    /// Applies the distance typed for constraint `id`.
    pub(in crate::app) fn apply_constraint_distance(&mut self, id: u64) {
        let Some(text) = self.constraint_texts.get(&id) else {
            return;
        };
        let Some(distance) = text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
        else {
            self.message = "Informe a distância em metros (ex.: 1,20).".to_owned();
            return;
        };
        let current = self
            .project
            .constraints
            .iter()
            .find(|constraint| constraint.id == id)
            .and_then(|constraint| constraint.kind.distance());
        if current.is_some_and(|value| (value - distance).abs() < 1e-6) {
            return;
        }
        let previous = self.project.clone();
        match self.project.set_constraint_distance(id, distance) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message = format!("Distância fixada em {}.", format_meters(distance));
            }
            Err(error) => {
                self.message = error;
                self.sync_inspector_fields();
            }
        }
    }

    /// The constraint texts follow the selected element's constraints.
    pub(in crate::app) fn sync_constraint_fields(&mut self) {
        self.constraint_texts.clear();
        for constraint in &self.project.constraints {
            if let Some(distance) = constraint.kind.distance() {
                self.constraint_texts
                    .insert(constraint.id, format!("{distance:.3}").replace('.', ","));
            }
        }
    }

    /// A name for a constrained line, for lists.
    pub(in crate::app) fn entity_name(&self, entity: Entity) -> String {
        match entity {
            Entity::Wall(id) => self
                .project
                .walls
                .iter()
                .position(|wall| wall.id == id)
                .map(|index| format!("Parede {:02}", index + 1))
                .unwrap_or_else(|| "Parede".to_owned()),
            Entity::Drawing(id) => self
                .project
                .drawing(id)
                .map(|drawing| drawing.shape.label().to_owned())
                .unwrap_or_else(|| "Desenho".to_owned()),
        }
    }

    /// Where each constraint's mark goes on the plan, its text, and
    /// whether it concerns the selection.
    pub(in crate::app) fn constraint_marks(&self) -> Vec<(Point, String, bool)> {
        let tool_on = matches!(self.tool, Tool::Constrain(_));
        self.project
            .constraints
            .iter()
            .filter_map(|constraint| {
                let kind = constraint.kind;
                let concerned = kind
                    .entities()
                    .iter()
                    .any(|entity| self.is_selected(entity.element()));
                if !tool_on && !concerned {
                    return None;
                }
                let at = match kind {
                    ConstraintKind::Parallel { b, .. }
                    | ConstraintKind::Perpendicular { b, .. }
                    | ConstraintKind::LineDistance { b, .. } => {
                        let (start, end) = self.project.entity_line(b)?;
                        start.midpoint(end)
                    }
                    ConstraintKind::Coincident { a, .. } => self.project.vertex_point(a)?,
                    ConstraintKind::PointDistance { a, b, .. } => self
                        .project
                        .vertex_point(a)?
                        .midpoint(self.project.vertex_point(b)?),
                };
                Some((at, kind.symbol(), concerned))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn two_clicks_make_each_kind_of_constraint() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let line = app
            .project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 2.0),
                end: Point::new(4.0, 2.5),
            })
            .unwrap();

        app.set_tool(Tool::Constrain(ConstraintTool::Parallel));
        app.constrain_click(ConstraintTool::Parallel, Point::new(2.0, 0.0));
        assert_eq!(
            app.constraint_pick,
            Some(ConstraintPick::Line(Entity::Wall(wall)))
        );
        app.constrain_click(ConstraintTool::Parallel, Point::new(2.0, 2.25));
        assert_eq!(app.project.constraints.len(), 1);
        let Shape::Line { start, end } = app.project.drawing(line).unwrap().shape else {
            panic!("a line");
        };
        assert!((start.y - end.y).abs() < 1e-4);
        assert!(app.history.can_undo());

        // The same line twice fixes its length; dragging the wall's end
        // then keeps it.
        app.set_tool(Tool::Constrain(ConstraintTool::Distance));
        app.constrain_click(ConstraintTool::Distance, Point::new(2.0, 0.05));
        app.constrain_click(ConstraintTool::Distance, Point::new(1.0, 0.05));
        assert_eq!(app.project.constraints.len(), 2);
        assert!(
            app.project.constraints[1]
                .kind
                .label()
                .starts_with("Comprimento fixo")
        );
        app.set_tool(Tool::Move);
        app.set_selection(Some(Selection::Wall(wall)));
        app.wall_length_text = "6,00".to_owned();
        app.apply_wall_length(wall);
        let length = app.project.walls[0].length();
        assert!((length - 4.0).abs() < 1e-4, "{length}");
        assert!(app.message.contains("restrição"));

        // A typed distance moves the free end instead.
        app.sync_constraint_fields();
        let id = app.project.constraints[1].id;
        app.constraint_texts.insert(id, "5".to_owned());
        app.apply_constraint_distance(id);
        assert!((app.project.walls[0].length() - 5.0).abs() < 1e-4);

        // Removing the selection's constraints frees it.
        app.remove_selection_constraints();
        assert!(app.project.constraints.is_empty());
        assert!(
            !app.constraint_marks()
                .iter()
                .any(|(_, _, concerned)| *concerned)
        );
    }
}
