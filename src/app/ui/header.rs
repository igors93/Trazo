use super::super::{DocDialog, FloorPlanApp, RibbonTab, Tool, theme::*};
use super::Glyph;
use super::icons::{self, Icon};
use super::widgets::{
    MENU_WIDTH, menu_check, menu_item, menu_section_label, menu_separator, submenu, tip,
    truncated_galley,
};
use crate::domain::model::OpeningKind;
use eframe::egui::{self, Align, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Style, Ui, vec2};

fn menu_bar_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(9.0, 4.0);
    style.spacing.item_spacing.x = 3.0;
    style.visuals.override_text_color = Some(APP_BAR_TEXT);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, APP_BAR_TEXT);
    style.visuals.widgets.hovered.weak_bg_fill = APP_BAR_HOVER;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.open.weak_bg_fill = APP_BAR_ACTIVE;
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.weak_bg_fill = APP_BAR_ACTIVE;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
}

fn app_bar_icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, active: bool) -> egui::Response {
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    let painter = ui.painter();
    if active || (enabled && response.is_pointer_button_down_on()) {
        painter.rect_filled(rect, 5, APP_BAR_ACTIVE);
    } else if enabled && response.hovered() {
        painter.rect_filled(rect, 5, APP_BAR_HOVER);
    }
    let ink = if !enabled {
        Color32::from_rgb(0x61, 0x6E, 0x80)
    } else if active || response.hovered() {
        Color32::WHITE
    } else {
        APP_BAR_TEXT
    };
    let acc = if !enabled {
        Color32::from_rgb(0x61, 0x6E, 0x80)
    } else {
        ACCENT_RING
    };
    icons::paint(
        painter,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        icon,
        ink,
        acc,
    );
    let resp = tip(response, tooltip);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn app_bar_menu_button(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    ui.menu_button(title, |ui| {
        // Restore light popup text color inside dark app-bar menus.
        ui.visuals_mut().override_text_color = None;
        add_contents(ui);
    });
}

impl FloorPlanApp {
    pub(super) fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().style(menu_bar_style).ui(ui, |ui| {
            ui.set_min_height(28.0);

            // Brand / Start Page toggle button (inspired by BIMCraft/CreaBIM)
            if app_bar_icon_button(
                ui,
                Icon::Plan,
                if self.show_start_page {
                    "Voltar para a planta"
                } else {
                    "Página inicial (recentes e modelos)"
                },
                self.show_start_page,
            )
            .clicked()
            {
                self.show_start_page = !self.show_start_page;
            }
            separator(ui);

            // Quick access toolbar
            if app_bar_icon_button(ui, Icon::New, "Nova planta em nova aba (Ctrl+N)", false)
                .clicked()
            {
                self.request_new_project();
            }
            if app_bar_icon_button(ui, Icon::Open, "Abrir projeto ou modelo (Ctrl+O)", false)
                .clicked()
            {
                self.request_open();
            }
            if app_bar_icon_button(ui, Icon::Save, "Salvar projeto (Ctrl+S)", false).clicked() {
                self.save();
            }
            separator(ui);
            if ui
                .add_enabled_ui(self.history.can_undo(), |ui| {
                    app_bar_icon_button(ui, Icon::Undo, "Desfazer (Ctrl+Z)", false)
                })
                .inner
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled_ui(self.history.can_redo(), |ui| {
                    app_bar_icon_button(ui, Icon::Redo, "Refazer (Ctrl+Y)", false)
                })
                .inner
                .clicked()
            {
                self.redo();
            }
            separator(ui);

            app_bar_menu_button(ui, "Arquivo", |ui| self.file_menu(ui));
            app_bar_menu_button(ui, "Editar", |ui| self.edit_menu(ui));
            app_bar_menu_button(ui, "Desenhar", |ui| self.draw_menu(ui));
            app_bar_menu_button(ui, "Engenharia", |ui| self.engineering_menu(ui));
            app_bar_menu_button(ui, "Exibir", |ui| self.view_menu(ui));
            app_bar_menu_button(ui, "Ajuda", |ui| {
                ui.set_width(MENU_WIDTH);
                if menu_item(ui, Some(Icon::Search), "Buscar comandos...", "Ctrl+K").clicked() {
                    self.open_command_palette();
                    ui.close();
                }
                if menu_item(ui, Some(Icon::Keyboard), "Atalhos e guia rápido", "F1").clicked() {
                    self.show_help = true;
                    ui.close();
                }
                if menu_item(ui, Some(Icon::Ai), "Conectar uma IA (MCP)...", "").clicked() {
                    self.show_mcp_dialog = true;
                    ui.close();
                }
                menu_separator(ui);
                let crash_hint = if crate::io::crash_log::has_crash_log() {
                    "log"
                } else {
                    ""
                };
                if menu_item(ui, Some(Icon::Info), "Relatar problema...", crash_hint).clicked() {
                    self.show_report_dialog = true;
                    ui.close();
                }
                let update_hint = match self.update_status {
                    crate::io::updates::UpdateStatus::Available(_) => "nova!",
                    _ => "",
                };
                if menu_item(
                    ui,
                    Some(Icon::Update),
                    "Verificar atualizações...",
                    update_hint,
                )
                .clicked()
                {
                    self.show_updates_dialog = true;
                    if matches!(
                        self.update_status,
                        crate::io::updates::UpdateStatus::Idle
                            | crate::io::updates::UpdateStatus::Failed(_)
                    ) {
                        self.start_update_check(ui.ctx());
                    }
                    ui.close();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let export = export_button(ui);
                egui::Popup::menu(&export).show(|ui| {
                    ui.visuals_mut().override_text_color = None;
                    self.export_menu(ui);
                });
                ui.add_space(8.0);
                if search_pill_button(ui).clicked() {
                    self.open_command_palette();
                }
                ui.add_space(8.0);
                self.document_chip(ui);
            });
        });
    }

    fn document_chip(&mut self, ui: &mut Ui) {
        let avail = ui.available_width();
        if avail < 96.0 {
            return;
        }
        let max_chip_w = (avail - 8.0).clamp(96.0, 260.0);
        let name = if self.project.name.trim().is_empty() {
            "Sem nome"
        } else {
            self.project.name.trim()
        };
        let file = self.document_name();
        let combined = if file.is_empty() || file == name {
            name.to_owned()
        } else {
            format!("{name} · {file}")
        };
        let painter = ui.painter();
        let text_max_w = (max_chip_w - 44.0).max(40.0);
        let title = truncated_galley(painter, &combined, strong(12.0), APP_BAR_TEXT, text_max_w);
        let width = (30.0 + title.size().x + 20.0).min(max_chip_w);
        let (rect, response) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
        let painter = ui.painter();
        painter.rect(
            rect,
            12,
            APP_BAR_HOVER,
            Stroke::new(1.0, APP_BAR_BORDER),
            StrokeKind::Inside,
        );
        icons::paint(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + 14.0, rect.center().y),
                vec2(13.0, 13.0),
            ),
            Icon::Plan,
            APP_BAR_MUTED,
            ACCENT_RING,
        );
        painter.galley(
            Pos2::new(rect.left() + 26.0, rect.center().y - title.size().y / 2.0),
            title,
            APP_BAR_TEXT,
        );
        let (dot, tip_text) = if self.dirty {
            (WARNING, format!("{combined} — Alterações não salvas"))
        } else {
            (SUCCESS, format!("{combined} — Todas as alterações salvas"))
        };
        painter.circle_filled(Pos2::new(rect.right() - 10.0, rect.center().y), 3.5, dot);
        tip(response, &tip_text);
    }

    fn file_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if menu_item(ui, Some(Icon::Plan), "Página inicial", "").clicked() {
            self.show_start_page = true;
            ui.close();
        }
        if menu_item(ui, Some(Icon::New), "Nova planta", "Ctrl+N").clicked() {
            self.request_new_project();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Plan), "Novo a partir de modelo...", "").clicked() {
            self.active_doc_dialog = DocDialog::Templates;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Open), "Abrir...", "Ctrl+O").clicked() {
            self.request_open();
            ui.close();
        }
        menu_separator(ui);
        if menu_item(ui, Some(Icon::Save), "Salvar", "Ctrl+S").clicked() {
            self.save();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Save), "Salvar como modelo (.trazot)...", "").clicked() {
            self.save_as_template();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Print), "Imprimir em escala...", "Ctrl+P").clicked() {
            self.open_print_dialog();
            ui.close();
        }
        menu_separator(ui);
        submenu(ui, Icon::DuplicateTab, "Abas e janelas", |ui| {
            if menu_item(
                ui,
                Some(Icon::DuplicateTab),
                "Nova visão vinculada (mesmo projeto)",
                "",
            )
            .clicked()
            {
                let next = match self.discipline {
                    crate::domain::discipline::Discipline::Architecture
                    | crate::domain::discipline::Discipline::All => {
                        crate::domain::discipline::Discipline::Electrical
                    }
                    crate::domain::discipline::Discipline::Electrical => {
                        crate::domain::discipline::Discipline::Structure
                    }
                    _ => crate::domain::discipline::Discipline::Architecture,
                };
                self.new_linked_tab(next);
                ui.close();
            }
            if menu_item(
                ui,
                Some(Icon::DuplicateTab),
                "Duplicar como cópia independente",
                "",
            )
            .clicked()
            {
                self.duplicate_tab();
                ui.close();
            }
            if menu_item(ui, Some(Icon::SplitView), "Lado a lado", "").clicked() {
                let other = (0..self.tab_count()).find(|index| *index != self.active_tab());
                if self.split_tab.is_some() || other.is_none() {
                    self.toggle_side_by_side(None);
                } else {
                    self.toggle_side_by_side(other);
                }
                ui.close();
            }
            if menu_item(ui, Some(Icon::PanelRight), "Nova janela", "Ctrl+Shift+N").clicked() {
                self.open_new_window();
                ui.close();
            }
            menu_separator(ui);
            if menu_item(ui, Some(Icon::Close), "Fechar aba", "Ctrl+W").clicked() {
                self.request_close_tab(self.active_tab());
                ui.close();
            }
        });
        submenu(ui, Icon::Import, "Importar e referências", |ui| {
            if menu_item(ui, Some(Icon::Import), "Importar DXF (AutoCAD)...", "").clicked() {
                self.import_dxf();
                ui.close();
            }
            if menu_item(ui, Some(Icon::Import), "Importar DWG (AutoCAD)...", "").clicked() {
                self.import_dwg();
                ui.close();
            }
            menu_separator(ui);
            if menu_item(
                ui,
                Some(Icon::ExportImage),
                "Imagem / PDF de fundo (Underlay)...",
                "",
            )
            .clicked()
            {
                self.active_doc_dialog = DocDialog::Underlay;
                ui.close();
            }
            if menu_item(ui, Some(Icon::Layers), "Referências externas (XREF)...", "").clicked() {
                self.active_doc_dialog = DocDialog::Xref;
                ui.close();
            }
            if menu_item(ui, Some(Icon::SplitView), "Comparar versões...", "").clicked() {
                self.active_doc_dialog = DocDialog::Compare;
                ui.close();
            }
        });
        submenu(ui, Icon::Export, "Exportar", |ui| {
            self.export_menu(ui);
        });
        menu_separator(ui);
        if menu_item(ui, Some(Icon::Close), "Sair", "Alt+F4").clicked() {
            self.request_close(ui.ctx());
            ui.close();
        }
    }

    fn engineering_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        menu_section_label(ui, "Cálculo e verificação");
        if menu_item(
            ui,
            Some(Icon::SectionView),
            "Pré-dimensionamento estrutural...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::Structure;
            ui.close();
        }
        if menu_item(
            ui,
            Some(Icon::Bolt),
            "Projeto elétrico e quadro (NBR 5410)...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::Electrical;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Warning), "Semáforo de segurança...", "").clicked() {
            self.active_doc_dialog = DocDialog::Safety;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Table), "Quantitativos da obra...", "").clicked() {
            self.active_doc_dialog = DocDialog::Quantities;
            ui.close();
        }
        if menu_item(
            ui,
            Some(Icon::CirculationCheck),
            "Circulação e interferências...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::Ergonomics;
            ui.close();
        }
        menu_separator(ui);
        menu_section_label(ui, "Documentação ABNT");
        if menu_item(ui, Some(Icon::Layers), "Camadas (Layers)...", "").clicked() {
            self.active_doc_dialog = DocDialog::Layers;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Ruler), "Cotas associativas...", "").clicked() {
            self.active_doc_dialog = DocDialog::Dimensions;
            ui.close();
        }
        if menu_item(ui, Some(Icon::TextNote), "Textos, notas e símbolos...", "").clicked() {
            self.active_doc_dialog = DocDialog::Notes;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Table), "Quadros técnicos...", "").clicked() {
            self.active_doc_dialog = DocDialog::Schedules;
            ui.close();
        }
        if menu_item(
            ui,
            Some(Icon::SectionView),
            "Cortes, fachadas e detalhes...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::Views;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Plan), "Prancha técnica (A4–A0)...", "").clicked() {
            self.active_doc_dialog = DocDialog::Sheet;
            ui.close();
        }
        if menu_item(
            ui,
            Some(Icon::ExportPdf),
            "Conjunto de pranchas e revisões...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::SheetSet;
            ui.close();
        }
        menu_separator(ui);
        menu_section_label(ui, "Edifício e biblioteca");
        if menu_item(ui, Some(Icon::Floors), "Pavimentos e níveis...", "").clicked() {
            self.active_doc_dialog = DocDialog::Floors;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Stairs), "Escadas, rampas e desníveis...", "").clicked() {
            self.active_doc_dialog = DocDialog::Circulation;
            ui.close();
        }
        if menu_item(
            ui,
            Some(Icon::Fixture(crate::domain::fixtures::FixtureKind::Sofa)),
            "Biblioteca de móveis e blocos...",
            "",
        )
        .clicked()
        {
            self.active_doc_dialog = DocDialog::Library;
            ui.close();
        }
    }

    fn edit_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if ui
            .add_enabled_ui(self.history.can_undo(), |ui| {
                menu_item(ui, Some(Icon::Undo), "Desfazer", "Ctrl+Z")
            })
            .inner
            .clicked()
        {
            self.undo();
            ui.close();
        }
        if ui
            .add_enabled_ui(self.history.can_redo(), |ui| {
                menu_item(ui, Some(Icon::Redo), "Refazer", "Ctrl+Y")
            })
            .inner
            .clicked()
        {
            self.redo();
            ui.close();
        }
        menu_separator(ui);
        self.clipboard_menu_items(ui);
        menu_separator(ui);
        self.transform_menu_items(ui);
        submenu(ui, Icon::Glyph(Glyph::Group), "Organizar", |ui| {
            self.arrange_menu_items(ui);
        });
        submenu(ui, Icon::Glyph(Glyph::FixedDistance), "Restrições", |ui| {
            self.constraint_menu_items(ui);
        });
        menu_separator(ui);
        self.selection_menu_items(ui);
        menu_separator(ui);
        if ui
            .add_enabled_ui(self.selection.is_some(), |ui| {
                menu_item(ui, Some(Icon::Delete), "Excluir seleção", "Delete")
            })
            .inner
            .clicked()
        {
            self.delete_selection();
            ui.close();
        }
        if ui
            .add_enabled_ui(self.selection.is_some(), |ui| {
                menu_item(
                    ui,
                    Some(Icon::Glyph(Glyph::ClearSelection)),
                    "Limpar seleção",
                    "Esc",
                )
            })
            .inner
            .clicked()
        {
            self.clear_selection();
            self.after_selection_change();
            ui.close();
        }
    }

    fn view_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if menu_item(ui, Some(Icon::ZoomFit), "Enquadrar tudo", "F").clicked() {
            self.fit_view_requested();
            ui.close();
        }
        if menu_item(ui, Some(Icon::ZoomIn), "Ampliar", "Roda ↑").clicked() {
            self.zoom_by(1.25);
            ui.close();
        }
        if menu_item(ui, Some(Icon::ZoomOut), "Reduzir", "Roda ↓").clicked() {
            self.zoom_by(0.8);
            ui.close();
        }
        if ui
            .add_enabled_ui(self.selection.is_some(), |ui| {
                menu_item(ui, Some(Icon::Search), "Aproximar da seleção", "")
            })
            .inner
            .clicked()
            && let Some(selection) = self.selection
        {
            self.zoom_to_selection(selection);
            ui.close();
        }
        menu_separator(ui);
        if menu_check(ui, "Grade", "F7", self.show_grid).clicked() {
            self.toggle_grid();
        }
        if menu_check(ui, "Cotas (cm)", "", self.show_dimensions).clicked() {
            self.show_dimensions = !self.show_dimensions;
        }
        if menu_check(ui, "Encaixe magnético", "F3", self.snap_enabled).clicked() {
            self.toggle_snap();
        }
        submenu(ui, Icon::Snap, "Encaixes avançados (OSNAP)", |ui| {
            self.osnap_menu(ui);
        });
        if menu_check(ui, "Modo orto", "F8", self.ortho_enabled).clicked() {
            self.toggle_ortho();
        }
        menu_separator(ui);
        if menu_check(ui, "Navegador do projeto", "", self.show_browser).clicked() {
            self.show_browser = !self.show_browser;
        }
        if menu_check(ui, "Propriedades", "", self.show_properties).clicked() {
            self.show_properties = !self.show_properties;
        }
        if menu_check(ui, "Faixa de opções (Ribbon)", "", !self.ribbon_collapsed).clicked() {
            self.ribbon_collapsed = !self.ribbon_collapsed;
        }
        if menu_check(ui, "Modo leve (computador simples)", "", self.light_mode)
            .on_hover_text("Sem animações contínuas e com menos memória para desfazer, para notebooks com 4 GB de RAM.")
            .clicked()
        {
            self.set_light_mode(!self.light_mode);
        }
        menu_separator(ui);
        submenu(ui, Icon::Plan, "Disciplina ativa", |ui| {
            use crate::domain::discipline::Discipline;
            for d in Discipline::ALL {
                if menu_check(ui, d.label(), "", self.discipline == d).clicked() {
                    self.set_discipline(d);
                    ui.close();
                }
            }
        });
    }

    pub(super) fn osnap_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if menu_check(ui, "Extremidade (ponta)", "", self.osnap.endpoint).clicked() {
            self.osnap.endpoint = !self.osnap.endpoint;
        }
        if menu_check(ui, "Ponto médio", "", self.osnap.midpoint).clicked() {
            self.osnap.midpoint = !self.osnap.midpoint;
        }
        if menu_check(ui, "Interseção", "", self.osnap.intersection).clicked() {
            self.osnap.intersection = !self.osnap.intersection;
        }
        if menu_check(ui, "Centro", "", self.osnap.center).clicked() {
            self.osnap.center = !self.osnap.center;
        }
        if menu_check(ui, "Perpendicular", "", self.osnap.perpendicular).clicked() {
            self.osnap.perpendicular = !self.osnap.perpendicular;
        }
        if menu_check(ui, "Tangente", "", self.osnap.tangent).clicked() {
            self.osnap.tangent = !self.osnap.tangent;
        }
        if menu_check(ui, "Prolongamento", "", self.osnap.extension).clicked() {
            self.osnap.extension = !self.osnap.extension;
        }
        menu_separator(ui);
        let all_on = self.osnap == crate::domain::snap::OsnapSettings::default();
        if menu_item(
            ui,
            Some(if all_on { Icon::Close } else { Icon::Check }),
            if all_on {
                "Desativar todos"
            } else {
                "Ativar todos"
            },
            "",
        )
        .clicked()
        {
            self.osnap = if all_on {
                crate::domain::snap::OsnapSettings {
                    endpoint: false,
                    midpoint: false,
                    intersection: false,
                    center: false,
                    perpendicular: false,
                    tangent: false,
                    extension: false,
                }
            } else {
                crate::domain::snap::OsnapSettings::default()
            };
        }
    }

    fn draw_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        let entries = [
            (Icon::Select, "Selecionar", "V", Tool::Move),
            (Icon::Terrain, "Terreno", "T", Tool::Terrain),
            (Icon::Wall, "Parede", "P", Tool::Wall),
            (Icon::Room, "Cômodo", "C", Tool::Room),
            (Icon::RoomPolygon, "Cômodo livre", "L", Tool::RoomPolygon),
            (Icon::Door, "Porta", "R", Tool::Opening(OpeningKind::Door)),
            (
                Icon::Window,
                "Janela",
                "J",
                Tool::Opening(OpeningKind::Window),
            ),
            (
                Icon::SaloonDoor,
                "Porta vai-e-vem",
                "",
                Tool::Opening(OpeningKind::SaloonDoor),
            ),
            (Icon::Gap, "Vão livre", "", Tool::Opening(OpeningKind::Gap)),
            (Icon::Gate, "Portão", "", Tool::Opening(OpeningKind::Gate)),
            (Icon::Ruler, "Medir distâncias", "M", Tool::Measure),
        ];
        let drafting = crate::app::tools::DrawKind::ALL.map(|kind| {
            (
                Icon::Glyph(kind.glyph()),
                kind.label(),
                kind.key().1,
                Tool::Draw(kind),
            )
        });
        let openings = entries
            .iter()
            .position(|entry| matches!(entry.3, Tool::Opening(_)))
            .unwrap_or(entries.len());
        let count = entries.len();
        for (index, (icon, label, key, tool)) in entries.into_iter().chain(drafting).enumerate() {
            if index == 1 || index == openings || index == count {
                menu_separator(ui);
            }
            if menu_item(ui, Some(icon), label, key).clicked() {
                self.set_tool(tool);
                if tool == Tool::Terrain {
                    self.sync_terrain_fields();
                }
                self.ribbon_tab = if matches!(tool, Tool::Draw(_)) {
                    RibbonTab::Drafting
                } else {
                    RibbonTab::Architecture
                };
                ui.close();
            }
        }
    }

    fn export_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if menu_item(ui, Some(Icon::Print), "Imprimir em escala...", "Ctrl+P").clicked() {
            self.open_print_dialog();
            ui.close();
        }
        menu_separator(ui);
        if menu_item(ui, Some(Icon::ExportImage), "Imagem PNG...", "").clicked() {
            self.export_png();
            ui.close();
        }
        let multi_tab = self.project_tab_count(self.active_project_id) > 1;
        let pdf_label = if multi_tab {
            "Documento PDF (todas as abas)..."
        } else {
            "Documento PDF..."
        };
        if menu_item(ui, Some(Icon::ExportPdf), pdf_label, "").clicked() {
            self.export_pdf();
            ui.close();
        }
        if multi_tab
            && menu_item(
                ui,
                Some(Icon::ExportPdf),
                "Documento PDF (aba atual)...",
                "",
            )
            .clicked()
        {
            self.export_active_tab_pdf();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Desenho SVG editável...", "").clicked() {
            self.export_svg();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Desenho DXF (AutoCAD)...", "").clicked() {
            self.export_dxf();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Desenho DWG (AutoCAD)...", "").clicked() {
            self.export_dwg();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Table), "Quantitativos CSV...", "").clicked() {
            self.export_csv();
            ui.close();
        }
    }
}

fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(10.0, 20.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 2.0),
            Pos2::new(rect.center().x, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, APP_BAR_BORDER),
    );
}

/// Compact quick-command search pill (`Ctrl+K`) in the dark App Bar.
fn search_pill_button(ui: &mut Ui) -> egui::Response {
    let label = ui
        .painter()
        .layout_no_wrap("Buscar".to_owned(), regular(12.0), APP_BAR_TEXT);
    let hint = ui
        .painter()
        .layout_no_wrap("Ctrl+K".to_owned(), strong(10.0), APP_BAR_MUTED);
    let width = 26.0 + label.size().x + 8.0 + hint.size().x + 12.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    let painter = ui.painter();
    let fill = if response.is_pointer_button_down_on() || response.hovered() {
        APP_BAR_ACTIVE
    } else {
        APP_BAR_HOVER
    };
    painter.rect(
        rect,
        6,
        fill,
        Stroke::new(1.0, APP_BAR_BORDER),
        StrokeKind::Inside,
    );
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 13.0, rect.center().y),
            vec2(13.0, 13.0),
        ),
        Icon::Search,
        APP_BAR_MUTED,
        ACCENT_RING,
    );
    let label_x = rect.left() + 25.0;
    let label_w = label.size().x;
    painter.galley(
        Pos2::new(label_x, rect.center().y - label.size().y / 2.0),
        label,
        APP_BAR_TEXT,
    );
    painter.galley(
        Pos2::new(
            label_x + label_w + 8.0,
            rect.center().y - hint.size().y / 2.0,
        ),
        hint,
        APP_BAR_MUTED,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Buscar"));
    tip(
        response,
        "Buscar qualquer comando, ferramenta, cálculo ou cômodo (Ctrl+K)",
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The call to action at the right of the menu bar, with a crisp dropdown chevron.
fn export_button(ui: &mut Ui) -> egui::Response {
    let label = ui
        .painter()
        .layout_no_wrap("Exportar".to_owned(), strong(12.0), Color32::WHITE);
    let width = 28.0 + label.size().x + 22.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    let painter = ui.painter();
    let fill = if response.is_pointer_button_down_on() {
        ACCENT_STRONG
    } else if response.hovered() {
        ACCENT_HOVER
    } else {
        ACCENT
    };
    painter.rect_filled(rect, 6, fill);
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            vec2(14.0, 14.0),
        ),
        Icon::Export,
        Color32::WHITE,
        Color32::WHITE,
    );
    let label_x = rect.left() + 26.0;
    painter.galley(
        Pos2::new(label_x, rect.center().y - label.size().y / 2.0),
        label,
        Color32::WHITE,
    );
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.right() - 11.0, rect.center().y + 0.5),
            vec2(11.0, 11.0),
        ),
        Icon::ChevronDown,
        Color32::WHITE,
        Color32::WHITE,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Exportar"));
    tip(
        response,
        "Exportar a planta e seus dados em PNG, PDF, SVG, DXF ou CSV",
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
