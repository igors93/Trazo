//! Selecting several elements at once: Shift+click adds or takes out, a box
//! drawn from empty space picks by window (left to right: what lies wholly
//! inside) or by crossing (right to left: whatever it touches), Ctrl+A
//! selects everything, and the selection filter says which kinds a box or
//! "select all" may pick.
//!
//! The primary element (`selection`) is the one the properties panel and
//! the vertex grips show; the others ride along in `selection_extra`.

use super::{FloorPlanApp, Selection};
use crate::domain::element::ElementKind;
use crate::domain::geometry::Point;

/// A box being drawn to select.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BoxSelect {
    pub start: Point,
    pub end: Point,
    /// Shift was held: what the box picks joins the selection.
    pub additive: bool,
    /// What a plain click at the start selects instead of a box (the
    /// terrain under the pointer), if anything.
    pub click: Option<Selection>,
}

impl BoxSelect {
    /// Drawn right to left, the box picks by crossing.
    pub fn crossing(&self) -> bool {
        self.end.x < self.start.x
    }
}

/// "1 parede", "3 paredes"...
fn counted(count: usize, kind: ElementKind) -> String {
    let (one, many) = match kind {
        ElementKind::Room => ("cômodo", "cômodos"),
        ElementKind::Wall => ("parede", "paredes"),
        ElementKind::Opening => ("abertura", "aberturas"),
        ElementKind::Fixture => ("objeto", "objetos"),
        ElementKind::Drawing => ("desenho", "desenhos"),
        ElementKind::Terrain => ("terreno", "terrenos"),
    };
    format!("{count} {}", if count == 1 { one } else { many })
}

impl FloorPlanApp {
    /// Every selected element, the primary one last.
    pub(super) fn selected(&self) -> Vec<Selection> {
        let mut all = self.selection_extra.clone();
        all.extend(self.selection);
        all
    }

    pub(super) fn selection_count(&self) -> usize {
        self.selection.map_or(0, |_| 1 + self.selection_extra.len())
    }

    /// Whether more than one element is selected.
    pub(super) fn is_multi_selection(&self) -> bool {
        self.selection.is_some() && !self.selection_extra.is_empty()
    }

    pub(super) fn is_selected(&self, item: Selection) -> bool {
        self.selection == Some(item) || self.selection_extra.contains(&item)
    }

    /// Selects one element, or nothing.
    pub(super) fn set_selection(&mut self, item: Option<Selection>) {
        self.selection = item;
        self.selection_extra.clear();
    }

    pub(super) fn clear_selection(&mut self) {
        self.set_selection(None);
    }

    /// Selects `items` (repeats dropped); the last one becomes the primary.
    pub(super) fn set_selection_set(&mut self, items: Vec<Selection>) {
        let mut unique: Vec<Selection> = Vec::with_capacity(items.len());
        for item in items {
            if let Some(index) = unique.iter().position(|kept| *kept == item) {
                unique.remove(index);
            }
            unique.push(item);
        }
        self.selection = unique.pop();
        self.selection_extra = unique;
    }

    /// Shift+click: adds `item` to the selection, or takes it out.
    pub(super) fn toggle_selection(&mut self, item: Selection) {
        let mut all = self.selected();
        if let Some(index) = all.iter().position(|kept| *kept == item) {
            all.remove(index);
        } else {
            all.push(item);
        }
        self.set_selection_set(all);
        self.after_selection_change();
    }

    /// "3 elementos: 2 paredes, 1 cômodo".
    pub(super) fn selection_summary(&self) -> String {
        let all = self.selected();
        let parts: Vec<String> = ElementKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let count = all.iter().filter(|item| item.kind() == kind).count();
                (count > 0).then(|| counted(count, kind))
            })
            .collect();
        format!("{} elementos: {}", all.len(), parts.join(", "))
    }

    /// The panel fields and the status line follow a new selection.
    pub(super) fn after_selection_change(&mut self) {
        self.sync_inspector_fields();
        self.message = if self.is_multi_selection() {
            format!(
                "{} selecionados. Arraste para mover juntos, setas deslocam, Delete exclui; Shift+clique adiciona ou retira.",
                self.selection_summary()
            )
        } else {
            Self::selection_message(self.selection).to_owned()
        };
    }

    /// Selects `items` from a command (select all, by kind...), with the
    /// selection tool; `additive` keeps what was selected.
    pub(super) fn select_many(&mut self, items: Vec<Selection>, additive: bool) {
        if self.tool != super::Tool::Move {
            self.set_tool(super::Tool::Move);
        }
        self.cancel_creation();
        let mut all = if additive {
            self.selected()
        } else {
            Vec::new()
        };
        all.extend(items);
        self.set_selection_set(all);
        if self.selection.is_none() {
            self.message = "Nada a selecionar com o filtro atual.".to_owned();
            self.sync_inspector_fields();
        } else {
            self.after_selection_change();
        }
    }

    /// Ctrl+A: every element the selection filter lets through.
    pub(super) fn select_all(&mut self) {
        let items = self.project.filtered_elements(self.selection_filter);
        self.select_many(items, false);
    }

    /// Every element like the primary selected one.
    pub(super) fn select_similar(&mut self) {
        let Some(model) = self.selection else {
            self.message = "Selecione um elemento para buscar os semelhantes.".to_owned();
            return;
        };
        let mut items = self.project.similar_elements(model);
        // The model stays the primary one.
        items.retain(|item| *item != model);
        items.push(model);
        self.select_many(items, false);
    }

    /// Everything the filter lets through that is not selected now.
    pub(super) fn invert_selection(&mut self) {
        let items: Vec<Selection> = self
            .project
            .filtered_elements(self.selection_filter)
            .into_iter()
            .filter(|item| !self.is_selected(*item))
            .collect();
        self.select_many(items, false);
    }

    /// Starts a selection box at `point`.
    pub(super) fn begin_box_select(
        &mut self,
        point: Point,
        additive: bool,
        click: Option<Selection>,
    ) {
        self.box_select = Some(BoxSelect {
            start: point,
            end: point,
            additive,
            click,
        });
    }

    pub(super) fn update_box_select(&mut self, point: Point) {
        if let Some(selection_box) = &mut self.box_select {
            selection_box.end = point;
        }
    }

    /// Ends the box: a box smaller than `min_size` metres is a click, which
    /// selects what was under it (or clears the selection); a real box
    /// selects by window or crossing.
    pub(super) fn finish_box_select(&mut self, min_size: f64) {
        let Some(selection_box) = self.box_select.take() else {
            return;
        };
        let (start, end) = (selection_box.start, selection_box.end);
        let is_click = (end.x - start.x).abs() < min_size && (end.y - start.y).abs() < min_size;
        if is_click {
            if selection_box.additive {
                if let Some(item) = selection_box.click {
                    self.toggle_selection(item);
                }
                return;
            }
            self.set_selection(selection_box.click);
            self.after_selection_change();
            return;
        }
        let picked = self.project.elements_in_box(
            start,
            end,
            selection_box.crossing(),
            self.selection_filter,
        );
        // Groups come whole.
        let items = self.project.with_whole_groups(&picked);
        let mut all = if selection_box.additive {
            self.selected()
        } else {
            Vec::new()
        };
        all.extend(items);
        self.set_selection_set(all);
        if self.selection.is_none() {
            self.message = if selection_box.crossing() {
                "Nada tocado pela seleção por cruzamento.".to_owned()
            } else {
                "Nenhum elemento inteiro dentro da janela (arraste da direita para a esquerda para selecionar por cruzamento).".to_owned()
            };
            self.sync_inspector_fields();
        } else {
            self.after_selection_change();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Tool;
    use crate::domain::element::SelectionFilter;
    use crate::domain::model::OpeningKind;
    use eframe::egui;

    fn app_with_rooms() -> (FloorPlanApp, u64, u64) {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let left = app
            .project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let right = app
            .project
            .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
            .unwrap();
        (app, left, right)
    }

    #[test]
    fn shift_click_adds_and_removes_and_the_last_is_primary() {
        let (mut app, left, right) = app_with_rooms();
        app.set_selection(Some(Selection::Room(left)));
        app.toggle_selection(Selection::Room(right));
        assert_eq!(app.selection, Some(Selection::Room(right)));
        assert!(app.is_selected(Selection::Room(left)));
        assert!(app.is_multi_selection() && app.selection_count() == 2);
        assert!(app.message.contains("2 elementos: 2 cômodos"));
        app.toggle_selection(Selection::Room(right));
        assert_eq!(app.selection, Some(Selection::Room(left)));
        assert!(!app.is_multi_selection());
        app.toggle_selection(Selection::Room(left));
        assert_eq!(app.selection_count(), 0);
    }

    #[test]
    fn boxes_select_by_window_or_by_crossing() {
        let (mut app, left, right) = app_with_rooms();
        // Left to right: only what lies wholly inside.
        app.begin_box_select(Point::new(-1.0, -1.0), false, None);
        app.update_box_select(Point::new(4.5, 4.0));
        app.finish_box_select(0.05);
        assert!(app.is_selected(Selection::Room(left)));
        assert!(!app.is_selected(Selection::Room(right)));

        // Right to left: whatever the box touches.
        app.begin_box_select(Point::new(4.5, 1.0), false, None);
        app.update_box_select(Point::new(3.5, 2.0));
        app.finish_box_select(0.05);
        assert!(app.is_selected(Selection::Room(left)));
        assert!(app.is_selected(Selection::Room(right)));

        // With the filter set to rooms only, walls stay out.
        app.selection_filter = SelectionFilter::only(ElementKind::Room);
        app.begin_box_select(Point::new(-1.0, -1.0), false, None);
        app.update_box_select(Point::new(9.0, 4.0));
        app.finish_box_select(0.05);
        assert_eq!(app.selection_count(), 2);

        // A click (a box with no size) on the terrain selects it; on empty
        // space it clears the selection.
        app.begin_box_select(Point::new(20.0, 20.0), false, None);
        app.finish_box_select(0.05);
        assert_eq!(app.selection_count(), 0);
        app.begin_box_select(Point::new(20.0, 20.0), false, Some(Selection::Terrain(42)));
        app.finish_box_select(0.05);
        assert_eq!(app.selection, Some(Selection::Terrain(42)));
    }

    #[test]
    fn select_all_similar_and_invert_follow_the_filter() {
        let (mut app, left, _) = app_with_rooms();
        let wall = app.project.rooms[0].wall_ids[0];
        let door = app
            .project
            .add_opening(wall, OpeningKind::Door, 0.5)
            .unwrap();
        app.set_tool(Tool::Wall);
        app.select_all();
        assert_eq!(app.tool, Tool::Move);
        // 2 rooms, 7 walls and a door.
        assert_eq!(app.selection_count(), 10);

        app.selection_filter = SelectionFilter::only(ElementKind::Wall);
        app.select_all();
        assert_eq!(app.selection_count(), 7);

        app.set_selection(Some(Selection::Room(left)));
        app.select_similar();
        assert_eq!(app.selection_count(), 2);
        assert_eq!(app.selection, Some(Selection::Room(left)));

        app.selection_filter = SelectionFilter::default();
        app.set_selection(Some(Selection::Opening(door)));
        app.invert_selection();
        assert_eq!(app.selection_count(), 9);
        assert!(!app.is_selected(Selection::Opening(door)));
    }
}
