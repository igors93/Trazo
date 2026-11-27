//! The water plan (#118): the water tank, the headers, the risers and the
//! branches with their DN, and the pressure left at every fixture, green
//! when good, amber when weak and red below the least of NBR 5626.

use super::{Plan, draw_building, label, wrap};
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::plumbing::cold_water::{ColdWater, cold_water};
use crate::domain::plumbing::hot_water::hot_water;
use crate::domain::plumbing::network::{Network, PipeRole, Pressure};
use crate::domain::plumbing::points::PointKind;
use crate::io::construction::{Drawn, INK, MUTED, Sheet, decimal};

use super::points::TABLE_W;

pub(crate) const GOOD: &str = "#2e7d32";
pub(crate) const WEAK: &str = "#c77700";
pub(crate) const LOW: &str = "#c0392b";

pub(crate) fn pressure_color(state: Pressure) -> &'static str {
    match state {
        Pressure::Good => GOOD,
        Pressure::Weak => WEAK,
        Pressure::Low => LOW,
    }
}

/// Draws a network's pipes in `color`: headers dashed (they run over the
/// slab), branches solid, each riser as a ring with its label, and the DN
/// of every horizontal run.
pub(crate) fn draw_network(sheet: &mut Sheet, plan: &Plan, network: &Network, color: &str) {
    for pipe in &network.pipes {
        let dash = match pipe.role {
            PipeRole::Header => Some("2.4 1.2"),
            PipeRole::Branch => None,
            PipeRole::Riser | PipeRole::Drop => continue,
        };
        let width = if pipe.role == PipeRole::Header {
            0.45
        } else {
            0.35
        };
        for leg in pipe.path.windows(2) {
            if leg[0].at.distance(leg[1].at) > 1e-6 {
                sheet.line(plan.pt(leg[0].at), plan.pt(leg[1].at), color, width, dash);
            }
        }
        // The DN on the longest leg.
        if let Some(leg) = pipe.path.windows(2).max_by(|a, b| {
            a[0].at
                .distance(a[1].at)
                .total_cmp(&b[0].at.distance(b[1].at))
        }) && leg[0].at.distance(leg[1].at) > 0.4
        {
            let mid = leg[0].at.midpoint(leg[1].at);
            let (x, y) = plan.pt(mid);
            label(
                sheet,
                (x + 0.6, y - 0.8),
                1.6,
                color,
                &format!("Ø{}", pipe.dn),
            );
        }
    }
    for group in &network.groups {
        let (x, y) = plan.pt(group.riser_at);
        let _ = std::fmt::Write::write_fmt(
            &mut sheet.svg,
            format_args!(
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.6\" fill=\"#ffffff\" stroke=\"{color}\" stroke-width=\"0.45\"/>\n<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"0.5\" fill=\"{color}\"/>\n"
            ),
        );
        label(sheet, (x - 4.0, y - 2.4), 1.8, color, &group.label);
    }
}

/// The tank: a square of 1 m with its volume and base height.
fn draw_tank(sheet: &mut Sheet, plan: &Plan, cold: &ColdWater) {
    let half = 0.5;
    let (x0, y0) = plan.pt(Point::new(cold.tank_at.x - half, cold.tank_at.y - half));
    let side = plan.mm(2.0 * half);
    sheet.rect(
        (x0, y0, side, side),
        "#e3f0fb",
        &PointKind::Cold.hex(),
        0.45,
    );
    sheet.line(
        (x0, y0),
        (x0 + side, y0 + side),
        &PointKind::Cold.hex(),
        0.2,
        None,
    );
    sheet.line(
        (x0 + side, y0),
        (x0, y0 + side),
        &PointKind::Cold.hex(),
        0.2,
        None,
    );
    label(
        sheet,
        (x0 + side + 1.0, y0 + 1.5),
        2.0,
        INK,
        &format!("Caixa-d'água {} L", decimal(cold.tank_l, 0)),
    );
    label(
        sheet,
        (x0 + side + 1.0, y0 + 4.0),
        1.7,
        MUTED,
        &format!("base +{} m", decimal(cold.tank_base_m, 2)),
    );
}

pub(crate) fn water_drawing(project: &Project) -> Option<Drawn> {
    let cold = cold_water(project)?;
    let network = &cold.network;
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let extent = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(network.outlets.iter().map(|o| o.node.at))
        .chain([cold.tank_at]);
    let plan = Plan::new(extent, (ax, ay, aw - TABLE_W - 6.0, ah));
    draw_building(&mut sheet, project, &plan);
    let blue = PointKind::Cold.hex();
    draw_network(&mut sheet, &plan, network, &blue);
    draw_tank(&mut sheet, &plan, &cold);
    let hot = hot_water(project);
    let red = PointKind::Hot.hex();
    if let Some(hot) = &hot
        && let Some(hot_network) = &hot.network
    {
        draw_network(&mut sheet, &plan, hot_network, &red);
        if let Some(at) = hot.heater_at {
            let (x, y) = plan.pt(at);
            sheet.rect((x - 2.0, y - 2.0, 4.0, 4.0), "#fdecea", &red, 0.45);
            label(
                &mut sheet,
                (x + 2.6, y + 0.8),
                1.9,
                &red,
                &match hot.heater {
                    crate::domain::plumbing::Heater::Gas => {
                        format!("Aquecedor a gás {} L/min", decimal(hot.gas_heater_l_min, 1))
                    }
                    _ => format!("Reservatório solar {} L", decimal(hot.boiler_l, 0)),
                },
            );
        }
    }
    for outlet in &network.outlets {
        let (x, y) = plan.pt(outlet.node.at);
        let color = pressure_color(outlet.state);
        let _ = std::fmt::Write::write_fmt(
            &mut sheet.svg,
            format_args!(
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"0.9\" fill=\"{color}\" stroke=\"#ffffff\" stroke-width=\"0.25\"/>\n"
            ),
        );
        label(
            &mut sheet,
            (x + 1.3, y + 2.6),
            1.7,
            color,
            &format!("{} m", decimal(outlet.pressure_m, 2)),
        );
    }

    let tx = ax + aw - TABLE_W;
    let mut ty = ay + 8.0;
    sheet.text(tx, ty, 3.0, true, INK, "Água fria (NBR 5626)");
    ty += 5.0;
    for line in [
        format!(
            "{} morador(es) × {} L/dia = {} L/dia",
            cold.residents,
            decimal(cold.per_person_l, 0),
            decimal(cold.daily_l, 0)
        ),
        format!(
            "Reserva de {} dia(s): caixa de {} L ({} kN cheia)",
            decimal(cold.reserve_days, 1),
            decimal(cold.tank_l, 0),
            decimal(cold.tank_full_kn, 1)
        ),
        format!(
            "Base da caixa +{} m · necessária +{} m · banho forte +{} m",
            decimal(cold.tank_base_m, 2),
            decimal(cold.needed_base_m, 2),
            decimal(cold.comfort_base_m, 2)
        ),
    ] {
        sheet.text(tx, ty, 2.2, false, INK, &line);
        ty += 3.6;
    }
    ty += 2.0;
    // Legend of the lines.
    let line_y = ty - 0.8;
    sheet.line(
        (tx, line_y),
        (tx + 8.0, line_y),
        &blue,
        0.45,
        Some("2.4 1.2"),
    );
    sheet.text(tx + 10.0, ty, 2.0, false, INK, "Barrilete (sobre a laje)");
    sheet.line((tx + 50.0, line_y), (tx + 58.0, line_y), &blue, 0.35, None);
    sheet.text(tx + 60.0, ty, 2.0, false, INK, "Ramal (na parede)");
    ty += 3.6;
    for (color, text) in [
        (GOOD, "pressão boa"),
        (WEAK, "chuveiro fraco"),
        (LOW, "abaixo do mínimo"),
    ] {
        let _ = std::fmt::Write::write_fmt(
            &mut sheet.svg,
            format_args!(
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"0.9\" fill=\"{color}\"/>\n",
                tx + 1.0,
                ty - 0.8
            ),
        );
        sheet.text(tx + 3.5, ty, 2.0, false, INK, text);
        ty += 3.2;
    }
    ty += 3.0;

    sheet.text(tx, ty, 3.0, true, INK, "Pressão nos aparelhos");
    ty += 5.0;
    let columns = [0.0, 30.0, 70.0, 84.0, 100.0];
    for (i, head) in ["Aparelho", "Cômodo", "h (m)", "Pressão", ""]
        .iter()
        .enumerate()
    {
        sheet.text(tx + columns[i], ty, 2.0, true, MUTED, head);
    }
    ty += 1.4;
    sheet.line((tx, ty), (tx + TABLE_W - 2.0, ty), MUTED, 0.2, None);
    ty += 3.4;
    for outlet in &network.outlets {
        let color = pressure_color(outlet.state);
        let room: String = outlet
            .room
            .clone()
            .unwrap_or_default()
            .chars()
            .take(24)
            .collect();
        let cells = [
            outlet.fixture.label().to_owned(),
            room,
            decimal(outlet.node.z, 2),
            format!("{} m", decimal(outlet.pressure_m, 2)),
            outlet.state.label().to_owned(),
        ];
        for (i, cell) in cells.iter().enumerate() {
            let ink = if i >= 3 { color } else { INK };
            sheet.text(tx + columns[i], ty, 2.0, false, ink, cell);
        }
        ty += 3.0;
    }
    ty += 3.0;
    if let Some(hot) = &hot {
        sheet.text(
            tx,
            ty,
            3.0,
            true,
            INK,
            &format!("Água quente · {}", hot.heater.label()),
        );
        ty += 4.6;
        for line in &hot.notes {
            for text in wrap(line, 78) {
                sheet.text(tx, ty, 1.9, false, INK, &text);
                ty += 2.6;
            }
            ty += 0.8;
        }
        for shower in hot
            .showers
            .iter()
            .filter(|s| hot.electrical_design && !s.ok())
        {
            let text = format!(
                "Chuveiro ({}) sem tomada própria suficiente no projeto elétrico",
                shower.room.clone().unwrap_or_default()
            );
            sheet.text(tx, ty, 1.9, false, LOW, &text);
            ty += 2.8;
        }
        if let Some(network) = &hot.network {
            for outlet in &network.outlets {
                let text = format!(
                    "AQ {}{}: {} m ({})",
                    outlet.fixture.label().to_lowercase(),
                    outlet
                        .room
                        .as_ref()
                        .map_or(String::new(), |r| format!(" · {r}")),
                    decimal(outlet.pressure_m, 2),
                    outlet.state.label()
                );
                sheet.text(tx, ty, 1.9, false, pressure_color(outlet.state), &text);
                ty += 2.8;
            }
        }
        ty += 2.0;
    }
    sheet.text(tx, ty, 3.0, true, INK, "Tubos de PVC soldável");
    ty += 4.6;
    let lengths = network
        .lengths_by_dn()
        .iter()
        .map(|(dn, m)| format!("Ø{dn}: {} m", decimal(*m, 1)))
        .collect::<Vec<_>>()
        .join(" · ");
    for text in wrap(&lengths, 70) {
        sheet.text(tx, ty, 2.1, false, INK, &text);
        ty += 3.2;
    }
    ty += 2.4;
    for warning in &cold.warnings {
        for text in wrap(warning, 78) {
            sheet.text(tx, ty, 1.9, false, LOW, &text);
            ty += 2.6;
        }
        ty += 1.0;
    }
    for note in &cold.notes {
        for text in wrap(note, 78) {
            sheet.text(tx, ty, 1.8, false, MUTED, &text);
            ty += 2.5;
        }
        ty += 0.8;
    }

    let lengths_hot = hot
        .as_ref()
        .and_then(|h| h.network.as_ref())
        .map(|n| n.lengths_by_dn());
    if let Some(lengths) = lengths_hot {
        let text = lengths
            .iter()
            .map(|(dn, m)| format!("Ø{dn}: {} m", decimal(*m, 1)))
            .collect::<Vec<_>>()
            .join(" · ");
        sheet.text(
            tx,
            ty,
            2.0,
            false,
            &red,
            &format!("Água quente (PPR ou CPVC): {text}"),
        );
    }
    let titled_hot = hot.as_ref().is_some_and(|h| h.network.is_some());
    Some(Drawn {
        sheet,
        title: if titled_hot {
            "Água fria e quente".to_owned()
        } else {
            "Água fria".to_owned()
        },
        scale: plan.scale.label(),
    })
}
