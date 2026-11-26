//! The setting-out sheet (#55, *planta de locação*).

use std::fmt::Write as _;
use std::path::Path;

use super::{Drawn, INK, MUTED, Sheet, bounds, decimal, finish, fit_scale};
use crate::domain::construction::setting_out::{SetPointKind, setting_out};
use crate::domain::geometry::Point;
use crate::domain::model::Project;

const AXIS: &str = "#2878d2";

/// The setting-out plan: the lot, the building, the string lines and the
/// cumulative distances from the origin, with the table of points.
pub(crate) fn setting_out_drawing(project: &Project) -> Option<Drawn> {
    let out = setting_out(project)?;
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let table_w = 78.0;
    let plan_w = aw - table_w - 26.0;
    let lot = project.terrain.as_ref().map(|t| t.outline());
    let extent = lot
        .clone()
        .unwrap_or_default()
        .into_iter()
        .chain(project.walls.iter().flat_map(|w| [w.start, w.end]))
        .chain([out.origin]);
    let (min, max) = bounds(extent, 0.5).unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let (w_m, h_m) = (max.x - min.x, max.y - min.y);
    let scale = fit_scale(w_m, h_m, plan_w, ah - 30.0);
    let s = scale.mm_per_meter();
    let ox = ax + 20.0 + (plan_w - w_m * s) / 2.0;
    let oy = ay + 4.0 + (ah - 30.0 - h_m * s) / 2.0;
    let pt = |p: Point| (ox + (p.x - min.x) * s, oy + (p.y - min.y) * s);

    if let Some(lot) = &lot {
        let points: Vec<(f64, f64)> = lot.iter().map(|p| pt(*p)).collect();
        sheet.polygon(&points, "#f8fbfe", "#5e8bbf", 0.3);
    }
    for wall in &project.walls {
        let length = wall.length().max(1e-9);
        let n = Point::new(
            -(wall.end.y - wall.start.y) / length,
            (wall.end.x - wall.start.x) / length,
        );
        let t = wall.thickness() / 2.0;
        let corner = |p: Point, k: f64| pt(Point::new(p.x + n.x * t * k, p.y + n.y * t * k));
        sheet.polygon(
            &[
                corner(wall.start, -1.0),
                corner(wall.end, -1.0),
                corner(wall.end, 1.0),
                corner(wall.start, 1.0),
            ],
            "#e9eef4",
            "#7d8b99",
            0.2,
        );
    }

    // String lines (gabarito) through every cumulative distance.
    let (left, top) = pt(min);
    let (right, bottom) = pt(max);
    let (origin_x, origin_y) = pt(out.origin);
    for x in &out.xs {
        let sx = origin_x + x * s;
        sheet.line((sx, top), (sx, bottom), AXIS, 0.15, Some("2 1"));
    }
    for y in &out.ys {
        let sy = origin_y - y * s;
        sheet.line((left, sy), (right, sy), AXIS, 0.15, Some("2 1"));
    }

    // Cumulative distances: along the bottom (X) and the left (Y).
    let base = bottom + 6.0;
    sheet.line(
        (origin_x, base),
        (origin_x + out.xs.last().copied().unwrap_or(0.0) * s, base),
        INK,
        0.25,
        None,
    );
    for x in &out.xs {
        let sx = origin_x + x * s;
        sheet.line((sx, base - 1.2), (sx, base + 1.2), INK, 0.25, None);
        let _ = writeln!(
            sheet.svg,
            "<text x=\"{sx:.2}\" y=\"{:.2}\" font-family=\"{}\" font-size=\"1.9\" fill=\"{INK}\" transform=\"rotate(90 {sx:.2} {:.2})\">{}</text>",
            base + 2.0,
            super::FONT,
            base + 2.0,
            decimal(*x, 2)
        );
    }
    let side = left - 6.0;
    sheet.line(
        (side, origin_y),
        (side, origin_y - out.ys.last().copied().unwrap_or(0.0) * s),
        INK,
        0.25,
        None,
    );
    for y in &out.ys {
        let sy = origin_y - y * s;
        sheet.line((side - 1.2, sy), (side + 1.2, sy), INK, 0.25, None);
        sheet.text(side - 12.0, sy + 0.7, 1.9, false, INK, &decimal(*y, 2));
    }

    // The origin and the points.
    sheet.line(
        (origin_x - 3.0, origin_y),
        (origin_x + 3.0, origin_y),
        "#c0392b",
        0.4,
        None,
    );
    sheet.line(
        (origin_x, origin_y - 3.0),
        (origin_x, origin_y + 3.0),
        "#c0392b",
        0.4,
        None,
    );
    sheet.text(
        origin_x + 1.5,
        origin_y + 3.5,
        2.2,
        true,
        "#c0392b",
        "0 (origem)",
    );
    for point in &out.points {
        let (px, py) = pt(point.position);
        let color = if point.kind == SetPointKind::Column {
            "#0f766e"
        } else {
            INK
        };
        let _ = writeln!(
            sheet.svg,
            "<circle cx=\"{px:.2}\" cy=\"{py:.2}\" r=\"0.7\" fill=\"{color}\"/>"
        );
        sheet.text(px + 1.0, py - 1.0, 1.9, true, color, &point.label);
    }

    // Table of points.
    let tx = ax + aw - table_w;
    let mut y = ay + 6.0;
    sheet.text(tx, y, 2.4, true, INK, "PONTOS DE LOCAÇÃO (m)");
    y += 3.6;
    sheet.text(tx, y, 2.0, true, MUTED, "Ponto        X (→)       Y (↑)");
    y += 3.2;
    let room = ((ay + ah - y - 18.0) / 3.0).max(1.0) as usize;
    for point in out.points.iter().take(room) {
        sheet.text(
            tx,
            y,
            2.0,
            false,
            INK,
            &format!(
                "{:<8} {:>9} {:>11}",
                point.label,
                decimal(point.x_m, 2),
                decimal(point.y_m, 2)
            ),
        );
        y += 3.0;
    }
    if out.points.len() > room {
        sheet.text(
            tx,
            y,
            1.9,
            false,
            MUTED,
            &format!("… e mais {} ponto(s).", out.points.len() - room),
        );
        y += 3.0;
    }
    y += 2.0;
    for note in [
        "Medidas acumuladas a partir da origem (canto do terreno),",
        "até o eixo das paredes e o centro dos pilares.",
        "Marque com gabarito e linhas; confira as diagonais",
        "de cada cômodo antes de cavar.",
    ] {
        sheet.text(tx, y, 1.9, false, MUTED, note);
        y += 2.8;
    }
    Some(Drawn {
        sheet,
        title: "Planta de locação".to_owned(),
        scale: scale.label(),
    })
}

/// The setting-out plan as one SVG sheet.
pub fn setting_out_svg(project: &Project) -> Result<String, String> {
    setting_out_drawing(project)
        .map(|drawn| finish(project, drawn, "01/01"))
        .ok_or_else(|| "Desenhe as paredes antes da planta de locação.".to_owned())
}

/// Writes the setting-out plan as a one-page PDF.
pub fn export_setting_out_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    super::write_pdf(&[setting_out_svg(project)?], path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_has_the_origin_the_points_and_their_distances() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(2.0, 5.0), Point::new(6.0, 9.0), "Sala")
            .unwrap();
        let svg = setting_out_svg(&project).unwrap();
        assert!(svg.contains("Planta de locação"));
        assert!(svg.contains("0 (origem)"));
        assert!(svg.contains("PONTOS DE LOCAÇÃO"));
        assert!(svg.contains("2,00"));
        assert!(setting_out_svg(&Project::default()).is_err());
    }
}
