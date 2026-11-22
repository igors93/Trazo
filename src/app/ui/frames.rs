//! UI tab for 2D plane-frame analysis (#94): joint/base fixity toggles,
//! vector diagram preview (deformed shape + beam & column bending moments),
//! and per-frame numerical summary tables.

use eframe::egui::{self, RichText, Ui};

use super::super::theme::*;
use crate::domain::{frame2d::FrameLineResult, structure::StructureSettings};

fn dec(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// Renders the "Pórticos 2D" tab inside the Structure window.
/// Mutates `settings` in-place when the user toggles joint/base fixity.
pub(super) fn frames_tab(
    ui: &mut Ui,
    frames: &[FrameLineResult],
    settings: &mut StructureSettings,
    preview: Option<&(egui::TextureHandle, egui::Vec2)>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut settings.frame_rigid_joints, "Nós rígidos (viga–pilar)")
            .on_hover_text(
                "Marcado: ligação monolítica viga–pilar transfere momentos aos pilares. Desmarcado: pilares articulados no topo (M_topo = 0).",
            );
        ui.checkbox(&mut settings.frame_fixed_bases, "Bases engastadas")
            .on_hover_text(
                "Marcado: bases dos pilares engastadas nas fundações (Rx, Ry, Mz). Desmarcado: bases rotuladas (Rx, Ry).",
            );
    });
    ui.add_space(6.0);

    if frames.is_empty() {
        ui.label(
            RichText::new(
                "Nenhum pórtico 2D disponível. Gere a estrutura (pilares e vigas) para analisar vigas e pilares acoplados.",
            )
            .color(MUTED),
        );
        return;
    }

    if let Some((texture, size)) = preview {
        let width = ui.available_width().max(200.0);
        let height = width * size.y / size.x.max(1.0);
        ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(width, height)));
        ui.add_space(10.0);
    }

    ui.label(RichText::new("Resumo dos pórticos planos 2D (rigidez direta)").font(strong(13.0)));
    ui.add_space(4.0);
    egui::Grid::new("frames_2d_summary")
        .striped(true)
        .spacing([12.0, 5.0])
        .show(ui, |ui| {
            for title in [
                "Pórtico",
                "Modelo",
                "M máx viga",
                "M máx pilar",
                "P concentrada",
                "Vento H",
                "Desloc. lateral Δh",
                "Limite H/850",
                "Situação",
            ] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for frame in frames {
                let model = match (frame.rigid_joints, frame.fixed_bases) {
                    (true, true) => "Rígido / Engast.",
                    (true, false) => "Rígido / Rotul.",
                    (false, true) => "Rotul. / Engast.",
                    (false, false) => "Rotul. / Contrav.",
                };
                ui.label(RichText::new(&frame.label).font(strong(12.0)));
                ui.label(model);
                ui.label(format!("{} kN·m", dec(frame.max_beam_moment_knm(), 1)));
                ui.label(format!("{} kN·m", dec(frame.max_column_moment_knm(), 1)));
                ui.label(format!("{} kN", dec(frame.total_point_load_kn(), 1)));
                ui.label(format!("{} kN", dec(frame.lateral_wind_kn, 1)));
                ui.label(format!("{} mm", dec(frame.max_lateral_drift_mm, 2)));
                ui.label(format!("{} mm", dec(frame.drift_limit_mm, 2)));
                if frame.drift_ok() {
                    ui.label(RichText::new("Atende").color(SUCCESS));
                } else {
                    ui.label(RichText::new("Revise").color(WARNING));
                }
                ui.end_row();
            }
        });

    ui.add_space(10.0);
    ui.label(RichText::new("Esforços nos pilares e reações de apoio (2D)").font(strong(13.0)));
    ui.add_space(4.0);
    egui::Grid::new("frames_2d_columns")
        .striped(true)
        .spacing([12.0, 5.0])
        .show(ui, |ui| {
            for title in [
                "Pórtico",
                "Pilar",
                "N (kN)",
                "V (kN)",
                "M base",
                "M topo",
                "Rx / Ry (kN)",
                "Mz base",
            ] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for frame in frames {
                for col in &frame.columns {
                    ui.label(&frame.label);
                    ui.label(RichText::new(&col.label).font(strong(12.0)));
                    ui.label(dec(col.axial_kn, 1));
                    ui.label(dec(col.shear_kn, 1));
                    ui.label(format!("{} kN·m", dec(col.moment_base_knm, 1)));
                    ui.label(format!("{} kN·m", dec(col.moment_top_knm, 1)));
                    ui.label(format!(
                        "{} / {}",
                        dec(col.reaction_rx_kn, 1),
                        dec(col.reaction_ry_kn, 1)
                    ));
                    ui.label(format!("{} kN·m", dec(col.reaction_mz_knm, 1)));
                    ui.end_row();
                }
            }
        });

    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Análise elástica linear de pórticos planos (3 GDL/nó: u, v, θ) com rigidez real de vigas e pilares, ligações rígidas ou rotuladas e equilíbrio global verificado.",
        )
        .size(11.0)
        .color(MUTED),
    );
}
