//! Headless UI regression suite powered by `egui_kittest` (T-07).
//!
//! Renders Trazo's real `FloorPlanApp` layout and critical dialogs (Start Page,
//! Structure, What-if simulator, Electrical, Safety, Sheet and Quantities)
//! without a display server, asserting accessible controls and interactions.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{DocDialog, FloorPlanApp};
use crate::domain::{fixtures::FixtureKind, geometry::Point, model::Project};

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

fn harness_with_app(configure: impl FnOnce(&mut FloorPlanApp)) -> Harness<'static, FloorPlanApp> {
    let ctx = egui::Context::default();
    let mut app = FloorPlanApp::new(&ctx);
    configure(&mut app);
    let mut harness = Harness::builder()
        .with_size([1280.0, 800.0])
        .with_max_steps(16)
        // Existing numeric and combo controls still lack AccessKit names;
        // this suite checks dialog behavior rather than accessibility audits.
        .with_accessibility_check(false)
        .with_context_setup(super::theme::apply)
        .build_ui_state(|ui, state: &mut FloorPlanApp| state.show_layout(ui), app);
    harness.step();
    harness.step();
    harness
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
