//! Structural column schedule table and A3 beam internal-forces SVG diagrams.

use crate::domain::model::Project;

use super::{decimal, section_cm};

/// The columns with their section, load and footing, for drawing sheets.
pub fn build_columns_schedule(project: &Project) -> crate::domain::schedules::ScheduleTable {
    let report = project.structure_report();
    let rows = report
        .columns
        .iter()
        .map(|column| {
            let footing = report
                .footings
                .iter()
                .find(|f| f.column.as_deref() == Some(column.label.as_str()));
            vec![
                column.label.clone(),
                section_cm(column.width_m, column.depth_m),
                format!("{:.0}", column.load_kn),
                footing.map_or("—".to_owned(), |f| {
                    format!("{} {}", f.label, section_cm(f.width_m, f.depth_m))
                }),
            ]
        })
        .collect();
    let s = project.structure.settings;
    crate::domain::schedules::ScheduleTable {
        title: format!(
            "Quadro de pilares e sapatas · fck {:.0} MPa · solo {:.0} kPa",
            s.fck_mpa, s.soil_kpa
        ),
        headers: ["Pilar", "Seção (cm)", "Carga (kN)", "Sapata (cm)"]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
        rows,
        footer: None,
    }
}

/// The internal forces of every beam as diagrams: each continuous beam with
/// its supports, the bending moment diagram (hogging up, sagging down) and
/// the values at supports and spans, plus the deflection of each span.
pub fn beam_diagrams_svg(project: &Project) -> String {
    let report = project.structure_report();
    let width = 420.0;
    let row = 62.0;
    let height = (40.0 + report.beams.len() as f64 * row + 20.0).max(297.0);
    let ink = "#1d2836";
    let blue = "#1565c0";
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
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                anchor: &str,
                color: &str,
                value: &str| {
        svg.push_str(&format!(
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\" text-anchor=\"{anchor}\">{}</text>\n",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        ));
    };
    text(
        &mut svg,
        20.0,
        24.0,
        5.0,
        true,
        "start",
        ink,
        "ESFORÇOS NAS VIGAS · MOMENTO FLETOR, CORTANTE E FLECHA",
    );
    let s = project.structure.settings;
    text(
        &mut svg,
        20.0,
        31.0,
        3.0,
        false,
        "start",
        ink,
        &format!(
            "{} · fck {:.0} MPa · sobrecarga {} kN/m² · revestimento {} kN/m² · {} pavimento(s) · valores característicos",
            project.name,
            s.fck_mpa,
            decimal(s.live_kn_m2, 1),
            decimal(s.finish_kn_m2, 1),
            s.floors.max(1)
        ),
    );
    if report.beams.is_empty() {
        text(
            &mut svg,
            20.0,
            48.0,
            3.6,
            false,
            "start",
            ink,
            "Nenhuma viga ainda: gere a estrutura ou ligue os pilares.",
        );
        svg.push_str("</svg>\n");
        return svg;
    }
    let left = 40.0;
    let usable = width - 40.0 - 120.0;
    for (index, line) in report.beams.iter().enumerate() {
        let top = 44.0 + index as f64 * row;
        let total: f64 = line.spans.iter().map(|s| s.length_m).sum::<f64>().max(0.1);
        let scale = usable / total;
        let axis = top + 22.0;
        text(
            &mut svg,
            20.0,
            axis + 1.2,
            3.4,
            true,
            "start",
            ink,
            &line.label,
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.2}\" y1=\"{axis:.2}\" x2=\"{:.2}\" y2=\"{axis:.2}\" stroke=\"{ink}\" stroke-width=\"0.6\"/>\n",
            left + total * scale
        ));
        let peak = line
            .spans
            .iter()
            .flat_map(|s| {
                [
                    s.moment_left.abs(),
                    s.moment_right.abs(),
                    s.moment_span.abs(),
                ]
            })
            .fold(0.1, f64::max);
        let mscale = 14.0 / peak;
        let mut x0 = left;
        let mut path = format!("M{left:.2} {axis:.2}");
        for (i, span) in line.spans.iter().enumerate() {
            let w = span.dead_kn_m + span.live_kn_m;
            let length = span.length_m;
            let v_left = w * length / 2.0 + (span.moment_right - span.moment_left) / length;
            for k in 0..=24 {
                let x = length * f64::from(k) / 24.0;
                let m = span.moment_left + v_left * x - w * x * x / 2.0;
                // Hogging drawn above the axis, sagging below (tension side).
                path.push_str(&format!(" L{:.2} {:.2}", x0 + x * scale, axis + m * mscale));
            }
            let mid = x0 + length * scale / 2.0;
            text(
                &mut svg,
                mid,
                axis + span.moment_span * mscale + 4.0,
                2.4,
                false,
                "middle",
                blue,
                &format!("M+ {}", decimal(span.moment_span, 1)),
            );
            // Short spans only get their length, so the texts do not meet.
            let caption = if length * scale < 75.0 {
                format!("{} m", decimal(length, 2))
            } else {
                format!(
                    "{} m · {} kN/m · {} · V {} kN · flecha {} mm",
                    decimal(length, 2),
                    decimal(w, 1),
                    section_cm(span.width_m, span.height_m),
                    decimal(span.shear, 1),
                    decimal(span.deflection_mm, 1)
                )
            };
            text(
                &mut svg,
                mid,
                top + 4.0,
                2.4,
                false,
                "middle",
                ink,
                &caption,
            );
            if i == 0 && span.moment_left.abs() > 0.05 {
                text(
                    &mut svg,
                    x0,
                    axis + span.moment_left * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_left, 1),
                );
            }
            if span.moment_right.abs() > 0.05 {
                text(
                    &mut svg,
                    x0 + length * scale,
                    axis + span.moment_right * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_right, 1),
                );
            }
            x0 += length * scale;
        }
        path.push_str(&format!(" L{x0:.2} {axis:.2} Z"));
        svg.push_str(&format!(
            "<path d=\"{path}\" fill=\"{blue}\" fill-opacity=\"0.15\" stroke=\"{blue}\" stroke-width=\"0.35\"/>\n"
        ));
        // Supports with their column label and reaction.
        let labels = project.structure_labels();
        let mut x = left;
        for (j, joint) in line.joints.iter().enumerate() {
            svg.push_str(&format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>\n",
                x,
                axis,
                x - 2.0,
                axis + 3.0,
                x + 2.0,
                axis + 3.0
            ));
            text(
                &mut svg,
                x,
                axis + 7.0,
                2.3,
                true,
                "middle",
                ink,
                labels.get(joint).map_or("?", String::as_str),
            );
            if let Some(r) = line.reactions.get(j) {
                text(
                    &mut svg,
                    x,
                    axis + 10.5,
                    2.2,
                    false,
                    "middle",
                    ink,
                    &format!("R {} kN", decimal(*r, 1)),
                );
            }
            if let Some(span) = line.spans.get(j) {
                x += span.length_m * scale;
            }
        }
        // Summary on the right.
        let sx = width - 112.0;
        let worst = line
            .spans
            .iter()
            .map(|s| s.moment_left.min(s.moment_right))
            .fold(0.0, f64::min);
        let best = line.spans.iter().map(|s| s.moment_span).fold(0.0, f64::max);
        let shear = line.spans.iter().map(|s| s.shear).fold(0.0, f64::max);
        let ok = line
            .spans
            .iter()
            .all(|s| s.deflection_mm <= s.limit_mm && s.height_m + 1e-6 >= s.height_min_m);
        for (k, item) in [
            format!("M− máx: {} kN·m", decimal(worst, 1)),
            format!("M+ máx: {} kN·m", decimal(best, 1)),
            format!("V máx: {} kN", decimal(shear, 1)),
            if ok {
                "Altura e flecha: atendem".to_owned()
            } else {
                "Altura ou flecha: revise".to_owned()
            },
        ]
        .iter()
        .enumerate()
        {
            text(
                &mut svg,
                sx,
                top + 12.0 + k as f64 * 5.0,
                2.7,
                k == 3,
                "start",
                if k == 3 && !ok { red } else { ink },
                item,
            );
        }
    }
    let notes_y = height - 22.0;
    for (i, note) in [
        "Vigas contínuas sobre apoios simples (pilares), carga uniforme por vão: peso próprio, lajes repartidas por charneiras plásticas (45°) e paredes dos pavimentos de cima.",
        "Flecha com carga quase permanente (g + 0,3q), inércia equivalente de Branson (Mr e armadura CA-50) e fluência (1 + 1,32), comparada a L/250 (NBR 6118).",
        "Pré-dimensionamento para estudo: o projeto estrutural final é de um engenheiro habilitado (ART).",
    ]
    .iter()
    .enumerate()
    {
        text(&mut svg, 20.0, notes_y + i as f64 * 4.0, 2.3, false, "start", ink, note);
    }
    svg.push_str("</svg>\n");
    svg
}
