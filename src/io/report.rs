//! Pre-sizing report (*memorial de pré-dimensionamento*, #103): a multi-page
//! PDF for a licensed professional to review and sign. It holds the inputs,
//! the assumptions and applicability limits, every structural and
//! electrical result with its traffic light, the warnings, and the
//! drawings (formwork plan, electrical plan, beam forces, single-line
//! diagram), with a review box for the professional's name, registration
//! and ART or RRT.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::domain::discipline::Discipline;
use crate::domain::distribution::{phase_label, section_label, single_line_svg};
use crate::domain::limits;
use crate::domain::model::Project;
use crate::domain::structure::{beam_diagrams_svg, section_cm};
use crate::domain::verdict::{Light, Verdict, safety_summary};

const W: f64 = 210.0;
const H: f64 = 297.0;
/// ABNT NBR 16752 margins: 25 mm for binding on the left, 7 mm elsewhere,
/// plus room for the running header and footer.
const LEFT: f64 = 25.0;
const RIGHT: f64 = 203.0;
const TOP: f64 = 26.0;
const BOTTOM: f64 = 280.0;
pub(super) const INK: &str = "#1d2836";
pub(super) const MUTED: &str = "#5b6b7c";
const TOTAL: &str = "{{TOTAL}}";

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn decimal(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    // "-0,0" for a value that rounds to zero reads as a sign error.
    let text = if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text[1..].to_owned()
    } else {
        text
    };
    text.replace('.', ",")
}

fn light_color(light: Light) -> &'static str {
    match light {
        Light::Green => "#22925f",
        Light::Yellow => "#d5841b",
        Light::Red => "#cc3f33",
    }
}

/// Splits `text` into lines of about `chars` characters, at spaces.
fn wrap(text: &str, chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > chars {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A4 pages written top to bottom, starting a new page when one fills up.
pub(super) struct Pages {
    project: String,
    done: Vec<String>,
    body: String,
    y: f64,
}

impl Pages {
    pub(super) fn new(project: &str) -> Self {
        Self {
            project: project.to_owned(),
            done: Vec::new(),
            body: String::new(),
            y: TOP,
        }
    }

    fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, color: &str, value: &str) {
        self.body.push_str(&format!(
            "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"Arial, sans-serif\" font-size=\"{size}\"{} fill=\"{color}\">{}</text>\n",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        ));
    }

    fn dot(&mut self, x: f64, y: f64, light: Light) {
        self.body.push_str(&format!(
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.3\" fill=\"{}\"/>\n",
            light_color(light)
        ));
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, color: &str) {
        self.body.push_str(&format!(
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{color}\" stroke-width=\"{width}\"/>\n"
        ));
    }

    fn page_break(&mut self) {
        let number = self.done.len() + 1;
        let mut page = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}mm\" height=\"{H}mm\" viewBox=\"0 0 {W} {H}\">\n\
             <rect width=\"{W}\" height=\"{H}\" fill=\"#ffffff\"/>\n"
        );
        page.push_str(&format!(
            "<text x=\"{LEFT}\" y=\"14\" font-family=\"Arial, sans-serif\" font-size=\"2.6\" fill=\"{MUTED}\">{} · Memorial de pré-dimensionamento</text>\n\
             <line x1=\"{LEFT}\" y1=\"16\" x2=\"{RIGHT}\" y2=\"16\" stroke=\"#b8c4d0\" stroke-width=\"0.2\"/>\n\
             <line x1=\"{LEFT}\" y1=\"286\" x2=\"{RIGHT}\" y2=\"286\" stroke=\"#b8c4d0\" stroke-width=\"0.2\"/>\n\
             <text x=\"{LEFT}\" y=\"290.5\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"{MUTED}\">Pré-dimensionamento para revisão de profissional habilitado. Não é projeto executivo.</text>\n\
             <text x=\"{RIGHT}\" y=\"290.5\" font-family=\"Arial, sans-serif\" font-size=\"2.4\" fill=\"{MUTED}\" text-anchor=\"end\">{number}/{TOTAL}</text>\n",
            esc(&self.project)
        ));
        page.push_str(&std::mem::take(&mut self.body));
        page.push_str("</svg>\n");
        self.done.push(page);
        self.y = TOP;
    }

    /// Makes room for `height` mm, breaking the page if needed.
    fn room(&mut self, height: f64) {
        if self.y + height > BOTTOM {
            self.page_break();
        }
    }

    pub(super) fn heading(&mut self, title: &str) {
        self.room(16.0);
        self.y += 4.0;
        let y = self.y;
        self.text(LEFT, y, 4.6, true, INK, title);
        self.y += 2.5;
        let y = self.y;
        self.line(LEFT, y, RIGHT, y, 0.35, INK);
        self.y += 5.0;
    }

    pub(super) fn subheading(&mut self, title: &str) {
        self.room(12.0);
        self.y += 2.0;
        let y = self.y;
        self.text(LEFT, y, 3.4, true, INK, title);
        self.y += 5.0;
    }

    pub(super) fn paragraph(&mut self, text: &str, size: f64, color: &str) {
        let chars = ((RIGHT - LEFT) / (size * 0.5)) as usize;
        for line in wrap(text, chars) {
            self.room(size * 1.5);
            let y = self.y;
            self.text(LEFT, y, size, false, color, &line);
            self.y += size * 1.45;
        }
        self.y += size * 0.4;
    }

    /// A table whose rows may start with a traffic light.
    fn table(&mut self, headers: &[&str], widths: &[f64], rows: &[(Option<Light>, Vec<String>)]) {
        let total: f64 = widths.iter().sum();
        let scale = (RIGHT - LEFT) / total;
        let row_h = 5.0;
        let header = |pages: &mut Pages| {
            let y = pages.y;
            pages.body.push_str(&format!(
                "<rect x=\"{LEFT}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{row_h}\" fill=\"#edf2f7\"/>\n",
                y - 3.6,
                RIGHT - LEFT
            ));
            let mut x = LEFT;
            for (title, w) in headers.iter().zip(widths) {
                pages.text(x + 1.0, y, 2.5, true, INK, title);
                x += w * scale;
            }
            pages.y += row_h;
        };
        self.room(row_h * 2.0);
        header(self);
        for (light, cells) in rows {
            if self.y + row_h > BOTTOM {
                self.page_break();
                header(self);
            }
            let y = self.y;
            let mut x = LEFT;
            for (i, (cell, w)) in cells.iter().zip(widths).enumerate() {
                if i == 0
                    && let Some(light) = light
                {
                    self.dot(x + 1.6, y - 0.9, *light);
                    self.text(x + 3.8, y, 2.5, false, INK, cell);
                } else {
                    self.text(x + 1.0, y, 2.5, false, INK, cell);
                }
                x += w * scale;
            }
            self.line(LEFT, y + 1.4, RIGHT, y + 1.4, 0.1, "#d5dde6");
            self.y += row_h;
        }
        self.y += 3.0;
    }

    pub(super) fn finish(mut self) -> Vec<String> {
        if !self.body.is_empty() {
            self.page_break();
        }
        let total = self.done.len().to_string();
        self.done
            .into_iter()
            .map(|page| page.replace(TOTAL, &total))
            .collect()
    }
}

/// The light of each element by its name in the verdicts.
fn lights(verdicts: &[Verdict]) -> HashMap<String, Light> {
    verdicts
        .iter()
        .map(|v| (v.element.clone(), v.light()))
        .collect()
}

/// The report's own A4 pages (without the drawings).
pub fn report_pages(project: &Project, date: &str) -> Vec<String> {
    let summary = safety_summary(project);
    let light_of = lights(&summary.verdicts);
    let report = project.structure_report();
    let reinforcement = project.cached_reinforcement_report();
    let schedule = project.load_schedule();
    let s = project.structure.settings;
    let tb = &project.sheet.title_block;
    let mut pages = Pages::new(&project.name);

    // Cover.
    pages.y = 60.0;
    pages.text(
        LEFT,
        60.0,
        8.0,
        true,
        INK,
        "MEMORIAL DE PRÉ-DIMENSIONAMENTO",
    );
    pages.text(
        LEFT,
        70.0,
        4.0,
        false,
        MUTED,
        "Estrutura e instalações elétricas · para revisão profissional",
    );
    pages.line(LEFT, 76.0, RIGHT, 76.0, 0.5, INK);
    let mut y = 88.0;
    for (label, value) in [
        ("Projeto", project.name.clone()),
        ("Cliente", tb.client.clone()),
        ("Endereço", tb.address.clone()),
        ("Elaborado por", tb.author.clone()),
        ("Registro", tb.registration.clone()),
        (
            "Data",
            if tb.date.trim().is_empty() {
                date.to_owned()
            } else {
                tb.date.clone()
            },
        ),
    ] {
        pages.text(LEFT, y, 3.0, true, MUTED, label);
        pages.text(
            LEFT + 35.0,
            y,
            3.4,
            false,
            INK,
            if value.trim().is_empty() {
                "—"
            } else {
                value.trim()
            },
        );
        y += 7.0;
    }
    y += 6.0;
    if let Some(worst) = summary.worst() {
        pages.dot(LEFT + 2.0, y - 1.2, worst);
        pages.text(
            LEFT + 6.0,
            y,
            4.0,
            true,
            light_color(worst),
            &format!("Situação geral: {}", worst.meaning()),
        );
        y += 7.0;
        pages.text(
            LEFT + 6.0,
            y,
            3.2,
            false,
            INK,
            &format!(
                "{} vermelho(s) · {} amarelo(s) · {} verde(s) em {} elemento(s) verificados",
                summary.red,
                summary.yellow,
                summary.green,
                summary.verdicts.len()
            ),
        );
        y += 10.0;
    }
    pages.body.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"22\" fill=\"#fdf3e7\" stroke=\"#d5841b\" stroke-width=\"0.3\"/>\n",
        LEFT - 3.0,
        RIGHT - LEFT + 6.0
    ));
    pages.y = y + 6.0;
    pages.paragraph(
        "Este documento é um pré-dimensionamento por regras práticas, para orientar decisões. Não é projeto estrutural nem elétrico executivo. Construir exige um profissional habilitado, com ART (CREA) ou RRT (CAU), e as licenças da prefeitura. Os métodos, normas e limites estão em docs/engineering.md do Trazo.",
        3.0,
        INK,
    );
    let y = 200.0;
    pages.text(
        LEFT,
        y,
        3.6,
        true,
        INK,
        "Revisão do profissional habilitado",
    );
    pages.body.push_str(&format!(
        "<rect x=\"{LEFT}\" y=\"{:.1}\" width=\"{:.1}\" height=\"64\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"0.35\"/>\n",
        y + 3.0,
        RIGHT - LEFT
    ));
    let mut ry = y + 12.0;
    for label in [
        "Nome",
        "Registro (CREA/CAU)",
        "ART/RRT nº",
        "Data",
        "Assinatura",
    ] {
        pages.text(LEFT + 3.0, ry, 3.0, false, INK, label);
        pages.line(LEFT + 45.0, ry + 0.5, RIGHT - 4.0, ry + 0.5, 0.2, MUTED);
        ry += 9.0;
    }
    pages.text(
        LEFT + 3.0,
        ry + 1.0,
        3.0,
        false,
        INK,
        "Parecer:  ( ) aprovado    ( ) aprovado com ressalvas    ( ) reprovado",
    );
    pages.page_break();

    // 1. Inputs.
    pages.heading("1. Dados de entrada");
    pages.subheading("Edificação");
    let storeys = limits::storeys(project);
    let rooms: Vec<(Option<Light>, Vec<String>)> = project
        .rooms
        .iter()
        .map(|room| {
            (
                None,
                vec![
                    room.display_name().to_owned(),
                    format!("{} m²", decimal(project.room_area(room).unwrap_or(0.0), 2)),
                ],
            )
        })
        .collect();
    pages.paragraph(
        &format!(
            "{} pavimento(s) com estrutura, pé-direito {} m, {} cômodo(s), {} parede(s).",
            storeys,
            decimal(project.wall_height, 2),
            project.rooms.len(),
            project.walls.len()
        ),
        3.0,
        INK,
    );
    if !rooms.is_empty() {
        pages.table(&["Cômodo", "Área (eixos)"], &[3.0, 1.0], &rooms);
    }
    pages.subheading("Materiais e cargas");
    let mut material_rows: Vec<(Option<Light>, Vec<String>)> = vec![
        (
            None,
            vec!["Concreto (fck)".into(), format!("{:.0} MPa", s.fck_mpa)],
        ),
        (None, vec!["Aço".into(), "CA-50".into()]),
        (
            None,
            vec![
                "Tensão admissível do solo".into(),
                format!("{:.0} kPa", s.soil_kpa),
            ],
        ),
        (
            None,
            vec![
                "Ocupação (NBR 6120 / 8681)".into(),
                format!(
                    "{} · ψ0 = {} · ψ1 = {} · ψ2 = {}",
                    s.occupancy.label(),
                    decimal(s.occupancy.psi0(), 2),
                    decimal(s.occupancy.psi1(), 2),
                    decimal(s.occupancy.psi2(), 2)
                ),
            ],
        ),
        (
            None,
            vec![
                "Vento (NBR 6123)".into(),
                format!(
                    "V0 = {:.0} m/s · cat. {} · S1 = {} · S3 = {}",
                    project.structure.wind.v0_ms,
                    project.structure.wind.category,
                    decimal(project.structure.wind.s1, 2),
                    decimal(project.structure.wind.s3, 2)
                ),
            ],
        ),
        (
            None,
            vec![
                "Sobrecarga".into(),
                format!("{} kN/m²", decimal(s.live_kn_m2, 1)),
            ],
        ),
        (
            None,
            vec![
                "Revestimento".into(),
                format!("{} kN/m²", decimal(s.finish_kn_m2, 1)),
            ],
        ),
        (
            None,
            vec![
                "Alvenaria".into(),
                format!("{} kN/m³", decimal(s.masonry_kn_m3, 0)),
            ],
        ),
        (
            None,
            vec![
                "Tensão dos circuitos".into(),
                format!(
                    "{:.0} V (uso específico {:.0} V)",
                    project.electrical.voltage_v, project.electrical.specific_voltage_v
                ),
            ],
        ),
        (
            None,
            vec!["Ligação em uso".into(), schedule.supply.label().to_owned()],
        ),
    ];
    if !report.timber_posts.is_empty() || !report.eave_beams.is_empty() {
        let species = project.structure.post_species;
        material_rows.push((
            None,
            vec![
                "Madeira (mourões/vigas)".into(),
                format!(
                    "{} ({}) · {}",
                    species.label(),
                    species.class().label(),
                    if project.structure.post_treated {
                        "tratada"
                    } else {
                        "sem tratamento"
                    }
                ),
            ],
        ));
    }
    pages.table(&["Parâmetro", "Valor"], &[3.0, 2.0], &material_rows);

    // 2. Assumptions and limits.
    pages.heading("2. Premissas, métodos e limites");
    for assumption in &summary.assumptions {
        pages.paragraph(&format!("• {assumption}"), 3.0, INK);
    }
    pages.subheading("Limites de aplicação");
    let refusal = limits::structural_refusal(project);
    let refused = limits::element_violations(&report);
    let mut limit_rows: Vec<(Option<Light>, Vec<String>)> = Vec::new();
    for (name, value, limit, ok) in [
        (
            "Pavimentos",
            storeys.to_string(),
            format!("até {}", limits::MAX_STOREYS),
            storeys <= limits::MAX_STOREYS,
        ),
        (
            "Pé-direito",
            format!("{} m", decimal(project.wall_height, 2)),
            format!("até {} m", decimal(limits::MAX_STOREY_HEIGHT_M, 1)),
            project.wall_height <= limits::MAX_STOREY_HEIGHT_M,
        ),
        (
            "Sobrecarga",
            format!("{} kN/m²", decimal(s.live_kn_m2, 1)),
            format!("até {} kN/m²", decimal(limits::MAX_LIVE_LOAD_KN_M2, 1)),
            s.live_kn_m2 <= limits::MAX_LIVE_LOAD_KN_M2,
        ),
    ] {
        limit_rows.push((
            Some(if ok { Light::Green } else { Light::Red }),
            vec![name.to_owned(), value, limit],
        ));
    }
    for violation in &refused {
        limit_rows.push((
            Some(Light::Red),
            vec![
                violation.element.clone(),
                decimal(violation.value, 2),
                format!("limite {}", decimal(violation.limit, 2)),
            ],
        ));
    }
    pages.table(
        &["Limite", "Projeto", "Aplicação"],
        &[3.0, 2.0, 2.0],
        &limit_rows,
    );
    if let Some(refusal) = &refusal {
        pages.paragraph(
            &format!("Dimensionamento estrutural recusado: {refusal}"),
            3.0,
            "#cc3f33",
        );
    }

    // 3. Structure.
    pages.heading("3. Resultados · estrutura");
    if refusal.is_some() {
        pages.paragraph(
            "O projeto está fora dos limites de aplicação: nenhum elemento estrutural foi dimensionado.",
            3.0,
            "#cc3f33",
        );
    } else if report.slabs.is_empty()
        && report.beams.is_empty()
        && report.columns.is_empty()
        && report.masonry.panels.is_empty()
    {
        pages.paragraph("Nenhum elemento estrutural no projeto.", 3.0, MUTED);
    } else {
        let refused_names: Vec<&str> = refused.iter().map(|v| v.element.as_str()).collect();
        if !report.masonry.bands.is_empty() {
            pages.subheading("Vergas e contravergas");
            let rows: Vec<_> = report
                .masonry
                .bands
                .iter()
                .map(|band| {
                    let name = format!("{} {}", band.kind.label(), band.label);
                    (
                        light_of.get(&name).copied(),
                        vec![
                            band.label.clone(),
                            band.kind.label().to_owned(),
                            decimal(band.clear_span_m, 2),
                            format!(
                                "{}/{}",
                                decimal(band.bearing_left_m * 100.0, 0),
                                decimal(band.bearing_right_m * 100.0, 0)
                            ),
                            section_cm(band.width_m, band.height_m),
                            band.bars.clone(),
                        ],
                    )
                })
                .collect();
            pages.table(
                &["Marca", "Tipo", "Vão (m)", "Apoios (cm)", "b×h (cm)", "Aço"],
                &[1.2, 1.7, 1.3, 1.8, 1.6, 1.5],
                &rows,
            );
        }
        if !report.masonry.panels.is_empty() {
            pages.subheading("Painéis de alvenaria");
            let rows: Vec<_> = report
                .masonry
                .panels
                .iter()
                .map(|panel| {
                    (
                        light_of.get(&format!("Alvenaria {}", panel.label)).copied(),
                        vec![
                            panel.label.clone(),
                            panel.kind.label().to_owned(),
                            format!(
                                "{}/{}",
                                decimal(panel.slenderness, 1),
                                decimal(panel.slenderness_limit, 0)
                            ),
                            format!(
                                "{}/{}",
                                decimal(panel.unbraced_m, 2),
                                decimal(panel.unbraced_limit_m, 2)
                            ),
                            if panel.compression_capacity_mpa > 0.0 {
                                format!(
                                    "{}/{}",
                                    decimal(panel.compression_mpa, 2),
                                    decimal(panel.compression_capacity_mpa, 2)
                                )
                            } else {
                                "—".to_owned()
                            },
                        ],
                    )
                })
                .collect();
            pages.table(
                &["Painel", "Uso", "h/t", "Sem amarração (m)", "σd/fd (MPa)"],
                &[1.2, 1.5, 1.4, 2.3, 2.0],
                &rows,
            );
        }
        let windy_panels: Vec<_> = report
            .masonry
            .panels
            .iter()
            .filter_map(|panel| Some((panel, panel.wind.as_ref()?)))
            .collect();
        if !windy_panels.is_empty() {
            pages.subheading("Muros sob vento e cintas");
            let rows: Vec<_> = windy_panels
                .iter()
                .map(|(panel, wind)| {
                    let bands = wind
                        .bond_beam
                        .as_ref()
                        .map_or("sem apoio".to_owned(), |bond| {
                            format!(
                                "{} + topo / {} m",
                                bond.intermediate_count,
                                decimal(bond.spacing_m, 2)
                            )
                        });
                    (
                        light_of.get(&format!("Alvenaria {}", panel.label)).copied(),
                        vec![
                            panel.label.clone(),
                            decimal(wind.q_kpa, 2),
                            format!(
                                "{}/{}",
                                decimal(wind.moment_knm_m, 2),
                                decimal(wind.capacity_knm_m, 2)
                            ),
                            bands,
                            wind.bond_beam.as_ref().map_or("—".to_owned(), |bond| {
                                format!(
                                    "{}×{} / {}",
                                    bond.across_m
                                        .map_or(">40".to_owned(), |w| decimal(w * 100.0, 0)),
                                    decimal(bond.height_m * 100.0, 0),
                                    bond.bars
                                )
                            }),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Painel",
                    "q (kPa)",
                    "Md/MRd (kN·m/m)",
                    "Cintas",
                    "Seção (cm)/aço",
                ],
                &[1.1, 1.3, 2.1, 2.2, 3.0],
                &rows,
            );
        }
        if !report.wind.global.is_empty() {
            pages.subheading("Estabilidade global sob vento");
            let rows: Vec<_> = report
                .wind
                .global
                .iter()
                .map(|global| {
                    (
                        light_of
                            .get(&format!("Estabilidade global {}", global.direction))
                            .copied(),
                        vec![
                            global.direction.to_owned(),
                            format!(
                                "{} / {}",
                                decimal(global.wind_kn, 1),
                                decimal(global.roof_uplift_kn, 1)
                            ),
                            format!("{:.2}/{:.2}", global.alpha, global.alpha_limit),
                            format!(
                                "{}/{}",
                                decimal(global.drift_mm, 1),
                                decimal(global.drift_limit_mm, 1)
                            ),
                            format!(
                                "{}/{}",
                                decimal(global.overturning_ratio, 2),
                                decimal(global.sliding_fs, 2)
                            ),
                            if global.supports_ok && global.bracing_ok {
                                "completo".to_owned()
                            } else {
                                "incompleto".to_owned()
                            },
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Direção",
                    "W / U (kN)",
                    "α / limite",
                    "Δ / limite (mm)",
                    "Tomb./Desl.",
                    "Caminho",
                ],
                &[1.0, 1.5, 1.3, 1.9, 1.7, 1.8],
                &rows,
            );
            for brace in &report.wind.bracing {
                pages.paragraph(&format!("Cobertura {}: contraventamento lateral exige {} kN ({}); resistência informada {}.",
                    brace.label, decimal(brace.demand_kn, 1), brace.tie_spec,
                    brace.capacity_kn.map_or("não informada".to_owned(), |kn| format!("{} kN", decimal(kn, 1)))), 3.0, INK);
            }
            for roof_wind in &report.wind.roofs {
                if roof_wind.lifts() {
                    pages.paragraph(&format!("Cobertura {}: sucção líquida de cálculo {} kN/m² → ancorar apoios para {} kN ({}).",
                        roof_wind.label, decimal(roof_wind.net_design_kpa, 2), decimal(roof_wind.anchor_kn, 1), roof_wind.anchor_spec), 3.0, INK);
                }
            }
        }
        if !report.slabs.is_empty() {
            pages.subheading("Lajes");
            let rows: Vec<_> = report
                .slabs
                .iter()
                .map(|slab| {
                    let name = format!("Laje {} ({})", slab.label, slab.room);
                    let out = refused_names.contains(&name.as_str());
                    (
                        light_of.get(&name).copied(),
                        vec![
                            format!("{} · {}", slab.label, slab.room),
                            slab.kind.label().to_owned(),
                            format!("{} × {} m", decimal(slab.lx_m, 2), decimal(slab.ly_m, 2)),
                            slab.panels.len().to_string(),
                            if out {
                                "fora dos limites".into()
                            } else {
                                format!("{:.0} cm", slab.thickness_m * 100.0)
                            },
                            if out {
                                "—".into()
                            } else {
                                format!("{:.0} cm", slab.thickness_min_m * 100.0)
                            },
                            format!("{} kN/m²", decimal(slab.load_kn_m2, 2)),
                            if slab.kind == crate::domain::structure::SlabKind::Solid {
                                format!(
                                    "{}/{}",
                                    decimal(slab.moment_x_knm_m, 2),
                                    decimal(slab.moment_y_knm_m, 2)
                                )
                            } else {
                                "—".to_owned()
                            },
                            if slab.kind == crate::domain::structure::SlabKind::Solid {
                                format!(
                                    "{}/{} mm",
                                    decimal(slab.deflection_mm, 1),
                                    decimal(slab.deflection_limit_mm, 1)
                                )
                            } else {
                                "fabricante".to_owned()
                            },
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Laje",
                    "Tipo",
                    "Vãos",
                    "Painéis",
                    "Esp.",
                    "Mín.",
                    "Carga",
                    "Mx/My",
                    "Flecha/lim.",
                ],
                &[2.2, 1.2, 1.5, 0.9, 1.0, 1.0, 1.2, 1.3, 1.5],
                &rows,
            );
        }
        if !report.concentrated_loads.is_empty() {
            pages.subheading("Cargas concentradas");
            let rows: Vec<_> = report
                .concentrated_loads
                .iter()
                .map(|load| {
                    (
                        None,
                        vec![
                            load.label.clone(),
                            load.kind.label().to_owned(),
                            format!("{} kN", decimal(load.load_kn, 1)),
                            load.support.clone(),
                            if load.local_moment_knm_m > 0.0 {
                                format!("{} kN·m/m", decimal(load.local_moment_knm_m, 1))
                            } else {
                                "—".to_owned()
                            },
                            if !load.supported {
                                "sem apoio"
                            } else if load.slab.is_some() && load.punching_ratio.is_none() {
                                "revisar sistema"
                            } else if load.punching_ratio.is_some_and(|ratio| ratio > 1.0) {
                                "punção excedida"
                            } else {
                                "atendida"
                            }
                            .to_owned(),
                        ],
                    )
                })
                .collect();
            pages.table(
                &["Carga", "Tipo", "Valor", "Caminho", "M local", "Situação"],
                &[1.1, 1.8, 1.2, 3.2, 1.5, 1.3],
                &rows,
            );
        }
        if !report.beams.is_empty() {
            pages.subheading("Vigas (valores característicos)");
            let mut rows = Vec::new();
            for line in &report.beams {
                for (i, span) in line.spans.iter().enumerate() {
                    let name = if line.spans.len() > 1 {
                        format!("Viga {} · vão {}", line.label, i + 1)
                    } else {
                        format!("Viga {}", line.label)
                    };
                    let out = refused_names.contains(&name.as_str());
                    let label = if line.spans.len() > 1 {
                        format!("{} v{}", line.label, i + 1)
                    } else {
                        line.label.clone()
                    };
                    rows.push((
                        light_of.get(&name).copied(),
                        if out {
                            vec![
                                label,
                                format!("{} m", decimal(span.length_m, 2)),
                                "fora dos limites".into(),
                                String::new(),
                                String::new(),
                                String::new(),
                                String::new(),
                            ]
                        } else {
                            vec![
                                label,
                                format!("{} m", decimal(span.length_m, 2)),
                                section_cm(span.width_m, span.height_m),
                                decimal(span.dead_kn_m + span.live_kn_m, 1),
                                decimal(span.moment_left.min(span.moment_right), 1),
                                decimal(span.moment_span, 1),
                                format!(
                                    "{}/{}",
                                    decimal(span.deflection_mm, 1),
                                    decimal(span.limit_mm, 0)
                                ),
                            ]
                        },
                    ));
                }
            }
            pages.table(
                &[
                    "Viga",
                    "Vão",
                    "b×h (cm)",
                    "q (kN/m)",
                    "M− (kN·m)",
                    "M+ (kN·m)",
                    "Flecha (mm)",
                ],
                &[1.6, 1.3, 1.5, 1.3, 1.5, 1.5, 1.6],
                &rows,
            );
        }
        if !report.loads.is_empty() {
            let n6120 = &report.loads.nbr6120;
            pages.subheading("Casos de carga (NBR 6120:2019) e vento na edificação (NBR 6123)");
            pages.paragraph(
                &format!(
                    "G1 (peso próprio estrutural) = {} kN · G2 (permanente não estrutural) = {} kN · Q (sobrecarga) = {} kN · Total gravitacional = {} kN. {}",
                    decimal(n6120.total_g1_kn, 1),
                    decimal(n6120.total_g2_kn, 1),
                    decimal(n6120.total_q_kn, 1),
                    decimal(n6120.total_gravity_kn(), 1),
                    n6120.live_reduction_note
                ),
                2.8,
                INK,
            );
            let bw = &report.loads.building_wind;
            if !bw.directions.is_empty() {
                let wrows: Vec<_> = bw
                    .directions
                    .iter()
                    .map(|d| {
                        let axis_key = if d.direction.along_x() {
                            "Vento na edificação (±X)"
                        } else {
                            "Vento na edificação (±Y)"
                        };
                        (
                            light_of.get(axis_key).copied(),
                            vec![
                                d.direction.label().to_owned(),
                                format!(
                                    "{}×{} m",
                                    decimal(d.facade_width_m, 1),
                                    decimal(d.depth_m, 1)
                                ),
                                format!(
                                    "+{}/{}",
                                    decimal(d.coeffs.cpe_windward, 2),
                                    decimal(d.coeffs.cpe_leeward, 2)
                                ),
                                decimal(d.coeffs.ca_drag, 2),
                                decimal(d.base_shear_kn, 1),
                                decimal(d.overturning_moment_knm, 1),
                                if d.overturning_safety_ratio.is_finite() {
                                    format!(
                                        "{}×",
                                        decimal(d.overturning_safety_ratio.min(999.9), 1)
                                    )
                                } else {
                                    "∞".to_owned()
                                },
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Direção",
                        "Fach.×Prof.",
                        "Cpe barl/sot",
                        "Ca",
                        "Vw (kN)",
                        "Mw (kN·m)",
                        "ELU-EQU",
                    ],
                    &[1.5, 1.7, 1.7, 0.9, 1.3, 1.5, 1.3],
                    &wrows,
                );
            }
            if !report.loads.combinations.is_empty() {
                pages.subheading("Combinações de ações (NBR 8681:2003 / NBR 6118 §11.8)");
                let crows: Vec<_> = report
                    .loads
                    .combinations
                    .iter()
                    .map(|c| {
                        (
                            None,
                            vec![
                                c.factors.code.to_owned(),
                                c.factors.category.label().to_owned(),
                                c.factors.formula.clone(),
                                decimal(c.vertical_kn, 1),
                                decimal(c.horizontal_x_kn, 1),
                                decimal(c.horizontal_y_kn, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Comb.",
                        "Tipo",
                        "Expressão",
                        "Fz (kN)",
                        "Fx (kN)",
                        "Fy (kN)",
                    ],
                    &[1.2, 1.7, 4.1, 1.1, 1.0, 1.0],
                    &crows,
                );
            }
        }
        if !report.frames.is_empty() {
            pages.subheading("Pórticos planos 2D (vigas + pilares · envoltórias NBR 8681)");
            let rows: Vec<_> = report
                .frames
                .iter()
                .map(|f| {
                    let model = match (f.rigid_joints, f.fixed_bases) {
                        (true, true) => "Rígido / Engast.",
                        (true, false) => "Rígido / Rotul.",
                        (false, true) => "Rotul. / Engast.",
                        (false, false) => "Rotul. / Contrav.",
                    };
                    let light = light_of.get(&f.label).copied().or(Some(if f.drift_ok() {
                        Light::Green
                    } else {
                        Light::Red
                    }));
                    let (m_beam, b_code) = if f.envelope.elu_min_beam_neg_knm.abs()
                        > f.envelope.elu_max_beam_pos_knm
                    {
                        (
                            f.envelope.elu_min_beam_neg_knm,
                            f.envelope.elu_min_beam_neg_combo.as_str(),
                        )
                    } else {
                        (
                            f.envelope.elu_max_beam_pos_knm,
                            f.envelope.elu_max_beam_pos_combo.as_str(),
                        )
                    };
                    (
                        light,
                        vec![
                            f.label.clone(),
                            model.to_owned(),
                            format!("{} ({b_code})", decimal(m_beam, 1)),
                            format!(
                                "{} ({})",
                                decimal(f.envelope.elu_max_column_moment_knm, 1),
                                f.envelope.elu_max_column_moment_combo
                            ),
                            format!("{} kN", decimal(f.characteristic_wind_kn, 1)),
                            format!(
                                "{}/{} mm",
                                decimal(f.max_lateral_drift_mm, 2),
                                decimal(f.drift_limit_mm, 2)
                            ),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Pórtico",
                    "Modelo",
                    "ELU Md,viga (kN·m)",
                    "ELU Md,pilar (kN·m)",
                    "Vento Wk",
                    "ELS-F Δh / H/850",
                ],
                &[1.6, 1.7, 2.0, 2.0, 1.2, 2.0],
                &rows,
            );
        }
        if !report.frame_failures.is_empty() {
            pages.subheading("Pórticos 2D não resolvidos");
            let rows: Vec<_> = report
                .frame_failures
                .iter()
                .map(|failure| {
                    (
                        Some(Light::Red),
                        vec![failure.label.clone(), failure.reason.clone()],
                    )
                })
                .collect();
            pages.table(&["Pórtico", "Motivo da falha"], &[2.2, 8.8], &rows);
            pages.paragraph(
                "Um resultado ausente não aprova o pórtico. Revise estabilidade, vínculos, apoios e geometria antes de construir.",
                2.7,
                "#cc3f33",
            );
        }
        if !report.columns.is_empty() {
            pages.subheading("Pilares");
            let rows: Vec<_> = report
                .columns
                .iter()
                .map(|c| {
                    (
                        light_of.get(&format!("Pilar {}", c.label)).copied(),
                        vec![
                            c.label.clone(),
                            section_cm(c.width_m, c.depth_m),
                            c.position.label().to_owned(),
                            decimal(c.load_kn, 1),
                            decimal(c.upper_load_kn, 1),
                            format!("{:.0}", c.area_needed_cm2.max(360.0)),
                            format!("{:.0}", c.area_cm2),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Pilar",
                    "Seção (cm)",
                    "Posição",
                    "N (kN)",
                    "De cima (kN)",
                    "Ac nec. (cm²)",
                    "Ac (cm²)",
                ],
                &[1.2, 1.5, 1.9, 1.3, 1.6, 1.8, 1.4],
                &rows,
            );
        }
        if !report.footings.is_empty() {
            pages.subheading("Sapatas");
            let rows: Vec<_> = report
                .footings
                .iter()
                .map(|f| {
                    let name = format!("Sapata {}", f.label);
                    let out = refused_names.contains(&name.as_str());
                    (
                        light_of.get(&name).copied(),
                        if out {
                            vec![
                                f.label.clone(),
                                f.column.clone().unwrap_or_else(|| "—".into()),
                                "fora dos limites".into(),
                                String::new(),
                                String::new(),
                                String::new(),
                                String::new(),
                            ]
                        } else {
                            vec![
                                f.label.clone(),
                                f.column.clone().unwrap_or_else(|| "—".into()),
                                section_cm(f.width_m, f.depth_m),
                                decimal(f.load_kn, 1),
                                format!("{:.0}/{:.0}", f.max_pressure_kpa, f.soil_kpa),
                                decimal(f.punching_ratio, 2),
                                decimal(f.settlement_mm, 1),
                            ]
                        },
                    )
                })
                .collect();
            pages.table(
                &[
                    "Sapata",
                    "Pilar",
                    "B×L (cm)",
                    "N (kN)",
                    "σ/σadm (kPa)",
                    "Punção",
                    "Recalque (mm)",
                ],
                &[1.2, 1.1, 1.5, 1.2, 1.7, 1.1, 1.3],
                &rows,
            );
        }
        if !report.pile_caps.is_empty() {
            pages.subheading("Blocos sobre broca (estacas)");
            let rows: Vec<_> = report
                .pile_caps
                .iter()
                .map(|c| {
                    let name = format!("Bloco sobre broca {}", c.label);
                    (
                        light_of.get(&name).copied(),
                        vec![
                            c.label.clone(),
                            c.column.clone().unwrap_or_else(|| "—".into()),
                            format!(
                                "{}×{:.0}",
                                section_cm(c.width_m, c.depth_m),
                                c.height_m * 100.0
                            ),
                            format!(
                                "{}×Ø{:.0}×{}m",
                                c.pile_count,
                                c.pile_diameter_m * 100.0,
                                decimal(c.pile_depth_m, 1)
                            ),
                            decimal(c.load_kn, 1),
                            decimal(c.allowable_kn, 1),
                            format!("{} m", decimal(c.anchor_depth_m, 2)),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Bloco",
                    "Pilar",
                    "B×L×H (cm)",
                    "Brocas",
                    "N (kN)",
                    "Radm (kN)",
                    "Arranque",
                ],
                &[1.1, 1.0, 1.7, 1.8, 1.1, 1.2, 1.2],
                &rows,
            );
        }
        let foundation_study = project.foundation_study();
        if project.structure.soil_survey.is_some() || !foundation_study.options.is_empty() {
            pages.subheading("Sondagem (SPT) e estudo de fundações");
            if let Some(survey) = &project.structure.soil_survey {
                let bore_rows: Vec<_> = survey
                    .boreholes()
                    .into_iter()
                    .map(|b| {
                        let base = survey.footing_depth_m;
                        let deepest = b.deepest_m();
                        let n_bulb = b.mean_n(base, base + 2.0);
                        let kpa =
                            n_bulb.map(|n| (20.0 * n).min(crate::domain::soil::MAX_FOOTING_KPA));
                        (
                            None,
                            vec![
                                b.name,
                                decimal(deepest, 1),
                                n_bulb.map_or("—".into(), |n| decimal(n, 1)),
                                b.water_table_m.map_or("Não encontrado".into(), |w| {
                                    format!("{} m", decimal(w, 2))
                                }),
                                kpa.map_or("—".into(), |v| format!("{v:.0}")),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Furo SPT",
                        "Prof. (m)",
                        "N̄ bulbo",
                        "Lençol freático",
                        "σadm (kPa)",
                    ],
                    &[1.5, 1.3, 1.3, 2.2, 1.4],
                    &bore_rows,
                );
            }
            if !foundation_study.options.is_empty() {
                let opt_rows: Vec<_> = foundation_study
                    .options
                    .iter()
                    .map(|opt| {
                        let light =
                            if foundation_study.recommended == Some(opt.option) || opt.viable {
                                Some(Light::Green)
                            } else {
                                Some(Light::Yellow)
                            };
                        let status = if foundation_study.recommended == Some(opt.option) {
                            "Recomendada"
                        } else if opt.viable {
                            "Viável"
                        } else {
                            "Não indicada"
                        };
                        let settlement = match (opt.settlement_mm, opt.differential_settlement_mm) {
                            (Some(s), Some(d)) => {
                                format!("{} (dif. {})", decimal(s, 1), decimal(d, 1))
                            }
                            (Some(s), None) => decimal(s, 1),
                            _ => "—".to_owned(),
                        };
                        (
                            light,
                            vec![
                                opt.option.label().to_owned(),
                                status.to_owned(),
                                if opt.concrete_m3 > 0.0 {
                                    decimal(opt.concrete_m3, 2)
                                } else {
                                    "—".to_owned()
                                },
                                settlement,
                                opt.summary.clone(),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Opção",
                        "Situação",
                        "Concreto (m³)",
                        "Recalque (mm)",
                        "Resumo",
                    ],
                    &[1.9, 1.4, 1.3, 1.6, 4.0],
                    &opt_rows,
                );
            }
        }
        if !report.balance_beams.is_empty() {
            pages.subheading("Vigas alavanca (fundações de divisa — NBR 6122)");
            let rows: Vec<_> = report
                .balance_beams
                .iter()
                .map(|b| {
                    let name = format!("Viga alavanca {} ({} → {})", b.label, b.column, b.anchor);
                    let fs = if b.pair_overturning_fs.is_infinite() {
                        "∞".to_owned()
                    } else {
                        decimal(b.pair_overturning_fs, 2)
                    };
                    let sec = b.height_m.map_or("insuf.".to_owned(), |h| {
                        format!("{:.0}×{:.0}", b.width_m * 100.0, h * 100.0)
                    });
                    (
                        light_of.get(&name).copied(),
                        vec![
                            b.label.clone(),
                            format!("{}→{}", b.column, b.anchor),
                            format!("{} / {:.0}", decimal(b.span_m, 2), b.eccentricity_m * 100.0),
                            format!("{}/{}", decimal(b.reaction_kn, 1), decimal(b.relief_kn, 1)),
                            fs,
                            format!(
                                "{}/{}",
                                b.boundary_pressure_kpa
                                    .map_or("—".to_owned(), |v| format!("{v:.0}")),
                                b.anchor_pressure_kpa
                                    .map_or("—".to_owned(), |v| format!("{v:.0}"))
                            ),
                            format!("{sec} · {} · {}", b.bars, b.stirrups),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Viga",
                    "Par",
                    "l(m)/e(cm)",
                    "R1/ΔP (kN)",
                    "FS tomb.",
                    "σ1/σ2 (kPa)",
                    "Seção / Aço",
                ],
                &[0.9, 1.2, 1.4, 1.5, 1.0, 1.4, 3.0],
                &rows,
            );
            for b in &report.balance_beams {
                if let Some(alt) = &b.strip_alternative {
                    pages.paragraph(&format!("• {}: {}", b.label, alt.note), 2.7, "#8a5a00");
                }
            }
        }
        if !report.timber_posts.is_empty() {
            pages.subheading("Mourões de madeira (NBR 7190)");
            let rows: Vec<_> = report
                .timber_posts
                .iter()
                .map(|p| {
                    let name = format!("Mourão {}", p.label);
                    (
                        light_of.get(&name).copied(),
                        vec![
                            p.label.clone(),
                            format!("{} ({})", p.species.label(), p.timber_class.label()),
                            format!(
                                "{}{}",
                                if p.round { "Ø" } else { "□" },
                                decimal(p.size_m * 100.0, 0)
                            ),
                            decimal(p.height_m, 2),
                            decimal(p.load_kn, 1),
                            format!(
                                "{} kN / {} kN·m",
                                decimal(p.wind_force_kn, 1),
                                decimal(p.wind_moment_knm, 1)
                            ),
                            if p.ratio.is_finite() {
                                format!("λ {:.0} · {:.0}%", p.slenderness, p.ratio * 100.0)
                            } else {
                                format!("λ {:.0} · —", p.slenderness)
                            },
                            format!(
                                "{}×{} cm · Ch {} kN",
                                decimal(p.base_m * 100.0, 0),
                                decimal(p.base_depth_m * 100.0, 0),
                                decimal(p.anchor_kn, 1)
                            ),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Mourão",
                    "Espécie / Classe",
                    "Seção",
                    "H (m)",
                    "N (kN)",
                    "Vento",
                    "λ / Uso",
                    "Base / Chumb.",
                ],
                &[1.1, 2.5, 0.9, 0.9, 0.9, 2.0, 1.7, 2.2],
                &rows,
            );
            pages.paragraph(
                "Mourões externos exigem madeira com tratamento preservativo compatível com a classe de uso e chumbador metálico mantendo a peça afastada do solo.",
                2.7,
                MUTED,
            );
        }
        if !report.eave_beams.is_empty() {
            pages.subheading("Vigas de madeira (beiral)");
            let rows: Vec<_> = report
                .eave_beams
                .iter()
                .map(|b| {
                    let name = format!("Viga {}", b.label);
                    (
                        light_of.get(&name).copied(),
                        vec![
                            b.label.clone(),
                            format!("{} ({})", b.species.label(), b.timber_class.label()),
                            format!("{}–{}", b.from_post, b.to_post),
                            decimal(b.span_m, 2),
                            format!("{:.0}×{:.0}", b.section_cm.0, b.section_cm.1),
                            format!(
                                "{:.0}% / {:.0}% / {:.0}%",
                                b.bending_ratio * 100.0,
                                b.shear_ratio * 100.0,
                                b.deflection_ratio * 100.0
                            ),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Viga",
                    "Espécie / Classe",
                    "Apoios",
                    "Vão (m)",
                    "b×h (cm)",
                    "Flex / Cis / Flecha",
                ],
                &[1.2, 2.8, 1.4, 1.1, 1.3, 2.4],
                &rows,
            );
        }
        if !report.misaligned_columns.is_empty() {
            pages.subheading("Pilares de transição");
            for m in &report.misaligned_columns {
                pages.paragraph(
                    &format!(
                        "• {} ({}): {:.1} kN {}",
                        m.column_label,
                        m.floor_name,
                        m.load_kn,
                        m.transfer_beam
                            .as_ref()
                            .map_or("sem apoio no pavimento inferior".to_owned(), |b| format!(
                                "sobre a viga {b}"
                            ))
                    ),
                    3.0,
                    INK,
                );
            }
        }

        if !reinforcement.is_empty() {
            pages.subheading("Armaduras preliminares");
            pages.paragraph(
                "Estimativa pelo método documentado para a NBR 6118:2023; a edição 2026 ainda exige auditoria. Não é detalhamento executivo: ancoragens, emendas, aberturas e juntas devem ser verificadas por engenheiro estrutural.",
                2.7,
                "#8a5a00",
            );

            if !reinforcement.beams.is_empty() {
                pages.paragraph(
                    "Vigas em pórticos: flexão e cortante pela envoltória ELU de cada viga, sem nova majoração. Superior contínua no vão inteiro; as colunas Sup. E/D indicam a mesma armadura. As combinações governantes constam nas verificações e na prancha de armaduras.",
                    2.7, INK,
                );
                let rows: Vec<_> = reinforcement
                    .beams
                    .iter()
                    .map(|beam| {
                        (
                            None,
                            vec![
                                beam.label.clone(),
                                beam.bottom.notation(),
                                beam.top.left().notation(),
                                if beam.frame_envelope.is_some() {
                                    "contínua".to_owned()
                                } else {
                                    beam.top.right().notation()
                                },
                                beam.stirrups.notation(),
                                format!(
                                    "{:.0}/{:.0}",
                                    beam.bottom_anchorage_m * 100.0,
                                    beam.top_anchorage_m * 100.0
                                ),
                                decimal(beam.mass_kg, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Viga",
                        "Inferior",
                        "Sup. E",
                        "Sup. D",
                        "Estribos",
                        "lb,nec (cm)",
                        "kg",
                    ],
                    &[1.3, 1.5, 1.4, 1.4, 1.9, 1.2, 0.9],
                    &rows,
                );
            }

            if !reinforcement.columns.is_empty() {
                let rows: Vec<_> = reinforcement
                    .columns
                    .iter()
                    .map(|column| {
                        (
                            None,
                            vec![
                                column.label.clone(),
                                column.longitudinal.notation(),
                                column.ties.notation(),
                                format!("{:.0} cm", column.lap_length_m * 100.0),
                                format!("{}%", decimal(column.steel_ratio * 100.0, 2)),
                                decimal(column.mass_kg, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Pilar",
                        "Longitudinal",
                        "Estribos",
                        "Traspasse l0",
                        "ρ",
                        "kg",
                    ],
                    &[1.2, 1.9, 2.1, 1.4, 1.1, 0.9],
                    &rows,
                );
            }

            if !reinforcement.slabs.is_empty() {
                let rows: Vec<_> = reinforcement
                    .slabs
                    .iter()
                    .map(|slab| {
                        (
                            None,
                            vec![
                                format!("{} · {}", slab.label, slab.room),
                                slab.kind.label().to_owned(),
                                slab.main.notation(),
                                slab.distribution.notation(),
                                decimal(slab.mass_kg, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &["Laje", "Tipo", "Principal", "Distribuição", "kg"],
                    &[2.3, 1.6, 2.0, 2.0, 1.0],
                    &rows,
                );
            }

            if !reinforcement.footings.is_empty() {
                let rows: Vec<_> = reinforcement
                    .footings
                    .iter()
                    .map(|footing| {
                        (
                            None,
                            vec![
                                footing.label.clone(),
                                footing.bars_x.notation(),
                                footing.bars_y.notation(),
                                decimal(footing.mass_kg, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &["Sapata", "Direção X", "Direção Y", "kg"],
                    &[1.5, 2.5, 2.5, 1.0],
                    &rows,
                );
            }

            pages.subheading("Tabela preliminar de aço");
            let rows: Vec<_> = reinforcement
                .schedule
                .iter()
                .map(|row| {
                    (
                        None,
                        vec![
                            row.mark.clone(),
                            format!("{} · {} ({})", row.element, row.position, row.shape),
                            decimal(row.diameter_mm, 1),
                            row.quantity.to_string(),
                            decimal(row.unit_length_m, 2),
                            decimal(row.total_length_m, 2),
                            decimal(row.mass_kg, 1),
                        ],
                    )
                })
                .collect();
            pages.table(
                &[
                    "Marca",
                    "Elemento · posição (forma)",
                    "Ø mm",
                    "Qtd.",
                    "Unit. m",
                    "Total m",
                    "kg",
                ],
                &[1.1, 3.5, 0.9, 0.8, 1.1, 1.1, 0.9],
                &rows,
            );
            if !reinforcement.by_diameter.is_empty() {
                pages.subheading("Resumo de aço por bitola (CA-50)");
                let summary_rows: Vec<_> = reinforcement
                    .by_diameter
                    .iter()
                    .map(|row| {
                        (
                            None,
                            vec![
                                format!("Ø{} mm", decimal(row.diameter_mm, 1)),
                                decimal(row.total_length_m, 1),
                                row.bars_12m.to_string(),
                                decimal(row.mass_kg, 1),
                                decimal(row.mass_with_waste_kg, 1),
                            ],
                        )
                    })
                    .collect();
                pages.table(
                    &[
                        "Bitola",
                        "Comprimento (m)",
                        "Barras 12 m (+10%)",
                        "Teórica (kg)",
                        "+10% perda (kg)",
                    ],
                    &[1.5, 2.0, 2.0, 1.8, 1.9],
                    &summary_rows,
                );
            }
            pages.paragraph(
                &format!(
                    "Massa teórica total: {} kg · Com +10% de corte e dobra: {} kg ({} barras comerciais de 12 m).",
                    decimal(reinforcement.total_mass_kg(), 1),
                    decimal(reinforcement.total_mass_with_waste_kg(), 1),
                    reinforcement.total_bars_12m()
                ),
                3.0,
                INK,
            );
            for warning in &reinforcement.warnings {
                pages.paragraph(&format!("• {warning}"), 2.7, "#8a5a00");
            }
        }
    }

    // 4. Electrical.
    pages.heading("4. Resultados · instalação elétrica");
    if schedule.circuits.is_empty() {
        pages.paragraph("Nenhum circuito no projeto.", 3.0, MUTED);
    } else {
        let rows: Vec<_> = schedule
            .circuits
            .iter()
            .map(|row| {
                (
                    light_of
                        .get(&format!("Circuito C{} ({})", row.number, row.kind.short()))
                        .copied(),
                    vec![
                        format!("C{}", row.number),
                        row.kind.short().to_owned(),
                        format!("{:.0}", row.power_va),
                        decimal(row.current_a, 1),
                        phase_label(&row.phases),
                        row.wire_mm2.map_or("—".into(), section_label),
                        row.breaker_a.map_or("—".into(), |b| format!("{b:.0}")),
                        if row.needs_rcd {
                            "Sim".into()
                        } else {
                            "—".into()
                        },
                        row.drop_percent.map_or("—".into(), |d| decimal(d, 2)),
                    ],
                )
            })
            .collect();
        pages.table(
            &[
                "Circ.",
                "Tipo",
                "VA",
                "I (A)",
                "Fase",
                "Fio (mm²)",
                "Disj. (A)",
                "DR",
                "Queda (%)",
            ],
            &[1.0, 1.0, 1.1, 1.0, 1.0, 1.3, 1.3, 0.8, 1.3],
            &rows,
        );
        let d = schedule.demand;
        pages.paragraph(
            &format!(
                "Carga instalada {:.0} VA; demanda provável {} kVA ({:.0} VA × {} + {:.0} VA × {}); ligação {}.",
                schedule.total_va,
                decimal(d.demand_va / 1000.0, 2),
                d.general_va,
                decimal(d.general_factor, 2),
                d.specific_va,
                decimal(d.specific_factor, 2),
                schedule.supply.label().to_lowercase()
            ),
            3.0,
            INK,
        );
        if let Some(e) = schedule.entrance {
            pages.paragraph(
                &format!(
                    "Entrada: disjuntor geral {}, ramal {} mm², eletroduto Ø{} mm, {} A na fase mais carregada.",
                    e.breaker_a.map_or("—".into(), |b| format!("{b:.0} A")),
                    e.wire_mm2.map_or("—".into(), section_label),
                    e.conduit_mm.map_or("—".into(), |c| c.to_string()),
                    decimal(e.current_a, 1)
                ),
                3.0,
                INK,
            );
        }
    }

    // 5. Every check.
    pages.heading("5. Verificações (semáforo)");
    pages.paragraph(
        "Verde: usa até 85 % do limite. Amarelo: entre 85 % e 100 %. Vermelho: não atende ou está fora dos limites de aplicação.",
        3.0,
        MUTED,
    );
    for verdict in &summary.verdicts {
        pages.room(12.0);
        let y = pages.y;
        pages.dot(LEFT + 1.5, y - 1.0, verdict.light());
        pages.text(
            LEFT + 4.5,
            y,
            3.0,
            true,
            INK,
            &format!("{} · {}", verdict.discipline.label(), verdict.element),
        );
        pages.y += 4.6;
        for check in &verdict.checks {
            let usage = if check.utilisation.is_finite() {
                format!(
                    " (uso {} %, folga {} %)",
                    decimal(check.utilisation * 100.0, 0),
                    decimal(check.margin_percent(), 0)
                )
            } else {
                String::new()
            };
            pages.room(5.0);
            let y = pages.y;
            pages.dot(LEFT + 6.0, y - 0.9, check.light);
            pages.text(
                LEFT + 9.0,
                y,
                2.7,
                true,
                INK,
                &format!("{}{usage}", check.name),
            );
            pages.y += 3.8;
            for line in wrap(&check.simple, 105) {
                pages.room(4.0);
                let y = pages.y;
                pages.text(LEFT + 9.0, y, 2.6, false, INK, &line);
                pages.y += 3.6;
            }
            for line in wrap(&check.detail, 118) {
                pages.room(4.0);
                let y = pages.y;
                pages.text(LEFT + 9.0, y, 2.3, false, MUTED, &line);
                pages.y += 3.3;
            }
            pages.y += 1.0;
        }
        pages.y += 1.5;
    }

    // 6. Warnings.
    pages.heading("6. Alertas");
    let warnings: Vec<&String> = report.warnings.iter().chain(&schedule.warnings).collect();
    if warnings.is_empty() {
        pages.paragraph("Nenhum alerta.", 3.0, MUTED);
    }
    for warning in warnings {
        pages.paragraph(&format!("• {warning}"), 3.0, INK);
    }

    // 7. Before each pour (#162).
    let checklists = crate::domain::protection::checklists::pour_checklists(&reinforcement);
    if !checklists.is_empty() {
        pages.heading("7. Antes de concretar");
        for list in &checklists {
            pages.subheading(&list.title);
            for element in &list.elements {
                pages.paragraph(&format!("• {element}"), 3.0, INK);
            }
            for item in &list.items {
                pages.paragraph(&format!("[  ] {item}"), 3.0, INK);
            }
        }
    }

    // Where to find help (#159).
    pages.heading(if checklists.is_empty() {
        "7. Onde buscar ajuda"
    } else {
        "8. Onde buscar ajuda"
    });
    pages.paragraph(
        "Resultado vermelho ou recusado não deve ser construído assim. Se a família não puder pagar um profissional:",
        3.0,
        MUTED,
    );
    for line in crate::domain::protection::help::help_lines(&project.protection.help_contacts) {
        pages.paragraph(&format!("• {line}"), 3.0, INK);
    }
    pages.finish()
}

/// The drawings appended to the report: formwork plan, electrical plan,
/// beam forces and single-line diagram, when the project has them.
pub fn drawing_pages(project: &Project) -> Vec<String> {
    let mut out = Vec::new();
    let has_structure = project.structural_columns().next().is_some();
    if has_structure && limits::structural_refusal(project).is_none() {
        out.push(super::export::sheet::scaled_sheet_svg_for_view(
            project,
            &super::export::ExportView::for_discipline(project, Discipline::Structure),
            None,
        ));
        if !project.structure.beams.is_empty() {
            out.push(beam_diagrams_svg(project));
        }
    }
    if !project.load_schedule().circuits.is_empty() {
        out.push(super::export::sheet::scaled_sheet_svg_for_view(
            project,
            &super::export::ExportView::for_discipline(project, Discipline::Electrical),
            None,
        ));
        out.push(single_line_svg(project));
    }
    out
}

/// Joins SVG pages (each with its own size) into one PDF.
pub fn pages_to_pdf(pages: &[String]) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
    let options = super::export::svg2pdf_options();
    let mut alloc = Ref::new(1);
    let catalog_id = alloc.bump();
    let tree_id = alloc.bump();
    let mut pdf = Pdf::new();
    let mut kids = Vec::new();
    let mut chunks = Vec::new();
    for markup in pages {
        let tree = svg2pdf::usvg::Tree::from_str(markup, &options).map_err(|e| e.to_string())?;
        let (chunk, svg_id) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let svg_id = map[&svg_id];
        // usvg sizes are CSS pixels (96 per inch); PDF uses points (72).
        let (w, h) = (
            tree.size().width() * 72.0 / 96.0,
            tree.size().height() * 72.0 / 96.0,
        );
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        kids.push(page_id);
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, w, h));
        page.parent(tree_id);
        page.contents(content_id);
        page.resources().x_objects().pair(Name(b"S"), svg_id);
        page.finish();
        let mut content = Content::new();
        content
            .transform([w, 0.0, 0.0, h, 0.0, 0.0])
            .x_object(Name(b"S"));
        pdf.stream(content_id, &content.finish());
        chunks.push(chunk);
    }
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id)
        .kids(kids.iter().copied())
        .count(kids.len() as i32);
    for chunk in &chunks {
        pdf.extend(chunk);
    }
    Ok(pdf.finish())
}

/// Today's date as dd/mm/aaaa (UTC), for the cover.
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02}/{month:02}/{year}")
}

/// Writes the pre-sizing report with its drawings to `path`.
pub fn export_presizing_report(
    project: &Project,
    date: &str,
    path: &Path,
) -> Result<usize, String> {
    let mut pages = report_pages(project, date);
    pages.extend(drawing_pages(project));
    let pdf = pages_to_pdf(&pages)?;
    fs::write(path, pdf).map_err(|e| e.to_string())?;
    Ok(pages.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    fn house() -> Project {
        let mut project = Project::default();
        project.name = "Casa Teste".to_owned();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
            .unwrap();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();
        project
    }

    #[test]
    fn the_report_holds_every_section_on_valid_numbered_pages() {
        let project = house();
        let pages = report_pages(&project, "21/11/2026");
        assert!(pages.len() >= 3);
        let all = pages.join("\n");
        for needed in [
            "MEMORIAL DE PRÉ-DIMENSIONAMENTO",
            "Revisão do profissional habilitado",
            "ART/RRT nº",
            "1. Dados de entrada",
            "2. Premissas, métodos e limites",
            "3. Resultados · estrutura",
            "Armaduras preliminares",
            "Tabela preliminar de aço",
            "Resumo de aço por bitola (CA-50)",
            "Traspasse l0",
            "4. Resultados · instalação elétrica",
            "5. Verificações (semáforo)",
            "6. Alertas",
            "Não é projeto executivo",
        ] {
            assert!(all.contains(needed), "{needed}");
        }
        let total = pages.len();
        for (i, page) in pages.iter().enumerate() {
            assert!(
                page.contains(&format!("{}/{total}", i + 1)),
                "page {}",
                i + 1
            );
            assert!(!page.contains(TOTAL));
            assert!(
                svg2pdf::usvg::Tree::from_str(page, &svg2pdf::usvg::Options::default()).is_ok()
            );
        }
    }

    #[test]
    fn concentrated_loads_are_printed_in_the_review_report() {
        let mut project = house();
        project
            .add_fixture(
                crate::domain::fixtures::FixtureKind::WaterTank,
                Point::new(2.0, 2.0),
                1.20,
                1.20,
                0,
            )
            .unwrap();
        let all = report_pages(&project, "21/11/2026").join("\n");
        assert!(all.contains("Cargas concentradas"));
        assert!(all.contains("CC1"));
        assert!(all.contains("Caixa-d'água"));
    }

    #[test]
    fn the_pdf_has_one_page_per_svg_page_and_the_drawings() {
        let project = house();
        let mut pages = report_pages(&project, "21/11/2026");
        let report_only = pages.len();
        let drawings = drawing_pages(&project);
        assert_eq!(
            drawings.len(),
            4,
            "formwork, beam forces, electrical plan, single-line"
        );
        pages.extend(drawings);
        let pdf = pages_to_pdf(&pages).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.contains(&format!("/Count {}", report_only + 4)),
            "page count"
        );
    }

    #[test]
    fn today_is_a_plausible_date() {
        let date = today();
        let parts: Vec<u32> = date.split('/').map(|p| p.parse().unwrap()).collect();
        assert!((1..=31).contains(&parts[0]) && (1..=12).contains(&parts[1]) && parts[2] >= 2024);
    }

    #[test]
    fn a_refused_project_says_so_and_shows_no_structural_numbers() {
        let mut project = house();
        project.structure.settings.floors = 5;
        let pages = report_pages(&project, "21/11/2026").join("\n");
        assert!(pages.contains("Dimensionamento estrutural recusado"));
        assert!(pages.contains("nenhum elemento estrutural foi dimensionado"));
        assert!(!pages.contains(">Pilares<"));
        assert!(
            drawing_pages(&project).len() == 2,
            "no formwork plan or beam forces"
        );
    }

    #[test]
    fn timber_posts_and_eave_beams_are_printed_in_the_review_report() {
        use crate::domain::fixtures::FixtureKind;
        use crate::domain::roof::{Roof, RoofSide, TimberSpecies};

        let mut project = Project::default();
        project.name = "Área Gourmet".to_owned();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project.structure.post_species = TimberSpecies::Itauba;
        project.structure.post_treated = true;
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(0.0, 6.0))
            .unwrap();
        for y in [0.0, 3.0, 6.0] {
            project
                .add_fixture(FixtureKind::Column, Point::new(0.0, y), 0.20, 0.30, 0)
                .unwrap();
            project
                .add_fixture(FixtureKind::TimberPost, Point::new(4.0, y), 0.16, 0.16, 0)
                .unwrap();
        }
        let roof = Roof::new(
            Point::new(0.0, 0.0),
            Point::new(4.0, 6.0),
            RoofSide::West,
            3.8,
            2.6,
        );
        project.add_roof(roof).unwrap();

        let pages = report_pages(&project, "22/11/2026");
        let all = pages.join("\n");
        assert!(all.contains("Madeira (mourões/vigas)"));
        assert!(all.contains("Itaúba"));
        assert!(all.contains("Mourões de madeira (NBR 7190)"));
        assert!(all.contains("Vigas de madeira (beiral)"));
        assert!(all.contains("M1"));
        assert!(all.contains("VM1"));
        assert!(all.contains("Ch "));
        for page in &pages {
            assert!(
                svg2pdf::usvg::Tree::from_str(page, &svg2pdf::usvg::Options::default()).is_ok()
            );
        }
    }

    #[test]
    fn multi_borehole_spt_and_foundation_study_appear_in_the_review_report() {
        use crate::domain::soil::{Borehole, SoilKind, SoilSurvey, SptReading};

        let mut project = house();
        let sp1 = Borehole::new(
            "SP-01".to_owned(),
            (1..=10)
                .map(|d| SptReading {
                    depth_m: f64::from(d),
                    n: 10,
                    soil: SoilKind::Sand,
                })
                .collect(),
            Some(5.0),
        )
        .unwrap();
        let sp2 = Borehole::new(
            "SP-02".to_owned(),
            (1..=10)
                .map(|d| SptReading {
                    depth_m: f64::from(d),
                    n: 7,
                    soil: SoilKind::Clay,
                })
                .collect(),
            Some(4.0),
        )
        .unwrap();
        project.set_soil_survey(Some(
            SoilSurvey::from_boreholes(vec![sp1, sp2], 1.0).unwrap(),
        ));

        let pages = report_pages(&project, "22/11/2026");
        let all = pages.join("\n");
        assert!(all.contains("Sondagem (SPT) e estudo de fundações"));
        assert!(all.contains("SP-01"));
        assert!(all.contains("SP-02"));
        assert!(all.contains("Estacas profundas (hélice contínua)"));
        assert!(all.contains("Recalque (mm)"));
        for page in &pages {
            assert!(
                svg2pdf::usvg::Tree::from_str(page, &svg2pdf::usvg::Options::default()).is_ok()
            );
        }
    }

    #[test]
    fn boundary_balance_beams_appear_in_the_review_report() {
        let mut project = Project::default();
        project.name = "Divisa Teste".to_owned();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 15.0));
        project
            .add_room(Point::new(0.0, 2.0), Point::new(4.0, 6.0), "Gourmet")
            .unwrap();
        project.generate_structure();
        let (added, _) = project.generate_balance_beams();
        assert_eq!(added, 2);

        let pages = report_pages(&project, "22/11/2026");
        let all = pages.join("\n");
        assert!(all.contains("Vigas alavanca (fundações de divisa — NBR 6122)"));
        assert!(all.contains("VA1"));
        assert!(all.contains("FS tomb."));
        assert!(all.contains("σ1/σ2 (kPa)"));
        for page in &pages {
            assert!(
                svg2pdf::usvg::Tree::from_str(page, &svg2pdf::usvg::Options::default()).is_ok()
            );
        }
    }
}
