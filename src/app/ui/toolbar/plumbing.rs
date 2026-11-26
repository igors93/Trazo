//! The "Hidráulica" ribbon tab (#96): the fixtures that take water, how the
//! water is heated, the plumbing view and its sheets.

use eframe::egui::Ui;

use crate::app::FloorPlanApp;
use crate::app::ui::icons::Icon;
use crate::app::ui::widgets::{
    RIBBON_PANEL_HEIGHT, ribbon_button, ribbon_panel, ribbon_small, ribbon_stack, tip,
    vertical_divider,
};
use crate::domain::discipline::Discipline;
use crate::domain::fixtures::FixtureKind;
use crate::domain::layers::LayerCategory;
use crate::domain::plumbing::Heater;

impl FloorPlanApp {
    pub(super) fn plumbing_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Aparelhos", |ui| {
            self.fixture_button(ui, FixtureKind::Toilet, "Vaso");
            self.fixture_button(ui, FixtureKind::Washbasin, "Lavatório");
            self.fixture_button(ui, FixtureKind::Shower, "Chuveiro");
            self.fixture_button(ui, FixtureKind::KitchenSink, "Pia");
            self.fixture_button(ui, FixtureKind::Washer, "Lavadora");
            self.fixture_button(ui, FixtureKind::Bathtub, "Banheira");
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Aquecimento", |ui| {
            ribbon_stack(ui, |ui| {
                for heater in Heater::ALL {
                    let on = self.project.plumbing.heater == heater;
                    let hint = match heater {
                        Heater::ElectricShower => {
                            "Chuveiro elétrico em cada banheiro: sem tubos de água quente"
                        }
                        Heater::Gas => {
                            "Aquecedor a gás de passagem: água quente na pia, no lavatório e no chuveiro"
                        }
                        Heater::Solar => {
                            "Coletor solar com reservatório: água quente na pia, no lavatório e no chuveiro"
                        }
                    };
                    let icon = match heater {
                        Heater::ElectricShower => Icon::Bolt,
                        Heater::Gas => Icon::Fixture(FixtureKind::Stove),
                        Heater::Solar => Icon::Fixture(FixtureKind::SunLounger),
                    };
                    if tip(ribbon_small(ui, icon, heater.label(), on), hint).clicked() {
                        self.set_plumbing_heater(heater);
                    }
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Projeto", |ui| {
            if tip(
                ribbon_button(ui, Icon::ExportPdf, "Projeto PDF", false, false),
                "Pranchas A3 do projeto hidrossanitário: pontos de água, esgoto e gás com legenda e quadro de aparelhos",
            )
            .clicked()
            {
                self.export_plumbing();
            }
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Exibição", |ui| {
            ribbon_stack(ui, |ui| {
                let only = self.discipline == Discipline::Plumbing;
                if tip(
                    ribbon_small(ui, Icon::Layers, "Só hidráulica", only),
                    "Esmaece a arquitetura e o mobiliário para destacar os aparelhos e os pontos de água, esgoto e gás",
                )
                .clicked()
                {
                    self.set_discipline(if only {
                        Discipline::All
                    } else {
                        Discipline::Plumbing
                    });
                }
                let visible = self.project.category_visible(LayerCategory::Plumbing);
                let (icon, label) = if visible {
                    (Icon::Eye, "Ocultar hidráulica")
                } else {
                    (Icon::EyeOff, "Mostrar hidráulica")
                };
                if tip(
                    ribbon_small(ui, icon, label, !visible),
                    "Mostra ou oculta a camada Instalações hidrossanitárias",
                )
                .clicked()
                {
                    self.toggle_plumbing_layer();
                }
            });
        });
    }

    /// Chooses how the water is heated, as one undo step.
    pub(crate) fn set_plumbing_heater(&mut self, heater: Heater) {
        if self.project.plumbing.heater == heater {
            return;
        }
        let previous = self.project.clone();
        self.project.plumbing.heater = heater;
        self.record_change(previous);
        self.message = format!("Aquecimento da água: {}.", heater.label().to_lowercase());
    }

    pub(crate) fn toggle_plumbing_layer(&mut self) {
        let previous = self.project.clone();
        self.project.ensure_default_layers();
        if let Some(layer) = self
            .project
            .layers
            .iter_mut()
            .find(|layer| layer.category == Some(LayerCategory::Plumbing))
        {
            layer.visible = !layer.visible;
        }
        self.record_change(previous);
    }
}
