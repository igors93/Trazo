//! Exact physical-scale (millimetre-accurate) sheet export in SVG and PDF.
//!
//! One SVG user unit equals exactly 1.00 mm on paper (`width="420.00mm"
//! height="297.00mm" viewBox="0 0 420.00 297.00"`), so a 1.00 m wall at 1:50
//! is rendered at literally 20.00 mm and checks out with a physical scale ruler.

use std::{fmt::Write as _, fs, path::Path};

use super::escaped;
use crate::domain::{
    annotations::{building_dimensions, centimetres},
    dimensions::ResolvedDimension,
    fixtures::{SymbolShape, Weight},
    geometry::Point,
    hatch::{HatchPattern, hatch_segments_in_polygon},
    layers::LayerCategory,
    model::{OpeningKind, Project},
    notes::{NoteKind, TechnicalSymbol},
};

/// Renders the project onto its configured physical sheet (`A4`–`A0` or custom)
/// at the exact configured scale (`1:25`–`1:200`), returning physical-mm SVG.
pub fn scaled_sheet_svg(project: &Project) -> String {
    let layout = project.sheet;
    let (sheet_w, sheet_h) = layout.sheet_mm();
    let (frame_x, frame_y, frame_w, frame_h) = layout.printable_rect_mm();
    let mm_per_m = layout.scale.mm_per_meter();

    let (min, max) = project
        .bounds()
        .unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let plan_w_mm = (max.x - min.x).max(0.5) * mm_per_m;
    let plan_h_mm = (max.y - min.y).max(0.5) * mm_per_m;

    // Reserve a small bottom strip for sheet metadata / scale badge.
    let draw_area_h = (frame_h - 14.0).max(20.0);
    let origin_x = frame_x + (frame_w - plan_w_mm) * 0.5;
    let origin_y = frame_y + (draw_area_h - plan_h_mm) * 0.5;

    let pt = |p: Point| -> (f64, f64) {
        (
            origin_x + (p.x - min.x) * mm_per_m,
            origin_y + (p.y - min.y) * mm_per_m,
        )
    };

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{sheet_w:.2}mm\" height=\"{sheet_h:.2}mm\" viewBox=\"0 0 {sheet_w:.2} {sheet_h:.2}\">\n\
         <rect width=\"{sheet_w:.2}\" height=\"{sheet_h:.2}\" fill=\"#ffffff\"/>\n\
         <defs><clipPath id=\"sheet-viewport\"><rect x=\"{frame_x:.2}\" y=\"{frame_y:.2}\" width=\"{frame_w:.2}\" height=\"{frame_h:.2}\"/></clipPath></defs>\n"
    );

    // Outer sheet cut guide and ABNT drawing margin frame.
    let _ = writeln!(
        svg,
        "<g id=\"prancha-moldura\">\n\
         <rect x=\"0\" y=\"0\" width=\"{sheet_w:.2}\" height=\"{sheet_h:.2}\" fill=\"none\" stroke=\"#b8c4d0\" stroke-width=\"0.25\"/>\n\
         <rect x=\"{frame_x:.2}\" y=\"{frame_y:.2}\" width=\"{frame_w:.2}\" height=\"{frame_h:.2}\" fill=\"none\" stroke=\"#1d2836\" stroke-width=\"0.50\"/>\n\
         </g>"
    );

    let _ = writeln!(
        svg,
        "<g id=\"planta-escala\" clip-path=\"url(#sheet-viewport)\">"
    );

    // Terrain.
    if project.category_visible(LayerCategory::Terrain)
        && let Some(terrain) = &project.terrain
    {
        let outline = terrain.outline();
        let points_str: String = outline
            .iter()
            .map(|p| {
                let (x, y) = pt(*p);
                format!("{x:.2},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            svg,
            "<polygon points=\"{points_str}\" fill=\"#f8fbfe\" stroke=\"#5e8bbf\" stroke-width=\"0.25\" stroke-dasharray=\"4.0 1.2 0.8 1.2\"/>"
        );
    }

    // Rooms and material hatches.
    if project.category_visible(LayerCategory::Rooms) {
        for room in &project.rooms {
            let Some(poly) = project.room_polygon(room) else {
                continue;
            };
            let pts: String = poly
                .iter()
                .map(|p| {
                    let (x, y) = pt(*p);
                    format!("{x:.2},{y:.2}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let fill = match room.hatch {
                Some(style) if style.pattern == HatchPattern::Solid => style.hex_color(),
                _ => "#f3f7fc".to_owned(),
            };
            let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"{fill}\"/>");
            if let Some(style) = room.hatch {
                let stroke = style.hex_color();
                for (a, b) in hatch_segments_in_polygon(&poly, &style) {
                    let (x1, y1) = pt(a);
                    let (x2, y2) = pt(b);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{stroke}\" stroke-width=\"0.13\" opacity=\"0.8\"/>"
                    );
                }
            }
        }
    }

    // Wall junctions + walls at exact scaled thickness.
    if project.category_visible(LayerCategory::Walls) {
        for junction in project.wall_junctions() {
            let pts: String = junction
                .outline
                .iter()
                .map(|p| {
                    let (x, y) = pt(*p);
                    format!("{x:.2},{y:.2}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"#1d2836\"/>");
        }
    }
    for wall in &project.walls {
        if !project.is_wall_visible(wall) {
            continue;
        }
        let wall_w_mm = (wall.thickness() * mm_per_m).max(0.35);
        let (x, y) = pt(wall.start);
        let len_mm = wall.length() * mm_per_m;
        let angle = (wall.end.y - wall.start.y)
            .atan2(wall.end.x - wall.start.x)
            .to_degrees();
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.2} {y:.2}) rotate({angle:.3})\">"
        );
        let mut openings: Vec<_> = project
            .openings
            .iter()
            .filter(|o| o.wall_id == wall.id)
            .collect();
        openings.sort_by(|a, b| a.t.total_cmp(&b.t));
        let mut cursor = 0.0;
        for op in openings {
            let half = op.width() * mm_per_m * 0.5;
            let center = op.t * len_mm;
            let left = center - half;
            let right = center + half;
            let _ = writeln!(
                svg,
                "<line x1=\"{cursor:.2}\" y1=\"0\" x2=\"{left:.2}\" y2=\"0\" stroke=\"#1d2836\" stroke-width=\"{wall_w_mm:.2}\" stroke-linecap=\"square\"/>"
            );
            match op.kind {
                OpeningKind::Door => {
                    let leaf = half * 2.0;
                    let side = if op.flip_side { -1.0 } else { 1.0 };
                    let (hinge, along) = if op.flip_hinge {
                        (right, -1.0)
                    } else {
                        (left, 1.0)
                    };
                    let arc: String = (0..=16)
                        .map(|s| {
                            let theta = std::f64::consts::FRAC_PI_2 * s as f64 / 16.0;
                            format!(
                                "{:.2},{:.2}",
                                hinge + along * theta.sin() * leaf,
                                side * theta.cos() * leaf
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{hinge:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.35\"/><polyline points=\"{arc}\" fill=\"none\" stroke=\"#627489\" stroke-width=\"0.18\"/>",
                        side * leaf
                    );
                }
                OpeningKind::SaloonDoor => {
                    let side = if op.flip_side { -1.0 } else { 1.0 };
                    for hinge in [left, right] {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{hinge:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.30\"/>",
                            side * half
                        );
                    }
                }
                OpeningKind::Gap => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.2}\" y1=\"0\" x2=\"{right:.2}\" y2=\"0\" stroke=\"#627489\" stroke-width=\"0.18\" stroke-dasharray=\"1.5 1.0\"/>"
                    );
                }
                OpeningKind::Window => {
                    let off = (wall_w_mm * 0.25).max(0.4);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.2}\" y1=\"{:.2}\" x2=\"{right:.2}\" y2=\"{:.2}\" stroke=\"#2878d2\" stroke-width=\"0.30\"/><line x1=\"{left:.2}\" y1=\"{off:.2}\" x2=\"{right:.2}\" y2=\"{off:.2}\" stroke=\"#2878d2\" stroke-width=\"0.30\"/>",
                        -off, -off
                    );
                }
            }
            cursor = right;
        }
        let _ = writeln!(
            svg,
            "<line x1=\"{cursor:.2}\" y1=\"0\" x2=\"{len_mm:.2}\" y2=\"0\" stroke=\"#1d2836\" stroke-width=\"{wall_w_mm:.2}\" stroke-linecap=\"square\"/></g>"
        );
    }

    // Fixtures.
    if project.category_visible(LayerCategory::Fixtures) {
        for fixture in &project.fixtures {
            for shape in fixture.symbol() {
                let (color, stroke_w) = match &shape {
                    SymbolShape::Polyline {
                        weight: Weight::Outline,
                        ..
                    }
                    | SymbolShape::Circle {
                        weight: Weight::Outline,
                        ..
                    } => ("#3b4e63", 0.22),
                    _ => ("#627489", 0.14),
                };
                match shape {
                    SymbolShape::Polyline {
                        points,
                        closed,
                        dashed,
                        ..
                    } => {
                        let pts = points
                            .iter()
                            .map(|local| {
                                let (px, py) = pt(fixture.to_world(*local));
                                format!("{px:.2},{py:.2}")
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        let elem = if closed { "polygon" } else { "polyline" };
                        let dash = if dashed {
                            " stroke-dasharray=\"1.2 0.8\""
                        } else {
                            ""
                        };
                        let _ = writeln!(
                            svg,
                            "<{elem} points=\"{pts}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{stroke_w:.2}\"{dash}/>"
                        );
                    }
                    SymbolShape::Circle { center, radius, .. } => {
                        let (cx, cy) = pt(fixture.to_world(center));
                        let _ = writeln!(
                            svg,
                            "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{:.2}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{stroke_w:.2}\"/>",
                            radius * mm_per_m
                        );
                    }
                }
            }
        }
    }

    // CAD Drawings.
    for drawing in &project.drawings {
        if !project.is_drawing_visible(drawing) {
            continue;
        }
        let outline = drawing.shape.outline();
        let Some((first, rest)) = outline.split_first() else {
            continue;
        };
        let (x, y) = pt(*first);
        let mut path = format!("M{x:.2} {y:.2}");
        for corner in rest {
            let (cx, cy) = pt(*corner);
            let _ = write!(path, " L{cx:.2} {cy:.2}");
        }
        let layer = project.effective_drawing_layer(drawing);
        let color = layer
            .map(|l| l.hex_color())
            .unwrap_or_else(|| "#1d2836".to_owned());
        let width_mm = layer.map(|l| l.line_weight_mm).unwrap_or(0.20);
        let dash = layer
            .and_then(|l| l.line_type.svg_dasharray_mm())
            .map(|d| format!(" stroke-dasharray=\"{d}\""))
            .unwrap_or_default();
        let _ = writeln!(
            svg,
            "<path d=\"{path}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width_mm:.2}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{dash}/>"
        );
    }

    // Building dimension chains & associative dimensions.
    if project.category_visible(LayerCategory::Dimensions) {
        if let Some(dims) = building_dimensions(project) {
            let (left, top) = pt(dims.min);
            for (row, stations) in [
                (top - 6.0, dims.xs.clone()),
                (top - 11.5, vec![dims.min.x, dims.max.x]),
            ] {
                let xs: Vec<f64> = stations
                    .iter()
                    .map(|x| pt(Point::new(*x, dims.min.y)).0)
                    .collect();
                let _ = writeln!(
                    svg,
                    "<line x1=\"{:.2}\" y1=\"{row:.2}\" x2=\"{:.2}\" y2=\"{row:.2}\" stroke=\"#3b4e63\" stroke-width=\"0.13\"/>",
                    xs[0],
                    xs[xs.len() - 1]
                );
                for x in &xs {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.25\"/>",
                        x - 0.8,
                        row + 0.8,
                        x + 0.8,
                        row - 0.8
                    );
                }
                for (pair, span) in stations.windows(2).zip(xs.windows(2)) {
                    let val = centimetres(pair[1] - pair[0]);
                    if span[1] - span[0] >= 4.0 {
                        let _ = writeln!(
                            svg,
                            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.3\" fill=\"#1d2836\" text-anchor=\"middle\">{val}</text>",
                            (span[0] + span[1]) * 0.5,
                            row - 0.8
                        );
                    }
                }
            }
            for (col, stations) in [
                (left - 6.0, dims.ys.clone()),
                (left - 11.5, vec![dims.min.y, dims.max.y]),
            ] {
                let ys: Vec<f64> = stations
                    .iter()
                    .map(|y| pt(Point::new(dims.min.x, *y)).1)
                    .collect();
                let _ = writeln!(
                    svg,
                    "<line x1=\"{col:.2}\" y1=\"{:.2}\" x2=\"{col:.2}\" y2=\"{:.2}\" stroke=\"#3b4e63\" stroke-width=\"0.13\"/>",
                    ys[0],
                    ys[ys.len() - 1]
                );
                for y in &ys {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.25\"/>",
                        col - 0.8,
                        y + 0.8,
                        col + 0.8,
                        y - 0.8
                    );
                }
                for (pair, span) in stations.windows(2).zip(ys.windows(2)) {
                    let val = centimetres(pair[1] - pair[0]);
                    if span[1] - span[0] >= 4.0 {
                        let (x, y) = (col - 0.9, (span[0] + span[1]) * 0.5);
                        let _ = writeln!(
                            svg,
                            "<text x=\"{x:.2}\" y=\"{y:.2}\" transform=\"rotate(-90 {x:.2} {y:.2})\" font-family=\"Arial, sans-serif\" font-size=\"2.3\" fill=\"#1d2836\" text-anchor=\"middle\">{val}</text>"
                        );
                    }
                }
            }
        }

        for dim in &project.dimensions {
            if !dim.visible {
                continue;
            }
            let Some(resolved) = project.resolve_dimension(dim) else {
                continue;
            };
            match &resolved {
                ResolvedDimension::Linear {
                    dim_p1,
                    dim_p2,
                    label,
                    ..
                } => {
                    let (x1, y1) = pt(*dim_p1);
                    let (x2, y2) = pt(*dim_p2);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#3b4e63\" stroke-width=\"0.15\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.3\" fill=\"#1d2836\" text-anchor=\"middle\">{}</text>",
                        (x1 + x2) * 0.5,
                        (y1 + y2) * 0.5 - 0.8,
                        escaped(label)
                    );
                }
                ResolvedDimension::Angular {
                    vertex,
                    start_angle,
                    sweep,
                    radius,
                    label,
                    ..
                } => {
                    let pts: String = resolved
                        .arc_points(16)
                        .into_iter()
                        .map(|p| {
                            let (x, y) = pt(p);
                            format!("{x:.2},{y:.2}")
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mid = crate::domain::drawing::point_at_angle(
                        *vertex,
                        radius + 0.18,
                        start_angle + sweep * 0.5,
                    );
                    let (mx, my) = pt(mid);
                    let _ = writeln!(
                        svg,
                        "<polyline points=\"{pts}\" fill=\"none\" stroke=\"#3b4e63\" stroke-width=\"0.15\"/><text x=\"{mx:.2}\" y=\"{my:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.3\" fill=\"#1d2836\" text-anchor=\"middle\">{}</text>",
                        escaped(label)
                    );
                }
            }
        }
    }

    // Room labels.
    if project.category_visible(LayerCategory::Rooms) {
        for room in &project.rooms {
            if let Some((anchor, _, _)) = project.room_label_anchor(room) {
                let (cx, cy) = pt(anchor);
                let area = project
                    .room_area(room)
                    .map(|a| format!("{a:.2} m²").replace('.', ","))
                    .unwrap_or_default();
                let _ = writeln!(
                    svg,
                    "<text x=\"{cx:.2}\" y=\"{cy:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.8\" font-weight=\"700\" fill=\"#1d2836\" text-anchor=\"middle\">{}</text>\n\
                     <text x=\"{cx:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.2\" fill=\"#3b4e63\" text-anchor=\"middle\">{}</text>",
                    escaped(room.display_name()),
                    cy + 3.2,
                    escaped(&area)
                );
            }
        }
    }

    // Notes & symbols.
    if project.category_visible(LayerCategory::Annotations) {
        for note in &project.notes {
            let (x, y) = pt(note.position);
            let txt = escaped(&note.text);
            match note.kind {
                NoteKind::Text | NoteKind::Note => {
                    let _ = writeln!(
                        svg,
                        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.5\" fill=\"#1d2836\">{txt}</text>"
                    );
                }
                NoteKind::Callout { target } => {
                    let (tx, ty) = pt(target);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{tx:.2}\" y2=\"{ty:.2}\" stroke=\"#2878d2\" stroke-width=\"0.20\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"#1d2836\">{txt}</text>",
                        x + 1.0,
                        y - 0.8
                    );
                }
                NoteKind::Symbol(TechnicalSymbol::LevelMark { .. }) => {
                    let _ = writeln!(
                        svg,
                        "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.6\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.22\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.2\" font-weight=\"700\" fill=\"#1d2836\">{txt}</text>",
                        x + 2.4,
                        y + 0.8
                    );
                }
                NoteKind::Symbol(_) => {
                    let _ = writeln!(
                        svg,
                        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.3\" font-weight=\"700\" fill=\"#1d2836\">{txt}</text>"
                    );
                }
            }
        }
    }

    svg.push_str("</g>\n");

    // Compact bottom-left scale caption on the sheet.
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"3.0\" font-weight=\"700\" fill=\"#1d2836\">{} · {} · Escala {}</text>",
        frame_x + 4.0,
        frame_y + frame_h - 4.0,
        escaped(&project.name),
        escaped(layout.size.short_name()),
        escaped(&layout.scale.label())
    );
    svg.push_str("</svg>");
    svg
}

pub fn export_scaled_svg(project: &Project, path: &Path) -> Result<(), String> {
    project.sheet.validate()?;
    fs::write(path, scaled_sheet_svg(project)).map_err(|error| error.to_string())
}

pub fn export_scaled_pdf(project: &Project, path: &Path) -> Result<(), String> {
    project.sheet.validate()?;
    let markup = scaled_sheet_svg(project);
    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree =
        svg2pdf::usvg::Tree::from_str(&markup, &options).map_err(|error| error.to_string())?;
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
    use crate::domain::sheet::{DrawingScale, Orientation, PaperSize, SheetLayout};

    #[test]
    fn scaled_sheet_svg_uses_exact_physical_mm_and_scale() {
        let mut project = Project::default();
        project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        project.sheet.scale = DrawingScale::S1_50;
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let markup = scaled_sheet_svg(&project);
        assert!(markup.contains("width=\"420.00mm\" height=\"297.00mm\""));
        assert!(markup.contains("viewBox=\"0 0 420.00 297.00\""));
        // 4.00 m at 1:50 = 80.00 mm!
        assert!(markup.contains("x2=\"80.00\""));
        assert!(markup.contains("Escala 1:50"));
    }
}
