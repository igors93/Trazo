//! The plumbing isometric and riser diagrams (#123), drawn from the model:
//!
//! - the **isometric** (30°, not to scale) of the cold-water network, and of
//!   the hot one when the house has it: the tank, the headers, each riser
//!   with its label, the branches and drops with their DN, and every
//!   outlet with its fixture and the pressure left there;
//! - the **riser diagram** (*esquema vertical*): the heights, from the
//!   tank's base down to each room's outlets, the vents rising over the
//!   roof and the inspection boxes under the ground.

use std::fmt::Write as _;

use super::water::pressure_color;
use super::{label, wrap};
use crate::domain::model::Project;
use crate::domain::plumbing::cold_water::cold_water;
use crate::domain::plumbing::hot_water::hot_water;
use crate::domain::plumbing::network::{Network, Node, PipeRole};
use crate::domain::plumbing::points::PointKind;
use crate::domain::plumbing::sewage::{DeviceKind, sewage};
use crate::io::construction::{Drawn, INK, MUTED, Sheet, decimal};

const COS30: f64 = 0.866_025_403_784_438_6;
const SIN30: f64 = 0.5;

/// The isometric projection of a node, in metres on the drawing.
fn iso(node: Node) -> (f64, f64) {
    (
        (node.at.x - node.at.y) * COS30,
        (node.at.x + node.at.y) * SIN30 - node.z,
    )
}

/// Every node the networks pass through.
fn nodes<'a>(networks: impl Iterator<Item = &'a Network>) -> Vec<Node> {
    networks
        .flat_map(|n| n.pipes.iter().flat_map(|p| p.path.clone()))
        .collect()
}

/// Draws one network into the isometric frame.
fn draw_iso(
    sheet: &mut Sheet,
    network: &Network,
    to: &dyn Fn(Node) -> (f64, f64),
    color: &str,
    labels: bool,
) {
    for pipe in &network.pipes {
        let width = match pipe.role {
            PipeRole::Header | PipeRole::Riser => 0.5,
            PipeRole::Branch => 0.4,
            PipeRole::Drop => 0.3,
        };
        let points: Vec<String> = pipe
            .path
            .iter()
            .map(|n| {
                let (x, y) = to(*n);
                format!("{x:.2},{y:.2}")
            })
            .collect();
        let _ = writeln!(
            sheet.svg,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width:.2}\" stroke-linejoin=\"round\"/>",
            points.join(" ")
        );
        if labels
            && pipe.length_m > 0.6
            && let Some(leg) = pipe.path.windows(2).max_by(|a, b| {
                let la = a[0].at.distance(a[1].at) + (a[0].z - a[1].z).abs();
                let lb = b[0].at.distance(b[1].at) + (b[0].z - b[1].z).abs();
                la.total_cmp(&lb)
            })
        {
            let mid = Node::new(leg[0].at.midpoint(leg[1].at), (leg[0].z + leg[1].z) / 2.0);
            let (x, y) = to(mid);
            label(
                sheet,
                (x + 0.8, y - 0.6),
                1.5,
                color,
                &format!("Ø{}", pipe.dn),
            );
        }
    }
    if labels {
        for group in &network.groups {
            if let Some(riser) = network
                .pipes
                .iter()
                .find(|p| p.role == PipeRole::Riser && network.groups.get(p.group) == Some(group))
                && let Some(top) = riser.path.first()
            {
                let (x, y) = to(*top);
                label(sheet, (x - 7.0, y - 1.2), 1.8, color, &group.label);
            }
        }
    }
}

/// The isometric and the riser diagram of the plumbing, on one sheet.
pub(crate) fn isometric_drawing(project: &Project) -> Option<Drawn> {
    let cold = cold_water(project)?;
    let hot = hot_water(project).and_then(|h| h.network);
    let drains = sewage(project);
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let blue = PointKind::Cold.hex();
    let red = PointKind::Hot.hex();
    let brown = PointKind::Drain.hex();

    // The isometric, on the left two thirds.
    let iso_w = aw * 0.62;
    sheet.text(
        ax + 6.0,
        ay + 8.0,
        3.0,
        true,
        INK,
        "Isométrico de água fria e quente (sem escala)",
    );
    let all = nodes(std::iter::once(&cold.network).chain(hot.iter()));
    let projected: Vec<(f64, f64)> = all.iter().map(|n| iso(*n)).collect();
    let (min_x, max_x) = projected
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.0), b.max(p.0))
        });
    let (min_y, max_y) = projected
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.1), b.max(p.1))
        });
    let (box_x, box_y, box_w, box_h) = (ax + 10.0, ay + 16.0, iso_w - 20.0, ah - 30.0);
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);
    let k = (box_w / span_x).min(box_h / span_y);
    let ox = box_x + (box_w - span_x * k) / 2.0;
    let oy = box_y + (box_h - span_y * k) / 2.0;
    let to = |n: Node| {
        let (x, y) = iso(n);
        (ox + (x - min_x) * k, oy + (y - min_y) * k)
    };
    if let Some(hot) = &hot {
        draw_iso(&mut sheet, hot, &to, &red, true);
    }
    draw_iso(&mut sheet, &cold.network, &to, &blue, true);
    // The tank: a small box over the source.
    let (tx, ty) = to(cold.network.source);
    sheet.rect((tx - 4.0, ty - 6.0, 8.0, 6.0), "#e3f0fb", &blue, 0.45);
    label(
        &mut sheet,
        (tx + 5.0, ty - 2.0),
        1.9,
        INK,
        &format!(
            "Caixa-d'água {} L · base +{} m",
            decimal(cold.tank_l, 0),
            decimal(cold.tank_base_m, 2)
        ),
    );
    for outlet in &cold.network.outlets {
        let (x, y) = to(outlet.node);
        let color = pressure_color(outlet.state);
        let _ = writeln!(
            sheet.svg,
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"0.8\" fill=\"{color}\"/>"
        );
        label(
            &mut sheet,
            (x + 1.2, y + 2.2),
            1.6,
            color,
            &format!(
                "{} h{} · {} m",
                outlet.fixture.label(),
                decimal(outlet.node.z, 2),
                decimal(outlet.pressure_m, 2)
            ),
        );
    }

    // The riser diagram, on the right third.
    let rx = ax + iso_w + 4.0;
    let rw = aw - iso_w - 8.0;
    sheet.line(
        (rx - 2.0, ay + 4.0),
        (rx - 2.0, ay + ah - 4.0),
        MUTED,
        0.2,
        Some("1 1"),
    );
    sheet.text(rx + 2.0, ay + 8.0, 3.0, true, INK, "Esquema vertical");
    let roof_z = project.wall_height + 0.6;
    let top_z = cold.tank_base_m.max(roof_z) + 1.2;
    let bottom_z = -drains.as_ref().map_or(0.6, |d| {
        d.devices
            .iter()
            .filter_map(|x| x.depth_m)
            .fold(0.6, f64::max)
    }) - 0.4;
    let (diagram_top, diagram_bottom) = (ay + 18.0, ay + ah - 46.0);
    let vz =
        |z: f64| diagram_top + (top_z - z) / (top_z - bottom_z) * (diagram_bottom - diagram_top);
    // The ground and the slab.
    sheet.line((rx, vz(0.0)), (rx + rw, vz(0.0)), INK, 0.4, None);
    label(&mut sheet, (rx, vz(0.0) - 0.8), 1.6, MUTED, "piso 0,00");
    sheet.line(
        (rx, vz(project.wall_height)),
        (rx + rw, vz(project.wall_height)),
        MUTED,
        0.25,
        Some("3 1"),
    );
    label(
        &mut sheet,
        (rx, vz(project.wall_height) - 0.8),
        1.6,
        MUTED,
        &format!("laje +{}", decimal(project.wall_height, 2)),
    );
    // The tank and the header along the top.
    let groups = cold.network.groups.len().max(1);
    let column_x = |i: usize| rx + 22.0 + (rw - 30.0) * (i as f64 + 0.5) / groups as f64;
    let tank_y = vz(cold.tank_base_m);
    sheet.rect((rx + 2.0, tank_y - 6.0, 14.0, 6.0), "#e3f0fb", &blue, 0.45);
    label(
        &mut sheet,
        (rx + 2.0, tank_y - 7.0),
        1.6,
        INK,
        &format!(
            "{} L +{}",
            decimal(cold.tank_l, 0),
            decimal(cold.tank_base_m, 2)
        ),
    );
    sheet.line(
        (rx + 16.0, tank_y),
        (column_x(groups - 1), tank_y),
        &blue,
        0.45,
        None,
    );
    for (i, group) in cold.network.groups.iter().enumerate() {
        let x = column_x(i);
        let outlets: Vec<_> = cold
            .network
            .outlets
            .iter()
            .filter(|o| {
                o.path
                    .last()
                    .is_some_and(|p| cold.network.pipes[*p].group == i)
            })
            .collect();
        let low = outlets
            .iter()
            .map(|o| o.node.z)
            .fold(f64::INFINITY, f64::min);
        sheet.line(
            (x, tank_y),
            (x, vz(low.min(cold.tank_base_m))),
            &blue,
            0.45,
            None,
        );
        label(
            &mut sheet,
            (x + 0.8, tank_y + 3.0),
            1.7,
            &blue,
            &group.label,
        );
        if let Some(room) = &group.room {
            let name: String = room.chars().take(14).collect();
            label(&mut sheet, (x - 6.0, vz(bottom_z) + 4.0), 1.5, MUTED, &name);
        }
        for outlet in outlets {
            let y = vz(outlet.node.z);
            let color = pressure_color(outlet.state);
            sheet.line((x, y), (x + 3.0, y), &blue, 0.3, None);
            let _ = writeln!(
                sheet.svg,
                "<circle cx=\"{:.2}\" cy=\"{y:.2}\" r=\"0.6\" fill=\"{color}\"/>",
                x + 3.0
            );
            label(
                &mut sheet,
                (x + 4.0, y + 0.6),
                1.4,
                color,
                &format!("{} {}", outlet.fixture.label(), decimal(outlet.node.z, 2)),
            );
        }
    }
    // Vents and boxes.
    if let Some(drains) = &drains {
        let vents: Vec<_> = drains
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Vent)
            .collect();
        for (i, vent) in vents.iter().enumerate() {
            let x = rx + rw - 6.0 - i as f64 * 5.0;
            sheet.line(
                (x, vz(0.0)),
                (x, vz(roof_z + 0.3)),
                "#3d8b4f",
                0.4,
                Some("2 0.8"),
            );
            label(
                &mut sheet,
                (x - 3.0, vz(roof_z + 0.3) - 1.0),
                1.5,
                "#3d8b4f",
                &vent.label,
            );
        }
        let boxes: Vec<_> = drains
            .devices
            .iter()
            .filter(|d| matches!(d.kind, DeviceKind::InspectionBox | DeviceKind::GreaseTrap))
            .collect();
        for (i, device) in boxes.iter().enumerate() {
            let depth = device.depth_m.unwrap_or(0.4);
            let x = rx + 4.0 + i as f64 * (rw - 8.0) / boxes.len().max(1) as f64;
            sheet.rect(
                (x, vz(0.0), 4.0, vz(-depth) - vz(0.0)),
                "#f3e6d8",
                &brown,
                0.3,
            );
            label(
                &mut sheet,
                (x, vz(-depth) + 2.4),
                1.4,
                &brown,
                &format!("{} −{}", device.label, decimal(depth, 2)),
            );
        }
    }
    let mut ny = ay + ah - 34.0;
    for note in [
        "Isométrico gerado do modelo: tubos com DN e pressão em cada ponto (verde boa, âmbar fraca, vermelho abaixo do mínimo).",
        "Esquema vertical: alturas em metros a partir do piso; tubos ventiladores 30 cm acima do telhado; caixas com a profundidade da saída.",
    ] {
        for text in wrap(note, 60) {
            sheet.text(rx, ny, 1.7, false, MUTED, &text);
            ny += 2.4;
        }
        ny += 0.8;
    }

    Some(Drawn {
        sheet,
        title: "Isométrico e esquema vertical".to_owned(),
        scale: "sem escala".to_owned(),
    })
}

/// The isometric sheet alone, as SVG.
pub fn isometric_svg(project: &Project) -> Result<String, String> {
    let drawn = isometric_drawing(project).ok_or_else(|| super::NO_FIXTURES.to_owned())?;
    Ok(crate::io::construction::finish(project, drawn, "01/01"))
}
