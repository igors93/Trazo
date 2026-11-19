use super::super::{FloorPlanApp, RibbonTab, Tool, theme::*};
use super::icons::{self, Icon};
use super::widgets::{MENU_WIDTH, icon_button, menu_check, menu_item, menu_separator, tip};
use crate::domain::model::OpeningKind;
use eframe::egui::{self, Align, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Style, Ui, vec2};

fn menu_bar_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(9.0, 5.0);
    style.spacing.item_spacing.x = 2.0;
    style.visuals.widgets.hovered.weak_bg_fill = ACCENT_SOFTER;
    style.visuals.widgets.open.weak_bg_fill = ACCENT_SOFT;
    style.visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
}

impl FloorPlanApp {
    pub(super) fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().style(menu_bar_style).ui(ui, |ui| {
            ui.set_min_height(28.0);
            let (logo, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            icons::paint_logo(ui.painter(), logo);
            ui.add_space(4.0);
            ui.label(RichText::new("Trazo").font(strong(14.5)).color(TEXT));
            ui.add_space(10.0);

            ui.menu_button("Arquivo", |ui| self.file_menu(ui));
            ui.menu_button("Editar", |ui| self.edit_menu(ui));
            ui.menu_button("Exibir", |ui| self.view_menu(ui));
            ui.menu_button("Desenhar", |ui| self.draw_menu(ui));
            ui.menu_button("Ajuda", |ui| {
                ui.set_width(MENU_WIDTH);
                if menu_item(ui, Some(Icon::Keyboard), "Atalhos e guia rápido", "F1").clicked() {
                    self.show_help = true;
                    ui.close();
                }
                if menu_item(ui, Some(Icon::Ai), "Conectar uma IA (MCP)...", "").clicked() {
                    self.show_mcp_dialog = true;
                    ui.close();
                }
            });

            ui.add_space(6.0);
            separator(ui);
            ui.add_space(4.0);

            // Quick access toolbar.
            if icon_button(ui, Icon::New, "Nova planta (Ctrl+N)", false).clicked() {
                self.request_new_project();
            }
            if icon_button(ui, Icon::Open, "Abrir projeto (Ctrl+O)", false).clicked() {
                self.request_open();
            }
            if icon_button(ui, Icon::Save, "Salvar projeto (Ctrl+S)", false).clicked() {
                self.save();
            }
            separator(ui);
            if ui
                .add_enabled_ui(self.history.can_undo(), |ui| {
                    icon_button(ui, Icon::Undo, "Desfazer (Ctrl+Z)", false)
                })
                .inner
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled_ui(self.history.can_redo(), |ui| {
                    icon_button(ui, Icon::Redo, "Refazer (Ctrl+Y)", false)
                })
                .inner
                .clicked()
            {
                self.redo();
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let export = export_button(ui);
                egui::Popup::menu(&export).show(|ui| self.export_menu(ui));
                ui.add_space(10.0);
                self.document_chip(ui);
            });
        });
    }

    fn document_chip(&mut self, ui: &mut Ui) {
        let name = if self.project.name.trim().is_empty() {
            "Sem nome".to_owned()
        } else {
            self.project.name.trim().to_owned()
        };
        let file = self.document_name();
        let title = ui.painter().layout_no_wrap(name, strong(12.5), TEXT);
        let detail = ui.painter().layout_no_wrap(file, regular(12.0), MUTED);
        let width = 16.0 + 18.0 + title.size().x + 8.0 + detail.size().x + 22.0;
        let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
        let painter = ui.painter();
        painter.rect(
            rect,
            13,
            PANEL_ALT,
            Stroke::new(1.0, BORDER),
            StrokeKind::Inside,
        );
        icons::paint(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + 18.0, rect.center().y),
                vec2(15.0, 15.0),
            ),
            Icon::Plan,
            TEXT_2,
            ACCENT,
        );
        let mut x = rect.left() + 32.0;
        let title_width = title.size().x;
        painter.galley(
            Pos2::new(x, rect.center().y - title.size().y / 2.0),
            title,
            TEXT,
        );
        x += title_width + 8.0;
        painter.galley(
            Pos2::new(x, rect.center().y - detail.size().y / 2.0),
            detail,
            MUTED,
        );
        let (dot, text) = if self.dirty {
            (WARNING, "Alterações não salvas")
        } else {
            (SUCCESS, "Todas as alterações foram salvas")
        };
        painter.circle_filled(Pos2::new(rect.right() - 12.0, rect.center().y), 3.5, dot);
        tip(response, text);
    }

    fn file_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if menu_item(ui, Some(Icon::New), "Nova planta", "Ctrl+N").clicked() {
            self.request_new_project();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Open), "Abrir...", "Ctrl+O").clicked() {
            self.request_open();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Save), "Salvar", "Ctrl+S").clicked() {
            self.save();
            ui.close();
        }
        menu_separator(ui);
        if menu_item(ui, Some(Icon::ExportImage), "Exportar imagem PNG...", "").clicked() {
            self.export_png();
            ui.close();
        }
        if menu_item(ui, Some(Icon::ExportPdf), "Exportar PDF...", "").clicked() {
            self.export_pdf();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Exportar SVG editável...", "").clicked() {
            self.export_svg();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Exportar quantitativos CSV...", "").clicked() {
            self.export_csv();
            ui.close();
        }
        menu_separator(ui);
        if menu_item(ui, Some(Icon::Close), "Sair", "Alt+F4").clicked() {
            self.request_close(ui.ctx());
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
                menu_item(ui, None, "Limpar seleção", "Esc")
            })
            .inner
            .clicked()
        {
            self.selection = None;
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
        ui.menu_button("Encaixes avançados (OSNAP)", |ui| self.osnap_menu(ui));
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
        menu_separator(ui);
        if menu_item(ui, Some(Icon::Plan), "Camadas...", "").clicked() {
            self.active_doc_dialog = super::super::DocDialog::Layers;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Ruler), "Cotas associativas...", "").clicked() {
            self.active_doc_dialog = super::super::DocDialog::Dimensions;
            ui.close();
        }
        if menu_item(ui, Some(Icon::Info), "Textos, notas e símbolos...", "").clicked() {
            self.active_doc_dialog = super::super::DocDialog::Notes;
            ui.close();
        }
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
            None,
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
        if menu_item(ui, Some(Icon::ExportImage), "Imagem PNG...", "").clicked() {
            self.export_png();
            ui.close();
        }
        if menu_item(ui, Some(Icon::ExportPdf), "Documento PDF...", "").clicked() {
            self.export_pdf();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Desenho SVG editável...", "").clicked() {
            self.export_svg();
            ui.close();
        }
        if menu_item(ui, Some(Icon::Export), "Quantitativos CSV...", "").clicked() {
            self.export_csv();
            ui.close();
        }
    }
}

fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 2.0),
            Pos2::new(rect.center().x, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, BORDER_STRONG),
    );
}

/// The call to action at the right of the menu bar.
fn export_button(ui: &mut Ui) -> egui::Response {
    let label =
        ui.painter()
            .layout_no_wrap("Exportar".to_owned(), strong(12.5), egui::Color32::WHITE);
    let (rect, response) =
        ui.allocate_exact_size(vec2(label.size().x + 44.0, 26.0), Sense::click());
    let painter = ui.painter();
    let fill = if response.is_pointer_button_down_on() || response.hovered() {
        ACCENT_STRONG
    } else {
        ACCENT
    };
    painter.rect_filled(rect, 6, fill);
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.center().y),
            vec2(15.0, 15.0),
        ),
        Icon::Export,
        egui::Color32::WHITE,
        egui::Color32::WHITE,
    );
    painter.galley(
        Pos2::new(rect.left() + 30.0, rect.center().y - label.size().y / 2.0),
        label,
        egui::Color32::WHITE,
    );
    tip(
        response,
        "Exportar a planta e seus dados em PNG, PDF, SVG ou CSV",
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
