//! The roof plan sheet (#71, *planta de cobertura*).

use std::fmt::Write as _;
use std::path::Path;

use super::{Drawn, INK, MUTED, Sheet, bounds, decimal, finish, fit_scale};
use crate::domain::construction::roof_plan::{
    DOWNPIPE_AREA_M2, GUTTER_FALL_PERCENT, RoofLineKind, roof_plans,
};
use crate::domain::geometry::Point;
use crate::domain::model::Project;

const DANGER: &str = "#c0392b";
const WATER: &str = "#2878d2";

/// How each kind of roof line is drawn: width, dash and colour.
fn line_style(kind: RoofLineKind) -> (f64, Option<&'static str>, &'static str) {
    match kind {
        RoofLineKind::Ridge => (0.55, Some("4 1 1 1"), INK),
        RoofLineKind::Hip => (0.45, None, INK),
        RoofLineKind::Eave => (0.4, None, INK),
        RoofLineKind::Verge => (0.3, None, MUTED),
        RoofLineKind::HighEdge => (0.45, Some("2 1"), INK),
    }
}

/// The roof plan: planes with slope arrows, ridge, hips, eaves and verges,
/// gutters and downpipes, and the table of roofs and gutters.
pub(crate) fn roof_plan_drawing(project: &Project) -> Option<Drawn> {
    let plans = roof_plans(project);
    if plans.is_empty() {
        return None;
    }
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let table_w = 96.0;
    let plan_w = aw - table_w - 8.0;
    let extent = plans
        .iter()
        .flat_map(|p| p.planes.iter().flat_map(|q| q.polygon.clone()))
        .chain(project.walls.iter().flat_map(|w| [w.start, w.end]));
    let (min, max) = bounds(extent, 0.8).unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let (w_m, h_m) = (max.x - min.x, max.y - min.y);
    let scale = fit_scale(w_m, h_m, plan_w, ah - 8.0);
    let s = scale.mm_per_meter();
    let ox = ax + 4.0 + (plan_w - w_m * s) / 2.0;
    let oy = ay + 4.0 + (ah - 8.0 - h_m * s) / 2.0;
    let pt = |p: Point| (ox + (p.x - min.x) * s, oy + (p.y - min.y) * s);

    for wall in &project.walls {
        sheet.line(
            pt(wall.start),
            pt(wall.end),
            "#b8c4d0",
            0.35,
            Some("1.5 0.8"),
        );
    }
    for plan in &plans {
        for plane in &plan.planes {
            let points: Vec<(f64, f64)> = plane.polygon.iter().map(|p| pt(*p)).collect();
            sheet.polygon(&points, "#fbf3ea", "none", 0.0);
            // Slope arrow, pointing the way the water runs.
            let centre = plane.centroid();
            let (cx, cy) = pt(centre);
            let (dx, dy) = (plane.downhill.x * 9.0, plane.downhill.y * 9.0);
            let (tx, ty) = (cx + dx, cy + dy);
            sheet.line((cx - dx * 0.2, cy - dy * 0.2), (tx, ty), INK, 0.3, None);
            let (px, py) = (-plane.downhill.y, plane.downhill.x);
            let _ = writeln!(
                sheet.svg,
                "<polygon points=\"{tx:.2},{ty:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{INK}\"/>",
                tx - plane.downhill.x * 2.0 + px * 1.0,
                ty - plane.downhill.y * 2.0 + py * 1.0,
                tx - plane.downhill.x * 2.0 - px * 1.0,
                ty - plane.downhill.y * 2.0 - py * 1.0
            );
            let color = if plan.slope_ok() { INK } else { DANGER };
            sheet.text(
                cx + px * 2.5 - 3.0,
                cy + py * 2.5,
                2.2,
                true,
                color,
                &format!("i = {} %", decimal(plan.slope_percent, 0)),
            );
        }
        for line in &plan.lines {
            let (width, dash, color) = line_style(line.kind);
            sheet.line(pt(line.from), pt(line.to), color, width, dash);
        }
        for gutter in &plan.gutters {
            let (a, b) = (pt(gutter.from), pt(gutter.to));
            sheet.line(a, b, WATER, 0.8, None);
            for pipe in &gutter.downpipes {
                let (px, py) = pt(*pipe);
                let _ = writeln!(
                    sheet.svg,
                    "<circle cx=\"{px:.2}\" cy=\"{py:.2}\" r=\"1.4\" fill=\"#ffffff\" stroke=\"{WATER}\" stroke-width=\"0.4\"/>"
                );
                sheet.text(px + 1.8, py + 2.8, 1.7, false, WATER, "AP Ø100");
            }
        }
        let first = plan
            .planes
            .first()
            .map(|p| p.centroid())
            .unwrap_or_default();
        let (lx, ly) = pt(first);
        sheet.text(lx - 4.0, ly - 6.0, 2.6, true, INK, &plan.label);
    }

    // Table.
    let tx = ax + aw - table_w;
    let mut y = ay + 6.0;
    sheet.text(tx, y, 2.4, true, INK, "COBERTURA");
    y += 4.0;
    let intensity = project.codes.site.rain_intensity_mm_h;
    for plan in &plans {
        sheet.text(
            tx,
            y,
            2.1,
            true,
            INK,
            &format!(
                "{} · {} · {}",
                plan.label,
                plan.style.label(),
                plan.tile.label()
            ),
        );
        y += 3.0;
        let (color, verdict) = if plan.slope_ok() {
            (INK, "ok")
        } else {
            (
                DANGER,
                "ABAIXO DO MÍNIMO: aumente o caimento ou troque a telha",
            )
        };
        sheet.text(
            tx,
            y,
            2.0,
            false,
            color,
            &format!(
                "Caimento {} % (mínimo da telha {} %): {verdict}",
                decimal(plan.slope_percent, 0),
                decimal(plan.min_slope_percent, 0)
            ),
        );
        y += 3.0;
        sheet.text(
            tx,
            y,
            2.0,
            false,
            INK,
            &format!("Área em planta: {} m²", decimal(plan.area_m2(), 1)),
        );
        y += 3.2;
        for (index, gutter) in plan.gutters.iter().enumerate() {
            let flow = intensity.map_or(String::new(), |i| {
                format!(" · {} L/min", decimal(gutter.flow_l_min(i), 0))
            });
            sheet.text(
                tx + 2.0,
                y,
                1.9,
                false,
                WATER,
                &format!(
                    "Calha {}: {} m, {} m²{flow}, {} condutor(es)",
                    index + 1,
                    decimal(gutter.length_m(), 2),
                    decimal(gutter.area_m2, 1),
                    gutter.downpipes.len()
                ),
            );
            y += 2.8;
        }
        y += 2.0;
        if y > ay + ah - 30.0 {
            break;
        }
    }
    y += 1.0;
    // Line legend.
    for kind in [
        RoofLineKind::Ridge,
        RoofLineKind::Hip,
        RoofLineKind::Eave,
        RoofLineKind::Verge,
        RoofLineKind::HighEdge,
    ] {
        let (width, dash, color) = line_style(kind);
        sheet.line((tx, y - 0.7), (tx + 8.0, y - 0.7), color, width, dash);
        sheet.text(tx + 10.0, y, 1.9, false, INK, kind.label());
        y += 2.8;
    }
    sheet.line((tx, y - 0.7), (tx + 8.0, y - 0.7), WATER, 0.8, None);
    sheet.text(
        tx + 10.0,
        y,
        1.9,
        false,
        INK,
        "calha; círculo AP = condutor de água pluvial",
    );
    y += 3.6;
    for note in [
        format!(
            "Calhas com caimento mínimo de {} % para os condutores (NBR 10844).",
            decimal(GUTTER_FALL_PERCENT, 1)
        ),
        format!(
            "Condutores AP Ø 100 mm: 1 cm² de seção por m² de telhado (até {} m² cada).",
            decimal(DOWNPIPE_AREA_M2, 0)
        ),
        "Dimensionamento final das águas pluviais pela NBR 10844 com a chuva da cidade.".to_owned(),
        "Setas: sentido da água. i = caimento do telhado.".to_owned(),
    ] {
        sheet.text(tx, y, 1.8, false, MUTED, &note);
        y += 2.7;
    }
    Some(Drawn {
        sheet,
        title: "Planta de cobertura".to_owned(),
        scale: scale.label(),
    })
}

/// The roof plan as one SVG sheet.
pub fn roof_plan_svg(project: &Project) -> Result<String, String> {
    roof_plan_drawing(project)
        .map(|drawn| finish(project, drawn, "01/01"))
        .ok_or_else(|| {
            "Defina o telhado (Estrutura › Telhado) antes da planta de cobertura.".to_owned()
        })
}

/// Writes the roof plan as a one-page PDF.
pub fn export_roof_plan_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    super::write_pdf(&[roof_plan_svg(project)?], path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::roof::{Roof, RoofSide, RoofStyle};

    #[test]
    fn the_roof_plan_shows_slope_gutters_and_downpipes() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 4.0), "Sala")
            .unwrap();
        assert!(roof_plan_svg(&project).is_err());
        let mut roof = Roof::new(
            Point::new(0.0, 0.0),
            Point::new(6.0, 4.0),
            RoofSide::North,
            3.8,
            2.8,
        );
        roof.style = RoofStyle::Gable;
        roof.overhang_m = 0.5;
        project.add_roof(roof).unwrap();
        let svg = roof_plan_svg(&project).unwrap();
        assert!(svg.contains("Planta de cobertura"));
        assert!(svg.contains("AP Ø100"));
        assert!(svg.contains("Calha 2"));
    }
}
