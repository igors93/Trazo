//! Closing tabs, with the unsaved-changes confirmation for the last view
//! of a project.

use super::{SharedProject, TabPane, ViewTab};
use crate::app::{FloorPlanApp, PendingAction};
use crate::domain::discipline::Discipline;
use crate::domain::model::Project;
use crate::io::recovery;

impl FloorPlanApp {
    /// Asks to save ONLY when closing the last open tab of a project with
    /// unsaved changes; closing a secondary linked view never discards data.
    pub(crate) fn request_close_tab(&mut self, index: usize) {
        if index != self.active_tab {
            self.switch_tab(index);
        }
        let last_link = self.project_tab_count(self.active_project_id) <= 1;
        if last_link && self.dirty {
            self.pending_action = Some(PendingAction::CloseTab);
        } else {
            self.close_active_tab();
        }
    }

    /// Closes the active tab without asking.
    ///
    /// If other tabs are still linked to the same project, the shared project
    /// and all its unsaved changes remain alive in them. If this was the last
    /// tab in the window, it turns into a fresh blank project.
    pub(crate) fn close_active_tab(&mut self) {
        self.settle_before_switch();
        let closed_pid = self.active_project_id;
        let still_linked = self.project_tab_count(closed_pid) > 1;

        if self.tabs.len() == 1 {
            recovery::clear();
            let new_pid = self.alloc_project_id();
            let new_tid = self.alloc_tab_id();
            self.put_shared_project(SharedProject::holding(
                new_pid,
                Project::default(),
                None,
                false,
            ));
            self.put_view_tab(ViewTab::new(new_tid, new_pid, Discipline::All));
            self.split_tab = None;
            self.active_pane = TabPane::Left;
            self.fit_pending = false;
            self.mcp_events.publish(&crate::mcp::events::plan_updated());
            self.sync_inspector_fields();
            self.sync_masonry_fields();
            self.sync_drafts();
            self.message = "Aba fechada.".to_owned();
            return;
        }

        let closed = self.active_tab;
        let closed_pane = self.active_pane;
        self.tabs.remove(closed);

        let adjusted_split = match self.split_tab {
            Some(split) if split == closed => None,
            Some(split) if split > closed => Some(split - 1),
            other => other,
        };

        let next_index = if let Some(other_visible) = adjusted_split {
            let same_pane = (0..self.tabs.len()).find(|&i| {
                i != other_visible
                    && self
                        .tab_view(i)
                        .is_some_and(|view| view.pane == closed_pane)
            });
            if let Some(index) = same_pane {
                self.split_tab = Some(other_visible);
                index
            } else {
                self.split_tab = None;
                for view in self.tabs.iter_mut().flatten() {
                    view.pane = TabPane::Left;
                }
                other_visible
            }
        } else {
            self.split_tab = None;
            for view in self.tabs.iter_mut().flatten() {
                view.pane = TabPane::Left;
            }
            closed.min(self.tabs.len() - 1)
        };

        let Some(next_view) = self.tabs.get_mut(next_index).and_then(Option::take) else {
            return;
        };
        let next_pid = next_view.project_id;

        if next_pid != closed_pid {
            if still_linked {
                // Other tabs still reference `closed_pid`: keep the shared project alive!
                let parked = self.take_shared_project();
                self.projects.insert(closed_pid, parked);
            } else {
                recovery::clear();
            }
            if let Some(target_project) = self.projects.remove(&next_pid) {
                self.put_shared_project(target_project);
                self.mcp_events.publish(&crate::mcp::events::plan_updated());
            }
        }

        self.put_view_tab(next_view);
        if self.split_tab.is_none() {
            self.active_pane = TabPane::Left;
        }
        self.active_tab = next_index;
        self.align_active_floor_to_tab();
        self.revalidate_selection();
        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        self.message = if still_linked {
            "Visão fechada (o projeto continua aberto nas outras abas vinculadas).".to_owned()
        } else {
            "Aba fechada.".to_owned()
        };
    }

    /// After saving or discarding the active project while closing the window:
    /// switches to the next distinct dirty project, or lets the window close.
    pub(crate) fn next_unsaved_before_closing(&mut self) -> bool {
        self.dirty = false;
        self.recovery_pending = false;
        recovery::clear();
        let next = (0..self.tabs.len()).find(|&index| {
            self.tab_project_id(index)
                .is_some_and(|pid| pid != self.active_project_id && self.project_dirty(pid))
        });
        match next {
            Some(index) => {
                self.switch_tab(index);
                self.pending_action = Some(PendingAction::CloseApp);
                true
            }
            None => {
                recovery::clear_tabs();
                false
            }
        }
    }
}
