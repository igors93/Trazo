//! Per-wall heights and sloped wall tops (#145).
//!
//! A wall may rise above or stop below the storey height, and its top may
//! slope from one end to the other: the gables under a mono-pitch roof, or
//! a taller boundary wall that leaves a ventilation gap under the eaves.
//! Walls without their own height keep following the storey height.
//!
//! The heights feed the masonry takeoff (face = length × mean height), the
//! weight of the walls on beams and foundations, and the height of the
//! columns standing in the wall (own weight, bar lengths and slenderness).

use super::fixtures::Fixture;
use super::geometry::{Point, distance_to_segment};
use super::model::{Opening, Project, Wall};

/// Lowest and highest wall top accepted, metres.
pub const MIN_WALL_HEIGHT_M: f64 = 0.5;
pub const MAX_WALL_HEIGHT_M: f64 = 12.0;

/// Heights closer than this are taken as equal, metres.
const SAME_HEIGHT_M: f64 = 0.005;

impl Wall {
    /// Height of the top at the start and at the end, metres. Walls without
    /// their own height follow `storey`.
    pub fn heights(&self, storey: f64) -> (f64, f64) {
        let start = self.height_m.unwrap_or(storey);
        (start, self.height_end_m.unwrap_or(start))
    }

    /// Average height of the top (the face area over the length).
    pub fn mean_height(&self, storey: f64) -> f64 {
        let (start, end) = self.heights(storey);
        (start + end) / 2.0
    }

    /// Height of the top at the point of the wall nearest `point`.
    pub fn height_at(&self, storey: f64, point: Point) -> f64 {
        let (start, end) = self.heights(storey);
        let axis = self.end.minus(self.start);
        let length_sq = axis.dot(axis);
        if length_sq < 1e-12 {
            return start;
        }
        let t = (point.minus(self.start).dot(axis) / length_sq).clamp(0.0, 1.0);
        start + (end - start) * t
    }

    /// Height of the top over an opening's centre.
    pub fn height_over(&self, storey: f64, opening: &Opening) -> f64 {
        self.height_at(storey, self.start.lerp(self.end, opening.t))
    }

    /// Whether the wall has a height of its own.
    pub fn has_own_height(&self) -> bool {
        self.height_m.is_some() || self.height_end_m.is_some()
    }

    /// The `(height_m, height_end_m)` of the piece between the fractions
    /// `from` and `to` of this wall, so a sloped top stays continuous when
    /// the wall is split or trimmed.
    pub fn piece_heights(&self, storey: f64, from: f64, to: f64) -> (Option<f64>, Option<f64>) {
        if !self.has_own_height() {
            return (None, None);
        }
        let (start, end) = self.heights(storey);
        let at = |t: f64| start + (end - start) * t.clamp(0.0, 1.0);
        normalized(at(from), at(to))
    }
}

/// Stores a straight top as `(start, None)`.
fn normalized(start: f64, end: f64) -> (Option<f64>, Option<f64>) {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    if (end - start).abs() < SAME_HEIGHT_M {
        (Some(round(start)), None)
    } else {
        (Some(round(start)), Some(round(end)))
    }
}

impl Project {
    /// Gives wall `id` its own height: `start` at its start and `end` at
    /// its end (a sloped top), or a straight top when `end` is `None`.
    /// `start: None` puts the wall back on the storey height.
    pub fn set_wall_height(
        &mut self,
        id: u64,
        start: Option<f64>,
        end: Option<f64>,
    ) -> Result<(), String> {
        for value in [start, end].into_iter().flatten() {
            if !value.is_finite() || !(MIN_WALL_HEIGHT_M..=MAX_WALL_HEIGHT_M).contains(&value) {
                return Err(format!(
                    "A altura da parede deve ficar entre {:.2} m e {:.2} m.",
                    MIN_WALL_HEIGHT_M, MAX_WALL_HEIGHT_M
                )
                .replace('.', ","));
            }
        }
        let storey = self.wall_height;
        let wall = self
            .walls
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Parede não encontrada.")?;
        let (height, height_end) = match (start, end) {
            (None, None) => (None, None),
            (start, end) => {
                let start = start.unwrap_or(storey);
                let (height, height_end) = normalized(start, end.unwrap_or(start));
                // A straight top at the storey height is the default.
                if height_end.is_none() && (start - storey).abs() < SAME_HEIGHT_M {
                    (None, None)
                } else {
                    (height, height_end)
                }
            }
        };
        if wall.height_m == height && wall.height_end_m == height_end {
            return Err("A parede já tem essa altura.".to_owned());
        }
        wall.height_m = height;
        wall.height_end_m = height_end;
        Ok(())
    }

    /// Masonry face of a wall, m²: length × mean height, less the doors
    /// and windows (each up to the top over it).
    pub fn wall_net_area(&self, wall: &Wall) -> f64 {
        let storey = self.wall_height.max(MIN_WALL_HEIGHT_M);
        let openings: f64 = self
            .openings
            .iter()
            .filter(|o| o.wall_id == wall.id)
            .map(|o| {
                let top = wall.height_over(storey, o);
                o.width() * o.height().unwrap_or(top).min(top)
            })
            .sum();
        (wall.length() * wall.mean_height(storey) - openings).max(0.0)
    }

    /// Height of a column, metres: the top of the tallest wall it stands
    /// in, or the storey height when it stands free.
    pub fn column_height(&self, column: &Fixture) -> f64 {
        let reach = column.width.max(column.depth) / 2.0 + 0.06;
        self.walls
            .iter()
            .filter(|w| distance_to_segment(column.center, w.start, w.end) < reach)
            .map(|w| w.height_at(self.wall_height, column.center))
            .fold(None, |tallest: Option<f64>, h| {
                Some(tallest.map_or(h, |t| t.max(h)))
            })
            .unwrap_or(self.wall_height)
    }
}

/// Slenderness of a column of free height `height` and smallest side
/// `side`: λ = le / i = 3,46 · le / h, with le = the free height (column
/// braced at the top by the beams).
pub fn slenderness(height: f64, side: f64) -> f64 {
    12f64.sqrt() * height / side.max(1e-6)
}

/// Above this slenderness the simplified pre-sizing is not valid (NBR 6118
/// 15.8.3: the standard-column methods stop at λ = 90).
pub const MAX_SIMPLE_SLENDERNESS: f64 = 90.0;

/// Up to this slenderness the local second-order effects can be ignored
/// (λ1, NBR 6118 15.8.2, taken at its usual 35).
pub const SECOND_ORDER_SLENDERNESS: f64 = 35.0;

/// Smallest side, in whole 5 cm, that keeps a column of free height
/// `height` within the simplified methods (λ ≤ 90).
pub fn side_for_slenderness(height: f64) -> f64 {
    let side = 12f64.sqrt() * height / MAX_SIMPLE_SLENDERNESS;
    ((side * 100.0 / 5.0 - 1e-9).ceil() * 5.0 / 100.0).max(0.14)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    fn project_with_wall() -> (Project, u64) {
        let mut project = Project::default();
        project.wall_height = 2.8;
        let id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(10.0, 0.0))
            .expect("wall");
        (project, id)
    }

    #[test]
    fn a_wall_without_its_own_height_follows_the_storey() {
        let (project, id) = project_with_wall();
        let wall = project.walls.iter().find(|w| w.id == id).unwrap();
        assert_eq!(wall.heights(2.8), (2.8, 2.8));
        assert!(!wall.has_own_height());
        assert!((project.wall_net_area(wall) - 28.0).abs() < 1e-9);
    }

    #[test]
    fn a_sloped_top_gives_the_mean_face_and_interpolates_along_the_wall() {
        let (mut project, id) = project_with_wall();
        project.set_wall_height(id, Some(4.3), Some(2.5)).unwrap();
        let wall = project.walls.iter().find(|w| w.id == id).unwrap().clone();
        assert!((wall.mean_height(2.8) - 3.4).abs() < 1e-9);
        assert!((wall.height_at(2.8, Point::new(5.0, 0.3)) - 3.4).abs() < 1e-9);
        assert!((project.wall_net_area(&wall) - 34.0).abs() < 1e-9);
        // Same top again is refused; the storey height clears it.
        assert!(project.set_wall_height(id, Some(4.3), Some(2.5)).is_err());
        project.set_wall_height(id, Some(2.8), None).unwrap();
        assert!(
            !project
                .walls
                .iter()
                .find(|w| w.id == id)
                .unwrap()
                .has_own_height()
        );
        assert!(project.set_wall_height(id, Some(0.2), None).is_err());
    }

    #[test]
    fn a_taller_wall_weighs_more_on_its_foundation() {
        let (mut project, id) = project_with_wall();
        project.generate_wall_foundations();
        let low = project.wall_foundation_effects().results[0].load_kn_m;
        project.set_wall_height(id, Some(4.2), None).unwrap();
        let high = project.wall_foundation_effects().results[0].load_kn_m;
        // 1.40 m more of wall, the strip's own weight unchanged.
        let thickness = project.walls[0].thickness();
        let extra = project.structure.settings.masonry_kn_m3 * thickness * 1.4;
        assert!((high - low - extra).abs() < 1e-9, "{low} → {high}");
    }

    #[test]
    fn splitting_a_sloped_wall_keeps_the_top_continuous() {
        let (mut project, id) = project_with_wall();
        project.set_wall_height(id, Some(4.0), Some(2.0)).unwrap();
        let new_id = project.split_wall_at(id, Point::new(5.0, 0.0)).unwrap();
        let first = project.walls.iter().find(|w| w.id == id).unwrap();
        let second = project.walls.iter().find(|w| w.id == new_id).unwrap();
        assert_eq!((first.height_m, first.height_end_m), (Some(4.0), Some(3.0)));
        assert_eq!(
            (second.height_m, second.height_end_m),
            (Some(3.0), Some(2.0))
        );
    }

    #[test]
    fn a_column_in_a_tall_wall_is_as_tall_as_the_wall_and_may_be_slender() {
        let (mut project, id) = project_with_wall();
        project.set_wall_height(id, Some(4.3), None).unwrap();
        let column = project
            .add_fixture(FixtureKind::Column, Point::new(5.0, 0.0), 0.14, 0.30, 0)
            .unwrap();
        let free = project
            .add_fixture(FixtureKind::Column, Point::new(5.0, 4.0), 0.14, 0.30, 0)
            .unwrap();
        let fixture = |id| project.fixtures.iter().find(|f| f.id == id).unwrap();
        assert!((project.column_height(fixture(column)) - 4.3).abs() < 1e-9);
        assert!((project.column_height(fixture(free)) - 2.8).abs() < 1e-9);
        let lambda = slenderness(4.3, 0.14);
        assert!((lambda - 106.4).abs() < 0.1, "{lambda}");
        assert!((side_for_slenderness(4.3) - 0.20).abs() < 1e-9);
        assert!(slenderness(2.8, 0.14) < MAX_SIMPLE_SLENDERNESS);
    }

    #[test]
    fn the_cached_structure_report_follows_a_new_wall_height() {
        let (mut project, id) = project_with_wall();
        project
            .add_fixture(FixtureKind::Column, Point::new(5.0, 0.0), 0.14, 0.30, 0)
            .unwrap();
        let before = project.cached_structure_report().columns[0].height_m;
        project.set_wall_height(id, Some(4.3), None).unwrap();
        let after = project.cached_structure_report().columns[0].height_m;
        assert!((before - 2.8).abs() < 1e-9, "{before}");
        assert!((after - 4.3).abs() < 1e-9, "{after}");
    }
}
