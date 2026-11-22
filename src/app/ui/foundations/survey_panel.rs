//! Multi-borehole SPT soil survey editor inside the "Solo e fundações" tab.

use eframe::egui::{self, RichText, Ui};

use super::{SoilAction, decimal};
use crate::app::theme::*;
use crate::app::ui::widgets::{primary_button, secondary_button};
use crate::domain::model::Project;
use crate::domain::soil::{Borehole, SoilKind, SoilSurvey, SptReading};

pub(super) fn show(ui: &mut Ui, project: &Project) -> Option<SoilAction> {
    let mut action = None;
    ui.add_space(8.0);
    ui.label(RichText::new("Sondagem (SPT)").font(strong(13.0)));

    let Some(survey) = &project.structure.soil_survey else {
        ui.label(
            RichText::new(format!(
                "Sem sondagem: a tensão do solo vem da tabela/configuração acima ({:.0} kPa). Com o laudo do SPT (um ou mais furos), o Trazo estima tensão, recalque, brocas e estacas profundas.",
                project.structure.settings.soil_kpa
            ))
            .color(TEXT_2),
        );
        if primary_button(ui, "Criar sondagem").clicked() {
            let readings = (1..=6)
                .map(|depth| SptReading {
                    depth_m: f64::from(depth),
                    n: 5,
                    soil: SoilKind::Clay,
                })
                .collect();
            action = SoilSurvey::new("SP-01".to_owned(), readings, None, 1.0)
                .ok()
                .map(|s| SoilAction::Survey(Some(s)));
        }
        return action;
    };

    let mut boreholes = survey.boreholes();
    let mut footing_depth_m = survey.footing_depth_m;
    let selected_id = egui::Id::unique("trazo_soil_selected_borehole");
    let mut selected: usize = ui
        .ctx()
        .data(|d| d.get_temp(selected_id))
        .unwrap_or(0)
        .min(boreholes.len().saturating_sub(1));
    let mut changed = false;

    // Borehole selector strip (SP-01, SP-02, ... + add/remove).
    ui.horizontal_wrapped(|ui| {
        for (idx, b) in boreholes.iter().enumerate() {
            if ui
                .selectable_label(selected == idx, RichText::new(&b.name).font(strong(12.0)))
                .clicked()
            {
                selected = idx;
            }
        }
        if secondary_button(ui, "+ Furo").clicked() {
            let next_num = boreholes.len() + 1;
            let template = boreholes
                .get(selected)
                .cloned()
                .unwrap_or_else(|| Borehole {
                    name: format!("SP-{next_num:02}"),
                    readings: (1..=6)
                        .map(|d| SptReading {
                            depth_m: f64::from(d),
                            n: 5,
                            soil: SoilKind::Clay,
                        })
                        .collect(),
                    water_table_m: None,
                });
            boreholes.push(Borehole {
                name: format!("SP-{next_num:02}"),
                readings: template.readings,
                water_table_m: template.water_table_m,
            });
            selected = boreholes.len() - 1;
            changed = true;
        }
        if boreholes.len() > 1
            && secondary_button(ui, "Remover furo")
                .on_hover_text("Remove o furo atualmente selecionado")
                .clicked()
        {
            boreholes.remove(selected);
            selected = selected.min(boreholes.len().saturating_sub(1));
            changed = true;
        }
    });

    ui.add_space(4.0);
    let active = &mut boreholes[selected];
    let deepest_active = active.deepest_m();
    egui::Grid::new("soil_survey_settings")
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            ui.label("Furo");
            changed |= ui
                .add(egui::TextEdit::singleline(&mut active.name).desired_width(110.0))
                .changed();
            ui.end_row();
            ui.label("Base das sapatas");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut footing_depth_m)
                        .range(0.4..=3.0)
                        .speed(0.0)
                        .max_decimals(2)
                        .suffix(" m"),
                )
                .changed();
            ui.end_row();
            ui.label("Lençol freático");
            ui.horizontal(|ui| {
                let mut found = active.water_table_m.is_some();
                if ui.checkbox(&mut found, "encontrado a").changed() {
                    active.water_table_m = found.then_some(deepest_active.min(3.0));
                    changed = true;
                }
                if let Some(water) = &mut active.water_table_m {
                    changed |= ui
                        .add(
                            egui::DragValue::new(water)
                                .range(0.0..=40.0)
                                .speed(0.0)
                                .max_decimals(2)
                                .suffix(" m"),
                        )
                        .changed();
                }
            });
            ui.end_row();
        });

    ui.add_space(4.0);
    let mut remove_row = None;
    let several = active.readings.len() > 1;
    egui::Grid::new(("soil_survey_readings", selected))
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for title in ["Profundidade", "N (golpes)", "Solo", ""] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for (i, reading) in active.readings.iter_mut().enumerate() {
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut reading.depth_m)
                            .range(0.5..=40.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut reading.n)
                            .range(0..=100)
                            .speed(0.0),
                    )
                    .changed();
                egui::ComboBox::from_id_salt(("soil_kind", selected, i))
                    .selected_text(reading.soil.label())
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for kind in SoilKind::ALL {
                            if ui
                                .selectable_label(reading.soil == kind, kind.label())
                                .clicked()
                            {
                                reading.soil = kind;
                                changed = true;
                            }
                        }
                    });
                if several
                    && ui
                        .small_button("✕")
                        .on_hover_text("Remover a leitura")
                        .clicked()
                {
                    remove_row = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove_row {
        active.readings.remove(i);
        changed = true;
    }

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if secondary_button(ui, "+ Leitura").clicked()
            && let Some(last) = active.readings.last().copied()
        {
            active.readings.push(SptReading {
                depth_m: (last.depth_m + 1.0).min(40.0),
                ..last
            });
            changed = true;
        }
        if let Some((n_bar, _, gov)) = survey.governing_footing_bulb(1.0) {
            let kpa = (20.0 * n_bar).min(crate::domain::soil::MAX_FOOTING_KPA);
            let label = if survey.borehole_count() > 1 {
                format!("Usar na tensão do solo ({kpa:.0} kPa · {gov})")
            } else {
                format!("Usar na tensão do solo ({kpa:.0} kPa)")
            };
            if secondary_button(ui, &label)
                .on_hover_text(
                    "σadm = 20 · N̄ kPa no bulbo da sapata (envoltória do furo mais fraco, máximo 400 kPa)",
                )
                .clicked()
            {
                action = Some(SoilAction::ApplyToSoil);
            }
        }
        if secondary_button(ui, "Remover sondagem").clicked() {
            action = Some(SoilAction::Survey(None));
        }
    });

    if survey.borehole_count() > 1 {
        let water_note = survey
            .governing_water_table_m()
            .map_or("não encontrado".to_owned(), |w| {
                format!("{} m", decimal(w, 2))
            });
        ui.label(
            RichText::new(format!(
                "Envoltória conservadora de {} furos: profundidade comum {} m · lençol mais raso: {water_note}.",
                survey.borehole_count(),
                decimal(survey.deepest_m(), 1),
            ))
            .size(11.5)
            .color(MUTED),
        );
    }

    ui.ctx().data_mut(|d| d.insert_temp(selected_id, selected));

    if changed {
        match SoilSurvey::from_boreholes(boreholes, footing_depth_m) {
            Ok(updated) => action = Some(SoilAction::Survey(Some(updated))),
            Err(error) => {
                ui.label(RichText::new(error).color(DANGER));
            }
        }
    }

    action
}
