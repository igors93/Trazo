//! Geometric constraints kept with the project: two lines parallel or
//! perpendicular, two points together, a fixed distance between two points
//! (the length of a line when they are its ends) or between two parallel
//! lines. After every edit the elements the person did not touch follow,
//! so the constraints hold; an edit that cannot keep them is refused.

use serde::{Deserialize, Serialize};

use super::Project;
use crate::domain::drawing::Shape;
use crate::domain::element::Element;
use crate::domain::geometry::{JOIN_TOLERANCE, POINT_TOLERANCE, Point, format_meters};

/// Below this sine two directions count as the same (0,006°).
const ANGLE_TOLERANCE: f64 = 1e-4;
/// Below this two lengths count as the same (0,1 mm).
const LENGTH_TOLERANCE: f64 = 1e-4;
/// How many rounds of adjustments an edit may take.
const MAX_PASSES: usize = 24;

/// A straight line of the plan: a wall's centre line or a drawn line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Entity {
    Wall(u64),
    Drawing(u64),
}

/// A point of a wall (0 its start, 1 its end) or of a drawing (one of its
/// grips: the ends of a line, a polyline's corners, an arc's ends...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Vertex {
    pub entity: Entity,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintKind {
    Parallel {
        a: Entity,
        b: Entity,
    },
    Perpendicular {
        a: Entity,
        b: Entity,
    },
    Coincident {
        a: Vertex,
        b: Vertex,
    },
    /// The distance between two points; the length of a line when they
    /// are its two ends.
    PointDistance {
        a: Vertex,
        b: Vertex,
        distance: f64,
    },
    /// The distance between two parallel lines.
    LineDistance {
        a: Entity,
        b: Entity,
        distance: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: u64,
    #[serde(flatten)]
    pub kind: ConstraintKind,
}

impl Entity {
    pub fn element(self) -> Element {
        match self {
            Entity::Wall(id) => Element::Wall(id),
            Entity::Drawing(id) => Element::Drawing(id),
        }
    }

    /// The entity an element is, if it can be constrained.
    pub fn of(element: Element) -> Option<Self> {
        match element {
            Element::Wall(id) => Some(Entity::Wall(id)),
            Element::Drawing(id) => Some(Entity::Drawing(id)),
            _ => None,
        }
    }
}

impl ConstraintKind {
    /// The entities it ties, the first one being the reference.
    pub fn entities(&self) -> [Entity; 2] {
        match *self {
            ConstraintKind::Parallel { a, b }
            | ConstraintKind::Perpendicular { a, b }
            | ConstraintKind::LineDistance { a, b, .. } => [a, b],
            ConstraintKind::Coincident { a, b } | ConstraintKind::PointDistance { a, b, .. } => {
                [a.entity, b.entity]
            }
        }
    }

    pub fn label(&self) -> String {
        match *self {
            ConstraintKind::Parallel { .. } => "Paralelas".to_owned(),
            ConstraintKind::Perpendicular { .. } => "Perpendiculares".to_owned(),
            ConstraintKind::Coincident { .. } => "Pontos coincidentes".to_owned(),
            ConstraintKind::PointDistance { a, b, distance } if a.entity == b.entity => {
                format!("Comprimento fixo {}", format_meters(distance))
            }
            ConstraintKind::PointDistance { distance, .. } => {
                format!("Distância fixa {}", format_meters(distance))
            }
            ConstraintKind::LineDistance { distance, .. } => {
                format!("Afastamento fixo {}", format_meters(distance))
            }
        }
    }

    /// A short mark for the plan: ∥, ⊥, ●, or the distance.
    pub fn symbol(&self) -> String {
        match *self {
            ConstraintKind::Parallel { .. } => "∥".to_owned(),
            ConstraintKind::Perpendicular { .. } => "⊥".to_owned(),
            ConstraintKind::Coincident { .. } => "●".to_owned(),
            ConstraintKind::PointDistance { distance, .. }
            | ConstraintKind::LineDistance { distance, .. } => {
                format!("↔ {}", format_meters(distance))
            }
        }
    }

    pub fn distance(&self) -> Option<f64> {
        match *self {
            ConstraintKind::PointDistance { distance, .. }
            | ConstraintKind::LineDistance { distance, .. } => Some(distance),
            _ => None,
        }
    }
}

/// The unit vector from `a` to `b`, or +x for a point.
fn unit(a: Point, b: Point) -> Point {
    b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0))
}

impl Project {
    /// The line of an entity: a wall's centre line, a drawn line's ends.
    pub fn entity_line(&self, entity: Entity) -> Option<(Point, Point)> {
        match entity {
            Entity::Wall(id) => self
                .walls
                .iter()
                .find(|wall| wall.id == id)
                .map(|wall| (wall.start, wall.end)),
            Entity::Drawing(id) => match self.drawing(id)?.shape {
                Shape::Line { start, end } => Some((start, end)),
                _ => None,
            },
        }
    }

    /// Where a vertex is.
    pub fn vertex_point(&self, vertex: Vertex) -> Option<Point> {
        match vertex.entity {
            Entity::Wall(id) => {
                let wall = self.walls.iter().find(|wall| wall.id == id)?;
                match vertex.index {
                    0 => Some(wall.start),
                    1 => Some(wall.end),
                    _ => None,
                }
            }
            Entity::Drawing(id) => Project::drawing_handles(&self.drawing(id)?.shape)
                .get(vertex.index)
                .copied(),
        }
    }

    /// The points of an entity that can be constrained.
    pub fn entity_vertices(&self, entity: Entity) -> Vec<(Vertex, Point)> {
        let count = match entity {
            Entity::Wall(_) => 2,
            Entity::Drawing(id) => self
                .drawing(id)
                .map(|drawing| Project::drawing_handles(&drawing.shape).len())
                .unwrap_or(0),
        };
        (0..count)
            .filter_map(|index| {
                let vertex = Vertex { entity, index };
                self.vertex_point(vertex).map(|point| (vertex, point))
            })
            .collect()
    }

    /// Whether everything the constraint names still exists in a form it
    /// applies to.
    fn constraint_valid(&self, kind: &ConstraintKind) -> bool {
        match *kind {
            ConstraintKind::Parallel { a, b }
            | ConstraintKind::Perpendicular { a, b }
            | ConstraintKind::LineDistance { a, b, .. } => {
                a != b && self.entity_line(a).is_some() && self.entity_line(b).is_some()
            }
            ConstraintKind::Coincident { a, b } | ConstraintKind::PointDistance { a, b, .. } => {
                a != b && self.vertex_point(a).is_some() && self.vertex_point(b).is_some()
            }
        }
    }

    /// How far the constraint is from holding (0 when it holds).
    fn constraint_error(&self, kind: &ConstraintKind) -> f64 {
        let line = |entity| self.entity_line(entity).unwrap_or_default();
        let point = |vertex| self.vertex_point(vertex).unwrap_or_default();
        match *kind {
            ConstraintKind::Parallel { a, b } => {
                let ((a0, a1), (b0, b1)) = (line(a), line(b));
                unit(a0, a1).cross(unit(b0, b1)).abs()
            }
            ConstraintKind::Perpendicular { a, b } => {
                let ((a0, a1), (b0, b1)) = (line(a), line(b));
                unit(a0, a1).dot(unit(b0, b1)).abs()
            }
            ConstraintKind::Coincident { a, b } => point(a).distance(point(b)),
            ConstraintKind::PointDistance { a, b, distance } => {
                (point(a).distance(point(b)) - distance).abs()
            }
            ConstraintKind::LineDistance { a, b, distance } => {
                let ((a0, a1), (b0, b1)) = (line(a), line(b));
                let normal = unit(a0, a1);
                let normal = Point::new(-normal.y, normal.x);
                (b0.midpoint(b1).minus(a0).dot(normal).abs() - distance).abs()
            }
        }
    }

    fn constraint_holds(&self, kind: &ConstraintKind) -> bool {
        match *kind {
            ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. } => {
                self.constraint_error(kind) <= ANGLE_TOLERANCE
            }
            // Lines kept apart are kept parallel too.
            ConstraintKind::LineDistance { a, b, .. } => {
                self.constraint_error(&ConstraintKind::Parallel { a, b }) <= ANGLE_TOLERANCE
                    && self.constraint_error(kind) <= LENGTH_TOLERANCE
            }
            _ => self.constraint_error(kind) <= LENGTH_TOLERANCE,
        }
    }

    /// Whether the entity is not where it was in `before`.
    fn entity_moved(&self, before: &Project, entity: Entity) -> bool {
        match entity {
            Entity::Wall(id) => {
                let now = self.walls.iter().find(|wall| wall.id == id);
                let then = before.walls.iter().find(|wall| wall.id == id);
                match (now, then) {
                    (Some(now), Some(then)) => now.start != then.start || now.end != then.end,
                    _ => true,
                }
            }
            Entity::Drawing(id) => {
                self.drawing(id).map(|d| &d.shape) != before.drawing(id).map(|d| &d.shape)
            }
        }
    }

    fn vertex_moved(&self, before: &Project, vertex: Vertex) -> bool {
        self.vertex_point(vertex) != before.vertex_point(vertex)
    }

    /// Moves a vertex: a wall's end takes the walls joined there along.
    fn move_vertex(&mut self, vertex: Vertex, to: Point) -> Result<(), String> {
        match vertex.entity {
            Entity::Wall(_) => {
                let anchor = self.vertex_point(vertex).ok_or("Parede não encontrada.")?;
                if anchor.near(to, 1e-9) {
                    return Ok(());
                }
                self.move_wall_corner(anchor, to)
            }
            Entity::Drawing(id) => self.move_drawing_handle(id, vertex.index, to),
        }
    }

    /// Puts a line's ends at `start` and `end`; a wall takes the walls
    /// joined to its ends along.
    fn set_entity_line(&mut self, entity: Entity, start: Point, end: Point) -> Result<(), String> {
        match entity {
            Entity::Drawing(id) => self.update_drawing(
                id,
                Shape::Line {
                    start: start.rounded(),
                    end: end.rounded(),
                },
            ),
            Entity::Wall(id) => {
                let (old_start, old_end) =
                    self.entity_line(entity).ok_or("Parede não encontrada.")?;
                let (start, end) = (start.rounded(), end.rounded());
                if let Some(terrain) = &self.terrain
                    && (!terrain.contains(start) || !terrain.contains(end))
                {
                    return Err("A parede sairia do terreno.".to_owned());
                }
                let mut tx = self.clone();
                let mut touched = Vec::new();
                // The walls joined at either end follow it.
                let follow = |point: Point| {
                    if point.near(old_start, JOIN_TOLERANCE) {
                        Some(start)
                    } else if point.near(old_end, JOIN_TOLERANCE) {
                        Some(end)
                    } else {
                        None
                    }
                };
                for wall in &mut tx.walls {
                    let mut changed = false;
                    if wall.id == id {
                        wall.start = start;
                        wall.end = end;
                        changed = true;
                    } else {
                        if let Some(to) = follow(wall.start) {
                            wall.start = to;
                            changed = true;
                        }
                        if let Some(to) = follow(wall.end) {
                            wall.end = to;
                            changed = true;
                        }
                    }
                    if changed {
                        if wall.length() < 0.25 - POINT_TOLERANCE {
                            return Err("Uma parede ligada ficaria menor que 0,25 m.".to_owned());
                        }
                        touched.push(wall.id);
                    }
                }
                tx.revalidate_openings_on_walls(&touched)?;
                tx.validate_affected_rooms(&touched)?;
                *self = tx;
                Ok(())
            }
        }
    }

    /// Makes one constraint hold by moving the side the edit did not
    /// touch (the second one when both or neither were).
    fn enforce(&mut self, before: &Project, kind: &ConstraintKind) -> Result<(), String> {
        match *kind {
            ConstraintKind::Parallel { a, b }
            | ConstraintKind::Perpendicular { a, b }
            | ConstraintKind::LineDistance { a, b, .. } => {
                let (fixed, moving) =
                    if self.entity_moved(before, b) && !self.entity_moved(before, a) {
                        (b, a)
                    } else {
                        (a, b)
                    };
                let (f0, f1) = self.entity_line(fixed).ok_or("Linha não encontrada.")?;
                let (m0, m1) = self.entity_line(moving).ok_or("Linha não encontrada.")?;
                let along = unit(f0, f1);
                let mut direction = along;
                if matches!(kind, ConstraintKind::Perpendicular { .. }) {
                    direction = Point::new(-direction.y, direction.x);
                }
                // Keep the line's own way round.
                if direction.dot(m1.minus(m0)) < 0.0 {
                    direction = direction.scaled(-1.0);
                }
                let length = m0.distance(m1);
                let joined =
                    |point: Point| point.near(f0, JOIN_TOLERANCE) || point.near(f1, JOIN_TOLERANCE);
                // It turns about the corner it shares with the other line,
                // so the corner stays; else about its middle.
                let (mut start, mut end) = if joined(m0) {
                    (m0, m0.translated(direction.scaled(length)))
                } else if joined(m1) {
                    (m1.translated(direction.scaled(-length)), m1)
                } else {
                    let middle = m0.midpoint(m1);
                    (
                        middle.translated(direction.scaled(-length / 2.0)),
                        middle.translated(direction.scaled(length / 2.0)),
                    )
                };
                if let ConstraintKind::LineDistance { distance, .. } = *kind {
                    // Then it slides square to the other to the distance.
                    let normal = Point::new(-along.y, along.x);
                    let offset = start.midpoint(end).minus(f0).dot(normal);
                    let target = if offset < 0.0 { -distance } else { distance };
                    let shift = normal.scaled(target - offset);
                    start = start.translated(shift);
                    end = end.translated(shift);
                }
                self.set_entity_line(moving, start, end)
            }
            ConstraintKind::Coincident { a, b } => {
                let (target, moving) = if self.vertex_moved(before, b)
                    && !self.vertex_moved(before, a)
                    && !self.entity_moved(before, a.entity)
                {
                    (b, a)
                } else {
                    (a, b)
                };
                let to = self.vertex_point(target).ok_or("Ponto não encontrado.")?;
                self.move_vertex(moving, to)
            }
            ConstraintKind::PointDistance { a, b, distance } => {
                let (pa, pb) = (
                    self.vertex_point(a).ok_or("Ponto não encontrado.")?,
                    self.vertex_point(b).ok_or("Ponto não encontrado.")?,
                );
                let (a_moved, b_moved) =
                    (self.vertex_moved(before, a), self.vertex_moved(before, b));
                if a.entity == b.entity {
                    // A fixed length: the end being dragged slides round the
                    // other one, like a rope.
                    if a_moved && !b_moved {
                        return self.move_vertex(a, pb.translated(unit(pb, pa).scaled(distance)));
                    }
                    return self.move_vertex(b, pa.translated(unit(pa, pb).scaled(distance)));
                }
                if self.entity_moved(before, b.entity) && !self.entity_moved(before, a.entity) {
                    self.move_vertex(a, pb.translated(unit(pb, pa).scaled(distance)))
                } else {
                    self.move_vertex(b, pa.translated(unit(pa, pb).scaled(distance)))
                }
            }
        }
    }

    /// Drops constraints whose elements are gone or changed kind; returns
    /// how many.
    pub fn prune_constraints(&mut self) -> usize {
        let before = self.constraints.len();
        let valid: Vec<bool> = self
            .constraints
            .iter()
            .map(|constraint| self.constraint_valid(&constraint.kind))
            .collect();
        let mut flags = valid.into_iter();
        self.constraints.retain(|_| flags.next().unwrap_or(false));
        before - self.constraints.len()
    }

    /// After an edit (from `before` to now), moves what the edit did not
    /// touch until every constraint holds; refuses the edit, leaving the
    /// project as it is, when that cannot be done.
    pub fn settle_constraints(&mut self, before: &Project) -> Result<(), String> {
        self.prune_constraints();
        if self.constraints.is_empty() {
            return Ok(());
        }
        let mut tx = self.clone();
        for _ in 0..MAX_PASSES {
            let mut adjusted = false;
            for constraint in tx.constraints.clone() {
                if tx.constraint_holds(&constraint.kind) {
                    continue;
                }
                tx.enforce(before, &constraint.kind).map_err(|error| {
                    format!(
                        "Não dá para manter a restrição \"{}\": {}",
                        constraint.kind.label(),
                        error.trim_end_matches('.')
                    ) + "."
                })?;
                adjusted = true;
            }
            if !adjusted {
                break;
            }
        }
        if let Some(broken) = tx
            .constraints
            .iter()
            .find(|constraint| !tx.constraint_holds(&constraint.kind))
        {
            return Err(format!(
                "A alteração conflita com a restrição \"{}\"; remova-a para fazer essa mudança.",
                broken.kind.label()
            ));
        }
        *self = tx;
        Ok(())
    }

    /// Makes the constraints of a file hold; those that cannot are dropped
    /// (so they never block every edit). Returns how many were dropped.
    pub fn repair_constraints(&mut self) -> usize {
        let snapshot = self.clone();
        if self.settle_constraints(&snapshot).is_ok() {
            return 0;
        }
        let holds: Vec<bool> = self
            .constraints
            .iter()
            .map(|constraint| self.constraint_holds(&constraint.kind))
            .collect();
        let before = self.constraints.len();
        let mut flags = holds.into_iter();
        self.constraints.retain(|_| flags.next().unwrap_or(false));
        before - self.constraints.len()
    }

    /// Adds a constraint and makes it hold, moving its second element;
    /// returns its id.
    pub fn add_constraint(&mut self, kind: ConstraintKind) -> Result<u64, String> {
        if !self.constraint_valid(&kind) {
            return Err(match kind {
                ConstraintKind::Coincident { .. } | ConstraintKind::PointDistance { .. } => {
                    "Escolha dois pontos diferentes de paredes ou desenhos."
                }
                _ => "Escolha duas linhas diferentes: paredes ou linhas desenhadas.",
            }
            .to_owned());
        }
        if let Some(distance) = kind.distance()
            && (!distance.is_finite() || !(0.0..=1000.0).contains(&distance))
        {
            return Err("A distância deve estar entre 0 e 1.000 m.".to_owned());
        }
        if let ConstraintKind::PointDistance { distance, .. } = kind
            && distance < 1e-3
        {
            return Err("Para pontos juntos, use a restrição de coincidência.".to_owned());
        }
        if let ConstraintKind::LineDistance { a, b, .. } = kind
            && !self.constraint_holds(&ConstraintKind::Parallel { a, b })
            && self.constraint_error(&ConstraintKind::Parallel { a, b }) > 0.02
        {
            return Err("As linhas precisam ser paralelas para fixar o afastamento.".to_owned());
        }
        let same = |other: &ConstraintKind| {
            other.entities() == kind.entities()
                && std::mem::discriminant(other) == std::mem::discriminant(&kind)
        };
        if self.constraints.iter().any(|existing| same(&existing.kind)) {
            return Err("Essa restrição já existe; ajuste-a nas propriedades.".to_owned());
        }
        let before = self.clone();
        let mut tx = self.clone();
        let id = tx.id();
        tx.constraints.push(Constraint { id, kind });
        tx.settle_constraints(&before)?;
        *self = tx;
        Ok(id)
    }

    /// Removes a constraint; the geometry stays as it is.
    pub fn remove_constraint(&mut self, id: u64) -> bool {
        let before = self.constraints.len();
        self.constraints.retain(|constraint| constraint.id != id);
        self.constraints.len() < before
    }

    /// Changes the distance a constraint keeps, moving its second element.
    pub fn set_constraint_distance(&mut self, id: u64, distance: f64) -> Result<(), String> {
        if !distance.is_finite() || !(0.001..=1000.0).contains(&distance) {
            return Err("A distância deve estar entre 0,001 e 1.000 m.".to_owned());
        }
        let before = self.clone();
        let mut tx = self.clone();
        let constraint = tx
            .constraints
            .iter_mut()
            .find(|constraint| constraint.id == id)
            .ok_or("Restrição não encontrada.")?;
        match &mut constraint.kind {
            ConstraintKind::PointDistance {
                distance: value, ..
            }
            | ConstraintKind::LineDistance {
                distance: value, ..
            } => *value = distance,
            _ => return Err("Essa restrição não tem distância.".to_owned()),
        }
        tx.settle_constraints(&before)?;
        *self = tx;
        Ok(())
    }

    /// The constraints on an entity.
    pub fn constraints_of(&self, entity: Entity) -> Vec<Constraint> {
        self.constraints
            .iter()
            .filter(|constraint| constraint.kind.entities().contains(&entity))
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(project: &mut Project, a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Drawing(
            project
                .add_drawing(Shape::Line {
                    start: Point::new(a.0, a.1),
                    end: Point::new(b.0, b.1),
                })
                .unwrap(),
        )
    }

    #[test]
    fn parallel_lines_follow_each_other() {
        let mut project = Project::default();
        let a = line(&mut project, (0.0, 0.0), (4.0, 0.0));
        let b = line(&mut project, (0.0, 2.0), (4.0, 3.0));
        project
            .add_constraint(ConstraintKind::Parallel { a, b })
            .unwrap();
        // The second line turned about its middle to lie level.
        let (b0, b1) = project.entity_line(b).unwrap();
        assert!((b0.y - b1.y).abs() < 1e-4 && (b0.midpoint(b1).y - 2.5).abs() < 1e-4);

        // Turning the first one turns the second.
        let before = project.clone();
        let Entity::Drawing(first) = a else { panic!() };
        project
            .move_drawing_handle(first, 1, Point::new(4.0, 4.0))
            .unwrap();
        project.settle_constraints(&before).unwrap();
        let (a0, a1) = project.entity_line(a).unwrap();
        let (b0, b1) = project.entity_line(b).unwrap();
        assert!(unit(a0, a1).cross(unit(b0, b1)).abs() < ANGLE_TOLERANCE);
        // A repeat is refused.
        assert!(
            project
                .add_constraint(ConstraintKind::Parallel { a, b })
                .is_err()
        );
    }

    #[test]
    fn perpendicular_walls_keep_their_corner() {
        let mut project = Project::default();
        let bottom = Entity::Wall(
            project
                .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
                .unwrap(),
        );
        let side = Entity::Wall(
            project
                .add_wall(Point::new(4.0, 0.0), Point::new(5.0, 3.0))
                .unwrap(),
        );
        project
            .add_constraint(ConstraintKind::Perpendicular { a: bottom, b: side })
            .unwrap();
        let (s0, s1) = project.entity_line(side).unwrap();
        assert!((s0.x - s1.x).abs() < 1e-4);
        // The corner between them stays joined.
        let (_, b1) = project.entity_line(bottom).unwrap();
        assert!(b1.near(s0, JOIN_TOLERANCE));
    }

    #[test]
    fn fixed_lengths_and_distances_hold_through_edits() {
        let mut project = Project::default();
        let a = line(&mut project, (0.0, 0.0), (3.0, 0.0));
        let start = Vertex {
            entity: a,
            index: 0,
        };
        let end = Vertex {
            entity: a,
            index: 1,
        };
        let id = project
            .add_constraint(ConstraintKind::PointDistance {
                a: start,
                b: end,
                distance: 3.0,
            })
            .unwrap();
        // Dragging the end aside keeps the length: it slides round.
        let before = project.clone();
        let Entity::Drawing(drawing) = a else {
            panic!()
        };
        project
            .move_drawing_handle(drawing, 1, Point::new(3.0, 3.0))
            .unwrap();
        project.settle_constraints(&before).unwrap();
        let (a0, a1) = project.entity_line(a).unwrap();
        assert!((a0.distance(a1) - 3.0).abs() < 1e-4);
        assert_eq!(a0, Point::new(0.0, 0.0));
        // A new value moves the free end.
        project.set_constraint_distance(id, 5.0).unwrap();
        let (a0, a1) = project.entity_line(a).unwrap();
        assert!((a0.distance(a1) - 5.0).abs() < 1e-4);

        // Two parallel lines 2 m apart: moving one carries the other.
        let mut project = Project::default();
        let near = line(&mut project, (0.0, 0.0), (4.0, 0.0));
        let far = line(&mut project, (0.0, 1.0), (4.0, 1.0));
        project
            .add_constraint(ConstraintKind::LineDistance {
                a: near,
                b: far,
                distance: 2.0,
            })
            .unwrap();
        assert!((project.entity_line(far).unwrap().0.y - 2.0).abs() < 1e-4);
        let before = project.clone();
        let Entity::Drawing(near_id) = near else {
            panic!()
        };
        project.move_drawing(near_id, Point::new(0.0, -1.0));
        project.settle_constraints(&before).unwrap();
        assert!((project.entity_line(far).unwrap().0.y - 1.0).abs() < 1e-4);
        // Lines at an angle cannot keep an offset.
        let slanted = line(&mut project, (0.0, 5.0), (4.0, 7.0));
        assert!(
            project
                .add_constraint(ConstraintKind::LineDistance {
                    a: near,
                    b: slanted,
                    distance: 1.0
                })
                .is_err()
        );
    }

    #[test]
    fn coincident_points_stay_together_and_conflicts_are_refused() {
        let mut project = Project::default();
        let a = line(&mut project, (0.0, 0.0), (2.0, 0.0));
        let b = line(&mut project, (3.0, 1.0), (5.0, 1.0));
        project
            .add_constraint(ConstraintKind::Coincident {
                a: Vertex {
                    entity: a,
                    index: 1,
                },
                b: Vertex {
                    entity: b,
                    index: 0,
                },
            })
            .unwrap();
        assert_eq!(
            project.entity_line(b).unwrap().0,
            project.entity_line(a).unwrap().1
        );
        // Moving the first line drags the joined point of the second.
        let before = project.clone();
        let Entity::Drawing(first) = a else { panic!() };
        project.move_drawing(first, Point::new(0.0, 1.0));
        project.settle_constraints(&before).unwrap();
        assert_eq!(project.entity_line(b).unwrap().0, Point::new(2.0, 1.0));

        // Contradictory constraints are refused: two lines kept parallel
        // 1 m apart cannot also share their first point.
        let mut project = Project::default();
        let a = line(&mut project, (0.0, 0.0), (2.0, 0.0));
        let b = line(&mut project, (0.0, 1.0), (2.0, 1.0));
        project
            .add_constraint(ConstraintKind::LineDistance {
                a,
                b,
                distance: 1.0,
            })
            .unwrap();
        let before = project.clone();
        assert!(
            project
                .add_constraint(ConstraintKind::Coincident {
                    a: Vertex {
                        entity: a,
                        index: 0
                    },
                    b: Vertex {
                        entity: b,
                        index: 0
                    },
                })
                .is_err()
        );
        assert_eq!(project, before);
        // An edit that breaks a constraint for good is refused too: the
        // first line's length is fixed and both its ends are dragged.
        let start = Vertex {
            entity: a,
            index: 0,
        };
        let end = Vertex {
            entity: a,
            index: 1,
        };
        project
            .add_constraint(ConstraintKind::PointDistance {
                a: start,
                b: end,
                distance: 2.0,
            })
            .unwrap();
        let mut stretched = project.clone();
        let Entity::Drawing(first) = a else { panic!() };
        stretched
            .update_drawing(
                first,
                Shape::Line {
                    start: Point::new(-1.0, 0.0),
                    end: Point::new(3.0, 0.0),
                },
            )
            .unwrap();
        // Both ends moved: the second end is put back at 2 m and the
        // parallel line follows; nothing contradicts that.
        stretched.settle_constraints(&project).unwrap();
        let (s0, s1) = stretched.entity_line(a).unwrap();
        assert!((s0.distance(s1) - 2.0).abs() < 1e-4);

        // Deleting an element drops its constraints.
        let Entity::Drawing(second) = b else { panic!() };
        project.delete_drawing(second);
        assert_eq!(project.prune_constraints(), 1);
        assert_eq!(project.constraints.len(), 1);
    }

    #[test]
    fn constraints_survive_files() {
        let mut project = Project::default();
        let a = line(&mut project, (0.0, 0.0), (2.0, 0.0));
        let b = line(&mut project, (0.0, 1.0), (2.0, 1.0));
        project
            .add_constraint(ConstraintKind::LineDistance {
                a,
                b,
                distance: 1.0,
            })
            .unwrap();
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"line_distance\""));
        let mut loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate_and_repair().unwrap();
        assert_eq!(loaded.constraints, project.constraints);
        let next = loaded
            .add_drawing(Shape::rectangle(Point::ORIGIN, Point::new(1.0, 1.0)))
            .unwrap();
        assert!(next > loaded.constraints[0].id);
        assert!(
            loaded.constraints_of(a).len() == 1
                && loaded.remove_constraint(loaded.constraints[0].id)
        );

        // A file whose constraints contradict each other opens without them.
        let mut contradictory = project.clone();
        let id = contradictory.id();
        contradictory.constraints.push(Constraint {
            id,
            kind: ConstraintKind::Coincident {
                a: Vertex {
                    entity: a,
                    index: 0,
                },
                b: Vertex {
                    entity: b,
                    index: 0,
                },
            },
        });
        let warnings = contradictory.validate_and_repair().unwrap();
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("impossíveis"))
        );
        assert!(
            contradictory
                .constraints
                .iter()
                .all(|constraint| contradictory.constraint_holds(&constraint.kind))
        );
    }
}
