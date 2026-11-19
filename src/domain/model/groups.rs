//! Groups: elements picked, moved and copied together, kept in the file.

use serde::{Deserialize, Serialize};

use super::Project;
use crate::domain::element::Element;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    /// Rooms, walls, fixtures and drawings (doors and windows follow their
    /// walls; the terrain is never grouped).
    pub members: Vec<Element>,
}

/// Whether an element can belong to a group.
fn groupable(element: Element) -> bool {
    matches!(
        element,
        Element::Room(_) | Element::Wall(_) | Element::Fixture(_) | Element::Drawing(_)
    )
}

impl Project {
    /// The group `element` belongs to, if any.
    pub fn group_of(&self, element: Element) -> Option<&Group> {
        self.groups
            .iter()
            .find(|group| group.members.contains(&element))
    }

    /// Groups `elements` (at least two that can be grouped) under `name`;
    /// they leave any group they were in. Returns the new group's id.
    pub fn group_elements(&mut self, elements: &[Element], name: &str) -> Result<u64, String> {
        let mut members: Vec<Element> = Vec::new();
        for element in elements {
            if groupable(*element) && self.element_exists(*element) && !members.contains(element) {
                members.push(*element);
            }
        }
        if members.len() < 2 {
            return Err(
                "Selecione pelo menos dois cômodos, paredes, objetos ou desenhos para agrupar."
                    .to_owned(),
            );
        }
        for group in &mut self.groups {
            group.members.retain(|member| !members.contains(member));
        }
        let id = self.id();
        let name = name.trim();
        let name = if name.is_empty() {
            format!("Grupo {}", self.groups.len() + 1)
        } else {
            name.to_owned()
        };
        self.groups.push(Group { id, name, members });
        self.prune_groups();
        Ok(id)
    }

    /// Undoes the groups any of `elements` belongs to; returns how many.
    pub fn ungroup_elements(&mut self, elements: &[Element]) -> usize {
        let before = self.groups.len();
        self.groups
            .retain(|group| !group.members.iter().any(|member| elements.contains(member)));
        before - self.groups.len()
    }

    /// Renames group `id`.
    pub fn rename_group(&mut self, id: u64, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Dê um nome ao grupo.".to_owned());
        }
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or("Grupo não encontrado.")?;
        group.name = name.to_owned();
        Ok(())
    }

    /// Drops members that no longer exist and groups left with fewer than
    /// two; returns how many groups went.
    pub fn prune_groups(&mut self) -> usize {
        let existing: Vec<Vec<bool>> = self
            .groups
            .iter()
            .map(|group| {
                group
                    .members
                    .iter()
                    .map(|member| self.element_exists(*member))
                    .collect()
            })
            .collect();
        for (group, keep) in self.groups.iter_mut().zip(existing) {
            let mut flags = keep.into_iter();
            group.members.retain(|_| flags.next().unwrap_or(false));
        }
        let before = self.groups.len();
        self.groups.retain(|group| group.members.len() >= 2);
        before - self.groups.len()
    }

    /// `elements` with every group they touch taken whole.
    pub fn with_whole_groups(&self, elements: &[Element]) -> Vec<Element> {
        let mut all = elements.to_vec();
        for element in elements {
            if let Some(group) = self.group_of(*element) {
                for member in &group.members {
                    if !all.contains(member) {
                        all.push(*member);
                    }
                }
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;
    use crate::domain::geometry::Point;

    fn three_lines() -> (Project, [u64; 3]) {
        let mut project = Project::default();
        let ids = [0.0, 1.0, 2.0].map(|y| {
            project
                .add_drawing(Shape::Line {
                    start: Point::new(0.0, y),
                    end: Point::new(2.0, y),
                })
                .unwrap()
        });
        (project, ids)
    }

    #[test]
    fn groups_gather_members_and_survive_files() {
        let (mut project, [a, b, c]) = three_lines();
        let group = project
            .group_elements(&[Element::Drawing(a), Element::Drawing(b)], "")
            .unwrap();
        assert_eq!(project.groups[0].name, "Grupo 1");
        assert_eq!(project.group_of(Element::Drawing(b)).unwrap().id, group);
        assert!(project.group_of(Element::Drawing(c)).is_none());
        assert_eq!(
            project.with_whole_groups(&[Element::Drawing(a)]),
            vec![Element::Drawing(a), Element::Drawing(b)]
        );
        // One element is not a group, and the terrain is never grouped.
        assert!(
            project
                .group_elements(&[Element::Drawing(c), Element::Terrain(1)], "X")
                .is_err()
        );

        // Regrouping takes members out of their old group, which dissolves.
        project
            .group_elements(&[Element::Drawing(b), Element::Drawing(c)], "Novo")
            .unwrap();
        assert_eq!(project.groups.len(), 1);
        assert_eq!(project.groups[0].name, "Novo");

        let json = serde_json::to_string(&project).unwrap();
        let mut loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate_and_repair().unwrap();
        assert_eq!(loaded.groups, project.groups);
        // New ids never reuse a group's.
        let next = loaded.add_drawing(Shape::rectangle(Point::ORIGIN, Point::new(1.0, 1.0)));
        assert!(next.unwrap() > loaded.groups[0].id);

        // Deleting a member leaves a group of one, which goes.
        project.delete_drawing(c);
        assert_eq!(project.prune_groups(), 1);
        assert!(project.groups.is_empty());
        assert!(project.rename_group(99, "x").is_err());
    }

    #[test]
    fn ungrouping_frees_the_members() {
        let (mut project, [a, b, _]) = three_lines();
        let id = project
            .group_elements(&[Element::Drawing(a), Element::Drawing(b)], "Mesa")
            .unwrap();
        project.rename_group(id, "Mesa e cadeiras").unwrap();
        assert_eq!(project.groups[0].name, "Mesa e cadeiras");
        assert_eq!(project.ungroup_elements(&[Element::Drawing(b)]), 1);
        assert!(project.group_of(Element::Drawing(a)).is_none());
    }
}
