//! The project seen as a set of elements: where each one lies, which ones a
//! window or a crossing box picks, which are alike, and moving or deleting
//! several at once as one change.

use std::collections::HashSet;

use super::Project;
use crate::domain::element::{Element, SelectionFilter};
use crate::domain::geometry::{EPSILON, Point};

/// Whether `point` lies in the box `min`–`max` (edges included).
fn inside_box(point: Point, min: Point, max: Point) -> bool {
    point.x >= min.x - EPSILON
        && point.x <= max.x + EPSILON
        && point.y >= min.y - EPSILON
        && point.y <= max.y + EPSILON
}

/// Whether the segment `a`-`b` touches the box `min`–`max` (Liang–Barsky).
fn segment_meets_box(a: Point, b: Point, min: Point, max: Point) -> bool {
    let d = b.minus(a);
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-d.x, a.x - min.x),
        (d.x, max.x - a.x),
        (-d.y, a.y - min.y),
        (d.y, max.y - a.y),
    ] {
        if p.abs() <= EPSILON {
            if q < -EPSILON {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > leave {
                    return false;
                }
                enter = enter.max(r);
            } else {
                if r < enter {
                    return false;
                }
                leave = leave.min(r);
            }
        }
    }
    enter <= leave + EPSILON
}

/// The four corners of a band of `half` width around `a`-`b`, reaching
/// `reach` past each end, closed back on the first corner.
fn band(a: Point, b: Point, half: f64, reach: f64) -> Vec<Point> {
    let Some(along) = b.minus(a).normalized() else {
        return vec![a];
    };
    let normal = Point::new(-along.y, along.x);
    let from = a.translated(along.scaled(-reach));
    let to = b.translated(along.scaled(reach));
    let first = from.translated(normal.scaled(half));
    vec![
        first,
        to.translated(normal.scaled(half)),
        to.translated(normal.scaled(-half)),
        from.translated(normal.scaled(-half)),
        first,
    ]
}

/// The smallest upright box around `points`.
fn span(points: &[Point]) -> Option<(Point, Point)> {
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(low, high), point| {
        (
            Point::new(low.x.min(point.x), low.y.min(point.y)),
            Point::new(high.x.max(point.x), high.y.max(point.y)),
        )
    }))
}

impl Project {
    /// Whether the element is still in the project.
    pub fn element_exists(&self, element: Element) -> bool {
        match element {
            Element::Wall(id) => self.walls.iter().any(|wall| wall.id == id),
            Element::Room(id) => self.rooms.iter().any(|room| room.id == id),
            Element::Opening(id) => self.openings.iter().any(|opening| opening.id == id),
            Element::Fixture(id) => self.fixtures.iter().any(|fixture| fixture.id == id),
            Element::Drawing(id) => self.drawing(id).is_some(),
            Element::Terrain(id) => self.terrain.as_ref().is_some_and(|t| t.id == id),
        }
    }

    /// The element's outline on the plan as a path (closed outlines end on
    /// their first point): a wall's band with its thickness, a room's
    /// sides, an opening's stretch of wall, a fixture's footprint, a
    /// drawing's line and the terrain's edge.
    pub fn element_outline(&self, element: Element) -> Option<Vec<Point>> {
        let rectangle = |low: Point, high: Point| {
            vec![
                low,
                Point::new(high.x, low.y),
                high,
                Point::new(low.x, high.y),
                low,
            ]
        };
        match element {
            Element::Wall(id) => {
                let wall = self.walls.iter().find(|wall| wall.id == id)?;
                let half = wall.thickness() / 2.0;
                Some(band(wall.start, wall.end, half, half))
            }
            Element::Room(id) => {
                let room = self.rooms.iter().find(|room| room.id == id)?;
                if let Some(mut polygon) = self.room_polygon(room) {
                    polygon.push(polygon[0]);
                    Some(polygon)
                } else {
                    let (low, high) = self.room_bounds(room)?;
                    Some(rectangle(low, high))
                }
            }
            Element::Opening(id) => {
                let opening = self.openings.iter().find(|opening| opening.id == id)?;
                let wall = self.walls.iter().find(|wall| wall.id == opening.wall_id)?;
                let length = wall.length().max(EPSILON);
                let half = opening.width() / 2.0 / length;
                let from = wall.start.lerp(wall.end, opening.t - half);
                let to = wall.start.lerp(wall.end, opening.t + half);
                Some(band(from, to, wall.thickness() / 2.0, 0.0))
            }
            Element::Fixture(id) => {
                let fixture = self.fixtures.iter().find(|fixture| fixture.id == id)?;
                let (low, high) = fixture.bounds();
                Some(rectangle(low, high))
            }
            Element::Drawing(id) => Some(self.drawing(id)?.shape.outline()),
            Element::Terrain(id) => {
                let terrain = self.terrain.as_ref().filter(|terrain| terrain.id == id)?;
                let mut outline = terrain.outline();
                outline.push(outline[0]);
                Some(outline)
            }
        }
    }

    /// The upright box around the element.
    pub fn element_bounds(&self, element: Element) -> Option<(Point, Point)> {
        span(&self.element_outline(element)?)
    }

    /// The box around all of `elements`.
    pub fn elements_bounds(&self, elements: &[Element]) -> Option<(Point, Point)> {
        let corners: Vec<Point> = elements
            .iter()
            .filter_map(|element| self.element_bounds(*element))
            .flat_map(|(low, high)| [low, high])
            .collect();
        span(&corners)
    }

    /// Whether the element lies wholly inside the box (selection by window).
    pub fn element_in_window(&self, element: Element, min: Point, max: Point) -> bool {
        self.element_outline(element)
            .is_some_and(|outline| outline.iter().all(|point| inside_box(*point, min, max)))
    }

    /// Whether the element's outline touches the box or lies inside it
    /// (selection by crossing). A box drawn inside a room or the terrain
    /// does not pick them: their edge must be reached.
    pub fn element_crosses_box(&self, element: Element, min: Point, max: Point) -> bool {
        self.element_outline(element).is_some_and(|outline| {
            if outline.len() == 1 {
                return inside_box(outline[0], min, max);
            }
            outline
                .windows(2)
                .any(|pair| segment_meets_box(pair[0], pair[1], min, max))
        })
    }

    /// Every element: the terrain, rooms, walls, openings, fixtures and
    /// drawings, in that order.
    pub fn elements(&self) -> Vec<Element> {
        self.terrain
            .iter()
            .map(|terrain| Element::Terrain(terrain.id))
            .chain(self.rooms.iter().map(|room| Element::Room(room.id)))
            .chain(self.walls.iter().map(|wall| Element::Wall(wall.id)))
            .chain(self.openings.iter().map(|o| Element::Opening(o.id)))
            .chain(self.fixtures.iter().map(|f| Element::Fixture(f.id)))
            .chain(self.drawings.iter().map(|d| Element::Drawing(d.id)))
            .collect()
    }

    /// The elements the filter lets through ("select all").
    pub fn filtered_elements(&self, filter: SelectionFilter) -> Vec<Element> {
        self.elements()
            .into_iter()
            .filter(|element| filter.accepts(element.kind()) && self.element_visible(*element))
            .collect()
    }

    /// Whether the element's layer is shown (hidden ones are not picked by
    /// a box or "select all").
    pub fn element_visible(&self, element: Element) -> bool {
        use crate::domain::layers::LayerCategory;
        match element {
            Element::Terrain(_) => self.category_visible(LayerCategory::Terrain),
            Element::Room(_) => self.category_visible(LayerCategory::Rooms),
            Element::Opening(_) => self.category_visible(LayerCategory::Openings),
            Element::Fixture(_) => self.category_visible(LayerCategory::Fixtures),
            Element::Wall(id) => self
                .walls
                .iter()
                .find(|wall| wall.id == id)
                .is_some_and(|wall| self.is_wall_visible(wall)),
            Element::Drawing(id) => self
                .drawing(id)
                .is_some_and(|drawing| self.is_drawing_visible(drawing)),
        }
    }

    /// The elements a box from `a` to `b` picks: those wholly inside it,
    /// or with `crossing` also those it touches.
    pub fn elements_in_box(
        &self,
        a: Point,
        b: Point,
        crossing: bool,
        filter: SelectionFilter,
    ) -> Vec<Element> {
        let min = Point::new(a.x.min(b.x), a.y.min(b.y));
        let max = Point::new(a.x.max(b.x), a.y.max(b.y));
        self.filtered_elements(filter)
            .into_iter()
            .filter(|element| {
                if crossing {
                    self.element_crosses_box(*element, min, max)
                } else {
                    self.element_in_window(*element, min, max)
                }
            })
            .collect()
    }

    /// The elements like `element`: walls of the same thickness, every
    /// room, openings of the same kind and size, fixtures of the same kind
    /// and drawings of the same shape.
    pub fn similar_elements(&self, element: Element) -> Vec<Element> {
        match element {
            Element::Wall(id) => {
                let Some(thickness) = self
                    .walls
                    .iter()
                    .find(|wall| wall.id == id)
                    .map(|wall| wall.thickness())
                else {
                    return Vec::new();
                };
                self.walls
                    .iter()
                    .filter(|wall| (wall.thickness() - thickness).abs() < 5e-4)
                    .map(|wall| Element::Wall(wall.id))
                    .collect()
            }
            Element::Room(_) => self.rooms.iter().map(|r| Element::Room(r.id)).collect(),
            Element::Opening(id) => {
                let Some(model) = self.openings.iter().find(|o| o.id == id) else {
                    return Vec::new();
                };
                let same_size = |a: Option<f64>, b: Option<f64>| match (a, b) {
                    (Some(a), Some(b)) => (a - b).abs() < 5e-3,
                    (None, None) => true,
                    _ => false,
                };
                self.openings
                    .iter()
                    .filter(|o| {
                        o.kind == model.kind
                            && (o.width() - model.width()).abs() < 5e-3
                            && same_size(o.height(), model.height())
                    })
                    .map(|o| Element::Opening(o.id))
                    .collect()
            }
            Element::Fixture(id) => {
                let Some(kind) = self.fixtures.iter().find(|f| f.id == id).map(|f| f.kind) else {
                    return Vec::new();
                };
                self.fixtures
                    .iter()
                    .filter(|f| f.kind == kind)
                    .map(|f| Element::Fixture(f.id))
                    .collect()
            }
            Element::Drawing(id) => {
                let Some(label) = self.drawing(id).map(|d| d.shape.label()) else {
                    return Vec::new();
                };
                self.drawings
                    .iter()
                    .filter(|d| d.shape.label() == label)
                    .map(|d| Element::Drawing(d.id))
                    .collect()
            }
            Element::Terrain(id) => {
                if self.element_exists(element) {
                    vec![Element::Terrain(id)]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// The walls that move with `elements`: the selected walls and those of
    /// the selected rooms.
    pub(super) fn walls_of(&self, elements: &[Element]) -> HashSet<u64> {
        let mut walls = HashSet::new();
        for element in elements {
            match *element {
                Element::Wall(id) if self.walls.iter().any(|wall| wall.id == id) => {
                    walls.insert(id);
                }
                Element::Room(id) => {
                    if let Some(room) = self.rooms.iter().find(|room| room.id == id) {
                        walls.extend(room.wall_ids.iter().copied());
                    }
                }
                _ => {}
            }
        }
        walls
    }

    /// Refuses to carry a selected room away from a neighbour it shares a
    /// wall with, as moving a single room does.
    pub(super) fn check_rooms_move_whole(
        &self,
        elements: &[Element],
        walls: &HashSet<u64>,
    ) -> Result<(), String> {
        for element in elements {
            let Element::Room(id) = *element else {
                continue;
            };
            let Some(room) = self.rooms.iter().find(|room| room.id == id) else {
                continue;
            };
            let torn = self.rooms.iter().any(|other| {
                other.id != id
                    && other.wall_ids.iter().any(|w| room.wall_ids.contains(w))
                    && other.wall_ids.iter().any(|w| !walls.contains(w))
            });
            if torn {
                return Err(format!(
                    "O cômodo \"{}\" compartilha paredes com cômodos fora da seleção; selecione-os juntos ou mova as paredes.",
                    room.display_name()
                ));
            }
        }
        Ok(())
    }

    /// Moves `elements` together by `delta`, as one change: walls (with
    /// the walls joined to them stretching to follow), whole rooms,
    /// fixtures and drawings; doors and windows on walls that stay slide
    /// along them. The move stops at the terrain's edge; the terrain itself
    /// carries the whole plan.
    pub fn move_elements(&mut self, elements: &[Element], delta: Point) -> Result<(), String> {
        if !delta.is_finite() {
            return Err("Deslocamento inválido.".to_owned());
        }
        if let Some(Element::Terrain(id)) = elements
            .iter()
            .find(|element| matches!(element, Element::Terrain(_)))
            .copied()
        {
            self.move_terrain(id, delta);
            return Ok(());
        }
        let walls = self.walls_of(elements);
        self.check_rooms_move_whole(elements, &walls)?;
        let fixtures: Vec<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Fixture(id) => Some(*id),
                _ => None,
            })
            .collect();

        // Stop at the terrain's edge, for everything that must stay in it.
        let mut held: Vec<Point> = Vec::new();
        for wall in self.walls.iter().filter(|wall| walls.contains(&wall.id)) {
            held.extend([wall.start, wall.end]);
        }
        for fixture in self.fixtures.iter().filter(|f| fixtures.contains(&f.id)) {
            let (low, high) = fixture.bounds();
            held.extend([low, high]);
        }
        let delta = match span(&held) {
            Some((low, high)) => self.clamp_delta_to_terrain(low, high, delta),
            None => delta,
        };
        if delta.x.abs() <= EPSILON && delta.y.abs() <= EPSILON {
            return Ok(());
        }

        let mut tx = self.clone();
        if !walls.is_empty() {
            let primary: Vec<u64> = walls.iter().copied().collect();
            let anchors: Vec<Point> = self
                .walls
                .iter()
                .filter(|wall| walls.contains(&wall.id))
                .flat_map(|wall| [wall.start, wall.end])
                .collect();
            // Clamped already; this also keeps the walls in the buildable limit.
            tx.translate_wall_anchors(&primary, &anchors, delta, true)?;
        }
        for fixture in tx.fixtures.iter_mut().filter(|f| fixtures.contains(&f.id)) {
            fixture.center = fixture.center.translated(delta).rounded();
        }
        if let Some(fixture) = tx
            .fixtures
            .iter()
            .filter(|f| fixtures.contains(&f.id))
            .find(|fixture| !tx.fixture_fits(fixture))
        {
            return Err(format!(
                "{} sairia do terreno.",
                fixture.kind.label().to_owned()
            ));
        }
        for element in elements {
            match *element {
                Element::Drawing(id) => tx.move_drawing(id, delta),
                Element::Opening(id) => {
                    let Some(opening) = tx.openings.iter().find(|o| o.id == id) else {
                        continue;
                    };
                    if walls.contains(&opening.wall_id) {
                        continue;
                    }
                    let Some(wall) = tx.walls.iter().find(|w| w.id == opening.wall_id) else {
                        continue;
                    };
                    let center = wall.start.lerp(wall.end, opening.t);
                    tx.move_opening_along_wall(id, center.translated(delta));
                }
                _ => {}
            }
        }
        *self = tx;
        Ok(())
    }

    /// Deletes `elements` as one change; returns how many were there.
    /// Rooms take their own walls (not those shared with a room that
    /// stays), and walls take their doors and windows.
    pub fn delete_elements(&mut self, elements: &[Element]) -> usize {
        let present = elements
            .iter()
            .filter(|element| self.element_exists(**element))
            .count();
        // Openings and loose items first, then rooms, walls and terrain.
        let rank = |element: &Element| match element {
            Element::Opening(_) => 0,
            Element::Fixture(_) | Element::Drawing(_) => 1,
            Element::Room(_) => 2,
            Element::Wall(_) => 3,
            Element::Terrain(_) => 4,
        };
        let mut ordered = elements.to_vec();
        ordered.sort_by_key(rank);
        for element in ordered {
            match element {
                Element::Opening(id) => {
                    self.delete_opening(id);
                }
                Element::Fixture(id) => {
                    self.delete_fixture(id);
                }
                Element::Drawing(id) => {
                    self.delete_drawing(id);
                }
                Element::Room(id) => {
                    self.delete_room(id);
                }
                Element::Wall(id) => self.delete_wall(id),
                Element::Terrain(id) => {
                    self.delete_terrain(id);
                }
            }
        }
        present
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;
    use crate::domain::element::ElementKind;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::model::OpeningKind;

    /// Two rooms side by side, a door, a sink and a circle out to the right.
    fn plan() -> (Project, u64, u64, u64, u64, u64) {
        let mut project = Project::default();
        let left = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let right = project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
            .unwrap();
        let bottom = project.rooms[0].wall_ids[0];
        let door = project.add_opening(bottom, OpeningKind::Door, 0.5).unwrap();
        let sink = project
            .add_fixture(FixtureKind::KitchenSink, Point::new(2.0, 1.5), 1.2, 0.6, 0)
            .unwrap();
        let circle = project
            .add_drawing(Shape::Circle {
                center: Point::new(12.0, 1.5),
                radius: 1.0,
            })
            .unwrap();
        (project, left, right, door, sink, circle)
    }

    #[test]
    fn window_picks_what_is_inside_and_crossing_what_it_touches() {
        let (project, left, right, door, sink, circle) = plan();
        let all = SelectionFilter::default();

        // A window around the left room (with its wall thickness).
        let window =
            project.elements_in_box(Point::new(-0.5, -0.5), Point::new(4.2, 3.5), false, all);
        assert!(window.contains(&Element::Room(left)));
        assert!(window.contains(&Element::Opening(door)));
        assert!(window.contains(&Element::Fixture(sink)));
        assert!(!window.contains(&Element::Room(right)));
        assert!(!window.contains(&Element::Drawing(circle)));
        // The shared wall lies within the window's reach too.
        assert_eq!(
            window
                .iter()
                .filter(|element| element.kind() == ElementKind::Wall)
                .count(),
            4
        );

        // A crossing box over the shared wall picks both rooms and the wall,
        // but no box inside a room picks the room.
        let crossing =
            project.elements_in_box(Point::new(3.9, 1.0), Point::new(4.1, 2.0), true, all);
        assert!(crossing.contains(&Element::Room(left)));
        assert!(crossing.contains(&Element::Room(right)));
        assert!(!crossing.contains(&Element::Fixture(sink)));
        let inside = project.elements_in_box(Point::new(5.0, 1.0), Point::new(6.0, 2.0), true, all);
        assert!(inside.is_empty());
        // The circle is picked by touching its line.
        let touch =
            project.elements_in_box(Point::new(12.8, 1.0), Point::new(13.5, 2.0), true, all);
        assert_eq!(touch, vec![Element::Drawing(circle)]);

        // The filter keeps out what it does not name.
        let rooms_only = project.elements_in_box(
            Point::new(-1.0, -1.0),
            Point::new(20.0, 5.0),
            false,
            SelectionFilter::only(ElementKind::Room),
        );
        assert_eq!(rooms_only, vec![Element::Room(left), Element::Room(right)]);
        assert_eq!(
            project
                .filtered_elements(SelectionFilter::only(ElementKind::Fixture))
                .len(),
            1
        );
    }

    #[test]
    fn similar_elements_share_kind_and_size() {
        let (mut project, left, _, door, _, circle) = plan();
        let top = project.rooms[0].wall_ids[2];
        let second = project.add_opening(top, OpeningKind::Door, 0.5).unwrap();
        let wide = project
            .add_opening_sized(top, OpeningKind::Door, 0.15, Some(0.7))
            .unwrap();
        let doors = project.similar_elements(Element::Opening(door));
        assert!(doors.contains(&Element::Opening(second)));
        assert!(!doors.contains(&Element::Opening(wide)));
        assert_eq!(project.similar_elements(Element::Room(left)).len(), 2);
        assert_eq!(project.similar_elements(Element::Wall(top)).len(), 7);
        assert_eq!(
            project.similar_elements(Element::Drawing(circle)),
            vec![Element::Drawing(circle)]
        );
        assert!(project.similar_elements(Element::Wall(999)).is_empty());
    }

    #[test]
    fn several_elements_move_together_and_stop_at_the_terrain() {
        let (mut project, left, right, door, sink, circle) = plan();
        let before_door = project.element_bounds(Element::Opening(door)).unwrap();
        project
            .move_elements(
                &[
                    Element::Room(left),
                    Element::Room(right),
                    Element::Fixture(sink),
                    Element::Drawing(circle),
                ],
                Point::new(1.0, 2.0),
            )
            .unwrap();
        let room = |project: &Project, id: u64| {
            project
                .element_bounds(Element::Room(id))
                .map(|(low, _)| low)
                .unwrap()
        };
        assert_eq!(room(&project, left), Point::new(1.0, 2.0));
        assert_eq!(room(&project, right), Point::new(5.0, 2.0));
        assert_eq!(project.fixtures[0].center, Point::new(3.0, 3.5));
        assert_eq!(
            project.drawings[0].shape,
            Shape::Circle {
                center: Point::new(13.0, 3.5),
                radius: 1.0
            }
        );
        // The door rode along on its wall.
        let after_door = project.element_bounds(Element::Opening(door)).unwrap();
        assert!(
            after_door
                .0
                .near(before_door.0.translated(Point::new(1.0, 2.0)), 1e-6)
        );

        // One room alone cannot leave its neighbour behind.
        let before = project.clone();
        assert!(
            project
                .move_elements(&[Element::Room(left)], Point::new(0.0, 1.0))
                .is_err()
        );
        assert_eq!(project, before);

        // Inside a terrain the group stops at the edge, all of it.
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        project
            .move_elements(
                &[Element::Room(left), Element::Room(right)],
                Point::new(5.0, 0.0),
            )
            .unwrap();
        assert_eq!(room(&project, right).x, 6.0);
        assert_eq!(room(&project, left).x, 2.0);
    }

    #[test]
    fn deleting_several_elements_is_one_change() {
        let (mut project, left, right, door, sink, circle) = plan();
        let removed = project.delete_elements(&[
            Element::Opening(door),
            Element::Room(left),
            Element::Fixture(sink),
            Element::Drawing(circle),
            Element::Wall(999),
        ]);
        assert_eq!(removed, 4);
        assert!(project.openings.is_empty() && project.fixtures.is_empty());
        assert!(project.drawings.is_empty());
        // The right room keeps the wall it shared.
        assert_eq!(project.rooms.len(), 1);
        assert_eq!(project.rooms[0].id, right);
        assert_eq!(project.walls.len(), 4);
    }
}
