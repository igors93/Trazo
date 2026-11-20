//! What an edit changed: the elements added, removed and altered between
//! two states of a project. It feeds the live highlight of the assistant's
//! edits and the change log assistants read to stay in step with the person.

use super::Project;
use crate::domain::element::Element;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChangeSet {
    pub added: Vec<Element>,
    pub removed: Vec<Element>,
    pub changed: Vec<Element>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// The elements still in the plan: added or altered.
    pub fn touched(&self) -> Vec<Element> {
        self.added.iter().chain(&self.changed).copied().collect()
    }
}

impl Project {
    /// Whether `element` is the same in both projects.
    fn element_unchanged(&self, before: &Project, element: Element) -> bool {
        match element {
            Element::Wall(id) => {
                self.walls.iter().find(|w| w.id == id) == before.walls.iter().find(|w| w.id == id)
            }
            Element::Room(id) => {
                self.rooms.iter().find(|r| r.id == id) == before.rooms.iter().find(|r| r.id == id)
                    // A room changes shape with its walls.
                    && self.rooms.iter().find(|r| r.id == id).is_none_or(|room| {
                        room.wall_ids.iter().all(|wall| {
                            self.element_unchanged(before, Element::Wall(*wall))
                        })
                    })
            }
            Element::Opening(id) => {
                let now = self.openings.iter().find(|o| o.id == id);
                let then = before.openings.iter().find(|o| o.id == id);
                // A door moves when its wall does.
                now == then
                    && now.is_none_or(|opening| {
                        self.element_unchanged(before, Element::Wall(opening.wall_id))
                    })
            }
            Element::Fixture(id) => {
                self.fixtures.iter().find(|f| f.id == id)
                    == before.fixtures.iter().find(|f| f.id == id)
            }
            Element::Drawing(id) => self.drawing(id) == before.drawing(id),
            Element::Terrain(_) => self.terrain == before.terrain,
        }
    }

    /// The elements added, removed and altered since `before`.
    pub fn changes_since(&self, before: &Project) -> ChangeSet {
        let mut changes = ChangeSet::default();
        for element in self.elements() {
            if !before.element_exists(element) {
                changes.added.push(element);
            } else if !self.element_unchanged(before, element) {
                changes.changed.push(element);
            }
        }
        for element in before.elements() {
            if !self.element_exists(element) {
                changes.removed.push(element);
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;
    use crate::domain::model::OpeningKind;

    #[test]
    fn changes_list_what_was_added_removed_and_altered() {
        let mut before = Project::default();
        let room = before
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let bottom = before.rooms[0].wall_ids[0];
        let right = before.rooms[0].wall_ids[1];
        let door = before.add_opening(bottom, OpeningKind::Door, 0.5).unwrap();
        let line = before
            .add_drawing(crate::domain::drawing::Shape::Line {
                start: Point::ORIGIN,
                end: Point::new(1.0, 0.0),
            })
            .unwrap();
        assert!(before.changes_since(&before).is_empty());

        let mut after = before.clone();
        after.move_wall(bottom, Point::new(0.0, -0.5));
        after.delete_drawing(line);
        let wall = after
            .add_wall(Point::new(10.0, 0.0), Point::new(12.0, 0.0))
            .unwrap();
        let changes = after.changes_since(&before);
        assert_eq!(changes.added, vec![Element::Wall(wall)]);
        assert_eq!(changes.removed, vec![Element::Drawing(line)]);
        // The moved wall, the walls stretched with it, its room and its door.
        for element in [
            Element::Wall(bottom),
            Element::Wall(right),
            Element::Room(room),
            Element::Opening(door),
        ] {
            assert!(changes.changed.contains(&element), "{element:?}");
        }
        assert_eq!(changes.touched().len(), changes.changed.len() + 1);
    }
}
