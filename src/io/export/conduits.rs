//! Plan marks in SVG exports: the conduits (eletrodutos) with their wire
//! ticks and the beams and labels of the formwork plan, shared by the plan
//! export and the scaled sheets so both draw the same marks.

use std::fmt::Write as _;

use crate::domain::conduits::{Conductor, PlanMark};
use crate::domain::geometry::Point;
use crate::domain::model::Project;

use super::{ELECTRICAL_INK, STRUCTURE_INK, escaped};

/// Writes the conduit marks. `point` maps plan metres to SVG units, and
/// `unit` is how many SVG units one metre takes; `heavy`, `thin` and
/// `font` are stroke widths and the text height in SVG units.
pub(super) fn write_conduits(
    svg: &mut String,
    project: &Project,
    point: &dyn Fn(Point) -> (f64, f64),
    unit: f64,
    widths: (f64, f64, f64),
) {
    if !project.conduits.is_empty() {
        write_marks(
            svg,
            &project.conduit_marks(),
            point,
            unit,
            widths,
            ELECTRICAL_INK,
            "eletrodutos",
        );
    }
    let lux_marks = project.lighting_plan_marks();
    if !lux_marks.is_empty() {
        write_marks(
            svg,
            &lux_marks,
            point,
            unit,
            widths,
            "#b46e0a",
            "iluminancia-lux",
        );
    }
    let pv_marks = project.pv_plan_marks();
    if !pv_marks.is_empty() {
        write_marks(
            svg,
            &pv_marks,
            point,
            unit,
            widths,
            "#1565c0",
            "arranjo-fotovoltaico",
        );
    }
    let earth_marks = project.earthing_plan_marks();
    if !earth_marks.is_empty() {
        write_marks(
            svg,
            &earth_marks,
            point,
            unit,
            widths,
            "#2e7d32",
            "aterramento-bep",
        );
    }
}

/// Writes structural plan marks filtered by `scope` (`All`, `Superstructure`
/// for `Planta de Forma`, or `Foundations` for `Planta de Fundações`).
pub(super) fn write_structure_scoped(
    svg: &mut String,
    project: &Project,
    scope: crate::domain::structure::StructureMarksScope,
    point: &dyn Fn(Point) -> (f64, f64),
    unit: f64,
    widths: (f64, f64, f64),
) {
    if project.structure.beams.is_empty()
        && project.structure.slabs.is_empty()
        && project.structure.balance_beams.is_empty()
        && project.structure.wall_foundations.is_empty()
        && project.structure.roofs.is_empty()
        && !project.fixtures.iter().any(|f| f.kind.is_structural())
    {
        return;
    }
    let group = match scope {
        crate::domain::structure::StructureMarksScope::Foundations => "fundacoes",
        _ => "estrutura",
    };
    write_marks(
        svg,
        &project.structure_marks_for(scope),
        point,
        unit,
        widths,
        STRUCTURE_INK,
        group,
    );
    write_simulation_overlay(svg, project, point, unit, widths);
}

/// Writes the active on-plan structural simulation overlays (#106) into SVG/PDF
/// exports so screen equals paper: utilisation traffic-light colours, internal-force
/// diagrams directly over beams, exaggerated deformed shapes, directional load path
/// from slab to soil, and overlap-free labels.
pub(super) fn write_simulation_overlay(
    svg: &mut String,
    project: &Project,
    point: &dyn Fn(Point) -> (f64, f64),
    _unit: f64,
    (heavy, thin, font): (f64, f64, f64),
) {
    use crate::domain::plan_results::{
        BeamDiagramOverlay, LoadPathStage, OverlayLabelTone, build_plan_simulation_overlay,
    };
    use crate::domain::verdict::Light;

    let cfg = project.structure.overlay;
    if cfg.is_empty() {
        return;
    }
    let report = project.cached_structure_report();
    let overlay = build_plan_simulation_overlay(project, &report, cfg);
    if overlay.is_empty() {
        return;
    }

    let light_hex = |light: Light| match light {
        Light::Green => "#2e7d32",
        Light::Yellow => "#e67e22",
        Light::Red => "#c62828",
    };
    let tone_hex = |tone: OverlayLabelTone| match tone {
        OverlayLabelTone::Green => "#1b5e20",
        OverlayLabelTone::Yellow => "#bf5a00",
        OverlayLabelTone::Red => "#b71c1c",
        OverlayLabelTone::MomentPos => "#1565c0",
        OverlayLabelTone::MomentNeg => "#b71c1c",
        OverlayLabelTone::Shear => "#6a1b9a",
        OverlayLabelTone::LoadPath => "#00695c",
    };
    let pts_attr = |pts: &[Point]| -> String {
        pts.iter()
            .map(|p| {
                let (x, y) = point(*p);
                format!("{x:.2},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    let _ = writeln!(svg, "<g id=\"simulacao-estrutural\">");

    // 1. Utilisation traffic-light colours on slabs, beams, footings and columns.
    for slab in &overlay.slabs {
        if slab.polygon.len() >= 3 {
            let color = light_hex(slab.light);
            let _ = writeln!(
                svg,
                "<polygon points=\"{}\" fill=\"{color}\" fill-opacity=\"0.14\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
                pts_attr(&slab.polygon),
                heavy * 0.9
            );
        }
    }
    for beam in &overlay.beams {
        let (ax, ay) = point(beam.start);
        let (bx, by) = point(beam.end);
        let color = light_hex(beam.light);
        let _ = writeln!(
            svg,
            "<line x1=\"{ax:.2}\" y1=\"{ay:.2}\" x2=\"{bx:.2}\" y2=\"{by:.2}\" stroke=\"{color}\" stroke-width=\"{:.2}\" stroke-opacity=\"0.42\"/>",
            heavy * 3.0
        );
        let _ = writeln!(
            svg,
            "<line x1=\"{ax:.2}\" y1=\"{ay:.2}\" x2=\"{bx:.2}\" y2=\"{by:.2}\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
            heavy * 1.1
        );
    }
    let write_box = |svg: &mut String,
                     el: &crate::domain::plan_results::PointUtilisationOverlay,
                     fill_opacity: f64| {
        let (sin, cos) = el.angle_rad.sin_cos();
        let hw = el.width_m.max(0.14) * 0.5;
        let hd = el.depth_m.max(0.14) * 0.5;
        let corners: Vec<Point> = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)]
            .into_iter()
            .map(|(lx, ly)| {
                Point::new(
                    el.center.x + lx * cos - ly * sin,
                    el.center.y + lx * sin + ly * cos,
                )
            })
            .collect();
        let color = light_hex(el.light);
        let _ = writeln!(
            svg,
            "<polygon points=\"{}\" fill=\"{color}\" fill-opacity=\"{fill_opacity:.2}\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
            pts_attr(&corners),
            heavy * 0.9
        );
    };
    for ft in &overlay.footings {
        write_box(svg, ft, 0.18);
    }
    for col in &overlay.columns {
        write_box(svg, col, 0.55);
    }

    // 2. Internal-force diagrams over beams.
    for diag in &overlay.beam_diagrams {
        let (pos_color, neg_color) = match diag.kind {
            BeamDiagramOverlay::Moment => ("#1565c0", "#c62828"),
            BeamDiagramOverlay::Shear => ("#6a1b9a", "#d84315"),
            BeamDiagramOverlay::None => continue,
        };
        for lobe in &diag.positive_lobes {
            let _ = writeln!(
                svg,
                "<polygon points=\"{}\" fill=\"{pos_color}\" fill-opacity=\"0.22\" stroke=\"{pos_color}\" stroke-width=\"{:.2}\"/>",
                pts_attr(lobe),
                thin * 1.1
            );
        }
        for lobe in &diag.negative_lobes {
            let _ = writeln!(
                svg,
                "<polygon points=\"{}\" fill=\"{neg_color}\" fill-opacity=\"0.22\" stroke=\"{neg_color}\" stroke-width=\"{:.2}\"/>",
                pts_attr(lobe),
                thin * 1.1
            );
        }
        if diag.curve.len() >= 2 {
            let _ = writeln!(
                svg,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{pos_color}\" stroke-width=\"{:.2}\"/>",
                pts_attr(&diag.curve),
                heavy * 0.9
            );
        }
    }

    // 3. Deformed shape over beams.
    for defl in &overlay.deformed_beams {
        let (ax, ay) = point(defl.start);
        let (bx, by) = point(defl.end);
        let color = light_hex(defl.light);
        let _ = writeln!(
            svg,
            "<line x1=\"{ax:.2}\" y1=\"{ay:.2}\" x2=\"{bx:.2}\" y2=\"{by:.2}\" stroke=\"#8c96a3\" stroke-width=\"{:.2}\" stroke-dasharray=\"{:.2} {:.2}\"/>",
            thin * 0.8,
            heavy * 2.0,
            heavy * 1.5
        );
        if defl.curve.len() >= 2 {
            let _ = writeln!(
                svg,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{:.2}\"/>",
                pts_attr(&defl.curve),
                heavy * 1.25
            );
        }
    }

    // 4. Static directional load-path arrows (slab -> beam -> column -> footing/soil).
    for arrow in &overlay.load_paths {
        let (ax, ay) = point(arrow.from);
        let (bx, by) = point(arrow.to);
        let dx = bx - ax;
        let dy = by - ay;
        let len = dx.hypot(dy);
        if len < 1e-3 {
            continue;
        }
        let ux = dx / len;
        let uy = dy / len;
        let nx = -uy;
        let ny = ux;
        let head = (heavy * 4.2).min(len * 0.35);
        let (color, stroke_w) = match arrow.stage {
            LoadPathStage::SlabToBeam => ("#0277bd", heavy * 0.9),
            LoadPathStage::BeamToColumn => ("#00897b", heavy * 1.1),
            LoadPathStage::ColumnToFooting => ("#c62828", heavy * 1.35),
        };
        let _ = writeln!(
            svg,
            "<line x1=\"{ax:.2}\" y1=\"{ay:.2}\" x2=\"{bx:.2}\" y2=\"{by:.2}\" stroke=\"{color}\" stroke-width=\"{stroke_w:.2}\"/>"
        );
        let lx = bx - ux * head + nx * (head * 0.45);
        let ly = by - uy * head + ny * (head * 0.45);
        let rx = bx - ux * head - nx * (head * 0.45);
        let ry = by - uy * head - ny * (head * 0.45);
        let _ = writeln!(
            svg,
            "<polygon points=\"{bx:.2},{by:.2} {lx:.2},{ly:.2} {rx:.2},{ry:.2}\" fill=\"{color}\"/>"
        );
    }

    // 5. Overlap-free labels.
    let label_font = (font * 0.92).max(1.6);
    for label in &overlay.labels {
        let (x, y) = point(label.anchor);
        let color = tone_hex(label.tone);
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{label_font:.2}\" font-weight=\"700\" fill=\"{color}\" stroke=\"#ffffff\" stroke-width=\"{:.2}\" paint-order=\"stroke\">{}</text>",
            label_font * 0.28,
            escaped(&label.text)
        );
    }

    svg.push_str("</g>\n");
}

/// Writes the beams and the P, V, L and S labels of the combined plan.
pub(super) fn write_structure(
    svg: &mut String,
    project: &Project,
    point: &dyn Fn(Point) -> (f64, f64),
    unit: f64,
    widths: (f64, f64, f64),
) {
    write_structure_scoped(
        svg,
        project,
        crate::domain::structure::StructureMarksScope::All,
        point,
        unit,
        widths,
    );
}

/// Writes plan marks in one colour, inside a group named `group`.
fn write_marks(
    svg: &mut String,
    marks: &[PlanMark],
    point: &dyn Fn(Point) -> (f64, f64),
    unit: f64,
    (heavy, thin, font): (f64, f64, f64),
    ink: &str,
    group: &str,
) {
    let dash = format!("{:.2} {:.2}", unit * 0.15, unit * 0.1);
    let _ = writeln!(svg, "<g id=\"{group}\">");
    for mark in marks {
        match mark {
            PlanMark::Line {
                points,
                dashed,
                heavy: is_conduit,
            } => {
                let list = points
                    .iter()
                    .map(|p| {
                        let (x, y) = point(*p);
                        format!("{x:.2},{y:.2}")
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = writeln!(
                    svg,
                    "<polyline points=\"{list}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"{:.2}\"{}/>",
                    if *is_conduit { heavy } else { thin },
                    if *dashed {
                        format!(" stroke-dasharray=\"{dash}\"")
                    } else {
                        String::new()
                    }
                );
            }
            PlanMark::Circle { center, radius } => {
                let (x, y) = point(*center);
                let _ = writeln!(
                    svg,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"{thin:.2}\"/>",
                    radius * unit
                );
            }
            PlanMark::Text { anchor, text } => {
                let (x, y) = point(*anchor);
                let _ = writeln!(
                    svg,
                    "<text x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"{font:.2}\" fill=\"{ink}\" stroke=\"#ffffff\" stroke-width=\"{:.2}\" paint-order=\"stroke\">{}</text>",
                    font * 0.25,
                    escaped(text)
                );
            }
        }
    }
    svg.push_str("</g>\n");
}

/// A small legend of the wire ticks, drawn at (`x`, `y`) in sheet
/// millimetres, when the plan has conduits.
pub(super) fn write_wire_legend(svg: &mut String, project: &Project, x: f64, y: f64) {
    if project.conduits.is_empty() {
        return;
    }
    let _ = writeln!(
        svg,
        "<g id=\"legenda-fiacao\"><text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"Arial, sans-serif\" font-size=\"2.2\" font-weight=\"700\" fill=\"#1d2836\">Fiação e eletrodutos</text>"
    );
    for (i, conductor) in Conductor::ALL.iter().enumerate() {
        let cy = y + 4.0 + i as f64 * 3.6;
        let cx = x + 3.0;
        let _ = writeln!(
            svg,
            "<line x1=\"{:.2}\" y1=\"{cy:.2}\" x2=\"{:.2}\" y2=\"{cy:.2}\" stroke=\"{ELECTRICAL_INK}\" stroke-width=\"0.25\"/>\
             <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{ELECTRICAL_INK}\" stroke-width=\"0.18\"/>",
            cx - 3.0,
            cx + 3.0,
            cx - 0.4,
            cy + 1.0,
            cx + 0.4,
            cy - 1.0
        );
        let top = (cx + 0.4, cy - 1.0);
        let extra = match conductor {
            Conductor::Phase => format!(
                "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{ELECTRICAL_INK}\" stroke-width=\"0.18\"/>",
                top.0,
                top.1,
                top.0 + 0.7,
                top.1
            ),
            Conductor::Neutral => String::new(),
            Conductor::Return => format!(
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"0.3\" fill=\"none\" stroke=\"{ELECTRICAL_INK}\" stroke-width=\"0.15\"/>",
                top.0,
                top.1 - 0.3
            ),
            Conductor::Earth => format!(
                "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{ELECTRICAL_INK}\" stroke-width=\"0.18\"/>",
                top.0 - 0.5,
                top.1,
                top.0 + 0.5,
                top.1
            ),
        };
        let _ = writeln!(
            svg,
            "{extra}<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.8\" fill=\"#1d2836\">{}</text>",
            cx + 5.0,
            cy + 0.6,
            conductor.label()
        );
    }
    let notes = [
        "Linha cheia: teto/parede · tracejada: piso",
        "Eletroduto sem indicação: Ø20 mm",
    ];
    for (i, note) in notes.iter().enumerate() {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{:.2}\" font-family=\"Arial, sans-serif\" font-size=\"1.7\" fill=\"#3b4e63\">{note}</text>",
            y + 19.0 + i as f64 * 2.8
        );
    }
    svg.push_str("</g>\n");
}
