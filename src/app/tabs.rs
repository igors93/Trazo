//! Several projects in one window, as tabs.
//!
//! The active tab's state lives in the app's own fields, so every tool,
//! panel and assistant keeps working on `self.project` as before. The other
//! tabs wait in `tabs`, each with its own project, undo history, file,
//! selection and view; switching tabs swaps that state in and out. The
//! clipboard belongs to the window, so elements copied in one tab paste into
//! another. A second tab can be shown beside the active one, with the same
//! view, to compare designs of the same building (architecture, electrical,
//! structure).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;

use super::{FloorPlanApp, OpenResult, PendingAction, Selection, Tool};
use crate::domain::geometry::Point;
use crate::domain::history::History;
use crate::domain::model::Project;
use crate::io::recovery;

/// Which side of a split view holds a tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum TabPane {
    #[default]
    Left,
    Right,
}

impl TabPane {
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// What belongs to one open project.
pub(crate) struct Document {
    pub(crate) project: Project,
    history: History,
    pub(crate) file_path: Option<PathBuf>,
    pub(crate) dirty: bool,
    recovery_pending: bool,
    selection: Option<Selection>,
    selection_extra: Vec<Selection>,
    scale: f32,
    pan: egui::Vec2,
    fit_pending: bool,
    measurements: Vec<(Point, Point)>,
    constraint_texts: HashMap<u64, String>,
    discipline: crate::domain::discipline::Discipline,
    pub(crate) pane: TabPane,
}

impl Document {
    /// A blank plan, as a new tab opens.
    pub(crate) fn blank() -> Self {
        Self::holding(Project::default(), None, false)
    }

    /// A tab holding `project`; `dirty` when it has unsaved changes.
    pub(crate) fn holding(project: Project, file_path: Option<PathBuf>, dirty: bool) -> Self {
        Self {
            project,
            history: History::new(),
            file_path,
            dirty,
            recovery_pending: false,
            selection: None,
            selection_extra: Vec::new(),
            scale: 60.0,
            pan: egui::Vec2::ZERO,
            fit_pending: true,
            measurements: Vec::new(),
            constraint_texts: HashMap::new(),
            discipline: Default::default(),
            pane: TabPane::Left,
        }
    }
}

/// What the tab strip shows for one tab.
pub(crate) struct TabInfo {
    pub(crate) title: String,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
    pub(crate) pane: TabPane,
    /// Displayed on screen right now (either active or the side-by-side tab).
    pub(crate) shown: bool,
}

fn title_of(project: &Project, file_path: Option<&Path>) -> String {
    let name = project.name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    file_path
        .and_then(|path| path.file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Sem título".to_owned())
}

impl FloorPlanApp {
    /// Moves the active project's state out, leaving a blank in its place.
    fn take_document(&mut self) -> Document {
        Document {
            project: std::mem::take(&mut self.project),
            history: std::mem::replace(&mut self.history, History::new()),
            file_path: self.file_path.take(),
            dirty: std::mem::take(&mut self.dirty),
            recovery_pending: std::mem::take(&mut self.recovery_pending),
            selection: self.selection.take(),
            selection_extra: std::mem::take(&mut self.selection_extra),
            scale: self.scale,
            pan: self.pan,
            fit_pending: self.fit_pending,
            measurements: std::mem::take(&mut self.measurements),
            constraint_texts: std::mem::take(&mut self.constraint_texts),
            discipline: self.discipline,
            pane: self.active_pane,
        }
    }

    /// Makes `document` the active project.
    fn put_document(&mut self, document: Document) {
        self.project = document.project;
        self.history = document.history;
        self.file_path = document.file_path;
        self.dirty = document.dirty;
        self.recovery_pending = document.recovery_pending;
        self.selection = document.selection;
        self.selection_extra = document.selection_extra;
        self.scale = document.scale;
        self.pan = document.pan;
        self.fit_pending = document.fit_pending;
        self.measurements = document.measurements;
        self.constraint_texts = document.constraint_texts;
        self.discipline = document.discipline;
        self.active_pane = document.pane;
        self.last_canvas_center = None;
    }

    /// Ends whatever is half done on the plan before another tab shows:
    /// drags are committed, drafts dropped. The selection stays with its tab.
    fn settle_before_switch(&mut self) {
        self.commit_item_drag();
        self.ai_activity.forget_highlight();
        self.cancel_creation();
        self.box_select = None;
        self.item_drag = None;
        self.vertex_drag = None;
        self.drag_initial_project = None;
        self.pending_room = None;
        self.zoom_target = None;
        self.tool = Tool::Move;
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_tab(&self) -> usize {
        self.active_tab
    }

    /// The tabs, in order, for the tab strip.
    pub(crate) fn tab_infos(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| match tab {
                Some(document) => TabInfo {
                    title: title_of(&document.project, document.file_path.as_deref()),
                    dirty: document.dirty,
                    active: false,
                    pane: document.pane,
                    shown: self.split_tab == Some(index),
                },
                None => TabInfo {
                    title: title_of(&self.project, self.file_path.as_deref()),
                    dirty: self.dirty,
                    active: index == self.active_tab,
                    pane: self.active_pane,
                    shown: index == self.active_tab,
                },
            })
            .collect()
    }

    pub(crate) fn active_pane(&self) -> TabPane {
        self.active_pane
    }

    pub(crate) fn split_ratio(&self) -> f32 {
        self.split_ratio
    }

    pub(crate) fn set_split_ratio(&mut self, ratio: f32) {
        let clamped = ratio.clamp(0.20, 0.80);
        if (self.split_ratio - clamped).abs() > 1e-4 {
            self.split_ratio = clamped;
            self.last_canvas_center = None;
        }
    }

    /// Which pane `index` belongs to while split.
    pub(crate) fn tab_pane(&self, index: usize) -> TabPane {
        if index == self.active_tab {
            self.active_pane
        } else {
            self.tab_document(index)
                .map(|doc| doc.pane)
                .unwrap_or(TabPane::Left)
        }
    }

    /// A project waiting in another tab.
    pub(crate) fn tab_document(&self, index: usize) -> Option<&Document> {
        self.tabs.get(index).and_then(Option::as_ref)
    }

    /// Whether any tab has unsaved changes.
    pub(crate) fn any_tab_dirty(&self) -> bool {
        self.dirty || self.tabs.iter().flatten().any(|document| document.dirty)
    }

    /// The active tab holds nothing yet: no file, no changes, an empty plan.
    pub(crate) fn active_is_blank(&self) -> bool {
        !self.dirty && self.file_path.is_none() && self.project == Project::default()
    }

    /// Shows the tab at `index`.
    pub(crate) fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab || self.tab_document(index).is_none() {
            return;
        }
        self.settle_before_switch();
        let old = self.active_tab;
        let old_pane = self.active_pane;
        let target_pane = self.tab_pane(index);
        let previous = self.take_document();
        let before = previous.project.clone();
        self.tabs[old] = Some(previous);
        let next = self.tabs[index]
            .take()
            .expect("an inactive tab holds its project");
        self.put_document(next);
        self.active_tab = index;
        // Side by side: focusing a tab on the other pane keeps the previous
        // active tab visible on its own pane, without jumping sides.
        if self.split_tab.is_some() && (target_pane != old_pane || self.split_tab == Some(index)) {
            self.split_tab = Some(old);
        }
        self.note_plan_change(&before, true);
        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        self.message = format!(
            "Aba: {}.",
            title_of(&self.project, self.file_path.as_deref())
        );
    }

    pub(crate) fn next_tab(&mut self, step: isize) {
        let count = self.tabs.len() as isize;
        if count > 1 {
            let next = (self.active_tab as isize + step).rem_euclid(count) as usize;
            self.switch_tab(next);
        }
    }

    /// Opens a blank plan in a new tab on the active pane.
    pub(crate) fn new_tab(&mut self) {
        self.new_tab_in_pane(self.active_pane);
    }

    /// Opens a blank plan in a new tab belonging to `pane`.
    pub(crate) fn new_tab_in_pane(&mut self, pane: TabPane) {
        let mut document = Document::blank();
        document.pane = if self.split_tab.is_some() {
            pane
        } else {
            TabPane::Left
        };
        self.tabs.push(Some(document));
        let index = self.tabs.len() - 1;
        self.switch_tab(index);
        self.scale = 60.0;
        self.pan = egui::Vec2::ZERO;
        self.fit_pending = false;
        self.terrain_width_text = "5,00".to_owned();
        self.terrain_height_text = "10,00".to_owned();
        self.message = "Nova aba com uma planta em branco.".to_owned();
    }

    /// Opens a copy of the active project in a new tab: the base for another
    /// design of the same building, such as the electrical or structural one.
    pub(crate) fn duplicate_tab(&mut self) {
        self.duplicate_tab_in_pane(self.active_tab, self.active_pane);
    }

    /// Copies the project in `source` into a new tab assigned to `pane`.
    pub(crate) fn duplicate_tab_in_pane(&mut self, source: usize, pane: TabPane) {
        let (mut copy, name, scale, pan, discipline) = if source == self.active_tab {
            (
                self.project.clone(),
                title_of(&self.project, self.file_path.as_deref()),
                self.scale,
                self.pan,
                self.discipline,
            )
        } else if let Some(doc) = self.tab_document(source) {
            (
                doc.project.clone(),
                title_of(&doc.project, doc.file_path.as_deref()),
                doc.scale,
                doc.pan,
                doc.discipline,
            )
        } else {
            return;
        };
        copy.name = format!("{name} (cópia)");
        let mut document = Document::holding(copy, None, true);
        document.scale = scale;
        document.pan = pan;
        document.fit_pending = false;
        document.discipline = discipline;
        document.pane = if self.split_tab.is_some() {
            pane
        } else {
            TabPane::Left
        };
        self.tabs.push(Some(document));
        let index = self.tabs.len() - 1;
        self.switch_tab(index);
        self.recovery_pending = true;
        self.message = format!(
            "Cópia de \"{name}\" numa nova aba: use-a para outra disciplina do mesmo imóvel."
        );
    }

    /// Moves tab `index` to `target` pane when viewing side by side.
    pub(crate) fn move_tab_to_pane(&mut self, index: usize, target: TabPane) {
        if self.split_tab.is_none() || index >= self.tabs.len() || self.tab_pane(index) == target {
            return;
        }
        let source_pane = target.opposite();
        if index == self.active_tab {
            self.active_pane = target;
        } else if let Some(doc) = self.tabs[index].as_mut() {
            doc.pane = target;
        }
        let replacement =
            (0..self.tabs.len()).find(|&i| i != index && self.tab_pane(i) == source_pane);
        match replacement {
            None => {
                // Moving the last tab off a side closes the split cleanly.
                self.toggle_side_by_side(None);
            }
            Some(other) => {
                if index == self.active_tab || self.split_tab == Some(index) {
                    self.split_tab = Some(other);
                }
            }
        }
    }

    /// Asks to save when needed, then closes the tab at `index`.
    pub(crate) fn request_close_tab(&mut self, index: usize) {
        if index != self.active_tab {
            self.switch_tab(index);
        }
        if self.dirty {
            self.pending_action = Some(PendingAction::CloseTab);
        } else {
            self.close_active_tab();
        }
    }

    /// Closes the active tab without asking. The last tab turns blank.
    pub(crate) fn close_active_tab(&mut self) {
        self.settle_before_switch();
        recovery::clear();
        if self.tabs.len() == 1 {
            let before = std::mem::take(&mut self.project);
            self.put_document(Document::blank());
            self.split_tab = None;
            self.active_pane = TabPane::Left;
            self.fit_pending = false;
            self.note_plan_change(&before, true);
            self.sync_inspector_fields();
            self.sync_drafts();
            self.message = "Aba fechada.".to_owned();
            return;
        }
        let closed = self.active_tab;
        let closed_pane = self.active_pane;
        let before = std::mem::take(&mut self.project);
        self.tabs.remove(closed);

        let adjusted_split = match self.split_tab {
            Some(split) if split == closed => None,
            Some(split) if split > closed => Some(split - 1),
            other => other,
        };

        if let Some(other_visible) = adjusted_split {
            let same_pane = (0..self.tabs.len()).find(|&i| {
                i != other_visible
                    && self
                        .tab_document(i)
                        .is_some_and(|doc| doc.pane == closed_pane)
            });
            if let Some(index) = same_pane {
                let next = self.tabs[index]
                    .take()
                    .expect("an inactive tab holds its project");
                self.put_document(next);
                self.active_tab = index;
                self.split_tab = Some(other_visible);
            } else {
                // No tabs left on the closed side: collapse back to single view.
                let next = self.tabs[other_visible]
                    .take()
                    .expect("an inactive tab holds its project");
                self.put_document(next);
                self.active_tab = other_visible;
                self.split_tab = None;
                self.active_pane = TabPane::Left;
                for doc in self.tabs.iter_mut().flatten() {
                    doc.pane = TabPane::Left;
                }
            }
        } else {
            let index = closed.min(self.tabs.len() - 1);
            let next = self.tabs[index]
                .take()
                .expect("an inactive tab holds its project");
            self.put_document(next);
            self.active_tab = index;
            self.split_tab = None;
            self.active_pane = TabPane::Left;
            for doc in self.tabs.iter_mut().flatten() {
                doc.pane = TabPane::Left;
            }
        }

        self.note_plan_change(&before, true);
        self.sync_inspector_fields();
        self.sync_masonry_fields();
        self.sync_drafts();
        self.message = "Aba fechada.".to_owned();
    }

    /// Opens `path` in its own tab: the tab it is already open in, the
    /// active tab when that one is blank, or a new tab.
    pub(crate) fn open_in_tab(&mut self, path: &Path) -> OpenResult {
        let already = (0..self.tabs.len()).find(|index| {
            let file = if *index == self.active_tab {
                self.file_path.as_deref()
            } else {
                self.tab_document(*index)
                    .and_then(|d| d.file_path.as_deref())
            };
            file.is_some_and(|file| file == path)
        });
        if let Some(index) = already {
            self.switch_tab(index);
            self.message = "Esse projeto já estava aberto nesta aba.".to_owned();
            return OpenResult::Success;
        }
        let created = !self.active_is_blank();
        if created {
            let mut document = Document::blank();
            document.pane = if self.split_tab.is_some() {
                self.active_pane
            } else {
                TabPane::Left
            };
            self.tabs.push(Some(document));
            let index = self.tabs.len() - 1;
            self.switch_tab(index);
        }
        let result = self.open_file(path);
        if matches!(result, OpenResult::Error) && created {
            let message = self.message.clone();
            self.close_active_tab();
            self.message = message;
        }
        result
    }

    /// Shows `index` beside the active tab, or hides the side view.
    pub(crate) fn toggle_side_by_side(&mut self, index: Option<usize>) {
        // Each half keeps the same point of the building in its centre.
        self.last_canvas_center = None;
        let target =
            index.filter(|index| *index != self.active_tab && self.tab_document(*index).is_some());
        match target {
            Some(other) => {
                self.active_pane = TabPane::Left;
                for (i, slot) in self.tabs.iter_mut().enumerate() {
                    if let Some(doc) = slot {
                        doc.pane = if i == other {
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
                for doc in self.tabs.iter_mut().flatten() {
                    doc.pane = TabPane::Left;
                }
                self.message = "Vista única.".to_owned();
            }
        }
    }

    /// Keeps a crash-recovery draft of every tab with unsaved changes.
    pub(crate) fn sync_drafts(&mut self) {
        if self.dirty {
            self.recovery_pending = true;
        } else {
            recovery::clear();
        }
        let waiting: Vec<&Project> = self
            .tabs
            .iter()
            .flatten()
            .filter(|document| document.dirty)
            .map(|document| &document.project)
            .collect();
        if let Err(error) = recovery::save_tabs(&waiting) {
            self.message = format!("Não foi possível proteger o rascunho das abas: {error}");
        }
    }

    /// After saving or discarding the active tab while closing the window:
    /// shows the next tab with unsaved changes, or lets the window close.
    pub(crate) fn next_unsaved_before_closing(&mut self) -> bool {
        self.dirty = false;
        self.recovery_pending = false;
        recovery::clear();
        let next = (0..self.tabs.len()).find(|index| {
            self.tab_document(*index)
                .is_some_and(|document| document.dirty)
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

    /// Reopens the tabs whose drafts survived a crash.
    pub(crate) fn restore_tab_drafts(&mut self) {
        for project in recovery::load_tabs() {
            self.tabs.push(Some(Document::holding(project, None, true)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::FloorPlanApp;

    fn wall(app: &mut FloorPlanApp, y: f64) -> u64 {
        let previous = app.project.clone();
        let id = app
            .project
            .add_wall(Point::new(0.0, y), Point::new(4.0, y))
            .unwrap();
        app.record_change(previous);
        id
    }

    #[test]
    fn each_tab_keeps_its_project_history_selection_and_view() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let first_wall = wall(&mut app, 0.0);
        app.selection = Some(Selection::Wall(first_wall));
        app.scale = 90.0;

        app.new_tab();
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active_tab(), 1);
        assert!(app.project.walls.is_empty() && !app.dirty);
        wall(&mut app, 2.0);
        wall(&mut app, 4.0);

        app.switch_tab(0);
        assert_eq!(app.project.walls.len(), 1);
        assert_eq!(app.selection, Some(Selection::Wall(first_wall)));
        assert_eq!(app.scale, 90.0);
        assert!(app.dirty);
        // Undo stays with its tab.
        app.undo();
        assert!(app.project.walls.is_empty());
        app.switch_tab(1);
        assert_eq!(app.project.walls.len(), 2);
        app.undo();
        assert_eq!(app.project.walls.len(), 1);

        let infos = app.tab_infos();
        assert!(infos[1].active && !infos[0].active);
        assert!(app.any_tab_dirty());
    }

    #[test]
    fn the_new_tab_button_always_opens_a_tab() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        assert!(app.active_is_blank());
        app.new_tab();
        app.new_tab();
        assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    }

    #[test]
    fn copy_in_one_tab_and_paste_in_another() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let id = wall(&mut app, 0.0);
        app.selection = Some(Selection::Wall(id));
        app.copy_selection();
        app.new_tab();
        app.paste();
        assert_eq!(app.project.walls.len(), 1);
    }

    #[test]
    fn a_duplicate_tab_is_a_base_for_another_discipline() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project.name = "Casa".to_owned();
        wall(&mut app, 0.0);
        app.duplicate_tab();
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.project.name, "Casa (cópia)");
        assert_eq!(app.project.walls.len(), 1);
        assert!(app.dirty && app.file_path.is_none());
        // Side by side with the original, which then trades places on click.
        app.toggle_side_by_side(Some(0));
        assert_eq!(app.split_tab, Some(0));
        app.switch_tab(0);
        assert_eq!(app.split_tab, Some(1));
    }

    #[test]
    fn closing_asks_for_unsaved_tabs_and_keeps_the_rest() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        wall(&mut app, 0.0);
        app.new_tab();
        // A blank tab closes at once.
        app.request_close_tab(1);
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.project.walls.len(), 1);
        // An unsaved one asks first.
        app.request_close_tab(0);
        assert_eq!(app.pending_action, Some(PendingAction::CloseTab));
        app.discard_changes_and_proceed(&context);
        assert_eq!(app.tab_count(), 1);
        assert!(app.project.walls.is_empty() && !app.dirty);

        // Closing the window walks through every unsaved tab.
        wall(&mut app, 0.0);
        app.new_tab();
        assert!(!app.dirty && app.any_tab_dirty());
        app.request_close(&context);
        assert_eq!(app.pending_action, Some(PendingAction::CloseApp));
        assert_eq!(app.active_tab(), 0, "it shows the tab that needs saving");
    }

    #[test]
    fn opening_a_file_uses_a_blank_tab_or_a_new_one() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let folder = crate::io::data_dir().join("tabs-open-test");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("casa.trazo");
        let mut project = Project::default();
        project.name = "Casa".to_owned();
        crate::io::project_file::save(&project, &path).unwrap();

        // The blank first tab takes the file.
        assert!(matches!(app.open_in_tab(&path), OpenResult::Success));
        assert_eq!(app.tab_count(), 1);
        // Opening it again goes back to that tab instead of a second copy.
        app.new_tab();
        assert!(matches!(app.open_in_tab(&path), OpenResult::Success));
        assert_eq!((app.tab_count(), app.active_tab()), (2, 0));
        // A missing file leaves no empty tab behind.
        wall(&mut app, 0.0);
        let tabs = app.tab_count();
        assert!(matches!(
            app.open_in_tab(&folder.join("nada.trazo")),
            OpenResult::Error
        ));
        assert_eq!(app.tab_count(), tabs);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn split_view_keeps_per_pane_tabs_and_clamps_split_ratio() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.project.name = "Arquitetura".to_owned();
        app.new_tab();
        app.project.name = "Elétrica".to_owned();

        // Split with tab 0 on the right, active tab 1 on the left.
        app.toggle_side_by_side(Some(0));
        assert_eq!(app.active_pane(), TabPane::Left);
        assert_eq!(app.tab_pane(1), TabPane::Left);
        assert_eq!(app.tab_pane(0), TabPane::Right);

        // Adding a new tab on the right pane keeps it on the right and preserves the left tab.
        app.new_tab_in_pane(TabPane::Right);
        assert_eq!(app.tab_count(), 3);
        assert_eq!(app.active_tab(), 2);
        assert_eq!(app.active_pane(), TabPane::Right);
        assert_eq!(app.split_tab, Some(1));

        // Closing one right tab keeps the other right tab and stays split.
        app.close_active_tab();
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active_pane(), TabPane::Right);
        assert_eq!(app.split_tab, Some(1));

        // Closing the last right tab collapses back to single view cleanly.
        app.close_active_tab();
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.split_tab, None);
        assert_eq!(app.active_pane(), TabPane::Left);

        // Split ratio is safely clamped.
        app.set_split_ratio(0.05);
        assert!((app.split_ratio() - 0.20).abs() < 1e-6);
        app.set_split_ratio(0.95);
        assert!((app.split_ratio() - 0.80).abs() < 1e-6);
        app.set_split_ratio(0.62);
        assert!((app.split_ratio() - 0.62).abs() < 1e-6);
    }
}
