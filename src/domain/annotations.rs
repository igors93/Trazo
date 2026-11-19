//! Dimension chains around the building, as construction drawings show
//! them: every wall face along the top and the left side, so each wall
//! thickness and each clear span reads in centimetres, plus the totals.

use super::{geometry::Point, model::Project};

/// Wall face positions along X and Y and the building's outer bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingDimensions {
    pub min: Point,
    pub max: Point,
    /// Sorted X positions of vertical wall faces (and the outer bounds).
    pub xs: Vec<f32>,
    /// Sorted Y positions of horizontal wall faces (and the outer bounds).
    pub ys: Vec<f32>,
}

fn sorted_unique(mut values: Vec<f32>) -> Vec<f32> {
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < 0.005);
    values
}

/// The dimension chains of the walls drawn so far, or `None` without walls.
pub fn building_dimensions(project: &Project) -> Option<BuildingDimensions> {
    if project.walls.is_empty() {
        return None;
    }
    let mut min = Point::new(f32::INFINITY, f32::INFINITY);
    let mut max = Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for wall in &project.walls {
        let half = wall.thickness() / 2.0;
        let (x0, x1) = (wall.start.x.min(wall.end.x), wall.start.x.max(wall.end.x));
        let (y0, y1) = (wall.start.y.min(wall.end.y), wall.start.y.max(wall.end.y));
        // Walls are drawn with square ends half a thickness long.
        min = Point::new(min.x.min(x0 - half), min.y.min(y0 - half));
        max = Point::new(max.x.max(x1 + half), max.y.max(y1 + half));
        if x1 - x0 < 1e-3 {
            xs.extend([x0 - half, x0 + half]);
        } else if y1 - y0 < 1e-3 {
            ys.extend([y0 - half, y0 + half]);
        }
    }
    xs.extend([min.x, max.x]);
    ys.extend([min.y, max.y]);
    Some(BuildingDimensions {
        min,
        max,
        xs: sorted_unique(xs),
        ys: sorted_unique(ys),
    })
}

/// A length in centimetres, as dimension chains write it: "240", "12,5".
pub fn centimetres(metres: f32) -> String {
    let value = metres * 100.0;
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}", value.round())
    } else {
        format!("{value:.1}").replace('.', ",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_gives_wall_thicknesses_and_its_clear_span() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
            .unwrap();
        let dims = building_dimensions(&project).unwrap();
        let spans: Vec<String> = dims.xs.windows(2).map(|w| centimetres(w[1] - w[0])).collect();
        assert_eq!(spans, ["12", "388", "12"]);
        assert_eq!(centimetres(dims.max.x - dims.min.x), "412");
        let spans: Vec<String> = dims.ys.windows(2).map(|w| centimetres(w[1] - w[0])).collect();
        assert_eq!(spans, ["12", "288", "12"]);
        assert_eq!(centimetres(0.125), "12,5");
        assert!(building_dimensions(&Project::default()).is_none());
    }
}
