//! Undo, redo and recording project changes in the history.

use super::{CreationState, FloorPlanApp};
use crate::domain::model::Project;
use crate::domain::protection::hypotheses::{HypothesisSource, track_changes};

impl FloorPlanApp {
    pub(super) fn undo(&mut self) {
        let before = self.project.clone();
        if self.history.undo(&mut self.project) {
            self.sync_report = crate::domain::dependencies::DependencySyncReport::default();
            self.note_plan_change(&before, false);
            self.dirty = true;
            self.recovery_pending = true;
            self.revalidate_selection();
            self.revalidate_linked_tab_selections();
            self.item_drag = None;
            self.drag_initial_project = None;
            self.creation_state = CreationState::Idle;
            self.pending_room = None;
            self.sync_inspector_fields();
            self.message = "Ação desfeita.".to_owned();
        }
    }

    pub(super) fn redo(&mut self) {
        let before = self.project.clone();
        if self.history.redo(&mut self.project) {
            self.sync_report = crate::domain::dependencies::DependencySyncReport::default();
            self.note_plan_change(&before, false);
            self.dirty = true;
            self.recovery_pending = true;
            self.revalidate_selection();
            self.revalidate_linked_tab_selections();
            self.item_drag = None;
            self.drag_initial_project = None;
            self.creation_state = CreationState::Idle;
            self.pending_room = None;
            self.sync_inspector_fields();
            self.message = "Ação refeita.".to_owned();
        }
    }

    /// Records `previous` as the undo step of the edit just made, once the
    /// constraints have been kept (moving what the edit did not touch).
    /// Returns false when the constraints refused the edit, or undid it,
    /// leaving the project as it was; the message then says why, and the
    /// caller must not announce a change.
    pub(super) fn record_change(&mut self, previous: Project) -> bool {
        // Deleted elements leave their groups (a group of one goes), and
        // their conduits go with them.
        self.project.prune_groups();
        self.project.prune_conduits();
        self.project.prune_structure();
        let edited = previous != self.project;
        if let Err(error) = self.project.settle_constraints(&previous) {
            self.project = previous;
            self.message = error;
            self.edit_notice = Some(self.message.clone());
            return false;
        }
        if previous == self.project {
            if edited {
                self.message = "As restrições mantiveram a geometria como estava; remova a restrição para fazer essa mudança.".to_owned();
                self.edit_notice = Some(self.message.clone());
                return false;
            }
            return true;
        }
        // Who changed each calculation hypothesis (soil, concrete, loads,
        // wind), so Trazo can contest a favourable value nobody backs up
        // (#168). Every edit passes here, from the window or from MCP.
        let source = if self.mcp_running.is_some() {
            HypothesisSource::Assistant
        } else {
            HypothesisSource::Person
        };
        track_changes(&previous, &mut self.project, source);
        // Propagate safe cross-discipline dependencies (wall-mounted devices,
        // footings under moved columns) within the same undo transaction.
        let sync = self.project.propagate_dependencies(&previous);
        if let Some(summary) = sync.summary_line() {
            self.edit_notice = Some(summary);
        }
        self.sync_report = sync;
        self.note_plan_change(&previous, false);
        // An AI batch records itself as one step when it ends.
        if self.batch_depth == 0 {
            self.history.record(previous);
        }
        self.mark_edited_outside_history();
        true
    }

    /// Marks the project as changed and due for the crash-recovery draft,
    /// for continuous form edits that are not individual undo steps (such as
    /// typing the project name or technical-sheet metadata).
    pub(super) fn mark_edited_outside_history(&mut self) {
        self.dirty = true;
        self.recovery_pending = true;
    }
}
