//! Boundary foundations panel (balance beams / *vigas alavanca* and strip-footing
//! alternative, #144) inside the "Solo e fundações" tab.

use eframe::egui::{self, RichText, Ui};

use super::{SoilAction, decimal};
use crate::app::theme::*;
use crate::app::ui::widgets::secondary_button;
use crate::domain::model::Project;

pub(super) fn show(ui: &mut Ui, project: &Project) -> Option<SoilAction> {
    let mut action = None;
    let report = project.cached_structure_report();
    let eccentric_unbalanced = report
        .footings
        .iter()
        .filter(|f| f.eccentricity_m > 0.05)
        .filter(|f| {
            !report
                .balance_beams
                .iter()
                .any(|b| b.footing_id == Some(f.id))
        })
        .count();

    if report.balance_beams.is_empty() && eccentric_unbalanced == 0 {
        return None;
    }

    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Fundações de divisa · vigas alavanca (NBR 6122)").font(strong(13.0)),
        );
        if eccentric_unbalanced > 0
            && secondary_button(
                ui,
                &format!("Lançar vigas alavanca ({eccentric_unbalanced} excêntrica(s))"),
            )
            .clicked()
        {
            action = Some(SoilAction::GenerateBalanceBeams);
        }
    });

    if report.balance_beams.is_empty() {
        ui.label(
            RichText::new(
                "Há sapata(s) excêntrica(s) na divisa sem viga de equilíbrio: lance vigas alavanca até os pilares internos ou adote sapata corrida na parede de divisa.",
            )
            .color(WARNING),
        );
        return action;
    }

    egui::Grid::new("foundation_balance_beams")
        .striped(true)
        .spacing([12.0, 5.0])
        .show(ui, |ui| {
            for title in [
                "Viga",
                "Par",
                "Vão / e",
                "R1 / ΔP",
                "FS tomb.",
                "σ1 / σ2 (kPa)",
                "Seção",
                "Armadura",
                "Situação",
            ] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for beam in &report.balance_beams {
                ui.label(RichText::new(&beam.label).font(strong(12.0)));
                ui.label(format!("{} → {}", beam.column, beam.anchor));
                ui.label(format!(
                    "{} m / {:.0} cm",
                    decimal(beam.span_m, 2),
                    beam.eccentricity_m * 100.0
                ));
                ui.label(format!(
                    "{} / {} kN",
                    decimal(beam.reaction_kn, 1),
                    decimal(beam.relief_kn, 1)
                ));
                let fs_txt = if beam.pair_overturning_fs.is_infinite() {
                    "∞".to_owned()
                } else {
                    decimal(beam.pair_overturning_fs, 2)
                };
                ui.label(fs_txt);
                let s1 = beam
                    .boundary_pressure_kpa
                    .map_or("—".to_owned(), |v| format!("{v:.0}"));
                let s2 = beam
                    .anchor_pressure_kpa
                    .map_or("—".to_owned(), |v| format!("{v:.0}"));
                ui.label(format!("{s1} / {s2}"));
                let sec = beam.height_m.map_or("insuficiente".to_owned(), |h| {
                    format!("{:.0}×{:.0} cm", beam.width_m * 100.0, h * 100.0)
                });
                ui.label(sec);
                ui.label(format!("{} · {}", beam.bars, beam.stirrups));
                let ok = beam.overturning_ok() && beam.pair_soil_ok() && beam.height_m.is_some();
                if ok {
                    ui.label(RichText::new("Atende").color(SUCCESS));
                } else {
                    ui.label(RichText::new("Revise").color(WARNING));
                }
                ui.end_row();
            }
        });

    for beam in &report.balance_beams {
        if let Some(alt) = &beam.strip_alternative {
            ui.label(
                RichText::new(format!("• {} ({}): {}", beam.label, beam.column, alt.note))
                    .size(11.5)
                    .color(WARNING),
            );
        }
    }

    action
}
