//! Interactive "E se? (What-if)" simulator UI (#107): live sliders for span,
//! section, load and material with highlighted green/yellow/red safe-range
//! bars, side-by-side baseline vs. simulated metrics and one-click Apply.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

use crate::app::theme::*;
use crate::domain::{
    model::Project,
    structure::OccupancyClass,
    whatif::{SafeRangeBand, WhatIfParams, evaluate_whatif},
};

use super::widgets::{primary_button, secondary_button};

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn light_color(key: &str) -> Color32 {
    match key {
        "verde" => SUCCESS,
        "amarelo" => WARNING,
        _ => DANGER,
    }
}

/// Paints a compact horizontal traffic-light safe-range bar (`min..=max`)
/// highlighting red, yellow and green sub-intervals plus the current tick mark.
fn paint_safe_range_bar(ui: &mut Ui, band: &SafeRangeBand) {
    let width = ui.available_width().clamp(160.0, 360.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 10.0), Sense::hover());
    let painter = ui.painter();
    let span = (band.max - band.min).max(1e-6);
    let to_x = |v: f64| -> f32 {
        let t = ((v - band.min) / span).clamp(0.0, 1.0) as f32;
        rect.left() + t * rect.width()
    };

    // Base track = red (unsafe / over limit).
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(198, 40, 40, 95));

    // Passing band (≤ 100%) = yellow.
    if let Some((lo, hi)) = band.passing {
        let r = Rect::from_min_max(
            Pos2::new(to_x(lo), rect.top()),
            Pos2::new(to_x(hi), rect.bottom()),
        );
        painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(239, 108, 0, 145));
    }

    // Comfortable band (≤ 85%) = green.
    if let Some((lo, hi)) = band.green {
        let r = Rect::from_min_max(
            Pos2::new(to_x(lo), rect.top()),
            Pos2::new(to_x(hi), rect.bottom()),
        );
        painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(46, 125, 50, 195));
    }

    // Current slider value marker.
    let cx = to_x(band.current);
    painter.line_segment(
        [
            Pos2::new(cx, rect.top() - 2.0),
            Pos2::new(cx, rect.bottom() + 2.0),
        ],
        Stroke::new(2.4, TEXT),
    );
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(0.8, BORDER_STRONG),
        egui::StrokeKind::Outside,
    );

    let green_desc = match band.green {
        Some((lo, hi)) => format!(
            "Faixa verde: {}–{} {}",
            decimal(lo, 1),
            decimal(hi, 1),
            band.unit
        ),
        None => "Sem faixa verde no intervalo atual".to_owned(),
    };
    ui.label(RichText::new(green_desc).size(10.5).color(MUTED));
}

/// Renders the What-if simulator tab inside the Structure window.
/// Returns `Some(params)` when the user clicks **Aplicar ao projeto**.
pub(super) fn whatif_tab(ui: &mut Ui, project: &Project) -> Option<WhatIfParams> {
    if project.structure.beams.is_empty() && project.structure.slabs.is_empty() {
        ui.label(
            RichText::new("Gere ou lance a estrutura primeiro para simular cenários E se?.")
                .color(MUTED),
        );
        return None;
    }

    let state_id = egui::Id::unique("trazo_whatif_params");
    let mut params: WhatIfParams = ui
        .ctx()
        .data(|d| d.get_temp::<WhatIfParams>(state_id))
        .unwrap_or_else(|| WhatIfParams::from_project(project));

    let mut apply_clicked = false;
    let mut reset_clicked = false;

    let result = evaluate_whatif(project, &params);
    let band_for = |name: &str| result.safe_ranges.iter().find(|b| b.parameter == name);

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(
                "Arraste os controles para testar vão, seção, carga e material sem alterar o projeto até clicar em Aplicar.",
            )
            .size(12.0)
            .color(TEXT_2),
        );
        ui.label(
            RichText::new(format!("({} ms)", decimal(result.elapsed_ms, 1)))
                .size(11.0)
                .color(MUTED),
        );
    });
    ui.add_space(6.0);

    // Top summary banner: Baseline vs. Simulated.
    egui::Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.columns(2, |cols| {
                cols[0].label(
                    RichText::new("Projeto atual")
                        .font(strong(12.5))
                        .color(TEXT_2),
                );
                cols[0].horizontal(|ui| {
                    ui.label(
                        RichText::new(result.baseline.overall_light.to_uppercase())
                            .font(strong(13.0))
                            .color(light_color(&result.baseline.overall_light)),
                    );
                    ui.label(format!(
                        "· máx. {} % ({})",
                        decimal(result.baseline.max_utilisation_pct, 0),
                        result.baseline.governing_element
                    ));
                });
                cols[0].label(
                    RichText::new(format!(
                        "Vão máx. {} m · M={} kN·m · δ={} mm · Aço {} kg",
                        decimal(result.baseline.max_span_m, 2),
                        decimal(result.baseline.max_beam_moment_knm, 1),
                        decimal(result.baseline.max_beam_deflection_mm, 1),
                        decimal(result.baseline.steel_mass_kg, 0),
                    ))
                    .size(11.5)
                    .color(TEXT_2),
                );

                cols[1].label(
                    RichText::new("Cenário simulado")
                        .font(strong(12.5))
                        .color(ACCENT_TEXT),
                );
                cols[1].horizontal(|ui| {
                    ui.label(
                        RichText::new(result.simulated.overall_light.to_uppercase())
                            .font(strong(13.0))
                            .color(light_color(&result.simulated.overall_light)),
                    );
                    ui.label(format!(
                        "· máx. {} % ({:+.0} pp)",
                        decimal(result.simulated.max_utilisation_pct, 0),
                        result.delta_utilisation_pct
                    ));
                });
                cols[1].label(
                    RichText::new(format!(
                        "Vão máx. {} m · M={} kN·m · δ={} mm · Δ aço {:+.1} kg · Δ conc. {:+.2} m³",
                        decimal(result.simulated.max_span_m, 2),
                        decimal(result.simulated.max_beam_moment_knm, 1),
                        decimal(result.simulated.max_beam_deflection_mm, 1),
                        result.delta_steel_kg,
                        result.delta_concrete_m3,
                    ))
                    .size(11.5)
                    .color(TEXT_2),
                );
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(&result.advice)
                    .font(strong(12.0))
                    .color(light_color(&result.simulated.overall_light)),
            );
        });

    ui.add_space(8.0);

    // Two-column slider layout with safe-range bars.
    ui.columns(2, |cols| {
        // Column 0: Span & Section.
        cols[0].label(RichText::new("Geometria e seção").font(strong(12.5)));
        cols[0].add_space(4.0);
        cols[0].add(
            egui::Slider::new(&mut params.span_scale, 0.60..=1.60)
                .text(format!(
                    "Escala do vão ({} m)",
                    decimal(result.simulated.max_span_m, 2)
                ))
                .step_by(0.02),
        );
        if let Some(band) = band_for("Vão governante") {
            paint_safe_range_bar(&mut cols[0], band);
        }
        cols[0].add_space(6.0);

        cols[0].add(
            egui::Slider::new(&mut params.beam_height_cm, 20.0..=80.0)
                .text("Altura da viga")
                .suffix(" cm")
                .step_by(1.0),
        );
        if let Some(band) = band_for("Altura da viga") {
            paint_safe_range_bar(&mut cols[0], band);
        }
        cols[0].add_space(6.0);

        cols[0].add(
            egui::Slider::new(&mut params.beam_width_cm, 12.0..=40.0)
                .text("Largura da viga")
                .suffix(" cm")
                .step_by(1.0),
        );
        cols[0].add(
            egui::Slider::new(&mut params.column_side_cm, 14.0..=50.0)
                .text("Lado mín. pilares")
                .suffix(" cm")
                .step_by(1.0),
        );
        cols[0].add(
            egui::Slider::new(&mut params.slab_thickness_cm, 8.0..=25.0)
                .text("Espessura lajes")
                .suffix(" cm")
                .step_by(1.0),
        );

        // Column 1: Loads & Materials.
        cols[1].label(RichText::new("Cargas e materiais").font(strong(12.5)));
        cols[1].add_space(4.0);
        cols[1].add(
            egui::Slider::new(&mut params.live_load_kn_m2, 0.5..=6.0)
                .text("Sobrecarga q")
                .suffix(" kN/m²")
                .step_by(0.1),
        );
        if let Some(band) = band_for("Sobrecarga") {
            paint_safe_range_bar(&mut cols[1], band);
        }
        cols[1].add_space(6.0);

        cols[1].add(
            egui::Slider::new(&mut params.fck_mpa, 20.0..=50.0)
                .text("Concreto fck")
                .suffix(" MPa")
                .step_by(5.0),
        );
        if let Some(band) = band_for("Concreto fck") {
            paint_safe_range_bar(&mut cols[1], band);
        }
        cols[1].add_space(6.0);

        cols[1].add(
            egui::Slider::new(&mut params.soil_kpa, 60.0..=500.0)
                .text("Solo admissível")
                .suffix(" kPa")
                .step_by(10.0),
        );
        if let Some(band) = band_for("Solo admissível") {
            paint_safe_range_bar(&mut cols[1], band);
        }
        cols[1].add_space(6.0);

        cols[1].add(
            egui::Slider::new(&mut params.wind_v0_ms, 30.0..=50.0)
                .text("Vento V0 (NBR 6123)")
                .suffix(" m/s")
                .step_by(1.0),
        );
        cols[1].horizontal(|ui| {
            ui.label("Uso (NBR 6120):");
            egui::ComboBox::from_id_salt("whatif_occupancy")
                .selected_text(params.occupancy.label())
                .show_ui(ui, |ui| {
                    for occ in OccupancyClass::ALL {
                        ui.selectable_value(&mut params.occupancy, occ, occ.label());
                    }
                });
        });
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if primary_button(ui, "Aplicar seções, cargas e materiais ao projeto").clicked() {
            apply_clicked = true;
        }
        if secondary_button(ui, "Resetar para o projeto atual").clicked() {
            reset_clicked = true;
        }
        if (params.span_scale - 1.0).abs() > 1e-3 {
            ui.label(
                RichText::new(
                    "Nota: a escala do vão é exploratória e não desloca paredes ao aplicar.",
                )
                .size(11.0)
                .color(MUTED),
            );
        }
    });

    if reset_clicked {
        params = WhatIfParams::from_project(project);
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(state_id, params.clone()));

    apply_clicked.then_some(params)
}
