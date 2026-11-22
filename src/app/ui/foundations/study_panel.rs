//! Comparative foundation study panel inside the "Solo e fundações" tab.

use eframe::egui::{RichText, Ui};

use super::decimal;
use crate::app::theme::*;
use crate::domain::model::Project;

pub(super) fn show(ui: &mut Ui, project: &Project) {
    ui.add_space(10.0);
    ui.label(RichText::new("Estudo comparativo de fundações").font(strong(13.0)));
    let study = project.foundation_study();
    if study.options.is_empty() {
        ui.label(
            RichText::new(
                "Gere a estrutura (pilares) e as fundações das paredes para comparar as opções.",
            )
            .color(MUTED),
        );
        return;
    }
    let borehole_note = if study.borehole_count > 1 {
        format!(
            " · {} furos SPT{}",
            study.borehole_count,
            study
                .governing_borehole
                .as_ref()
                .map_or(String::new(), |g| format!(" (crítico: {g})"))
        )
    } else if study.borehole_count == 1 {
        " · 1 furo SPT".to_owned()
    } else {
        " · tabela de solo".to_owned()
    };
    ui.label(
        RichText::new(format!(
            "Carga total {} kN sobre {} m² de projeção ({} pilar(es){}{}).",
            decimal(study.total_load_kn, 0),
            decimal(study.footprint_m2, 1),
            study.column_loads_kn.len(),
            if study.strip_loads.is_empty() {
                String::new()
            } else {
                format!(", {} parede(s) em sapata corrida", study.strip_loads.len())
            },
            borehole_note,
        ))
        .color(TEXT_2),
    );
    for option in &study.options {
        ui.add_space(6.0);
        let recommended = study.recommended == Some(option.option);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(option.option.label()).font(strong(12.5)));
            if recommended {
                ui.label(
                    RichText::new("recomendada")
                        .font(strong(12.0))
                        .color(SUCCESS),
                );
            } else if option.viable {
                ui.label(RichText::new("viável").color(SUCCESS));
            } else {
                ui.label(RichText::new("não indicada").color(WARNING));
            }
            if option.concrete_m3 > 0.0 {
                ui.label(
                    RichText::new(format!("{} m³ de concreto", decimal(option.concrete_m3, 2)))
                        .color(MUTED),
                );
            }
            if let Some(s) = option.settlement_mm {
                let s_text = match option.differential_settlement_mm {
                    Some(d) => format!("recalque {} mm (dif. {} mm)", decimal(s, 1), decimal(d, 1)),
                    None => format!("recalque {} mm", decimal(s, 1)),
                };
                ui.label(RichText::new(s_text).color(MUTED));
            }
        });
        ui.label(&option.summary);
        for note in &option.notes {
            ui.label(RichText::new(format!("• {note}")).size(11.5).color(MUTED));
        }
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Sapatas: σadm = 20·N̄ kPa (5 ≤ N̄ ≤ 20, envoltória do furo mais fraco) e recalque elástico imediato ≤ 25 mm (dif. ≤ 15 mm). Radier: recalque ≤ 40 mm. Brocas: Décourt-Quaresma 3–6 m acima do lençol. Estacas profundas: hélice contínua 6–20 m (opera abaixo do lençol). Recomendada: a opção viável com menor volume de concreto. Estudo preliminar — projeto executivo exige engenheiro geotécnico e sondagem local (NBR 6122, NBR 6484).",
        )
        .size(11.0)
        .color(MUTED),
    );
}
