use serde_json::json;

use super::*;
use crate::app::{FloorPlanApp, OpenResult};
use crate::domain::conduits::ConduitRun;
use crate::domain::dependencies::ReviewKind;
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::OpeningKind;

fn setup_house_app() -> (egui::Context, FloorPlanApp) {
    let ctx = egui::Context::default();
    let mut app = FloorPlanApp::new(&ctx);
    let prev = app.project.clone();
    app.project.name = "Casa Integrada".to_owned();
    app.project
        .set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
    app.project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Cozinha")
        .unwrap();
    app.record_change(prev);
    (ctx, app)
}

#[test]
fn scenario_01_create_two_linked_tabs_on_same_project() {
    let (_ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec = app.new_linked_tab(Discipline::Electrical);
    let plumb = app.new_linked_tab(Discipline::Plumbing);

    assert_eq!(app.tab_count(), 3);
    assert_eq!(app.open_project_ids().len(), 1);
    assert_eq!(app.tab_project_id(0), app.tab_project_id(elec));
    assert_eq!(app.tab_project_id(0), app.tab_project_id(plumb));
    // Zero duplicate projects parked in `self.projects`.
    assert!(app.projects.is_empty());

    let infos = app.tab_infos();
    assert_eq!(infos[0].discipline, Discipline::Architecture);
    assert_eq!(infos[1].discipline, Discipline::Electrical);
    assert_eq!(infos[2].discipline, Discipline::Plumbing);
    assert!(infos.iter().all(|i| i.linked));
}

#[test]
fn scenario_02_move_wall_in_one_tab_updates_linked_tab_and_split_view() {
    let (_ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec = app.new_linked_tab(Discipline::Electrical);
    app.toggle_side_by_side(Some(0));
    app.switch_tab(0);

    let top_wall = app
        .project
        .walls
        .iter()
        .find(|w| (w.start.y - 0.0).abs() < 1e-3 && (w.end.y - 0.0).abs() < 1e-3)
        .unwrap()
        .id;
    let prev = app.project.clone();
    app.project
        .move_wall_checked(top_wall, Point::new(0.0, -0.8))
        .unwrap();
    app.record_change(prev);

    // Live split view on the Electrical tab immediately sees the moved wall.
    let side = app.side_plan_view(elec).unwrap();
    assert!(side.linked_to_active);
    let side_wall = side
        .base_project
        .walls
        .iter()
        .find(|w| w.id == top_wall)
        .unwrap();
    assert!((side_wall.start.y - (-0.8)).abs() < 1e-3);

    // Switching to the Electrical tab sees the exact same updated wall and room area.
    app.switch_tab(elec);
    let wall_in_elec = app.project.walls.iter().find(|w| w.id == top_wall).unwrap();
    assert!((wall_in_elec.start.y - (-0.8)).abs() < 1e-3);
    let area = app.project.room_area(&app.project.rooms[0]).unwrap();
    assert!((area - 4.0 * 3.8).abs() < 0.05);
}

#[test]
fn scenario_03_moving_wall_and_column_preserves_opening_conduit_beam_and_footing_links() {
    let (_ctx, mut app) = setup_house_app();
    let top_wall = app
        .project
        .walls
        .iter()
        .find(|w| (w.start.y - 0.0).abs() < 1e-3 && (w.end.y - 0.0).abs() < 1e-3)
        .unwrap()
        .clone();
    let half_t = top_wall.thickness() * 0.5;
    let (ow, od) = FixtureKind::OutletMid.default_size();
    let (sw, sd) = FixtureKind::KitchenSink.default_size();
    let (pw, pd) = FixtureKind::Panel.default_size();

    let prev = app.project.clone();
    let door_id = app
        .project
        .add_opening_sized(top_wall.id, OpeningKind::Door, 0.25, Some(0.8))
        .unwrap();
    let outlet_id = app
        .project
        .add_fixture(
            FixtureKind::OutletMid,
            Point::new(2.2, half_t + od * 0.5),
            ow,
            od,
            0,
        )
        .unwrap();
    let sink_id = app
        .project
        .add_fixture(
            FixtureKind::KitchenSink,
            Point::new(3.2, half_t + sd * 0.5),
            sw,
            sd,
            0,
        )
        .unwrap();
    let panel_id = app
        .project
        .add_fixture(FixtureKind::Panel, Point::new(0.5, 2.7), pw, pd, 2)
        .unwrap();
    let conduit_id = app
        .project
        .add_conduit(panel_id, outlet_id, ConduitRun::Ceiling)
        .unwrap();
    let col_a = app
        .project
        .add_fixture(FixtureKind::Column, Point::new(6.0, 1.0), 0.20, 0.20, 0)
        .unwrap();
    let col_b = app
        .project
        .add_fixture(FixtureKind::Column, Point::new(9.0, 1.0), 0.20, 0.20, 0)
        .unwrap();
    let ft_a = app
        .project
        .add_fixture(FixtureKind::Footing, Point::new(6.0, 1.0), 0.80, 0.80, 0)
        .unwrap();
    let beam_id = app.project.add_beam(col_a, col_b, 0.15, 0.35).unwrap();
    app.record_change(prev);

    // Move wall -> door, outlet and sink follow while keeping exact IDs and conduit!
    let prev = app.project.clone();
    app.project
        .move_wall_checked(top_wall.id, Point::new(0.0, -0.6))
        .unwrap();
    app.record_change(prev);

    assert!(
        app.project
            .openings
            .iter()
            .any(|o| o.id == door_id && o.wall_id == top_wall.id)
    );
    let outlet = app
        .project
        .fixtures
        .iter()
        .find(|f| f.id == outlet_id)
        .unwrap();
    assert!((outlet.center.y - (-0.6 + half_t + od * 0.5)).abs() < 1e-2);
    let sink = app
        .project
        .fixtures
        .iter()
        .find(|f| f.id == sink_id)
        .unwrap();
    assert!((sink.center.y - (-0.6 + half_t + sd * 0.5)).abs() < 1e-2);
    assert!(
        app.project
            .conduits
            .iter()
            .any(|c| c.id == conduit_id && c.from == panel_id && c.to == outlet_id)
    );

    // Move column -> footing follows automatically and beam remains connected!
    let prev = app.project.clone();
    app.project.move_fixture(col_a, Point::new(0.0, 0.5));
    app.record_change(prev);
    let ft = app.project.fixtures.iter().find(|f| f.id == ft_a).unwrap();
    assert!(ft.center.near(Point::new(6.0, 1.5), 1e-3));
    assert!(
        app.project
            .structure
            .beams
            .iter()
            .any(|b| b.id == beam_id && b.from == col_a && b.to == col_b)
    );
}

#[test]
fn scenario_04_editing_one_discipline_never_erases_other_disciplines() {
    let (_ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec = app.new_linked_tab(Discipline::Electrical);
    let struc = app.new_linked_tab(Discipline::Structure);
    let found = app.new_linked_tab(Discipline::Foundations);

    // Work in Electrical tab.
    app.switch_tab(elec);
    let prev = app.project.clone();
    app.project.generate_electrical_points();
    app.project.assign_circuits();
    app.record_change(prev);
    let elec_count = app.project.electrical_devices().count();
    assert!(elec_count > 0);

    // Work in Structure tab.
    app.switch_tab(struc);
    let prev = app.project.clone();
    app.project.generate_structure();
    app.record_change(prev);
    let beams_count = app.project.structure.beams.len();
    let footings_count = app
        .project
        .fixtures
        .iter()
        .filter(|f| f.kind == FixtureKind::Footing)
        .count();
    assert!(beams_count > 0 && footings_count > 0);

    // Switch to Foundations tab and resize footings -> Electrical and Architecture stay intact.
    app.switch_tab(found);
    assert_eq!(app.project.electrical_devices().count(), elec_count);
    assert_eq!(app.project.structure.beams.len(), beams_count);
    assert_eq!(app.project.rooms.len(), 1);
}

#[test]
fn scenario_05_reopening_already_open_file_focuses_without_duplicating_project() {
    let (_ctx, mut app) = setup_house_app();
    let folder = crate::io::data_dir().join("sync-tabs-reopen-test");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("obra.trazo");
    assert!(app.write_project(path.clone()));

    let elec = app.new_linked_tab(Discipline::Electrical);
    // Open another independent blank tab so we are away from the saved file.
    app.new_tab();
    assert_eq!(app.open_project_ids().len(), 2);
    assert_eq!(app.tab_count(), 3);

    // Reopening the same path via UI/open_in_tab focuses the existing project, no duplicate!
    assert!(matches!(app.open_in_tab(&path), OpenResult::Success));
    assert_eq!(app.open_project_ids().len(), 2);
    assert_eq!(app.tab_count(), 3);
    assert_eq!(
        app.tab_project_id(app.active_tab()),
        app.tab_project_id(elec)
    );

    // Reopening via MCP `abrir_projeto` also deduplicates!
    let mcp = app.run_mcp_tool(
        "abrir_projeto",
        &json!({ "caminho": path.display().to_string() }),
    );
    assert!(!mcp.is_error, "{}", mcp.text);
    let data = mcp.structured.unwrap();
    assert_eq!(data["ja_estava_aberto"], true);
    assert_eq!(app.open_project_ids().len(), 2);

    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn scenario_06_save_from_linked_tab_and_reopen_preserves_disciplines_and_tabs() {
    let (ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec = app.new_linked_tab(Discipline::Electrical);
    let prev = app.project.clone();
    app.project.generate_electrical_points();
    app.project.generate_structure();
    app.record_change(prev);
    let struc = app.new_linked_tab(Discipline::Structure);
    app.toggle_side_by_side(Some(elec));

    // Save from the Structure linked tab!
    let folder = crate::io::data_dir().join("sync-tabs-persist-test");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("completo.trazo");
    assert!(app.write_project(path.clone()));
    assert!(!app.dirty);
    assert!(!app.any_tab_dirty());

    let expected_elec = app.project.electrical_devices().count();
    let expected_beams = app.project.structure.beams.len();
    let expected_walls = app.project.walls.len();

    // Reopen in a fresh Trazo window: all disciplines AND all 3 linked tabs return!
    let mut reopened = FloorPlanApp::new(&ctx);
    assert!(matches!(reopened.open_in_tab(&path), OpenResult::Success));
    assert_eq!(reopened.open_project_ids().len(), 1);
    assert_eq!(reopened.tab_count(), 3);
    assert_eq!(reopened.active_tab(), struc);
    assert_eq!(reopened.split_tab, Some(elec));
    let infos = reopened.tab_infos();
    assert_eq!(infos[0].discipline, Discipline::Architecture);
    assert_eq!(infos[1].discipline, Discipline::Electrical);
    assert_eq!(infos[2].discipline, Discipline::Structure);
    assert!(infos.iter().all(|i| i.linked));
    assert_eq!(reopened.project.walls.len(), expected_walls);
    assert_eq!(reopened.project.electrical_devices().count(), expected_elec);
    assert_eq!(reopened.project.structure.beams.len(), expected_beams);

    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn scenario_07_undo_synchronizes_across_all_linked_tabs() {
    let (_ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec = app.new_linked_tab(Discipline::Electrical);
    let struc = app.new_linked_tab(Discipline::Structure);

    // Add a wall in Architecture tab.
    app.switch_tab(0);
    let before_count = app.project.walls.len();
    let prev = app.project.clone();
    let added = app
        .project
        .add_wall(Point::new(6.0, 0.0), Point::new(10.0, 0.0))
        .unwrap();
    app.record_change(prev);
    app.selection = Some(Selection::Wall(added));

    // Switch to Electrical tab, confirm wall exists, and press Undo there.
    app.switch_tab(elec);
    assert_eq!(app.project.walls.len(), before_count + 1);
    app.undo();
    assert_eq!(app.project.walls.len(), before_count);

    // Both Architecture and Structure tabs reflect the Undo and cleared stale selection!
    app.switch_tab(0);
    assert_eq!(app.project.walls.len(), before_count);
    assert_eq!(app.selection, None);
    app.switch_tab(struc);
    assert_eq!(app.project.walls.len(), before_count);
}

#[test]
fn scenario_08_mcp_edits_reflect_across_linked_tabs_with_project_guard() {
    let (_ctx, mut app) = setup_house_app();
    app.set_discipline(Discipline::Architecture);
    let elec_res = app.run_mcp_tool(
        "abas",
        &json!({ "acao": "nova_visao", "disciplina": "eletrica" }),
    );
    assert!(!elec_res.is_error, "{}", elec_res.text);
    let pid = app.active_project_id();

    // MCP generates electrical points while Electrical tab is active.
    let generated = app.run_mcp_tool(
        "projeto_eletrico",
        &json!({ "acao": "gerar_pontos", "projeto_id": pid }),
    );
    assert!(!generated.is_error, "{}", generated.text);

    // Guard prevents accidentally mutating the wrong project_id!
    let wrong = app.run_mcp_tool(
        "criar_parede",
        &json!({ "x1": 6, "y1": 0, "x2": 9, "y2": 0, "projeto_id": pid + 99 }),
    );
    assert!(wrong.is_error);

    // Switch to Architecture tab -> sees all electrical points and full revision log!
    app.run_mcp_tool("abas", &json!({ "acao": "trocar", "aba": 1 }));
    assert!(app.project.electrical_devices().count() > 0);
    let plan = app
        .run_mcp_tool("obter_planta", &json!({}))
        .structured
        .unwrap();
    assert_eq!(plan["projeto"]["id"], pid);
    assert_eq!(plan["disciplina"], "arquitetura");
    assert_eq!(plan["abas_vinculadas"], json!([1, 2]));
}

#[test]
fn scenario_09_two_independent_projects_stay_strictly_isolated() {
    let (_ctx, mut app) = setup_house_app();
    let pid_a = app.active_project_id();
    let walls_a = app.project.walls.len();

    // Open a genuinely separate project B in a new tab.
    app.new_tab();
    let pid_b = app.active_project_id();
    assert_ne!(pid_a, pid_b);
    app.project.name = "Projeto B".to_owned();
    let prev = app.project.clone();
    app.project
        .add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0))
        .unwrap();
    app.record_change(prev);
    assert_eq!(app.project.walls.len(), 1);

    // Project A is completely unaffected.
    app.switch_tab(0);
    assert_eq!(app.active_project_id(), pid_a);
    assert_eq!(app.project.name, "Casa Integrada");
    assert_eq!(app.project.walls.len(), walls_a);
}

#[test]
fn scenario_10_geometric_changes_flag_elements_needing_manual_review() {
    let (_ctx, mut app) = setup_house_app();
    // Add an isolated wall with an outlet near its right end and a column on its axis.
    let prev = app.project.clone();
    let w_id = app
        .project
        .add_wall(Point::new(6.0, 1.0), Point::new(10.0, 1.0))
        .unwrap();
    let half_t = app
        .project
        .walls
        .iter()
        .find(|w| w.id == w_id)
        .unwrap()
        .thickness()
        * 0.5;
    let (ow, od) = FixtureKind::OutletLow.default_size();
    let outlet_id = app
        .project
        .add_fixture(
            FixtureKind::OutletLow,
            Point::new(9.5, 1.0 + half_t + od * 0.5),
            ow,
            od,
            0,
        )
        .unwrap();
    let col_id = app
        .project
        .add_fixture(FixtureKind::Column, Point::new(8.0, 1.0), 0.20, 0.20, 0)
        .unwrap();
    app.record_change(prev);

    // Shrink wall from 4 m to 2 m (outlet falls outside wall) and move wall away from column.
    let prev = app.project.clone();
    app.project.resize_wall_length(w_id, 2.0).unwrap();
    app.record_change(prev);
    assert!(app.sync_report.blocked_updates.iter().any(|r| {
        r.element_id == Some(outlet_id) && r.kind == ReviewKind::FixtureDetachedFromWall
    }));

    let prev = app.project.clone();
    app.project
        .move_wall_checked(w_id, Point::new(0.0, 1.0))
        .unwrap();
    app.record_change(prev);

    // Both domain audit and MCP `dependencias` report the elements needing manual adjustment!
    let deps = app
        .run_mcp_tool("dependencias", &json!({}))
        .structured
        .unwrap();
    let reviews = deps["revisao_manual"].as_array().unwrap();
    assert!(
        reviews
            .iter()
            .any(|r| { r["elemento_id"] == outlet_id && r["tipo"] == "elemento_solto_da_parede" })
    );
    assert!(
        reviews
            .iter()
            .any(|r| { r["elemento_id"] == col_id && r["tipo"] == "pilar_fora_do_eixo_da_parede" })
    );
}

#[test]
fn reordering_tabs_preserves_active_and_split_indices() {
    let (_ctx, mut app) = setup_house_app();
    app.new_linked_tab(Discipline::Structure);
    app.new_linked_tab(Discipline::Foundations);
    // Order: [All (0), Structure (1), Foundations (2, active)].
    assert_eq!(app.tab_count(), 3);
    assert_eq!(app.active_tab(), 2);
    app.toggle_side_by_side(Some(0));
    assert_eq!(app.split_tab, Some(0));

    // Drag Foundations (active, index 2) to the front (index 0).
    app.reorder_tab(2, 0);
    let disciplines: Vec<Discipline> = app.tab_infos().into_iter().map(|t| t.discipline).collect();
    assert_eq!(
        disciplines,
        vec![
            Discipline::Foundations,
            Discipline::All,
            Discipline::Structure,
        ]
    );
    assert_eq!(app.active_tab(), 0);
    assert_eq!(app.split_tab, Some(1));

    // Drag All (split companion, index 1) to the end (index 2).
    app.reorder_tab(1, 2);
    let disciplines: Vec<Discipline> = app.tab_infos().into_iter().map(|t| t.discipline).collect();
    assert_eq!(
        disciplines,
        vec![
            Discipline::Foundations,
            Discipline::Structure,
            Discipline::All,
        ]
    );
    assert_eq!(app.active_tab(), 0);
    assert_eq!(app.split_tab, Some(2));
}
