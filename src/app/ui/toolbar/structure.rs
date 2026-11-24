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
            self.fixture_button(ui, FixtureKind::TimberPost, "Mourão");
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
                ribbon_button(ui, Icon::Glyph(Glyph::Join), "Vigas alavanca", false, false),
                "Liga cada pilar de divisa, cuja sapata fica fora do centro para não sair do lote, ao pilar interno mais próximo por uma viga alavanca, que centra a sapata e descarrega o momento",
            )
            .clicked()
            {
                self.generate_balance_beams();
            }
            if tip(
                ribbon_button(ui, Icon::Glyph(Glyph::Line), "Baldrames", false, false),
                "Fundação de todas as paredes do térreo: viga baldrame sobre as sapatas onde a parede vai de pilar a pilar, sapata corrida nas demais (como as paredes internas dos banheiros)",
            )
            .clicked()
            {
                self.generate_wall_foundations();
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
        ribbon_panel(ui, "Simulação na planta", |ui| {
            if tip(
                ribbon_button(ui, Icon::SplitView, "E se?", false, false),
                "Simulador interativo (What-if): controles deslizantes de vão, seção, carga e material com faixa segura em tempo real",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Structure;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("trazo_structure_tab"), 7_u8));
            }
            if tip(
                ribbon_button(ui, Icon::Grid, "Grelha", false, false),
                "Analogia de grelha acoplada (lajes + vigas, incluindo lajes em L e irregulares) com mapa de flechas e momentos",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Structure;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("trazo_structure_tab"), 8_u8));
            }
            ribbon_stack(ui, |ui| {
                let util_on = self.project.structure.overlay.utilisation_colors;
                if tip(
                    ribbon_small(ui, Icon::CirculationCheck, "Cores semáforo", util_on),
                    "Colore lajes, vigas, pilares e sapatas pelo aproveitamento (verde ≤ 85 %, amarelo ≤ 100 %, vermelho > 100 %) na tela e no papel",
                )
                .clicked()
                {
                    let prev = self.project.clone();
                    self.project.structure.overlay.utilisation_colors = !util_on;
                    self.record_change(prev);
                }
                let diag = self.project.structure.overlay.beam_diagrams;
                let diag_label = match diag {
                    crate::domain::plan_results::BeamDiagramOverlay::None => "Diagramas (M/V)",
                    crate::domain::plan_results::BeamDiagramOverlay::Moment => "Momento (M)",
                    crate::domain::plan_results::BeamDiagramOverlay::Shear => "Cortante (V)",
                };
                if tip(
                    ribbon_small(
                        ui,
                        Icon::SectionView,
                        diag_label,
                        diag != crate::domain::plan_results::BeamDiagramOverlay::None,
                    ),
                    "Alterna diagramas sobre as vigas na planta e no papel: Desligado → Momento fletor (M) → Cortante (V)",
                )
                .clicked()
                {
                    let prev = self.project.clone();
                    self.project.structure.overlay.beam_diagrams = match diag {
                        crate::domain::plan_results::BeamDiagramOverlay::None => {
                            crate::domain::plan_results::BeamDiagramOverlay::Moment
                        }
                        crate::domain::plan_results::BeamDiagramOverlay::Moment => {
                            crate::domain::plan_results::BeamDiagramOverlay::Shear
                        }
                        crate::domain::plan_results::BeamDiagramOverlay::Shear => {
                            crate::domain::plan_results::BeamDiagramOverlay::None
                        }
                    };
                    self.record_change(prev);
                }
            });
            ribbon_stack(ui, |ui| {
                let defl_on = self.project.structure.overlay.deformed_shape;
                if tip(
                    ribbon_small(ui, Icon::Glyph(Glyph::Spline), "Deformada", defl_on),
                    "Exibe a linha elástica exagerada sobre as vigas na planta e no papel com flecha máxima (L/xxx)",
                )
                .clicked()
                {
                    let prev = self.project.clone();
                    self.project.structure.overlay.deformed_shape = !defl_on;
                    self.record_change(prev);
                }
                let path_on = self.project.structure.overlay.load_path;
                if tip(
                    ribbon_small(ui, Icon::Glyph(Glyph::Move), "Caminho cargas", path_on),
                    "Mostra o fluxo de cargas da laje → viga → pilar → sapata/solo (animado na tela e com setas no papel)",
                )
                .clicked()
                {
                    let prev = self.project.clone();
                    self.project.structure.overlay.load_path = !path_on;
                    self.record_change(prev);
                }
            });
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
