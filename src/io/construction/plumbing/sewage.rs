//! The sewage plan (#120): the fixtures' drains, the floor traps, the
//! grease trap, the inspection boxes with their depths, the vents and the
//! main to the street sewer or the septic tank.

use super::{Plan, draw_building, label, wrap};
use crate::domain::model::Project;
use crate::domain::plumbing::points::PointKind;
use crate::domain::plumbing::septic::{SepticDesign, WELL_SOAKAWAY_M, WELL_TANK_M, septic_design};
use crate::domain::plumbing::sewage::{DeviceKind, DrainRole, Outfall, Sewage, sewage};
use crate::io::construction::{Drawn, INK, MUTED, Sheet, decimal};

use super::points::TABLE_W;
use super::water::LOW;

/// Draws the drains and the devices of the sewage design.
pub(crate) fn draw_sewage(sheet: &mut Sheet, plan: &Plan, design: &Sewage) {
    let brown = PointKind::Drain.hex();
    for drain in &design.drains {
        let (width, dash) = match drain.role {
            DrainRole::Fixture => (0.3, None),
            DrainRole::Branch => (0.45, None),
            DrainRole::Main => (0.6, Some("3 1")),
        };
        for leg in drain.path.windows(2) {
            sheet.line(plan.pt(leg[0]), plan.pt(leg[1]), &brown, width, dash);
        }
        if drain.role != DrainRole::Fixture
            && let Some(leg) = drain
                .path
                .windows(2)
                .max_by(|a, b| a[0].distance(a[1]).total_cmp(&b[0].distance(b[1])))
            && leg[0].distance(leg[1]) > 0.6
        {
            let (x, y) = plan.pt(leg[0].midpoint(leg[1]));
            label(
                sheet,
                (x + 0.6, y - 0.8),
                1.5,
                &brown,
                &format!("DN{} i={}%", drain.dn, decimal(drain.slope * 100.0, 0)),
            );
        }
    }
    for device in &design.devices {
        let (x, y) = plan.pt(device.at);
        match device.kind {
            DeviceKind::FloorTrap => {
                sheet.rect((x - 1.0, y - 1.0, 2.0, 2.0), "#ffffff", &brown, 0.35);
                let _ = std::fmt::Write::write_fmt(
                    &mut sheet.svg,
                    format_args!(
                        "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"0.6\" fill=\"none\" stroke=\"{brown}\" stroke-width=\"0.25\"/>\n"
                    ),
                );
                label(sheet, (x + 1.4, y + 2.4), 1.6, &brown, &device.label);
            }
            DeviceKind::GreaseTrap | DeviceKind::InspectionBox => {
                let fill = if device.kind == DeviceKind::GreaseTrap {
                    "#f3e6d8"
                } else {
                    "#ffffff"
                };
                sheet.rect((x - 1.5, y - 1.5, 3.0, 3.0), fill, &brown, 0.4);
                let depth = device
                    .depth_m
                    .map_or(String::new(), |d| format!(" −{}", decimal(d, 2)));
                label(
                    sheet,
                    (x + 2.0, y - 1.2),
                    1.7,
                    &brown,
                    &format!("{}{depth}", device.label),
                );
            }
            DeviceKind::Vent => {
                let _ = std::fmt::Write::write_fmt(
                    &mut sheet.svg,
                    format_args!(
                        "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.1\" fill=\"#ffffff\" stroke=\"#3d8b4f\" stroke-width=\"0.4\"/>\n"
                    ),
                );
                label(
                    sheet,
                    (x - 7.0, y - 1.6),
                    1.6,
                    "#3d8b4f",
                    &format!("{} DN{}", device.label, device.dn),
                );
            }
        }
    }
    let (x, y) = plan.pt(design.exit_at);
    let text = match design.outfall {
        Outfall::PublicSewer => "Rede pública de esgoto",
        Outfall::Septic => "Fossa séptica",
    };
    sheet.polygon(
        &[(x - 1.6, y - 1.6), (x + 1.6, y - 1.6), (x, y + 1.2)],
        &brown,
        &brown,
        0.2,
    );
    label(sheet, (x + 2.2, y + 0.6), 1.9, &brown, text);
}

fn circle(
    sheet: &mut Sheet,
    (x, y): (f64, f64),
    r: f64,
    fill: &str,
    stroke: &str,
    width: f64,
    dash: Option<&str>,
) {
    let dash = dash.map_or(String::new(), |d| format!(" stroke-dasharray=\"{d}\""));
    let _ = std::fmt::Write::write_fmt(
        &mut sheet.svg,
        format_args!(
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{width:.2}\"{dash}/>\n"
        ),
    );
}

/// The septic tank, the filter and the soakaways where they were placed,
/// joined in order, and every well with the 15 m and 20 m it keeps clear.
pub(crate) fn draw_septic(
    sheet: &mut Sheet,
    plan: &Plan,
    project: &Project,
    design: &SepticDesign,
) {
    let brown = PointKind::Drain.hex();
    let water = "#3d8b4f";
    for (index, well) in project.plumbing.wells.iter().enumerate() {
        let at = plan.pt(*well);
        circle(
            sheet,
            at,
            plan.mm(WELL_SOAKAWAY_M),
            "none",
            water,
            0.25,
            Some("2 1.5"),
        );
        circle(
            sheet,
            at,
            plan.mm(WELL_TANK_M),
            "none",
            water,
            0.25,
            Some("1 1"),
        );
        circle(sheet, at, 1.4, "#e6f4ea", water, 0.4, None);
        label(
            sheet,
            (at.0 + 1.8, at.1 - 1.2),
            1.8,
            water,
            &format!("Poço {}", index + 1),
        );
    }
    let Some(settings) = project.plumbing.septic.as_ref() else {
        return;
    };
    let mut chain = Vec::new();
    if let Some(tank) = settings.tank_at {
        let at = plan.pt(tank);
        circle(
            sheet,
            at,
            plan.mm(design.diameter_m / 2.0).max(1.0),
            "#f3e6d8",
            &brown,
            0.45,
            None,
        );
        label(
            sheet,
            (at.0 + plan.mm(design.diameter_m / 2.0) + 1.0, at.1),
            1.8,
            &brown,
            &format!("Tanque séptico {} L", decimal(design.volume_l, 0)),
        );
        chain.push(tank);
    }
    if design.filter
        && let Some(filter) = settings.filter_at
    {
        let at = plan.pt(filter);
        circle(
            sheet,
            at,
            plan.mm(design.filter_diameter_m / 2.0).max(1.0),
            "#efe7dc",
            &brown,
            0.4,
            Some("1 0.6"),
        );
        label(
            sheet,
            (
                at.0 + plan.mm(design.filter_diameter_m / 2.0) + 1.0,
                at.1 + 2.4,
            ),
            1.7,
            &brown,
            &format!("Filtro anaeróbio {} L", decimal(design.filter_volume_l, 0)),
        );
        chain.push(filter);
    }
    for (index, soakaway) in settings.soakaways_at.iter().enumerate() {
        let at = plan.pt(*soakaway);
        circle(
            sheet,
            at,
            plan.mm(design.soakaway_diameter_m / 2.0).max(1.0),
            "#ffffff",
            &brown,
            0.4,
            Some("0.8 0.5"),
        );
        label(
            sheet,
            (
                at.0 + plan.mm(design.soakaway_diameter_m / 2.0) + 1.0,
                at.1 + 0.6,
            ),
            1.7,
            &brown,
            &format!(
                "Sumidouro {} · {} m",
                index + 1,
                decimal(design.soakaway_depth_m, 1)
            ),
        );
        if let Some(previous) = chain.last() {
            sheet.line(plan.pt(*previous), at, &brown, 0.35, Some("3 1"));
        }
    }
    for pair in chain.windows(2) {
        sheet.line(
            plan.pt(pair[0]),
            plan.pt(pair[1]),
            &brown,
            0.35,
            Some("3 1"),
        );
    }
}

pub(crate) fn sewage_drawing(project: &Project) -> Option<Drawn> {
    let design = sewage(project)?;
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let septic = septic_design(project);
    let septic_points: Vec<_> = project
        .plumbing
        .septic
        .iter()
        .flat_map(|s| {
            s.tank_at
                .into_iter()
                .chain(s.filter_at)
                .chain(s.soakaways_at.clone())
        })
        .chain(project.plumbing.wells.clone())
        .collect();
    let extent = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(design.devices.iter().map(|d| d.at))
        .chain([design.exit_at])
        .chain(septic_points);
    let plan = Plan::new(extent, (ax, ay, aw - TABLE_W - 6.0, ah));
    draw_building(&mut sheet, project, &plan);
    if let Some(septic) = &septic {
        draw_septic(&mut sheet, &plan, project, septic);
    } else if !project.plumbing.wells.is_empty() {
        for (index, well) in project.plumbing.wells.iter().enumerate() {
            let at = plan.pt(*well);
            circle(&mut sheet, at, 1.4, "#e6f4ea", "#3d8b4f", 0.4, None);
            label(
                &mut sheet,
                (at.0 + 1.8, at.1 - 1.2),
                1.8,
                "#3d8b4f",
                &format!("Poço {}", index + 1),
            );
        }
    }
    draw_sewage(&mut sheet, &plan, &design);

    let tx = ax + aw - TABLE_W;
    let mut ty = ay + 8.0;
    sheet.text(tx, ty, 3.0, true, INK, "Esgoto (NBR 8160)");
    ty += 5.0;
    let brown = PointKind::Drain.hex();
    for (width, dash, text) in [
        (0.3, None, "Ramal de descarga (aparelho)"),
        (0.45, None, "Ramal de esgoto (cômodo)"),
        (0.6, Some("3 1"), "Coletor enterrado"),
    ] {
        sheet.line((tx, ty - 0.8), (tx + 8.0, ty - 0.8), &brown, width, dash);
        sheet.text(tx + 10.0, ty, 2.0, false, INK, text);
        ty += 3.4;
    }
    sheet.text(
        tx,
        ty,
        2.0,
        false,
        MUTED,
        "CS caixa sifonada · CG caixa de gordura · CI caixa de inspeção (−profundidade, m) · TV tubo ventilador",
    );
    ty += 6.0;
    sheet.text(tx, ty, 3.0, true, INK, "Caixas e ventilação");
    ty += 5.0;
    let columns = [0.0, 14.0, 50.0, 84.0, 100.0];
    for (i, head) in ["", "Tipo", "Cômodo", "DN", "Prof. (m)"].iter().enumerate() {
        sheet.text(tx + columns[i], ty, 2.0, true, MUTED, head);
    }
    ty += 1.4;
    sheet.line((tx, ty), (tx + TABLE_W - 2.0, ty), MUTED, 0.2, None);
    ty += 3.4;
    for device in &design.devices {
        let room: String = device
            .room
            .clone()
            .unwrap_or_default()
            .chars()
            .take(20)
            .collect();
        let cells = [
            device.label.clone(),
            device.kind.label().to_owned(),
            room,
            format!("{}", device.dn),
            device.depth_m.map_or("—".to_owned(), |d| decimal(d, 2)),
        ];
        for (i, cell) in cells.iter().enumerate() {
            sheet.text(tx + columns[i], ty, 2.0, false, INK, cell);
        }
        ty += 2.9;
    }
    ty += 3.0;
    if let Some(septic) = &septic {
        sheet.text(
            tx,
            ty,
            3.0,
            true,
            INK,
            "Fossa séptica (NBR 7229, NBR 13969)",
        );
        ty += 4.6;
        let mut lines = vec![
            format!(
                "Tanque de {} L úteis: anéis de {} m × {} m de água, ou {} × {} m com {} m",
                decimal(septic.volume_l, 0),
                decimal(septic.diameter_m, 2),
                decimal(septic.cylinder_depth_m, 2),
                decimal(septic.length_m, 2),
                decimal(septic.width_m, 2),
                decimal(septic.depth_m, 2)
            ),
            format!(
                "{} sumidouro(s) de {} m, {} m de altura útil ({} m²)",
                septic.soakaway_count,
                decimal(septic.soakaway_diameter_m, 2),
                decimal(septic.soakaway_height_m, 1),
                decimal(septic.area_needed_m2, 1)
            ),
        ];
        if septic.filter {
            lines.push(format!(
                "Filtro anaeróbio de {} L, {} m de diâmetro",
                decimal(septic.filter_volume_l, 0),
                decimal(septic.filter_diameter_m, 2)
            ));
        }
        for line in lines {
            sheet.text(tx, ty, 2.0, false, INK, &line);
            ty += 3.0;
        }
        for distance in septic.distances.iter().filter(|d| !d.ok()) {
            let text = format!(
                "{} a {} m da {}: mínimo {} m",
                distance.element,
                decimal(distance.distance_m, 2),
                distance.to,
                decimal(distance.normative_m, 1)
            );
            sheet.text(tx, ty, 1.9, false, LOW, &text);
            ty += 2.8;
        }
        for note in septic
            .notes
            .iter()
            .filter(|n| n.contains("poço") || n.contains("Filtro") || n.contains("filtro"))
        {
            for text in wrap(note, 78) {
                sheet.text(tx, ty, 1.8, false, MUTED, &text);
                ty += 2.5;
            }
        }
        ty += 3.0;
    }
    for warning in &design.warnings {
        for text in wrap(warning, 78) {
            sheet.text(tx, ty, 1.9, false, LOW, &text);
            ty += 2.6;
        }
        ty += 1.0;
    }
    for note in &design.notes {
        for text in wrap(note, 78) {
            sheet.text(tx, ty, 1.8, false, MUTED, &text);
            ty += 2.5;
        }
        ty += 0.8;
    }

    Some(Drawn {
        sheet,
        title: if septic.is_some() {
            "Esgoto sanitário e fossa séptica".to_owned()
        } else {
            "Esgoto sanitário".to_owned()
        },
        scale: plan.scale.label(),
    })
}
