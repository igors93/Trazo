use super::*;
use crate::domain::geometry::Point;
use crate::domain::model::OpeningKind;
use eframe::egui;

fn app() -> FloorPlanApp {
    FloorPlanApp::new(&egui::Context::default())
}

fn run(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
    app.run_mcp_tool(name, &args)
}

fn ok(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
    let output = run(app, name, args);
    assert!(!output.is_error, "{name}: {}", output.text);
    output
}

#[test]
fn builds_a_plan_in_the_assistants_coordinates() {
    let mut app = app();
    ok(
        &mut app,
        "definir_terreno",
        json!({ "largura": 12, "comprimento": 25 }),
    );
    let sala = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Sala", "x": 1, "y": 1, "largura": 5, "comprimento": 4 }),
    );
    ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Cozinha", "x": "6", "y": 1, "largura": "3,5", "comprimento": 4 }),
    );

    let plan = app.plan_summary();
    assert_eq!(plan["terreno"]["x"], 0.0);
    assert_eq!(plan["terreno"]["y"], 0.0);
    assert_eq!(plan["terreno"]["largura"], 12.0);
    assert_eq!(plan["comodos"][0]["x"], 1.0);
    assert_eq!(plan["comodos"][0]["y"], 1.0);
    assert_eq!(plan["comodos"][0]["area_m2"], 20.0);
    assert_eq!(plan["comodos"][1]["largura"], 3.5);
    // Sala and Cozinha share the wall at x = 6.
    assert_eq!(plan["paredes"].as_array().unwrap().len(), 7);
    assert!(plan["area_util_m2"].as_f64().unwrap() < 34.0);
    assert!(plan["area_construida_m2"].as_f64().unwrap() > 34.0);
    assert_eq!(plan["area_terreno_m2"], 300.0);

    // The wall along the bottom of the Sala lies at y = 1.
    let walls = sala.structured.unwrap()["paredes"].clone();
    let bottom = plan["paredes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|wall| {
            walls.as_array().unwrap().contains(&wall["id"])
                && wall["inicio"]["y"] == 1.0
                && wall["fim"]["y"] == 1.0
        })
        .unwrap()["id"]
        .clone();
    let door = ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": bottom, "tipo": "porta", "largura": 1.0 }),
    );
    let door_id = door.structured.unwrap()["id"].as_u64().unwrap();
    let plan = app.plan_summary();
    assert_eq!(plan["aberturas"][0]["tipo"], "porta");
    assert_eq!(plan["aberturas"][0]["largura"], 1.0);
    assert_eq!(plan["aberturas"][0]["centro"]["y"], 1.0);
    assert_eq!(app.selection, Some(Selection::Opening(door_id)));
    assert!(app.dirty && app.history.can_undo());
    assert_eq!(plan["aberturas"][0]["codigo"], "P1");
    assert_eq!(plan["quadro_esquadrias"][0]["altura"], 2.1);

    // A window gets its own code, height and sill.
    let window = ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": bottom, "tipo": "janela", "posicao": 0.75 }),
    );
    let window_id = window.structured.unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "editar_abertura",
        json!({ "id": window_id, "altura": 1.2, "peitoril": 0.9 }),
    );
    let plan = app.plan_summary();
    let frames = plan["quadro_esquadrias"].as_array().unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1]["codigo"], "J1");
    assert_eq!(frames[1]["altura"], 1.2);
    assert_eq!(frames[1]["peitoril"], 0.9);
    assert!(
        run(
            &mut app,
            "editar_abertura",
            json!({ "id": door_id, "peitoril": 1.0 })
        )
        .is_error
    );
}

#[test]
fn places_fixtures_and_saloon_doors() {
    let mut app = app();
    ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Gourmet", "x": 0, "y": 0, "largura": 5, "comprimento": 4 }),
    );
    // Against the left wall (x = 0): the back faces left and touches the
    // brick face at half the wall thickness.
    let grill = ok(
        &mut app,
        "inserir_objeto",
        json!({ "tipo": "churrasqueira", "x": 0.4, "y": 2, "encostar_na_parede": true }),
    );
    let grill_id = grill.structured.unwrap()["id"].as_u64().unwrap();
    let plan = app.plan_summary();
    let placed = &plan["objetos"][0];
    assert_eq!(placed["rotacao"], 270);
    let half = round(crate::domain::masonry::LEGACY_THICKNESS / 2.0);
    assert!((placed["centro"]["x"].as_f64().unwrap() - (half + 0.3)).abs() < 1e-3);
    assert_eq!(
        plan["espessura_parede_m"],
        round(crate::domain::masonry::LEGACY_THICKNESS)
    );

    ok(
        &mut app,
        "editar_objeto",
        json!({ "id": grill_id, "largura": 1.2, "rotacao": 90 }),
    );
    let edited = &app.plan_summary()["objetos"][0];
    assert_eq!(edited["largura"], 1.2);
    assert_eq!(edited["rotacao"], 90);
    assert!(
        run(
            &mut app,
            "inserir_objeto",
            json!({ "tipo": "piscina", "x": 1, "y": 1 })
        )
        .is_error
    );
    assert!(
        run(
            &mut app,
            "editar_objeto",
            json!({ "id": grill_id, "rotacao": 45 })
        )
        .is_error
    );

    let wall = plan["paredes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|wall| wall["inicio"]["y"] == 4.0 && wall["fim"]["y"] == 4.0)
        .unwrap()["id"]
        .clone();
    ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall, "tipo": "porta_vai_e_vem", "largura": 0.7 }),
    );
    assert_eq!(
        app.plan_summary()["aberturas"][0]["tipo"],
        "porta_vai_e_vem"
    );
    ok(&mut app, "excluir_elemento", json!({ "id": grill_id }));
    assert!(app.project.fixtures.is_empty());

    // A long open span: picked near its stretch of wall, not metres away.
    let side = plan["paredes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|wall| wall["inicio"]["x"] == 5.0 && wall["fim"]["x"] == 5.0)
        .unwrap()["id"]
        .clone();
    ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": side, "tipo": "vao_livre", "largura": 3.8 }),
    );
    let gap = app
        .project
        .openings
        .iter()
        .find(|opening| opening.kind == OpeningKind::Gap)
        .unwrap()
        .id;
    assert_eq!(app.plan_summary()["aberturas"][1]["tipo"], "vao_livre");
    assert_eq!(
        app.project.nearest_opening(Point::new(4.9, -3.6), 0.2),
        Some(gap)
    );
    assert_eq!(
        app.project.nearest_opening(Point::new(2.5, -2.0), 0.2),
        None
    );
}

#[test]
fn failed_changes_leave_no_trace() {
    let mut app = app();
    ok(
        &mut app,
        "definir_terreno",
        json!({ "largura": 10, "comprimento": 10 }),
    );
    let before = app.project.clone();
    let undo_depth = app.history.can_undo();
    for (name, args) in [
        (
            "criar_comodo",
            json!({ "nome": "Fora", "x": 8, "y": 8, "largura": 5, "comprimento": 5 }),
        ),
        (
            "criar_comodo",
            json!({ "nome": "", "x": 1, "y": 1, "largura": 2, "comprimento": 2 }),
        ),
        (
            "inserir_abertura",
            json!({ "parede_id": 999, "tipo": "porta" }),
        ),
        ("editar_comodo", json!({ "id": "abc" })),
        (
            "criar_parede",
            json!({ "x1": 0, "y1": 0, "x2": "muito", "y2": 0 }),
        ),
        ("mover_elemento", json!({ "id": 12345, "dx": 1, "dy": 0 })),
    ] {
        let output = run(&mut app, name, args);
        assert!(output.is_error, "{name} should fail");
    }
    assert_eq!(app.project, before);
    assert_eq!(app.history.can_undo(), undo_depth);
    assert!(app.message.starts_with("IA · "));
}

#[test]
fn opening_with_an_invalid_width_is_not_left_behind() {
    let mut app = app();
    let wall = ok(
        &mut app,
        "criar_parede",
        json!({ "x1": 0, "y1": 0, "x2": 6, "y2": 0 }),
    );
    let wall_id = wall.structured.unwrap()["id"].clone();
    let output = run(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall_id, "tipo": "janela", "largura": 9 }),
    );
    assert!(output.is_error);
    assert!(app.project.openings.is_empty());
}

#[test]
fn edits_moves_deletes_and_undo() {
    let mut app = app();
    let room = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Quarto", "x": 0, "y": 0, "largura": 3, "comprimento": 3 }),
    );
    let room_id = room.structured.unwrap()["id"].clone();
    ok(
        &mut app,
        "editar_comodo",
        json!({ "id": room_id, "nome": "Suíte", "largura": 4 }),
    );
    let plan = app.plan_summary();
    assert_eq!(plan["comodos"][0]["nome"], "Suíte");
    assert_eq!(plan["comodos"][0]["largura"], 4.0);
    assert_eq!(plan["comodos"][0]["comprimento"], 3.0);

    // A standalone wall moves up by 2 m in the assistant's coordinates.
    let wall = ok(
        &mut app,
        "criar_parede",
        json!({ "x1": 10, "y1": 0, "x2": 10, "y2": 3 }),
    );
    let wall_id = wall.structured.unwrap()["id"].clone();
    ok(
        &mut app,
        "mover_elemento",
        json!({ "id": wall_id, "dx": 0, "dy": 2 }),
    );
    let moved = app.plan_summary()["paredes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|wall| wall["id"] == wall_id)
        .unwrap()
        .clone();
    assert_eq!(moved["inicio"]["y"], 2.0);
    assert_eq!(moved["fim"]["y"], 5.0);

    ok(&mut app, "excluir_elemento", json!({ "id": wall_id }));
    assert_eq!(app.project.walls.len(), 4);
    ok(&mut app, "desfazer", json!({}));
    assert_eq!(app.project.walls.len(), 5);
    ok(&mut app, "refazer", json!({ "passos": 3 }));
    assert_eq!(app.project.walls.len(), 4);
    assert!(run(&mut app, "refazer", json!({})).is_error);
}

#[test]
fn new_project_respects_unsaved_changes() {
    let mut app = app();
    ok(
        &mut app,
        "criar_parede",
        json!({ "x1": 0, "y1": 0, "x2": 3, "y2": 0 }),
    );
    assert!(run(&mut app, "novo_projeto", json!({})).is_error);
    assert_eq!(app.project.walls.len(), 1);
    ok(
        &mut app,
        "novo_projeto",
        json!({ "nome": "Casa nova", "descartar_alteracoes": true }),
    );
    assert!(app.project.walls.is_empty());
    assert_eq!(app.project.name, "Casa nova");
    assert!(app.dirty);
    assert!(app.recovery_pending);
}

#[test]
fn saves_exports_reopens_and_pictures_the_plan() {
    let folder = std::env::temp_dir().join(format!("trazo-mcp-{}", std::process::id()));
    let mut app = app();
    ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Sala", "x": 0, "y": 0, "largura": 4, "comprimento": 3 }),
    );
    let project_path = folder.join("casa");
    ok(
        &mut app,
        "salvar_projeto",
        json!({ "caminho": project_path.display().to_string() }),
    );
    let saved = folder.join("casa.trazo");
    assert!(saved.is_file());
    assert!(!app.dirty);
    let pdf = folder.join("casa.pdf");
    ok(
        &mut app,
        "exportar_planta",
        json!({ "formato": "PDF", "caminho": pdf.display().to_string() }),
    );
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));

    ok(&mut app, "novo_projeto", json!({}));
    ok(
        &mut app,
        "abrir_projeto",
        json!({ "caminho": saved.display().to_string() }),
    );
    assert_eq!(app.project.rooms.len(), 1);

    let picture = ok(&mut app, "imagem_da_planta", json!({ "tamanho_max": 300 }));
    assert!(picture.image_png.unwrap().starts_with(b"\x89PNG"));
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn file_names_are_made_safe_and_unique() {
    assert_eq!(file_stem("Casa: Jardim/2"), "Casa- Jardim-2");
    assert_eq!(file_stem("  ..  "), "planta");
    let folder = std::env::temp_dir().join(format!("trazo-unique-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let first = folder.join("planta.pdf");
    std::fs::write(&first, b"x").unwrap();
    assert_eq!(unique_path(first.clone()), folder.join("planta (2).pdf"));
    let _ = std::fs::remove_dir_all(folder);
    assert!(
        resolve_path("casa", "trazo").ends_with("Trazo/casa.trazo")
            || resolve_path("casa", "trazo").ends_with("Trazo\\casa.trazo")
    );
}
