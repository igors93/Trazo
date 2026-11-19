use super::super::{FloorPlanApp, RibbonTab, Selection, Tool, theme::*};
use super::icons::Icon;
use super::widgets::{
    icon_button_sized, ribbon_button, ribbon_panel, ribbon_small, ribbon_stack, tip,
    vertical_divider,
};
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::OpeningKind;
use eframe::egui::{self, Margin, PointerButton, Pos2, Sense, Stroke, Ui, vec2};

mod drafting;

const TABS: [(RibbonTab, &str); 5] = [
    (RibbonTab::Architecture, "Arquitetura"),
    (RibbonTab::Interiors, "Interiores"),
    (RibbonTab::Drafting, "Desenho"),
    (RibbonTab::View, "Vista"),
    (RibbonTab::Output, "Saída"),
];

impl FloorPlanApp {
    pub(super) fn ribbon(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        self.ribbon_tabs(ui);
        if self.ribbon_collapsed {
            let rect = ui.min_rect();
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0, BORDER),
            );
            return;
        }
        let body = egui::Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin {
                left: 8,
                right: 8,
                top: 4,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    match self.ribbon_tab {
                        RibbonTab::Architecture => self.architecture_tab(ui),
                        RibbonTab::Interiors => self.interiors_tab(ui),
                        RibbonTab::Drafting => self.drafting_tab(ui),
                        RibbonTab::View => self.view_tab(ui),
                        RibbonTab::Output => self.output_tab(ui),
                    }
                });
            });
        let rect = body.response.rect;
        let painter = ui.painter();
        for y in [rect.top(), rect.bottom()] {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0, BORDER),
            );
        }
    }

    fn ribbon_tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(27.0);
            ui.add_space(10.0);
            for (tab, label) in TABS {
                let active = self.ribbon_tab == tab;
                let font = if active { strong(13.0) } else { regular(13.0) };
                let color = if active { ACCENT_TEXT } else { TEXT_2 };
                let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
                let (rect, response) =
                    ui.allocate_exact_size(vec2(galley.size().x + 26.0, 27.0), Sense::click());
                let painter = ui.painter();
                if active && !self.ribbon_collapsed {
                    painter.rect_filled(
                        rect.shrink2(vec2(0.0, 2.0)),
                        egui::CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: 0,
                            se: 0,
                        },
                        PANEL_ALT,
                    );
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            Pos2::new(rect.left() + 8.0, rect.bottom() - 2.5),
                            Pos2::new(rect.right() - 8.0, rect.bottom()),
                        ),
                        1.0,
                        ACCENT,
                    );
                } else if response.hovered() {
                    painter.rect_filled(rect.shrink2(vec2(0.0, 3.0)), 6, ACCENT_SOFTER);
                }
                painter.galley(
                    Pos2::new(
                        rect.center().x - galley.size().x / 2.0,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    color,
                );
                let response = tip(
                    response.on_hover_cursor(egui::CursorIcon::PointingHand),
                    if active {
                        "Clique de novo para recolher ou expandir o ribbon"
                    } else {
                        label
                    },
                );
                if response.clicked() {
                    if active {
                        self.ribbon_collapsed = !self.ribbon_collapsed;
                    } else {
                        self.ribbon_tab = tab;
                        self.ribbon_collapsed = false;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let (icon, hint) = if self.ribbon_collapsed {
                    (Icon::ChevronDown, "Expandir o ribbon")
                } else {
                    (Icon::ChevronUp, "Recolher o ribbon para ganhar espaço")
                };
                if icon_button_sized(ui, icon, hint, false, 22.0, 13.0).clicked() {
                    self.ribbon_collapsed = !self.ribbon_collapsed;
                }
            });
        });
    }

    fn tool_button(&mut self, ui: &mut Ui, tool: Tool, icon: Icon, label: &str, hint: &str) {
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
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Construir", |ui| {
            self.tool_button(
                ui,
                Tool::Terrain,
                Icon::Terrain,
                "Terreno",
                "Criar ou dimensionar o terreno: arraste ou clique em dois cantos (T)",
            );
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
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Aberturas", |ui| {
            self.opening_tile(ui, OpeningKind::Door, "Porta", Icon::Door);
            self.opening_tile(ui, OpeningKind::Window, "Janela", Icon::Window);
            self.opening_tile(ui, OpeningKind::SaloonDoor, "Vai-e-vem", Icon::SaloonDoor);
            self.opening_tile(ui, OpeningKind::Gap, "Vão livre", Icon::Gap);
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Modificar", |ui| {
            ribbon_stack(ui, |ui| {
                if ui
                    .add_enabled_ui(self.history.can_undo(), |ui| {
                        tip(
                            ribbon_small(ui, Icon::Undo, "Desfazer", false),
                            "Desfazer (Ctrl+Z)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled_ui(self.history.can_redo(), |ui| {
                        tip(
                            ribbon_small(ui, Icon::Redo, "Refazer", false),
                            "Refazer (Ctrl+Y)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.redo();
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
            });
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Precisão", |ui| {
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(ui, Icon::Snap, "Encaixe", self.snap_enabled),
                    "Encaixe magnético na grade de 0,25 m e nas extremidades das paredes (F3)",
                )
                .clicked()
                {
                    self.toggle_snap();
                }
                if tip(
                    ribbon_small(ui, Icon::Ortho, "Orto", self.ortho_enabled),
                    "Restringe paredes à horizontal ou vertical (F8)",
                )
                .clicked()
                {
                    self.toggle_ortho();
                }
                if tip(
                    ribbon_small(ui, Icon::Grid, "Grade", self.show_grid),
                    "Mostrar ou ocultar a grade (F7)",
                )
                .clicked()
                {
                    self.toggle_grid();
                }
            });
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Planta", |ui| {
            let can_center = self.project.terrain.is_some()
                && !self.project.walls.is_empty()
                && self.project.content_fits_terrain();
            if ui
                .add_enabled_ui(can_center, |ui| {
                    tip(
                        ribbon_button(ui, Icon::Center, "Centralizar", false, false),
                        "Centralizar a planta dentro do terreno",
                    )
                })
                .inner
                .clicked()
            {
                self.center_content_on_terrain();
            }
            if tip(
                ribbon_button(ui, Icon::ZoomFit, "Enquadrar", false, false),
                "Ajustar toda a planta na janela (F)",
            )
            .clicked()
            {
                self.fit_view_requested();
            }
        });
    }

    fn fixture_button(&mut self, ui: &mut Ui, kind: FixtureKind, label: &str) {
        let (width, depth) = kind.default_size();
        let hint = format!(
            "{}: {width:.2} × {depth:.2} m. Clique na planta; perto de uma parede ele encosta nela.",
            kind.label()
        )
        .replace('.', ",");
        if tip(
            ribbon_button(
                ui,
                Icon::Fixture(kind),
                label,
                self.tool == Tool::Fixture(kind),
                false,
            ),
            &hint,
        )
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
            self.fixture_button(ui, FixtureKind::WoodStove, "F. a lenha");
            self.fixture_button(ui, FixtureKind::Grill, "Churrasq.");
            self.fixture_button(ui, FixtureKind::Fridge, "Geladeira");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Banheiro", |ui| {
            self.fixture_button(ui, FixtureKind::Washbasin, "Lavatório");
            self.fixture_button(ui, FixtureKind::Toilet, "Vaso");
            self.fixture_button(ui, FixtureKind::Shower, "Chuveiro");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Mobiliário", |ui| {
            self.fixture_button(ui, FixtureKind::Table, "Mesa");
            self.fixture_button(ui, FixtureKind::Shelves, "Prateleira");
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Ajustar", |ui| {
            ribbon_stack(ui, |ui| {
                let can_rotate = matches!(self.selection, Some(Selection::Fixture(_)))
                    || matches!(self.tool, Tool::Fixture(_));
                if ui
                    .add_enabled_ui(can_rotate, |ui| {
                        tip(
                            ribbon_small(ui, Icon::Redo, "Girar 90°", false),
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
        vertical_divider(ui, 72.0);
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
        });
        vertical_divider(ui, 72.0);
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
        vertical_divider(ui, 72.0);
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
        ribbon_panel(ui, "Inteligência artificial", |ui| {
            if tip(
                ribbon_button(ui, Icon::Ai, "Conectar IA", self.show_mcp_dialog, false),
                "Usar o Trazo pelo ChatGPT, Claude ou outro assistente (MCP)",
            )
            .clicked()
            {
                self.show_mcp_dialog = true;
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Projeto", |ui| {
            if tip(
                ribbon_button(ui, Icon::New, "Novo", false, false),
                "Nova planta (Ctrl+N)",
            )
            .clicked()
            {
                self.request_new_project();
            }
            if tip(
                ribbon_button(ui, Icon::Open, "Abrir", false, false),
                "Abrir projeto (Ctrl+O)",
            )
            .clicked()
            {
                self.request_open();
            }
            if tip(
                ribbon_button(ui, Icon::Save, "Salvar", false, false),
                "Salvar projeto (Ctrl+S)",
            )
            .clicked()
            {
                self.save();
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Exportar", |ui| {
            if tip(
                ribbon_button(ui, Icon::ExportImage, "Imagem PNG", false, false),
                "Exportar a planta como imagem PNG em alta resolução",
            )
            .clicked()
            {
                self.export_png();
            }
            if tip(
                ribbon_button(ui, Icon::ExportPdf, "Documento PDF", false, false),
                "Exportar a planta como PDF vetorial",
            )
            .clicked()
            {
                self.export_pdf();
            }
            if tip(
                ribbon_button(ui, Icon::Export, "SVG editável", false, false),
                "Exportar textos e formas como elementos SVG editáveis",
            )
            .clicked()
            {
                self.export_svg();
            }
            if tip(
                ribbon_button(ui, Icon::Export, "Planilha CSV", false, false),
                "Exportar áreas, esquadrias e quantitativos para planilha",
            )
            .clicked()
            {
                self.export_csv();
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
