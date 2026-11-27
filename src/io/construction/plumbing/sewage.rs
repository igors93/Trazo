//! The sewage plan (#120): the fixtures' drains, the floor traps, the
//! grease trap, the inspection boxes with their depths, the vents and the
//! main to the street sewer or the septic tank.

use super::{Plan, draw_building, label, wrap};
use crate::domain::model::Project;
use crate::domain::plumbing::points::PointKind;
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

pub(crate) fn sewage_drawing(project: &Project) -> Option<Drawn> {
    let design = sewage(project)?;
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let extent = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(design.devices.iter().map(|d| d.at))
        .chain([design.exit_at]);
    let plan = Plan::new(extent, (ax, ay, aw - TABLE_W - 6.0, ah));
    draw_building(&mut sheet, project, &plan);
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
        title: "Esgoto sanitário".to_owned(),
        scale: plan.scale.label(),
    })
}
