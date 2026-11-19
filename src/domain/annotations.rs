//! Dimension chains around the building, as construction drawings show
//! them: every wall face along the top and the left side, so each wall
//! thickness and each clear span reads in centimetres, plus the totals.

use super::{
    geometry::Point,
    model::{Opening, OpeningKind, Project},
};

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

/// One type of door or window: every opening with the same kind and size
/// shares a code, as the schedule of a construction drawing lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameType {
    pub code: String,
    pub kind: OpeningKind,
    pub width: f32,
    pub height: Option<f32>,
    pub sill: f32,
    pub ids: Vec<u64>,
}

impl FrameType {
    /// "P1 80/210", "J1 120/100", "V1 1205".
    pub fn tag(&self) -> String {
        match self.height {
            Some(height) => format!(
                "{} {}/{}",
                self.code,
                centimetres(self.width),
                centimetres(height)
            ),
            None => format!("{} {}", self.code, centimetres(self.width)),
        }
    }
}

fn prefix(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "P",
        OpeningKind::SaloonDoor => "PV",
        OpeningKind::Window => "J",
        OpeningKind::Gap => "V",
    }
}

fn kind_order(kind: OpeningKind) -> u8 {
    match kind {
        OpeningKind::Door => 0,
        OpeningKind::SaloonDoor => 1,
        OpeningKind::Window => 2,
        OpeningKind::Gap => 3,
    }
}

/// The door and window types of the project: doors, then windows and open
/// spans, each numbered from the smallest to the largest.
pub fn frame_types(project: &Project) -> Vec<FrameType> {
    let mut types: Vec<FrameType> = Vec::new();
    let same = |a: f32, b: f32| (a - b).abs() < 0.005;
    for opening in &project.openings {
        let fits = |frame: &FrameType| {
            frame.kind == opening.kind
                && same(frame.width, opening.width())
                && match (frame.height, opening.height()) {
                    (Some(a), Some(b)) => same(a, b),
                    (None, None) => true,
                    _ => false,
                }
                && same(frame.sill, opening.sill())
        };
        if let Some(frame) = types.iter_mut().find(|frame| fits(frame)) {
            frame.ids.push(opening.id);
            continue;
        }
        types.push(FrameType {
            code: String::new(),
            kind: opening.kind,
            width: opening.width(),
            height: opening.height(),
            sill: opening.sill(),
            ids: vec![opening.id],
        });
    }
    types.sort_by(|a, b| {
        kind_order(a.kind)
            .cmp(&kind_order(b.kind))
            .then(a.width.total_cmp(&b.width))
            .then(a.height.unwrap_or(0.0).total_cmp(&b.height.unwrap_or(0.0)))
            .then(a.sill.total_cmp(&b.sill))
    });
    let mut number = 0;
    for index in 0..types.len() {
        number = if index > 0 && types[index - 1].kind == types[index].kind {
            number + 1
        } else {
            1
        };
        types[index].code = format!("{}{number}", prefix(types[index].kind));
    }
    types
}

/// The type of `opening` among `types`.
pub fn frame_of<'a>(types: &'a [FrameType], opening: &Opening) -> Option<&'a FrameType> {
    types.iter().find(|frame| frame.ids.contains(&opening.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_openings_share_a_type_code() {
        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(12.0, 0.0))
            .unwrap();
        project.add_opening(wall, OpeningKind::Door, 0.1).unwrap();
        project
            .add_opening(wall, OpeningKind::Window, 0.35)
            .unwrap();
        project.add_opening(wall, OpeningKind::Door, 0.6).unwrap();
        project
            .add_opening_sized(wall, OpeningKind::Door, 0.85, Some(0.7))
            .unwrap();
        let types = frame_types(&project);
        let tags: Vec<String> = types.iter().map(FrameType::tag).collect();
        assert_eq!(tags, ["P1 70/210", "P2 90/210", "J1 120/100"]);
        assert_eq!(types[1].ids.len(), 2);
        assert_eq!(types[2].sill, 1.10);
    }

    #[test]
    fn a_room_gives_wall_thicknesses_and_its_clear_span() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
            .unwrap();
        let dims = building_dimensions(&project).unwrap();
        let spans: Vec<String> = dims
            .xs
            .windows(2)
            .map(|w| centimetres(w[1] - w[0]))
            .collect();
        assert_eq!(spans, ["12", "388", "12"]);
        assert_eq!(centimetres(dims.max.x - dims.min.x), "412");
        let spans: Vec<String> = dims
            .ys
            .windows(2)
            .map(|w| centimetres(w[1] - w[0]))
            .collect();
        assert_eq!(spans, ["12", "288", "12"]);
        assert_eq!(centimetres(0.125), "12,5");
        assert!(building_dimensions(&Project::default()).is_none());
    }
}
