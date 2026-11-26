use super::*;
use crate::io::project_file;
use eframe::egui::Key;

fn press(app: &mut FloorPlanApp, context: &egui::Context, key: Key, modifiers: egui::Modifiers) {
    // Press and release, so a later press of the same key is not a repeat.
    let event = |pressed| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    context.begin_pass(egui::RawInput {
        events: vec![event(true), event(false)],
        ..Default::default()
    });
    app.handle_shortcuts(context);
    context.end_pass().textures_delta.clear();
}

#[test]
fn shortcuts_do_not_edit_the_project_behind_a_dialog() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let previous = app.project.clone();
    let id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    app.record_change(previous);
    app.selection = Some(Selection::Wall(id));
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };

    // "Save changes?" is open: Delete, Esc and Ctrl+Z leave everything alone.
    app.pending_action = Some(PendingAction::CloseTab);
    press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
    press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
    press(&mut app, &context, Key::Z, ctrl);
    press(&mut app, &context, Key::P, egui::Modifiers::NONE);
    assert_eq!(app.project.walls.len(), 1);
    assert_eq!(app.selection, Some(Selection::Wall(id)));
    assert_eq!(app.tool, Tool::Move);
    assert_eq!(app.pending_action, Some(PendingAction::CloseTab));

    // The help is open: editing keys wait, Esc closes it.
    app.pending_action = None;
    app.show_help = true;
    press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
    press(&mut app, &context, Key::Z, ctrl);
    assert_eq!(app.project.walls.len(), 1);
    press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
    assert!(!app.show_help);
    assert_eq!(app.selection, Some(Selection::Wall(id)));

    // With no dialog, the same keys work again.
    press(&mut app, &context, Key::Delete, egui::Modifiers::NONE);
    assert!(app.project.walls.is_empty());
    press(&mut app, &context, Key::Z, ctrl);
    assert_eq!(app.project.walls.len(), 1);
}

#[test]
fn renaming_the_project_is_protected_by_the_recovery_draft() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    assert!(!app.dirty && !app.recovery_pending);
    app.project.name = "Casa de praia".to_owned();
    app.mark_edited_outside_history();
    assert!(app.dirty);
    assert!(app.recovery_pending);
    assert!(!app.history.can_undo());
}

/// Starts dragging wall `id` and moves it 2 m to the right.
fn drag_wall(app: &mut FloorPlanApp, id: u64) {
    app.selection = Some(Selection::Wall(id));
    app.drag_initial_project = Some(app.project.clone());
    app.item_drag = Some(ItemDrag {
        selection: Selection::Wall(id),
        last: Point::new(2.0, 0.0),
        grab_offset: Point::new(0.0, 0.0),
        moved: true,
        group: false,
    });
    app.project.move_wall(id, Point::new(2.0, 0.0));
}

#[test]
fn switching_tools_mid_drag_keeps_the_move_undoable_and_unsaved() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    drag_wall(&mut app, id);
    press(&mut app, &context, Key::P, egui::Modifiers::NONE);
    assert_eq!(app.tool, Tool::Wall);
    assert!(app.dirty && app.recovery_pending);
    assert!(app.drag_initial_project.is_none() && app.item_drag.is_none());
    app.undo();
    assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
}

#[test]
fn deleting_mid_drag_undoes_the_delete_before_the_move() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    drag_wall(&mut app, id);
    app.delete_selection();
    assert!(app.project.walls.is_empty());
    app.undo();
    assert_eq!(app.project.walls[0].start, Point::new(2.0, 0.0));
    app.undo();
    assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
}

#[test]
fn arrow_keys_nudge_the_selection_as_undoable_steps() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    app.selection = Some(Selection::Wall(id));

    press(&mut app, &context, Key::ArrowRight, egui::Modifiers::NONE);
    assert_eq!(app.project.walls[0].start, Point::new(0.25, 0.0));
    press(&mut app, &context, Key::ArrowDown, egui::Modifiers::SHIFT);
    assert_eq!(app.project.walls[0].start, Point::new(0.25, 1.0));
    assert!(app.dirty);

    app.undo();
    assert_eq!(app.project.walls[0].start, Point::new(0.25, 0.0));
    app.undo();
    assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));

    // Nothing selected, or Ctrl held: the arrows do nothing.
    press(&mut app, &context, Key::ArrowRight, egui::Modifiers::CTRL);
    app.selection = None;
    press(&mut app, &context, Key::ArrowRight, egui::Modifiers::NONE);
    assert_eq!(app.project.walls[0].start, Point::new(0.0, 0.0));
}

#[test]
fn arrows_slide_openings_along_their_wall_only() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let wall = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
        .unwrap();
    let door = app
        .project
        .add_opening(wall, OpeningKind::Door, 0.5)
        .unwrap();
    app.selection = Some(Selection::Opening(door));

    app.nudge_selection(Point::new(1.0, 0.0));
    assert!((app.opening_center(door).unwrap().x - 4.0).abs() < 1e-3);
    let before = app.project.clone();
    app.nudge_selection(Point::new(0.0, 1.0));
    assert_eq!(app.project, before);
    assert!(app.message.contains("ao longo da parede"));
}

#[test]
fn nudging_stops_at_the_terrain_edge() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project
        .set_terrain(Point::new(0.0, 0.0), Point::new(5.0, 5.0));
    let id = app
        .project
        .add_wall(Point::new(0.0, 1.0), Point::new(3.0, 1.0))
        .unwrap();
    app.selection = Some(Selection::Wall(id));
    app.history.clear();
    app.dirty = false;
    app.nudge_selection(Point::new(-0.25, 0.0));
    assert_eq!(app.project.walls[0].start, Point::new(0.0, 1.0));
    assert!(!app.dirty && !app.history.can_undo());
    assert!(app.message.contains("terreno"));
}

#[test]
fn interface_preferences_are_tracked_for_saving() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    assert_eq!(app.current_settings(), Settings::default());
    app.toggle_grid();
    app.toggle_ortho();
    app.ribbon_collapsed = true;
    assert_ne!(app.current_settings(), app.saved_settings);
    app.persist_settings();
    assert_eq!(app.saved_settings, app.current_settings());
    assert!(!app.saved_settings.show_grid && app.saved_settings.ortho_enabled);
}

#[test]
fn doors_open_towards_the_click_and_flip_with_i() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
    // The leaf swings along (-dy, dx) = (0, 1) by default: below the wall.
    app.add_opening_at(Point::new(2.0, 0.3), OpeningKind::Door);
    assert!(!app.project.openings[0].flip_side);
    app.add_opening_at(Point::new(3.2, -0.3), OpeningKind::Window);
    app.project.openings.clear();
    app.add_opening_at(Point::new(2.0, -0.3), OpeningKind::Door);
    let door = app.project.openings[0].id;
    assert!(app.project.openings[0].flip_side);

    app.selection = Some(Selection::Opening(door));
    press(&mut app, &context, Key::I, egui::Modifiers::NONE);
    assert!(!app.project.openings[0].flip_side);
    press(&mut app, &context, Key::I, egui::Modifiers::SHIFT);
    assert!(app.project.openings[0].flip_hinge);
    app.undo();
    assert!(!app.project.openings[0].flip_hinge);
}

#[test]
fn letter_keys_pick_tools_and_escape_clears_the_selection() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    press(&mut app, &context, Key::P, egui::Modifiers::NONE);
    assert_eq!(app.tool, Tool::Wall);
    press(&mut app, &context, Key::J, egui::Modifiers::NONE);
    assert_eq!(app.tool, Tool::Opening(OpeningKind::Window));
    press(&mut app, &context, Key::V, egui::Modifiers::NONE);
    assert_eq!(app.tool, Tool::Move);

    let id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    app.selection = Some(Selection::Wall(id));
    press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
    assert_eq!(app.selection, None);
}

#[test]
fn ctrl_z_undoes_and_ctrl_y_restores_the_project() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let previous = app.project.clone();
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
    app.record_change(previous);
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    for (key, expected_walls) in [(Key::Z, 0), (Key::Y, 1)] {
        context.begin_pass(egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        });
        app.handle_shortcuts(&context);
        assert_eq!(app.project.walls.len(), expected_walls);
        context.end_pass().textures_delta.clear();
    }
}

#[test]
fn fit_view_shows_a_fifty_meter_plot() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project
        .set_terrain(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0));
    app.fit_view(rect);
    let view = canvas::ViewTransform {
        rect,
        scale: app.scale,
        pan: app.pan,
    };
    let a = view.to_screen(Point::new(0.0, 0.0));
    let b = view.to_screen(Point::new(50.0, 50.0));
    assert!(rect.contains(a) && rect.contains(b));
    assert!(app.scale > 3.0);
}

#[test]
fn terrain_dimension_input_accepts_portuguese_decimals() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.terrain_width_text = "5,5".to_owned();
    app.terrain_height_text = "10".to_owned();
    assert_eq!(app.terrain_dimensions(), Some((5.5, 10.0)));
    app.terrain_width_text = "0".to_owned();
    assert_eq!(app.terrain_dimensions(), None);
}

#[test]
fn terrain_created_after_room_contains_the_room() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.project
        .add_room(Point::new(-20.0, 2.0), Point::new(-10.0, 10.0), "Garagem");
    app.terrain_width_text = "10".to_owned();
    app.terrain_height_text = "50".to_owned();
    app.create_terrain_from_dimensions();
    assert!(app.project.content_inside_terrain());
}

#[test]
fn opening_inspector_does_not_shift_the_plan_on_screen() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(740.0, 700.0));
    app.prepare_canvas_view(wide);
    let before = canvas::ViewTransform {
        rect: wide,
        scale: app.scale,
        pan: app.pan,
    }
    .to_screen(Point::new(5.0, 10.0));
    app.prepare_canvas_view(narrow);
    let after = canvas::ViewTransform {
        rect: narrow,
        scale: app.scale,
        pan: app.pan,
    }
    .to_screen(Point::new(5.0, 10.0));
    assert_eq!(before, after);
}

#[test]
fn initial_tool_is_selection_move() {
    let context = egui::Context::default();
    let app = FloorPlanApp::new(&context);
    assert_eq!(app.tool, Tool::Move);
}

#[test]
fn a_new_plan_opens_in_a_tab_and_keeps_the_unsaved_one() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let prev = app.project.clone();
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
    app.record_change(prev);
    assert!(app.dirty);

    // Nothing is asked and nothing is lost: the new plan gets a tab.
    app.request_new_project();
    assert_eq!(app.pending_action, None);
    assert_eq!(app.tab_count(), 2);
    assert!(app.project.walls.is_empty() && !app.dirty);

    app.switch_tab(0);
    assert_eq!(app.project.walls.len(), 1);
    assert!(app.dirty);
}

#[test]
fn request_close_on_clean_project_closes_and_on_dirty_asks_confirmation() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    assert!(!app.dirty);

    app.request_close(&context);
    assert_eq!(app.pending_action, None);

    let prev = app.project.clone();
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
    app.record_change(prev);
    assert!(app.dirty);

    app.request_close(&context);
    assert_eq!(app.pending_action, Some(PendingAction::CloseApp));

    app.cancel_pending_action();
    assert_eq!(app.pending_action, None);
    assert!(app.dirty);
}

#[test]
fn failed_or_cancelled_open_preserves_dirty_state_and_project() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let prev = app.project.clone();
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0));
    app.record_change(prev);
    assert!(app.dirty);

    let invalid_path =
        std::env::temp_dir().join(format!("trazo-invalid-{}.json", std::process::id()));
    std::fs::write(&invalid_path, "not a valid json").unwrap();

    let result = app.open_file(&invalid_path);
    assert_eq!(result, OpenResult::Error);
    assert!(app.dirty);
    assert_eq!(app.project.walls.len(), 1);
    let _ = std::fs::remove_file(invalid_path);
}

#[test]
fn room_rename_preserves_walls_openings_and_works_with_shared_walls() {
    let mut project = Project::default();
    let r1 = project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    let _r2 = project
        .add_room(Point::new(4.0, 0.0), Point::new(8.0, 3.0), "Quarto")
        .unwrap();
    let r1_wall = project.rooms[0].wall_ids[0];
    let op_id = project
        .add_opening(r1_wall, OpeningKind::Door, 0.5)
        .unwrap();

    assert!(project.rename_room(r1, "Estar").unwrap());
    assert_eq!(project.rooms[0].display_name(), "Estar");
    assert_eq!(project.walls.len(), 7);
    assert_eq!(project.openings.len(), 1);
    assert_eq!(project.openings[0].id, op_id);
}

#[test]
fn room_resize_preserves_openings_and_wall_ids() {
    let mut project = Project::default();
    let r_id = project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Suíte")
        .unwrap();
    let bottom_wall_id = project.rooms[0].wall_ids[0];
    let door_id = project
        .add_opening(bottom_wall_id, OpeningKind::Door, 0.5)
        .unwrap();
    let orig_wall_ids = project.rooms[0].wall_ids.clone();

    assert!(project.resize_room_dimensions(r_id, 6.0, 5.0).is_ok());

    assert_eq!(project.rooms[0].wall_ids, orig_wall_ids);
    assert_eq!(project.openings.len(), 1);
    assert_eq!(project.openings[0].id, door_id);
    assert_eq!(project.openings[0].wall_id, bottom_wall_id);

    let res = project.resize_room_dimensions(r_id, 0.80, 5.0);
    assert!(res.is_err());
    let (min, max) = project.room_bounds(&project.rooms[0]).unwrap();
    assert!((max.x - min.x - 6.0).abs() < 1e-3);
}

#[test]
fn creation_flow_creates_elements_correctly() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);

    app.set_tool(Tool::Wall);
    app.complete_creation(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
    assert_eq!(app.project.walls.len(), 1);

    app.set_tool(Tool::Room);
    app.complete_creation(Point::new(0.0, 0.0), Point::new(3.0, 3.0));
    assert!(app.pending_room.is_some());
}

#[test]
fn wall_numerical_resize_updates_length() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let w_id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
        .unwrap();
    app.selection = Some(Selection::Wall(w_id));
    app.sync_inspector_fields();
    assert_eq!(app.wall_length_text, "4,00");

    app.wall_length_text = "6,50".to_owned();
    app.apply_wall_length(w_id);
    assert!((app.project.walls[0].length() - 6.5).abs() < 1e-3);
    assert!(app.dirty);

    // Undo reverts the length change
    app.undo();
    assert!((app.project.walls[0].length() - 4.0).abs() < 1e-3);
}

#[test]
fn room_numerical_resize_updates_dimensions() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let r_id = app
        .project
        .add_room(Point::new(0.0, 0.0), Point::new(3.0, 4.0), "Sala")
        .unwrap();
    app.selection = Some(Selection::Room(r_id));
    app.sync_inspector_fields();
    assert_eq!(app.room_width_text, "3,00");
    assert_eq!(app.room_height_text, "4,00");

    app.room_name_text = "Living".to_owned();
    app.room_width_text = "5,00".to_owned();
    app.room_height_text = "6,00".to_owned();
    app.apply_room_dimensions(r_id);

    let room = &app.project.rooms[0];
    assert_eq!(room.display_name(), "Living");
    assert!((app.project.room_area(room).unwrap() - 30.0).abs() < 1e-2);

    // Undo restores previous dimensions
    app.undo();
    let room = &app.project.rooms[0];
    assert_eq!(room.display_name(), "Sala");
    assert!((app.project.room_area(room).unwrap() - 12.0).abs() < 1e-2);
}

#[test]
fn opening_selection_width_update_and_deletion() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let w_id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0))
        .unwrap();
    let op_id = app
        .project
        .add_opening(w_id, OpeningKind::Door, 0.5)
        .unwrap();

    app.selection = Some(Selection::Opening(op_id));
    app.sync_inspector_fields();
    assert_eq!(app.opening_width_text, "0,90");

    // Custom width
    app.opening_width_text = "1,20".to_owned();
    app.apply_opening_width(op_id);
    assert_eq!(app.project.openings[0].width(), 1.20);

    // Delete opening
    app.delete_selection();
    assert_eq!(app.project.openings.len(), 0);
    assert_eq!(app.selection, None);

    // Undo restores opening
    app.undo();
    assert_eq!(app.project.openings.len(), 1);
    assert_eq!(app.project.openings[0].width(), 1.20);
}

#[test]
fn undo_revalidates_selection_when_item_no_longer_exists() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let prev = app.project.clone();
    let w_id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
        .unwrap();
    app.record_change(prev);

    app.selection = Some(Selection::Wall(w_id));
    assert!(app.selection.is_some());

    // Undo wall creation: wall disappears, selection must be cleared
    app.undo();
    assert_eq!(app.project.walls.len(), 0);
    assert_eq!(app.selection, None);

    // Redo restores wall
    app.redo();
    assert_eq!(app.project.walls.len(), 1);
}

#[test]
fn no_op_and_validation_errors_do_not_pollute_history_or_dirty() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let w_id = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
        .unwrap();
    app.dirty = false;
    app.history.clear();

    // 1. Same wall length: no-op, dirty stays false, history remains empty
    app.wall_length_text = "4,00".to_owned();
    app.apply_wall_length(w_id);
    assert!(!app.dirty);
    assert!(!app.history.can_undo());

    // 2. Invalid wall length: error, dirty stays false, history remains empty
    app.wall_length_text = "0,10".to_owned(); // below 0.25m min
    app.apply_wall_length(w_id);
    assert!(!app.dirty);
    assert!(!app.history.can_undo());
    assert!(app.message.contains("O comprimento deve estar entre"));

    // 3. Opening width tests
    let op_id = app
        .project
        .add_opening(w_id, OpeningKind::Door, 0.5)
        .unwrap();
    app.dirty = false;
    app.history.clear();

    // Same opening width
    app.opening_width_text = "0,90".to_owned();
    app.apply_opening_width(op_id);
    assert!(!app.dirty);
    assert!(!app.history.can_undo());

    // Invalid opening width (below min)
    app.opening_width_text = "0,20".to_owned();
    app.apply_opening_width(op_id);
    assert!(!app.dirty);
    assert!(!app.history.can_undo());
    assert!(app.message.contains("Valor fora dos limites permitidos"));

    // 4. Room dimensions no-op
    let r_id = app
        .project
        .add_room(Point::new(10.0, 10.0), Point::new(14.0, 14.0), "Quarto")
        .unwrap();
    app.dirty = false;
    app.history.clear();

    app.room_name_text = "Quarto".to_owned();
    app.room_width_text = "4,00".to_owned();
    app.room_height_text = "4,00".to_owned();
    app.apply_room_dimensions(r_id);
    assert!(!app.dirty);
    assert!(!app.history.can_undo());
}

#[test]
fn creation_state_flow_two_clicks_drag_esc_and_tool_switch() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);

    // Initial state
    assert_eq!(app.creation_state(), CreationState::Idle);
    assert!(app.draft_start().is_none());

    // 1. Tool switch resets creation state
    app.set_tool(Tool::Wall);
    app.creation_state = CreationState::WaitingSecondPoint {
        start: Point::new(1.0, 1.0),
    };
    assert_eq!(app.draft_start(), Some(Point::new(1.0, 1.0)));

    app.set_tool(Tool::Room);
    assert_eq!(app.creation_state(), CreationState::Idle);
    assert!(app.draft_start().is_none());

    // 2. Escape cancels creation
    app.set_tool(Tool::Wall);
    app.creation_state = CreationState::WaitingSecondPoint {
        start: Point::new(2.0, 2.0),
    };
    app.cancel_creation();
    assert_eq!(app.creation_state(), CreationState::Idle);
    assert!(app.draft_start().is_none());

    // 3. Two-click completion creates wall
    app.set_tool(Tool::Wall);
    let start = Point::new(0.0, 0.0);
    let end = Point::new(5.0, 0.0);
    app.creation_state = CreationState::WaitingSecondPoint { start };
    app.complete_creation(start, end);
    app.creation_state = CreationState::Idle;
    assert_eq!(app.project.walls.len(), 1);
    assert_eq!(app.creation_state(), CreationState::Idle);

    // 4. Drag completion creates wall
    app.creation_state = CreationState::Dragging {
        start: Point::new(0.0, 0.0),
    };
    let drag_end = Point::new(0.0, 4.0);
    app.complete_creation(Point::new(0.0, 0.0), drag_end);
    app.creation_state = CreationState::Idle;
    assert_eq!(app.project.walls.len(), 2);
    assert_eq!(app.creation_state(), CreationState::Idle);
}

#[test]
fn typing_distance_and_polar_during_drawing_creates_exact_walls() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    app.set_tool(Tool::Wall);

    // First point via absolute coordinates `1;2` (Y up, as the status
    // bar shows it: 2 m above the origin is -2 in plan coordinates).
    app.draft_input = "1;2".to_owned();
    app.commit_numeric_input();
    assert_eq!(app.draft_start(), Some(Point::new(1.0, -2.0)));
    assert!(app.draft_input.is_empty());

    // Second point via distance `3,47` aiming right.
    app.draft_aim = Some(Point::new(5.0, -2.0));
    app.draft_input = "3,47".to_owned();
    app.commit_numeric_input();
    assert_eq!(app.project.walls.len(), 1);
    assert!((app.project.walls[0].length() - 3.47).abs() < 1e-4);

    // Escape clears typed input before cancelling the draft.
    app.apply_canvas_point(Point::new(0.0, 0.0));
    app.draft_input = "4,5<45".to_owned();
    press(&mut app, &context, Key::Escape, egui::Modifiers::NONE);
    assert!(app.draft_input.is_empty());
    assert_eq!(app.draft_start(), Some(Point::new(0.0, 0.0)));
}

#[test]
fn wall_reference_shifts_walls_and_rooms_and_tab_cycles_it() {
    use crate::domain::wall_reference::WallReference;

    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let half = app.project.masonry.thickness() * 0.5;
    app.set_tool(Tool::Wall);
    assert_eq!(app.wall_reference, WallReference::Center);

    // Tab cycles Center -> External -> Internal -> Center.
    press(&mut app, &context, Key::Tab, egui::Modifiers::NONE);
    assert_eq!(app.wall_reference, WallReference::External);

    app.complete_creation(Point::new(0.0, 0.0), Point::new(4.0, 0.0));
    assert_eq!(app.project.walls.len(), 1);
    assert!((app.project.walls[0].start.y - half).abs() < 1e-4);
    assert!((app.project.walls[0].end.y - half).abs() < 1e-4);

    press(&mut app, &context, Key::Tab, egui::Modifiers::NONE);
    assert_eq!(app.wall_reference, WallReference::Internal);

    // Room referenced by Internal expands centreline by half thickness on each side.
    app.set_tool(Tool::Room);
    app.complete_creation(Point::new(1.0, 1.0), Point::new(4.0, 5.0));
    let corners = app.pending_room.as_ref().expect("pending room corners");
    assert!((corners[0].x - (1.0 - half)).abs() < 1e-4);
    assert!((corners[2].x - (4.0 + half)).abs() < 1e-4);
    assert!((corners[0].y - (1.0 - half)).abs() < 1e-4);
    assert!((corners[2].y - (5.0 + half)).abs() < 1e-4);
}

#[test]
fn startup_with_file_argument_loads_project() {
    let mut project = Project::default();
    project.name = "Projeto por duplo clique".to_owned();
    project
        .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
        .unwrap();
    let path =
        std::env::temp_dir().join(format!("trazo-startup-open-{}.trazo", std::process::id()));
    project_file::save(&project, &path).unwrap();

    let context = egui::Context::default();
    let app = FloorPlanApp::new_with_file(&context, Some(path.clone()));
    assert_eq!(app.project.name, "Projeto por duplo clique");
    assert_eq!(app.project.walls.len(), 1);
    assert_eq!(app.file_path.as_deref(), Some(path.as_path()));
    assert!(!app.dirty);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn ctrl_p_opens_exact_scale_print_dialog() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    assert_eq!(app.active_doc_dialog, DocDialog::None);
    press(&mut app, &context, Key::P, egui::Modifiers::CTRL);
    assert_eq!(app.active_doc_dialog, DocDialog::Print);
}

#[test]
fn opening_trazot_template_creates_untitled_project_and_compressed_trazo_embeds_thumbnail() {
    let mut template = Project::default();
    template.name = "Modelo Padrão Escritório".to_owned();
    template
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    let tpl_guid = template.project_guid();

    let dir = std::env::temp_dir().join(format!("trazo-t06-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tpl_path = dir.join("padrao.trazot");
    let proj_path = dir.join("obra.trazo");

    project_file::save(&template, &tpl_path).unwrap();
    let raw_tpl = std::fs::read(&tpl_path).unwrap();
    assert!(crate::io::container::is_zip(&raw_tpl));
    assert!(crate::io::container::read_thumbnail(&tpl_path).is_some());

    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    assert_eq!(app.open_file(&tpl_path), OpenResult::Success);
    assert_eq!(
        app.file_path, None,
        "opening .trazot must leave file_path empty"
    );
    assert!(!app.dirty);
    assert_eq!(app.project.rooms.len(), 1);
    assert_ne!(
        app.project.project_guid(),
        tpl_guid,
        "new project from template must get a fresh project GUID"
    );
    assert_eq!(app.user_templates.shown().len(), 1);
    assert!(app.recent_files.entries.is_empty());

    // Saving as `.trazo` writes a compressed ZIP with thumbnail and records in recents.
    assert!(app.write_project(proj_path.clone()));
    assert!(crate::io::container::read_thumbnail(&proj_path).is_some());
    assert_eq!(app.recent_files.entries.len(), 1);

    // Start Page recovery actions work cleanly.
    app.pending_recovery = Some(template.clone());
    app.show_start_page = true;
    app.defer_pending_draft();
    assert!(app.pending_recovery.is_none());
    assert!(app.show_start_page);

    app.pending_recovery = Some(template);
    app.recover_pending_draft();
    assert!(app.pending_recovery.is_none());
    assert!(!app.show_start_page);
    assert!(app.dirty);
    assert_eq!(app.project.name, "Modelo Padrão Escritório");

    let _ = std::fs::remove_dir_all(&dir);
}
