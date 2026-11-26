//! Moving between tabs and arranging them: switching, cycling, moving a
//! tab to the other pane, reordering and the side-by-side split.

use super::{TabPane, adjust_reordered_index, format_tab_title};
use crate::app::FloorPlanApp;

impl FloorPlanApp {
    /// Shows the tab at `index`.
    ///
    /// When switching between two linked tabs of the same project, the shared
    /// [`Project`], [`History`], `dirty` flag and [`PlanLog`] remain untouched
    /// in place (zero copy).
    pub(crate) fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab || self.tab_view(index).is_none() {
            return;
        }
        self.settle_before_switch();
        let old = self.active_tab;
        let old_pane = self.active_pane;
        let old_pid = self.active_project_id;
        let target_pane = self.tab_pane(index);

        let Some(next_view) = self.tabs.get_mut(index).and_then(Option::take) else {
            return;
        };
        let previous_view = self.take_view_tab();
        self.tabs[old] = Some(previous_view);
        let next_pid = next_view.project_id;

        if next_pid != old_pid {
            if let Some(target_project) = self.projects.remove(&next_pid) {
                let parked = self.take_shared_project();
                self.projects.insert(old_pid, parked);
                self.put_shared_project(target_project);
                // Notify MCP subscribers that the active project changed without
                // mutating the target project's own revision history.
                self.mcp_events.publish(&crate::mcp::events::plan_updated());
            } else {
                // Defensive rollback if tab state was somehow inconsistent.
                self.tabs[index] = Some(next_view);
                if let Some(prev) = self.tabs[old].take() {
                    self.put_view_tab(prev);
                }
                return;
            }
        }

        self.put_view_tab(next_view);
        self.active_tab = index;
        self.align_active_floor_to_tab();
        self.revalidate_selection();

        // Side by side: focusing a tab on the other pane keeps the previous
        // active tab visible on its own pane, without jumping sides.
        if self.split_tab.is_some() && (target_pane != old_pane || self.split_tab == Some(index)) {
            self.split_tab = Some(old);
        }

        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        let linked = self.project_tab_count(self.active_project_id) > 1;
        let label = format_tab_title(
            &self.project,
            self.file_path.as_deref(),
            self.active_tab_title.as_deref(),
            self.discipline,
            linked,
        );
        self.message = format!("Aba: {label}.");
    }

    pub(crate) fn next_tab(&mut self, step: isize) {
        let count = self.tabs.len() as isize;
        if count > 1 {
            let next = (self.active_tab as isize + step).rem_euclid(count) as usize;
            self.switch_tab(next);
        }
    }

    /// Moves tab `index` to `target` pane when viewing side by side.
    pub(crate) fn move_tab_to_pane(&mut self, index: usize, target: TabPane) {
        if self.split_tab.is_none() || index >= self.tabs.len() || self.tab_pane(index) == target {
            return;
        }
        let source_pane = target.opposite();
        if index == self.active_tab {
            self.active_pane = target;
        } else if let Some(view) = self.tabs[index].as_mut() {
            view.pane = target;
        }
        let replacement =
            (0..self.tabs.len()).find(|&i| i != index && self.tab_pane(i) == source_pane);
        match replacement {
            None => {
                self.toggle_side_by_side(None);
            }
            Some(other) => {
                if index == self.active_tab || self.split_tab == Some(index) {
                    self.split_tab = Some(other);
                }
            }
        }
    }

    /// Reorders the tab at `from` to index `to`, keeping `active_tab` and
    /// `split_tab` pointing to the exact same logical tabs.
    pub(crate) fn reorder_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let moved = self.tabs.remove(from);
        self.tabs.insert(to, moved);
        self.active_tab = adjust_reordered_index(self.active_tab, from, to);
        self.split_tab = self
            .split_tab
            .map(|idx| adjust_reordered_index(idx, from, to));
    }

    /// Shows `index` beside the active tab, or hides the side view.
    pub(crate) fn toggle_side_by_side(&mut self, index: Option<usize>) {
        self.last_canvas_center = None;
        let target =
            index.filter(|index| *index != self.active_tab && self.tab_view(*index).is_some());
        match target {
            Some(other) => {
                self.active_pane = TabPane::Left;
                for (i, slot) in self.tabs.iter_mut().enumerate() {
                    if let Some(view) = slot {
                        view.pane = if i == other {
                            TabPane::Right
                        } else {
                            TabPane::Left
                        };
                    }
                }
                self.split_tab = Some(other);
                self.message = "Lado a lado: arraste a divisória para redimensionar; clique no outro lado para editá-lo.".to_owned();
            }
            None => {
                self.split_tab = None;
                self.active_pane = TabPane::Left;
                for view in self.tabs.iter_mut().flatten() {
                    view.pane = TabPane::Left;
                }
                self.message = "Vista única.".to_owned();
            }
        }
    }
}
