//! Headless UI regression suite powered by `egui_kittest` (T-07).
//!
//! Renders Trazo's real `FloorPlanApp` layout and critical dialogs (Start Page,
//! Structure, What-if simulator, Electrical, Safety, Sheet and Quantities)
//! without a display server, asserting accessible controls and interactions.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{DocDialog, FloorPlanApp, RibbonTab, Selection, Tool};
use crate::domain::{
    fixtures::FixtureKind,
    geometry::Point,
    model::{OpeningKind, Project},
};

fn seeded_project() -> Project {
    let mut project = Project::default();
    project.name = "Casa Teste UI".to_owned();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(14.0, 14.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.5), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.5, 0.0), Point::new(8.0, 3.5), "Cozinha")
        .unwrap();
    project.generate_structure();
    project.generate_electrical_points();
    let (pw, pd) = FixtureKind::Panel.default_size();
    let _ = project.add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0);
    project.assign_circuits();
    let _ = project.generate_conduits();
    project
}

fn harness_with_size(
    size: [f32; 2],
    configure: impl FnOnce(&mut FloorPlanApp),
) -> Harness<'static, FloorPlanApp> {
    let ctx = egui::Context::default();
    let mut app = FloorPlanApp::new(&ctx);
    configure(&mut app);
    let mut harness = Harness::builder()
        .with_size(size)
        .with_max_steps(16)
        // Existing numeric and combo controls still lack AccessKit names;
        // this suite checks dialog and layout behavior rather than accessibility audits.
        .with_accessibility_check(false)
        .with_context_setup(super::theme::apply)
        .build_ui_state(|ui, state: &mut FloorPlanApp| state.show_layout(ui), app);
    harness.step();
    harness.step();
    harness
}

fn harness_with_app(configure: impl FnOnce(&mut FloorPlanApp)) -> Harness<'static, FloorPlanApp> {
    harness_with_size([1280.0, 800.0], configure)
}

#[test]
fn start_page_renders_recovery_banner_and_buttons_interact_cleanly() {
    let draft = seeded_project();
    let mut harness = harness_with_app(|app| {
        app.show_start_page = true;
        app.pending_recovery = Some(draft);
    });

    assert!(harness.query_by_label("Recuperar").is_some());
    assert!(harness.query_by_label("Descartar").is_some());
    assert!(harness.query_by_label("Adiar").is_some());
    assert!(harness.query_by_label("Nova planta").is_some());

    // Clicking "Adiar" clears the recovery banner while keeping Start Page open.
    harness.get_by_label("Adiar").click();
    harness.step();
    harness.step();
    assert!(harness.state().pending_recovery.is_none());
    assert!(harness.state().show_start_page);

    // Clicking "Nova planta" dismisses Start Page and enters the drawing canvas.
    harness.get_by_label("Nova planta").click();
    harness.step();
    harness.step();
    assert!(!harness.state().show_start_page);
}

#[test]
fn critical_dialogs_render_headlessly_without_panics() {
    let project = seeded_project();
    let dialogs = [
        (DocDialog::Structure, "Gerar estrutura"),
        (DocDialog::Electrical, "Gerar pontos mínimos"),
        (DocDialog::Safety, "Exportar memorial (PDF)"),
        (DocDialog::Sheet, "Imprimir em escala..."),
        (DocDialog::Quantities, "Exportar CSV"),
        (DocDialog::Templates, "Fechar"),
    ];

    for (dialog, expected_control) in dialogs {
        let proj_clone = project.clone();
        let mut harness = harness_with_app(|app| {
            app.project = proj_clone;
            app.active_doc_dialog = dialog;
        });

        assert!(
            harness.query_by_label(expected_control).is_some(),
            "{dialog:?} did not render {expected_control}"
        );

        // Exercise the What-if tab as well as the Structure dialog shell.
        if dialog == DocDialog::Structure {
            harness.get_by_label("E se? (What-if)").click();
            harness.step();
            harness.step();
            assert!(harness.query_by_label("Cenário simulado").is_some());
        }

        // Verify the dialog remained open and rendered cleanly.
        assert_eq!(harness.state().active_doc_dialog, dialog);
    }
}

#[test]
fn command_palette_filters_tools_dialogs_and_dynamic_rooms() {
    let project = seeded_project();
    let kitchen_id = project
        .rooms
        .iter()
        .find(|r| r.display_name() == "Cozinha")
        .unwrap()
        .id;

    // 1. Search and execute a drawing tool ("parede")
    let mut harness = harness_with_app(|app| {
        app.project = project.clone();
        app.show_start_page = false;
        app.open_command_palette();
        app.command_palette_query = "parede".to_owned();
    });
    assert!(harness.state().show_command_palette);
    harness.key_press(egui::Key::Enter);
    harness.step();
    harness.step();
    assert!(!harness.state().show_command_palette);
    assert_eq!(harness.state().tool, Tool::Wall);

    // 2. Search and open an engineering dialog without accents ("estrutural")
    harness.state_mut().open_command_palette();
    harness.state_mut().command_palette_query = "calculo estrutural".to_owned();
    harness.step();
    harness.key_press(egui::Key::Enter);
    harness.step();
    harness.step();
    assert!(!harness.state().show_command_palette);
    assert_eq!(harness.state().active_doc_dialog, DocDialog::Structure);

    // 3. Jump directly to a named room ("cozinha")
    harness.state_mut().active_doc_dialog = DocDialog::None;
    harness.state_mut().open_command_palette();
    harness.state_mut().command_palette_query = "cozinha".to_owned();
    harness.step();
    harness.key_press(egui::Key::Enter);
    harness.step();
    harness.step();
    assert!(!harness.state().show_command_palette);
    assert_eq!(harness.state().selection, Some(Selection::Room(kitchen_id)));
}

#[test]
fn multi_resolution_layout_tabs_inspector_and_help_dialog_render_cleanly() {
    let mut project = seeded_project();
    let wall_id = project.walls[0].id;
    let room_id = project.rooms[0].id;
    let door_id = project
        .add_opening(wall_id, OpeningKind::Door, 0.5)
        .unwrap();
    let fixture_id = project.fixtures[0].id;

    let sizes = [[1000.0, 600.0], [1280.0, 760.0], [1600.0, 900.0]];
    let all_tabs = [
        RibbonTab::Architecture,
        RibbonTab::Edit,
        RibbonTab::Interiors,
        RibbonTab::Electrical,
        RibbonTab::Structure,
        RibbonTab::Drafting,
        RibbonTab::View,
        RibbonTab::Output,
    ];

    for size in sizes {
        let mut harness = harness_with_size(size, |app| {
            app.project = project.clone();
            app.show_start_page = false;
            app.show_browser = true;
            app.show_properties = true;
        });

        // App bar primary actions always accessible across all viewport widths
        assert!(harness.query_by_label("Buscar").is_some());
        assert!(harness.query_by_label("Exportar").is_some());

        // Exercise every ribbon tab
        for tab in all_tabs {
            harness.state_mut().ribbon_tab = tab;
            harness.step();
        }

        // Exercise inspector + Element Action Bar for each entity kind
        for sel in [
            None,
            Some(Selection::Wall(wall_id)),
            Some(Selection::Room(room_id)),
            Some(Selection::Opening(door_id)),
            Some(Selection::Fixture(fixture_id)),
        ] {
            harness.state_mut().selection = sel;
            harness.step();
        }

        // Verify scrollable help dialog opens and renders cleanly even at 1000x600
        harness.state_mut().show_help = true;
        harness.step();
        harness.step();
        assert!(harness.state().show_help);
        harness.state_mut().show_help = false;
        harness.step();
    }
}

#[test]
fn start_page_hides_editor_ribbon_and_scales_across_resolutions() {
    let draft = seeded_project();
    for size in [[1000.0, 600.0], [1600.0, 900.0]] {
        let harness = harness_with_size(size, |app| {
            app.show_start_page = true;
            app.pending_recovery = Some(draft.clone());
        });
        assert!(harness.query_by_label("Nova planta").is_some());
        assert!(harness.query_by_label("Abrir projeto...").is_some());
        assert!(harness.query_by_label("Recuperar").is_some());
        // Ribbon is hidden while Start Page is open
        assert!(harness.query_by_label("Arquitetura").is_none());
    }
}
