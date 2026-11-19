//! Menu entries of the clipboard and of the modify commands, shared by the
//! "Editar" menu and the plan's context menu.

use eframe::egui::Ui;

use super::super::{FloorPlanApp, RibbonTab};
use super::Glyph;
use super::icons::Icon;
use super::widgets::{MENU_WIDTH, menu_item};
use crate::app::tools::ModifyKind;

impl FloorPlanApp {
    /// Copy, cut, paste and duplicate.
    pub(in crate::app) fn clipboard_menu_items(&mut self, ui: &mut Ui) {
        let selected = self.selection.is_some();
        let entries = [
            (Glyph::Copy, "Copiar", "Ctrl+C", selected, 0),
            (Glyph::Cut, "Recortar", "Ctrl+X", selected, 1),
            (Glyph::Paste, "Colar", "Ctrl+V", self.clipboard.is_some(), 2),
            (Glyph::Duplicate, "Duplicar", "Ctrl+D", selected, 3),
        ];
        for (glyph, label, keys, enabled, action) in entries {
            if ui
                .add_enabled_ui(enabled, |ui| {
                    menu_item(ui, Some(Icon::Glyph(glyph)), label, keys)
                })
                .inner
                .clicked()
            {
                match action {
                    0 => {
                        self.copy_selection();
                    }
                    1 => self.cut_selection(),
                    2 => self.paste(),
                    _ => self.duplicate_selection(),
                }
                ui.close();
            }
        }
    }

    /// Quick turns and flips of the selection, and the modify tools.
    pub(in crate::app) fn transform_menu_items(&mut self, ui: &mut Ui) {
        let selected = self.selection.is_some();
        let quick = [
            (Icon::Redo, "Girar 90° no sentido horário", "G", 0),
            (
                Icon::Undo,
                "Girar 90° no sentido anti-horário",
                "Shift+G",
                1,
            ),
            (
                Icon::Glyph(Glyph::Mirror),
                "Espelhar ↔ (esquerda e direita)",
                "H",
                2,
            ),
            (
                Icon::Glyph(Glyph::MirrorVertical),
                "Espelhar ↕ (cima e baixo)",
                "Shift+H",
                3,
            ),
        ];
        for (icon, label, keys, action) in quick {
            if ui
                .add_enabled_ui(selected, |ui| menu_item(ui, Some(icon), label, keys))
                .inner
                .clicked()
            {
                match action {
                    0 => self.rotate_selection_quarter(true),
                    1 => self.rotate_selection_quarter(false),
                    2 => self.mirror_selection(true),
                    _ => self.mirror_selection(false),
                }
                ui.close();
            }
        }
        ui.menu_button("Ferramentas de modificação", |ui| {
            ui.set_width(MENU_WIDTH);
            for kind in ModifyKind::ALL {
                let enabled = !kind.uses_selection() || selected;
                let keys = kind.key().map_or("", |(_, keys)| keys);
                if ui
                    .add_enabled_ui(enabled, |ui| {
                        menu_item(ui, Some(Icon::Glyph(kind.glyph())), kind.label(), keys)
                    })
                    .inner
                    .on_hover_text(kind.hint())
                    .clicked()
                {
                    self.choose_modify_tool(kind);
                    self.ribbon_tab = RibbonTab::Edit;
                    ui.close();
                }
            }
        });
    }
}
