//! The "Editar" ribbon tab: clipboard, transforms (move, copy, turn,
//! mirror), offset, trim and extend, and the selection commands.

use eframe::egui::Ui;

use crate::app::tools::ModifyKind;
use crate::app::ui::Glyph;
use crate::app::ui::icons::Icon;
use crate::app::ui::widgets::{
    ribbon_button, ribbon_panel, ribbon_small, ribbon_stack, tip, vertical_divider,
};
use crate::app::{FloorPlanApp, Tool};

impl FloorPlanApp {
    /// A button that picks a modify tool.
    fn modify_button(&mut self, ui: &mut Ui, kind: ModifyKind) {
        let enabled = !kind.uses_selection() || self.selection.is_some();
        let hint = match kind.key() {
            Some((_, key)) => format!("{} ({key})", kind.hint()),
            None if kind.uses_selection() => format!("{} Precisa de uma seleção.", kind.hint()),
            None => kind.hint().to_owned(),
        };
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                tip(
                    ribbon_button(
                        ui,
                        Icon::Glyph(kind.glyph()),
                        kind.label(),
                        self.tool == Tool::Modify(kind),
                        false,
                    ),
                    &hint,
                )
            })
            .inner;
        if response.clicked() {
            self.choose_modify_tool(kind);
        }
    }

    pub(super) fn edit_tab(&mut self, ui: &mut Ui) {
        let selected = self.selection.is_some();
        ribbon_panel(ui, "Área de transferência", |ui| {
            ribbon_stack(ui, |ui| {
                if ui
                    .add_enabled_ui(selected, |ui| {
                        tip(
                            ribbon_small(ui, Icon::Glyph(Glyph::Copy), "Copiar", false),
                            "Copiar a seleção (Ctrl+C)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.copy_selection();
                }
                if ui
                    .add_enabled_ui(selected, |ui| {
                        tip(
                            ribbon_small(ui, Icon::Glyph(Glyph::Cut), "Recortar", false),
                            "Recortar a seleção (Ctrl+X)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.cut_selection();
                }
                if ui
                    .add_enabled_ui(self.clipboard.is_some(), |ui| {
                        tip(
                            ribbon_small(ui, Icon::Glyph(Glyph::Paste), "Colar", false),
                            "Colar o que foi copiado; pelo teclado cola sob o cursor (Ctrl+V)",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.paste();
                }
            });
            if ui
                .add_enabled_ui(selected, |ui| {
                    tip(
                        ribbon_button(ui, Icon::Glyph(Glyph::Duplicate), "Duplicar", false, false),
                        "Uma cópia da seleção 1 m abaixo e à direita (Ctrl+D)",
                    )
                })
                .inner
                .clicked()
            {
                self.duplicate_selection();
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Transformar", |ui| {
            for kind in [
                ModifyKind::Move,
                ModifyKind::Copy,
                ModifyKind::Rotate,
                ModifyKind::Mirror,
            ] {
                self.modify_button(ui, kind);
            }
            ribbon_stack(ui, |ui| {
                for (icon, label, hint, action) in [
                    (
                        Icon::Redo,
                        "Girar 90° ↻",
                        "Girar a seleção 90° no sentido horário, em torno do centro (G)",
                        0,
                    ),
                    (
                        Icon::Undo,
                        "Girar 90° ↺",
                        "Girar a seleção 90° no sentido anti-horário (Shift+G)",
                        1,
                    ),
                    (
                        Icon::Glyph(Glyph::Mirror),
                        "Espelhar ↔",
                        "Espelhar a seleção da esquerda para a direita, no lugar (H)",
                        2,
                    ),
                ] {
                    if ui
                        .add_enabled_ui(selected, |ui| {
                            tip(ribbon_small(ui, icon, label, false), hint)
                        })
                        .inner
                        .clicked()
                    {
                        match action {
                            0 => self.rotate_selection_quarter(true),
                            1 => self.rotate_selection_quarter(false),
                            _ => self.mirror_selection(true),
                        }
                    }
                }
            });
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Geometria", |ui| {
            for kind in [ModifyKind::Offset, ModifyKind::Trim, ModifyKind::Extend] {
                self.modify_button(ui, kind);
            }
        });
        vertical_divider(ui, 72.0);
        ribbon_panel(ui, "Seleção", |ui| {
            ribbon_stack(ui, |ui| {
                if tip(
                    ribbon_small(ui, Icon::Select, "Selecionar tudo", false),
                    "Selecionar tudo o que o filtro deixa passar (Ctrl+A)",
                )
                .clicked()
                {
                    self.select_all();
                }
                if ui
                    .add_enabled_ui(selected, |ui| {
                        tip(
                            ribbon_small(ui, Icon::Search, "Semelhantes", false),
                            "Selecionar os elementos do mesmo tipo do selecionado",
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.select_similar();
                }
                if tip(
                    ribbon_small(ui, Icon::Undo, "Inverter", false),
                    "Selecionar o que não está selecionado (segue o filtro)",
                )
                .clicked()
                {
                    self.invert_selection();
                }
            });
        });
    }
}
