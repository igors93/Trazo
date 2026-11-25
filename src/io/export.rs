use std::{
    borrow::Cow,
    fmt::Write as _,
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};

use crate::domain::{
    annotations::{building_dimensions, centimetres},
    discipline::Discipline,
    fixtures::{Fixture, FixtureKind, SymbolShape, Weight},
    geometry::{Point, WALL_THICKNESS_METERS, format_meters},
    layers::LayerCategory,
    model::{OpeningKind, Project},
};

/// Electrical devices and their tags.
pub(crate) const ELECTRICAL_INK: &str = "#c62828";
pub(crate) const STRUCTURE_INK: &str = "#00695c";

/// Opacity of the white contrast veil placed over the architectural underlay in
/// specialized discipline exports, matching the on-screen canvas contrast.
pub(crate) const CONTRAST_VEIL_OPACITY: f64 = 0.72;

mod conduits;
mod csv;
mod drawings;
mod labels;
pub mod sheet;

pub use sheet::{export_scaled_pdf_for_view, export_scaled_pdf_views, export_scaled_svg_for_view};
mod markups;

/// Visual configuration of one open view/tab to export, matching how Trazo
/// displays synchronized workspace tabs on screen (discipline visibility,
/// contrast underlay for focused disciplines, floor selection, hidden layers
/// and title).
#[derive(Clone, Debug)]
pub struct ExportView {
    pub title: String,
    pub discipline: Discipline,
    pub floor_id: Option<u64>,
    pub hidden_layers: Vec<u64>,
}

impl ExportView {
    pub fn for_discipline(project: &Project, discipline: Discipline) -> Self {
        let base = project.name.trim();
        let title = if base.is_empty() {
            discipline.label().to_owned()
        } else if discipline.is_all() {
            base.to_owned()
        } else {
            format!("{base} · {}", discipline.label())
        };
        Self {
            title,
            discipline,
            floor_id: project.floors.active_floor_id,
            hidden_layers: Vec::new(),
        }
    }
}

/// Resolves `project` onto `floor_id` (if specified and different from the
/// active floor), avoiding cloning when the active floor already matches.
pub(crate) fn project_on_floor(project: &Project, floor_id: Option<u64>) -> Cow<'_, Project> {
    if let Some(target_floor) = floor_id
        && project.floors.active_floor_id != Some(target_floor)
        && project
            .floors
            .levels
            .iter()
            .any(|level| level.id == target_floor)
    {
        let mut switched = project.clone();
        let _ = switched.switch_floor(target_floor);
        return Cow::Owned(switched);
    }
    Cow::Borrowed(project)
}

/// Whether `fixture` belongs to the crisp foreground overlay of a focused
/// `discipline` tab, matching `canvas::paint_discipline_overlay`.
pub(crate) fn fixture_in_discipline_overlay(discipline: Discipline, fixture: &Fixture) -> bool {
    if discipline.focus().is_none() {
        return false;
    }
    discipline.allows_fixture(fixture.kind)
}

const PIXELS_PER_METER: f64 = 100.0;
const MARGIN: f64 = 130.0;

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

fn write_fixture_svg(
    svg: &mut String,
    fixture: &Fixture,
    point: &dyn Fn(Point) -> (f64, f64),
    pixels_per_meter: f64,
) {
    let electrical = fixture.kind.is_electrical();
    let interior = !electrical && !fixture.kind.is_structural();
    let corner = |x: f64, y: f64| {
        let (px, py) = point(fixture.to_world(Point::new(x, y)));
        format!("{px:.1},{py:.1}")
    };
    let (hw, hd) = (fixture.width / 2.0, fixture.depth / 2.0);
    let transparent_backing = electrical
        || matches!(
            fixture.kind,
            FixtureKind::Footing
                | FixtureKind::PileCap
                | FixtureKind::WaterTank
                | FixtureKind::HeavyEquipment
        );
    if !transparent_backing && !interior {
        let _ = writeln!(
            svg,
            "<polygon points=\"{} {} {} {}\" fill=\"#ffffff\"/>",
            corner(-hw, -hd),
            corner(hw, -hd),
            corner(hw, hd),
            corner(-hw, hd)
        );
    }
    for shape in fixture.symbol() {
        let weight = match &shape {
            SymbolShape::Polyline { weight, .. } | SymbolShape::Circle { weight, .. } => *weight,
        };
        let (color, width) = match (weight, electrical) {
            (Weight::Outline, false) => ("#3b4e63", 2.0),
            (Weight::Detail, false) => ("#627489", 1.2),
            (Weight::Fine, false) => ("#84909e", 0.7),
            (Weight::Outline, true) => (ELECTRICAL_INK, 1.8),
            (Weight::Detail, true) => (ELECTRICAL_INK, 1.0),
            (Weight::Fine, true) => (ELECTRICAL_INK, 0.7),
        };
        let (color, width) = match (weight, fixture.kind.is_structural()) {
            (Weight::Outline, true) => (STRUCTURE_INK, 1.8),
            (Weight::Detail, true) => (STRUCTURE_INK, 0.8),
            (Weight::Fine, true) => (STRUCTURE_INK, 0.6),
            _ => (color, width),
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
                let fill = if interior && closed && weight == Weight::Outline {
                    "#ffffff"
                } else {
                    "none"
                };
                let dash = if dashed {
                    " stroke-dasharray=\"6 4\""
                } else {
                    ""
                };
                let _ = writeln!(
                    svg,
                    "<{element} points=\"{points}\" fill=\"{fill}\" stroke=\"{color}\" stroke-width=\"{width}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{dash}/>"
                );
            }
            SymbolShape::Circle { center, radius, .. } => {
                let fill = if interior && weight == Weight::Outline {
                    "#ffffff"
                } else {
                    "none"
                };
                let (cx, cy) = point(fixture.to_world(center));
                let _ = writeln!(
                    svg,
                    "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{:.1}\" fill=\"{fill}\" stroke=\"{color}\" stroke-width=\"{width}\"/>",
                    radius * pixels_per_meter
                );
            }
        }
    }
}

fn write_discipline_overlay_svg(
    svg: &mut String,
    project: &Project,
    discipline: Discipline,
    point: &dyn Fn(Point) -> (f64, f64),
    pixels_per_meter: f64,
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
        write_fixture_svg(svg, fixture, point, pixels_per_meter);
    }
    match discipline {
        Discipline::Structure => {
            if project.category_visible(LayerCategory::Structure) {
                conduits::write_structure_scoped(
                    svg,
                    project,
                    crate::domain::structure::StructureMarksScope::Superstructure,
                    point,
                    pixels_per_meter,
                    (1.4, 1.0, (pixels_per_meter * 0.14).clamp(8.0, 16.0)),
                );
            }
        }
        Discipline::Foundations => {
            if project.category_visible(LayerCategory::Structure) {
                conduits::write_structure_scoped(
                    svg,
                    project,
                    crate::domain::structure::StructureMarksScope::Foundations,
                    point,
                    pixels_per_meter,
                    (1.4, 1.0, (pixels_per_meter * 0.14).clamp(8.0, 16.0)),
                );
            }
        }
        Discipline::Electrical => {
            if project.category_visible(LayerCategory::Electrical) {
                conduits::write_conduits(
                    svg,
                    project,
                    point,
                    pixels_per_meter,
                    (1.6, 1.0, (pixels_per_meter * 0.13).clamp(8.0, 15.0)),
                );
                for (anchor, text) in project.electrical_tags() {
                    let (x, y) = point(anchor);
                    let _ = writeln!(
                        svg,
                        "<text x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{:.1}\" fill=\"{ELECTRICAL_INK}\" stroke=\"#ffffff\" stroke-width=\"3\" paint-order=\"stroke\">{}</text>",
                        (pixels_per_meter * 0.14).clamp(9.0, 16.0),
                        escaped(&text)
                    );
                }
            }
        }
        Discipline::Plumbing | Discipline::All | Discipline::Architecture => {}
    }
    svg.push_str("</g>\n");
}

#[cfg(test)]
fn document(project: &Project) -> SvgDocument {
    document_with_options(project, None, &project.name, None)
}

fn document_for_view(project: &Project, view: &ExportView) -> SvgDocument {
    let base = project_on_floor(project, view.floor_id);
    let title = if view.title.trim().is_empty() {
        project.name.as_str()
    } else {
        view.title.as_str()
    };
    let subtitle = if view.discipline.focus().is_some() {
        Some(view.discipline.sheet_title())
    } else {
        None
    };
    if view.discipline.focus().is_some() {
        let underlay = base.for_underlay(view.discipline, &view.hidden_layers);
        let overlay = base.for_view(view.discipline, &view.hidden_layers);
        document_with_options(
            &underlay,
            Some((&overlay, view.discipline)),
            title,
            subtitle,
        )
    } else {
        let shown = base.for_view(view.discipline, &view.hidden_layers);
        document_with_options(&shown, None, title, subtitle)
    }
}

fn document_with_options(
    project: &Project,
    focused_overlay: Option<(&Project, Discipline)>,
    title: &str,
    subtitle: Option<&str>,
) -> SvgDocument {
    let bounds_source = focused_overlay.map_or(project, |(overlay, _)| overlay);
    let (min, max) = markups::bounds(bounds_source)
        .or_else(|| markups::bounds(project))
        .unwrap_or((Point::new(0.0, 0.0), Point::new(9.0, 6.0)));
    let span_x = (max.x - min.x).max(6.0);
    let span_y = (max.y - min.y).max(4.0);
    let pixels_per_meter = PIXELS_PER_METER.min(5000.0 / span_x).min(5000.0 / span_y);
    let wall_width = (pixels_per_meter * WALL_THICKNESS_METERS).clamp(1.0, 12.0);
    let width = (span_x * pixels_per_meter + MARGIN * 2.0).ceil() as u32;
    let height = (span_y * pixels_per_meter + MARGIN * 2.0).ceil() as u32;
    let point = |p: Point| -> (f64, f64) {
        (
            MARGIN + (p.x - min.x) * pixels_per_meter,
            MARGIN + (p.y - min.y) * pixels_per_meter,
        )
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    if project.category_visible(crate::domain::layers::LayerCategory::Terrain)
        && let Some(terrain) = &project.terrain
    {
        let (x, y) = point(terrain.min);
        let terrain_points = terrain
            .outline()
            .into_iter()
            .map(|corner| {
                let (x, y) = point(corner);
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            svg,
            "<polygon points=\"{terrain_points}\" fill=\"#f8fbfe\" stroke=\"#5e8bbf\" stroke-width=\"2.5\" stroke-dasharray=\"28 8 5 8\"/>"
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
        for lot in &project.lots {
            let points = lot
                .vertices
                .iter()
                .map(|corner| {
                    let (x, y) = point(*corner);
                    format!("{x:.1},{y:.1}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(
                svg,
                "<polygon points=\"{points}\" fill=\"none\" stroke=\"#5e8bbf\" stroke-width=\"1.5\" stroke-dasharray=\"10 6\" data-lot=\"{}\"/>",
                escaped(&lot.name)
            );
        }
        for (a, b) in terrain.setback_segments(project.setbacks) {
            let (x1, y1) = point(a);
            let (x2, y2) = point(b);
            let _ = writeln!(
                svg,
                "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#d5841b\" stroke-width=\"1.5\" stroke-dasharray=\"9 5\" data-building-limit=\"true\"/>"
            );
        }
    }
    for underlay in &project.underlays {
        if !underlay.visible {
            continue;
        }
        let Some(ref b64) = underlay.embedded_png_base64 else {
            continue;
        };
        let (x, y) = point(underlay.origin);
        let w = underlay.width_m * pixels_per_meter;
        let h = underlay.height_m() * pixels_per_meter;
        let opacity = underlay.opacity.clamp(0.05, 1.0);
        if underlay.rotation_deg.abs() > 1e-3 {
            let cx = x + w * 0.5;
            let cy = y + h * 0.5;
            let _ = writeln!(
                svg,
                "<image href=\"data:image/png;base64,{b64}\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" opacity=\"{opacity:.2}\" transform=\"rotate({:.2} {cx:.1} {cy:.1})\" data-underlay=\"{}\"/>",
                underlay.rotation_deg,
                escaped(&underlay.name)
            );
        } else {
            let _ = writeln!(
                svg,
                "<image href=\"data:image/png;base64,{b64}\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" opacity=\"{opacity:.2}\" data-underlay=\"{}\"/>",
                escaped(&underlay.name)
            );
        }
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Rooms) {
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
                let fill = match room.hatch {
                    Some(style) if style.pattern == crate::domain::hatch::HatchPattern::Solid => {
                        style.hex_color()
                    }
                    _ => "#eef5fd".to_owned(),
                };
                let _ = writeln!(svg, "<polygon points=\"{points_str}\" fill=\"{fill}\"/>");
                if let Some(style) = room.hatch {
                    let stroke_color = style.hex_color();
                    for (a, b) in crate::domain::hatch::hatch_segments_in_polygon(&poly, &style) {
                        let (x1, y1) = point(a);
                        let (x2, y2) = point(b);
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{stroke_color}\" stroke-width=\"0.9\" opacity=\"0.75\"/>"
                        );
                    }
                }
            }
        }
    }
    let base_wall_width = wall_width;
    let border_px = (base_wall_width * 0.20).clamp(1.0, 2.2);
    let draw_hollow = base_wall_width >= 2.0 * border_px + 1.0;

    // Collect solid masonry runs per visible wall in local wall coordinates so
    // Pass 1 (dark outer contour), Pass 2 (white hollow core), and Pass 3
    // (openings) stay aligned with clean mitered L/T/X junctions.
    let wall_entries: Vec<_> =
        if project.category_visible(crate::domain::layers::LayerCategory::Walls) {
            project
                .walls
                .iter()
                .filter(|wall| project.is_wall_visible(wall))
                .map(|wall| {
                    let wall_w = (pixels_per_meter * wall.thickness()).max(1.0);
                    let half_w = wall_w * 0.5;
                    let (x, y) = point(wall.start);
                    let length = wall.length() * pixels_per_meter;
                    let angle = (wall.end.y - wall.start.y)
                        .atan2(wall.end.x - wall.start.x)
                        .to_degrees();
                    let mut openings: Vec<_> = project
                        .openings
                        .iter()
                        .filter(|opening| opening.wall_id == wall.id)
                        .collect();
                    openings.sort_by(|a, b| a.t.total_cmp(&b.t));
                    let mut runs = Vec::with_capacity(openings.len() + 1);
                    let mut cursor = 0.0;
                    let mut run_start = -half_w;
                    for opening in &openings {
                        let half = opening.width() * pixels_per_meter / 2.0;
                        let center = opening.t * length;
                        let left = center - half;
                        let right = center + half;
                        if left > cursor {
                            runs.push((run_start, left));
                        }
                        cursor = right;
                        run_start = right;
                    }
                    if cursor < length {
                        runs.push((run_start, length + half_w));
                    }
                    (wall, wall_w, half_w, x, y, length, angle, openings, runs)
                })
                .collect()
        } else {
            Vec::new()
        };

    // Pass 1: outer wall contours (junctions + wall runs).
    if project.category_visible(crate::domain::layers::LayerCategory::Walls) {
        for junction in project.wall_junctions() {
            let points = junction
                .outline
                .into_iter()
                .map(|corner| {
                    let (x, y) = point(corner);
                    format!("{x:.1},{y:.1}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(
                svg,
                "<polygon points=\"{points}\" fill=\"#1d2836\" data-wall-junction=\"{:?}\"/>",
                junction.kind
            );
        }
    }
    for &(_, wall_w, half_w, x, y, _, angle, _, ref runs) in &wall_entries {
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.1} {y:.1}) rotate({angle:.3})\">"
        );
        for &(x0, x1) in runs {
            let w = (x1 - x0).max(0.0);
            if w > 0.05 {
                let _ = writeln!(
                    svg,
                    "<rect x=\"{x0:.1}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"{wall_w:.1}\" fill=\"#1d2836\"/>",
                    -half_w
                );
            }
        }
        svg.push_str("</g>\n");
    }

    // Pass 2: inner hollow white core (inset junctions + inset runs) and
    // multilayer wall assemblies.
    if draw_hollow && project.category_visible(crate::domain::layers::LayerCategory::Walls) {
        let border_m = border_px / pixels_per_meter;
        for junction in project.wall_junctions_with_inset(border_m) {
            let points = junction
                .outline
                .into_iter()
                .map(|corner| {
                    let (x, y) = point(corner);
                    format!("{x:.1},{y:.1}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(svg, "<polygon points=\"{points}\" fill=\"#ffffff\"/>");
        }
    }
    for &(wall, wall_w, half_w, x, y, _, angle, _, ref runs) in &wall_entries {
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.1} {y:.1}) rotate({angle:.3})\">"
        );
        if let Some(assembly) = &wall.assembly
            && wall_w >= 6.0
            && assembly.total_thickness() > 1e-6
        {
            for &(x0, x1) in runs {
                let w = (x1 - x0).max(0.0);
                if w > 0.05 {
                    for (s0, s1, layer) in assembly.layer_offsets() {
                        let y0 = s0 * pixels_per_meter;
                        let h = ((s1 - s0) * pixels_per_meter).max(0.5);
                        let _ = writeln!(
                            svg,
                            "<rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"{}\" stroke=\"#1d2836\" stroke-width=\"0.4\"/>",
                            layer.hex_color()
                        );
                    }
                }
            }
        } else {
            let inner_h = wall_w - 2.0 * border_px;
            if inner_h >= 1.0 {
                for &(x0, x1) in runs {
                    let ix0 = x0 + border_px;
                    let ix1 = x1 - border_px;
                    if ix1 - ix0 > 0.5 {
                        let _ = writeln!(
                            svg,
                            "<rect x=\"{ix0:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{inner_h:.1}\" fill=\"#ffffff\"/>",
                            -(half_w - border_px),
                            ix1 - ix0
                        );
                    }
                }
            }
        }
        svg.push_str("</g>\n");
    }

    // Pass 3: openings (jambs, doors, windows, passages) on top of walls.
    for &(_, wall_w, half_w, x, y, length, angle, ref openings, _) in &wall_entries {
        if openings.is_empty() {
            continue;
        }
        let wall_width = wall_w;
        let _ = writeln!(
            svg,
            "<g transform=\"translate({x:.1} {y:.1}) rotate({angle:.3})\">"
        );
        for opening in openings {
            let half = opening.width() * pixels_per_meter / 2.0;
            let center = opening.t * length;
            let left = center - half;
            let right = center + half;
            for jamb_x in [left, right] {
                let _ = writeln!(
                    svg,
                    "<line x1=\"{jamb_x:.1}\" y1=\"{:.1}\" x2=\"{jamb_x:.1}\" y2=\"{half_w:.1}\" stroke=\"#1d2836\" stroke-width=\"{border_px:.1}\"/>",
                    -half_w
                );
            }
            match opening.kind {
                OpeningKind::Door | OpeningKind::Gate => {
                    use crate::domain::openings::DoorType;
                    let leaf = half * 2.0;
                    let side = if opening.flip_side { -1.0 } else { 1.0 };
                    let (hinge, along) = if opening.flip_hinge {
                        (right, -1.0)
                    } else {
                        (left, 1.0)
                    };
                    match opening.effective_door_type() {
                        DoorType::Swing => {
                            let arc = (0..=20)
                                .map(|step| {
                                    let theta = std::f64::consts::FRAC_PI_2 * step as f64 / 20.0;
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
                        DoorType::DoubleSwing => {
                            for (h, dir_h) in [(left, 1.0), (right, -1.0)] {
                                let arc = (0..=14)
                                    .map(|step| {
                                        let theta =
                                            std::f64::consts::FRAC_PI_2 * step as f64 / 14.0;
                                        format!(
                                            "{:.1},{:.1}",
                                            h + dir_h * theta.sin() * half,
                                            side * theta.cos() * half
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let _ = writeln!(
                                    svg,
                                    "<line x1=\"{h:.1}\" y1=\"0\" x2=\"{h:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"2.6\"/><polyline points=\"{arc}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.3\"/>",
                                    side * half
                                );
                            }
                        }
                        DoorType::Sliding => {
                            let y_track = side * (wall_width * 0.5 + 3.0);
                            let y_leaf = side * (wall_width * 0.5 + 6.0);
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.1}\" y1=\"{y_track:.1}\" x2=\"{right:.1}\" y2=\"{y_track:.1}\" stroke=\"#8ea2b8\" stroke-width=\"1.4\"/><line x1=\"{hinge:.1}\" y1=\"{y_leaf:.1}\" x2=\"{:.1}\" y2=\"{y_leaf:.1}\" stroke=\"#1d2836\" stroke-width=\"3.2\"/>",
                                hinge + along * leaf
                            );
                        }
                        DoorType::PocketSliding => {
                            let pocket = hinge - along * leaf;
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{hinge:.1}\" y1=\"0\" x2=\"{pocket:.1}\" y2=\"0\" stroke=\"#8ea2b8\" stroke-width=\"2.0\" stroke-dasharray=\"5 4\"/><line x1=\"{hinge:.1}\" y1=\"0\" x2=\"{:.1}\" y2=\"0\" stroke=\"#1d2836\" stroke-width=\"3.2\"/>",
                                hinge + along * (leaf * 0.65)
                            );
                        }
                        DoorType::Pivot => {
                            let pivot = hinge + along * (leaf * 0.20);
                            let short_r = leaf * 0.20;
                            let long_r = leaf * 0.80;
                            let big_arc = (0..=16)
                                .map(|s| {
                                    let th = std::f64::consts::FRAC_PI_2 * s as f64 / 16.0;
                                    format!(
                                        "{:.1},{:.1}",
                                        pivot + along * th.sin() * long_r,
                                        side * th.cos() * long_r
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let _ = writeln!(
                                svg,
                                "<circle cx=\"{pivot:.1}\" cy=\"0\" r=\"3\" fill=\"#1d2836\"/><line x1=\"{pivot:.1}\" y1=\"{:.1}\" x2=\"{pivot:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"3\"/><polyline points=\"{big_arc}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.4\"/>",
                                -side * short_r,
                                side * long_r
                            );
                        }
                        DoorType::Bifold => {
                            let step = leaf * 0.72 / 4.0;
                            let peak = half * 0.55;
                            let pts: String = (0..=4)
                                .map(|i| {
                                    let y = if i % 2 == 1 { side * peak } else { 0.0 };
                                    format!("{:.1},{y:.1}", hinge + along * step * i as f64)
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{pts}\" fill=\"none\" stroke=\"#1d2836\" stroke-width=\"2.4\"/><line x1=\"{left:.1}\" y1=\"0\" x2=\"{right:.1}\" y2=\"0\" stroke=\"#8ea2b8\" stroke-width=\"1.2\" stroke-dasharray=\"5 4\"/>"
                            );
                        }
                        DoorType::Revolving => {
                            let d = half * std::f64::consts::FRAC_1_SQRT_2;
                            let _ = writeln!(
                                svg,
                                "<circle cx=\"{center:.1}\" cy=\"0\" r=\"{half:.1}\" fill=\"none\" stroke=\"#8ea2b8\" stroke-width=\"1.4\"/><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"2.4\"/><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#1d2836\" stroke-width=\"2.4\"/>",
                                center - d,
                                -d,
                                center + d,
                                d,
                                center - d,
                                d,
                                center + d,
                                -d
                            );
                        }
                    }
                }
                OpeningKind::SaloonDoor => {
                    // Two half leaves swinging both ways (dashed on the far side).
                    let quarter = |hinge: f64, toward: f64, side: f64| {
                        (0..=12)
                            .map(|step| {
                                let theta = std::f64::consts::FRAC_PI_2 * step as f64 / 12.0;
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
                    use crate::domain::openings::WindowType;
                    let side = if opening.flip_side { -1.0 } else { 1.0 };
                    let off = (half_w - 0.5).max(2.5);
                    let _ = writeln!(
                        svg,
                        "<rect x=\"{left:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#2878d2\" fill-opacity=\"0.10\"/><line x1=\"{left:.1}\" y1=\"{:.1}\" x2=\"{right:.1}\" y2=\"{:.1}\" stroke=\"#2878d2\" stroke-width=\"1.8\"/><line x1=\"{left:.1}\" y1=\"{off:.1}\" x2=\"{right:.1}\" y2=\"{off:.1}\" stroke=\"#2878d2\" stroke-width=\"1.8\"/>",
                        -off,
                        right - left,
                        off * 2.0,
                        -off,
                        -off
                    );
                    match opening.effective_window_type() {
                        WindowType::Sliding => {
                            let ov = (half * 0.14).min(6.0);
                            let inner_off = (off * 0.35).max(1.2);
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#2878d2\" stroke-width=\"1.5\"/><line x1=\"{:.1}\" y1=\"{inner_off:.1}\" x2=\"{right:.1}\" y2=\"{inner_off:.1}\" stroke=\"#2878d2\" stroke-width=\"1.5\"/>",
                                -inner_off,
                                center + ov,
                                -inner_off,
                                center - ov
                            );
                        }
                        WindowType::Basculante => {
                            let py = side * 9.0;
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.1}\" y1=\"{py:.1}\" x2=\"{right:.1}\" y2=\"{py:.1}\" stroke=\"#2878d2\" stroke-width=\"1.3\" stroke-dasharray=\"5 3\"/><circle cx=\"{center:.1}\" cy=\"0\" r=\"2.2\" fill=\"#2878d2\"/>"
                            );
                        }
                        WindowType::MaximAr => {
                            let py = side * 12.0;
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{left:.1},0 {center:.1},{py:.1} {right:.1},0\" fill=\"none\" stroke=\"#2878d2\" stroke-width=\"1.4\" stroke-dasharray=\"5 3\"/>"
                            );
                        }
                        WindowType::Casement | WindowType::TiltTurn => {
                            let r = half * 0.75;
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.1}\" y1=\"0\" x2=\"{left:.1}\" y2=\"{:.1}\" stroke=\"#2878d2\" stroke-width=\"1.8\"/><line x1=\"{right:.1}\" y1=\"0\" x2=\"{right:.1}\" y2=\"{:.1}\" stroke=\"#2878d2\" stroke-width=\"1.8\"/>",
                                side * r,
                                side * r
                            );
                        }
                        WindowType::Fixed => {
                            let _ = writeln!(
                                svg,
                                "<line x1=\"{left:.1}\" y1=\"0\" x2=\"{right:.1}\" y2=\"0\" stroke=\"#2878d2\" stroke-width=\"1.8\"/><line x1=\"{:.1}\" y1=\"-3\" x2=\"{:.1}\" y2=\"3\" stroke=\"#2878d2\" stroke-width=\"1.3\"/><line x1=\"{:.1}\" y1=\"3\" x2=\"{:.1}\" y2=\"-3\" stroke=\"#2878d2\" stroke-width=\"1.3\"/>",
                                center - 3.0,
                                center + 3.0,
                                center - 3.0,
                                center + 3.0
                            );
                        }
                        WindowType::BayWindow => {
                            let py = side * 16.0;
                            let inset = half * 0.35;
                            let _ = writeln!(
                                svg,
                                "<polyline points=\"{left:.1},0 {:.1},{py:.1} {:.1},{py:.1} {right:.1},0\" fill=\"none\" stroke=\"#2878d2\" stroke-width=\"2.2\"/>",
                                left + inset,
                                right - inset
                            );
                        }
                    }
                }
            }
        }
        svg.push_str("</g>\n");
    }
    for fixture in &project.fixtures {
        if !project.category_visible(Project::fixture_category(fixture)) {
            continue;
        }
        write_fixture_svg(&mut svg, fixture, &point, pixels_per_meter);
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Electrical) {
        conduits::write_conduits(
            &mut svg,
            project,
            &point,
            pixels_per_meter,
            (1.6, 1.0, (pixels_per_meter * 0.13).clamp(8.0, 15.0)),
        );
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Structure) {
        conduits::write_structure(
            &mut svg,
            project,
            &point,
            pixels_per_meter,
            (1.4, 1.0, (pixels_per_meter * 0.14).clamp(8.0, 16.0)),
        );
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Electrical) {
        for (anchor, text) in project.electrical_tags() {
            let (x, y) = point(anchor);
            let _ = writeln!(
                svg,
                "<text x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{:.1}\" fill=\"{ELECTRICAL_INK}\" stroke=\"#ffffff\" stroke-width=\"3\" paint-order=\"stroke\">{}</text>",
                (pixels_per_meter * 0.14).clamp(9.0, 16.0),
                escaped(&text)
            );
        }
    }
    // Dimension chains in centimetres above and left of the building.
    if let Some(dims) = building_dimensions(project) {
        let (left, top) = point(dims.min);
        let x_of = |x: f64| point(Point::new(x, dims.min.y)).0;
        let y_of = |y: f64| point(Point::new(dims.min.x, y)).1;
        let style = "stroke=\"#627489\" stroke-width=\"1\"";
        let text = "font-family=\"Arial, sans-serif\" font-size=\"12\" fill=\"#3b4e63\" text-anchor=\"middle\"";
        for (row, stations) in [
            (top - 30.0, dims.xs.clone()),
            (top - 58.0, vec![dims.min.x, dims.max.x]),
        ] {
            let xs: Vec<f64> = stations.iter().map(|x| x_of(*x)).collect();
            let _ = writeln!(
                svg,
                "<line x1=\"{:.1}\" y1=\"{row:.1}\" x2=\"{:.1}\" y2=\"{row:.1}\" {style}/>",
                xs[0],
                xs[xs.len() - 1]
            );
            for x in &xs {
                let _ = writeln!(
                    svg,
                    "<line x1=\"{x:.1}\" y1=\"{:.1}\" x2=\"{x:.1}\" y2=\"{:.1}\" {style}/><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#3b4e63\" stroke-width=\"1.5\"/>",
                    row - 6.0,
                    row + 10.0,
                    x - 4.0,
                    row + 4.0,
                    x + 4.0,
                    row - 4.0
                );
            }
            for (pair, span) in stations.windows(2).zip(xs.windows(2)) {
                let value = centimetres(pair[1] - pair[0]);
                if value.len() as f64 * 7.0 + 4.0 <= span[1] - span[0] {
                    let _ = writeln!(
                        svg,
                        "<text x=\"{:.1}\" y=\"{:.1}\" {text}>{value}</text>",
                        (span[0] + span[1]) / 2.0,
                        row - 4.0
                    );
                }
            }
        }
        for (column, stations) in [
            (left - 30.0, dims.ys.clone()),
            (left - 58.0, vec![dims.min.y, dims.max.y]),
        ] {
            let ys: Vec<f64> = stations.iter().map(|y| y_of(*y)).collect();
            let _ = writeln!(
                svg,
                "<line x1=\"{column:.1}\" y1=\"{:.1}\" x2=\"{column:.1}\" y2=\"{:.1}\" {style}/>",
                ys[0],
                ys[ys.len() - 1]
            );
            for y in &ys {
                let _ = writeln!(
                    svg,
                    "<line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" {style}/><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#3b4e63\" stroke-width=\"1.5\"/>",
                    column - 6.0,
                    column + 10.0,
                    column - 4.0,
                    y + 4.0,
                    column + 4.0,
                    y - 4.0
                );
            }
            for (pair, span) in stations.windows(2).zip(ys.windows(2)) {
                let value = centimetres(pair[1] - pair[0]);
                if value.len() as f64 * 7.0 + 4.0 <= span[1] - span[0] {
                    let (x, y) = (column - 4.0, (span[0] + span[1]) / 2.0);
                    let _ = writeln!(
                        svg,
                        "<text x=\"{x:.1}\" y=\"{y:.1}\" transform=\"rotate(-90 {x:.1} {y:.1})\" {text}>{value}</text>"
                    );
                }
            }
        }
    }

    drawings::write_drawings(&mut svg, project, &point);

    // Stairs, ramps, landings, railings and level transitions.
    if !project.circulations.is_empty() {
        let _ = writeln!(svg, "<g id=\"circulacao-vertical\">");
        for elem in &project.circulations {
            let geom = elem.plan_geometry();
            for poly in &geom.polygons {
                let pts: String = poly
                    .iter()
                    .map(|p| {
                        let (x, y) = point(*p);
                        format!("{x:.1},{y:.1}")
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = writeln!(
                    svg,
                    "<polygon points=\"{pts}\" fill=\"#f1f5f9\" stroke=\"#1d2836\" stroke-width=\"1.5\"/>"
                );
            }
            for (a, b) in &geom.lines {
                let (x1, y1) = point(*a);
                let (x2, y2) = point(*b);
                let _ = writeln!(
                    svg,
                    "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#3b4e63\" stroke-width=\"1.0\"/>"
                );
            }
            for (a, b) in &geom.railing_lines {
                let (x1, y1) = point(*a);
                let (x2, y2) = point(*b);
                let _ = writeln!(
                    svg,
                    "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#1d2836\" stroke-width=\"2.2\"/>"
                );
            }
            if let Some((a, b)) = geom.arrow {
                let (x1, y1) = point(a);
                let (x2, y2) = point(b);
                let _ = writeln!(
                    svg,
                    "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#2878d2\" stroke-width=\"1.5\"/>"
                );
            }
            if !geom.label.is_empty() {
                let (lx, ly) = point(geom.label_pos);
                let _ = writeln!(
                    svg,
                    "<text x=\"{lx:.1}\" y=\"{ly:.1}\" font-family=\"Arial, sans-serif\" font-size=\"10\" font-weight=\"700\" fill=\"#1d2836\" text-anchor=\"middle\">{}</text>",
                    escaped(&geom.label)
                );
            }
        }
        svg.push_str("</g>\n");
    }

    if project.category_visible(crate::domain::layers::LayerCategory::Dimensions) {
        for dim in &project.dimensions {
            if !dim.visible {
                continue;
            }
            let Some(resolved) = project.resolve_dimension(dim) else {
                continue;
            };
            match &resolved {
                crate::domain::dimensions::ResolvedDimension::Linear {
                    p1,
                    p2,
                    dim_p1,
                    dim_p2,
                    label,
                    ..
                } => {
                    let (x1, y1) = point(*p1);
                    let (x2, y2) = point(*p2);
                    let (dx1, dy1) = point(*dim_p1);
                    let (dx2, dy2) = point(*dim_p2);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{dx1:.1}\" y2=\"{dy1:.1}\" stroke=\"#627489\" stroke-width=\"0.9\"/><line x1=\"{x2:.1}\" y1=\"{y2:.1}\" x2=\"{dx2:.1}\" y2=\"{dy2:.1}\" stroke=\"#627489\" stroke-width=\"0.9\"/><line x1=\"{dx1:.1}\" y1=\"{dy1:.1}\" x2=\"{dx2:.1}\" y2=\"{dy2:.1}\" stroke=\"#3b4e63\" stroke-width=\"1.1\"/>"
                    );
                    for (tx, ty) in [(dx1, dy1), (dx2, dy2)] {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#3b4e63\" stroke-width=\"1.5\"/>",
                            tx - 4.0,
                            ty + 4.0,
                            tx + 4.0,
                            ty - 4.0
                        );
                    }
                    let (mx, my) = ((dx1 + dx2) * 0.5, (dy1 + dy2) * 0.5);
                    let angle_deg = (dy2 - dy1).atan2(dx2 - dx1).to_degrees();
                    let mut text_angle = angle_deg;
                    if text_angle > 90.0 {
                        text_angle -= 180.0;
                    } else if text_angle <= -90.0 {
                        text_angle += 180.0;
                    }
                    let rad = text_angle.to_radians();
                    // Shift text slightly above the dimension line in its local reading frame.
                    let tx = mx + 5.0 * rad.sin();
                    let ty = my - 5.0 * rad.cos();
                    let transform = if text_angle.abs() > 0.5 {
                        format!(" transform=\"rotate({text_angle:.1} {tx:.1} {ty:.1})\"")
                    } else {
                        String::new()
                    };
                    let _ = writeln!(
                        svg,
                        "<text x=\"{tx:.1}\" y=\"{ty:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" fill=\"#1c2b3c\" text-anchor=\"middle\"{transform}>{}</text>",
                        escaped(label)
                    );
                }
                crate::domain::dimensions::ResolvedDimension::Angular {
                    vertex,
                    start_angle,
                    sweep,
                    radius,
                    label,
                    ..
                } => {
                    let pts: String = resolved
                        .arc_points(20)
                        .into_iter()
                        .map(|p| {
                            let (x, y) = point(p);
                            format!("{x:.1},{y:.1}")
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(
                        svg,
                        "<polyline points=\"{pts}\" fill=\"none\" stroke=\"#3b4e63\" stroke-width=\"1.1\"/>"
                    );
                    let mid = crate::domain::drawing::point_at_angle(
                        *vertex,
                        radius + 0.18,
                        start_angle + sweep * 0.5,
                    );
                    let (mx, my) = point(mid);
                    let _ = writeln!(
                        svg,
                        "<text x=\"{mx:.1}\" y=\"{my:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" fill=\"#1c2b3c\" text-anchor=\"middle\">{}</text>",
                        escaped(label)
                    );
                }
            }
        }
    }

    if project.category_visible(crate::domain::layers::LayerCategory::Annotations) {
        use crate::domain::notes::{NoteKind, TechnicalSymbol};
        for note in &project.notes {
            let (x, y) = point(note.position);
            let text = escaped(&note.text);
            match note.kind {
                NoteKind::Text => {
                    let _ = writeln!(
                        svg,
                        "<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"Arial, sans-serif\" font-size=\"{:.1}\" fill=\"#1c2b3c\">{text}</text>",
                        note.font_size
                    );
                }
                NoteKind::Note => {
                    let w = (note.text.chars().count() as f64 * 7.2 + 16.0).max(40.0);
                    let _ = writeln!(
                        svg,
                        "<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"24\" rx=\"3\" fill=\"#fffdf2\" stroke=\"#90a4b8\" stroke-width=\"1\"/><text x=\"{:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"12\" fill=\"#1c2b3c\">{text}</text>",
                        y - 16.0,
                        x + 8.0,
                        y
                    );
                }
                NoteKind::Callout { target } => {
                    let (tx, ty) = point(target);
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x:.1}\" y1=\"{y:.1}\" x2=\"{tx:.1}\" y2=\"{ty:.1}\" stroke=\"#2878d2\" stroke-width=\"1.2\"/><circle cx=\"{tx:.1}\" cy=\"{ty:.1}\" r=\"3\" fill=\"#2878d2\"/><text x=\"{:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"12\" fill=\"#1c2b3c\">{text}</text>",
                        x + 6.0,
                        y - 4.0
                    );
                }
                NoteKind::Symbol(TechnicalSymbol::LevelMark { .. }) => {
                    let _ = writeln!(
                        svg,
                        "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"7\" fill=\"#ffffff\" stroke=\"#1c2b3c\" stroke-width=\"1.2\"/><line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#1c2b3c\" stroke-width=\"1.1\"/><line x1=\"{x:.1}\" y1=\"{:.1}\" x2=\"{x:.1}\" y2=\"{:.1}\" stroke=\"#1c2b3c\" stroke-width=\"1.1\"/><text x=\"{:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" font-weight=\"700\" fill=\"#1c2b3c\">{text}</text>",
                        x - 10.0,
                        x + 10.0,
                        y - 10.0,
                        y + 10.0,
                        x + 12.0,
                        y + 4.0
                    );
                }
                NoteKind::Symbol(TechnicalSymbol::SlopeArrow { .. }) => {
                    let _ = writeln!(
                        svg,
                        "<line x1=\"{x:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#1c2b3c\" stroke-width=\"1.2\"/><polyline points=\"{:.1},{:.1} {:.1},{y:.1} {:.1},{:.1}\" fill=\"none\" stroke=\"#1c2b3c\" stroke-width=\"1.2\"/><text x=\"{:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" fill=\"#3b4e63\" text-anchor=\"middle\">{text}</text>",
                        x + 34.0,
                        x + 28.0,
                        y - 4.0,
                        x + 34.0,
                        x + 28.0,
                        y + 4.0,
                        x + 17.0,
                        y - 6.0
                    );
                }
                NoteKind::Symbol(TechnicalSymbol::NorthArrow) => {
                    let _ = writeln!(
                        svg,
                        "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"11\" fill=\"#ffffff\" stroke=\"#1c2b3c\" stroke-width=\"1.2\"/><polygon points=\"{x:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#1d2836\"/><text x=\"{x:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" font-weight=\"700\" fill=\"#1c2b3c\" text-anchor=\"middle\">{text}</text>",
                        y - 9.0,
                        x + 5.0,
                        y + 6.0,
                        x - 5.0,
                        y + 6.0,
                        y - 14.0
                    );
                }
                NoteKind::Symbol(TechnicalSymbol::EntryArrow) => {
                    let _ = writeln!(
                        svg,
                        "<polygon points=\"{x:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#2878d2\"/><text x=\"{x:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"10\" font-weight=\"700\" fill=\"#2878d2\" text-anchor=\"middle\">{text}</text>",
                        y - 8.0,
                        x + 7.0,
                        y + 5.0,
                        x - 7.0,
                        y + 5.0,
                        y + 17.0
                    );
                }
            }
        }
        // Section lines & detail callouts.
        for section in &project.views.sections {
            let (x1, y1) = point(section.start);
            let (x2, y2) = point(section.end);
            let tag = escaped(&section.label);
            let _ = writeln!(
                svg,
                "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"#1c2b3c\" stroke-width=\"1.8\" stroke-dasharray=\"14 4 3 4\"/>\n\
                 <circle cx=\"{x1:.1}\" cy=\"{y1:.1}\" r=\"10\" fill=\"#ffffff\" stroke=\"#1c2b3c\" stroke-width=\"1.5\"/><text x=\"{x1:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"10\" font-weight=\"700\" fill=\"#1c2b3c\" text-anchor=\"middle\">{tag}</text>\n\
                 <circle cx=\"{x2:.1}\" cy=\"{y2:.1}\" r=\"10\" fill=\"#ffffff\" stroke=\"#1c2b3c\" stroke-width=\"1.5\"/><text x=\"{x2:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"10\" font-weight=\"700\" fill=\"#1c2b3c\" text-anchor=\"middle\">{tag}</text>",
                y1 + 3.5,
                y2 + 3.5
            );
        }
        for detail in &project.views.details {
            let (cx, cy) = point(detail.center);
            let r = detail.radius * pixels_per_meter;
            let tag = match &detail.target_sheet {
                Some(s) => format!("{} ({s})", detail.label),
                None => detail.label.clone(),
            };
            let _ = writeln!(
                svg,
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{r:.1}\" fill=\"none\" stroke=\"#c27803\" stroke-width=\"1.5\" stroke-dasharray=\"8 4\"/><text x=\"{cx:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"11\" font-weight=\"700\" fill=\"#c27803\" text-anchor=\"middle\">{}</text>",
                cy - r - 6.0,
                escaped(&tag)
            );
        }
    }

    let wall_width = base_wall_width;
    let mut occupied = labels::write_room_labels(&mut svg, project, &point, wall_width);
    labels::write_frame_tags(&mut svg, project, &point, pixels_per_meter, &mut occupied);
    labels::write_wall_labels(
        &mut svg,
        project,
        &point,
        pixels_per_meter,
        wall_width,
        &mut occupied,
    );
    if let Some((overlay_project, discipline)) = focused_overlay {
        let _ = writeln!(
            svg,
            "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\" fill-opacity=\"{CONTRAST_VEIL_OPACITY:.2}\" data-contrast-veil=\"true\"/>"
        );
        write_discipline_overlay_svg(
            &mut svg,
            overlay_project,
            discipline,
            &point,
            pixels_per_meter,
        );
    }
    let _ = writeln!(
        svg,
        "<text x=\"{MARGIN:.1}\" y=\"55\" font-family=\"Arial, sans-serif\" font-size=\"23\" font-weight=\"700\" fill=\"#1c2b3c\">{}</text>",
        escaped(title)
    );
    if let Some(sub) = subtitle {
        let _ = writeln!(
            svg,
            "<text x=\"{MARGIN:.1}\" y=\"78\" font-family=\"Arial, sans-serif\" font-size=\"14\" fill=\"#5b6b7c\">{}</text>",
            escaped(sub)
        );
    }
    markups::write(&mut svg, bounds_source, point, 12.0);
    svg.push_str("</svg>");
    SvgDocument {
        markup: svg,
        width,
        height,
    }
}

/// Shared system font database for `resvg` rasterisation, loaded once per
/// process instead of scanning the OS font directories on every preview.
pub fn resvg_options() -> resvg::usvg::Options<'static> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    let fontdb = Arc::clone(FONTS.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    }));
    resvg::usvg::Options {
        fontdb,
        ..resvg::usvg::Options::default()
    }
}

/// Shared system font database for `svg2pdf` PDF export, loaded once per
/// process.
pub fn svg2pdf_options() -> svg2pdf::usvg::Options<'static> {
    static FONTS: OnceLock<Arc<svg2pdf::usvg::fontdb::Database>> = OnceLock::new();
    let fontdb = Arc::clone(FONTS.get_or_init(|| {
        let mut db = svg2pdf::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    }));
    svg2pdf::usvg::Options {
        fontdb,
        ..svg2pdf::usvg::Options::default()
    }
}

/// Rasterizes an SVG document, scaled down so that its longer side is at most
/// `max_side` pixels (never up).
fn render_document(
    document: &SvgDocument,
    max_side: Option<u32>,
) -> Result<resvg::tiny_skia::Pixmap, String> {
    let options = resvg_options();
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

#[cfg(test)]
pub fn png(project: &Project, path: &Path) -> Result<(), String> {
    render_document(&document(project), None)?
        .save_png(path)
        .map_err(|error| error.to_string())
}

/// Exports one view/tab as a high-resolution PNG matching its discipline view.
pub fn png_for_view(project: &Project, view: &ExportView, path: &Path) -> Result<(), String> {
    render_document(&document_for_view(project, view), None)?
        .save_png(path)
        .map_err(|error| error.to_string())
}

/// The plan as PNG bytes no larger than `max_side` pixels, for previews such
/// as the picture an AI assistant asks for.
#[cfg(test)]
pub fn png_preview(project: &Project, max_side: u32) -> Result<Vec<u8>, String> {
    render_document(&document(project), Some(max_side))?
        .encode_png()
        .map_err(|error| error.to_string())
}

/// Preview PNG bytes rendered for a specific discipline/view (with faded
/// architectural underlay for specialized tabs).
pub fn png_preview_for_view(
    project: &Project,
    view: &ExportView,
    max_side: u32,
) -> Result<Vec<u8>, String> {
    render_document(&document_for_view(project, view), Some(max_side))?
        .encode_png()
        .map_err(|error| error.to_string())
}

fn write_single_svg_pdf(markup: &str, path: &Path) -> Result<(), String> {
    let options = svg2pdf_options();
    let tree =
        svg2pdf::usvg::Tree::from_str(markup, &options).map_err(|error| error.to_string())?;
    let pdf = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    fs::write(path, pdf).map_err(|error| error.to_string())
}

#[cfg(test)]
pub fn pdf(project: &Project, path: &Path) -> Result<(), String> {
    write_single_svg_pdf(&document(project).markup, path)
}

/// Exports one view/tab as a vector PDF matching its discipline view on screen.
pub fn pdf_for_view(project: &Project, view: &ExportView, path: &Path) -> Result<(), String> {
    write_single_svg_pdf(&document_for_view(project, view).markup, path)
}

/// Exports multiple open tabs/views of `project` into a single multi-page
/// vector PDF — one page per tab, preserving each tab's discipline visibility,
/// faded architectural underlay and floor selection.
pub fn pdf_views(project: &Project, views: &[ExportView], path: &Path) -> Result<usize, String> {
    if views.is_empty() {
        return Err("Nenhuma aba disponível para exportar em PDF.".to_owned());
    }
    if views.len() == 1 {
        pdf_for_view(project, &views[0], path)?;
        return Ok(1);
    }
    let pages: Vec<String> = views
        .iter()
        .map(|view| document_for_view(project, view).markup)
        .collect();
    let pdf = crate::io::report::pages_to_pdf(&pages)?;
    fs::write(path, pdf).map_err(|error| error.to_string())?;
    Ok(pages.len())
}

/// Grava a planta como SVG vetorial editável, mantendo textos e formas como
/// elementos independentes para edição em programas gráficos e CAD.
#[cfg(test)]
pub fn svg(project: &Project, path: &Path) -> Result<(), String> {
    fs::write(path, document(project).markup).map_err(|error| error.to_string())
}

/// Exports one view/tab as an editable vector SVG matching its discipline view.
pub fn svg_for_view(project: &Project, view: &ExportView, path: &Path) -> Result<(), String> {
    fs::write(path, document_for_view(project, view).markup).map_err(|error| error.to_string())
}

/// Exporta áreas, esquadrias e quantitativos em CSV UTF-8.
pub fn csv(project: &Project, path: &Path) -> Result<(), String> {
    csv::write(project, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    /// A contact sheet exercises the production fixture exporter and provides
    /// a reproducible visual QA artifact when TRAZO_INTERIOR_PREVIEW_DIR is set.
    #[test]
    fn detailed_interior_catalog_exports_and_rasterizes() {
        let mut markup = String::from(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1440" height="1560" viewBox="0 0 1440 1560"><rect width="1440" height="1560" fill="#f1f5f9"/>"##,
        );
        for (i, kind) in FixtureKind::ALL.into_iter().enumerate() {
            let (width, depth) = kind.default_size();
            let fixture = Fixture {
                id: i as u64,
                kind,
                center: Point::new(0.0, 0.0),
                width,
                depth,
                quarter_turns: 0,
                angle_rad: 0.0,
                electrical: None,
                structural_load_kn: None,
                pile: None,
            };
            let (low, high) = crate::domain::fixtures::symbol_bounds(&fixture.symbol()).unwrap();
            let center = low.midpoint(high);
            let x = (i % 4) as f64 * 360.0;
            let y = (i / 4) as f64 * 260.0;
            let scale = (300.0 / (high.x - low.x)).min(170.0 / (high.y - low.y));
            let at = |p: Point| {
                (
                    x + 180.0 + (p.x - center.x) * scale,
                    y + 155.0 + (p.y - center.y) * scale,
                )
            };
            let _ = writeln!(
                markup,
                r##"<rect x="{}" y="{}" width="340" height="240" rx="12" fill="#ffffff" stroke="#d8e0e9"/><text x="{}" y="{}" font-family="Arial" font-size="18" fill="#26374b">{}</text><text x="{}" y="{}" font-family="Arial" font-size="13" fill="#718096">{:.2} × {:.2} m</text>"##,
                x + 10.0,
                y + 10.0,
                x + 28.0,
                y + 39.0,
                kind.label(),
                x + 28.0,
                y + 60.0,
                width,
                depth
            );
            write_fixture_svg(&mut markup, &fixture, &at, scale);
        }
        markup.push_str("</svg>");
        let tree = resvg::usvg::Tree::from_str(&markup, &resvg_options()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(1440, 1560).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert!(pixmap.data().iter().any(|value| *value < 100));
        if let Some(dir) = std::env::var_os("TRAZO_INTERIOR_PREVIEW_DIR") {
            let dir = std::path::PathBuf::from(dir);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("interiores.svg"), markup).unwrap();
            pixmap.save_png(dir.join("interiores.png")).unwrap();
        }
    }

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
    fn exports_png_pdf_and_editable_svg() {
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
        let svg_path = directory.join("plan.svg");
        png(&project, &png_path).unwrap();
        pdf(&project, &pdf_path).unwrap();
        svg(&project, &svg_path).unwrap();
        assert!(fs::read(&png_path).unwrap().starts_with(b"\x89PNG"));
        assert!(fs::read(&pdf_path).unwrap().starts_with(b"%PDF"));
        let svg_content = fs::read_to_string(&svg_path).unwrap();
        assert!(svg_content.starts_with("<svg"));
        assert!(svg_content.contains("<text"));
        assert!(svg_content.contains("<line"));
        assert!(svg_content.contains("Quarto personalizado"));
        let _ = fs::remove_file(png_path);
        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_file(svg_path);
        let _ = fs::remove_dir(directory);
    }

    #[test]
    fn visible_underlay_exports_as_svg_image_with_opacity() {
        let mut project = Project::default();
        let id = project.add_underlay(
            "Levantamento".to_owned(),
            "lev.png".to_owned(),
            1,
            100,
            50,
            Point::new(0.0, 0.0),
            10.0,
            Some("iVBORw0KGgo=".to_owned()),
        );
        let markup = document(&project).markup;
        assert!(markup.contains("<image href=\"data:image/png;base64,iVBORw0KGgo=\""));
        assert!(markup.contains("opacity=\"0.45\""));
        assert!(markup.contains("data-underlay=\"Levantamento\""));

        project
            .underlays
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .visible = false;
        let hidden_markup = document(&project).markup;
        assert!(!hidden_markup.contains("data-underlay=\"Levantamento\""));
    }

    #[test]
    fn shared_font_databases_are_reused_across_calls() {
        let a = resvg_options();
        let b = resvg_options();
        assert!(Arc::ptr_eq(&a.fontdb, &b.fontdb));

        let c = svg2pdf_options();
        let d = svg2pdf_options();
        assert!(Arc::ptr_eq(&c.fontdb, &d.fontdb));
    }

    #[test]
    fn multi_tab_views_render_distinct_disciplines_and_export_multipage_pdf() {
        let mut project = Project::default();
        project.name = "Casa Teste".to_owned();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Cozinha")
            .unwrap();
        // Add furniture (stove) and structure (columns, footings, beams, water tank).
        project
            .add_fixture(FixtureKind::Stove, Point::new(1.0, 1.0), 0.6, 0.6, 0)
            .unwrap();
        project.generate_structure();
        project.generate_wall_foundations();
        project
            .add_fixture(FixtureKind::WaterTank, Point::new(2.0, 1.5), 1.0, 1.0, 0)
            .unwrap();

        let general_view = ExportView::for_discipline(&project, Discipline::All);
        let structure_view = ExportView::for_discipline(&project, Discipline::Structure);
        let foundations_view = ExportView::for_discipline(&project, Discipline::Foundations);

        let general_svg = document_for_view(&project, &general_view).markup;
        let structure_svg = document_for_view(&project, &structure_view).markup;
        let foundations_svg = document_for_view(&project, &foundations_view).markup;

        // General tab: no contrast veil, no structural/foundation overlays.
        assert!(!general_svg.contains("data-contrast-veil=\"true\""));
        assert!(!general_svg.contains("id=\"estrutura\""));
        assert!(!general_svg.contains("id=\"fundacoes\""));

        // Structure tab (Planta de Forma): columns, upper beams, slabs and water tank,
        // WITHOUT isolated footings (S#) or baldrames (VB#).
        assert!(structure_svg.contains("data-contrast-veil=\"true\""));
        assert!(structure_svg.contains("data-discipline=\"estrutura\""));
        assert!(structure_svg.contains("id=\"estrutura\""));
        assert!(structure_svg.contains("Planta de Forma"));
        assert!(structure_svg.contains(">P1 "));
        assert!(structure_svg.contains(">V1 "));
        assert!(structure_svg.contains(">L1 "));
        assert!(structure_svg.contains(">CC1 "));
        assert!(!structure_svg.contains(">S1 "));
        assert!(!structure_svg.contains(">VB1 "));

        // Foundations tab (Planta de Fundações): columns, isolated footings and baldrames,
        // WITHOUT upper beams (V#), slabs (L#) or water tank (CC#).
        assert!(foundations_svg.contains("data-contrast-veil=\"true\""));
        assert!(foundations_svg.contains("data-discipline=\"fundacoes\""));
        assert!(foundations_svg.contains("id=\"fundacoes\""));
        assert!(foundations_svg.contains("Planta de Fundações"));
        assert!(foundations_svg.contains(">P1 "));
        assert!(foundations_svg.contains(">S1 "));
        assert!(foundations_svg.contains(">VB1 "));
        assert!(!foundations_svg.contains(">V1 "));
        assert!(!foundations_svg.contains(">L1 "));
        assert!(!foundations_svg.contains(">CC1 "));

        // Multi-page PDF has 3 pages (one per tab).
        let directory = std::env::temp_dir().join(format!("trazo-tabs-pdf-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let pdf_path = directory.join("abas.pdf");
        let pages = pdf_views(
            &project,
            &[general_view, structure_view, foundations_view],
            &pdf_path,
        )
        .unwrap();
        assert_eq!(pages, 3);
        let bytes = fs::read(&pdf_path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 3"), "expected 3 PDF pages");
        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_dir(directory);
    }
}
