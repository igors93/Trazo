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
    if project.conduits.is_empty() {
        return;
    }
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
