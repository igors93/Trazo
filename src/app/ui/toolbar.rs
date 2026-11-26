use super::super::{FloorPlanApp, RibbonTab, Selection, Tool, theme::*};
use super::icons::Icon;
use super::widgets::{
    RIBBON_PANEL_HEIGHT, icon_button_sized, ribbon_button, ribbon_panel, ribbon_small,
    ribbon_stack, tip, vertical_divider,
};
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::OpeningKind;
use eframe::egui::{self, Margin, PointerButton, Pos2, Sense, Stroke, Ui, vec2};

mod drafting;
mod edit;
mod electrical;
mod structure;

const TABS: [(RibbonTab, &str); 9] = [
    (RibbonTab::Architecture, "Arquitetura"),
    (RibbonTab::Edit, "Editar"),
    (RibbonTab::Interiors, "Interiores"),
    (RibbonTab::Electrical, "Elétrica"),
    (RibbonTab::Structure, "Estrutura"),
    (RibbonTab::Drafting, "Desenho"),
    (RibbonTab::Markups, "Marcações"),
    (RibbonTab::View, "Vista"),
    (RibbonTab::Output, "Saída"),
];

impl FloorPlanApp {
    pub(super) fn ribbon(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let active_tab_span = self.ribbon_tabs(ui);
        if self.ribbon_collapsed {
            let rect = ui.min_rect();
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0, BORDER),
            );
            return;
        }
        let body_h = ui.available_height().max(RIBBON_PANEL_HEIGHT + 8.0);
        let body = egui::Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin {
                left: 10,
                right: 10,
                top: 5,
                bottom: 2,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height((body_h - 7.0).max(RIBBON_PANEL_HEIGHT));
                ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
                egui::ScrollArea::horizontal()
                    .id_salt("ribbon_body")
                    .scroll_source(egui::scroll_area::ScrollSource {
                        scroll_bar: true,
                        mouse_wheel: true,
                        ..egui::scroll_area::ScrollSource::NONE
                    })
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
                            match self.ribbon_tab {
                                RibbonTab::Architecture => self.architecture_tab(ui),
                                RibbonTab::Interiors => self.interiors_tab(ui),
                                RibbonTab::Electrical => self.electrical_tab(ui),
                                RibbonTab::Structure => self.structure_tab(ui),
                                RibbonTab::Edit => self.edit_tab(ui),
                                RibbonTab::Drafting => self.drafting_tab(ui),
                                RibbonTab::Markups => self.markups_tab(ui),
                                RibbonTab::View => self.view_tab(ui),
                                RibbonTab::Output => self.output_tab(ui),
                            }
                        });
                    });
            });
        let rect = body.response.rect;
        let painter = ui.painter();
        // Top hairline breaks cleanly under the active tab so tab and body visually merge.
        if let Some((tab_l, tab_r)) = active_tab_span {
            if tab_l > rect.left() {
                painter.line_segment(
                    [
                        Pos2::new(rect.left(), rect.top()),
                        Pos2::new(tab_l, rect.top()),
                    ],
                    Stroke::new(1.0, BORDER),
                );
            }
            if tab_r < rect.right() {
                painter.line_segment(
                    [
                        Pos2::new(tab_r, rect.top()),
                        Pos2::new(rect.right(), rect.top()),
                    ],
                    Stroke::new(1.0, BORDER),
                );
            }
        } else {
            painter.line_segment(
                [
                    Pos2::new(rect.left(), rect.top()),
                    Pos2::new(rect.right(), rect.top()),
                ],
                Stroke::new(1.0, BORDER),
            );
        }
        painter.line_segment(
            [
                Pos2::new(rect.left(), rect.bottom()),
                Pos2::new(rect.right(), rect.bottom()),
            ],
            Stroke::new(1.0, BORDER),
        );
    }

    fn ribbon_tabs(&mut self, ui: &mut Ui) -> Option<(f32, f32)> {
        let mut active_span = None;
        ui.horizontal(|ui| {
            ui.set_min_height(27.0);
            ui.add_space(10.0);
            for (tab, label) in TABS {
                let active = self.ribbon_tab == tab;
                let font = if active { strong(12.5) } else { regular(12.5) };
                let color = if active { ACCENT_TEXT } else { TEXT_2 };
                let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
                let (rect, response) =
                    ui.allocate_exact_size(vec2(galley.size().x + 26.0, 27.0), Sense::click());
                let painter = ui.painter();
                if active && !self.ribbon_collapsed {
                    let tab_rect = egui::Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top() + 2.0),
                        rect.right_bottom(),
                    );
                    painter.rect_filled(
                        tab_rect,
                        egui::CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: 0,
                            se: 0,
                        },
                        PANEL_ALT,
                    );
                    // Subtle left/top/right hairline + crisp top accent bar
                    painter.line_segment(
                        [
                            Pos2::new(tab_rect.left(), tab_rect.top() + 3.0),
                            tab_rect.left_bottom(),
                        ],
                        Stroke::new(1.0, BORDER),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(tab_rect.right(), tab_rect.top() + 3.0),
                            tab_rect.right_bottom(),
                        ],
                        Stroke::new(1.0, BORDER),
                    );
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            Pos2::new(tab_rect.left() + 6.0, tab_rect.top()),
                            Pos2::new(tab_rect.right() - 6.0, tab_rect.top() + 2.5),
                        ),
                        1.0,
                        ACCENT,
                    );
                    active_span = Some((tab_rect.left(), tab_rect.right()));
                } else if response.hovered() {
                    painter.rect_filled(rect.shrink2(vec2(1.0, 3.0)), 6, ACCENT_SOFTER);
                }
                painter.galley(
                    Pos2::new(
                        rect.center().x - galley.size().x / 2.0,
                        rect.center().y - galley.size().y / 2.0 + 1.0,
                    ),
                    galley,
                    color,
                );
                let response = tip(
                    response.on_hover_cursor(egui::CursorIcon::PointingHand),
                    if active {
                        "Clique de novo para recolher ou expandir a faixa de opções"
                    } else {
                        label
                    },
                );
                if response.clicked() {
                    if active {
                        self.ribbon_collapsed = !self.ribbon_collapsed;
                    } else {
                        if tab == RibbonTab::Markups {
                            self.set_tool(Tool::Markup(None));
                        } else if matches!(self.tool, Tool::Markup(_)) {
                            self.set_tool(Tool::Move);
                        }
                        self.ribbon_tab = tab;
                        self.ribbon_collapsed = false;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let (icon, hint) = if self.ribbon_collapsed {
                    (Icon::ChevronDown, "Expandir a faixa de opções")
                } else {
                    (
                        Icon::ChevronUp,
                        "Recolher a faixa de opções para ganhar espaço",
                    )
                };
                if icon_button_sized(ui, icon, hint, false, 22.0, 13.0).clicked() {
                    self.ribbon_collapsed = !self.ribbon_collapsed;
                }
            });
        });
        active_span
    }

    pub(super) fn tool_button(
        &mut self,
        ui: &mut Ui,
        tool: Tool,
        icon: Icon,
        label: &str,
        hint: &str,
    ) {
        let response = tip(
            ribbon_button(ui, icon, label, self.tool == tool, false),
            hint,
        );
        if response.clicked() {
            self.set_tool(tool);
            if tool == Tool::Terrain {
                self.sync_terrain_fields();
            }
        }
    }

    fn architecture_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Seleção", |ui| {
            self.tool_button(
                ui,
                Tool::Move,
                Icon::Select,
                "Selecionar",
                "Selecionar e mover elementos da planta (V)",
            );
            self.tool_button(
                ui,
                Tool::Measure,
                Icon::Ruler,
                "Medir",
                "Régua: mede entre dois pontos sem alterar nada (M)",
            );
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Construir", |ui| {
            self.tool_button(
                ui,
                Tool::Wall,
                Icon::Wall,
                "Parede",
                "Desenhar parede: arraste ou clique em dois pontos (P)",
            );
            self.tool_button(
                ui,
                Tool::Room,
                Icon::Room,
                "Cômodo",
                "Criar cômodo retangular: arraste ou clique em dois cantos (C)",
            );
            self.tool_button(
                ui,
                Tool::RoomPolygon,
                Icon::RoomPolygon,
                "Cômodo livre",
                "Cômodo em L, U ou de qualquer forma: clique em cada canto e feche no primeiro (L)",
            );
            self.tool_button(
                ui,
                Tool::Terrain,
                Icon::Terrain,
                "Terreno",
                "Criar ou dimensionar o terreno: arraste ou clique em dois cantos (T)",
            );
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Stairs,
                        "Escadas",
                        self.active_doc_dialog == super::super::DocDialog::Circulation,
                    ),
                    "Escadas, rampas NBR 9050, patamares, guarda-corpos e desníveis",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Circulation;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Floors,
                        "Pavimentos",
                        self.active_doc_dialog == super::super::DocDialog::Floors,
                    ),
                    "Gerenciar pavimentos (subsolo, térreo, superior), níveis, pé-direito e underlay",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Floors;
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Aberturas", |ui| {
            self.opening_tile(ui, OpeningKind::Door, "Porta", Icon::Door);
            self.opening_tile(ui, OpeningKind::Window, "Janela", Icon::Window);
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Gate,
                        "Portão",
                        self.tool == Tool::Opening(OpeningKind::Gate),
                    ),
                    "Inserir portão: clique numa parede para colocar",
                )
                .clicked()
                {
                    self.set_tool(Tool::Opening(OpeningKind::Gate));
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Gap,
                        "Vão livre",
                        self.tool == Tool::Opening(OpeningKind::Gap),
                    ),
                    "Inserir vão livre: clique numa parede para colocar",
                )
                .clicked()
                {
                    self.set_tool(Tool::Opening(OpeningKind::Gap));
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::SaloonDoor,
                        "Vai-e-vem",
                        self.tool == Tool::Opening(OpeningKind::SaloonDoor),
                    ),
                    "Inserir porta vai-e-vem: clique numa parede para colocar",
                )
                .clicked()
                {
                    self.set_tool(Tool::Opening(OpeningKind::SaloonDoor));
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Planta", |ui| {
            let can_center = self.project.terrain.is_some()
                && !self.project.walls.is_empty()
                && self.project.content_fits_terrain();
            ribbon_stack(ui, |ui| {
                if ui
                    .add_enabled_ui(can_center, |ui| {
                        tip(
                            ribbon_small(ui, Icon::Center, "Centralizar", false),
                            "Centralizar a planta dentro do terreno",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.center_content_on_terrain();
                }
                if tip(
                    ribbon_small(ui, Icon::Room, "Detectar cômodos", false),
                    "Criar ambientes automaticamente a partir de paredes fechadas",
                )
                .clicked()
                {
                    self.detect_enclosed_rooms();
                }
                if tip(
                    ribbon_small(ui, Icon::ZoomFit, "Enquadrar tudo", false),
                    "Ajustar toda a planta na janela (F)",
                )
                .clicked()
                {
                    self.fit_view_requested();
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Normas", |ui| {
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Ruler,
                        "Normas e Clima",
                        self.active_doc_dialog == super::super::DocDialog::Codes,
                    ),
                    "Motor de códigos de obra (Base → Emendas → Projeto), local/clima e verificação normativa",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Codes;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::CirculationCheck,
                        "Segurança",
                        self.active_doc_dialog == super::super::DocDialog::Safety,
                    ),
                    "Semáforo unificado de estrutura, elétrica e códigos de obra",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Safety;
                }
            });
        });
    }

    fn fixture_button(&mut self, ui: &mut Ui, kind: FixtureKind, label: &str) {
        let (width, depth) = kind.default_size();
        let dimensions = format!("{width:.2} × {depth:.2} m").replace('.', ",");
        let placement = if kind.mounts_on_wall() {
            "Perto de uma parede, encosta na face dela."
        } else {
            "Escolha o ponto de inserção; G gira o objeto."
        };
        let hint = format!(
            "{}: {dimensions}. Clique na planta. {placement}",
            kind.label()
        );
        if ribbon_button(
            ui,
            Icon::Fixture(kind),
            label,
            self.tool == Tool::Fixture(kind),
            false,
        )
        .on_hover_ui(|ui| {
            ui.strong(kind.label());
            let (rect, _) = ui.allocate_exact_size(vec2(180.0, 124.0), Sense::hover());
            super::icons::paint_fixture_preview(
                ui.painter(),
                rect,
                kind,
                (width, depth),
                TEXT_2,
                ACCENT,
            );
            ui.label(&hint);
            if let Some(seats) = kind.seat_count(width) {
                ui.label(format!("{seats} lugares no símbolo em planta"));
            }
        })
        .clicked()
        {
            self.set_tool(Tool::Fixture(kind));
        }
    }

    fn interiors_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Cozinha e churrasco", |ui| {
            self.fixture_button(ui, FixtureKind::Counter, "Bancada");
            self.fixture_button(ui, FixtureKind::KitchenSink, "Pia");
            self.fixture_button(ui, FixtureKind::Stove, "Fogão");
            self.fixture_button(ui, FixtureKind::WoodStove, "Fogão a lenha");
            self.fixture_button(ui, FixtureKind::Grill, "Churrasqueira");
            self.fixture_button(ui, FixtureKind::Fridge, "Geladeira");
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Banheiro", |ui| {
            self.fixture_button(ui, FixtureKind::Washbasin, "Lavatório");
            self.fixture_button(ui, FixtureKind::Toilet, "Vaso");
            self.fixture_button(ui, FixtureKind::Shower, "Chuveiro");
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Mobiliário", |ui| {
            self.fixture_button(ui, FixtureKind::Sofa, "Sofá");
            self.fixture_button(ui, FixtureKind::BedDouble, "Cama");
            self.fixture_button(ui, FixtureKind::Wardrobe, "Armário");
            self.fixture_button(ui, FixtureKind::Table, "Mesa");
            self.fixture_button(ui, FixtureKind::Desk, "Escrivaninha");
            self.fixture_button(ui, FixtureKind::Washer, "Lavadora");
            self.fixture_button(ui, FixtureKind::Shelves, "Prateleira");
            if tip(
                ribbon_button(
                    ui,
                    Icon::Search,
                    "Biblioteca",
                    self.active_doc_dialog == super::super::DocDialog::Library,
                    false,
                ),
                "Abrir biblioteca completa e personalizável de móveis, equipamentos e símbolos",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Library;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::CirculationCheck,
                    "Circulação",
                    self.show_ergonomics
                        || self.active_doc_dialog == super::super::DocDialog::Ergonomics,
                    false,
                ),
                "Verificar circulação, interferências, raio de portas e áreas de uso do mobiliário",
            )
            .clicked()
            {
                self.show_ergonomics = true;
                self.active_doc_dialog = super::super::DocDialog::Ergonomics;
            }
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Garagem e lazer", |ui| {
            self.fixture_button(ui, FixtureKind::Car, "Carro");
            self.fixture_button(ui, FixtureKind::Umbrella, "Guarda-sol");
            self.fixture_button(ui, FixtureKind::SunLounger, "Cadeira de sol");
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Ajustar", |ui| {
            ribbon_stack(ui, |ui| {
                let can_rotate = matches!(self.selection, Some(Selection::Fixture(_)))
                    || matches!(self.tool, Tool::Fixture(_));
                if ui
                    .add_enabled_ui(can_rotate, |ui| {
                        tip(
                            ribbon_small(
                                ui,
                                Icon::Glyph(super::Glyph::RotateCw),
                                "Girar 90°",
                                false,
                            ),
                            "Gira o objeto selecionado, ou o próximo a ser colocado (G)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.rotate_fixture_or_next();
                }
                if ui
                    .add_enabled_ui(self.selection.is_some(), |ui| {
                        tip(
                            ribbon_small(ui, Icon::Delete, "Excluir", false),
                            "Excluir o elemento selecionado (Delete)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.delete_selection();
                }
                if tip(
                    ribbon_small(ui, Icon::Select, "Selecionar", self.tool == Tool::Move),
                    "Voltar à seleção (V)",
                )
                .clicked()
                {
                    self.set_tool(Tool::Move);
                }
            });
        });
    }

    fn view_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Navegar", |ui| {
            if tip(
                ribbon_button(ui, Icon::ZoomFit, "Enquadrar", false, false),
                "Ajustar toda a planta na janela (F)",
            )
            .clicked()
            {
                self.fit_view_requested();
            }
            if tip(
                ribbon_button(ui, Icon::ZoomIn, "Ampliar", false, false),
                "Aproximar a vista",
            )
            .clicked()
            {
                self.zoom_by(1.25);
            }
            if tip(
                ribbon_button(ui, Icon::ZoomOut, "Reduzir", false, false),
                "Afastar a vista",
            )
            .clicked()
            {
                self.zoom_by(0.8);
            }
            let selection = self.selection;
            if ui
                .add_enabled_ui(selection.is_some(), |ui| {
                    tip(
                        ribbon_button(ui, Icon::Search, "Seleção", false, false),
                        "Aproximar do elemento selecionado",
                    )
                })
                .inner
                .clicked()
                && let Some(selection) = selection
            {
                self.zoom_to_selection(selection);
            }
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Exibição", |ui| {
            if tip(
                ribbon_button(ui, Icon::Grid, "Grade", self.show_grid, false),
                "Mostrar ou ocultar a grade (F7)",
            )
            .clicked()
            {
                self.toggle_grid();
            }
            if tip(
                ribbon_button(ui, Icon::Ruler, "Cotas", self.show_dimensions, false),
                "Mostrar as cotas em cm em volta da construção (faces das paredes e total)",
            )
            .clicked()
            {
                self.show_dimensions = !self.show_dimensions;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Layers,
                    "Camadas",
                    self.active_doc_dialog == super::super::DocDialog::Layers,
                    false,
                ),
                "Gerenciar camadas (cor, espessura, tipo de linha, visibilidade e bloqueio)",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Layers;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::ExportImage,
                    "Fundo",
                    self.active_doc_dialog == super::super::DocDialog::Underlay,
                    false,
                ),
                "Importar imagem ou PDF de fundo com calibração por 2 pontos, opacidade e bloqueio",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Underlay;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Layers,
                    "XREF",
                    self.active_doc_dialog == super::super::DocDialog::Xref,
                    false,
                ),
                "Anexar referências externas (.trazo / .dxf) com atualização automática ao vivo",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Xref;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::SplitView,
                    "Comparar",
                    self.active_doc_dialog == super::super::DocDialog::Compare
                        || self.compare_state.overlay_active,
                    false,
                ),
                "Comparar visualmente duas versões do projeto (adições, remoções e alterações)",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Compare;
            }
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Janelas", |ui| {
            if tip(
                ribbon_button(ui, Icon::PanelLeft, "Navegador", self.show_browser, false),
                "Mostrar ou ocultar o navegador do projeto",
            )
            .clicked()
            {
                self.show_browser = !self.show_browser;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::PanelRight,
                    "Propriedades",
                    self.show_properties,
                    false,
                ),
                "Mostrar ou ocultar o painel de propriedades",
            )
            .clicked()
            {
                self.show_properties = !self.show_properties;
            }
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Ajuda", |ui| {
            if tip(
                ribbon_button(ui, Icon::Keyboard, "Atalhos", self.show_help, false),
                "Atalhos de teclado e guia rápido (F1)",
            )
            .clicked()
            {
                self.show_help = true;
            }
        });
    }

    fn output_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Exportar", |ui| {
            let multi_tab = self.project_tab_count(self.active_project_id) > 1;
            let pdf_tip = if multi_tab {
                "Exportar todas as abas abertas deste projeto em um único PDF"
            } else {
                "Exportar a planta da aba atual como PDF vetorial"
            };
            if tip(
                ribbon_button(ui, Icon::ExportPdf, "PDF", false, false),
                pdf_tip,
            )
            .clicked()
            {
                self.export_pdf();
            }
            if tip(
                ribbon_button(ui, Icon::ExportImage, "PNG", false, false),
                "Exportar a planta como imagem PNG em alta resolução",
            )
            .clicked()
            {
                self.export_png();
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Print,
                    "Imprimir",
                    self.active_doc_dialog == super::super::DocDialog::Print,
                    false,
                ),
                "Imprimir prancha em escala exata com pré-visualização (Ctrl+P)",
            )
            .clicked()
            {
                self.open_print_dialog();
            }
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(ui, Icon::Export, "SVG editável", false),
                    "Exportar textos e formas como elementos SVG editáveis",
                )
                .clicked()
                {
                    self.export_svg();
                }
                if tip(
                    ribbon_small(ui, Icon::Export, "AutoCAD DXF", false),
                    "Exportar a planta em formato AutoCAD DXF",
                )
                .clicked()
                {
                    self.export_dxf();
                }
                if tip(
                    ribbon_small(ui, Icon::Export, "AutoCAD DWG", false),
                    "Exportar a planta em formato AutoCAD DWG",
                )
                .clicked()
                {
                    self.export_dwg();
                }
            });
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(ui, Icon::Table, "Planilha CSV", false),
                    "Exportar áreas, esquadrias e quantitativos para planilha CSV",
                )
                .clicked()
                {
                    self.export_csv();
                }
                if multi_tab
                    && tip(
                        ribbon_small(ui, Icon::ExportPdf, "PDF aba atual", false),
                        "Exportar somente a aba ativa como PDF vetorial",
                    )
                    .clicked()
                {
                    self.export_active_tab_pdf();
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Documentação ABNT", |ui| {
            if tip(
                ribbon_button(
                    ui,
                    Icon::Plan,
                    "Prancha",
                    self.active_doc_dialog == super::super::DocDialog::Sheet,
                    false,
                ),
                "Configurar prancha (A4–A0), escala exata e carimbo",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Sheet;
            }
            if tip(
                ribbon_button(
                    ui,
                    Icon::Table,
                    "Quantitativos",
                    self.active_doc_dialog == super::super::DocDialog::Quantities,
                    false,
                ),
                "Blocos, meios-blocos, canaletas, vergas, argamassa, reboco e acabamentos por ambiente",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Quantities;
            }
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(
                        ui,
                        Icon::SectionView,
                        "Cortes e fachadas",
                        self.active_doc_dialog == super::super::DocDialog::Views,
                    ),
                    "Criar e exportar cortes, fachadas e chamadas de detalhe",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Views;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Table,
                        "Quadros técnicos",
                        self.active_doc_dialog == super::super::DocDialog::Schedules,
                    ),
                    "Configurar quadros de áreas, esquadrias e ambientes",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Schedules;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::ExportPdf,
                        "Conjunto de pranchas",
                        self.active_doc_dialog == super::super::DocDialog::SheetSet,
                    ),
                    "Gerenciar conjunto de pranchas, paginação e revisões",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::SheetSet;
                }
            });
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Ruler,
                        "Cotas associativas",
                        self.active_doc_dialog == super::super::DocDialog::Dimensions,
                    ),
                    "Gerenciar cotas associativas na planta",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Dimensions;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::TextNote,
                        "Notas e símbolos",
                        self.active_doc_dialog == super::super::DocDialog::Notes,
                    ),
                    "Inserir textos, chamadas e símbolos técnicos",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Notes;
                }
                if tip(
                    ribbon_small(
                        ui,
                        Icon::Layers,
                        "Camadas",
                        self.active_doc_dialog == super::super::DocDialog::Layers,
                    ),
                    "Gerenciar camadas CAD, cores e penas",
                )
                .clicked()
                {
                    self.active_doc_dialog = super::super::DocDialog::Layers;
                }
            });
        });
        vertical_divider(ui, RIBBON_PANEL_HEIGHT);
        ribbon_panel(ui, "Modelos e IA", |ui| {
            if tip(
                ribbon_button(
                    ui,
                    Icon::Plan,
                    "Modelos",
                    self.active_doc_dialog == super::super::DocDialog::Templates,
                    false,
                ),
                "Criar projeto a partir de modelos ou exemplos prontos",
            )
            .clicked()
            {
                self.active_doc_dialog = super::super::DocDialog::Templates;
            }
            if tip(
                ribbon_button(ui, Icon::Ai, "Conectar IA", self.show_mcp_dialog, false),
                "Usar o Trazo pelo ChatGPT, Claude ou outro assistente (MCP)",
            )
            .clicked()
            {
                self.show_mcp_dialog = true;
            }
        });
    }

    fn opening_tile(&mut self, ui: &mut Ui, kind: OpeningKind, label: &str, icon: Icon) {
        let tooltip = match kind {
            OpeningKind::Door => "Inserir porta: clique numa parede ou arraste até ela (R)",
            OpeningKind::Window => "Inserir janela: clique numa parede ou arraste até ela (J)",
            OpeningKind::SaloonDoor => {
                "Inserir porta vai-e-vem (tipo faroeste): duas folhas que abrem para os dois lados"
            }
            OpeningKind::Gap => {
                "Abrir um vão livre (sem porta), como o lado aberto de uma área coberta; até 30 m"
            }
            OpeningKind::Gate => {
                "Inserir portão de entrada num muro ou parede: de correr por padrão, de 0,80 m (social) a 8 m (garagem)"
            }
        };
        let response = tip(
            ribbon_button(ui, icon, label, self.tool == Tool::Opening(kind), true),
            tooltip,
        );
        let pressed_on_tile = ui.input(|input| {
            input.events.iter().any(|event| {
                matches!(event, egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if response.rect.contains(*pos))
            })
        });
        if response.clicked() {
            self.set_tool(Tool::Opening(kind));
        }
        if pressed_on_tile
            || response.is_pointer_button_down_on()
            || response.drag_started()
            || response.dragged()
            || response.drag_stopped()
        {
            self.opening_drag = Some(kind);
        }
    }
}
