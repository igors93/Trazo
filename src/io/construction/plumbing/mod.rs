//! The plumbing sheets (roadmap section E), A3 landscape in one PDF: the
//! points plan with its legend and fixture schedule (#72).

pub mod points;

use std::fmt::Write as _;
use std::path::Path;

use super::{Drawn, MUTED, Sheet, bounds, finish_all, fit_scale};
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::plumbing::points::{PointKind, has_points};
use crate::domain::sheet::DrawingScale;

/// Grey of the walls behind the plumbing.
const WALL: &str = "#d3dbe4";

/// The building drawn to scale inside a part of the sheet.
pub(crate) struct Plan {
    min: Point,
    s: f64,
    ox: f64,
    oy: f64,
    pub scale: DrawingScale,
}

impl Plan {
    /// Fits `extent` (metres) into `area` (x, y, width, height, mm).
    pub(crate) fn new(extent: impl IntoIterator<Item = Point>, area: (f64, f64, f64, f64)) -> Self {
        let (ax, ay, aw, ah) = area;
        let (min, max) =
            bounds(extent, 0.8).unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
        let (w_m, h_m) = (max.x - min.x, max.y - min.y);
        let scale = fit_scale(w_m, h_m, aw - 8.0, ah - 8.0);
        let s = scale.mm_per_meter();
        Self {
            min,
            s,
            ox: ax + 4.0 + (aw - 8.0 - w_m * s) / 2.0,
            oy: ay + 4.0 + (ah - 8.0 - h_m * s) / 2.0,
            scale,
        }
    }

    pub(crate) fn pt(&self, p: Point) -> (f64, f64) {
        (
            self.ox + (p.x - self.min.x) * self.s,
            self.oy + (p.y - self.min.y) * self.s,
        )
    }

    /// Millimetres on the sheet for `metres` in the model.
    pub(crate) fn mm(&self, metres: f64) -> f64 {
        metres * self.s
    }
}

/// The walls in light grey, the room names and the outline of every fixture
/// that takes water, drains or gas.
pub(crate) fn draw_building(sheet: &mut Sheet, project: &Project, plan: &Plan) {
    for wall in &project.walls {
        let _ = writeln!(
            sheet.svg,
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{WALL}\" stroke-width=\"{:.2}\" stroke-linecap=\"square\"/>",
            plan.pt(wall.start).0,
            plan.pt(wall.start).1,
            plan.pt(wall.end).0,
            plan.pt(wall.end).1,
            plan.mm(wall.thickness()).max(0.4)
        );
    }
    for room in &project.rooms {
        if let Some(centre) = project.room_centroid(room) {
            let (x, y) = plan.pt(centre);
            let name = room.display_name();
            let width = name.chars().count() as f64 * 1.1;
            sheet.text(x - width / 2.0, y, 2.2, false, MUTED, name);
        }
    }
    for fixture in project.fixtures.iter().filter(|f| has_points(f.kind)) {
        let (w, d) = (fixture.width / 2.0, fixture.depth / 2.0);
        let corners: Vec<(f64, f64)> = [(-w, -d), (w, -d), (w, d), (-w, d)]
            .into_iter()
            .map(|(x, y)| plan.pt(fixture.to_world(Point::new(x, y))))
            .collect();
        sheet.polygon(&corners, "none", MUTED, 0.18);
    }
}

/// The symbol of a point: an open circle for water, a filled one for the
/// drain, a square for gas.
pub(crate) fn point_symbol(sheet: &mut Sheet, kind: PointKind, (x, y): (f64, f64), r: f64) {
    let color = kind.hex();
    match kind {
        PointKind::Cold | PointKind::Hot => {
            let _ = writeln!(
                sheet.svg,
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"#ffffff\" stroke=\"{color}\" stroke-width=\"0.40\"/>"
            );
        }
        PointKind::Drain => {
            let _ = writeln!(
                sheet.svg,
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"{color}\" stroke=\"{color}\" stroke-width=\"0.30\"/>"
            );
        }
        PointKind::Gas => sheet.rect((x - r, y - r, 2.0 * r, 2.0 * r), "#ffffff", &color, 0.40),
    }
}

/// A small label with a white halo, readable over walls and pipes.
pub(crate) fn label(sheet: &mut Sheet, (x, y): (f64, f64), size: f64, color: &str, value: &str) {
    let _ = writeln!(
        sheet.svg,
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{}\" font-size=\"{size:.2}\" fill=\"{color}\" stroke=\"#ffffff\" stroke-width=\"0.55\" paint-order=\"stroke\">{}</text>",
        super::FONT,
        super::esc(value)
    );
}

/// Splits `text` into lines of about `chars` characters.
pub(crate) fn wrap(text: &str, chars: usize) -> Vec<String> {
    crate::io::report::wrap(text, chars)
}

/// The plumbing sheets, in order, waiting for their title blocks.
pub(crate) fn plumbing_drawings(project: &Project) -> Vec<Drawn> {
    points::points_drawing(project).into_iter().collect()
}

/// The plumbing sheets, numbered `01/NN`.
pub fn plumbing_pages(project: &Project) -> Vec<String> {
    finish_all(project, plumbing_drawings(project))
}

/// Writes the plumbing sheets as one PDF; returns the page count.
pub fn export_plumbing_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    let pages = plumbing_pages(project);
    if pages.is_empty() {
        return Err(NO_FIXTURES.to_owned());
    }
    super::write_pdf(&pages, path)
}

/// What to say when the plan has nothing that takes water.
pub const NO_FIXTURES: &str = "Coloque os aparelhos (vaso, chuveiro, lavatório, pia, máquina de lavar) antes de gerar o projeto hidrossanitário.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    #[test]
    fn the_points_sheet_has_its_legend_and_schedule() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(2.5, 2.5), "Banheiro")
            .unwrap();
        for (kind, at) in [
            (FixtureKind::Toilet, Point::new(0.5, 0.5)),
            (FixtureKind::Shower, Point::new(1.8, 1.8)),
        ] {
            let (w, d) = kind.default_size();
            project.add_fixture(kind, at, w, d, 0).unwrap();
        }
        let pages = plumbing_pages(&project);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("Quadro de aparelhos"));
        assert!(pages[0].contains("Vaso sanitário"));
        assert!(pages[0].contains("01/01"));
        assert!(plumbing_pages(&Project::default()).is_empty());
    }
}
