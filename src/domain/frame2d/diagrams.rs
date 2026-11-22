//! A3 landscape vector SVG sheet showing 2D plane-frame deformed shapes,
//! beam & column bending moment diagrams, support reactions and lateral drift (#94).

use std::fmt::Write;

use crate::domain::model::Project;

use super::{
    builder::FrameLineResult,
    model::{MemberKind, SupportKind},
};

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn dec(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// Renders an A3 landscape SVG (1189 × 841) with the 2D frame analysis
/// results: deformed shape (left) and bending moment diagram + reactions (right)
/// for each continuous portal frame in the project.
pub fn frame_diagrams_svg(project: &Project) -> String {
    let report = project.cached_structure_report();
    let frames = &report.frames;
    let w = 1189.0_f64;
    let h = 841.0_f64;

    let mut svg = String::with_capacity(16_384);
    let _ = writeln!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}">"##
    );
    svg.push_str(
        r##"<rect width="100%" height="100%" fill="#ffffff"/>
<rect x="24" y="24" width="1141" height="793" fill="none" stroke="#1e293b" stroke-width="1.4"/>
"##,
    );

    let settings = project.structure.settings;
    let joint_txt = if settings.frame_rigid_joints {
        "Nós rígidos (monolíticos)"
    } else {
        "Nós rotulados (pilares articulados no topo)"
    };
    let base_txt = if settings.frame_fixed_bases {
        "Bases engastadas"
    } else {
        "Bases rotuladas"
    };

    let _ = writeln!(
        svg,
        r##"<text x="42" y="54" font-family="Arial, sans-serif" font-size="18" font-weight="bold" fill="#0f172a">Análise de pórticos planos 2D — Deformada e Diagrama de Momentos (M)</text>
<text x="42" y="74" font-family="Arial, sans-serif" font-size="11.5" fill="#475569">{} · fck {} MPa · {} · {} · Vento V0 = {} m/s (NBR 6123) · Método da Rigidez Direta (Euler–Bernoulli)</text>
<line x1="24" y1="86" x2="1165" y2="86" stroke="#cbd5e1" stroke-width="1"/>"##,
        esc(&project.name),
        dec(settings.fck_mpa, 0),
        joint_txt,
        base_txt,
        dec(project.structure.wind.v0_ms, 0),
    );

    if frames.is_empty() {
        svg.push_str(
            r##"<text x="594" y="420" text-anchor="middle" font-family="Arial, sans-serif" font-size="15" fill="#64748b">Nenhum pórtico 2D disponível. Gere a estrutura (pilares e vigas) para visualizar a deformada e os esforços.</text>
</svg>
"##,
        );
        return svg;
    }

    // Show up to 4 portal frames per A3 sheet cleanly stacked in rows.
    let shown = frames.len().min(4);
    let top_y = 98.0;
    let footer_y = 796.0;
    let avail_h = footer_y - top_y;
    let row_gap = 12.0;
    let row_h = (avail_h - row_gap * (shown.saturating_sub(1) as f64)) / shown as f64;

    for (idx, frame) in frames.iter().take(shown).enumerate() {
        let ry = top_y + idx as f64 * (row_h + row_gap);
        draw_frame_row(&mut svg, frame, 40.0, ry, 1109.0, row_h);
    }

    let extra_note = if frames.len() > shown {
        format!(
            "Mostrando os {} primeiros de {} pórticos 2D. ",
            shown,
            frames.len()
        )
    } else {
        String::new()
    };
    let max_res = frames
        .iter()
        .map(|f| f.equilibrium_residual_kn)
        .fold(0.0_f64, f64::max);

    let _ = writeln!(
        svg,
        r##"<line x1="24" y1="800" x2="1165" y2="800" stroke="#cbd5e1" stroke-width="1"/>
<text x="42" y="812" font-family="Arial, sans-serif" font-size="10" fill="#64748b">{extra_note}Equilíbrio global verificado (resíduo máx. = {max_res:.2e} kN). Análise elástica linear preliminar — revisar com engenheiro habilitado (ART/RRT).</text>
</svg>"##
    );
    svg
}

fn draw_frame_row(svg: &mut String, frame: &FrameLineResult, x: f64, y: f64, w: f64, h: f64) {
    // Card border & title bar.
    let _ = writeln!(
        svg,
        r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="6" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1"/>"##
    );
    let drift_color = if frame.drift_ok() {
        "#15803d"
    } else {
        "#b91c1c"
    };
    let brace_tag = if frame.braced_sway {
        " · Contraventado"
    } else {
        ""
    };
    let _ = writeln!(
        svg,
        r##"<text x="{:.1}" y="{:.1}" font-family="Arial, sans-serif" font-size="12" font-weight="bold" fill="#0f172a">{} ({} m × H = {} m){brace_tag}</text>
<text x="{:.1}" y="{:.1}" text-anchor="end" font-family="Arial, sans-serif" font-size="10.5" fill="{drift_color}">M_viga máx = {} kN·m · M_pilar máx = {} kN·m · Desloc. lateral Δh = {} mm (lim. {} mm) · H_vento = {} kN</text>"##,
        x + 12.0,
        y + 18.0,
        esc(&frame.label),
        dec(frame.total_length_m, 2),
        dec(frame.story_height_m, 2),
        x + w - 12.0,
        y + 18.0,
        dec(frame.max_beam_moment_knm(), 1),
        dec(frame.max_column_moment_knm(), 1),
        dec(frame.max_lateral_drift_mm, 2),
        dec(frame.drift_limit_mm, 2),
        dec(frame.lateral_wind_kn, 1),
    );

    let panel_y = y + 26.0;
    let panel_h = (h - 34.0).max(50.0);
    let half_w = (w - 30.0) * 0.5;
    let left_x = x + 10.0;
    let right_x = left_x + half_w + 10.0;

    draw_deformed_panel(svg, frame, left_x, panel_y, half_w, panel_h);
    draw_moment_panel(svg, frame, right_x, panel_y, half_w, panel_h);
}

fn frame_viewport(frame: &FrameLineResult, px: f64, py: f64, pw: f64, ph: f64) -> (f64, f64, f64) {
    let pad_x = 46.0;
    let pad_top = 28.0;
    let pad_bot = 30.0;
    let span_x = frame.total_length_m.max(1.0);
    let span_y = frame.story_height_m.max(1.0);
    let sx = (pw - 2.0 * pad_x).max(40.0) / span_x;
    let sy = (ph - pad_top - pad_bot).max(30.0) / span_y;
    let scale = sx.min(sy);
    let ox = px + (pw - span_x * scale) * 0.5;
    let oy = py + ph - pad_bot;
    (ox, oy, scale)
}

fn draw_supports(svg: &mut String, frame: &FrameLineResult, ox: f64, oy: f64, scale: f64) {
    for r in &frame.reactions {
        let sx = ox + r.x_m * scale;
        let sy = oy - r.y_m * scale;
        match r.support {
            SupportKind::Fixed => {
                let _ = writeln!(
                    svg,
                    r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#0f172a" stroke-width="2"/>"##,
                    sx - 9.0,
                    sy,
                    sx + 9.0,
                    sy
                );
                for k in -2..=2 {
                    let hx = sx + k as f64 * 4.0;
                    let _ = writeln!(
                        svg,
                        r##"<line x1="{hx:.1}" y1="{sy:.1}" x2="{:.1}" y2="{:.1}" stroke="#475569" stroke-width="1"/>"##,
                        hx - 3.5,
                        sy + 6.0
                    );
                }
            }
            SupportKind::Pinned | SupportKind::RollerY => {
                let _ = writeln!(
                    svg,
                    r##"<polygon points="{sx:.1},{sy:.1} {:.1},{:.1} {:.1},{:.1}" fill="#e2e8f0" stroke="#0f172a" stroke-width="1.2"/>"##,
                    sx - 7.0,
                    sy + 10.0,
                    sx + 7.0,
                    sy + 10.0
                );
            }
            SupportKind::RollerX => {
                let _ = writeln!(
                    svg,
                    r##"<polygon points="{sx:.1},{sy:.1} {:.1},{:.1} {:.1},{:.1}" fill="#e2e8f0" stroke="#0f172a" stroke-width="1.2"/>"##,
                    sx - 9.0,
                    sy - 5.0,
                    sx - 9.0,
                    sy + 5.0
                );
            }
            SupportKind::Free => {}
        }
    }
}

fn draw_deformed_panel(svg: &mut String, frame: &FrameLineResult, x: f64, y: f64, w: f64, h: f64) {
    let _ = writeln!(
        svg,
        r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="4" fill="#ffffff" stroke="#e2e8f0" stroke-width="1"/>
<text x="{:.1}" y="{:.1}" font-family="Arial, sans-serif" font-size="10" font-weight="bold" fill="#334155">Deformada (ampliada)</text>"##,
        x + 8.0,
        y + 13.0
    );

    let (ox, oy, scale) = frame_viewport(frame, x, y, w, h);

    // Find max displacement to choose a readable magnification factor (target ~16 px max deflection).
    let max_disp_m = frame
        .members
        .iter()
        .flat_map(|m| m.samples.iter())
        .map(|s| s.dx_m.hypot(s.dy_m))
        .fold(0.0_f64, f64::max);
    let amp = if max_disp_m > 1e-9 {
        (16.0 / (max_disp_m * scale)).clamp(1.0, 400.0)
    } else {
        1.0
    };

    // Undeformed skeleton (dashed grey).
    for m in &frame.members {
        if let (Some(first), Some(last)) = (m.samples.first(), m.samples.last()) {
            let _ = writeln!(
                svg,
                r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#94a3b8" stroke-width="1.2" stroke-dasharray="4 3"/>"##,
                ox + first.x0 * scale,
                oy - first.y0 * scale,
                ox + last.x0 * scale,
                oy - last.y0 * scale
            );
        }
    }

    // Deformed curve (solid blue).
    for m in &frame.members {
        let pts: String = m
            .samples
            .iter()
            .map(|s| {
                let px = ox + (s.x0 + s.dx_m * amp) * scale;
                let py = oy - (s.y0 + s.dy_m * amp) * scale;
                format!("{px:.1},{py:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            svg,
            r##"<polyline points="{pts}" fill="none" stroke="#2563eb" stroke-width="2.0"/>"##
        );
    }

    draw_supports(svg, frame, ox, oy, scale);

    // Column & beam labels.
    for m in &frame.members {
        if let Some(mid) = m.samples.get(m.samples.len() / 2) {
            let mx = ox + mid.x0 * scale;
            let my = oy - mid.y0 * scale;
            let (lx, ly) = match m.kind {
                MemberKind::Beam => (mx, my - 6.0),
                MemberKind::Column => (mx + 14.0, my + 3.0),
            };
            let _ = writeln!(
                svg,
                r##"<text x="{lx:.1}" y="{ly:.1}" text-anchor="middle" font-family="Arial, sans-serif" font-size="9" fill="#475569">{}</text>"##,
                esc(&m.label)
            );
        }
    }

    let _ = writeln!(
        svg,
        r##"<text x="{:.1}" y="{:.1}" text-anchor="end" font-family="Arial, sans-serif" font-size="9" fill="#64748b">Escala def. ×{:.0}</text>"##,
        x + w - 8.0,
        y + 13.0,
        amp
    );
}

fn draw_moment_panel(svg: &mut String, frame: &FrameLineResult, x: f64, y: f64, w: f64, h: f64) {
    let _ = writeln!(
        svg,
        r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="4" fill="#ffffff" stroke="#e2e8f0" stroke-width="1"/>
<text x="{:.1}" y="{:.1}" font-family="Arial, sans-serif" font-size="10" font-weight="bold" fill="#334155">Diagrama de Momento Fletor M (kN·m) e Reações</text>"##,
        x + 8.0,
        y + 13.0
    );

    let (ox, oy, scale) = frame_viewport(frame, x, y, w, h);
    let max_m = frame
        .members
        .iter()
        .flat_map(|m| m.samples.iter())
        .map(|s| s.moment_knm.abs())
        .fold(0.0_f64, f64::max)
        .max(1e-6);
    let m_scale = 20.0 / max_m;

    // Draw filled bending-moment polygons on every beam and column.
    for m in &frame.members {
        let (Some(first), Some(last)) = (m.samples.first(), m.samples.last()) else {
            continue;
        };
        let dx = last.x0 - first.x0;
        let dy = last.y0 - first.y0;
        let len = dx.hypot(dy).max(1e-9);
        // Outward normal in SVG screen coords pointing toward the local -y' tension
        // fiber (world -y' = (+dy/L, -dx/L) -> SVG (vx, -vy) = (+dy/L, +dx/L)):
        // horizontal beams plot positive sagging M downward; vertical columns plot
        // tension-face M on the exact tension side.
        let nx_svg = dy / len;
        let ny_svg = dx / len;

        let mut poly = Vec::with_capacity(m.samples.len() + 2);
        poly.push(format!(
            "{:.1},{:.1}",
            ox + first.x0 * scale,
            oy - first.y0 * scale
        ));
        for s in &m.samples {
            let bx = ox + s.x0 * scale;
            let by = oy - s.y0 * scale;
            let off = s.moment_knm * m_scale;
            poly.push(format!("{:.1},{:.1}", bx + nx_svg * off, by + ny_svg * off));
        }
        poly.push(format!(
            "{:.1},{:.1}",
            ox + last.x0 * scale,
            oy - last.y0 * scale
        ));

        let (fill, stroke) = match m.kind {
            MemberKind::Beam => ("rgba(37, 99, 235, 0.18)", "#1d4ed8"),
            MemberKind::Column => ("rgba(220, 38, 38, 0.18)", "#b91c1c"),
        };
        let _ = writeln!(
            svg,
            r##"<polygon points="{}" fill="{fill}" stroke="{stroke}" stroke-width="1.2"/>"##,
            poly.join(" ")
        );
        // Frame centerline on top.
        let _ = writeln!(
            svg,
            r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#0f172a" stroke-width="1.6"/>"##,
            ox + first.x0 * scale,
            oy - first.y0 * scale,
            ox + last.x0 * scale,
            oy - last.y0 * scale
        );

        // Peak moment label on this member.
        if let Some(peak) = m
            .samples
            .iter()
            .max_by(|a, b| a.moment_knm.abs().total_cmp(&b.moment_knm.abs()))
            && peak.moment_knm.abs() >= 0.05
        {
            let bx = ox + peak.x0 * scale;
            let by = oy - peak.y0 * scale;
            let off = peak.moment_knm * m_scale;
            let lx = bx + nx_svg * (off + off.signum() * 7.0);
            let ly = by + ny_svg * (off + off.signum() * 7.0) + 3.0;
            let _ = writeln!(
                svg,
                r##"<text x="{lx:.1}" y="{ly:.1}" text-anchor="middle" font-family="Arial, sans-serif" font-size="8.5" font-weight="bold" fill="{stroke}">{}</text>"##,
                dec(peak.moment_knm, 1)
            );
        }
    }

    draw_supports(svg, frame, ox, oy, scale);

    // Base vertical reaction labels under each column support.
    for r in &frame.reactions {
        if matches!(r.support, SupportKind::Fixed | SupportKind::Pinned) {
            let sx = ox + r.x_m * scale;
            let sy = oy - r.y_m * scale;
            let _ = writeln!(
                svg,
                r##"<text x="{sx:.1}" y="{:.1}" text-anchor="middle" font-family="Arial, sans-serif" font-size="8.5" fill="#0f172a">R={} kN</text>"##,
                sy + 21.0,
                dec(r.ry_kn, 1)
            );
        }
    }
}
