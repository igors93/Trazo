//! Foundation commands in the interface: balance beams for boundary
//! footings (#144), wall foundations (#143) and the soil survey with the
//! foundation study (#112).

use eframe::egui::{self, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button};
use crate::domain::model::Project;
use crate::domain::soil::{SoilKind, SoilSurvey, SptReading};

impl FloorPlanApp {
    /// Gives every ground-floor wall a ground beam or a strip footing.
    pub(crate) fn generate_wall_foundations(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (beams, strips) = self.project.generate_wall_foundations();
        if beams + strips == 0 {
            self.message =
                "Todas as paredes do térreo já têm fundação (ou você está num pavimento superior)."
                    .to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "Fundação das paredes: {beams} viga(s) baldrame e {strips} sapata(s) corrida(s). Use Ajustar sapatas para incluir o peso das paredes."
            );
        }
    }

    /// Lays a balance beam from every off-centre boundary footing to the
    /// nearest interior column.
    pub(crate) fn generate_balance_beams(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let (added, alone) = self.project.generate_balance_beams();
        if added == 0 {
            self.message = if alone > 0 {
                format!(
                    "{alone} pilar(es) de divisa sem pilar interno a até 10 m para ancorar uma viga alavanca."
                )
            } else {
                "Nenhuma sapata de divisa fora do centro do pilar (ou todas já têm viga alavanca)."
                    .to_owned()
            };
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "{added} viga(s) alavanca lançada(s): as sapatas de divisa ficam centradas.{}",
                if alone > 0 {
                    format!(" {alone} pilar(es) de divisa ficaram sem pilar interno para ancorar.")
                } else {
                    String::new()
                }
            );
        }
    }
}

/// What the soil tab asks for (#112).
pub(super) enum SoilAction {
    /// Replace or remove the survey.
    Survey(Option<SoilSurvey>),
    /// Take the survey's footing pressure as the soil pressure.
    ApplyToSoil,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// The "Solo e fundações" tab of the structure window: the SPT survey and
/// the foundation options for the real loads.
pub(super) fn soil_tab(ui: &mut Ui, project: &Project) -> Option<SoilAction> {
    let mut action = None;
    let heading = |ui: &mut Ui, text: &str| {
        ui.add_space(8.0);
        ui.label(RichText::new(text).font(strong(13.0)));
    };
    heading(ui, "Sondagem (SPT)");
    let Some(survey) = &project.structure.soil_survey else {
        ui.label(
            RichText::new(format!(
                "Sem sondagem: a tensão do solo vem da configuração acima ({:.0} kPa). Com o laudo do SPT, o Trazo estima a tensão das sapatas e dimensiona as brocas.",
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
    let mut edited = survey.clone();
    let mut changed = false;
    egui::Grid::new("soil_survey_settings")
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            ui.label("Furo");
            ui.label(RichText::new(&survey.name).font(strong(12.5)));
            ui.end_row();
            ui.label("Base das sapatas");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut edited.footing_depth_m)
                        .range(0.4..=3.0)
                        .speed(0.0)
                        .max_decimals(2)
                        .suffix(" m"),
                )
                .changed();
            ui.end_row();
            ui.label("Lençol freático");
            ui.horizontal(|ui| {
                let mut found = edited.water_table_m.is_some();
                if ui.checkbox(&mut found, "encontrado a").changed() {
                    edited.water_table_m = found.then_some(survey.deepest_m().min(3.0));
                    changed = true;
                }
                if let Some(water) = &mut edited.water_table_m {
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
    let several = edited.readings.len() > 1;
    egui::Grid::new("soil_survey_readings")
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for title in ["Profundidade", "N (golpes)", "Solo", ""] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for (i, reading) in edited.readings.iter_mut().enumerate() {
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
                egui::ComboBox::from_id_salt(("soil_kind", i))
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
        edited.readings.remove(i);
        changed = true;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if secondary_button(ui, "+ Leitura").clicked()
            && let Some(last) = edited.readings.last().copied()
        {
            edited.readings.push(SptReading {
                depth_m: last.depth_m + 1.0,
                ..last
            });
            changed = true;
        }
        if let Some(kpa) = survey.footing_kpa(1.0)
            && secondary_button(ui, &format!("Usar na tensão do solo ({kpa:.0} kPa)"))
                .on_hover_text(
                    "σadm = 20 · N̄ kPa no bulbo da sapata (da base até 2 m abaixo), no máximo 400 kPa",
                )
                .clicked()
        {
            action = Some(SoilAction::ApplyToSoil);
        }
        if secondary_button(ui, "Remover sondagem").clicked() {
            action = Some(SoilAction::Survey(None));
        }
    });
    if changed {
        match SoilSurvey::new(
            edited.name.clone(),
            edited.readings.clone(),
            edited.water_table_m,
            edited.footing_depth_m,
        ) {
            Ok(survey) => action = Some(SoilAction::Survey(Some(survey))),
            Err(error) => {
                ui.label(RichText::new(error).color(DANGER));
            }
        }
    }

    heading(ui, "Estudo de fundações");
    let study = project.foundation_study();
    if study.options.is_empty() {
        ui.label(
            RichText::new(
                "Gere a estrutura (pilares) e as fundações das paredes para comparar as opções.",
            )
            .color(MUTED),
        );
        return action;
    }
    ui.label(
        RichText::new(format!(
            "Carga total {} kN sobre {} m² de projeção ({} pilar(es){}).",
            decimal(study.total_load_kn, 0),
            decimal(study.footprint_m2, 1),
            study.column_loads_kn.len(),
            if study.strip_loads.is_empty() {
                String::new()
            } else {
                format!(", {} parede(s) em sapata corrida", study.strip_loads.len())
            }
        ))
        .color(TEXT_2),
    );
    for option in &study.options {
        ui.add_space(6.0);
        let recommended = study.recommended == Some(option.option);
        ui.horizontal(|ui| {
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
        });
        ui.label(&option.summary);
        for note in &option.notes {
            ui.label(RichText::new(format!("• {note}")).size(11.5).color(MUTED));
        }
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new("Sapatas: σadm = 20 · N̄ kPa (N̄/50 MPa, 5 ≤ N̄ ≤ 20) da base até 2B abaixo. Brocas: Décourt-Quaresma para estacas escavadas, carga admissível = mín((Rp + Rl)/2; Rp/4 + Rl/1,3), ponta até 20 %, de 3 a 6 m e acima do lençol. Recomendada: a viável com menos concreto. É um estudo inicial: o projeto de fundações é de um engenheiro habilitado, com sondagem no local (NBR 6122, NBR 6484).")
            .size(11.0)
            .color(MUTED),
    );
    action
}
