use super::*;
use crate::app::{FloorPlanApp, OpenResult, PendingAction};

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
    // Undo stays with its project.
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
fn linked_tabs_share_single_project_history_and_live_edits() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project.name = "Residência".to_owned();
    app.set_discipline(Discipline::Architecture);
    let w = wall(&mut app, 0.0);

    let elec_idx = app.new_linked_tab(Discipline::Electrical);
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.tab_project_id(0), app.tab_project_id(elec_idx));
    assert!(app.tab_infos()[0].linked && app.tab_infos()[1].linked);

    // Editing in Architecture immediately reflects in Electrical (and live split view).
    app.toggle_side_by_side(Some(0));
    app.switch_tab(0);
    let prev = app.project.clone();
    app.project
        .move_wall_checked(w, Point::new(0.0, 1.5))
        .unwrap();
    app.record_change(prev);

    // Side view reads live shared model!
    let side = app.side_plan_view(elec_idx).unwrap();
    assert!(side.linked_to_active);
    assert_eq!(side.discipline, Discipline::Electrical);
    assert!((side.base_project.walls[0].start.y - 1.5).abs() < 1e-4);

    // Switching to Electrical tab sees the exact same moved wall, and Undo
    // in Electrical reverts it in Architecture too!
    app.switch_tab(elec_idx);
    assert!((app.project.walls[0].start.y - 1.5).abs() < 1e-4);
    app.undo();
    assert!((app.project.walls[0].start.y - 0.0).abs() < 1e-4);
    app.switch_tab(0);
    assert!((app.project.walls[0].start.y - 0.0).abs() < 1e-4);

    // Closing one linked view does NOT prompt or discard unsaved data.
    app.request_close_tab(elec_idx);
    assert_eq!(app.pending_action, None);
    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.project.walls.len(), 1);
    assert!(app.dirty);

    // Closing the LAST link to the dirty project DOES prompt.
    app.request_close_tab(0);
    assert_eq!(app.pending_action, Some(PendingAction::CloseTab));
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
fn a_duplicate_tab_is_an_independent_copy() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project.name = "Casa".to_owned();
    wall(&mut app, 0.0);
    app.duplicate_tab();
    assert_eq!(app.tab_count(), 2);
    assert_ne!(app.tab_project_id(0), app.tab_project_id(1));
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

    // Closing the window walks through every unsaved project once.
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

#[test]
fn inactive_split_pane_respects_its_tab_discipline() {
    use crate::domain::layers::LayerCategory;

    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.discipline = Discipline::Structure;
    app.new_linked_tab(Discipline::Electrical);
    app.toggle_side_by_side(Some(0));

    let (shown, discipline) = app.side_plan_discipline_project(0).unwrap();
    assert_eq!(discipline, Discipline::Structure);
    assert!(shown.category_visible(LayerCategory::Structure));
    assert!(!shown.category_visible(LayerCategory::Electrical));
    assert!(!shown.category_visible(LayerCategory::Fixtures));
}
