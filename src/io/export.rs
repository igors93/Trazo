use std::{fmt::Write as _, fs, path::Path};

use crate::domain::{
    geometry::{Point, WALL_THICKNESS_METERS, format_meters},
    model::{OpeningKind, Project},
};

mod labels;

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
    let span_x = (max.x - min.x).max(6.0);
    let span_y = (max.y - min.y).max(4.0);
    let pixels_per_meter = PIXELS_PER_METER.min(5000.0 / span_x).min(5000.0 / span_y);
    let wall_width = (pixels_per_meter * WALL_THICKNESS_METERS).clamp(1.0, 12.0);
    let width = (span_x * pixels_per_meter + MARGIN * 2.0).ceil() as u32;
    let height = (span_y * pixels_per_meter + MARGIN * 2.0).ceil() as u32;
    let point = |p: Point| -> (f32, f32) {
        (
            MARGIN + (p.x - min.x) * pixels_per_meter,
            MARGIN + (p.y - min.y) * pixels_per_meter,
        )
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    if let Some(terrain) = &project.terrain {
        let (x, y) = point(terrain.min);
        let w = terrain.width() * pixels_per_meter;
        let h = terrain.height() * pixels_per_meter;
        let _ = writeln!(
            svg,
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"#f0f7fc\" stroke=\"#4f7ca9\" stroke-width=\"3\"/>"
        );
        let label = format!(
            "Terreno · {} × {} · {:.2} m²",
            format_meters(terrain.width()),
            format_meters(terrain.height()),
            terrain.area()
        )
        .replace('.', ",");
        let _ = writeln!(
            svg,
            "<text x=\"{x:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"15\" fill=\"#365a7b\">{}</text>",
            y - 12.0,
            escaped(&label)
        );
    }
    for room in &project.rooms {
        if let Some(poly) = project.room_polygon(room) {
            let points_str: String = poly
                .iter()
                .map(|p| {
                    let (x, y) = point(*p);
                    format!("{:.1},{:.1}", x, y)
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(svg, "<polygon points=\"{}\" fill=\"#eff8f7\"/>", points_str);
        } else if let Some((a, b)) = project.room_bounds(room) {
            let (x, y) = point(a);
            let w = (b.x - a.x) * pixels_per_meter;
            let h = (b.y - a.y) * pixels_per_meter;
            let _ = writeln!(
                svg,
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"#eff8f7\"/>"
            );
        }
    }
    for wall in &project.walls {
        let (x, y) = point(wall.start);
        let length = wall.length() * pixels_per_meter;
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
            let half = opening.width() * pixels_per_meter / 2.0;
            let center = opening.t * length;
            let left = center - half;
            let right = center + half;
            let _ = writeln!(
                svg,
                "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{left:.1}\" y2=\"0\" stroke=\"#24354b\" stroke-width=\"{wall_width:.1}\" stroke-linecap=\"square\"/>"
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
            "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{length:.1}\" y2=\"0\" stroke=\"#24354b\" stroke-width=\"{wall_width:.1}\" stroke-linecap=\"square\"/></g>"
        );
    }
    let mut occupied = labels::write_room_labels(&mut svg, project, &point, wall_width);
    labels::write_wall_labels(
        &mut svg,
        project,
        &point,
        pixels_per_meter,
        wall_width,
        &mut occupied,
    );
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
    use crate::domain::model::OpeningKind;

    #[test]
    fn long_room_names_are_kept_inside_their_exported_room() {
        let mut project = Project::default();
        let room_id = project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(0.5, 0.5),
                "Quarto com nome excepcionalmente longo",
            )
            .unwrap();
        let svg = document(&project).markup;
        assert!(svg.contains(&format!("clipPath id=\"room-label-{room_id}\"")));
        assert!(svg.contains('…'));
        assert!(!svg.contains("Quarto com nome excepcionalmente longo</text>"));
        assert!(!svg.contains("0,50 m × 0,50 m</text>"));
    }

    #[test]
    fn dimensions_are_never_truncated_to_a_different_value() {
        assert!(labels::fit_export_text("12,50 m × 6,00 m", 20.0, 14, 10, false).is_none());
    }

    #[test]
    fn large_terrain_export_has_bounded_dimensions() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(500.0, 500.0));
        let svg = document(&project);
        assert!(svg.width <= 5260 && svg.height <= 5260);
    }

    #[test]
    fn exports_png_and_pdf() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(5.0, 10.0));
        project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(4.0, 3.0),
                "Quarto personalizado",
            )
            .unwrap();
        assert!(document(&project).markup.contains("Quarto personalizado"));
        assert!(document(&project).markup.contains("50,00 m²"));
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
