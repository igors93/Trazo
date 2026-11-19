//! References to the elements of a project (a wall, a room, a door...), the
//! kinds they come in and the filter that says which kinds a selection by
//! window, by crossing or "select all" may pick.

use serde::{Deserialize, Serialize};

/// One element of the project, by kind and id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Fixture(u64),
    Wall(u64),
    Room(u64),
    Terrain(u64),
    Opening(u64),
    /// A line, arc, circle or other drawing entity.
    Drawing(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementKind {
    Terrain,
    Room,
    Wall,
    Opening,
    Fixture,
    Drawing,
}

impl Element {
    pub fn kind(self) -> ElementKind {
        match self {
            Element::Fixture(_) => ElementKind::Fixture,
            Element::Wall(_) => ElementKind::Wall,
            Element::Room(_) => ElementKind::Room,
            Element::Terrain(_) => ElementKind::Terrain,
            Element::Opening(_) => ElementKind::Opening,
            Element::Drawing(_) => ElementKind::Drawing,
        }
    }

    #[cfg(test)]
    pub fn id(self) -> u64 {
        match self {
            Element::Fixture(id)
            | Element::Wall(id)
            | Element::Room(id)
            | Element::Terrain(id)
            | Element::Opening(id)
            | Element::Drawing(id) => id,
        }
    }
}

impl ElementKind {
    pub const ALL: [ElementKind; 6] = [
        ElementKind::Room,
        ElementKind::Wall,
        ElementKind::Opening,
        ElementKind::Fixture,
        ElementKind::Drawing,
        ElementKind::Terrain,
    ];

    /// The kind's name in the plural, as filters and counts show it.
    pub fn plural(self) -> &'static str {
        match self {
            ElementKind::Terrain => "Terreno",
            ElementKind::Room => "Cômodos",
            ElementKind::Wall => "Paredes",
            ElementKind::Opening => "Aberturas",
            ElementKind::Fixture => "Objetos",
            ElementKind::Drawing => "Desenhos",
        }
    }
}

/// Which kinds of element a selection by window or crossing, or "select
/// all", picks. A click always picks what is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectionFilter {
    pub rooms: bool,
    pub walls: bool,
    pub openings: bool,
    pub fixtures: bool,
    pub drawings: bool,
    pub terrain: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self {
            rooms: true,
            walls: true,
            openings: true,
            fixtures: true,
            drawings: true,
            terrain: true,
        }
    }
}

impl SelectionFilter {
    pub fn accepts(self, kind: ElementKind) -> bool {
        match kind {
            ElementKind::Room => self.rooms,
            ElementKind::Wall => self.walls,
            ElementKind::Opening => self.openings,
            ElementKind::Fixture => self.fixtures,
            ElementKind::Drawing => self.drawings,
            ElementKind::Terrain => self.terrain,
        }
    }

    /// Lets `kind` through or not.
    pub fn set(&mut self, kind: ElementKind, on: bool) {
        let slot = match kind {
            ElementKind::Room => &mut self.rooms,
            ElementKind::Wall => &mut self.walls,
            ElementKind::Opening => &mut self.openings,
            ElementKind::Fixture => &mut self.fixtures,
            ElementKind::Drawing => &mut self.drawings,
            ElementKind::Terrain => &mut self.terrain,
        };
        *slot = on;
    }

    /// Whether every kind gets through.
    pub fn is_open(self) -> bool {
        self == Self::default()
    }

    /// A filter that lets only `kind` through.
    #[cfg(test)]
    pub fn only(kind: ElementKind) -> Self {
        let mut filter = Self {
            rooms: false,
            walls: false,
            openings: false,
            fixtures: false,
            drawings: false,
            terrain: false,
        };
        filter.set(kind, true);
        filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_let_through_the_kinds_they_name() {
        let mut filter = SelectionFilter::default();
        assert!(filter.is_open());
        assert!(
            ElementKind::ALL
                .into_iter()
                .all(|kind| filter.accepts(kind))
        );
        filter.set(ElementKind::Terrain, false);
        assert!(!filter.accepts(ElementKind::Terrain) && filter.accepts(ElementKind::Wall));
        assert!(!filter.is_open());
        let walls = SelectionFilter::only(ElementKind::Wall);
        assert_eq!(
            ElementKind::ALL
                .into_iter()
                .filter(|kind| walls.accepts(*kind))
                .count(),
            1
        );
        assert_eq!(Element::Room(7).kind(), ElementKind::Room);
        assert_eq!(Element::Drawing(9).id(), 9);
        // Old settings files without a filter read as "everything".
        let read: SelectionFilter = serde_json::from_str("{\"walls\": false}").unwrap();
        assert!(!read.walls && read.rooms && read.terrain);
    }
}
