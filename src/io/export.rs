use std::{fmt::Write as _, fs, path::Path};

use crate::domain::{
    fixtures::{SymbolShape, Weight},
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
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"#f8fbfe\" stroke=\"#5e8bbf\" stroke-width=\"2.5\" stroke-dasharray=\"28 8 5 8\"/>"
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
            "<text x=\"{x:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"15\" fill=\"#3d6a9c\">{}</text>",
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
            let _ = writeln!(svg, "<polygon points=\"{}\" fill=\"#eef5fd\"/>", points_str);
        }
    }
    let base_wall_width = wall_width;
    for wall in &project.walls {
        // Each wall at its own thickness (block plus plaster).
        let wall_width = (pixels_per_meter * wall.thickness()).max(1.0);
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
                "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{left:.1}\" y2=\"0\" stroke=\"#1d2836\" stroke-width=\"{wall_width:.1}\" stroke-linecap=\"square\"/>"
            );
            match opening.kind {
                OpeningKind::Door => {
                    let leaf = half * 2.0;
                    let side = if opening.flip_side { -1.0 } else { 1.0 };
                    let (hinge, along) = if opening.flip_hinge {
                        (right, -1.0)
                    } else {
                        (left, 1.0)
                    };
                    let arc = (0..=20)
                        .map(|step| {
                            let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 20.0;
                            format!(
                                "{:.1},{:.1}",
                                hinge + along * theta.sin() * leaf,
                                side * theta.cos() * leaf
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{hinge:.1}\" y1=\"0\" x2=\"{hinge:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"3\"/><polyline points=\"{arc}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.5\"/>",
                        side * leaf
                    );
                }
                OpeningKind::SaloonDoor => {
                    // Two half leaves swinging both ways (dashed on the far side).
                    let quarter = |hinge: f32, toward: f32, side: f32| {
                        (0..=12)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 12.0;
                                format!(
                                    "{:.1},{:.1}",
                                    hinge + toward * theta.sin() * half,
                                    side * theta.cos() * half
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    let side = if opening.flip_side { -1.0 } else { 1.0 };
                    for (hinge, toward) in [(left, 1.0), (right, -1.0)] {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{hinge:.1}\" y1=\"0\" x2=\"{hinge:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"3\"/><polyline points=\"{}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.5\"/><polyline points=\"{}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.5\" stroke-dasharray=\"5 4\"/>",
                            side * half,
                            quarter(hinge, toward, side),
                            quarter(hinge, toward, -side)
                        );
                    }
                }
                OpeningKind::Gap => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.1}\" y1=\"0\" x2=\"{right:.1}\" y2=\"0\" stroke=\"#8ea2b8\" stroke-width=\"1.5\" stroke-dasharray=\"8 6\"/>"
                    );
                }
                OpeningKind::Window => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.1}\" y1=\"-4\" x2=\"{right:.1}\" y2=\"-4\" stroke=\"#2878d2\" stroke-width=\"3\"/><line x1=\"{left:.1}\" y1=\"4\" x2=\"{right:.1}\" y2=\"4\" stroke=\"#2878d2\" stroke-width=\"3\"/>"
                    );
                }
            }
            cursor = right;
        }
        let _ = writeln!(
            svg,
            "<line x1=\"{cursor:.1}\" y1=\"0\" x2=\"{length:.1}\" y2=\"0\" stroke=\"#1d2836\" stroke-width=\"{wall_width:.1}\" stroke-linecap=\"square\"/></g>"
        );
    }
    for fixture in &project.fixtures {
        let corner = |x: f32, y: f32| {
            let (px, py) = point(fixture.to_world(Point::new(x, y)));
            format!("{px:.1},{py:.1}")
        };
        let (hw, hd) = (fixture.width / 2.0, fixture.depth / 2.0);
        let _ = writeln!(
            svg,
            "<polygon points=\"{} {} {} {}\" fill=\"#ffffff\"/>",
            corner(-hw, -hd),
            corner(hw, -hd),
            corner(hw, hd),
            corner(-hw, hd)
        );
        for shape in fixture.symbol() {
            let weight = match &shape {
                SymbolShape::Polyline { weight, .. } | SymbolShape::Circle { weight, .. } => *weight,
            };
            let (color, width) = match weight {
                Weight::Outline => ("#3b4e63", 2.0),
                Weight::Detail => ("#627489", 1.2),
            };
            match shape {
                SymbolShape::Polyline {
                    points,
                    closed,
                    dashed,
                    ..
                } => {
                    let points = points
                        .iter()
                        .map(|local| corner(local.x, local.y))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let element = if closed { "polygon" } else { "polyline" };
                    let dash = if dashed { " stroke-dasharray=\"6 4\"" } else { "" };
                    let _ = writeln!(
                        svg,
                        "<{element} points=\"{points}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width}\"{dash}/>"
                    );
                }
                SymbolShape::Circle { center, radius, .. } => {
                    let (cx, cy) = point(fixture.to_world(center));
                    let _ = writeln!(
                        svg,
                        "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width}\"/>",
                        radius * pixels_per_meter
                    );
                }
            }
        }
    }
    let wall_width = base_wall_width;
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
        "<text x=\"{MARGIN:.1}\" y=\"55\" font-family=\"Arial, sans-serif\" font-size=\"23\" font-weight=\"700\" fill=\"#1c2b3c\">{}</text>",
        escaped(&project.name)
    );
    svg.push_str("</svg>");
    SvgDocument {
        markup: svg,
        width,
        height,
    }
}

/// Rasterizes the plan, scaled down so that its longer side is at most
/// `max_side` pixels (never up).
fn render(project: &Project, max_side: Option<u32>) -> Result<resvg::tiny_skia::Pixmap, String> {
    let document = document(project);
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(&document.markup, &options)
        .map_err(|error| error.to_string())?;
    let scale = max_side
        .map(|side| (side as f32 / document.width.max(document.height) as f32).min(1.0))
        .unwrap_or(1.0);
    let width = ((document.width as f32 * scale).round() as u32).max(1);
    let height = ((document.height as f32 * scale).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or("A planta é grande demais para exportar como imagem")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

pub fn png(project: &Project, path: &Path) -> Result<(), String> {
    render(project, None)?
        .save_png(path)
        .map_err(|error| error.to_string())
}

/// The plan as PNG bytes no larger than `max_side` pixels, for previews such
/// as the picture an AI assistant asks for.
pub fn png_preview(project: &Project, max_side: u32) -> Result<Vec<u8>, String> {
    render(project, Some(max_side))?
        .encode_png()
        .map_err(|error| error.to_string())
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
    fn preview_png_fits_the_requested_size() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(12.0, 25.0));
        let bytes = png_preview(&project, 512).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
        // The IHDR chunk holds width and height right after the signature.
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!(width.max(height), 512);
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
