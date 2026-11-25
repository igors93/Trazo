//! A3 landscape schematic reinforcement detailing vector sheet (SVG/PDF).

use crate::domain::{
    model::Project,
    structure::{SlabKind, StructureReport},
};

use super::{BeamTopBars, ReinforcementReport, decimal, diameter_text};

fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Renders an A3 landscape schematic reinforcement detailing sheet (SVG)
/// illustrating the governing beam, column, footing, slab summary, and
/// CA-50 steel summary by diameter.
pub fn reinforcement_detail_svg(
    project: &Project,
    structure: &StructureReport,
    report: &ReinforcementReport,
) -> String {
    use std::fmt::Write as _;

    let fck = project.structure.settings.fck_mpa;
    let project_name = if project.name.trim().is_empty() {
        "Projeto sem título"
    } else {
        project.name.trim()
    };

    let mut svg = String::with_capacity(16_384);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="297mm" viewBox="0 0 420 297">"#
    );
    let _ = writeln!(
        svg,
        r#"  <style>
    .bg {{ fill: #ffffff; }}
    .frame {{ fill: none; stroke: #1f2937; stroke-width: 0.45; }}
    .panel {{ fill: #fafbfc; stroke: #cbd5e1; stroke-width: 0.35; rx: 1.5; }}
    .panel-hdr {{ fill: #1e293b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 4.1px; font-weight: 700; }}
    .title {{ fill: #0f172a; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 6.2px; font-weight: 700; }}
    .sub {{ fill: #475569; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.4px; }}
    .lbl {{ fill: #1e293b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.1px; }}
    .lbl-b {{ fill: #0f172a; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 3.2px; font-weight: 700; }}
    .dim {{ fill: #334155; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 2.8px; }}
    .muted {{ fill: #64748b; font-family: "DejaVu Sans", "Liberation Sans", Arial, sans-serif; font-size: 2.7px; }}
    .conc {{ fill: #e2e8f0; stroke: #334155; stroke-width: 0.4; }}
    .stirrup {{ fill: none; stroke: #0284c7; stroke-width: 0.45; }}
    .bar-pos {{ fill: none; stroke: #dc2626; stroke-width: 0.7; stroke-linecap: round; stroke-linejoin: round; }}
    .bar-neg {{ fill: none; stroke: #b45309; stroke-width: 0.65; stroke-linecap: round; stroke-linejoin: round; }}
    .bar-dot {{ fill: #dc2626; stroke: #7f1d1d; stroke-width: 0.2; }}
    .bar-dot-top {{ fill: #b45309; stroke: #78350f; stroke-width: 0.2; }}
    .dim-line {{ stroke: #64748b; stroke-width: 0.25; }}
    .tbl-hdr {{ fill: #e2e8f0; }}
    .tbl-row {{ fill: #ffffff; }}
    .tbl-alt {{ fill: #f8fafc; }}
    .tbl-tot {{ fill: #e0f2fe; }}
    .tbl-grid {{ stroke: #cbd5e1; stroke-width: 0.22; }}
  </style>"#
    );

    let _ = writeln!(
        svg,
        r#"  <rect class="bg" x="0" y="0" width="420" height="297"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <rect class="frame" x="8" y="8" width="404" height="281"/>"#
    );

    // Header band
    let _ = writeln!(
        svg,
        r#"  <text class="title" x="14" y="18.5">DETALHAMENTO ESQUEMÁTICO DE ARMADURAS — PRÉ-DIMENSIONAMENTO</text>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="sub" x="14" y="24.5">Projeto: {}  ·  Concreto C{:.0} (fck = {:.0} MPa)  ·  Aço CA-50  ·  ABNT NBR 6118:2023</text>"#,
        escape_xml(project_name),
        fck,
        fck
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="406" y="18.5" text-anchor="end">Aço teórico: {} kg  |  Com +10%: {} kg ({} barras de 12 m)</text>"#,
        decimal(report.total_mass_kg(), 1),
        decimal(report.total_mass_with_waste_kg(), 1),
        report.total_bars_12m()
    );
    let _ = writeln!(
        svg,
        r#"  <text class="sub" x="406" y="24.5" text-anchor="end">{} vigas · {} pilares · {} lajes · {} sapatas · {} posições na tabela</text>"#,
        report.beams.len(),
        report.columns.len(),
        report.slabs.len(),
        report.footings.len(),
        report.schedule.len()
    );
    let _ = writeln!(
        svg,
        r##"  <line x1="12" y1="28.5" x2="408" y2="28.5" stroke="#94a3b8" stroke-width="0.35"/>"##
    );

    // -------------------------------------------------------------------------
    // Panel 1: Governing beam (x: 12..252, y: 33..159)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="12" y="33" width="240" height="126"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="17" y="40.5">1. VIGA DE REFERÊNCIA — SEÇÃO TRANSVERSAL E ELEVAÇÃO ESQUEMÁTICA</text>"#
    );
    if let Some(beam) = report
        .beams
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let span_match = structure.beams.iter().find_map(|line| {
            line.spans
                .iter()
                .enumerate()
                .find(|(idx, sp)| sp.beam == beam.beam_id && idx + 1 == beam.span_index)
                .map(|(_, sp)| sp)
        });
        let b_cm = beam.width_m * 100.0;
        let h_cm = beam.height_m * 100.0;
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="17" y="47">{} — seção {:.0} × {:.0} cm · vão L = {} m · massa = {} kg</text>"#,
            escape_xml(&beam.label),
            b_cm,
            h_cm,
            decimal(beam.length_m, 2),
            decimal(beam.mass_kg, 1)
        );

        // Cross section box (x: 24..72, y: 55..118)
        let max_w = 40.0_f64;
        let max_h = 56.0_f64;
        let scale = (max_w / beam.width_m.max(0.10)).min(max_h / beam.height_m.max(0.20));
        let sw = beam.width_m * scale;
        let sh = beam.height_m * scale;
        let sx = 46.0 - sw * 0.5;
        let sy = 56.0 + (max_h - sh) * 0.5;
        let cov = (0.03 * scale).clamp(2.0, sw * 0.25);

        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{sx:.1}" y="{sy:.1}" width="{sw:.1}" height="{sh:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="stirrup" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="1.2"/>"#,
            sx + cov,
            sy + cov,
            (sw - 2.0 * cov).max(2.0),
            (sh - 2.0 * cov).max(2.0)
        );
        // Draw bottom bar dots
        let draw_bar_row = |svg: &mut String, count: u32, y_pos: f64, cls: &str| {
            let n = count.clamp(2, 6);
            let inner_l = sx + cov + 1.5;
            let inner_r = sx + sw - cov - 1.5;
            for i in 0..n {
                let t = if n <= 1 {
                    0.5
                } else {
                    f64::from(i) / f64::from(n - 1)
                };
                let cx = inner_l + t * (inner_r - inner_l).max(0.0);
                let _ = writeln!(
                    svg,
                    r#"  <circle class="{cls}" cx="{cx:.1}" cy="{y_pos:.1}" r="1.25"/>"#
                );
            }
        };
        draw_bar_row(&mut svg, beam.bottom.count, sy + sh - cov - 1.4, "bar-dot");
        let top_count = beam.top.left().count.max(beam.top.right().count);
        draw_bar_row(&mut svg, top_count, sy + cov + 1.4, "bar-dot-top");

        // Dimensions for cross section
        let _ = writeln!(
            svg,
            r#"  <line class="dim-line" x1="{sx:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            sy + sh + 4.0,
            sx + sw,
            sy + sh + 4.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">b = {:.0} cm</text>"#,
            sx + sw * 0.5,
            sy + sh + 8.0,
            b_cm
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="end">h = {:.0} cm</text>"#,
            sx - 2.0,
            sy + sh * 0.5 + 1.0,
            h_cm
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="46" y="126" text-anchor="middle">c = 3 cm · estribos {}</text>"#,
            escape_xml(&beam.stirrups.notation())
        );

        // Longitudinal elevation (x: 84..242, y: 58..112)
        let ex = 94.0;
        let ew = 136.0;
        let ey = 64.0;
        let eh = 34.0;
        // Support pillars
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="10" height="{:.1}"/>"#,
            ex - 10.0,
            ey,
            eh + 12.0
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="10" height="{:.1}"/>"#,
            ex + ew,
            ey,
            eh + 12.0
        );
        // Beam elevation outline
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{ex:.1}" y="{ey:.1}" width="{ew:.1}" height="{eh:.1}"/>"#
        );
        // Stirrup vertical ticks
        for i in 1..17 {
            let tx = ex + ew * (f64::from(i) / 17.0);
            let _ = writeln!(
                svg,
                r#"  <line class="stirrup" x1="{tx:.1}" y1="{:.1}" x2="{tx:.1}" y2="{:.1}" stroke-opacity="0.55"/>"#,
                ey + 3.0,
                ey + eh - 3.0
            );
        }
        // Bottom positive bar with 90-degree hooks into supports
        let by = ey + eh - 4.0;
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-pos" points="{:.1},{:.1} {:.1},{by:.1} {:.1},{by:.1} {:.1},{:.1}"/>"#,
            ex - 6.5,
            by - 6.0,
            ex - 6.5,
            ex + ew + 6.5,
            ex + ew + 6.5,
            by - 6.0
        );
        // Top bars follow the actual schedule: continuous in frame-designed beams.
        let ty = ey + 4.0;
        let continuous_top = matches!(&beam.top, BeamTopBars::Continuous(_));
        let neg_len = ew * 0.30;
        if continuous_top {
            let _ = writeln!(
                svg,
                r#"  <polyline class="bar-neg" points="{:.1},{:.1} {:.1},{ty:.1} {:.1},{ty:.1} {:.1},{:.1}"/>"#,
                ex - 6.5,
                ty + 6.0,
                ex - 6.5,
                ex + ew + 6.5,
                ex + ew + 6.5,
                ty + 6.0
            );
        } else {
            let _ = writeln!(
                svg,
                r#"  <polyline class="bar-neg" points="{:.1},{:.1} {:.1},{ty:.1} {:.1},{ty:.1}"/>"#,
                ex - 6.5,
                ty + 6.0,
                ex - 6.5,
                ex + neg_len
            );
            let _ = writeln!(
                svg,
                r#"  <polyline class="bar-neg" points="{:.1},{ty:.1} {:.1},{ty:.1} {:.1},{:.1}"/>"#,
                ex + ew - neg_len,
                ex + ew + 6.5,
                ex + ew + 6.5,
                ty + 6.0
            );
        }

        // Callouts on elevation
        let top_description = if continuous_top {
            format!("Superior contínua: {}", beam.top.left().notation())
        } else {
            format!(
                "Superior esq.: {} · dir.: {}",
                beam.top.left().notation(),
                beam.top.right().notation()
            )
        };
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">{} (lb,nec = {:.0} cm, zona {})</text>"#,
            ex + ew * 0.5,
            ey - 4.0,
            escape_xml(&top_description),
            beam.top_anchorage_m * 100.0,
            if beam.height_m >= 0.30 { "má" } else { "boa" }
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">Inferior: {} · lb,nec = {:.0} cm (gancho 90°, boa aderência)</text>"#,
            ex + ew * 0.5,
            ey + eh + 8.5,
            escape_xml(&beam.bottom.notation()),
            beam.bottom_anchorage_m * 100.0
        );
        let top_length_description = if continuous_top {
            "Superior em todo o vão + ancoragem nas duas extremidades".to_owned()
        } else {
            format!(
                "Negativo = 0,30·L ({} m) + ancoragem",
                decimal((0.30 * beam.length_m).max(0.60), 2)
            )
        };
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">Vão L = {} m  ·  {}</text>"#,
            ex + ew * 0.5,
            ey + eh + 14.5,
            decimal(beam.length_m, 2),
            escape_xml(&top_length_description)
        );

        // Branson B12 cracked-section box
        let branson_line = if let Some(sp) = span_match {
            let stage = if sp.cracked {
                "Estádio II (fissurada)"
            } else {
                "Estádio I (não fissurada)"
            };
            format!(
                "Branson B12: {} · Mr = {} kN·m · I_eq/Ic = {:.0}% · As = {} cm² · flecha = {} mm (L/{:.0})",
                stage,
                decimal(sp.cracking_moment_knm, 1),
                sp.i_eq_ratio * 100.0,
                decimal(sp.branson_as_cm2, 2),
                decimal(sp.deflection_mm, 1),
                if sp.deflection_mm > 1e-6 {
                    sp.length_m * 1000.0 / sp.deflection_mm
                } else {
                    9999.0
                }
            )
        } else {
            format!(
                "Armadura inferior efetiva: {} cm² (req. {} cm²) · domínio {}",
                decimal(beam.bottom.provided_cm2, 2),
                decimal(beam.bottom.required_cm2, 2),
                if beam.flexural_domain_ok {
                    "OK"
                } else {
                    "EXCEDIDO"
                }
            )
        };
        let _ = writeln!(
            svg,
            r##"  <rect x="17" y="138" width="230" height="15" fill="#f1f5f9" stroke="#cbd5e1" stroke-width="0.25" rx="1"/>"##
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="21" y="144.2">{}</text>"#,
            escape_xml(&branson_line)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="21" y="149.8">Estribos: {} ({} un., compr. unit. {} m) · Verificação de biela comprimida: {}</text>"#,
            escape_xml(&beam.stirrups.notation()),
            beam.stirrups.count,
            decimal(beam.stirrups.length_each_m, 2),
            if beam.stirrups.shear_capacity_ok {
                "OK"
            } else {
                "EXCEDIDA"
            }
        );
        if let Some(e) = &beam.frame_envelope {
            let _ = writeln!(
                svg,
                r#"  <text class="muted" x="21" y="156">Pórtico 2D · ELU: Md+ = {} ({}) · Md− = {} ({}) kN·m · Vsd = {} kN ({})</text>"#,
                decimal(e.max_positive_moment_knm, 2),
                escape_xml(&e.positive_combination),
                decimal(e.min_negative_moment_knm, 2),
                escape_xml(&e.negative_combination),
                decimal(e.max_shear_kn, 2),
                escape_xml(&e.shear_combination),
            );
        }
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="54">Nenhuma viga armada neste projeto.</text>"#
        );
    }

    // -------------------------------------------------------------------------
    // Panel 2: Governing column (x: 258..408, y: 33..159)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="258" y="33" width="150" height="126"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="263" y="40.5">2. PILAR DE REFERÊNCIA — SEÇÃO E TRASPASSE</text>"#
    );
    if let Some(col) = report
        .columns
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="263" y="47">{} — {:.0} × {:.0} cm · H = {} m · ρ = {}%</text>"#,
            escape_xml(&col.label),
            col.width_m * 100.0,
            col.depth_m * 100.0,
            decimal(col.height_m, 2),
            decimal(col.steel_ratio * 100.0, 2)
        );
        // Column cross-section (centered around x=292, y=82)
        let scale = (36.0 / col.width_m.max(0.14)).min(36.0 / col.depth_m.max(0.14));
        let cw = col.width_m * scale;
        let ch = col.depth_m * scale;
        let cx = 292.0 - cw * 0.5;
        let cy = 80.0 - ch * 0.5;
        let cov = (0.03 * scale).clamp(2.2, cw.min(ch) * 0.25);
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{cx:.1}" y="{cy:.1}" width="{cw:.1}" height="{ch:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="stirrup" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="1.1"/>"#,
            cx + cov,
            cy + cov,
            (cw - 2.0 * cov).max(2.0),
            (ch - 2.0 * cov).max(2.0)
        );
        // Place corner + side bars
        let x0 = cx + cov + 1.3;
        let x1 = cx + cw - cov - 1.3;
        let y0 = cy + cov + 1.3;
        let y1 = cy + ch - cov - 1.3;
        let mut pts = vec![(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
        if col.longitudinal.count >= 6 {
            pts.push(((x0 + x1) * 0.5, y0));
            pts.push(((x0 + x1) * 0.5, y1));
        }
        if col.longitudinal.count >= 8 {
            pts.push((x0, (y0 + y1) * 0.5));
            pts.push((x1, (y0 + y1) * 0.5));
        }
        for (px, py) in pts {
            let _ = writeln!(
                svg,
                r#"  <circle class="bar-dot" cx="{px:.1}" cy="{py:.1}" r="1.3"/>"#
            );
        }
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">{:.0} × {:.0} cm</text>"#,
            cx + cw * 0.5,
            cy + ch + 6.0,
            col.width_m * 100.0,
            col.depth_m * 100.0
        );

        // Column elevation with lap splice (x: 342..366, y: 58..126)
        let elx = 346.0;
        let elw = 18.0;
        let ely = 62.0;
        let elh = 62.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{elx:.1}" y="{ely:.1}" width="{elw:.1}" height="{elh:.1}"/>"#
        );
        // Ties along elevation
        for i in 1..10 {
            let ty = ely + elh * (f64::from(i) / 10.0);
            let _ = writeln!(
                svg,
                r#"  <line class="stirrup" x1="{:.1}" y1="{ty:.1}" x2="{:.1}" y2="{ty:.1}"/>"#,
                elx + 2.0,
                elx + elw - 2.0
            );
        }
        // Longitudinal bars + starter lap zone extending above
        let lap_h = 13.0;
        let _ = writeln!(
            svg,
            r#"  <line class="bar-pos" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            elx + 3.2,
            ely - lap_h,
            elx + 3.2,
            ely + elh - 2.0
        );
        let _ = writeln!(
            svg,
            r#"  <line class="bar-pos" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            elx + elw - 3.2,
            ely - lap_h,
            elx + elw - 3.2,
            ely + elh - 2.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}">l0 = {:.0} cm</text>"#,
            elx + elw + 3.5,
            ely - 4.0,
            col.lap_length_m * 100.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}">H = {} m</text>"#,
            elx + elw + 3.5,
            ely + elh * 0.5,
            decimal(col.height_m, 2)
        );
        let _ = writeln!(
            svg,
            r##"  <rect x="263" y="138" width="140" height="15" fill="#f1f5f9" stroke="#cbd5e1" stroke-width="0.25" rx="1"/>"##
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="267" y="144.2">Longitudinal: {} (As = {} cm²) · l0 = {:.0} cm</text>"#,
            escape_xml(&col.longitudinal.notation()),
            decimal(col.longitudinal.provided_cm2, 2),
            col.lap_length_m * 100.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="267" y="149.8">Estribos: {} ({} un.) · massa total = {} kg</text>"#,
            escape_xml(&col.ties.notation()),
            col.ties.count,
            decimal(col.mass_kg, 1)
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="263" y="54">Nenhum pilar armado neste projeto.</text>"#
        );
    }

    // -------------------------------------------------------------------------
    // Panel 3: Footing + slabs summary (x: 12..204, y: 164..274)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="12" y="164" width="192" height="110"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="17" y="171.5">3. SAPATA DE REFERÊNCIA E RESUMO DE LAJES</text>"#
    );
    if let Some(ft) = report
        .footings
        .iter()
        .max_by(|a, b| a.mass_kg.total_cmp(&b.mass_kg))
    {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="17" y="178">{} — {:.0} × {:.0} × {:.0} cm · cobrimento c = 7 cm · massa = {} kg</text>"#,
            escape_xml(&ft.label),
            ft.width_m * 100.0,
            ft.depth_m * 100.0,
            ft.height_m * 100.0,
            decimal(ft.mass_kg, 1)
        );
        // Footing plan view (left)
        let fx = 24.0;
        let fy = 185.0;
        let fw = 44.0;
        let fh = 38.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{fx:.1}" y="{fy:.1}" width="{fw:.1}" height="{fh:.1}"/>"#
        );
        // Column footprint in center
        let _ = writeln!(
            svg,
            r##"  <rect x="{:.1}" y="{:.1}" width="10" height="10" fill="#94a3b8" stroke="#334155" stroke-width="0.3"/>"##,
            fx + fw * 0.5 - 5.0,
            fy + fh * 0.5 - 5.0
        );
        // Bottom mesh grid lines
        for i in 1..6 {
            let x = fx + fw * (f64::from(i) / 6.0);
            let y = fy + fh * (f64::from(i) / 6.0);
            let _ = writeln!(
                svg,
                r#"  <line class="bar-pos" x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{:.1}" stroke-width="0.35"/>"#,
                fy + 3.0,
                fy + fh - 3.0
            );
            let _ = writeln!(
                svg,
                r#"  <line class="bar-neg" x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke-width="0.35"/>"#,
                fx + 3.0,
                fx + fw - 3.0
            );
        }
        let _ = writeln!(
            svg,
            r#"  <text class="dim" x="{:.1}" y="{:.1}" text-anchor="middle">Planta ({:.0}×{:.0} cm)</text>"#,
            fx + fw * 0.5,
            fy + fh + 5.5,
            ft.width_m * 100.0,
            ft.depth_m * 100.0
        );

        // Footing cross-section (right)
        let sx = 92.0;
        let sy = 197.0;
        let sw = 94.0;
        let sh = 24.0;
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{sx:.1}" y="{sy:.1}" width="{sw:.1}" height="{sh:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <rect class="conc" x="{:.1}" y="{:.1}" width="14" height="12"/>"#,
            sx + sw * 0.5 - 7.0,
            sy - 12.0
        );
        // Bottom mesh bar with upturned 90-degree hooks
        let _ = writeln!(
            svg,
            r#"  <polyline class="bar-pos" points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}"/>"#,
            sx + 4.0,
            sy + 6.0,
            sx + 4.0,
            sy + sh - 4.0,
            sx + sw - 4.0,
            sy + sh - 4.0,
            sx + sw - 4.0,
            sy + 6.0
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">Malha X: {} · Malha Y: {}</text>"#,
            sx + sw * 0.5,
            sy + sh + 6.5,
            escape_xml(&ft.bars_x.notation()),
            escape_xml(&ft.bars_y.notation())
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="185">Nenhuma sapata armada neste projeto.</text>"#
        );
    }

    // Slabs summary strip at bottom of panel 3
    let _ = writeln!(
        svg,
        r##"  <line x1="17" y1="236" x2="199" y2="236" stroke="#cbd5e1" stroke-width="0.25"/>"##
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="17" y="242.5">Lajes armadas ({})</text>"#,
        report.slabs.len()
    );
    if report.slabs.is_empty() {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="17" y="249">Nenhuma laje armada neste projeto.</text>"#
        );
    } else {
        for (idx, slab) in report.slabs.iter().take(4).enumerate() {
            let y = 249.0 + (idx as f64) * 5.6;
            let kind = match slab.kind {
                SlabKind::Solid => "maciça",
                SlabKind::Ribbed => "treliçada (equiv.)",
            };
            let _ = writeln!(
                svg,
                r#"  <text class="lbl" x="17" y="{y:.1}">{} ({}) — {}: X {} · Y {} ({} kg)</text>"#,
                escape_xml(&slab.label),
                escape_xml(&slab.room),
                kind,
                escape_xml(&slab.main.notation()),
                escape_xml(&slab.distribution.notation()),
                decimal(slab.mass_kg, 1)
            );
        }
        if report.slabs.len() > 4 {
            let _ = writeln!(
                svg,
                r#"  <text class="muted" x="17" y="271.5">... e mais {} laje(s) na tabela completa de aço.</text>"#,
                report.slabs.len() - 4
            );
        }
    }

    // -------------------------------------------------------------------------
    // Panel 4: CA-50 Diameter Summary Table (x: 210..408, y: 164..274)
    // -------------------------------------------------------------------------
    let _ = writeln!(
        svg,
        r#"  <rect class="panel" x="210" y="164" width="198" height="110"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="panel-hdr" x="215" y="171.5">4. RESUMO DE AÇO POR BITOLA — CA-50 (BARRAS DE 12 m)</text>"#
    );
    let tx = 215.0;
    let ty = 176.0;
    let tw = 188.0;
    let row_h = 7.2;
    let cols = [0.0, 28.0, 68.0, 108.0, 148.0, 188.0];

    // Table header
    let _ = writeln!(
        svg,
        r#"  <rect class="tbl-hdr" x="{tx:.1}" y="{ty:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
    );
    let headers = [
        ("Bitola", cols[0] + 3.0, "start"),
        ("Compr. (m)", cols[2] - 3.0, "end"),
        ("Barras 12 m", cols[3] - 3.0, "end"),
        ("Teórica (kg)", cols[4] - 3.0, "end"),
        ("+10% (kg)", cols[5] - 3.0, "end"),
    ];
    for (label, offset, anchor) in headers {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="{anchor}">{label}</text>"#,
            tx + offset,
            ty + 4.9
        );
    }

    for (idx, row) in report.by_diameter.iter().enumerate() {
        let ry = ty + row_h * ((idx + 1) as f64);
        let bg_cls = if idx % 2 == 0 { "tbl-row" } else { "tbl-alt" };
        let _ = writeln!(
            svg,
            r#"  <rect class="{bg_cls}" x="{tx:.1}" y="{ry:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}">Ø{} mm</text>"#,
            tx + cols[0] + 3.0,
            ry + 4.9,
            diameter_text(row.diameter_mm)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[2] - 3.0,
            ry + 4.9,
            decimal(row.total_length_m, 1)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[3] - 3.0,
            ry + 4.9,
            row.bars_12m
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[4] - 3.0,
            ry + 4.9,
            decimal(row.mass_kg, 1)
        );
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
            tx + cols[5] - 3.0,
            ry + 4.9,
            decimal(row.mass_with_waste_kg, 1)
        );
    }

    let tot_y = ty + row_h * ((report.by_diameter.len() + 1) as f64);
    let total_len: f64 = report.by_diameter.iter().map(|r| r.total_length_m).sum();
    let _ = writeln!(
        svg,
        r#"  <rect class="tbl-tot" x="{tx:.1}" y="{tot_y:.1}" width="{tw:.1}" height="{row_h:.1}"/>"#
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}">TOTAL</text>"#,
        tx + cols[0] + 3.0,
        tot_y + 4.9
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[2] - 3.0,
        tot_y + 4.9,
        decimal(total_len, 1)
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[3] - 3.0,
        tot_y + 4.9,
        report.total_bars_12m()
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[4] - 3.0,
        tot_y + 4.9,
        decimal(report.total_mass_kg(), 1)
    );
    let _ = writeln!(
        svg,
        r#"  <text class="lbl-b" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#,
        tx + cols[5] - 3.0,
        tot_y + 4.9,
        decimal(report.total_mass_with_waste_kg(), 1)
    );

    // Table outer border
    let tbl_h = row_h * ((report.by_diameter.len() + 2) as f64);
    let _ = writeln!(
        svg,
        r#"  <rect fill="none" class="tbl-grid" x="{tx:.1}" y="{ty:.1}" width="{tw:.1}" height="{tbl_h:.1}"/>"#
    );

    // Warnings / notes inside Panel 4 below table
    let note_y = (tot_y + row_h + 7.0).min(258.0);
    if report.warnings.is_empty() {
        let _ = writeln!(
            svg,
            r#"  <text class="muted" x="215" y="{note_y:.1}">Sem alertas de domínio, biela comprimida ou espaçamento nesta estimativa.</text>"#
        );
    } else {
        let _ = writeln!(
            svg,
            r#"  <text class="lbl-b" x="215" y="{note_y:.1}">Alertas ({}):</text>"#,
            report.warnings.len()
        );
        for (idx, w) in report.warnings.iter().take(2).enumerate() {
            let trimmed: String = w.chars().take(78).collect();
            let suffix = if w.chars().count() > 78 { "..." } else { "" };
            let _ = writeln!(
                svg,
                r#"  <text class="muted" x="215" y="{:.1}">• {}{suffix}</text>"#,
                note_y + 5.0 + (idx as f64) * 4.8,
                escape_xml(&trimmed)
            );
        }
    }

    // Footer disclaimer
    let _ = writeln!(
        svg,
        r#"  <text class="muted" x="14" y="284.5">CARÁTER PRELIMINAR: Estimativa pela ABNT NBR 6118:2023 (flexão/cortante, Branson B12, ancoragem §9.4, traspasse §9.5, +10% perda). Não substitui projeto executivo nem ART/RRT de engenheiro habilitado.</text>"#
    );
    let _ = writeln!(svg, "</svg>");
    svg
}
