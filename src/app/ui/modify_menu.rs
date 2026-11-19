//! Menu entries of the clipboard and of the modify commands, shared by the
//! "Editar" menu and the plan's context menu.

use eframe::egui::Ui;

use super::super::{FloorPlanApp, RibbonTab, Tool};
use super::Glyph;
use super::icons::Icon;
use super::widgets::{MENU_WIDTH, menu_item};
use crate::app::tools::{ConstraintTool, ModifyKind};
use crate::domain::model::{Alignment, Axis, Entity};

impl FloorPlanApp {
    /// The constraint tools, and removing the selection's constraints.
    pub(in crate::app) fn constraint_menu_items(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        for kind in ConstraintTool::ALL {
            if menu_item(ui, Some(Icon::Glyph(kind.glyph())), kind.label(), "")
                .on_hover_text(kind.hint())
                .clicked()
            {
                self.set_tool(Tool::Constrain(kind));
                self.ribbon_tab = RibbonTab::Edit;
                ui.close();
            }
        }
        let constrained = self.selection_has_constraints();
        if ui
            .add_enabled_ui(constrained, |ui| {
                menu_item(ui, Some(Icon::Delete), "Remover restrições da seleção", "")
            })
            .inner
            .clicked()
        {
            self.remove_selection_constraints();
            ui.close();
        }
    }

    /// Whether any selected wall or drawing is constrained.
    pub(in crate::app) fn selection_has_constraints(&self) -> bool {
        self.selected()
            .into_iter()
            .filter_map(Entity::of)
            .any(|entity| !self.project.constraints_of(entity).is_empty())
    }

    /// The six alignments.
    pub(in crate::app) fn align_menu_items(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        for alignment in Alignment::ALL {
            if menu_item(ui, None, alignment.label(), "").clicked() {
                self.align_selection(alignment);
                ui.close();
            }
        }
    }

    /// Distribute across or down.
    pub(in crate::app) fn distribute_menu_items(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        for axis in [Axis::Horizontal, Axis::Vertical] {
            if menu_item(ui, Some(Icon::Glyph(Glyph::Distribute)), axis.label(), "").clicked() {
                self.distribute_selection(axis);
                ui.close();
            }
        }
    }

    /// Join, group, ungroup, align, distribute and repeat.
    pub(in crate::app) fn arrange_menu_items(&mut self, ui: &mut Ui) {
        let selected = self.selection.is_some();
        let several = self.is_multi_selection();
        let grouped = self
            .selected()
            .iter()
            .any(|item| self.project.group_of(*item).is_some());
        let entries = [
            (Glyph::Join, "Unir", "U", several, 0),
            (Glyph::Group, "Agrupar", "Ctrl+G", several, 1),
            (Glyph::Ungroup, "Desagrupar", "Ctrl+Shift+G", grouped, 2),
            (Glyph::Repeat, "Repetir...", "", selected, 3),
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
                    0 => self.join_selection(),
                    1 => self.group_selection(),
                    2 => self.ungroup_selection(),
                    _ => self.open_repeat_dialog(),
                }
                ui.close();
            }
        }
        ui.add_enabled_ui(several, |ui| {
            ui.menu_button("Alinhar", |ui| self.align_menu_items(ui));
        });
        ui.add_enabled_ui(self.selection_count() >= 3, |ui| {
            ui.menu_button("Distribuir", |ui| self.distribute_menu_items(ui));
        });
    }

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
