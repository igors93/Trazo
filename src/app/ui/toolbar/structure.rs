//! The "Estrutura" ribbon tab: columns and footings placed on the plan,
//! beams between columns, the generated structure, its pre-sizing and the
//! forces in the beams, and the formwork view.

use eframe::egui::{self, Ui};

use crate::app::ui::icons::{Glyph, Icon};
use crate::app::ui::widgets::{
    ribbon_button, ribbon_panel, ribbon_small, ribbon_stack, tip, vertical_divider,
};
use crate::app::{DocDialog, FloorPlanApp};
use crate::domain::discipline::Discipline;
use crate::domain::fixtures::FixtureKind;
use crate::domain::layers::LayerCategory;

impl FloorPlanApp {
    pub(super) fn structure_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Elementos", |ui| {
            self.fixture_button(ui, FixtureKind::Column, "Pilar");
            self.fixture_button(ui, FixtureKind::Footing, "Sapata");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Cargas", |ui| {
            self.fixture_button(ui, FixtureKind::WaterTank, "Caixa-d'água");
            self.fixture_button(ui, FixtureKind::HeavyEquipment, "Equipamento");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Vigas", |ui| {
            if tip(
                ribbon_button(ui, Icon::Glyph(Glyph::Join), "Ligar pilares", false, false),
                "Liga por vigas os pilares selecionados, na ordem em que foram escolhidos (Shift+clique), com altura de um décimo do vão",
            )
            .clicked()
            {
                self.connect_selected_columns();
            }
            if tip(
                ribbon_button(ui, Icon::Delete, "Remover vigas", false, false),
                "Remove as vigas entre os pilares selecionados (ou todas as de um pilar)",
            )
            .clicked()
            {
                self.remove_selected_beams();
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Projeto (NBR 6118)", |ui| {
            if tip(
                ribbon_button(ui, Icon::Glyph(Glyph::Rectangle), "Gerar estrutura", false, false),
                "Lança pilares nos encontros e nas pontas das paredes (e a cada 4,5 m nas longas, longe de portas e janelas), vigas sobre as paredes, lajes treliçadas nos cômodos e sapatas dimensionadas pelas cargas",
            )
            .clicked()
            {
                self.generate_structure();
            }
            let open = self.active_doc_dialog == DocDialog::Structure;
            if tip(
                ribbon_button(ui, Icon::Table, "Pré-dimensionar", open, false),
                "Lajes, vigas, pilares e sapatas com cargas, seções mínimas e alertas, e os materiais e cargas do cálculo",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Structure;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("trazo_structure_tab"), 0_u8));
            }
            if tip(
                ribbon_button(ui, Icon::SectionView, "Esforços", false, false),
                "Momento fletor, cortante, reações e flecha de cada viga contínua, para exportar em PDF ou SVG",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Structure;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("trazo_structure_tab"), 1_u8));
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Glyph(Glyph::Distribute),
                    "Ajustar sapatas",
                    false,
                    false,
                ),
                "Redimensiona as sapatas pela carga de cada pilar e pela tensão admissível do solo",
            )
            .clicked()
            {
                self.size_footings();
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::CirculationCheck,
                    "Segurança",
                    self.active_doc_dialog == DocDialog::Safety,
                    false,
                ),
                "Semáforo de todas as verificações: verde com folga, amarelo no limite, vermelho não atende, com o que mudar",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Safety;
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Exibição", |ui| {
            ribbon_stack(ui, |ui| {
                let only = self.discipline == Discipline::Structure;
                if tip(
                    ribbon_small(ui, Icon::Layers, "Só estrutura", only),
                    "Planta de forma: esmaece a arquitetura, oculta o mobiliário e a elétrica e destaca a estrutura",
                )
                .clicked()
                {
                    self.set_discipline(if only {
                        Discipline::All
                    } else {
                        Discipline::Structure
                    });
                }
                let visible = self.project.category_visible(LayerCategory::Structure);
                let (icon, label) = if visible {
                    (Icon::Eye, "Ocultar estrutura")
                } else {
                    (Icon::EyeOff, "Mostrar estrutura")
                };
                if tip(
                    ribbon_small(ui, icon, label, !visible),
                    "Mostra ou oculta a camada Estrutura",
                )
                .clicked()
                {
                    self.toggle_structure_layer();
                }
            });
        });
    }
}
