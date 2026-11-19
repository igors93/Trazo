//! The options bar of the modify tools: the step reached, "keep the
//! original" for turning and mirroring, and the offset distance.

use eframe::egui::{self, Key, Ui};

use super::{caption, chip, divider};
use crate::app::FloorPlanApp;
use crate::app::tools::ModifyKind;
use crate::app::ui::widgets::{tip, toggle_pill};
use crate::domain::geometry::format_meters;

impl FloorPlanApp {
    pub(super) fn modify_options(&mut self, ui: &mut Ui, kind: ModifyKind) {
        if kind.uses_selection() {
            chip(ui, &self.selection_summary_short());
            let step = match (kind, self.modify_anchor.is_some()) {
                (ModifyKind::Rotate, false) => "1. clique no centro",
                (ModifyKind::Rotate, true) => "2. clique no ângulo ou digite os graus",
                (ModifyKind::Mirror, false) => "1. primeiro ponto do eixo",
                (ModifyKind::Mirror, true) => "2. segundo ponto do eixo",
                (_, false) => "1. clique no ponto base",
                (_, true) => "2. clique no destino ou digite a distância",
            };
            caption(ui, step);
            if kind.can_keep_original() {
                divider(ui);
                if toggle_pill(
                    ui,
                    "Manter original",
                    self.modify_keep,
                    "Ligado: cria uma cópia girada ou espelhada e mantém os originais",
                )
                .clicked()
                {
                    self.modify_keep = !self.modify_keep;
                }
            }
            return;
        }
        if kind == ModifyKind::Offset {
            caption(ui, "Distância");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.offset_distance_text)
                    .desired_width(56.0)
                    .align(egui::Align2::RIGHT_CENTER),
            );
            caption(ui, "m");
            let typed = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            match self.offset_distance() {
                Some(distance) => {
                    if typed {
                        self.message =
                            format!("Distância de deslocamento: {}.", format_meters(distance));
                    }
                }
                None => {
                    caption(ui, "informe um valor maior que zero");
                }
            }
            divider(ui);
            caption(
                ui,
                if self.modify_target.is_some() {
                    "Clique no lado para onde deslocar · Esc escolhe outro elemento"
                } else {
                    "Clique na parede ou desenho a deslocar (paredes: de eixo a eixo)"
                },
            );
            return;
        }
        if kind == ModifyKind::Split {
            let at_point = self.split_parts <= 1;
            if toggle_pill(
                ui,
                "No ponto clicado",
                at_point,
                "Corta a parede ou o desenho em dois onde você clicar",
            )
            .clicked()
            {
                self.split_parts = 1;
            }
            if toggle_pill(
                ui,
                "Em partes iguais",
                !at_point,
                "Corta a parede ou o desenho clicado em partes do mesmo comprimento",
            )
            .clicked()
                && at_point
            {
                self.split_parts = 2;
            }
            if !at_point {
                caption(ui, "Partes");
                ui.add(egui::DragValue::new(&mut self.split_parts).range(2..=100));
            }
            divider(ui);
            caption(
                ui,
                "Paredes mantêm cômodos, portas e janelas; o corte não passa por uma abertura.",
            );
            return;
        }
        caption(
            ui,
            match kind {
                ModifyKind::Trim => {
                    "Clique no trecho a remover; o corte vai até as paredes, desenhos ou divisas mais próximas."
                }
                _ => "Clique perto da ponta a estender até a próxima parede, desenho ou divisa.",
            },
        );
        tip(
            chip(ui, "Paredes e desenhos 2D"),
            "Portas e janelas no trecho a aparar impedem o corte; pontas ligadas a outras paredes não são estendidas.",
        );
    }

    /// "Parede", "Cômodo" or "3 elementos" for the options bar.
    fn selection_summary_short(&self) -> String {
        match self.selection_count() {
            0 => "nada selecionado".to_owned(),
            1 => self
                .selection
                .map(|item| item.kind().singular().to_owned())
                .unwrap_or_default(),
            count => format!("{count} elementos"),
        }
    }
}

impl FloorPlanApp {
    pub(super) fn constrain_options(
        &mut self,
        ui: &mut Ui,
        kind: crate::app::tools::ConstraintTool,
    ) {
        use crate::app::tools::ConstraintTool;
        let step = match (kind, self.constraint_pick.is_some()) {
            (ConstraintTool::Coincident, false) => "1. ponto de referência",
            (ConstraintTool::Coincident, true) => "2. ponto que vai até ele",
            (ConstraintTool::Distance, false) => "1. primeiro ponto ou linha",
            (ConstraintTool::Distance, true) => "2. segundo ponto ou linha",
            (_, false) => "1. linha de referência",
            (_, true) => "2. linha que se ajusta",
        };
        chip(ui, step);
        caption(ui, kind.hint());
        divider(ui);
        caption(
            ui,
            &format!(
                "{} restrição(ões) na planta",
                self.project.constraints.len()
            ),
        );
    }
}
