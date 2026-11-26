//! The plumbing discipline on exported plans and sheets (#96): every point
//! in its colour, with its tag and height.

use std::fmt::Write as _;

use crate::domain::geometry::Point;
use crate::domain::layers::LayerCategory;
use crate::domain::model::Project;
use crate::domain::plumbing::points::{PointKind, plumbing_points};

/// Sizes of the plumbing marks in the output's own unit (mm on a sheet,
/// pixels on an image).
pub(crate) struct Marks {
    /// Radius of a point.
    pub radius: f64,
    /// Stroke of an open point.
    pub stroke: f64,
    /// Size of the tags; none leaves them out.
    pub font: Option<f64>,
}

/// Writes the plan's plumbing points when the plumbing layer is shown.
pub(crate) fn write_plumbing(
    svg: &mut String,
    project: &Project,
    pt: &dyn Fn(Point) -> (f64, f64),
    marks: &Marks,
) {
    if !project.category_visible(LayerCategory::Plumbing) {
        return;
    }
    let points = plumbing_points(project);
    if points.is_empty() {
        return;
    }
    let r = marks.radius;
    let _ = writeln!(svg, "<g id=\"hidraulica\">");
    let mut fixture = None;
    let mut row = 0.0;
    for point in &points.points {
        let (x, y) = pt(point.at);
        let color = point.kind.hex();
        match point.kind {
            PointKind::Cold | PointKind::Hot => {
                let _ = writeln!(
                    svg,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"#ffffff\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
                    marks.stroke
                );
            }
            PointKind::Drain => {
                let _ = writeln!(
                    svg,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"{color}\"/>"
                );
            }
            PointKind::Gas => {
                let _ = writeln!(
                    svg,
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
                    x - r,
                    y - r,
                    2.0 * r,
                    2.0 * r,
                    marks.stroke
                );
            }
        }
        let Some(size) = marks.font else {
            continue;
        };
        if fixture != Some(point.fixture_id) {
            fixture = Some(point.fixture_id);
            row = 0.0;
        }
        let tag = format!(
            "{} {}",
            point.kind.tag(),
            format!("{:.2}", point.height_m).replace('.', ",")
        );
        let _ = writeln!(
            svg,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"{size:.2}\" fill=\"{color}\" stroke=\"#ffffff\" stroke-width=\"{:.2}\" paint-order=\"stroke\">{tag}</text>",
            x + r * 1.6,
            y + size * 0.35 + row,
            size * 0.3
        );
        row += size * 1.15;
    }
    svg.push_str("</g>\n");
}
