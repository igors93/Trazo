//! The setting-out plan (#55, *locação*): where the building goes on the
//! lot, as cumulative distances from one corner, for marking it out with a
//! batter board (*gabarito*) and string lines. Every wall corner and every
//! column centre gets its distance along X (to the right) and Y (up) from
//! the origin.

use crate::domain::geometry::Point;
use crate::domain::model::Project;

/// What a setting-out point marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetPointKind {
    /// A wall end or corner, on the wall's axis.
    Corner,
    /// A column centre.
    Column,
}

/// One point to mark on the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct SetPoint {
    pub label: String,
    pub kind: SetPointKind,
    /// Where it is in the model (Y down).
    pub position: Point,
    /// Distance from the origin to the right, metres.
    pub x_m: f64,
    /// Distance from the origin upwards, metres.
    pub y_m: f64,
}

/// The setting-out of the building.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingOut {
    /// The origin in the model: the lot's lower-left corner as people see
    /// the plan (Y up), or the building's when there is no lot.
    pub origin: Point,
    /// Distinct cumulative distances along X and Y, metres, sorted.
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    pub points: Vec<SetPoint>,
}

/// Distances closer than this are one string line.
const MERGE_M: f64 = 0.05;

/// "A", "B", … "Z", "AA", "AB"…
fn corner_label(mut index: usize) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, char::from(b'A' + (index % 26) as u8));
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    label
}

fn distinct(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for value in values {
        if out.last().is_none_or(|last| value - last > MERGE_M) {
            out.push(value);
        }
    }
    out
}

/// The setting-out of the project's building, `None` without walls.
pub fn setting_out(project: &Project) -> Option<SettingOut> {
    if project.walls.is_empty() {
        return None;
    }
    let mut corners: Vec<Point> = Vec::new();
    for wall in &project.walls {
        for end in [wall.start, wall.end] {
            if !corners.iter().any(|c| c.distance(end) < 0.01) {
                corners.push(end);
            }
        }
    }
    let lot = project.terrain.as_ref().map(|t| t.outline());
    let reference: Vec<Point> = lot.clone().unwrap_or_else(|| corners.clone());
    let left = reference.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let bottom = reference
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let origin = Point::new(left, bottom);
    // Reading order on the plan as people see it: top to bottom, left to right.
    corners.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut points: Vec<SetPoint> = corners
        .into_iter()
        .enumerate()
        .map(|(index, position)| SetPoint {
            label: corner_label(index),
            kind: SetPointKind::Corner,
            position,
            x_m: position.x - origin.x,
            y_m: origin.y - position.y,
        })
        .collect();
    let labels: std::collections::HashMap<u64, String> = project
        .structure_report()
        .columns
        .into_iter()
        .map(|c| (c.id, c.label))
        .collect();
    for column in project.structural_columns() {
        let position = column.center;
        points.push(SetPoint {
            label: labels
                .get(&column.id)
                .cloned()
                .unwrap_or_else(|| format!("Pilar {}", column.id)),
            kind: SetPointKind::Column,
            position,
            x_m: position.x - origin.x,
            y_m: origin.y - position.y,
        });
    }
    let xs = distinct(points.iter().map(|p| p.x_m).collect());
    let ys = distinct(points.iter().map(|p| p.y_m).collect());
    Some(SettingOut {
        origin,
        xs,
        ys,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_run_from_the_lot_corner_and_corners_get_letters() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(2.0, 5.0), Point::new(6.0, 9.0), "Sala")
            .unwrap();
        let out = setting_out(&project).unwrap();
        assert!((out.origin.x - 0.0).abs() < 1e-9);
        assert!((out.origin.y - 20.0).abs() < 1e-9);
        assert_eq!(
            out.points
                .iter()
                .filter(|p| p.kind == SetPointKind::Corner)
                .count(),
            4
        );
        assert!(out.xs.iter().any(|x| (x - 2.0).abs() < 1e-6));
        assert!(out.ys.iter().any(|y| (y - 11.0).abs() < 1e-6));
        assert_eq!(corner_label(0), "A");
        assert_eq!(corner_label(26), "AA");
    }
}
