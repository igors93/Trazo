use crate::app::FloorPlanApp;
use crate::domain::model::OpeningKind;
use crate::domain::openings::DoorType;
use crate::mcp::ToolOutput;
use eframe::egui;
use serde_json::json;

#[test]
fn an_entrance_gate_goes_in_the_front_wall_and_slides_by_default() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    app.run_mcp_tool(
        "definir_terreno",
        &json!({ "largura": 12, "comprimento": 25 }),
    );
    let wall = app.run_mcp_tool(
        "criar_parede",
        &json!({ "x1": 0.5, "y1": 0.5, "x2": 11.5, "y2": 0.5 }),
    );
    let wall = wall.structured.unwrap()["id"].as_u64().unwrap();
    let gate = app.run_mcp_tool(
        "inserir_abertura",
        &json!({ "parede_id": wall, "tipo": "portao", "posicao": 0.7 }),
    );
    assert!(!gate.is_error, "{}", gate.text);
    let opening = app.project.openings.last().unwrap().clone();
    assert_eq!(opening.kind, OpeningKind::Gate);
    assert_eq!(opening.width(), 3.0);
    assert_eq!(opening.effective_door_type(), DoorType::Sliding);
    assert_eq!(opening.height(), Some(2.2));
    // Gates are tagged PT in the schedule, apart from doors.
    let types = crate::domain::annotations::frame_types(&app.project);
    let tag = crate::domain::annotations::frame_of(&types, &opening);
    assert_eq!(tag.map(|frame| frame.code.as_str()), Some("PT1"));
    // From a pedestrian gate (0.80 m) to a garage gate (8 m).
    assert!(
        app.run_mcp_tool(
            "inserir_abertura",
            &json!({ "parede_id": wall, "tipo": "portao", "posicao": 0.2, "largura": 0.5 }),
        )
        .is_error
    );
    let pedestrian = app.run_mcp_tool(
        "inserir_abertura",
        &json!({ "parede_id": wall, "tipo": "portao", "posicao": 0.2, "largura": 0.9 }),
    );
    assert!(!pedestrian.is_error, "{}", pedestrian.text);
    let listed = app.run_mcp_tool("obter_planta", &json!({}));
    let openings = listed.structured.unwrap()["aberturas"].clone();
    assert!(
        openings
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["tipo"] == "portao")
    );
}

#[test]
fn exports_and_imports_dxf_through_mcp() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let created = app.run_mcp_tool(
        "criar_comodo",
        &json!({ "nome": "Sala", "x": 0.0, "y": 0.0, "largura": 4.0, "comprimento": 3.0 }),
    );
    assert!(!created.is_error, "{}", created.text);
    let dxf_path = std::env::temp_dir().join(format!("trazo-mcp-dxf-{}.dxf", std::process::id()));
    let exported = app.run_mcp_tool(
        "exportar_planta",
        &json!({ "formato": "dxf", "caminho": dxf_path.display().to_string() }),
    );
    assert!(!exported.is_error, "{}", exported.text);
    assert!(dxf_path.is_file());

    let reset = app.run_mcp_tool("novo_projeto", &json!({ "descartar_alteracoes": true }));
    assert!(!reset.is_error, "{}", reset.text);
    assert!(app.project.walls.is_empty());
    let imported = app.run_mcp_tool(
        "importar_dxf",
        &json!({ "caminho": dxf_path.display().to_string() }),
    );
    assert!(!imported.is_error, "{}", imported.text);
    assert!(app.project.walls.len() >= 4);
    assert!(app.history.can_undo());
    let _ = std::fs::remove_file(&dxf_path);
}

#[test]
fn mcp_exposes_and_selects_elements_by_uuid_and_ifc_global_id() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let created = app.run_mcp_tool(
        "criar_comodo",
        &json!({ "nome": "Quarto", "x": 0.0, "y": 0.0, "largura": 3.5, "comprimento": 3.0 }),
    );
    assert!(!created.is_error, "{}", created.text);

    let plan = app.run_mcp_tool("obter_planta", &json!({}));
    let data = plan.structured.unwrap();
    assert!(data["projeto_guid"].as_str().is_some());
    assert_eq!(data["projeto_ifc_global_id"].as_str().unwrap().len(), 22);
    let room_entry = &data["comodos"][0];
    let room_id = room_entry["id"].as_u64().unwrap();
    let room_guid_str = room_entry["guid"].as_str().unwrap();
    let room_guid = crate::domain::guid::Guid::parse(room_guid_str).unwrap();

    // Select by canonical UUID string.
    let sel_uuid = app.run_mcp_tool("selecionar", &json!({ "guid": room_guid_str }));
    assert!(!sel_uuid.is_error, "{}", sel_uuid.text);
    assert_eq!(app.selection, Some(crate::app::Selection::Room(room_id)));

    // Clear selection, then select by 22-char IFC GlobalId.
    app.run_mcp_tool("selecionar", &json!({}));
    assert_eq!(app.selection, None);
    let sel_ifc = app.run_mcp_tool(
        "selecionar",
        &json!({ "guid": room_guid.to_ifc_global_id() }),
    );
    assert!(!sel_ifc.is_error, "{}", sel_ifc.text);
    assert_eq!(app.selection, Some(crate::app::Selection::Room(room_id)));
}

#[test]
fn hostile_params_never_panic_or_break_model() {
    let context = egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    // Seed a valid room so element-editing tools have live state alongside bogus IDs.
    let seeded = app.run_mcp_tool(
        "criar_comodo",
        &json!({ "nome": "Sala", "x": 0.0, "y": 0.0, "largura": 4.0, "comprimento": 3.0 }),
    );
    assert!(!seeded.is_error, "{}", seeded.text);

    let huge_str = "A".repeat(10_000);
    let hostile_payloads: Vec<serde_json::Value> = vec![
        json!({}),
        json!({
            "id": 999_999_999u64,
            "ids": [0, 999_999_999u64],
            "parede_id": 999_999_999u64,
            "guid": "00000000-0000-0000-0000-000000000000",
            "x": "NaN",
            "y": "Infinity",
            "x1": -0.0,
            "y1": 1e308,
            "x2": -1e308,
            "y2": 1e-300,
            "dx": "Infinity",
            "dy": "-Infinity",
            "largura": -5.0,
            "comprimento": 0.0,
            "altura": 1e9,
            "nome": huge_str,
            "acao": "inexistente",
            "tipo": "desconhecido",
            "operacao": "invalida",
            "pontos": [],
        }),
        json!({
            "id": "not-a-number",
            "ids": "abc,def",
            "guid": "not-a-valid-guid",
            "x": null,
            "y": true,
            "largura": "NaN",
            "comprimento": "Infinity",
            "pontos": [
                {"x": 0.0, "y": 0.0},
                {"x": 0.0, "y": 0.0},
                {"x": 0.0, "y": 0.0}
            ],
            "acoes": [{"ferramenta": "criar_parede", "argumentos": {"x1": "NaN"}}],
        }),
        json!({
            "id": 0,
            "x": 1e20,
            "y": -1e20,
            "largura": 1e20,
            "comprimento": 1e20,
            "espessura": -1.0,
            "rotacao": 13.7,
            "passos": -100,
            "todos_pavimentos": "talvez",
            "caminho": "",
            "formato": "invalido",
        }),
    ];

    let tools = crate::mcp::protocol::tool_names();
    assert!(tools.len() >= 40, "expected full MCP catalogue");

    let assert_safe = |app: &FloorPlanApp, tool_name: &str, output: &ToolOutput| {
        assert!(
            !output.text.contains("Erro interno do Trazo"),
            "{tool_name} swallowed a panic: {}",
            output.text
        );
        // Check after every call: a later tool may reset the project and
        // otherwise hide corruption caused by an earlier hostile input.
        for wall in &app.project.walls {
            assert!(wall.start.is_finite() && wall.end.is_finite());
        }
        for opening in &app.project.openings {
            assert!(opening.t.is_finite() && opening.width().is_finite());
        }
        for fixture in &app.project.fixtures {
            assert!(fixture.center.is_finite());
            assert!(fixture.width.is_finite() && fixture.depth.is_finite());
        }
    };

    for tool_name in tools {
        // Skip tools that could write outside temp when given empty caminho.
        if matches!(tool_name, "salvar_projeto" | "exportar_planta") {
            for payload in &hostile_payloads {
                let mut p = payload.clone();
                if let Some(obj) = p.as_object_mut() {
                    obj.insert("formato".to_owned(), json!("formato_inexistente"));
                    obj.insert(
                        "caminho".to_owned(),
                        json!(
                            std::env::temp_dir()
                                .join("trazo-hostile-nonexistent-dir-xyz/out.invalid")
                                .display()
                                .to_string()
                        ),
                    );
                }
                let output = app.run_mcp_tool(tool_name, &p);
                assert_safe(&app, tool_name, &output);
            }
            continue;
        }
        for payload in &hostile_payloads {
            let output = app.run_mcp_tool(tool_name, payload);
            assert_safe(&app, tool_name, &output);
        }
    }
}
