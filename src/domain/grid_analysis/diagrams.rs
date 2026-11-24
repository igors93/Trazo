//! Printable A3 landscape vector sheet (`grid_analysis_svg`) for the coupled
//! slab + beam grillage analysis (#109): deflection heatmap across every slab
//! cell (including L-shaped and irregular slabs), beams, column reactions and
//! summary tables.

use std::fmt::Write as _;

use crate::domain::model::Project;

use super::GridAnalysisReport;

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Smooth blue -> cyan -> green -> yellow -> red colour ramp for `t` in `0..=1`.
fn heat_color(t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.25 {
        let u = t / 0.25;
        (33.0 + u * 8.0, 150.0 + u * 45.0, 243.0 - u * 40.0)
    } else if t < 0.50 {
        let u = (t - 0.25) / 0.25;
        (41.0 + u * 60.0, 195.0 - u * 10.0, 203.0 - u * 120.0)
    } else if t < 0.75 {
        let u = (t - 0.50) / 0.25;
        (101.0 + u * 145.0, 185.0 - u * 15.0, 83.0 - u * 55.0)
    } else {
        let u = (t - 0.75) / 0.25;
        (246.0 - u * 42.0, 170.0 - u * 128.0, 28.0 + u * 12.0)
    };
    format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8)
}

/// Renders an A3 landscape SVG sheet summarizing the coupled slab+beam grillage
/// analysis (`Analogia de Grelha`). Exportable directly to SVG and PDF.
pub fn grid_analysis_svg(project: &Project, report: &GridAnalysisReport) -> String {
    let w = 1189.0_f64;
    let h = 841.0_f64;
    let mut svg = String::with_capacity(16_384);
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">"
    );
    let _ = writeln!(
        svg,
        "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>"
    );
    let _ = writeln!(
        svg,
        "<rect x=\"24\" y=\"24\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"#1f2933\" stroke-width=\"1.8\"/>",
        w - 48.0,
        h - 48.0
    );

    // Header.
    let _ = writeln!(
        svg,
        "<text x=\"44\" y=\"58\" font-family=\"Arial, sans-serif\" font-size=\"20\" font-weight=\"700\" fill=\"#102a43\">Trazo · Analogia de Grelha (Lajes + Vigas acopladas)</text>"
    );
    let _ = writeln!(
        svg,
        "<text x=\"44\" y=\"80\" font-family=\"Arial, sans-serif\" font-size=\"12\" fill=\"#486581\">Projeto: {} · {} nós · {} barras · ΣF={} kN · ΣR={} kN (erro {} kN) · NBR 6118 §14.7.5</text>",
        esc(&project.name),
        report.node_count,
        report.member_count,
        decimal(report.total_applied_kn, 1),
        decimal(report.total_reaction_kn, 1),
        decimal(report.equilibrium_error_kn, 3)
    );
    let _ = writeln!(
        svg,
        "<line x1=\"44\" y1=\"92\" x2=\"{:.0}\" y2=\"92\" stroke=\"#bcccdc\" stroke-width=\"1\"/>",
        w - 44.0
    );

    // Left panel: deflection heatmap & coupled beam mesh.
    let map_x = 48.0;
    let map_y = 112.0;
    let map_w = 640.0;
    let map_h = 620.0;
    let _ = writeln!(
        svg,
        "<rect x=\"{map_x:.0}\" y=\"{map_y:.0}\" width=\"{map_w:.0}\" height=\"{map_h:.0}\" fill=\"#f8fafc\" stroke=\"#cbd2d9\" stroke-width=\"1\"/>"
    );

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for cell in &report.heatmap_cells {
        min_x = min_x.min(cell.min.x);
        max_x = max_x.max(cell.max.x);
        min_y = min_y.min(cell.min.y);
        max_y = max_y.max(cell.max.y);
    }
    for r in &report.reactions {
        min_x = min_x.min(r.point.x);
        max_x = max_x.max(r.point.x);
        min_y = min_y.min(r.point.y);
        max_y = max_y.max(r.point.y);
    }

    if min_x.is_finite() && max_x > min_x && max_y > min_y {
        let pad = 38.0;
        let span_x = (max_x - min_x).max(0.5);
        let span_y = (max_y - min_y).max(0.5);
        let scale = ((map_w - 2.0 * pad) / span_x).min((map_h - 2.0 * pad) / span_y);
        let ox = map_x + (map_w - span_x * scale) * 0.5;
        let oy = map_y + (map_h - span_y * scale) * 0.5;
        let to_svg =
            |x: f64, y: f64| -> (f64, f64) { (ox + (x - min_x) * scale, oy + (y - min_y) * scale) };
        let w_peak = report.max_elastic_w_mm.max(1e-4);

        // Deflection cells.
        for cell in &report.heatmap_cells {
            let (x0, y0) = to_svg(cell.min.x, cell.min.y);
            let (x1, y1) = to_svg(cell.max.x, cell.max.y);
            let fill = heat_color(cell.w_elastic_mm.abs() / w_peak);
            let _ = writeln!(
                svg,
                "<rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{fill}\" fill-opacity=\"0.78\" stroke=\"#ffffff\" stroke-width=\"0.6\"/>",
                (x1 - x0).max(1.0),
                (y1 - y0).max(1.0)
            );
        }

        // Coupled beams overlay.
        for beam in project.valid_beams() {
            if let Some((a, b)) = project.beam_endpoints(beam) {
                let (ax, ay) = to_svg(a.x, a.y);
                let (bx, by) = to_svg(b.x, b.y);
                let _ = writeln!(
                    svg,
                    "<line x1=\"{ax:.1}\" y1=\"{ay:.1}\" x2=\"{bx:.1}\" y2=\"{by:.1}\" stroke=\"#102a43\" stroke-width=\"2.6\"/>"
                );
            }
        }

        // Column support reactions.
        for r in &report.reactions {
            let (rx, ry) = to_svg(r.point.x, r.point.y);
            let _ = writeln!(
                svg,
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"10\" fill=\"#102a43\" stroke=\"#ffffff\" stroke-width=\"1.2\"/>",
                rx - 5.0,
                ry - 5.0
            );
            let _ = writeln!(
                svg,
                "<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"Arial, sans-serif\" font-size=\"10\" font-weight=\"700\" fill=\"#102a43\" stroke=\"#ffffff\" stroke-width=\"2.4\" paint-order=\"stroke\">{} ({} kN)</text>",
                rx + 8.0,
                ry + 3.5,
                esc(&r.label),
                decimal(r.reaction_kn, 0)
            );
        }
    } else {
        let _ = writeln!(
            svg,
            "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"middle\" font-family=\"Arial, sans-serif\" font-size=\"14\" fill=\"#627d98\">Nenhuma laje ou viga lançada para análise de grelha.</text>",
            map_x + map_w * 0.5,
            map_y + map_h * 0.5
        );
    }

    // Colour scale legend below map.
    let leg_y = map_y + map_h + 18.0;
    let _ = writeln!(
        svg,
        "<text x=\"{map_x:.0}\" y=\"{:.0}\" font-family=\"Arial, sans-serif\" font-size=\"11\" font-weight=\"700\" fill=\"#102a43\">Deslocamento elástico w (mm): 0,0</text>",
        leg_y + 12.0
    );
    for i in 0..20 {
        let t = (i as f64) / 19.0;
        let fill = heat_color(t);
        let _ = writeln!(
            svg,
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"14\" height=\"12\" fill=\"{fill}\"/>",
            map_x + 200.0 + (i as f64) * 14.0,
            leg_y + 2.0
        );
    }
    let _ = writeln!(
        svg,
        "<text x=\"{:.1}\" y=\"{:.0}\" font-family=\"Arial, sans-serif\" font-size=\"11\" font-weight=\"700\" fill=\"#102a43\">{} mm (longo prazo ≈ {} mm)</text>",
        map_x + 488.0,
        leg_y + 12.0,
        decimal(report.max_elastic_w_mm, 2),
        decimal(report.max_elastic_w_mm * 2.5, 1)
    );

    // Right panel: Slab & Beam summary tables.
    let tx = 712.0;
    let mut ty = 126.0;
    let _ = writeln!(
        svg,
        "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"14\" font-weight=\"700\" fill=\"#102a43\">Lajes · momentos (kN·m/m) e flecha (mm)</text>"
    );
    ty += 22.0;
    let _ = writeln!(
        svg,
        "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" font-weight=\"700\" fill=\"#486581\">Laje / Cômodo             mx+    my+    mxy    w_tot / lim</text>"
    );
    ty += 6.0;
    let _ = writeln!(
        svg,
        "<line x1=\"{tx:.0}\" y1=\"{ty:.0}\" x2=\"{:.0}\" y2=\"{ty:.0}\" stroke=\"#bcccdc\" stroke-width=\"1\"/>",
        w - 44.0
    );
    ty += 16.0;
    for s in report.slabs.iter().take(12) {
        let tag = if s.is_irregular { " [L/irreg.]" } else { "" };
        let name = format!("{} {}{}", s.label, s.room, tag);
        let _ = writeln!(
            svg,
            "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" fill=\"#1f2933\">{}</text>",
            esc(&name)
        );
        let _ = writeln!(
            svg,
            "<text x=\"{:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" fill=\"#102a43\">{}   {}   {}   {} / {} mm</text>",
            tx + 185.0,
            decimal(s.mx_pos_knm_m, 2),
            decimal(s.my_pos_knm_m, 2),
            decimal(s.mxy_max_knm_m, 2),
            decimal(s.w_long_term_mm, 1),
            decimal(s.limit_mm, 1)
        );
        ty += 18.0;
    }

    ty += 16.0;
    let _ = writeln!(
        svg,
        "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"14\" font-weight=\"700\" fill=\"#102a43\">Vigas acopladas · flexão, cortante e torção</text>"
    );
    ty += 22.0;
    let _ = writeln!(
        svg,
        "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" font-weight=\"700\" fill=\"#486581\">Viga          M+ (kN·m)   M− (kN·m)   V (kN)   T (kN·m)   w (mm)</text>"
    );
    ty += 6.0;
    let _ = writeln!(
        svg,
        "<line x1=\"{tx:.0}\" y1=\"{ty:.0}\" x2=\"{:.0}\" y2=\"{ty:.0}\" stroke=\"#bcccdc\" stroke-width=\"1\"/>",
        w - 44.0
    );
    ty += 16.0;
    for b in report.beams.iter().take(16) {
        let _ = writeln!(
            svg,
            "<text x=\"{tx:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" fill=\"#1f2933\">{}</text>",
            esc(&b.label)
        );
        let _ = writeln!(
            svg,
            "<text x=\"{:.0}\" y=\"{ty:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10.5\" fill=\"#102a43\">{}       {}       {}       {}       {}</text>",
            tx + 95.0,
            decimal(b.m_pos_knm, 1),
            decimal(b.m_neg_knm, 1),
            decimal(b.v_max_kn, 1),
            decimal(b.t_max_knm, 2),
            decimal(b.w_elastic_mm, 2)
        );
        ty += 17.0;
    }

    let _ = writeln!(
        svg,
        "<text x=\"44\" y=\"{:.0}\" font-family=\"Arial, sans-serif\" font-size=\"10\" fill=\"#627d98\">Pré-dimensionamento por analogia de grelha elástica linear (Ecs, ν=0,20, rigidez à torção fissurada 0,15·GJ nas vigas). Não substitui projeto executivo com ART/RRT.</text>",
        h - 34.0
    );
    svg.push_str("</svg>\n");
    svg
}
