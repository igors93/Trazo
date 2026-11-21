//! Where a room's name and size go: like a room tag placed by hand, at the
//! free spot nearest the middle of the room, clear of furniture symbols and
//! door swings. Shared by the canvas and the exported drawings, so the
//! screen and the PDF agree.

use super::fixtures::SymbolShape;
use super::geometry::Point;
use super::layers::LayerCategory;
use super::model::{OpeningKind, Project};
use super::openings::DoorType;

/// An axis-aligned box in any 2D space: model metres, screen pixels or SVG
/// units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Area {
    /// The box spanning two opposite corners, in any order.
    pub fn from_corners(a: (f64, f64), b: (f64, f64)) -> Self {
        Self {
            min_x: a.0.min(b.0),
            min_y: a.1.min(b.1),
            max_x: a.0.max(b.0),
            max_y: a.1.max(b.1),
        }
    }

    pub fn centered(center: (f64, f64), size: (f64, f64)) -> Self {
        Self {
            min_x: center.0 - size.0 / 2.0,
            min_y: center.1 - size.1 / 2.0,
            max_x: center.0 + size.0 / 2.0,
            max_y: center.1 + size.1 / 2.0,
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    /// Whether the boxes overlap or come closer than `gap`.
    pub fn intersects(&self, other: &Self, gap: f64) -> bool {
        self.min_x < other.max_x + gap
            && other.min_x < self.max_x + gap
            && self.min_y < other.max_y + gap
            && other.min_y < self.max_y + gap
    }
}

/// Where a label of `size` fits inside `inner` without touching anything
/// `blocked`: the free spot nearest the middle, or `None`.
pub fn label_spot(
    inner: Area,
    size: (f64, f64),
    blocked: &dyn Fn(Area) -> bool,
) -> Option<(f64, f64)> {
    const STEPS: i32 = 10;
    let center = inner.center();
    let reach_x = ((inner.width() - size.0) / 2.0).max(0.0);
    let reach_y = ((inner.height() - size.1) / 2.0).max(0.0);
    let mut spots: Vec<(f64, f64)> = (-STEPS..=STEPS)
        .flat_map(|i| (-STEPS..=STEPS).map(move |j| (i, j)))
        .map(|(i, j)| {
            (
                center.0 + reach_x * f64::from(i) / f64::from(STEPS),
                center.1 + reach_y * f64::from(j) / f64::from(STEPS),
            )
        })
        .collect();
    let distance = |spot: &(f64, f64)| (spot.0 - center.0).powi(2) + (spot.1 - center.1).powi(2);
    spots.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    spots
        .into_iter()
        .find(|spot| !blocked(Area::centered(*spot, size)))
}

/// Nudges `PlanMark::Text` anchors so no two labels overlap each other or any
/// pre-existing `obstacles` (e.g. electrical device tags), picking the closest
/// free spot around each label's preferred anchor.
pub fn resolve_plan_mark_labels(marks: &mut [super::conduits::PlanMark], obstacles: &[Area]) {
    const GAP: f64 = 0.03;
    const RADII: [f64; 6] = [0.16, 0.30, 0.46, 0.66, 0.90, 1.20];
    const ANGLES: usize = 12;

    let mut placed: Vec<Area> = obstacles.to_vec();
    for mark in marks.iter_mut() {
        let super::conduits::PlanMark::Text { anchor, text } = mark else {
            continue;
        };
        let size = super::electrical::tag_size(text);
        let preferred = Area::centered((anchor.x, anchor.y), size);
        let collides = |candidate: &Area, boxes: &[Area]| {
            boxes.iter().any(|other| candidate.intersects(other, GAP))
        };
        if !collides(&preferred, &placed) {
            placed.push(preferred);
            continue;
        }
        let mut chosen: Option<(Point, Area)> = None;
        'rings: for &radius in &RADII {
            for step in 0..ANGLES {
                let theta = std::f64::consts::TAU * (step as f64) / (ANGLES as f64);
                let pt = Point::new(
                    anchor.x + radius * theta.cos(),
                    anchor.y + radius * theta.sin(),
                );
                let box_at = Area::centered((pt.x, pt.y), size);
                if !collides(&box_at, &placed) {
                    chosen = Some((pt, box_at));
                    break 'rings;
                }
            }
        }
        if let Some((pt, box_at)) = chosen {
            *anchor = pt;
            placed.push(box_at);
        } else {
            placed.push(preferred);
        }
    }
}

impl Project {
    /// What a room label should keep clear of, as model-space corner pairs:
    /// each furniture symbol (chairs included) and the area each door leaf
    /// sweeps. Hidden layers are left out.
    pub fn label_obstacles(&self) -> Vec<(Point, Point)> {
        let mut boxes = Vec::new();
        {
            for fixture in &self.fixtures {
                if !self.category_visible(Self::fixture_category(fixture)) {
                    continue;
                }
                let (mut low, mut high) = fixture.bounds();
                let mut include = |point: Point| {
                    low = Point::new(low.x.min(point.x), low.y.min(point.y));
                    high = Point::new(high.x.max(point.x), high.y.max(point.y));
                };
                for shape in fixture.symbol() {
                    match shape {
                        SymbolShape::Polyline { points, .. } => {
                            for point in points {
                                include(fixture.to_world(point));
                            }
                        }
                        SymbolShape::Circle { center, radius, .. } => {
                            let middle = fixture.to_world(center);
                            include(Point::new(middle.x - radius, middle.y - radius));
                            include(Point::new(middle.x + radius, middle.y + radius));
                        }
                    }
                }
                boxes.push((low, high));
            }
        }
        if self.category_visible(LayerCategory::Electrical) {
            for (anchor, text) in self.cached_electrical_tags().iter() {
                let (width, height) = super::electrical::tag_size(text);
                let half = Point::new(width / 2.0, height / 2.0);
                boxes.push((anchor.minus(half), anchor.translated(half)));
            }
            if !self.conduits.is_empty() {
                for mark in self.cached_conduit_marks().iter() {
                    if let super::conduits::PlanMark::Text { anchor, text } = mark {
                        let (width, height) = super::electrical::tag_size(text);
                        let half = Point::new(width / 2.0, height / 2.0);
                        boxes.push((anchor.minus(half), anchor.translated(half)));
                    }
                }
            }
        }
        if self.category_visible(LayerCategory::Structure)
            && (!self.structure.beams.is_empty()
                || !self.structure.slabs.is_empty()
                || self.fixtures.iter().any(|f| f.kind.is_structural()))
        {
            for mark in self.cached_structure_marks().iter() {
                if let super::conduits::PlanMark::Text { anchor, text } = mark {
                    let (width, height) = super::electrical::tag_size(text);
                    let half = Point::new(width / 2.0, height / 2.0);
                    boxes.push((anchor.minus(half), anchor.translated(half)));
                }
            }
        }
        if self.category_visible(LayerCategory::Openings) {
            for opening in &self.openings {
                let Some(wall) = self.walls.iter().find(|wall| wall.id == opening.wall_id) else {
                    continue;
                };
                let Some(along) = wall.end.minus(wall.start).normalized() else {
                    continue;
                };
                let half = opening.width() / 2.0;
                // How far the leaves reach, and whether they swing both ways.
                let (reach, both_sides) = match opening.kind {
                    OpeningKind::Door | OpeningKind::Gate => match opening.effective_door_type() {
                        DoorType::Sliding | DoorType::PocketSliding => continue,
                        DoorType::DoubleSwing => (half, false),
                        DoorType::Pivot => (opening.width() * 0.8, false),
                        DoorType::Bifold => (opening.width() * 0.35, false),
                        _ => (opening.width(), false),
                    },
                    OpeningKind::SaloonDoor => (half, true),
                    _ => continue,
                };
                let normal = Point::new(-along.y, along.x);
                let side = if opening.flip_side {
                    normal.scaled(-1.0)
                } else {
                    normal
                };
                let center = wall.start.lerp(wall.end, opening.t);
                let from = center.translated(along.scaled(-half));
                let to = center.translated(along.scaled(half));
                let signs: &[f64] = if both_sides { &[1.0, -1.0] } else { &[1.0] };
                for sign in signs {
                    let push = side.scaled(reach * sign);
                    let corners = [from, to, from.translated(push), to.translated(push)];
                    let low = corners.iter().fold(corners[0], |low, p| {
                        Point::new(low.x.min(p.x), low.y.min(p.y))
                    });
                    let high = corners.iter().fold(corners[0], |high, p| {
                        Point::new(high.x.max(p.x), high.y.max(p.y))
                    });
                    boxes.push((low, high));
                }
            }
        }
        boxes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_moves_off_the_furniture_to_the_nearest_free_spot() {
        let inner = Area::from_corners((0.0, 0.0), (200.0, 160.0));
        let size = (60.0, 30.0);
        // Nothing in the way: the middle of the room.
        assert_eq!(label_spot(inner, size, &|_| false), Some(inner.center()));
        // A table in the middle: the nearest free spot, still inside.
        let table = Area::centered(inner.center(), (120.0, 60.0));
        let spot = label_spot(inner, size, &|block| block.intersects(&table, 0.0)).unwrap();
        let block = Area::centered(spot, size);
        let inside = block.min_x >= inner.min_x
            && block.max_x <= inner.max_x
            && block.min_y >= inner.min_y
            && block.max_y <= inner.max_y;
        assert!(!block.intersects(&table, 0.0) && inside);
        assert!(
            (spot.0 - inner.center().0).abs() < 1e-9,
            "moves up or down first"
        );
        // A room full of furniture has no free spot.
        assert_eq!(label_spot(inner, size, &|_| true), None);
    }

    #[test]
    fn furniture_and_door_swings_are_obstacles() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        assert!(project.label_obstacles().is_empty());
        let wall = project.walls[0].id;
        project
            .add_opening(wall, OpeningKind::Door, 0.5)
            .expect("door on the wall");
        let obstacles = project.label_obstacles();
        assert_eq!(obstacles.len(), 1, "one leaf sweep");
        let (low, high) = obstacles[0];
        // The sweep reaches about one door width off the wall.
        let depth = (high.x - low.x).min(high.y - low.y);
        assert!(depth > 0.5, "{depth}");
    }

    #[test]
    fn conduit_and_structure_labels_avoid_overlapping_at_crowded_junctions() {
        let mut project = super::super::samples::SampleProject::ALL[0].load_embedded();
        project.generate_structure();
        project.generate_electrical_points();
        if project.main_panel().is_none() {
            let (pw, pd) = super::super::fixtures::FixtureKind::Panel.default_size();
            let room_anchor = project.room_label_anchor(&project.rooms[0]).unwrap().0;
            project
                .add_fixture(
                    super::super::fixtures::FixtureKind::Panel,
                    room_anchor,
                    pw,
                    pd,
                    0,
                )
                .unwrap();
        }
        project.assign_circuits();
        project.generate_conduits().unwrap();

        let assert_no_overlaps = |marks: &[super::super::conduits::PlanMark], kind: &str| {
            let boxes: Vec<(Area, &str)> = marks
                .iter()
                .filter_map(|m| match m {
                    super::super::conduits::PlanMark::Text { anchor, text } => Some((
                        Area::centered(
                            (anchor.x, anchor.y),
                            super::super::electrical::tag_size(text),
                        ),
                        text.as_str(),
                    )),
                    _ => None,
                })
                .collect();
            assert!(!boxes.is_empty(), "{kind} should produce labels");
            for i in 0..boxes.len() {
                for j in i + 1..boxes.len() {
                    assert!(
                        !boxes[i].0.intersects(&boxes[j].0, 0.0),
                        "{kind} labels overlap: '{}' ({:?}) vs '{}' ({:?})",
                        boxes[i].1,
                        boxes[i].0,
                        boxes[j].1,
                        boxes[j].0
                    );
                }
            }
        };

        assert_no_overlaps(&project.structure_marks(), "structure");
        assert_no_overlaps(&project.conduit_marks(), "conduit");
    }
}
