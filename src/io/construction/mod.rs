//! Construction drawings on A3 sheets (roadmap M2): the first-course plan
//! and wall elevations (#54), the setting-out plan (#55), the roof plan
//! (#71) and the one-click construction set (#57). Every sheet shares the
//! frame, the ABNT title block and an exact print scale (1:50 when it fits),
//! with one SVG unit per millimetre of paper.

pub mod paging;
pub mod setting_out;

use std::fmt::Write as _;

use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::sheet::{DrawingScale, Orientation, PaperSize, SheetLayout};

pub(crate) const INK: &str = "#1d2836";
pub(crate) const MUTED: &str = "#5b6b7c";
pub(crate) const FONT: &str = "Arial, sans-serif";

pub(crate) fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// An A3 landscape sheet being drawn.
pub(crate) struct Sheet {
    pub svg: String,
    /// The drawing area above the title block: x, y, width, height (mm).
    pub area: (f64, f64, f64, f64),
    frame: (f64, f64, f64, f64),
}

impl Sheet {
    pub(crate) fn new() -> Self {
        let layout = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        let (w, h) = layout.sheet_mm();
        let (fx, fy, fw, fh) = layout.printable_rect_mm();
        let (_, area_h) = layout.drawing_area_mm();
        let svg = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}mm\" height=\"{h:.2}mm\" viewBox=\"0 0 {w:.2} {h:.2}\">\n\
             <rect width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#ffffff\"/>\n\
             <rect x=\"0\" y=\"0\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"none\" stroke=\"#b8c4d0\" stroke-width=\"0.25\"/>\n\
             <rect x=\"{fx:.2}\" y=\"{fy:.2}\" width=\"{fw:.2}\" height=\"{fh:.2}\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"0.50\"/>\n"
        );
        Self {
            svg,
            area: (fx, fy, fw, area_h),
            frame: (fx, fy, fw, fh),
        }
    }

    pub(crate) fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, color: &str, value: &str) {
        let weight = if bold { " font-weight=\"700\"" } else { "" };
        let _ = writeln!(
            self.svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT}\" font-size=\"{size:.2}\"{weight} fill=\"{color}\">{}</text>",
            esc(value)
        );
    }

    pub(crate) fn rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: &str,
        stroke: &str,
        width: f64,
    ) {
        let _ = writeln!(
            self.svg,
            "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{width:.2}\"/>"
        );
    }

    pub(crate) fn line(
        &mut self,
        a: (f64, f64),
        b: (f64, f64),
        color: &str,
        width: f64,
        dash: Option<&str>,
    ) {
        let dash = dash.map_or(String::new(), |d| format!(" stroke-dasharray=\"{d}\""));
        let _ = writeln!(
            self.svg,
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{color}\" stroke-width=\"{width:.2}\"{dash}/>",
            a.0, a.1, b.0, b.1
        );
    }

    pub(crate) fn polygon(&mut self, points: &[(f64, f64)], fill: &str, stroke: &str, width: f64) {
        let list: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{x:.2},{y:.2}"))
            .collect();
        let _ = writeln!(
            self.svg,
            "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{width:.2}\"/>",
            list.join(" ")
        );
    }
}

/// A sheet drawn and waiting for its title block and page number.
pub(crate) struct Drawn {
    pub sheet: Sheet,
    pub title: String,
    pub scale: String,
}

/// Closes the sheets with the title block, numbered `01/NN` in order.
pub(crate) fn finish_all(project: &Project, drawn: Vec<Drawn>) -> Vec<String> {
    let total = drawn.len();
    drawn
        .into_iter()
        .enumerate()
        .map(|(index, d)| finish(project, d, &format!("{:02}/{total:02}", index + 1)))
        .collect()
}

pub(crate) fn finish(project: &Project, drawn: Drawn, number: &str) -> String {
    let Drawn {
        mut sheet,
        title,
        scale,
    } = drawn;
    let mut stub = Project::default();
    stub.name = project.name.clone();
    stub.sheet.title_block = project.sheet.title_block.clone();
    stub.sheet.title_block.drawing_title = title;
    stub.sheet.title_block.sheet_number = number.to_owned();
    let (fx, fy, fw, fh) = sheet.frame;
    crate::io::export::sheet::write_title_block(
        &mut sheet.svg,
        &stub,
        fx,
        fy,
        fw,
        fh,
        &scale,
        "A3",
    );
    sheet.svg.push_str("</svg>\n");
    sheet.svg
}

/// The largest standard scale (1:25 down to 1:200) at which `w × h` metres
/// fits `area_w × area_h` millimetres.
pub(crate) fn fit_scale(w_m: f64, h_m: f64, area_w: f64, area_h: f64) -> DrawingScale {
    let preferred = DrawingScale::S1_50;
    if preferred.fits(w_m, h_m, area_w, area_h) {
        preferred
    } else {
        DrawingScale::best_fit(w_m, h_m, area_w, area_h)
    }
}

/// Bounds of a set of points, with a margin, metres.
pub(crate) fn bounds(
    points: impl IntoIterator<Item = Point>,
    margin: f64,
) -> Option<(Point, Point)> {
    let mut iter = points.into_iter();
    let first = iter.next()?;
    let (mut min, mut max) = (first, first);
    for p in iter {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    Some((
        Point::new(min.x - margin, min.y - margin),
        Point::new(max.x + margin, max.y + margin),
    ))
}

/// Joins finished sheets into one PDF file; returns the page count.
pub(crate) fn write_pdf(pages: &[String], path: &std::path::Path) -> Result<usize, String> {
    let pdf = crate::io::report::pages_to_pdf(pages)?;
    std::fs::write(path, pdf).map_err(|e| format!("Não foi possível gravar: {e}"))?;
    Ok(pages.len())
}
