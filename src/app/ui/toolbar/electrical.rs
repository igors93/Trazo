//! The "Elétrica" ribbon tab: electrical devices placed on the floor plan,
//! and the NBR 5410 design commands (minimum points, circuits, schedule).

use eframe::egui::Ui;

use crate::app::ui::icons::Icon;
use crate::app::ui::widgets::{
    ribbon_button, ribbon_panel, ribbon_small, ribbon_stack, tip, vertical_divider,
};
use crate::app::{DocDialog, FloorPlanApp};
use crate::domain::fixtures::FixtureKind;
use crate::domain::layers::LayerCategory;

impl FloorPlanApp {
    pub(super) fn electrical_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Tomadas", |ui| {
            self.fixture_button(ui, FixtureKind::OutletLow, "Baixa");
            self.fixture_button(ui, FixtureKind::OutletMid, "Média");
            self.fixture_button(ui, FixtureKind::OutletHigh, "Alta");
            self.fixture_button(ui, FixtureKind::OutletSpecific, "TUE");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Iluminação e comando", |ui| {
            self.fixture_button(ui, FixtureKind::CeilingLight, "Luz teto");
            self.fixture_button(ui, FixtureKind::WallLight, "Arandela");
            self.fixture_button(ui, FixtureKind::Switch, "Interruptor");
            self.fixture_button(ui, FixtureKind::SwitchTwoWay, "Paralelo");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Quadro e dados", |ui| {
            self.fixture_button(ui, FixtureKind::Panel, "Quadro");
            self.fixture_button(ui, FixtureKind::DataPoint, "Dados/TV");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Projeto (NBR 5410)", |ui| {
            if tip(
                ribbon_button(ui, Icon::Bolt, "Gerar pontos", false, false),
                "Adiciona o que a NBR 5410 exige e falta em cada cômodo: ponto de luz, tomadas distribuídas nas paredes, interruptor junto à porta e TUE no chuveiro",
            )
            .clicked()
            {
                self.generate_electrical_points();
            }
            if tip(
                ribbon_button(ui, Icon::Glyph(crate::app::ui::icons::Glyph::Distribute), "Circuitos", false, false),
                "Distribui os pontos em circuitos: iluminação separada das tomadas, cozinha e serviço em circuitos próprios e um circuito por TUE",
            )
            .clicked()
            {
                self.assign_electrical_circuits();
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Table,
                    "Quadro de cargas",
                    self.active_doc_dialog == DocDialog::Electrical,
                    false,
                ),
                "Previsão de cargas por cômodo, circuitos com corrente, fio e disjuntor, e alertas",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Electrical;
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Exibição", |ui| {
            ribbon_stack(ui, |ui| {
                // One toggle, so the group keeps the ribbon's three rows.
                let voltage = self.project.electrical.voltage_v;
                let label = format!("Tensão {voltage:.0} V");
                if tip(
                    ribbon_small(ui, Icon::Bolt, &label, false),
                    "Tensão fase-neutro dos circuitos de iluminação e tomadas. Clique para alternar entre 127 V e 220 V",
                )
                .clicked()
                {
                    self.set_electrical_voltage(if voltage < 170.0 { 220.0 } else { 127.0 });
                }
                let only = self.electrical_only;
                if tip(
                    ribbon_small(ui, Icon::Layers, "Só elétrica", only),
                    "Esmaece arquitetura e mobiliário para destacar a instalação elétrica",
                )
                .clicked()
                {
                    self.electrical_only = !only;
                }
                let visible = self.project.category_visible(LayerCategory::Electrical);
                let (vis_icon, vis_label) = if visible {
                    (Icon::Eye, "Ocultar elétrica")
                } else {
                    (Icon::EyeOff, "Mostrar elétrica")
                };
                if tip(
                    ribbon_small(ui, vis_icon, vis_label, !visible),
                    "Mostra ou oculta a camada Instalações elétricas",
                )
                .clicked()
                {
                    self.toggle_electrical_layer();
                }
            });
        });
    }
}
