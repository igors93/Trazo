//! A detached piece of a project — walls with their rooms and openings,
//! fixtures and drawings — for copying and pasting, and turning or
//! mirroring elements in place or as copies.

use std::collections::{HashMap, HashSet};

use super::{Constraint, ConstraintKind, Entity, Group, Opening, Project, Room, Wall};
use crate::domain::drawing::Drawing;
use crate::domain::element::Element;
use crate::domain::fixtures::Fixture;
use crate::domain::geometry::Point;
use crate::domain::transform::Transform;

/// Elements taken out of a project with their relations: rooms keep their
/// walls and openings stay on theirs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fragment {
    pub walls: Vec<Wall>,
    pub rooms: Vec<Room>,
    pub openings: Vec<Opening>,
    pub fixtures: Vec<Fixture>,
    pub drawings: Vec<Drawing>,
    /// Groups whose members all came along (by their original ids).
    pub groups: Vec<Group>,
    /// Constraints between elements that all came along.
    pub constraints: Vec<Constraint>,
}

impl Fragment {
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty() && self.fixtures.is_empty() && self.drawings.is_empty()
    }

    /// How many elements it holds (walls, rooms, openings, fixtures and
    /// drawings).
    pub fn len(&self) -> usize {
        self.walls.len()
            + self.rooms.len()
            + self.openings.len()
            + self.fixtures.len()
            + self.drawings.len()
    }

    /// The box around everything in it.
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut points: Vec<Point> = self
            .walls
            .iter()
            .flat_map(|wall| [wall.start, wall.end])
            .collect();
        for fixture in &self.fixtures {
            let (low, high) = fixture.bounds();
            points.extend([low, high]);
        }
        for drawing in &self.drawings {
            let (low, high) = drawing.shape.bounds();
            points.extend([low, high]);
        }
        let first = *points.first()?;
        Some(points.iter().fold((first, first), |(low, high), point| {
            (
                Point::new(low.x.min(point.x), low.y.min(point.y)),
                Point::new(high.x.max(point.x), high.y.max(point.y)),
            )
        }))
    }

    /// The middle of its box.
    pub fn center(&self) -> Option<Point> {
        self.bounds().map(|(low, high)| low.midpoint(high))
    }

    /// Applies `transform` to everything in it. Doors turned over by a
    /// mirror open to the mirrored side; fixtures must stay square to the
    /// plan.
    pub fn transform(&mut self, transform: &Transform) -> Result<(), String> {
        if !transform.is_valid() {
            return Err("Transformação inválida.".to_owned());
        }
        for fixture in &mut self.fixtures {
            fixture.quarter_turns = transform.quarter_turns(fixture.quarter_turns).ok_or(
                "Objetos (bancadas, pias, mesas...) só giram em múltiplos de 90° e só espelham em eixos horizontais, verticais ou a 45°.",
            )?;
            fixture.center = transform.apply(fixture.center);
        }
        for wall in &mut self.walls {
            wall.start = transform.apply(wall.start);
            wall.end = transform.apply(wall.end);
        }
        if transform.reflects() {
            for opening in &mut self.openings {
                opening.flip_side = !opening.flip_side;
            }
        }
        for drawing in &mut self.drawings {
            drawing.shape = transform.apply_shape(&drawing.shape);
            drawing.shape.validate()?;
        }
        Ok(())
    }

    /// A copy with `transform` applied.
    pub fn transformed(&self, transform: &Transform) -> Result<Self, String> {
        let mut copy = self.clone();
        copy.transform(transform)?;
        Ok(copy)
    }
}

impl Project {
    /// The fragment `elements` make: the selected walls and those of the
    /// selected rooms, every room whose walls all come along, the doors
    /// and windows on those walls, and the selected fixtures and drawings.
    pub fn fragment(&self, elements: &[Element]) -> Fragment {
        let walls = self.walls_of(elements);
        let fixtures: HashSet<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Fixture(id) => Some(*id),
                _ => None,
            })
            .collect();
        let drawings: HashSet<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Drawing(id) => Some(*id),
                _ => None,
            })
            .collect();
        let rooms: Vec<Room> = self
            .rooms
            .iter()
            .filter(|room| room.wall_ids.iter().all(|id| walls.contains(id)))
            .cloned()
            .collect();
        let included = |member: &Element| match member {
            Element::Wall(id) => walls.contains(id),
            Element::Room(id) => rooms.iter().any(|room| room.id == *id),
            Element::Fixture(id) => fixtures.contains(id),
            Element::Drawing(id) => drawings.contains(id),
            _ => false,
        };
        let groups: Vec<Group> = self
            .groups
            .iter()
            .filter(|group| group.members.iter().all(included))
            .cloned()
            .collect();
        let constraints: Vec<Constraint> = self
            .constraints
            .iter()
            .filter(|constraint| {
                constraint
                    .kind
                    .entities()
                    .iter()
                    .all(|entity| included(&entity.element()))
            })
            .copied()
            .collect();
        Fragment {
            walls: self
                .walls
                .iter()
                .filter(|wall| walls.contains(&wall.id))
                .cloned()
                .collect(),
            rooms,
            groups,
            constraints,
            openings: self
                .openings
                .iter()
                .filter(|opening| walls.contains(&opening.wall_id))
                .cloned()
                .collect(),
            fixtures: self
                .fixtures
                .iter()
                .filter(|fixture| fixtures.contains(&fixture.id))
                .copied()
                .collect(),
            drawings: self
                .drawings
                .iter()
                .filter(|drawing| drawings.contains(&drawing.id))
                .cloned()
                .collect(),
        }
    }

    /// Whether every wall and fixture of `fragment` lies in the terrain.
    /// Whether the walls of `fragment` lie within the buildable limit and
    /// its fixtures within the terrain, as new walls and fixtures must.
    fn fragment_fits(&self, fragment: &Fragment) -> bool {
        fragment.walls.iter().all(|wall| {
            self.point_inside_buildable_limit(wall.start)
                && self.point_inside_buildable_limit(wall.end)
                && self.point_inside_buildable_limit(wall.start.midpoint(wall.end))
        }) && fragment
            .fixtures
            .iter()
            .all(|fixture| self.fixture_fits(fixture))
    }

    /// Adds a copy of `fragment` with new ids, as one change; returns the
    /// new elements to select (rooms, walls outside them, fixtures and
    /// drawings).
    pub fn insert_fragment(&mut self, fragment: &Fragment) -> Result<Vec<Element>, String> {
        if fragment.is_empty() {
            return Err("Nada para colar.".to_owned());
        }
        if !self.fragment_fits(fragment) {
            return Err("A cópia precisa caber dentro do terreno.".to_owned());
        }
        let mut tx = self.clone();
        let mut new_walls: HashMap<u64, u64> = HashMap::new();
        for wall in &fragment.walls {
            let id = tx.id();
            new_walls.insert(wall.id, id);
            tx.walls.push(Wall { id, ..wall.clone() });
        }
        let mut created = Vec::new();
        // Old element → its copy, to rebuild the copied groups.
        let mut copies: HashMap<Element, Element> = new_walls
            .iter()
            .map(|(old, new)| (Element::Wall(*old), Element::Wall(*new)))
            .collect();
        let mut in_rooms: HashSet<u64> = HashSet::new();
        for room in &fragment.rooms {
            let wall_ids: Vec<u64> = room
                .wall_ids
                .iter()
                .filter_map(|id| new_walls.get(id).copied())
                .collect();
            in_rooms.extend(wall_ids.iter().copied());
            let id = tx.id();
            tx.rooms.push(Room {
                id,
                wall_ids,
                ..room.clone()
            });
            created.push(Element::Room(id));
            copies.insert(Element::Room(room.id), Element::Room(id));
        }
        // Walls outside the copied rooms, in the order they were copied.
        let mut loose: Vec<u64> = new_walls
            .values()
            .copied()
            .filter(|id| !in_rooms.contains(id))
            .collect();
        loose.sort_unstable();
        created.extend(loose.into_iter().map(Element::Wall));
        for opening in &fragment.openings {
            let Some(&wall_id) = new_walls.get(&opening.wall_id) else {
                continue;
            };
            let id = tx.id();
            tx.openings.push(Opening {
                id,
                wall_id,
                ..opening.clone()
            });
        }
        for fixture in &fragment.fixtures {
            let id = tx.id();
            tx.fixtures.push(Fixture { id, ..*fixture });
            created.push(Element::Fixture(id));
            copies.insert(Element::Fixture(fixture.id), Element::Fixture(id));
        }
        for drawing in &fragment.drawings {
            drawing.shape.validate()?;
            let id = tx.id();
            tx.drawings.push(Drawing {
                id,
                shape: drawing.shape.clone(),
                layer_id: drawing.layer_id,
            });
            created.push(Element::Drawing(id));
            copies.insert(Element::Drawing(drawing.id), Element::Drawing(id));
        }
        // Constraints between copies tie the copies.
        let copy_of = |entity: Entity| match copies.get(&entity.element()) {
            Some(Element::Wall(id)) => Some(Entity::Wall(*id)),
            Some(Element::Drawing(id)) => Some(Entity::Drawing(*id)),
            _ => None,
        };
        for constraint in &fragment.constraints {
            let mut kind = constraint.kind;
            let mapped = match &mut kind {
                ConstraintKind::Parallel { a, b }
                | ConstraintKind::Perpendicular { a, b }
                | ConstraintKind::LineDistance { a, b, .. } => match (copy_of(*a), copy_of(*b)) {
                    (Some(new_a), Some(new_b)) => {
                        (*a, *b) = (new_a, new_b);
                        true
                    }
                    _ => false,
                },
                ConstraintKind::Coincident { a, b }
                | ConstraintKind::PointDistance { a, b, .. } => {
                    match (copy_of(a.entity), copy_of(b.entity)) {
                        (Some(new_a), Some(new_b)) => {
                            (a.entity, b.entity) = (new_a, new_b);
                            true
                        }
                        _ => false,
                    }
                }
            };
            if mapped {
                let id = tx.id();
                tx.constraints.push(Constraint { id, kind });
            }
        }
        // Groups copied whole are groups again.
        for group in &fragment.groups {
            let members: Vec<Element> = group
                .members
                .iter()
                .filter_map(|member| copies.get(member).copied())
                .collect();
            if members.len() >= 2 {
                let id = tx.id();
                tx.groups.push(Group {
                    id,
                    name: group.name.clone(),
                    members,
                });
            }
        }
        *self = tx;
        Ok(created)
    }

    /// Adds copies of `elements` with `transform` applied (copy, mirror or
    /// turn keeping the originals); returns the copies.
    pub fn copy_elements(
        &mut self,
        elements: &[Element],
        transform: &Transform,
    ) -> Result<Vec<Element>, String> {
        if elements
            .iter()
            .any(|element| matches!(element, Element::Terrain(_)))
            && self.fragment(elements).is_empty()
        {
            return Err("O terreno não pode ser copiado.".to_owned());
        }
        let fragment = self.fragment(elements).transformed(transform)?;
        if fragment.is_empty() {
            return Err(
                "Nada para copiar: portas e janelas são copiadas com as suas paredes.".to_owned(),
            );
        }
        self.insert_fragment(&fragment)
    }

    /// Moves, turns or mirrors `elements` in place, as one change. A move
    /// stretches the walls joined to them (see `move_elements`); turning
    /// and mirroring leave the other walls where they are, and refuse to
    /// break a room that is not wholly selected.
    pub fn transform_elements(
        &mut self,
        elements: &[Element],
        transform: &Transform,
    ) -> Result<(), String> {
        if let Transform::Translate(delta) = *transform {
            return self.move_elements(elements, delta);
        }
        if elements
            .iter()
            .any(|element| matches!(element, Element::Terrain(_)))
        {
            return Err(
                "O terreno não gira nem espelha: tire-o da seleção (Shift+clique).".to_owned(),
            );
        }
        let walls = self.walls_of(elements);
        self.check_rooms_move_whole(elements, &walls)?;
        let fragment = self.fragment(elements).transformed(transform)?;
        if fragment.is_empty() {
            return Err(
                "Nada para transformar: portas e janelas seguem as suas paredes.".to_owned(),
            );
        }
        if !self.fragment_fits(&fragment) {
            return Err("A seleção sairia do terreno.".to_owned());
        }
        let mut tx = self.clone();
        for moved in &fragment.walls {
            if let Some(wall) = tx.walls.iter_mut().find(|wall| wall.id == moved.id) {
                wall.start = moved.start;
                wall.end = moved.end;
            }
        }
        for moved in &fragment.openings {
            if let Some(opening) = tx.openings.iter_mut().find(|o| o.id == moved.id) {
                opening.flip_side = moved.flip_side;
            }
        }
        for moved in &fragment.fixtures {
            if let Some(fixture) = tx.fixtures.iter_mut().find(|f| f.id == moved.id) {
                *fixture = *moved;
            }
        }
        for moved in &fragment.drawings {
            if let Some(drawing) = tx.drawings.iter_mut().find(|d| d.id == moved.id) {
                drawing.shape = moved.shape.clone();
            }
        }
        let wall_ids: Vec<u64> = fragment.walls.iter().map(|wall| wall.id).collect();
        tx.revalidate_openings_on_walls(&wall_ids)?;
        tx.validate_affected_rooms(&wall_ids)?;
        *self = tx;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::model::OpeningKind;

    fn room_with_door() -> (Project, u64, u64, u64) {
        let mut project = Project::default();
        let room = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Banheiro")
            .unwrap();
        let bottom = project.rooms[0].wall_ids[0];
        let door = project
            .add_opening(bottom, OpeningKind::Door, 0.25)
            .unwrap();
        let toilet = project
            .add_fixture(FixtureKind::Toilet, Point::new(3.0, 1.0), 0.4, 0.65, 0)
            .unwrap();
        (project, room, door, toilet)
    }

    #[test]
    fn copies_keep_rooms_walls_and_openings_together() {
        let (mut project, room, _, toilet) = room_with_door();
        let copies = project
            .copy_elements(
                &[Element::Room(room), Element::Fixture(toilet)],
                &Transform::Translate(Point::new(5.0, 0.0)),
            )
            .unwrap();
        assert_eq!(project.rooms.len(), 2);
        assert_eq!(project.walls.len(), 8);
        assert_eq!(project.openings.len(), 2);
        assert_eq!(project.fixtures.len(), 2);
        assert_eq!(copies.len(), 2);
        let Element::Room(copy) = copies[0] else {
            panic!("the room comes first");
        };
        let copy = project.rooms.iter().find(|r| r.id == copy).unwrap();
        assert_eq!(copy.name, "Banheiro");
        assert_eq!(project.room_area(copy), Some(12.0));
        assert_eq!(project.room_bounds(copy).unwrap().0, Point::new(5.0, 0.0));
        // The copied door sits on a copied wall, at the same spot.
        let door = project.openings.last().unwrap();
        assert!(copy.wall_ids.contains(&door.wall_id));
        assert_eq!(project.fixtures[1].center, Point::new(8.0, 1.0));

        // Copies must fit the terrain.
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(12.0, 6.0));
        assert!(
            project
                .copy_elements(
                    &[Element::Room(room)],
                    &Transform::Translate(Point::new(10.0, 0.0))
                )
                .is_err()
        );
        // An opening alone has nothing to stand on.
        let door_alone = Element::Opening(project.openings[0].id);
        assert!(
            project
                .copy_elements(&[door_alone], &Transform::Translate(Point::new(0.0, 1.0)))
                .is_err()
        );
    }

    #[test]
    fn mirroring_in_place_turns_doors_and_fixtures_over() {
        let (mut project, room, door, toilet) = room_with_door();
        let before = project.openings[0].clone();
        let axis = Transform::mirror(Point::new(2.0, 0.0), Point::new(2.0, 3.0)).unwrap();
        project
            .transform_elements(&[Element::Room(room), Element::Fixture(toilet)], &axis)
            .unwrap();
        // Same room, mirrored onto itself.
        let room_ref = project.rooms.iter().find(|r| r.id == room).unwrap();
        assert_eq!(project.room_area(room_ref), Some(12.0));
        let door_now = project.openings.iter().find(|o| o.id == door).unwrap();
        assert_eq!(door_now.flip_side, !before.flip_side);
        assert_eq!(project.fixtures[0].center, Point::new(1.0, 1.0));
        // The door is now near the other end of the room.
        let (low, _) = project.element_bounds(Element::Opening(door)).unwrap();
        assert!(low.x > 2.0);
    }

    #[test]
    fn turning_refuses_askew_fixtures_and_torn_rooms() {
        let (mut project, room, _, toilet) = room_with_door();
        let before = project.clone();
        let askew = Transform::rotate_degrees(Point::new(2.0, 1.5), 45.0);
        assert!(
            project
                .transform_elements(&[Element::Room(room), Element::Fixture(toilet)], &askew)
                .is_err()
        );
        assert_eq!(project, before);
        // The room alone turns fine at 45°.
        project
            .transform_elements(&[Element::Room(room)], &askew)
            .unwrap();
        let room_ref = project.rooms.iter().find(|r| r.id == room).unwrap();
        assert!((project.room_area(room_ref).unwrap() - 12.0).abs() < 1e-3);

        // One wall of a room cannot turn away from it.
        let mut torn = before.clone();
        let wall = torn.rooms[0].wall_ids[1];
        let quarter = Transform::rotate_degrees(Point::new(4.0, 0.0), 90.0);
        assert!(
            torn.transform_elements(&[Element::Wall(wall)], &quarter)
                .is_err()
        );
        // Nor does the terrain turn.
        torn.set_terrain(Point::new(-5.0, -5.0), Point::new(10.0, 10.0));
        let terrain = torn.terrain.as_ref().unwrap().id;
        assert!(
            torn.transform_elements(&[Element::Terrain(terrain)], &quarter)
                .is_err()
        );
    }

    #[test]
    fn drawings_copy_turned_and_mirrored() {
        let mut project = Project::default();
        let line = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 0.0),
            })
            .unwrap();
        let copies = project
            .copy_elements(
                &[Element::Drawing(line)],
                &Transform::rotate_degrees(Point::ORIGIN, 90.0),
            )
            .unwrap();
        let Element::Drawing(copy) = copies[0] else {
            panic!("a drawing");
        };
        assert_eq!(
            project.drawing(copy).unwrap().shape,
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(0.0, -2.0)
            }
        );
        assert_eq!(project.drawings.len(), 2);
    }
}
