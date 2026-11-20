//! The "Desenho" ribbon tab: 2D drafting tools that draw plain geometry
//! (lines, polylines, rectangles, arcs, circles and splines).

use eframe::egui::Ui;

use crate::app::tools::DrawKind;
use crate::app::ui::icons::Icon;
use crate::app::ui::widgets::ribbon_panel;
use crate::app::{FloorPlanApp, Tool};

impl FloorPlanApp {
    pub(super) fn drafting_tab(&mut self, ui: &mut Ui) {
        ribbon_panel(ui, "Desenhar", |ui| {
            for kind in DrawKind::ALL {
                let (_, shortcut) = kind.key();
                let hint = format!("{} ({shortcut})", kind.hint());
                self.tool_button(
                    ui,
                    Tool::Draw(kind),
                    Icon::Glyph(kind.glyph()),
                    kind.label(),
                    &hint,
                );
            }
        });
    }
}
