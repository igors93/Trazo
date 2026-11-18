use std::{fmt::Write as _, fs, path::Path};

use crate::domain::{
    geometry::{Point, format_meters},
    model::{OpeningKind, Project},
};

const PIXELS_PER_METER: f32 = 100.0;
const MARGIN: f32 = 130.0;

struct SvgDocument {
    markup: String,
    width: u32,
    height: u32,
}

fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn document(project: &Project) -> SvgDocument {
    let (min, max) = project
        .bounds()
        .unwrap_or((Point::new(0.0, 0.0), Point::new(9.0, 6.0)));
    let width = (((max.x - min.x).max(6.0) * PIXELS_PER_METER) + MARGIN * 2.0).ceil() as u32;
    let height = (((max.y - min.y).max(4.0) * PIXELS_PER_METER) + MARGIN * 2.0).ceil() as u32;
    let point = |p: Point| -> (f32, f32) {
        (
            MARGIN + (p.x - min.x) * PIXELS_PER_METER,
            MARGIN + (p.y - min.y) * PIXELS_PER_METER,
        )
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    for room in &project.rooms {
        if let Some((a, b)) = project.room_bounds(room) {
            let (x, y) = point(a);
            let w = (b.x - a.x) * PIXELS_PER_METER;
            let h = (b.y - a.y) * PIXELS_PER_METER;
            let _ = writeln!(
                svg,
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"#eff8f7\"/>"
            );
            let center = point(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
            let _ = writeln!(
                svg,
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"20\" font-weight=\"700\" fill=\"#1e3a4a\">{}</text>",
                center.0,
                center.1 - 4.0,
                escaped(room.kind.label())
            );
            let dimensions = format!(
                "{} × {}",
                format_meters(b.x - a.x),
                format_meters(b.y - a.y)
            );
            let _ = writeln!(
                svg,
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"14\" fill=\"#64748b\">{}</text>",
                center.0,
                center.1 + 20.0,
                dimensions
            );
        }
    }
    for wall in &project.walls {
        let (x, y) = point(wall.start);
        let length = wall.length() * PIXELS_PER_METER;
        let angle = (wall.end.y - wall.start.y)
            .atan2(wall.end.x - wall.start.x)
            .to_degrees();
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.1} {y:.1}) rotate({angle:.3})\">"
        );
        let mut openings: Vec<_> = project
            .openings
            .iter()
            .filter(|opening| opening.wall_id == wall.id)
            .collect();
        openings.sort_by(|a, b| a.t.total_cmp(&b.t));
        let mut cursor = 0.0;
        for opening in openings {
            let half = opening.kind.width() * PIXELS_PER_METER / 2.0;
            let center = opening.t * length;
            let left = center - half;
            let right = center + half;
            let _ = writeln!(
                svg,
                "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{left:.1}\" y2=\"0\" stroke=\"#24354b\" stroke-width=\"11\" stroke-linecap=\"square\"/>"
            );
            match opening.kind {
                OpeningKind::Door => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.1}\" y1=\"0\" x2=\"{left:.1}\" y2=\"{:.1}\" stroke=\"#35b4a3\" stroke-width=\"4\"/><path d=\"M {left:.1} {:.1} A {:.1} {:.1} 0 0 0 {right:.1} 0\" fill=\"none\" stroke=\"#8bcfc5\" stroke-width=\"2\" stroke-dasharray=\"5 5\"/>",
                        half * 2.0,
                        half * 2.0,
                        half * 2.0,
                        half * 2.0
                    );
                }
                OpeningKind::Window => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.1}\" y1=\"-4\" x2=\"{right:.1}\" y2=\"-4\" stroke=\"#27a8c2\" stroke-width=\"3\"/><line x1=\"{left:.1}\" y1=\"4\" x2=\"{right:.1}\" y2=\"4\" stroke=\"#27a8c2\" stroke-width=\"3\"/>"
                    );
                }
            }
            cursor = right;
        }
        let _ = writeln!(
            svg,
            "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{length:.1}\" y2=\"0\" stroke=\"#24354b\" stroke-width=\"11\" stroke-linecap=\"square\"/></g>"
        );
        let center = point(wall.point_at(0.5));
        let dx = wall.end.x - wall.start.x;
        let dy = wall.end.y - wall.start.y;
        let normal_length = (dx * dx + dy * dy).sqrt().max(0.001);
        let (nx, ny) = (-dy / normal_length, dx / normal_length);
        let _ = writeln!(
            svg,
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"13\" fill=\"#52657a\">{}</text>",
            center.0 + nx * 25.0,
            center.1 + ny * 25.0,
            format_meters(wall.length())
        );
    }
    let _ = writeln!(
        svg,
        "<text x=\"{MARGIN:.1}\" y=\"55\" font-family=\"Arial, sans-serif\" font-size=\"23\" font-weight=\"700\" fill=\"#172438\">{}</text>",
        escaped(&project.name)
    );
    svg.push_str("</svg>");
    SvgDocument {
        markup: svg,
        width,
        height,
    }
}

pub fn png(project: &Project, path: &Path) -> Result<(), String> {
    let document = document(project);
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(&document.markup, &options)
        .map_err(|error| error.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(document.width, document.height)
        .ok_or("A planta é grande demais para exportar como imagem")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(path).map_err(|error| error.to_string())
}

pub fn pdf(project: &Project, path: &Path) -> Result<(), String> {
    let document = document(project);
    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = svg2pdf::usvg::Tree::from_str(&document.markup, &options)
        .map_err(|error| error.to_string())?;
    let pdf = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    fs::write(path, pdf).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{OpeningKind, RoomKind};

    #[test]
    fn exports_png_and_pdf() {
        let mut project = Project::default();
        project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(4.0, 3.0),
                RoomKind::Bedroom,
            )
            .unwrap();
        let wall_id = project.walls[0].id;
        project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();
        let directory = std::env::temp_dir().join(format!("trazo-export-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let png_path = directory.join("plan.png");
        let pdf_path = directory.join("plan.pdf");
        png(&project, &png_path).unwrap();
        pdf(&project, &pdf_path).unwrap();
        assert!(fs::read(&png_path).unwrap().starts_with(b"\x89PNG"));
        assert!(fs::read(&pdf_path).unwrap().starts_with(b"%PDF"));
        let _ = fs::remove_file(png_path);
        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_dir(directory);
    }
}
