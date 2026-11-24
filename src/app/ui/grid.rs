//! Coupled slab + beam grillage analysis UI tab (#109).

use eframe::egui::{self, RichText, TextureHandle, Ui, Vec2};

use crate::app::theme::*;
use crate::domain::{grid_analysis::GridAnalysisReport, verdict::Light};

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

pub(super) fn grid_tab(
    ui: &mut Ui,
    report: &GridAnalysisReport,
    preview: Option<&(TextureHandle, Vec2)>,
) {
    if let Some(err) = &report.error {
        ui.label(RichText::new(format!("Análise de grelha: {err}")).color(DANGER));
        return;
    }
    if report.slabs.is_empty() && report.beams.is_empty() {
        ui.label(
            RichText::new("Gere ou lance lajes e vigas para executar a analogia de grelha.")
                .color(MUTED),
        );
        return;
    }

    ui.label(
        RichText::new(format!(
            "Malha acoplada lajes + vigas (3 GDL/nó: w, θx, θy · NBR 6118 §14.7.5): {} nós · {} barras · ΣF={} kN · ΣR={} kN (equilíbrio {} kN).",
            report.node_count,
            report.member_count,
            decimal(report.total_applied_kn, 1),
            decimal(report.total_reaction_kn, 1),
            decimal(report.equilibrium_error_kn, 3)
        ))
        .size(12.0)
        .color(TEXT_2),
    );
    ui.add_space(6.0);

    if let Some((texture, size)) = preview {
        let width = ui.available_width().max(240.0);
        let height = (width * size.y / size.x.max(1.0)).min(420.0);
        ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(width, height)));
        ui.add_space(8.0);
    }

    if !report.slabs.is_empty() {
        ui.label(RichText::new("Lajes · momentos por metro e flecha").font(strong(13.0)));
        egui::Grid::new("grid_slabs_table")
            .striped(true)
            .spacing([12.0, 5.0])
            .show(ui, |ui| {
                for title in [
                    "Laje",
                    "Cômodo",
                    "Geometria",
                    "mx+ (kN·m/m)",
                    "my+ (kN·m/m)",
                    "mxy (kN·m/m)",
                    "w elást. (mm)",
                    "w total / L/250",
                    "Situação",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for s in &report.slabs {
                    ui.label(RichText::new(&s.label).font(strong(12.0)));
                    ui.label(&s.room);
                    ui.label(if s.is_irregular {
                        "Irregular / L"
                    } else {
                        "Retangular"
                    });
                    ui.label(decimal(s.mx_pos_knm_m, 2));
                    ui.label(decimal(s.my_pos_knm_m, 2));
                    ui.label(decimal(s.mxy_max_knm_m, 2));
                    ui.label(decimal(s.w_elastic_mm, 2));
                    ui.label(format!(
                        "{} / {} mm",
                        decimal(s.w_long_term_mm, 1),
                        decimal(s.limit_mm, 1)
                    ));
                    let color = match s.light {
                        Light::Green => SUCCESS,
                        Light::Yellow => WARNING,
                        Light::Red => DANGER,
                    };
                    ui.label(RichText::new(s.light.label()).color(color));
                    ui.end_row();
                }
            });
    }

    if !report.beams.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new("Vigas acopladas · flexão, cortante e torção").font(strong(13.0)));
        egui::Grid::new("grid_beams_table")
            .striped(true)
            .spacing([12.0, 5.0])
            .show(ui, |ui| {
                for title in [
                    "Viga",
                    "M+ (kN·m)",
                    "M− (kN·m)",
                    "V máx (kN)",
                    "Torção T (kN·m)",
                    "w elást. (mm)",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for b in &report.beams {
                    ui.label(RichText::new(&b.label).font(strong(12.0)));
                    ui.label(decimal(b.m_pos_knm, 1));
                    ui.label(decimal(b.m_neg_knm, 1));
                    ui.label(decimal(b.v_max_kn, 1));
                    ui.label(decimal(b.t_max_knm, 2));
                    ui.label(decimal(b.w_elastic_mm, 2));
                    ui.end_row();
                }
            });
    }
}
