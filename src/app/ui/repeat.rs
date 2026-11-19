//! The "Repetir" dialog: copies of the selection in rows and columns, or
//! around a centre.

use eframe::egui::{self, Key, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::Glyph;
use super::dialogs::{backdrop, dialog_frame, dialog_title};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::model::MAX_COPIES;

/// A labelled number field of the dialog.
fn field(ui: &mut Ui, label: &str, text: &mut String, unit: &str, hint: &str) {
    ui.label(RichText::new(label).color(TEXT_2));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(text)
                .desired_width(80.0)
                .align(egui::Align2::RIGHT_CENTER),
        )
        .on_hover_text(hint);
        if !unit.is_empty() {
            ui.label(RichText::new(unit).color(MUTED));
        }
    });
    ui.end_row();
}

impl FloorPlanApp {
    pub(super) fn repeat_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.repeat_form.clone() else {
            return;
        };
        let mut create = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("repeat_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(ctx, |ui| {
                ui.set_width(380.0);
                dialog_title(
                    ui,
                    Icon::Glyph(Glyph::Repeat),
                    "Repetir",
                    &format!("{} — o original é o primeiro item.", self.selection_summary_text()),
                );
                ui.horizontal(|ui| {
                    if toggle_pill(ui, "Linhas e colunas", !form.polar, "Cópias em grade")
                        .clicked()
                    {
                        form.polar = false;
                    }
                    if toggle_pill(ui, "Em volta de um centro", form.polar, "Cópias em círculo")
                        .clicked()
                    {
                        form.polar = true;
                    }
                });
                ui.add_space(10.0);
                egui::Grid::new("repeat_grid")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        if form.polar {
                            field(ui, "Itens no total", &mut form.count, "", "Contando o original");
                            field(
                                ui,
                                "Ângulo total",
                                &mut form.degrees,
                                "°",
                                "360° espalha por igual em volta; positivo gira no sentido anti-horário",
                            );
                            field(ui, "Centro X", &mut form.center_x, "m", "Coordenada X do centro");
                            field(
                                ui,
                                "Centro Y",
                                &mut form.center_y,
                                "m",
                                "Coordenada Y do centro (para cima, como na barra de status)",
                            );
                        } else {
                            field(ui, "Colunas", &mut form.columns, "", "Para a direita");
                            field(ui, "Linhas", &mut form.rows, "", "Para cima");
                            field(
                                ui,
                                "Espaçamento X",
                                &mut form.spacing_x,
                                "m",
                                "Distância entre colunas (negativo: para a esquerda)",
                            );
                            field(
                                ui,
                                "Espaçamento Y",
                                &mut form.spacing_y,
                                "m",
                                "Distância entre linhas (negativo: para baixo)",
                            );
                        }
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Até {MAX_COPIES} cópias. Objetos só giram em passos de 90°; tudo vira um passo de desfazer."
                    ))
                    .size(11.5)
                    .color(MUTED),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    create = primary_button(ui, "Criar cópias").clicked();
                    cancel = secondary_button(ui, "Cancelar").clicked();
                });
                if ui.input(|input| input.key_pressed(Key::Enter)) {
                    create = true;
                }
                if ui.input(|input| input.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            });
        if cancel {
            self.repeat_form = None;
            return;
        }
        self.repeat_form = Some(form);
        if create {
            self.apply_repeat();
        }
    }

    /// "3 elementos: 2 paredes, 1 cômodo", or the one element's kind.
    fn selection_summary_text(&self) -> String {
        if self.is_multi_selection() {
            self.selection_summary()
        } else {
            self.selection
                .map(|item| item.kind().singular().to_owned())
                .unwrap_or_else(|| "Nada selecionado".to_owned())
        }
    }
}
