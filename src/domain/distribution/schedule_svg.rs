//! Panel load schedule table and single-line diagram (diagrama unifilar) SVG.

use std::fmt::Write;

use crate::domain::{model::Project, schedules::ScheduleTable};

use super::{decimal, esc, phase_label, section_label};

/// The panel schedule for drawing sheets.
pub fn build_panel_schedule(project: &Project) -> ScheduleTable {
    let schedule = project.load_schedule();
    let rows = schedule
        .circuits
        .iter()
        .map(|row| {
            vec![
                format!("C{}", row.number),
                row.kind.short().to_owned(),
                phase_label(&row.phases),
                format!("{:.0}", row.power_va),
                decimal(row.current_a, 1),
                row.wire_mm2.map_or("—".to_owned(), section_label),
                row.breaker_a.map_or("—".to_owned(), |b| format!("{b:.0}")),
                if row.needs_rcd { "Sim" } else { "—" }.to_owned(),
            ]
        })
        .collect();
    let main = schedule
        .entrance
        .and_then(|e| e.breaker_a)
        .map_or("—".to_owned(), |b| format!("{b:.0}"));
    ScheduleTable {
        title: format!(
            "Quadro de cargas (QD) · {} · demanda {} kVA",
            schedule.supply.label(),
            decimal(schedule.demand.demand_va / 1000.0, 2)
        ),
        headers: ["Circ.", "Tipo", "Fase", "VA", "A", "mm²", "Disj.", "DR"]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
        rows,
        footer: Some(vec![
            "Total".to_owned(),
            String::new(),
            String::new(),
            format!("{:.0}", schedule.total_va),
            String::new(),
            String::new(),
            format!("DG {main}"),
            String::new(),
        ]),
    }
}

/// The single-line diagram of the board and its panel schedule, as a
/// landscape A3 drawing (taller when there are many circuits).
pub fn single_line_svg(project: &Project) -> String {
    let schedule = project.load_schedule();
    let settings = project.electrical;
    let rows = &schedule.circuits;
    let step = 8.0;
    let top = 92.0;
    let width = 420.0;
    let table_rows = rows.len() as f64 + 1.0;
    let height = (top + rows.len() as f64 * step + 40.0)
        .max(60.0 + table_rows * 6.0 + 110.0)
        .max(297.0);
    let ink = "#1d2836";
    let red = "#c62828";
    let font = "font-family=\"Arial, sans-serif\"";
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                anchor: &str,
                value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{ink}\" text-anchor=\"{anchor}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };
    let line = |svg: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, w: f64, color: &str| {
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{color}\" stroke-width=\"{w}\"/>"
        );
    };
    text(
        &mut svg,
        20.0,
        24.0,
        5.0,
        true,
        "start",
        "DIAGRAMA UNIFILAR · QUADRO DE DISTRIBUIÇÃO (QD)",
    );
    let citation = settings.utility.citation();
    text(
        &mut svg,
        20.0,
        31.0,
        3.2,
        false,
        "start",
        &format!(
            "{} · ligação {} · {:.0}/{:.0} V · {} ({})",
            project.name,
            schedule.supply.label().to_lowercase(),
            settings.voltage_v,
            if settings.voltage_v < 170.0 {
                220.0
            } else {
                380.0
            },
            citation.standard,
            citation.edition
        ),
    );
    if rows.is_empty() {
        text(
            &mut svg,
            20.0,
            48.0,
            3.6,
            false,
            "start",
            "Nenhum circuito ainda: gere os pontos e distribua os circuitos.",
        );
        svg.push_str("</svg>\n");
        return svg;
    }
    let entrance = schedule.entrance;
    let phases = schedule.supply.phases().max(1);
    let poles = |n: u8| format!("{n}P");

    // Utility, meter, main breaker and surge protection.
    let x = 40.0;
    text(
        &mut svg,
        x,
        42.0,
        3.0,
        true,
        "middle",
        "Rede da distribuidora",
    );
    line(&mut svg, x, 44.0, x, 50.0, 0.5, ink);
    let _ = writeln!(
        svg,
        "<rect x=\"{:.2}\" y=\"50\" width=\"14\" height=\"8\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>",
        x - 7.0
    );
    text(&mut svg, x, 55.4, 3.0, true, "middle", "kWh");
    line(&mut svg, x, 58.0, x, 64.0, 0.5, ink);
    // A breaker: the blade open at an angle, a cross at the fixed contact.
    let breaker_v = |svg: &mut String, x: f64, y: f64| {
        line(svg, x, y, x + 3.2, y + 6.0, 0.5, ink);
        line(svg, x - 1.0, y + 6.0, x + 1.0, y + 8.0, 0.4, ink);
        line(svg, x + 1.0, y + 6.0, x - 1.0, y + 8.0, 0.4, ink);
        line(svg, x, y + 7.0, x, y + 10.0, 0.5, ink);
    };
    breaker_v(&mut svg, x, 64.0);
    let main_label = match entrance.and_then(|e| e.breaker_a) {
        Some(b) => format!("DG {} {b:.0} A", poles(phases)),
        None => "DG —".to_owned(),
    };
    text(&mut svg, x + 6.0, 69.0, 3.0, true, "start", &main_label);
    if let Some(e) = entrance {
        let feeder = match (e.wire_mm2, e.earth_mm2) {
            (Some(w), Some(t)) => format!(
                "Ramal: {}#{} mm² (F) + {} mm² (N) + {} mm² (T){}",
                phases,
                section_label(w),
                section_label(w),
                section_label(t),
                e.conduit_mm
                    .map_or(String::new(), |d| format!(" · Ø{d} mm"))
            ),
            _ => "Ramal: consulte a distribuidora".to_owned(),
        };
        text(&mut svg, x + 12.0, 51.0, 2.6, false, "start", &feeder);
        text(
            &mut svg,
            x + 12.0,
            56.0,
            2.6,
            false,
            "start",
            &format!(
                "Demanda {} kVA · {} A na fase mais carregada",
                decimal(schedule.demand.demand_va / 1000.0, 2),
                decimal(e.current_a, 1)
            ),
        );
    }
    line(&mut svg, x, 74.0, x, 82.0, 0.5, ink);
    // Surge protection (DPS) to earth, beside the main breaker.
    line(&mut svg, x, 78.0, x + 22.0, 78.0, 0.35, ink);
    let _ = writeln!(
        svg,
        "<rect x=\"{:.2}\" y=\"75.5\" width=\"8\" height=\"5\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>",
        x + 22.0
    );
    text(&mut svg, x + 26.0, 79.0, 2.2, true, "middle", "DPS");
    line(&mut svg, x + 30.0, 78.0, x + 34.0, 78.0, 0.35, ink);
    for (i, half) in [3.0, 2.0, 1.0].iter().enumerate() {
        let gx = x + 34.0 + i as f64 * 0.8;
        line(&mut svg, gx, 78.0 - half, gx, 78.0 + half, 0.35, ink);
    }

    // The busbar and one branch per circuit.
    let bus_x = x;
    let bus_bottom = top + (rows.len() as f64 - 1.0) * step + 2.0;
    line(&mut svg, bus_x, 82.0, bus_x, bus_bottom, 1.2, ink);
    for (i, row) in rows.iter().enumerate() {
        let y = top + i as f64 * step;
        line(&mut svg, bus_x, y, bus_x + 14.0, y, 0.4, ink);
        // Breaker, lying down.
        let bx = bus_x + 14.0;
        line(&mut svg, bx, y, bx + 6.0, y - 3.0, 0.45, ink);
        line(&mut svg, bx + 6.0, y - 1.0, bx + 8.0, y + 1.0, 0.35, ink);
        line(&mut svg, bx + 6.0, y + 1.0, bx + 8.0, y - 1.0, 0.35, ink);
        line(&mut svg, bx + 7.0, y, bx + 24.0, y, 0.4, ink);
        text(
            &mut svg,
            bx + 4.0,
            y - 3.6,
            2.4,
            true,
            "middle",
            &row.breaker_a.map_or("—".to_owned(), |b| {
                format!("{}{b:.0} A", if row.poles >= 2 { "2P " } else { "" })
            }),
        );
        let mut cx = bx + 24.0;
        if row.needs_rcd {
            let _ = writeln!(
                svg,
                "<ellipse cx=\"{:.2}\" cy=\"{y:.2}\" rx=\"3.4\" ry=\"1.8\" fill=\"none\" stroke=\"{red}\" stroke-width=\"0.4\"/>",
                cx + 4.0
            );
            text(
                &mut svg,
                cx + 4.0,
                y - 2.6,
                2.0,
                false,
                "middle",
                "DR 30 mA",
            );
            line(&mut svg, cx, y, cx + 12.0, y, 0.4, ink);
            cx += 12.0;
        } else {
            line(&mut svg, cx, y, cx + 12.0, y, 0.4, ink);
            cx += 12.0;
        }
        line(&mut svg, cx, y, cx + 40.0, y, 0.4, ink);
        // Wires ticks: phases, neutral, earth.
        let wires = usize::from(row.poles.max(1)) + usize::from(row.poles < 2) + 1;
        for k in 0..wires {
            let tx = cx + 6.0 + k as f64 * 1.6;
            line(&mut svg, tx - 0.6, y + 1.2, tx + 0.6, y - 1.2, 0.3, ink);
        }
        text(
            &mut svg,
            cx + 13.0,
            y - 1.0,
            2.4,
            false,
            "start",
            &format!(
                "{} mm²{}",
                row.wire_mm2.map_or("—".to_owned(), section_label),
                if row.wire_by_drop { " *" } else { "" }
            ),
        );
        text(
            &mut svg,
            cx + 30.0,
            y - 1.0,
            2.4,
            true,
            "start",
            &phase_label(&row.phases),
        );
        text(
            &mut svg,
            cx + 42.0,
            y + 0.9,
            2.6,
            true,
            "start",
            &format!("C{}", row.number),
        );
        let rooms = if row.rooms.is_empty() {
            String::new()
        } else {
            format!(" · {}", row.rooms.join(", "))
        };
        let board_tag = row
            .board
            .as_deref()
            .filter(|b| *b != "QDG")
            .map_or(String::new(), |b| format!("[{b}] "));
        let mut description = format!(
            "{board_tag}{}{} · {:.0} VA",
            row.kind.label(),
            rooms,
            row.power_va
        );
        if description.chars().count() > 46 {
            description = description.chars().take(45).collect::<String>() + "…";
        }
        text(
            &mut svg,
            cx + 50.0,
            y + 0.9,
            2.4,
            false,
            "start",
            &description,
        );
    }
    // Earth and neutral bars.
    let bars_y = bus_bottom + 8.0;
    text(
        &mut svg,
        bus_x,
        bars_y,
        2.4,
        false,
        "start",
        "Barramentos de neutro e de terra (PE) no quadro; PE ligado ao aterramento.",
    );
    if !schedule.sub_boards.is_empty() {
        let mut sy = bars_y + 7.0;
        text(
            &mut svg,
            bus_x,
            sy,
            2.6,
            true,
            "start",
            "Quadros secundários (QDC):",
        );
        for sub in &schedule.sub_boards {
            sy += 4.5;
            let circs = if sub.circuits.is_empty() {
                "sem circuitos".to_owned()
            } else {
                sub.circuits
                    .iter()
                    .map(|c| format!("C{c}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let feeder = format!(
                "{} ({}) · demanda {} kVA · disj. {} · alim. {} mm²",
                sub.label,
                circs,
                decimal(sub.demand_va / 1000.0, 2),
                sub.breaker_a
                    .map_or("—".to_owned(), |b| format!("{b:.0} A")),
                sub.wire_mm2.map_or("—".to_owned(), section_label),
            );
            text(&mut svg, bus_x + 2.0, sy, 2.3, false, "start", &feeder);
        }
    }

    // Panel schedule.
    let tx = 222.0;
    let columns: [(&str, f64); 12] = [
        ("Circ.", 11.0),
        ("Tipo", 14.0),
        ("Fases", 11.0),
        ("Pot. (VA)", 17.0),
        ("A (VA)", 15.0),
        ("B (VA)", 15.0),
        ("C (VA)", 15.0),
        ("I (A)", 12.0),
        ("Fio (mm²)", 15.0),
        ("Disj.", 12.0),
        ("DR", 9.0),
        ("Queda", 12.0),
    ];
    let table_w: f64 = columns.iter().map(|(_, w)| w).sum();
    let row_h = 6.0;
    let mut y = 42.0;
    text(
        &mut svg,
        tx,
        y - 3.0,
        3.4,
        true,
        "start",
        "QUADRO DE CARGAS E DISTRIBUIÇÃO",
    );
    let total_rows = rows.len() + 2;
    let _ = writeln!(
        svg,
        "<rect x=\"{tx}\" y=\"{y}\" width=\"{table_w}\" height=\"{:.1}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>\n\
         <rect x=\"{tx}\" y=\"{y}\" width=\"{table_w}\" height=\"{row_h}\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.3\"/>",
        total_rows as f64 * row_h
    );
    let cell_row = |svg: &mut String, y: f64, cells: &[String], bold: bool| {
        let mut cx = tx;
        for ((_, w), cell) in columns.iter().zip(cells) {
            text(svg, cx + w / 2.0, y + 4.1, 2.4, bold, "middle", cell);
            cx += w;
        }
    };
    cell_row(
        &mut svg,
        y,
        &columns
            .iter()
            .map(|(h, _)| (*h).to_owned())
            .collect::<Vec<_>>(),
        true,
    );
    y += row_h;
    for row in rows {
        let mut per_phase = [String::new(), String::new(), String::new()];
        for phase in &row.phases {
            per_phase[usize::from(*phase).min(2)] =
                format!("{:.0}", row.power_va / row.phases.len().max(1) as f64);
        }
        let [a, b, c] = per_phase;
        cell_row(
            &mut svg,
            y,
            &[
                format!("C{}", row.number),
                row.kind.short().to_owned(),
                phase_label(&row.phases),
                format!("{:.0}", row.power_va),
                a,
                b,
                c,
                decimal(row.current_a, 1),
                row.wire_mm2.map_or("—".to_owned(), section_label),
                row.breaker_a.map_or("—".to_owned(), |v| format!("{v:.0}")),
                if row.needs_rcd { "Sim" } else { "—" }.to_owned(),
                row.drop_percent
                    .map_or("—".to_owned(), |d| format!("{} %", decimal(d, 1))),
            ],
            false,
        );
        line(&mut svg, tx, y, tx + table_w, y, 0.15, "#b8c4d0");
        y += row_h;
    }
    line(&mut svg, tx, y, tx + table_w, y, 0.35, ink);
    let phase_total = |i: usize| {
        if i < usize::from(phases) {
            format!("{:.0}", schedule.phase_va[i])
        } else {
            String::new()
        }
    };
    cell_row(
        &mut svg,
        y,
        &[
            "Total".to_owned(),
            String::new(),
            String::new(),
            format!("{:.0}", schedule.total_va),
            phase_total(0),
            phase_total(1),
            phase_total(2),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        true,
    );
    let mut cx = tx;
    for (_, w) in &columns[..columns.len() - 1] {
        cx += w;
        line(&mut svg, cx, 42.0, cx, y + row_h, 0.15, "#b8c4d0");
    }
    y += row_h + 10.0;

    // Demand and entrance summary.
    let d = schedule.demand;
    let mut summary = vec![
        format!(
            "Carga instalada: {:.0} VA ({} kVA) em {} circuito(s).",
            schedule.total_va,
            decimal(schedule.total_va / 1000.0, 2),
            rows.len()
        ),
        format!(
            "Iluminação e TUG: {:.0} VA × {} = {:.0} VA",
            d.general_va,
            decimal(d.general_factor, 2),
            d.general_va * d.general_factor
        ),
        format!(
            "TUE: {:.0} VA em {} circuito(s) × {} = {:.0} VA",
            d.specific_va,
            d.specific_count,
            decimal(d.specific_factor, 2),
            d.specific_va * d.specific_factor
        ),
        format!(
            "Demanda provável: {} kVA · ligação {}",
            decimal(d.demand_va / 1000.0, 2),
            schedule.supply.label().to_lowercase()
        ),
    ];
    if let Some(e) = entrance {
        summary.push(format!(
            "Disjuntor geral: {} · ramal {} mm² · eletroduto Ø{} mm",
            e.breaker_a
                .map_or("—".to_owned(), |b| format!("{} {b:.0} A", poles(phases))),
            e.wire_mm2.map_or("—".to_owned(), section_label),
            e.conduit_mm.map_or("—".to_owned(), |c| c.to_string())
        ));
    }
    for (i, item) in summary.iter().enumerate() {
        text(&mut svg, tx, y + i as f64 * 5.0, 2.8, i == 3, "start", item);
    }
    y += summary.len() as f64 * 5.0 + 6.0;
    for (i, note) in [
        "Fios de cobre com isolação de PVC 750 V, método B1, 30 °C. Seções mínimas: 1,5 mm² (iluminação) e 2,5 mm² (tomadas).",
        "* Fio aumentado para manter a queda de tensão até 4 % (NBR 5410 6.2.7). Queda calculada com ρ = 1/56 Ω·mm²/m.",
        "Fatores de demanda usuais das distribuidoras para residências; confira a norma da sua distribuidora.",
        "Pré-dimensionamento: confira sempre o projeto com um profissional habilitado (ART/RRT).",
    ]
    .iter()
    .enumerate()
    {
        text(&mut svg, tx, y + i as f64 * 4.2, 2.3, false, "start", note);
    }
    svg.push_str("</svg>\n");
    svg
}
