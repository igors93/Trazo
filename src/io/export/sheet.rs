//! Exact physical-scale (millimetre-accurate) sheet export in SVG and PDF.
//!
//! One SVG user unit equals exactly 1.00 mm on paper (`width="420.00mm"
//! height="297.00mm" viewBox="0 0 420.00 297.00"`), so a 1.00 m wall at 1:50
//! is rendered at literally 20.00 mm and checks out with a physical scale ruler.

use std::{fmt::Write as _, fs, path::Path};

use super::{
    CONTRAST_VEIL_OPACITY, ExportView, escaped, fixture_in_discipline_overlay, project_on_floor,
};
use crate::domain::{
    annotations::building_dimensions,
    dimensions::ResolvedDimension,
    discipline::Discipline,
    doc_style::DimensionTick,
    fixtures::{Fixture, SymbolShape, Weight},
    geometry::Point,
    hatch::{HatchPattern, hatch_segments_in_polygon},
    layers::LayerCategory,
    model::{OpeningKind, Project},
    notes::{NoteKind, TechnicalSymbol},
};

fn write_dimension_tick(
    svg: &mut String,
    tick: DimensionTick,
    x: f64,
    y: f64,
    color: &str,
    horizontal: bool,
) {
    match tick {
        DimensionTick::Oblique45 => {
            let _ = writeln!(
                svg,
                "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{color}\" stroke-width=\"0.25\"/>",
                x - 0.8,
                y + 0.8,
                x + 0.8,
                y - 0.8
            );
        }
        DimensionTick::Dot => {
            let _ = writeln!(
                svg,
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"0.55\" fill=\"{color}\"/>"
            );
        }
        DimensionTick::Arrow => {
            if horizontal {
                let _ = writeln!(
                    svg,
                    "<polygon points=\"{x:.2},{y:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{color}\"/>",
                    x - 1.1,
                    y - 0.45,
                    x - 1.1,
                    y + 0.45
                );
            } else {
                let _ = writeln!(
                    svg,
                    "<polygon points=\"{x:.2},{y:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{color}\"/>",
                    x - 0.45,
                    y - 1.1,
                    x + 0.45,
                    y - 1.1
                );
            }
        }
    }
}

fn write_sheet_fixture(
    svg: &mut String,
    fixture: &Fixture,
    pt: &dyn Fn(Point) -> (f64, f64),
    mm_per_m: f64,
) {
    let electrical = fixture.kind.is_electrical();
    for shape in fixture.symbol() {
        let (color, stroke_w) = match &shape {
            SymbolShape::Polyline {
                weight: Weight::Outline,
                ..
            }
            | SymbolShape::Circle {
                weight: Weight::Outline,
                ..
            } if electrical => (crate::io::export::ELECTRICAL_INK, 0.2),
            _ if electrical => (crate::io::export::ELECTRICAL_INK, 0.12),
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
        let (color, stroke_w) = match &shape {
            SymbolShape::Polyline {
                weight: Weight::Outline,
                ..
            }
            | SymbolShape::Circle {
                weight: Weight::Outline,
                ..
            } if fixture.kind.is_structural() => (crate::io::export::STRUCTURE_INK, 0.25),
            _ if fixture.kind.is_structural() => (crate::io::export::STRUCTURE_INK, 0.1),
            _ => (color, stroke_w),
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

fn write_sheet_discipline_overlay(
    svg: &mut String,
    project: &Project,
    discipline: Discipline,
    pt: &dyn Fn(Point) -> (f64, f64),
    mm_per_m: f64,
) {
    let _ = writeln!(
        svg,
        "<g id=\"disciplina-foco\" data-discipline=\"{}\">",
        discipline.key()
    );
    for fixture in &project.fixtures {
        if !fixture_in_discipline_overlay(discipline, fixture) {
            continue;
        }
        if !project.category_visible(Project::fixture_category(fixture)) {
            continue;
        }
        write_sheet_fixture(svg, fixture, pt, mm_per_m);
    }
    match discipline {
        Discipline::Structure => {
            if project.category_visible(LayerCategory::Structure) {
                super::conduits::write_structure_scoped(
                    svg,
                    project,
                    crate::domain::structure::StructureMarksScope::Superstructure,
                    pt,
                    mm_per_m,
                    (0.25, 0.15, (mm_per_m * 0.12).clamp(1.4, 2.4)),
                );
            }
        }
        Discipline::Foundations => {
            if project.category_visible(LayerCategory::Structure) {
                super::conduits::write_structure_scoped(
                    svg,
                    project,
                    crate::domain::structure::StructureMarksScope::Foundations,
                    pt,
                    mm_per_m,
                    (0.25, 0.15, (mm_per_m * 0.12).clamp(1.4, 2.4)),
                );
            }
        }
        Discipline::Electrical => {
            if project.category_visible(LayerCategory::Electrical) {
                super::conduits::write_conduits(
                    svg,
                    project,
                    pt,
                    mm_per_m,
                    (0.25, 0.15, (mm_per_m * 0.11).clamp(1.3, 2.2)),
                );
            }
        }
        Discipline::Plumbing | Discipline::All | Discipline::Architecture => {}
    }
    svg.push_str("</g>\n");
}

/// Renders the project onto its configured physical sheet (`A4`–`A0` or custom)
/// at the exact configured scale (`1:25`–`1:200`), returning physical-mm SVG.
pub fn scaled_sheet_svg(project: &Project) -> String {
    scaled_sheet_svg_with_overlay(project, None)
}

/// Renders one view/tab onto the project's configured physical sheet at exact
/// scale, matching the tab's discipline (with faded architectural reference
/// underlay for specialized disciplines), floor, hidden layers and pagination.
pub fn scaled_sheet_svg_for_view(
    project: &Project,
    view: &ExportView,
    sheet_page: Option<(usize, usize)>,
) -> String {
    let base = project_on_floor(project, view.floor_id);
    let mut themed_base = base.into_owned();
    let multi_floor = themed_base.floors.levels.len() > 1;
    let floor_suffix = if multi_floor {
        themed_base
            .active_floor()
            .map(|f| format!(" · {}", f.name))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let default_title = themed_base.sheet.title_block.drawing_title.trim();
    if view.discipline.focus().is_some()
        || default_title.is_empty()
        || default_title.eq_ignore_ascii_case("Planta Baixa")
        || sheet_page.is_some()
    {
        themed_base.sheet.title_block.drawing_title =
            format!("{}{}", view.discipline.sheet_title(), floor_suffix);
    }
    if let Some((index, total)) = sheet_page {
        themed_base.sheet.title_block.sheet_number = format!("{index:02}/{total:02}");
    }
    if view.discipline.focus().is_some() {
        let underlay = themed_base.for_underlay(view.discipline, &view.hidden_layers);
        let overlay = themed_base.for_view(view.discipline, &view.hidden_layers);
        scaled_sheet_svg_with_overlay(&underlay, Some((&overlay, view.discipline)))
    } else {
        let shown = themed_base.for_view(view.discipline, &view.hidden_layers);
        scaled_sheet_svg_with_overlay(&shown, None)
    }
}

fn scaled_sheet_svg_with_overlay(
    project: &Project,
    focused_overlay: Option<(&Project, Discipline)>,
) -> String {
    let layout = &project.sheet;
    let doc_style = &project.doc_style;
    let wall_color = doc_style.wall_color();
    let opening_color = doc_style.opening_color();
    let dim_color = doc_style.dimension_color();
    let room_fill = doc_style.room_fill();
    let font_family = escaped(&doc_style.font_family);
    let (sheet_w, sheet_h) = layout.sheet_mm();
    let (frame_x, frame_y, frame_w, frame_h) = layout.printable_rect_mm();
    let mm_per_m = layout.scale.mm_per_meter();

    let bounds_source = focused_overlay.map_or(project, |(overlay, _)| overlay);
    let (min, max) = bounds_source
        .bounds()
        .or_else(|| project.bounds())
        .unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let plan_w_m = (max.x - min.x).max(0.5);
    let plan_h_m = (max.y - min.y).max(0.5);
    let plan_w_mm = plan_w_m * mm_per_m;
    let plan_h_mm = plan_h_m * mm_per_m;

    let reserve_bottom = if layout.title_block.show_title_block {
        50.0
    } else {
        14.0
    };
    let draw_area_h = (frame_h - reserve_bottom).max(20.0);
    let plan_cropped = !layout.scale.fits(plan_w_m, plan_h_m, frame_w, draw_area_h);
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
                Some(style) if style.pattern == HatchPattern::Solid && !doc_style.monochrome => {
                    style.hex_color()
                }
                _ => room_fill.to_owned(),
            };
            let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"{fill}\"/>");
            if let Some(style) = room.hatch {
                let stroke = if doc_style.monochrome {
                    "#444444".to_owned()
                } else {
                    style.hex_color()
                };
                for (a, b) in hatch_segments_in_polygon(&poly, &style) {
                    let (x1, y1) = pt(a);
                    let (x2, y2) = pt(b);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{stroke}\" stroke-width=\"{:.2}\" opacity=\"0.8\"/>",
                        doc_style.hatch_pen_mm
                    );
                }
            }
        }
    }

    // Architectural double-line (hollow) walls at exact scaled thickness in
    // three passes so L-, T-, and X-junctions merge seamlessly:
    //   1) outer dark contour (junction patches + wall runs),
    //   2) inner white hollow cavity (inset junctions + inset runs),
    //   3) openings (jambs, doors, windows, passages) on top.
    let base_wall_mm = (project.masonry.thickness() * mm_per_m).max(doc_style.wall_pen_mm);
    let border_mm = doc_style.wall_pen_mm.min(base_wall_mm * 0.28).max(0.18);
    let draw_hollow = base_wall_mm >= 2.0 * border_mm + 0.3;

    let sheet_walls: Vec<_> = if project.category_visible(LayerCategory::Walls) {
        project
            .walls
            .iter()
            .filter(|wall| project.is_wall_visible(wall))
            .map(|wall| {
                let wall_w_mm = (wall.thickness() * mm_per_m).max(doc_style.wall_pen_mm);
                let half_w = wall_w_mm * 0.5;
                let (x, y) = pt(wall.start);
                let len_mm = wall.length() * mm_per_m;
                let angle = (wall.end.y - wall.start.y)
                    .atan2(wall.end.x - wall.start.x)
                    .to_degrees();
                let mut openings: Vec<_> = project
                    .openings
                    .iter()
                    .filter(|o| o.wall_id == wall.id)
                    .collect();
                openings.sort_by(|a, b| a.t.total_cmp(&b.t));
                let mut runs = Vec::with_capacity(openings.len() + 1);
                let mut cursor = 0.0;
                let mut run_start = -half_w;
                for op in &openings {
                    let half = op.width() * mm_per_m * 0.5;
                    let center = op.t * len_mm;
                    let left = center - half;
                    let right = center + half;
                    if left > cursor {
                        runs.push((run_start, left));
                    }
                    cursor = right;
                    run_start = right;
                }
                if cursor < len_mm {
                    runs.push((run_start, len_mm + half_w));
                }
                (wall_w_mm, half_w, x, y, len_mm, angle, openings, runs)
            })
            .collect()
    } else {
        Vec::new()
    };

    // Pass 1: outer contours.
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
            let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"{wall_color}\"/>");
        }
    }
    for &(wall_w_mm, half_w, x, y, _, angle, _, ref runs) in &sheet_walls {
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.2} {y:.2}) rotate({angle:.3})\">"
        );
        for &(x0, x1) in runs {
            let w = (x1 - x0).max(0.0);
            if w > 0.02 {
                let _ = writeln!(
                    svg,
                    "<rect x=\"{x0:.2}\" y=\"{:.2}\" width=\"{w:.2}\" height=\"{wall_w_mm:.2}\" fill=\"{wall_color}\"/>",
                    -half_w
                );
            }
        }
        svg.push_str("</g>\n");
    }

    // Pass 2: inner white hollow cavity.
    if draw_hollow && project.category_visible(LayerCategory::Walls) {
        let border_m = border_mm / mm_per_m;
        for junction in project.wall_junctions_with_inset(border_m) {
            let pts: String = junction
                .outline
                .iter()
                .map(|p| {
                    let (x, y) = pt(*p);
                    format!("{x:.2},{y:.2}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"#ffffff\"/>");
        }
    }
    for &(wall_w_mm, half_w, x, y, _, angle, _, ref runs) in &sheet_walls {
        let inner_h = wall_w_mm - 2.0 * border_mm;
        if inner_h < 0.3 {
            continue;
        }
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.2} {y:.2}) rotate({angle:.3})\">"
        );
        for &(x0, x1) in runs {
            let ix0 = x0 + border_mm;
            let ix1 = x1 - border_mm;
            if ix1 - ix0 > 0.1 {
                let _ = writeln!(
                    svg,
                    "<rect x=\"{ix0:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{inner_h:.2}\" fill=\"#ffffff\"/>",
                    -(half_w - border_mm),
                    ix1 - ix0
                );
            }
        }
        svg.push_str("</g>\n");
    }

    // Pass 3: openings (jambs, doors, windows, passages) on top of walls.
    for &(wall_w_mm, half_w, x, y, len_mm, angle, ref openings, _) in &sheet_walls {
        if openings.is_empty() {
            continue;
        }
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.2} {y:.2}) rotate({angle:.3})\">"
        );
        for op in openings {
            let half = op.width() * mm_per_m * 0.5;
            let center = op.t * len_mm;
            let left = center - half;
            let right = center + half;
            for jamb_x in [left, right] {
                let _ = writeln!(
                    svg,
                    "<line x1=\"{jamb_x:.2}\" y1=\"{:.2}\" x2=\"{jamb_x:.2}\" y2=\"{half_w:.2}\" stroke=\"{wall_color}\" stroke-width=\"{border_mm:.2}\"/>",
                    -half_w
                );
            }
            match op.kind {
                OpeningKind::Door | OpeningKind::Gate => {
                    use crate::domain::openings::DoorType;
                    let leaf = half * 2.0;
                    let side = if op.flip_side { -1.0 } else { 1.0 };
                    let (hinge, along) = if op.flip_hinge {
                        (right, -1.0)
                    } else {
                        (left, 1.0)
                    };
                    match op.effective_door_type() {
                        DoorType::Swing => {
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
                                "<line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{hinge:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/><polyline points=\"{arc}\" fill=\"none\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/>",
                                side * leaf,
                                doc_style.opening_pen_mm,
                                doc_style.dimension_pen_mm
                            );
                        }
                        DoorType::DoubleSwing => {
                            for (h, dir_h) in [(left, 1.0), (right, -1.0)] {
                                let arc: String = (0..=12)
                                    .map(|s| {
                                        let theta = std::f64::consts::FRAC_PI_2 * s as f64 / 12.0;
                                        format!(
                                            "{:.2},{:.2}",
                                            h + dir_h * theta.sin() * half,
                                            side * theta.cos() * half
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let _ = writeln!(
                                    svg,
                                    "<line x1=\"{h:.2}\" y1=\"0\" x2=\"{h:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/><polyline points=\"{arc}\" fill=\"none\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/>",
                                    side * half,
                                    doc_style.opening_pen_mm,
                                    doc_style.dimension_pen_mm
                                );
                            }
                        }
                        DoorType::Sliding => {
                            let y_leaf = side * (wall_w_mm * 0.5 + 0.8);
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{hinge:.2}\" y1=\"{y_leaf:.2}\" x2=\"{:.2}\" y2=\"{y_leaf:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                                hinge + along * leaf,
                                doc_style.opening_pen_mm * 1.3
                            );
                        }
                        DoorType::PocketSliding => {
                            let pocket = hinge - along * leaf;
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{pocket:.2}\" y2=\"0\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\" stroke-dasharray=\"1.2 0.8\"/><line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{:.2}\" y2=\"0\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                                doc_style.dimension_pen_mm,
                                hinge + along * (leaf * 0.65),
                                doc_style.opening_pen_mm * 1.3
                            );
                        }
                        DoorType::Pivot => {
                            let pivot = hinge + along * (leaf * 0.20);
                            let _ = writeln!(
                                svg,
                                "<circle cx=\"{pivot:.2}\" cy=\"0\" r=\"0.4\" fill=\"{wall_color}\"/><line x1=\"{pivot:.2}\" y1=\"{:.2}\" x2=\"{pivot:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                                -side * leaf * 0.20,
                                side * leaf * 0.80,
                                doc_style.opening_pen_mm
                            );
                        }
                        DoorType::Bifold => {
                            let step = leaf * 0.72 / 4.0;
                            let peak = half * 0.55;
                            let pts: String = (0..=4)
                                .map(|i| {
                                    let y = if i % 2 == 1 { side * peak } else { 0.0 };
                                    format!("{:.2},{y:.2}", hinge + along * step * i as f64)
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{pts}\" fill=\"none\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                                doc_style.opening_pen_mm
                            );
                        }
                        DoorType::Revolving => {
                            let d = half * std::f64::consts::FRAC_1_SQRT_2;
                            let _ = writeln!(
                                svg,
                                "<circle cx=\"{center:.2}\" cy=\"0\" r=\"{half:.2}\" fill=\"none\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/><line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/><line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                                doc_style.dimension_pen_mm,
                                center - d,
                                -d,
                                center + d,
                                d,
                                doc_style.opening_pen_mm,
                                center - d,
                                d,
                                center + d,
                                -d,
                                doc_style.opening_pen_mm
                            );
                        }
                    }
                }
                OpeningKind::SaloonDoor => {
                    let side = if op.flip_side { -1.0 } else { 1.0 };
                    for hinge in [left, right] {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{hinge:.2}\" y1=\"0\" x2=\"{hinge:.2}\" y2=\"{:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                            side * half,
                            doc_style.opening_pen_mm
                        );
                    }
                }
                OpeningKind::Gap => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.2}\" y1=\"0\" x2=\"{right:.2}\" y2=\"0\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\" stroke-dasharray=\"1.5 1.0\"/>",
                        doc_style.dimension_pen_mm
                    );
                }
                OpeningKind::Window => {
                    use crate::domain::openings::WindowType;
                    let off = (wall_w_mm * 0.25).max(0.4);
                    let side = if op.flip_side { -1.0 } else { 1.0 };
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{left:.2}\" y1=\"{:.2}\" x2=\"{right:.2}\" y2=\"{:.2}\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/><line x1=\"{left:.2}\" y1=\"{off:.2}\" x2=\"{right:.2}\" y2=\"{off:.2}\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/>",
                        -off, -off, doc_style.opening_pen_mm, doc_style.opening_pen_mm
                    );
                    match op.effective_window_type() {
                        WindowType::Sliding => {
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.2}\" y1=\"0\" x2=\"{right:.2}\" y2=\"0\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/>",
                                doc_style.dimension_pen_mm
                            );
                        }
                        WindowType::Basculante => {
                            let py = side * (off + 1.0);
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.2}\" y1=\"{py:.2}\" x2=\"{right:.2}\" y2=\"{py:.2}\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\" stroke-dasharray=\"1.0 0.7\"/>",
                                doc_style.dimension_pen_mm
                            );
                        }
                        WindowType::MaximAr => {
                            let py = side * (off + 1.8);
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{left:.2},0 {center:.2},{py:.2} {right:.2},0\" fill=\"none\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\" stroke-dasharray=\"1.0 0.7\"/>",
                                doc_style.dimension_pen_mm
                            );
                        }
                        WindowType::Casement | WindowType::TiltTurn => {
                            let r = half * 0.75;
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.2}\" y1=\"0\" x2=\"{left:.2}\" y2=\"{:.2}\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/><line x1=\"{right:.2}\" y1=\"0\" x2=\"{right:.2}\" y2=\"{:.2}\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/>",
                                side * r,
                                doc_style.dimension_pen_mm,
                                side * r,
                                doc_style.dimension_pen_mm
                            );
                        }
                        WindowType::Fixed => {
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.2}\" y1=\"0\" x2=\"{right:.2}\" y2=\"0\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/>",
                                doc_style.opening_pen_mm
                            );
                        }
                        WindowType::BayWindow => {
                            let py = side * (off + 2.5);
                            let inset = half * 0.35;
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{left:.2},0 {:.2},{py:.2} {:.2},{py:.2} {right:.2},0\" fill=\"none\" stroke=\"{opening_color}\" stroke-width=\"{:.2}\"/>",
                                left + inset,
                                right - inset,
                                doc_style.opening_pen_mm
                            );
                        }
                    }
                }
            }
        }
        svg.push_str("</g>\n");
    }

    // Fixtures.
    for fixture in &project.fixtures {
        if !project.category_visible(Project::fixture_category(fixture)) {
            continue;
        }
        write_sheet_fixture(&mut svg, fixture, &pt, mm_per_m);
    }

    // Beams and labels of the formwork plan.
    if project.category_visible(LayerCategory::Structure) {
        super::conduits::write_structure(
            &mut svg,
            project,
            &pt,
            mm_per_m,
            (0.25, 0.15, (mm_per_m * 0.12).clamp(1.4, 2.4)),
        );
    }

    // Conduits and their wiring.
    if project.category_visible(LayerCategory::Electrical) {
        super::conduits::write_conduits(
            &mut svg,
            project,
            &pt,
            mm_per_m,
            (0.25, 0.15, (mm_per_m * 0.11).clamp(1.3, 2.2)),
        );
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
        let color = if doc_style.monochrome {
            wall_color.to_owned()
        } else {
            layer
                .map(|l| l.hex_color())
                .unwrap_or_else(|| wall_color.to_owned())
        };
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

    // Stairs, ramps, landings, railings and level transitions.
    for elem in &project.circulations {
        let geom = elem.plan_geometry();
        for poly in &geom.polygons {
            let pts: String = poly
                .iter()
                .map(|p| {
                    let (x, y) = pt(*p);
                    format!("{x:.2},{y:.2}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(
                svg,
                "<polygon points=\"{pts}\" fill=\"{room_fill}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                doc_style.opening_pen_mm
            );
        }
        for (a, b) in &geom.lines {
            let (x1, y1) = pt(*a);
            let (x2, y2) = pt(*b);
            let _ = writeln!(
                svg,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/>",
                doc_style.dimension_pen_mm
            );
        }
        for (a, b) in &geom.railing_lines {
            let (x1, y1) = pt(*a);
            let (x2, y2) = pt(*b);
            let _ = writeln!(
                svg,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{wall_color}\" stroke-width=\"{:.2}\"/>",
                doc_style.wall_pen_mm
            );
        }
        if !geom.label.is_empty() {
            let (lx, ly) = pt(geom.label_pos);
            let _ = writeln!(
                svg,
                "<text x=\"{lx:.2}\" y=\"{ly:.2}\" font-family=\"{font_family}\" font-size=\"2.1\" font-weight=\"700\" fill=\"{wall_color}\" text-anchor=\"middle\">{}</text>",
                escaped(&geom.label)
            );
        }
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
                    "<line x1=\"{:.2}\" y1=\"{row:.2}\" x2=\"{:.2}\" y2=\"{row:.2}\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/>",
                    xs[0],
                    xs[xs.len() - 1],
                    doc_style.dimension_pen_mm
                );
                for x in &xs {
                    write_dimension_tick(&mut svg, doc_style.tick, *x, row, wall_color, true);
                }
                for (pair, span) in stations.windows(2).zip(xs.windows(2)) {
                    let val = doc_style.format_length(pair[1] - pair[0]);
                    if span[1] - span[0] >= 4.0 {
                        let _ = writeln!(
                            svg,
                            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"{font_family}\" font-size=\"2.3\" fill=\"{wall_color}\" text-anchor=\"middle\">{}</text>",
                            (span[0] + span[1]) * 0.5,
                            row - 0.8,
                            escaped(&val)
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
                    "<line x1=\"{col:.2}\" y1=\"{:.2}\" x2=\"{col:.2}\" y2=\"{:.2}\" stroke=\"{dim_color}\" stroke-width=\"{:.2}\"/>",
                    ys[0],
                    ys[ys.len() - 1],
                    doc_style.dimension_pen_mm
                );
                for y in &ys {
                    write_dimension_tick(&mut svg, doc_style.tick, col, *y, wall_color, false);
                }
                for (pair, span) in stations.windows(2).zip(ys.windows(2)) {
                    let val = doc_style.format_length(pair[1] - pair[0]);
                    if span[1] - span[0] >= 4.0 {
                        let (x, y) = (col - 0.9, (span[0] + span[1]) * 0.5);
                        let _ = writeln!(
                            svg,
                            "<text x=\"{x:.2}\" y=\"{y:.2}\" transform=\"rotate(-90 {x:.2} {y:.2})\" font-family=\"{font_family}\" font-size=\"2.3\" fill=\"{wall_color}\" text-anchor=\"middle\">{}</text>",
                            escaped(&val)
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
                        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{font_family}\" font-size=\"2.3\" font-weight=\"700\" fill=\"{wall_color}\">{txt}</text>"
                    );
                }
            }
        }
        for section in &project.views.sections {
            let (x1, y1) = pt(section.start);
            let (x2, y2) = pt(section.end);
            let tag = escaped(&section.label);
            let _ = writeln!(
                svg,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{wall_color}\" stroke-width=\"0.35\" stroke-dasharray=\"5.0 1.4 0.9 1.4\"/>\n\
                 <circle cx=\"{x1:.2}\" cy=\"{y1:.2}\" r=\"2.4\" fill=\"#ffffff\" stroke=\"{wall_color}\" stroke-width=\"0.30\"/><text x=\"{x1:.2}\" y=\"{:.2}\" font-family=\"{font_family}\" font-size=\"2.0\" font-weight=\"700\" fill=\"{wall_color}\" text-anchor=\"middle\">{tag}</text>\n\
                 <circle cx=\"{x2:.2}\" cy=\"{y2:.2}\" r=\"2.4\" fill=\"#ffffff\" stroke=\"{wall_color}\" stroke-width=\"0.30\"/><text x=\"{x2:.2}\" y=\"{:.2}\" font-family=\"{font_family}\" font-size=\"2.0\" font-weight=\"700\" fill=\"{wall_color}\" text-anchor=\"middle\">{tag}</text>",
                y1 + 0.7,
                y2 + 0.7
            );
        }
        for detail in &project.views.details {
            let (cx, cy) = pt(detail.center);
            let r = detail.radius * mm_per_m;
            let tag = match &detail.target_sheet {
                Some(s) => format!("{} ({s})", detail.label),
                None => detail.label.clone(),
            };
            let _ = writeln!(
                svg,
                "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{r:.2}\" fill=\"none\" stroke=\"{wall_color}\" stroke-width=\"0.28\" stroke-dasharray=\"2.2 1.2\"/><text x=\"{cx:.2}\" y=\"{:.2}\" font-family=\"{font_family}\" font-size=\"2.2\" font-weight=\"700\" fill=\"{wall_color}\" text-anchor=\"middle\">{}</text>",
                cy - r - 1.2,
                escaped(&tag)
            );
        }
    }

    if let Some((overlay_project, discipline)) = focused_overlay {
        let _ = writeln!(
            svg,
            "<rect x=\"{frame_x:.2}\" y=\"{frame_y:.2}\" width=\"{frame_w:.2}\" height=\"{frame_h:.2}\" fill=\"#ffffff\" fill-opacity=\"{CONTRAST_VEIL_OPACITY:.2}\" data-contrast-veil=\"true\"/>"
        );
        write_sheet_discipline_overlay(&mut svg, overlay_project, discipline, &pt, mm_per_m);
    }

    svg.push_str("</g>\n");

    let tb = &layout.title_block;
    if tb.show_title_block {
        write_title_block(
            &mut svg,
            project,
            frame_x,
            frame_y,
            frame_w,
            frame_h,
            &layout.scale.label(),
            layout.size.short_name(),
        );
    } else {
        let _ = writeln!(
            svg,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"3.0\" font-weight=\"700\" fill=\"#1d2836\">{} · {} · Escala {}</text>",
            frame_x + 4.0,
            frame_y + frame_h - 4.0,
            escaped(&project.name),
            escaped(layout.size.short_name()),
            escaped(&layout.scale.label())
        );
    }
    if tb.show_legend {
        write_legend(&mut svg, frame_x + 4.0, frame_y + frame_h - 32.0);
        let wire_source = focused_overlay.map_or(project, |(overlay, _)| overlay);
        if wire_source.category_visible(LayerCategory::Electrical) {
            super::conduits::write_wire_legend(
                &mut svg,
                wire_source,
                frame_x + 4.0,
                frame_y + frame_h - 62.0,
            );
        }
    }
    if tb.show_north {
        write_north_symbol(&mut svg, frame_x + frame_w - 14.0, frame_y + 15.0);
    }
    if tb.show_scale_bar {
        write_scale_bar(&mut svg, frame_x + 4.0, frame_y + frame_h - 8.0, mm_per_m);
    }
    if project.schedules.include_on_sheet {
        write_schedules_on_sheet(&mut svg, project, frame_x, frame_y, frame_w, frame_h);
    }
    if plan_cropped {
        let suggested =
            crate::domain::sheet::DrawingScale::best_fit(plan_w_m, plan_h_m, frame_w, draw_area_h);
        let suggestion = if suggested.fits(plan_w_m, plan_h_m, frame_w, draw_area_h) {
            format!("use folha maior ou escala {}", suggested.label())
        } else {
            "use uma folha maior".to_owned()
        };
        let msg = format!(
            "AVISO: planta recortada na folha {} em {} ({suggestion}).",
            layout.size.short_name(),
            layout.scale.label()
        );
        let box_w = (frame_w - 28.0).clamp(60.0, 155.0);
        let bx = frame_x + 3.0;
        let by = frame_y + 3.0;
        let _ = writeln!(
            svg,
            "<g id=\"aviso-recorte\" data-cropped=\"true\">\n\
             <rect x=\"{bx:.2}\" y=\"{by:.2}\" width=\"{box_w:.2}\" height=\"6.5\" rx=\"1.0\" fill=\"#fff3cd\" stroke=\"#b45309\" stroke-width=\"0.35\"/>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"{font_family}\" font-size=\"2.4\" font-weight=\"700\" fill=\"#92400e\">{}</text>\n\
             </g>",
            bx + 2.5,
            by + 4.2,
            escaped(&msg)
        );
    }
    svg.push_str("</svg>");
    svg
}

fn write_schedules_on_sheet(
    svg: &mut String,
    project: &Project,
    frame_x: f64,
    frame_y: f64,
    frame_w: f64,
    frame_h: f64,
) {
    let tables = crate::domain::schedules::active_schedules(project, &project.schedules);
    if tables.is_empty() {
        return;
    }
    let table_w = 88.0_f64.min(frame_w * 0.42);
    let x = frame_x + frame_w - table_w - 3.0;
    let title_block_h = if project.sheet.title_block.show_title_block {
        44.0_f64.min(frame_h * 0.35) + 4.0
    } else {
        12.0
    };
    let max_bottom = frame_y + frame_h - title_block_h;
    let mut cursor_y = frame_y + 26.0;
    let row_h = 3.8;

    let _ = writeln!(svg, "<g id=\"quadros\">");
    for table in &tables {
        let total_rows = 2 + table.rows.len() + usize::from(table.footer.is_some());
        let h = total_rows as f64 * row_h + 1.5;
        if cursor_y + h > max_bottom {
            break;
        }
        let cols = table.headers.len().max(1);
        let col_w = table_w / cols as f64;
        let _ = writeln!(
            svg,
            "<rect x=\"{x:.2}\" y=\"{cursor_y:.2}\" width=\"{table_w:.2}\" height=\"{h:.2}\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.24\"/>\n\
             <rect x=\"{x:.2}\" y=\"{cursor_y:.2}\" width=\"{table_w:.2}\" height=\"{row_h:.2}\" fill=\"#edf2f7\" stroke=\"#1d2836\" stroke-width=\"0.20\"/>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.1\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
            x + 1.8,
            cursor_y + 2.8,
            escaped(&table.title)
        );
        let mut y = cursor_y + row_h;
        // Headers
        for (i, header) in table.headers.iter().enumerate() {
            let _ = writeln!(
                svg,
                "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#3b4e63\">{}</text>",
                x + 1.5 + i as f64 * col_w,
                y + 2.7,
                escaped(header)
            );
        }
        y += row_h;
        let _ = writeln!(
            svg,
            "<line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"#b8c4d0\" stroke-width=\"0.16\"/>",
            x + table_w
        );
        for row in &table.rows {
            for (i, cell) in row.iter().enumerate() {
                let _ = writeln!(
                    svg,
                    "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.65\" fill=\"#1d2836\">{}</text>",
                    x + 1.5 + i as f64 * col_w,
                    y + 2.7,
                    escaped(cell)
                );
            }
            y += row_h;
        }
        if let Some(footer) = &table.footer {
            let _ = writeln!(
                svg,
                "<line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"#1d2836\" stroke-width=\"0.18\"/>",
                x + table_w
            );
            for (i, cell) in footer.iter().enumerate() {
                let _ = writeln!(
                    svg,
                    "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
                    x + 1.5 + i as f64 * col_w,
                    y + 2.7,
                    escaped(cell)
                );
            }
        }
        cursor_y += h + 3.0;
    }
    svg.push_str("</g>\n");
}

#[allow(clippy::too_many_arguments)]
fn write_title_block(
    svg: &mut String,
    project: &Project,
    frame_x: f64,
    frame_y: f64,
    frame_w: f64,
    frame_h: f64,
    scale_label: &str,
    paper_name: &str,
) {
    let tb = &project.sheet.title_block;
    let w = 175.0_f64.min(frame_w);
    let h = 44.0_f64.min(frame_h * 0.35);
    let x = frame_x + frame_w - w;
    let y = frame_y + frame_h - h;

    let _ = writeln!(svg, "<g id=\"carimbo\">");
    let _ = writeln!(
        svg,
        "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.45\"/>"
    );
    // Horizontal dividers.
    for dy in [12.0, 22.0, 32.0] {
        if dy < h {
            let _ = writeln!(
                svg,
                "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.22\"/>",
                y + dy,
                x + w,
                y + dy
            );
        }
    }
    // Vertical dividers on lower rows (scale / date / sheet / rev).
    let col1 = x + w * 0.50;
    let col2 = x + w * 0.72;
    let col3 = x + w * 0.86;
    for cx in [col1, col2, col3] {
        let _ = writeln!(
            svg,
            "<line x1=\"{cx:.2}\" y1=\"{:.2}\" x2=\"{cx:.2}\" y2=\"{:.2}\" stroke=\"#1d2836\" stroke-width=\"0.22\"/>",
            y + 22.0,
            y + h
        );
    }

    let drawing_title = if tb.drawing_title.trim().is_empty() {
        "Planta Baixa"
    } else {
        tb.drawing_title.trim()
    };
    let client = if tb.client.trim().is_empty() {
        "—"
    } else {
        tb.client.trim()
    };
    let author = if tb.author.trim().is_empty() {
        "—"
    } else {
        tb.author.trim()
    };
    let reg = if tb.registration.trim().is_empty() {
        ""
    } else {
        tb.registration.trim()
    };
    let author_line = if reg.is_empty() {
        author.to_owned()
    } else {
        format!("{author} · {reg}")
    };
    let date = if tb.date.trim().is_empty() {
        "—"
    } else {
        tb.date.trim()
    };
    let sheet_num = if tb.sheet_number.trim().is_empty() {
        "01/01"
    } else {
        tb.sheet_number.trim()
    };
    let rev = if tb.revision.trim().is_empty() {
        "R00"
    } else {
        tb.revision.trim()
    };

    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#627489\">PROJETO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"4.0\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
        x + 2.5,
        y + 3.5,
        x + 2.5,
        y + 9.2,
        escaped(&project.name)
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#627489\">DESENHO / CONTEÚDO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"3.0\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
        x + 2.5,
        y + 15.2,
        x + 2.5,
        y + 19.8,
        escaped(drawing_title)
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">CLIENTE / ENDEREÇO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"#1d2836\">{}</text>",
        x + 2.5,
        y + 25.0,
        x + 2.5,
        y + 29.5,
        escaped(&if tb.address.trim().is_empty() {
            client.to_owned()
        } else {
            format!("{client} · {}", tb.address.trim())
        })
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">RESPONSÁVEL TÉCNICO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"#1d2836\">{}</text>",
        x + 2.5,
        y + 35.0,
        x + 2.5,
        y + 40.0,
        escaped(&author_line)
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">ESCALA</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"3.0\" font-weight=\"700\" fill=\"#1d2836\">{}</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">FORMATO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.8\" fill=\"#1d2836\">{}</text>",
        col1 + 2.0,
        y + 25.0,
        col1 + 2.0,
        y + 29.8,
        escaped(scale_label),
        col1 + 2.0,
        y + 35.0,
        col1 + 2.0,
        y + 40.0,
        escaped(paper_name)
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">DATA</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"#1d2836\">{}</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">REVISÃO</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.6\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
        col2 + 2.0,
        y + 25.0,
        col2 + 2.0,
        y + 29.8,
        escaped(date),
        col2 + 2.0,
        y + 35.0,
        col2 + 2.0,
        y + 40.0,
        escaped(rev)
    );
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.9\" fill=\"#627489\">PRANCHA</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"4.2\" font-weight=\"700\" fill=\"#1d2836\">{}</text>",
        col3 + 2.0,
        y + 26.0,
        col3 + 2.0,
        y + 37.0,
        escaped(sheet_num)
    );
    svg.push_str("</g>\n");
}

fn write_legend(svg: &mut String, x: f64, y: f64) {
    let _ = writeln!(
        svg,
        "<g id=\"legenda\">\n\
         <rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"64\" height=\"20\" fill=\"#ffffff\" stroke=\"#b8c4d0\" stroke-width=\"0.22\"/>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.2\" font-weight=\"700\" fill=\"#1d2836\">LEGENDA</text>\n\
         <rect x=\"{:.2}\" y=\"{:.2}\" width=\"9.5\" height=\"1.8\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.35\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\">Alvenaria</text>\n\
         <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#2878d2\" stroke-width=\"0.6\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\">Esquadrias</text>\n\
         <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#5e8bbf\" stroke-width=\"0.4\" stroke-dasharray=\"2.5 1.0 0.5 1.0\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\">Divisa do terreno</text>\n\
         </g>",
        x + 2.5,
        y + 3.8,
        x + 2.5,
        y + 7.1,
        x + 14.5,
        y + 8.8,
        x + 2.5,
        y + 12.5,
        x + 12.0,
        y + 12.5,
        x + 14.5,
        y + 13.3,
        x + 2.5,
        y + 17.0,
        x + 12.0,
        y + 17.0,
        x + 14.5,
        y + 17.8,
    );
}

fn write_north_symbol(svg: &mut String, cx: f64, cy: f64) {
    let _ = writeln!(
        svg,
        "<g id=\"norte\">\n\
         <circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"5.5\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.28\"/>\n\
         <polygon points=\"{cx:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#1d2836\"/>\n\
         <text x=\"{cx:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"3.0\" font-weight=\"700\" fill=\"#1d2836\" text-anchor=\"middle\">N</text>\n\
         </g>",
        cy - 4.5,
        cx + 2.4,
        cy + 3.0,
        cx - 2.4,
        cy + 3.0,
        cy - 6.8
    );
}

fn write_scale_bar(svg: &mut String, x: f64, y: f64, mm_per_m: f64) {
    // 2 metres on paper divided into 4 segments of 0.5 m.
    let total_m = if mm_per_m >= 20.0 { 2.0 } else { 4.0 };
    let total_mm = total_m * mm_per_m;
    let part = total_mm / 4.0;
    let _ = writeln!(svg, "<g id=\"escala-grafica\">");
    for i in 0..4 {
        let fill = if i % 2 == 0 { "#1d2836" } else { "#ffffff" };
        let _ = writeln!(
            svg,
            "<rect x=\"{:.2}\" y=\"{y:.2}\" width=\"{part:.2}\" height=\"2.0\" fill=\"{fill}\" stroke=\"#1d2836\" stroke-width=\"0.2\"/>",
            x + part * i as f64
        );
    }
    let _ = writeln!(
        svg,
        "<text x=\"{x:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\">0</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\" text-anchor=\"middle\">{:.0} m</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" fill=\"#3b4e63\" text-anchor=\"end\">{:.0} m</text>\n\
         </g>",
        y - 1.0,
        x + total_mm * 0.5,
        y - 1.0,
        total_m * 0.5,
        x + total_mm,
        y - 1.0,
        total_m
    );
}

pub fn export_scaled_svg_for_view(
    project: &Project,
    view: &ExportView,
    path: &Path,
) -> Result<(), String> {
    project.sheet.validate()?;
    fs::write(path, scaled_sheet_svg_for_view(project, view, None))
        .map_err(|error| error.to_string())
}

pub fn export_scaled_pdf(project: &Project, path: &Path) -> Result<(), String> {
    project.sheet.validate()?;
    let markup = scaled_sheet_svg(project);
    super::write_single_svg_pdf(&markup, path)
}

pub fn export_scaled_pdf_for_view(
    project: &Project,
    view: &ExportView,
    path: &Path,
) -> Result<(), String> {
    project.sheet.validate()?;
    let markup = scaled_sheet_svg_for_view(project, view, None);
    super::write_single_svg_pdf(&markup, path)
}

/// Exports multiple open tabs/views of `project` onto separate scaled ABNT
/// sheets inside a single multi-page PDF, automatically numbering each sheet
/// (`01/03`, `02/03`, ...).
pub fn export_scaled_pdf_views(
    project: &Project,
    views: &[ExportView],
    path: &Path,
) -> Result<usize, String> {
    project.sheet.validate()?;
    if views.is_empty() {
        return Err("Nenhuma aba disponível para exportar pranchas em PDF.".to_owned());
    }
    if views.len() == 1 {
        export_scaled_pdf_for_view(project, &views[0], path)?;
        return Ok(1);
    }
    let total = views.len();
    let pages: Vec<String> = views
        .iter()
        .enumerate()
        .map(|(index, view)| scaled_sheet_svg_for_view(project, view, Some((index + 1, total))))
        .collect();
    let pdf = crate::io::report::pages_to_pdf(&pages)?;
    fs::write(path, pdf).map_err(|error| error.to_string())?;
    Ok(pages.len())
}

/// Renders a specific sheet from a `SheetSet`, applying its own layout,
/// pagination (`01/03`), latest revision code, status badge and revision table
/// positioned directly above the title block.
pub fn scaled_sheet_item_svg(
    project: &Project,
    item: &crate::domain::sheet_set::SheetItem,
) -> String {
    // Each sheet shows its own discipline (#96) with the same faded reference
    // underlay used on screen for specialized disciplines.
    let mut themed = project.clone();
    themed.sheet = item.layout.clone();
    themed.sheet.title_block.sheet_number = item.number.clone();
    themed.sheet.title_block.revision = item.latest_revision_code().to_owned();
    if !item.title.trim().is_empty() {
        themed.sheet.title_block.drawing_title = format!(
            "{} · {} ({})",
            item.code,
            item.title.trim(),
            item.status.label()
        );
    }
    let mut svg = if item.discipline.focus().is_some() {
        let underlay = themed.for_underlay(item.discipline, &[]);
        let overlay = themed.for_view(item.discipline, &[]);
        scaled_sheet_svg_with_overlay(&underlay, Some((&overlay, item.discipline)))
    } else {
        let shown = themed.for_view(item.discipline, &[]);
        scaled_sheet_svg_with_overlay(&shown, None)
    };
    if !item.revisions.is_empty()
        && let Some(end_idx) = svg.rfind("</svg>")
    {
        let (frame_x, frame_y, frame_w, frame_h) = themed.sheet.printable_rect_mm();
        let mut rev_block = String::new();
        write_revision_table(&mut rev_block, item, frame_x, frame_y, frame_w, frame_h);
        svg.insert_str(end_idx, &rev_block);
    }
    svg
}

fn write_revision_table(
    svg: &mut String,
    item: &crate::domain::sheet_set::SheetItem,
    frame_x: f64,
    frame_y: f64,
    frame_w: f64,
    frame_h: f64,
) {
    let w = 175.0_f64.min(frame_w);
    let tb_h = if item.layout.title_block.show_title_block {
        44.0_f64.min(frame_h * 0.35)
    } else {
        8.0
    };
    let row_h = 3.6;
    let visible_revs: Vec<_> = item.revisions.iter().rev().take(4).collect();
    let table_h = (visible_revs.len() + 2) as f64 * row_h + 1.0;
    let x = frame_x + frame_w - w;
    let y = (frame_y + frame_h - tb_h - table_h - 1.5).max(frame_y + 2.0);

    let _ = writeln!(svg, "<g id=\"quadro-revisoes\">");
    let _ = writeln!(
        svg,
        "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{table_h:.2}\" fill=\"#ffffff\" stroke=\"#1d2836\" stroke-width=\"0.24\"/>\n\
         <rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{row_h:.2}\" fill=\"#edf2f7\" stroke=\"#1d2836\" stroke-width=\"0.20\"/>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.0\" font-weight=\"700\" fill=\"#1d2836\">CONTROLE DE REVISÕES — {} ({})</text>",
        x + 2.0,
        y + 2.6,
        escaped(&item.code),
        escaped(item.status.label())
    );
    let mut cy = y + row_h;
    let _ = writeln!(
        svg,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#3b4e63\">REV</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#3b4e63\">DATA</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#3b4e63\">RESP.</text>\n\
         <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#3b4e63\">DESCRIÇÃO</text>",
        x + 2.0,
        cy + 2.5,
        x + 16.0,
        cy + 2.5,
        x + 40.0,
        cy + 2.5,
        x + 76.0,
        cy + 2.5
    );
    cy += row_h;
    for rev in visible_revs {
        let _ = writeln!(
            svg,
            "<line x1=\"{x:.2}\" y1=\"{cy:.2}\" x2=\"{:.2}\" y2=\"{cy:.2}\" stroke=\"#cbd5e1\" stroke-width=\"0.14\"/>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" font-weight=\"700\" fill=\"#1d2836\">{}</text>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" fill=\"#1d2836\">{}</text>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" fill=\"#1d2836\">{}</text>\n\
             <text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" fill=\"#1d2836\">{}</text>",
            x + w,
            x + 2.0,
            cy + 2.5,
            escaped(&rev.code),
            x + 16.0,
            cy + 2.5,
            escaped(&rev.date),
            x + 40.0,
            cy + 2.5,
            escaped(&rev.author),
            x + 76.0,
            cy + 2.5,
            escaped(&rev.description)
        );
        cy += row_h;
    }
    svg.push_str("</g>\n");
}

pub fn export_sheet_item_svg(
    project: &Project,
    item: &crate::domain::sheet_set::SheetItem,
    path: &Path,
) -> Result<(), String> {
    item.layout.validate()?;
    fs::write(path, scaled_sheet_item_svg(project, item)).map_err(|error| error.to_string())
}

pub fn export_sheet_item_pdf(
    project: &Project,
    item: &crate::domain::sheet_set::SheetItem,
    path: &Path,
) -> Result<(), String> {
    item.layout.validate()?;
    let markup = scaled_sheet_item_svg(project, item);
    let options = super::svg2pdf_options();
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
        // 4.00 m at 1:50 = 80.00 mm (+ 2 × half wall thickness end caps),
        // rendered as a hollow architectural double-line wall.
        let expected_h = project.masonry.thickness() * 20.0;
        let expected_w = 80.0 + expected_h;
        assert!(markup.contains(&format!(
            "width=\"{expected_w:.2}\" height=\"{expected_h:.2}\""
        )));
        assert!(markup.contains("fill=\"#ffffff\""));
        assert!(markup.contains("1:50"));
        assert!(markup.contains("id=\"carimbo\""));
        assert!(markup.contains("id=\"legenda\""));
        assert!(markup.contains("id=\"norte\""));
        assert!(markup.contains("id=\"escala-grafica\""));
        assert!(markup.contains("id=\"quadros\""));
        assert!(markup.contains("Quadro de Áreas"));
        assert!(markup.contains("01/01"));
    }

    #[test]
    fn doc_style_presets_apply_monochrome_colors_and_metre_units() {
        use crate::domain::doc_style::{DimensionTick, DimensionUnit, DocStyle, DocStylePreset};

        let mut project = Project::default();
        project.doc_style = DocStyle::from_preset(DocStylePreset::ObraMonocromatico);
        project.doc_style.unit = DimensionUnit::Metres;
        project.doc_style.tick = DimensionTick::Dot;
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let markup = scaled_sheet_svg(&project);
        assert!(markup.contains("fill=\"#000000\""));
        assert!(markup.contains("fill=\"#ffffff\""));
        assert!(markup.contains("4,00 m"));
        assert!(markup.contains("<circle "));
    }

    #[test]
    fn sheet_set_item_renders_revision_control_table_and_pagination() {
        use crate::domain::sheet_set::SheetContent;

        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let base = project.sheet.clone();
        let s1 = project
            .sheet_set
            .add_sheet("Planta Baixa", SheetContent::FloorPlan, &base);
        let _s2 = project
            .sheet_set
            .add_sheet("Corte AA", SheetContent::Section { id: 1 }, &base);
        project
            .sheet_set
            .add_revision(s1, "19/11/2026", "Arq. Ana", "Revisão de cotas")
            .unwrap();

        let item = project.sheet_set.sheets[0].clone();
        let markup = scaled_sheet_item_svg(&project, &item);
        assert!(markup.contains("id=\"quadro-revisoes\""));
        assert!(markup.contains("01/02"));
        assert!(markup.contains("R01"));
        assert!(markup.contains("Revisão de cotas"));
    }

    #[test]
    fn cropped_sheet_includes_warning_banner_and_fitting_sheet_omits_it() {
        let mut project = Project::default();
        project.sheet = SheetLayout::standard(PaperSize::A4, Orientation::Portrait);
        project.sheet.scale = DrawingScale::S1_25;
        // 10 m × 8 m at 1:25 needs 400 × 320 mm — overflows A4 portrait.
        project
            .add_room(Point::new(0.0, 0.0), Point::new(10.0, 8.0), "Salão")
            .unwrap();
        let cropped = scaled_sheet_svg(&project);
        assert!(cropped.contains("id=\"aviso-recorte\""));
        assert!(cropped.contains("data-cropped=\"true\""));
        assert!(cropped.contains("AVISO: planta recortada"));

        // At 1:100 on A3 landscape (420 × 297 mm) it fits comfortably.
        project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        project.sheet.scale = DrawingScale::S1_100;
        let fitting = scaled_sheet_svg(&project);
        assert!(!fitting.contains("id=\"aviso-recorte\""));
    }

    #[test]
    fn scaled_pdf_views_paginates_each_discipline_tab_separately() {
        let mut project = Project::default();
        project.name = "Residência".to_owned();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        project.sheet.scale = DrawingScale::S1_100;
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();

        let views = vec![
            ExportView::for_discipline(&project, Discipline::All),
            ExportView::for_discipline(&project, Discipline::Structure),
            ExportView::for_discipline(&project, Discipline::Foundations),
        ];
        let s1 = scaled_sheet_svg_for_view(&project, &views[0], Some((1, 3)));
        let s2 = scaled_sheet_svg_for_view(&project, &views[1], Some((2, 3)));
        let s3 = scaled_sheet_svg_for_view(&project, &views[2], Some((3, 3)));
        assert!(s1.contains("01/03"));
        assert!(s1.contains("Planta Baixa"));
        assert!(!s1.contains("data-contrast-veil=\"true\""));
        assert!(s2.contains("02/03"));
        assert!(s2.contains("Planta de Forma"));
        assert!(s2.contains("data-contrast-veil=\"true\""));
        assert!(s3.contains("03/03"));
        assert!(s3.contains("Planta de Fundações"));
        assert!(s3.contains("data-contrast-veil=\"true\""));

        let dir = std::env::temp_dir().join(format!("trazo-sheets-pdf-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let pdf_path = dir.join("pranchas-abas.pdf");
        let pages = export_scaled_pdf_views(&project, &views, &pdf_path).unwrap();
        assert_eq!(pages, 3);
        let bytes = fs::read(&pdf_path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(String::from_utf8_lossy(&bytes).contains("/Count 3"));
        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_dir(dir);
    }
}
