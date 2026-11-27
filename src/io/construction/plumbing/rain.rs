//! The rainwater plan (#122): the roofs' outlines, the lot's boundary and
//! any pool, every gutter with its size and flow, the downpipes it needs,
//! and the cistern for reuse.

use super::{Plan, draw_building, label, wrap};
use crate::domain::construction::roof_plan::roof_plans;
use crate::domain::model::Project;
use crate::domain::plumbing::rainwater::rainwater;
use crate::io::construction::{Drawn, INK, MUTED, Sheet, decimal};

use super::points::TABLE_W;
use super::water::LOW;

/// Colour of the rainwater.
pub(crate) const RAIN: &str = "#2a8c8c";

pub(crate) fn rain_drawing(project: &Project) -> Option<Drawn> {
    let rain = rainwater(project)?;
    let plans = roof_plans(project);
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let boundary: Vec<_> = project
        .terrain
        .as_ref()
        .map(crate::domain::model::Terrain::outline)
        .unwrap_or_default();
    let extent = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(
            plans
                .iter()
                .flat_map(|p| p.planes.iter().flat_map(|q| q.polygon.clone())),
        )
        .chain(boundary.iter().copied());
    let plan = Plan::new(extent, (ax, ay, aw - TABLE_W - 6.0, ah));
    if boundary.len() >= 3 {
        let points: Vec<(f64, f64)> = boundary.iter().map(|p| plan.pt(*p)).collect();
        for i in 0..points.len() {
            sheet.line(
                points[i],
                points[(i + 1) % points.len()],
                MUTED,
                0.3,
                Some("4 1 1 1"),
            );
        }
    }
    for (room, _) in project.pool_specs() {
        if let Some(polygon) = project.room_polygon(room) {
            let points: Vec<(f64, f64)> = polygon.iter().map(|p| plan.pt(*p)).collect();
            sheet.polygon(&points, "#dff1fb", "#7fb8d8", 0.3);
        }
    }
    draw_building(&mut sheet, project, &plan);
    for roof in &plans {
        for plane in &roof.planes {
            let points: Vec<(f64, f64)> = plane.polygon.iter().map(|p| plan.pt(*p)).collect();
            sheet.polygon(&points, "none", INK, 0.25);
        }
    }
    for gutter in &rain.gutters {
        sheet.line(plan.pt(gutter.from), plan.pt(gutter.to), RAIN, 0.6, None);
        let (x, y) = plan.pt(gutter.from.midpoint(gutter.to));
        label(
            &mut sheet,
            (x + 0.8, y - 1.0),
            1.6,
            RAIN,
            &format!(
                "calha Ø{} · {} L/min",
                decimal(gutter.gutter_mm, 0),
                decimal(gutter.flow_l_min, 0)
            ),
        );
        for downpipe in &gutter.downpipes {
            let (x, y) = plan.pt(*downpipe);
            let _ = std::fmt::Write::write_fmt(
                &mut sheet.svg,
                format_args!(
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.2\" fill=\"#ffffff\" stroke=\"{RAIN}\" stroke-width=\"0.45\"/>\n"
                ),
            );
            label(
                &mut sheet,
                (x + 1.6, y + 2.4),
                1.5,
                RAIN,
                &format!("AP Ø{}", decimal(gutter.downpipe_mm, 0)),
            );
        }
    }

    let tx = ax + aw - TABLE_W;
    let mut ty = ay + 8.0;
    sheet.text(tx, ty, 3.0, true, INK, "Águas pluviais (NBR 10844)");
    ty += 5.0;
    sheet.text(
        tx,
        ty,
        2.1,
        false,
        INK,
        &format!(
            "Chuva de {} mm/h{} · {} m² de cobertura · {} L/min",
            decimal(rain.intensity_mm_h, 0),
            if rain.city_intensity { " (cidade)" } else { "" },
            decimal(rain.roof_area_m2, 1),
            decimal(rain.total_flow_l_min, 0)
        ),
    );
    ty += 6.0;
    let columns = [0.0, 34.0, 52.0, 70.0, 86.0, 102.0];
    for (i, head) in ["Calha", "Área (m²)", "Vazão", "Calha Ø", "Descidas", "Ø"]
        .iter()
        .enumerate()
    {
        sheet.text(tx + columns[i], ty, 2.0, true, MUTED, head);
    }
    ty += 1.4;
    sheet.line((tx, ty), (tx + TABLE_W - 2.0, ty), MUTED, 0.2, None);
    ty += 3.4;
    for gutter in &rain.gutters {
        let short = gutter.downpipes_needed > gutter.downpipes.len() && !rain.flat_slab;
        let roof: String = gutter.roof.chars().take(18).collect();
        let cells = [
            roof,
            decimal(gutter.area_m2, 1),
            format!("{} L/min", decimal(gutter.flow_l_min, 0)),
            format!("{} mm", decimal(gutter.gutter_mm, 0)),
            format!("{} / {}", gutter.downpipes.len(), gutter.downpipes_needed),
            format!("{} mm", decimal(gutter.downpipe_mm, 0)),
        ];
        for (i, cell) in cells.iter().enumerate() {
            let ink = if short && i == 4 { LOW } else { INK };
            sheet.text(tx + columns[i], ty, 2.0, false, ink, cell);
        }
        ty += 2.9;
    }
    ty += 3.0;
    sheet.text(tx, ty, 3.0, true, INK, "Cisterna (NBR 15527)");
    ty += 4.6;
    sheet.text(
        tx,
        ty,
        2.1,
        false,
        INK,
        &format!(
            "{} L · descarte das primeiras águas: {} L",
            decimal(rain.cistern_l, 0),
            decimal(rain.first_flush_l, 0)
        ),
    );
    ty += 6.0;
    for warning in &rain.warnings {
        for text in wrap(warning, 78) {
            sheet.text(tx, ty, 1.9, false, LOW, &text);
            ty += 2.6;
        }
        ty += 1.0;
    }
    for note in &rain.notes {
        for text in wrap(note, 78) {
            sheet.text(tx, ty, 1.8, false, MUTED, &text);
            ty += 2.5;
        }
        ty += 0.8;
    }

    Some(Drawn {
        sheet,
        title: "Águas pluviais".to_owned(),
        scale: plan.scale.label(),
    })
}
