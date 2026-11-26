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
fn markup_ribbon_and_comment_dialog_work_at_different_window_sizes() {
    use crate::domain::markups::{Markup, MarkupKind};
    for size in [[960.0, 720.0], [1440.0, 900.0]] {
        let mut harness = harness_with_size(size, |app| {
            app.project = Project::default();
            app.show_start_page = false;
            app.ribbon_tab = RibbonTab::Markups;
            app.ribbon_collapsed = false;
            app.set_tool(Tool::Markup(None));
            let id = app
                .project
                .add_markup(Markup {
                    id: 0,
                    kind: MarkupKind::Rectangle,
                    points: vec![Point::ORIGIN, Point::new(2.0, 2.0)],
                    color: [220, 65, 65],
                    comment: "Revisar armário".into(),
                    resolved: false,
                    linked_elements: vec![],
                })
                .unwrap();
            app.sync_markup_context();
            app.markup_state.selected = Some(id);
        });
        assert!(harness.query_by_label("Lista e comentários").is_some());
        harness.get_by_label("Lista e comentários").click();
        harness.step();
        harness.step();
        assert_eq!(harness.state().active_doc_dialog, DocDialog::Markups);
        assert!(harness.query_by_label("Salvar comentário").is_some());
        harness
            .state_mut()
            .markup_state
            .edit
            .as_mut()
            .unwrap()
            .comment = "Aumentar o armário".into();
        harness.get_by_label("Salvar comentário").click();
        harness.step();
        harness.step();
        assert_eq!(
            harness.state().project.markups[0].comment,
            "Aumentar o armário"
        );
        harness.get_by_label("Fechar").click();
        harness.step();
        harness.step();
        assert_eq!(harness.state().active_doc_dialog, DocDialog::None);
        harness.state_mut().undo();
        assert_eq!(
            harness.state().project.markups[0].comment,
            "Revisar armário"
        );
    }
}

#[test]
fn library_pointer_placement_preserves_model_dimensions_and_free_rotation() {
    use crate::domain::library::LibraryItem;
    let mut app = FloorPlanApp::new(&egui::Context::default());
    app.project
        .set_terrain(Point::new(-10.0, -10.0), Point::new(20.0, 20.0));
    let model = LibraryItem::builtin_catalog()
        .into_iter()
        .find(|i| i.name.starts_with("Sofá retrátil"))
        .unwrap();
    app.start_library_placement(model.clone());
    app.fixture_turns = 1;
    let ghost = app.fixture_placement(FixtureKind::Sofa, Point::new(3.0, 3.0));
    assert_eq!((ghost.width, ghost.depth), (model.width_m, model.depth_m));
    assert_eq!(ghost.quarter_turns, 1);
    app.place_fixture(FixtureKind::Sofa, Point::new(3.0, 3.0));
    let placed = app.project.fixtures.last().unwrap();
    assert_eq!(
        (placed.width, placed.depth, placed.quarter_turns),
        (ghost.width, ghost.depth, 1)
    );

    app.set_tool(Tool::Fixture(FixtureKind::Car));
    assert!(app.fixture_preset.is_none());
    app.fixture_turns = 3;
    app.project
        .add_wall(Point::new(0.0, 3.2), Point::new(10.0, 3.2))
        .unwrap();
    let ghost = app.fixture_placement(FixtureKind::Car, Point::new(7.0, 3.0));
    assert_eq!((ghost.width, ghost.depth), FixtureKind::Car.default_size());
    assert_eq!(ghost.quarter_turns, 3);
    assert_eq!(ghost.center, Point::new(7.0, 3.0));
    app.set_tool(Tool::Fixture(FixtureKind::DiningSet));
    app.fixture_turns = 1;
    let ghost = app.fixture_placement(FixtureKind::DiningSet, Point::new(7.0, 3.0));
    assert_eq!(ghost.center, Point::new(7.0, 3.0));
    assert_eq!(ghost.quarter_turns, 1);
}

#[test]
fn interior_library_previews_and_accent_search_render_at_multiple_sizes() {
    for size in [[960.0, 720.0], [1440.0, 900.0]] {
        let mut harness = harness_with_size(size, |app| {
            app.show_start_page = false;
            app.active_doc_dialog = DocDialog::Library;
            app.library_search = "sofa retratil".to_owned();
        });
        assert!(harness.query_by_label("Sofá retrátil (2,50 m)").is_some());
        harness.get_by_label("Posicionar com mouse").click();
        harness.step();
        harness.step();
        assert_eq!(harness.state().active_doc_dialog, DocDialog::None);
        assert_eq!(
            harness.state().fixture_tool_size(FixtureKind::Sofa),
            (2.5, 1.1)
        );
    }
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
        RibbonTab::Plumbing,
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
fn start_page_recent_cards_fit_the_window_and_distinguish_matching_project_titles() {
    use crate::io::templates::RecentEntry;

    let filenames = [
        "Muro_Broca.trazo",
        "Muro_Estrutural.trazo",
        "muro planta.trazo",
    ];
    for size in [
        [640.0, 900.0],
        [960.0, 900.0],
        [1440.0, 900.0],
        [2560.0, 900.0],
    ] {
        let mut harness = harness_with_size(size, |app| {
            app.show_start_page = true;
            app.pending_recovery = None;
            app.recent_files.entries = filenames
                .iter()
                .map(|filename| {
                    let path = std::env::temp_dir()
                        .join("trazo-start-page-fixture")
                        .join(filename);
                    // Exercise the same card geometry without accessing real user files.
                    app.thumbnail_cache.insert(path.clone(), None);
                    RecentEntry {
                        path,
                        title: "Minha planta".into(),
                        wall_count: 4,
                        room_count: 0,
                    }
                })
                .collect();
        });
        let cards: Vec<_> = filenames
            .iter()
            .map(|filename| harness.get_by_label(filename).rect())
            .collect();
        for rect in &cards {
            assert!(
                rect.left() >= 16.0 && rect.right() <= size[0] - 16.0,
                "Recent card extends outside {size:?}: {rect:?}"
            );
        }
        let content_width = (size[0] - 32.0).min(960.0);
        let expected_left = (size[0] - content_width) * 0.5 + 11.0;
        assert!(
            (cards[0].left() - expected_left).abs() < 20.0,
            "Start Page is not centred at {size:?}"
        );
        if size[0] >= 960.0 {
            assert!((cards[0].top() - cards[2].top()).abs() < 2.0);
            assert!(cards[0].right() < cards[1].left());
            assert!(cards[1].right() < cards[2].left());
        } else {
            assert!(cards[2].top() > cards[0].bottom());
        }
        // Matching project titles represent distinct files; removing one recent
        // entry must retain the other two.
        harness
            .get_by_label("Remover Muro_Broca.trazo dos recentes")
            .click();
        harness.step();
        harness.step();
        assert_eq!(harness.state().recent_files.entries.len(), 2);
        assert!(
            harness
                .state()
                .recent_files
                .entries
                .iter()
                .all(|entry| entry.path.file_name().unwrap() != "Muro_Broca.trazo")
        );
        assert!(harness.query_by_label("Muro_Estrutural.trazo").is_some());
        assert!(harness.query_by_label("muro planta.trazo").is_some());
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
