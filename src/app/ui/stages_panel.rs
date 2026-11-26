//! "Construção em etapas" inside the "Segurança" window (#158): the floors
//! the family means to build later, and whether the foundation is built.

use eframe::egui::{self, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::model::Project;
use crate::domain::protection::stages::{
    extension_beyond_built, mark_foundation_built, plan_future_floors,
};

/// What the person asked for in the staged-building panel.
pub(super) enum StageAction {
    FutureFloors(u32),
    FoundationBuilt(bool),
}

pub(super) fn show(ui: &mut Ui, project: &Project) -> Option<StageAction> {
    let mut action = None;
    let plan = project.protection.stages;
    let beyond = extension_beyond_built(project);
    egui::CollapsingHeader::new(
        RichText::new("Construção em etapas")
            .font(strong(13.0))
            .color(if beyond.is_some() { DANGER } else { TEXT }),
    )
    .id_salt("safety_stages")
    .default_open(beyond.is_some())
    .show(ui, |ui| {
        ui.label(
            RichText::new(
                "Vai subir outro andar depois? Diga agora: a fundação, os pilares e a laje do térreo serão calculados para aguentar.",
            )
            .size(11.5)
            .color(TEXT_2),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label("Andares que pretende construir depois:");
            for floors in 0..=2_u32 {
                if ui
                    .selectable_label(plan.future_floors == floors, floors.to_string())
                    .clicked()
                    && plan.future_floors != floors
                {
                    action = Some(StageAction::FutureFloors(floors));
                }
            }
        });
        let built = plan.built_for.is_some();
        let label = if built {
            "Desmarcar \"fundação já feita\""
        } else {
            "A fundação já foi feita"
        };
        if secondary_button(ui, label)
            .on_hover_text("Marca para quantos andares a fundação foi feita; uma ampliação além disso fica vermelha.")
            .clicked()
        {
            action = Some(StageAction::FoundationBuilt(!built));
        }
        if let Some(for_floors) = plan.built_for {
            ui.label(
                RichText::new(format!("Fundação feita para {for_floors} pavimento(s)."))
                    .color(MUTED),
            );
        }
        if let Some((built, now)) = beyond {
            ui.label(
                RichText::new(format!(
                    "A fundação foi feita para {built} pavimento(s) e o projeto agora tem {now}: pare e procure ajuda."
                ))
                .color(DANGER),
            );
        }
    });
    action
}

impl FloorPlanApp {
    pub(super) fn apply_stage_action(&mut self, action: StageAction) {
        let previous = self.project.clone();
        let result = match action {
            StageAction::FutureFloors(floors) => plan_future_floors(&mut self.project, floors)
                .map(|()| {
                    if floors == 0 {
                        "Sem andares futuros: a estrutura volta a carregar só o que está desenhado.".to_owned()
                    } else {
                        format!("A estrutura agora carrega mais {floors} andar(es) futuro(s). Ajuste as sapatas às novas cargas.")
                    }
                }),
            StageAction::FoundationBuilt(built) => {
                mark_foundation_built(&mut self.project, built);
                Ok(if built {
                    "Fundação marcada como feita.".to_owned()
                } else {
                    "Fundação desmarcada.".to_owned()
                })
            }
        };
        match result {
            Ok(message) => {
                if self.record_change(previous) {
                    self.message = message;
                }
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
            }
        }
    }
}
