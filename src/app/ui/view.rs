//! The view's label, the floating navigation bar and the context menu of
//! the plan.

use super::super::{FloorPlanApp, RibbonTab, Selection, Tool, canvas::overlay, theme::*};
use super::icons::{self, Icon};
use super::options_bar::{tool_icon, tool_name};
use super::widgets::{
    MENU_WIDTH, caps_galley, icon_button_sized, menu_check, menu_item, menu_separator, submenu,
};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, vec2};

impl FloorPlanApp {
    /// The view's name at the top left of the plan, on a light plate.
    pub(in crate::app) fn paint_view_label(&self, painter: &egui::Painter, rect: Rect) {
        // Padding at the plate's ends, the gap between an icon and its
        // text, and the room on each side of the divider.
        const PAD: f32 = 12.0;
        const ICON_GAP: f32 = 7.0;
        const DIVIDER_GAP: f32 = 12.0;
        const VIEW_ICON: f32 = 15.0;
        const TOOL_ICON: f32 = 13.0;
        let floor = self
            .project
            .active_floor()
            .map(|level| level.name.as_str())
            .unwrap_or("Térreo");
        let title = painter.layout_no_wrap(format!("Planta baixa · {floor}"), strong(12.5), TEXT);
        let tool = painter.layout_no_wrap(tool_name(self.tool).to_owned(), regular(11.5), TEXT_2);
        let unit = painter.layout_no_wrap("metros".to_owned(), regular(11.5), MUTED);
        let (title_width, tool_width, unit_width) = (title.size().x, tool.size().x, unit.size().x);
        let width = PAD
            + VIEW_ICON
            + ICON_GAP
            + title_width
            + 2.0 * DIVIDER_GAP
            + TOOL_ICON
            + ICON_GAP
            + tool_width
            + DIVIDER_GAP
            + unit_width
            + PAD;
        let plate = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.top() + 12.0),
            vec2(width, 30.0),
        );
        if !rect.contains_rect(plate) {
            return;
        }
        overlay::paint_floating_plate(painter, plate);
        let middle = plate.center().y;
        let divider = |x: f32| {
            painter.line_segment(
                [
                    Pos2::new(x, plate.top() + 8.0),
                    Pos2::new(x, plate.bottom() - 8.0),
                ],
                Stroke::new(1.0, BORDER),
            );
        };
        let mut x = plate.left() + PAD;
        icons::paint(
            painter,
            Rect::from_min_size(
                Pos2::new(x, middle - VIEW_ICON / 2.0),
                vec2(VIEW_ICON, VIEW_ICON),
            ),
            Icon::Plan,
            TEXT_2,
            ACCENT,
        );
        x += VIEW_ICON + ICON_GAP;
        painter.galley(Pos2::new(x, middle - title.size().y / 2.0), title, TEXT);
        x += title_width + DIVIDER_GAP;
        divider(x);
        x += DIVIDER_GAP;
        icons::paint(
            painter,
            Rect::from_min_size(
                Pos2::new(x, middle - TOOL_ICON / 2.0),
                vec2(TOOL_ICON, TOOL_ICON),
            ),
            tool_icon(self.tool),
            ACCENT_TEXT,
            ACCENT,
        );
        x += TOOL_ICON + ICON_GAP;
        painter.galley(Pos2::new(x, middle - tool.size().y / 2.0), tool, TEXT_2);
        // The unit, set apart by a dot.
        x += tool_width + DIVIDER_GAP / 2.0;
        painter.circle_filled(Pos2::new(x, middle), 1.5, FAINT);
        x += DIVIDER_GAP / 2.0;
        painter.galley(Pos2::new(x, middle - unit.size().y / 2.0), unit, MUTED);
    }

    /// Matching top-left view plate for the inactive side of a split view.
    pub(in crate::app) fn paint_side_view_label(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        floor_name: &str,
    ) {
        const PAD: f32 = 12.0;
        const ICON_GAP: f32 = 7.0;
        const DIVIDER_GAP: f32 = 12.0;
        const VIEW_ICON: f32 = 15.0;
        let title_text = format!("Planta baixa · {floor_name}");
        let title = painter.layout_no_wrap(title_text, strong(12.5), TEXT);
        let hint = painter.layout_no_wrap("Clique para editar".to_owned(), regular(11.5), MUTED);
        let (title_width, hint_width) = (title.size().x, hint.size().x);
        let width = PAD + VIEW_ICON + ICON_GAP + title_width + 2.0 * DIVIDER_GAP + hint_width + PAD;
        let plate = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.top() + 12.0),
            vec2(width, 30.0),
        );
        if !rect.contains_rect(plate) {
            return;
        }
        overlay::paint_floating_plate(painter, plate);
        let middle = plate.center().y;
        let mut x = plate.left() + PAD;
        icons::paint(
            painter,
            Rect::from_min_size(
                Pos2::new(x, middle - VIEW_ICON / 2.0),
                vec2(VIEW_ICON, VIEW_ICON),
            ),
            Icon::Plan,
            TEXT_2,
            ACCENT,
        );
        x += VIEW_ICON + ICON_GAP;
        painter.galley(Pos2::new(x, middle - title.size().y / 2.0), title, TEXT);
        x += title_width + DIVIDER_GAP;
        painter.line_segment(
            [
                Pos2::new(x, plate.top() + 8.0),
                Pos2::new(x, plate.bottom() - 8.0),
            ],
            Stroke::new(1.0, BORDER),
        );
        x += DIVIDER_GAP;
        painter.galley(Pos2::new(x, middle - hint.size().y / 2.0), hint, MUTED);
    }

    /// A short name for the selected element, for the context menu.
    fn selection_title(&self, selection: Selection) -> String {
        match selection {
            Selection::Wall(id) => self
                .project
                .walls
                .iter()
                .find(|wall| wall.id == id)
                .map(|wall| format!("Parede · {}", format_meters(wall.length())))
                .unwrap_or_else(|| "Parede".to_owned()),
            Selection::Room(id) => self
                .project
                .rooms
                .iter()
                .find(|room| room.id == id)
                .map(|room| format!("Cômodo · {}", room.display_name()))
                .unwrap_or_else(|| "Cômodo".to_owned()),
            Selection::Opening(id) => self
                .project
                .openings
                .iter()
                .find(|opening| opening.id == id)
                .map(|opening| {
                    format!(
                        "{} · {}",
                        opening.kind.label(),
                        format_meters(opening.width())
                    )
                })
                .unwrap_or_else(|| "Abertura".to_owned()),
            Selection::Terrain(_) => "Terreno".to_owned(),
            Selection::Drawing(id) => self
                .project
                .drawing(id)
                .map(|drawing| {
                    format!(
                        "{} · {}",
                        drawing.shape.label(),
                        format_meters(drawing.shape.length())
                    )
                })
                .unwrap_or_else(|| "Desenho".to_owned()),
            Selection::Fixture(id) => self
                .project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .map(|fixture| {
                    format!(
                        "{} · {:.2} × {:.2} m",
                        fixture.kind.label(),
                        fixture.width,
                        fixture.depth
                    )
                    .replace('.', ",")
                })
                .unwrap_or_else(|| "Objeto".to_owned()),
        }
    }

    /// The right-click menu of the plan: what can be done with the element
    /// under the pointer, then the view and drawing commands.
    pub(in crate::app) fn canvas_context_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        if let Some(selection) = self.selection {
            let multi = self.is_multi_selection();
            let title = if multi {
                self.selection_summary()
            } else {
                self.selection_title(selection)
            };
            let galley = caps_galley(ui.painter(), &title, 10.5, MUTED);
            let (rect, _) = ui.allocate_exact_size(vec2(MENU_WIDTH, 22.0), Sense::hover());
            let title_clip = Rect::from_min_max(
                Pos2::new(rect.left() + 10.0, rect.top()),
                Pos2::new(rect.right() - 10.0, rect.bottom()),
            );
            ui.painter().with_clip_rect(title_clip).galley(
                Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
                galley,
                MUTED,
            );
            if menu_item(ui, Some(Icon::PanelRight), "Propriedades", "").clicked() {
                self.show_properties = true;
                ui.close();
            }
            if menu_item(ui, Some(Icon::Search), "Aproximar", "").clicked() {
                self.zoom_to_selection(selection);
                ui.close();
            }
            if !multi
                && matches!(selection, Selection::Fixture(_))
                && menu_item(
                    ui,
                    Some(Icon::Glyph(super::Glyph::RotateCw)),
                    "Girar 90°",
                    "G",
                )
                .clicked()
            {
                self.rotate_fixture_or_next();
                ui.close();
            }
            let is_door = match selection {
                _ if multi => false,
                Selection::Opening(id) => self
                    .project
                    .openings
                    .iter()
                    .any(|opening| opening.id == id && opening.kind.is_door()),
                _ => false,
            };
            if is_door {
                if menu_item(ui, Some(Icon::Door), "Abrir para o outro lado", "I").clicked() {
                    self.flip_selected_door(true, false);
                    ui.close();
                }
                if menu_item(
                    ui,
                    Some(Icon::Glyph(super::Glyph::Mirror)),
                    "Trocar lado da dobradiça",
                    "Shift+I",
                )
                .clicked()
                {
                    self.flip_selected_door(false, true);
                    ui.close();
                }
            }
            if menu_item(
                ui,
                Some(Icon::Glyph(super::Glyph::SelectSimilar)),
                "Selecionar semelhantes",
                "",
            )
            .clicked()
            {
                self.select_similar();
                ui.close();
            }
            menu_separator(ui);
            self.clipboard_menu_items(ui);
            submenu(
                ui,
                Icon::Glyph(super::Glyph::Rotate),
                "Girar e espelhar",
                |ui| {
                    self.transform_menu_items(ui);
                },
            );
            submenu(ui, Icon::Glyph(super::Glyph::Group), "Organizar", |ui| {
                self.arrange_menu_items(ui);
            });
            let delete = if multi { "Excluir todos" } else { "Excluir" };
            if menu_item(ui, Some(Icon::Delete), delete, "Delete").clicked() {
                self.delete_selection();
                ui.close();
            }
            menu_separator(ui);
        } else if self.clipboard.is_some() {
            if menu_item(
                ui,
                Some(Icon::Glyph(super::Glyph::Paste)),
                "Colar aqui",
                "Ctrl+V",
            )
            .clicked()
            {
                self.paste_at(self.context_point);
                ui.close();
            }
            menu_separator(ui);
        }
        if menu_item(
            ui,
            Some(Icon::Glyph(super::Glyph::SelectAll)),
            "Selecionar tudo",
            "Ctrl+A",
        )
        .clicked()
        {
            self.select_all();
            ui.close();
        }
        if menu_item(ui, Some(Icon::ZoomFit), "Enquadrar tudo", "F").clicked() {
            self.fit_view_requested();
            ui.close();
        }
        submenu(ui, Icon::Wall, "Desenhar", |ui| {
            for (icon, label, key, tool) in [
                (Icon::Select, "Selecionar", "V", Tool::Move),
                (Icon::Terrain, "Terreno", "T", Tool::Terrain),
                (Icon::Wall, "Parede", "P", Tool::Wall),
                (Icon::Room, "Cômodo", "C", Tool::Room),
                (Icon::Door, "Porta", "R", Tool::Opening(OpeningKind::Door)),
                (
                    Icon::Window,
                    "Janela",
                    "J",
                    Tool::Opening(OpeningKind::Window),
                ),
                (Icon::Gate, "Portão", "", Tool::Opening(OpeningKind::Gate)),
            ] {
                if menu_item(ui, Some(icon), label, key).clicked() {
                    self.set_tool(tool);
                    if tool == Tool::Terrain {
                        self.sync_terrain_fields();
                    }
                    self.ribbon_tab = RibbonTab::Architecture;
                    ui.close();
                }
            }
        });
        menu_separator(ui);
        if menu_check(ui, "Grade", "F7", self.show_grid).clicked() {
            self.toggle_grid();
        }
        if menu_check(ui, "Encaixe magnético", "F3", self.snap_enabled).clicked() {
            self.toggle_snap();
        }
        if menu_check(ui, "Modo orto", "F8", self.ortho_enabled).clicked() {
            self.toggle_ortho();
        }
    }

    /// The floating zoom bar at the top right of the plan.
    pub(in crate::app) fn canvas_navigation(&mut self, ui: &mut Ui, rect: Rect) {
        overlay::paint_floating_plate(ui.painter(), rect);
        let size = overlay::NAVIGATION_BUTTON;
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect.shrink(3.0))
                .layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add_space(1.0);
                if icon_button_sized(ui, Icon::ZoomIn, "Ampliar", false, size, 17.0).clicked() {
                    self.zoom_by(1.25);
                }
                if icon_button_sized(ui, Icon::ZoomOut, "Reduzir", false, size, 17.0).clicked() {
                    self.zoom_by(0.8);
                }
                if icon_button_sized(ui, Icon::ZoomFit, "Enquadrar tudo (F)", false, size, 17.0)
                    .clicked()
                {
                    self.fit_view_requested();
                }
                let (line, _) = ui.allocate_exact_size(vec2(size, 9.0), Sense::hover());
                ui.painter().line_segment(
                    [
                        Pos2::new(line.left() + 6.0, line.center().y),
                        Pos2::new(line.right() - 6.0, line.center().y),
                    ],
                    Stroke::new(1.0, BORDER),
                );
                if icon_button_sized(
                    ui,
                    Icon::Grid,
                    "Mostrar ou ocultar a grade (F7)",
                    self.show_grid,
                    size,
                    17.0,
                )
                .clicked()
                {
                    self.toggle_grid();
                }
            },
        );
    }
}
