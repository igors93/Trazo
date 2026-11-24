//! What an edit changed: the elements added, removed and altered between
//! two states of a project. It feeds the live highlight of the assistant's
//! edits and the change log assistants read to stay in step with the person.

use std::collections::{HashMap, HashSet};

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
    /// The elements added, removed and altered since `before` in O(N) time.
    pub fn changes_since(&self, before: &Project) -> ChangeSet {
        let mut changes = ChangeSet::default();

        let prev_walls: HashMap<u64, &super::Wall> =
            before.walls.iter().map(|w| (w.id, w)).collect();
        let prev_rooms: HashMap<u64, &super::Room> =
            before.rooms.iter().map(|r| (r.id, r)).collect();
        let prev_openings: HashMap<u64, &super::Opening> =
            before.openings.iter().map(|o| (o.id, o)).collect();
        let prev_fixtures: HashMap<u64, &crate::domain::fixtures::Fixture> =
            before.fixtures.iter().map(|f| (f.id, f)).collect();
        let prev_drawings: HashMap<u64, &crate::domain::drawing::Drawing> =
            before.drawings.iter().map(|d| (d.id, d)).collect();

        let mut changed_walls: HashSet<u64> = HashSet::new();
        let mut cur_walls: HashSet<u64> = HashSet::with_capacity(self.walls.len());
        for w in &self.walls {
            cur_walls.insert(w.id);
            match prev_walls.get(&w.id) {
                None => {
                    changes.added.push(Element::Wall(w.id));
                    changed_walls.insert(w.id);
                }
                Some(&prev) if prev != w => {
                    changes.changed.push(Element::Wall(w.id));
                    changed_walls.insert(w.id);
                }
                Some(_) => {}
            }
        }

        let mut cur_rooms: HashSet<u64> = HashSet::with_capacity(self.rooms.len());
        for r in &self.rooms {
            cur_rooms.insert(r.id);
            match prev_rooms.get(&r.id) {
                None => changes.added.push(Element::Room(r.id)),
                Some(&prev)
                    if prev != r || r.wall_ids.iter().any(|wid| changed_walls.contains(wid)) =>
                {
                    changes.changed.push(Element::Room(r.id));
                }
                Some(_) => {}
            }
        }

        let mut cur_openings: HashSet<u64> = HashSet::with_capacity(self.openings.len());
        for o in &self.openings {
            cur_openings.insert(o.id);
            match prev_openings.get(&o.id) {
                None => changes.added.push(Element::Opening(o.id)),
                Some(&prev) if prev != o || changed_walls.contains(&o.wall_id) => {
                    changes.changed.push(Element::Opening(o.id));
                }
                Some(_) => {}
            }
        }

        let mut cur_fixtures: HashSet<u64> = HashSet::with_capacity(self.fixtures.len());
        for f in &self.fixtures {
            cur_fixtures.insert(f.id);
            match prev_fixtures.get(&f.id) {
                None => changes.added.push(Element::Fixture(f.id)),
                Some(&prev) if prev != f => changes.changed.push(Element::Fixture(f.id)),
                Some(_) => {}
            }
        }

        let mut cur_drawings: HashSet<u64> = HashSet::with_capacity(self.drawings.len());
        for d in &self.drawings {
            cur_drawings.insert(d.id);
            match prev_drawings.get(&d.id) {
                None => changes.added.push(Element::Drawing(d.id)),
                Some(&prev) if prev != d => changes.changed.push(Element::Drawing(d.id)),
                Some(_) => {}
            }
        }

        if let Some(t) = &self.terrain {
            match &before.terrain {
                Some(prev) if prev.id == t.id => {
                    if prev != t {
                        changes.changed.push(Element::Terrain(t.id));
                    }
                }
                _ => changes.added.push(Element::Terrain(t.id)),
            }
        }

        // Removed elements.
        for w in &before.walls {
            if !cur_walls.contains(&w.id) {
                changes.removed.push(Element::Wall(w.id));
            }
        }
        for r in &before.rooms {
            if !cur_rooms.contains(&r.id) {
                changes.removed.push(Element::Room(r.id));
            }
        }
        for o in &before.openings {
            if !cur_openings.contains(&o.id) {
                changes.removed.push(Element::Opening(o.id));
            }
        }
        for f in &before.fixtures {
            if !cur_fixtures.contains(&f.id) {
                changes.removed.push(Element::Fixture(f.id));
            }
        }
        for d in &before.drawings {
            if !cur_drawings.contains(&d.id) {
                changes.removed.push(Element::Drawing(d.id));
            }
        }
        if let Some(prev) = &before.terrain
            && self.terrain.as_ref().is_none_or(|t| t.id != prev.id)
        {
            changes.removed.push(Element::Terrain(prev.id));
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
