use crate::app::FloorPlanApp;
use crate::domain::{fixtures::FixtureKind, geometry::Point, wall_foundations::WallFoundationKind};
use eframe::egui;
use serde_json::json;

#[test]
fn pre_dimensionamento_exposes_tall_panel_wind_and_ground_beam_reactions() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    app.project.masonry.block_width = 0.20;
    let wall = app
        .project
        .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
        .unwrap();
    app.project.set_wall_height(wall, Some(4.3), None).unwrap();
    for x in [0.0, 3.0, 6.0] {
        app.project
            .add_fixture(FixtureKind::Column, Point::new(x, 0.0), 0.20, 0.20, 0)
            .unwrap();
    }
    app.project
        .set_wall_foundation(wall, WallFoundationKind::GroundBeam, None)
        .unwrap();
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    let wind = &report["alvenaria"]["paineis"][0]["vento_fora_plano"];
    assert!(wind["q_kpa"].as_f64().unwrap() > 0.0);
    assert!(wind["cintas"]["intermediarias"].as_u64().unwrap() > 0);
    assert_eq!(report["vento"]["paineis_alvenaria"][0]["rotulo"], "AL1");
    let configured = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({"acao":"configurar_vento", "v0_ms":40.0}),
    );
    assert!(!configured.is_error, "{}", configured.text);
    assert!(
        configured.structured.unwrap()["paineis_alvenaria"][0]["q_kn_m2"]
            .as_f64()
            .unwrap()
            > wind["q_kpa"].as_f64().unwrap()
    );
    assert_eq!(report["limites"]["recusado"], serde_json::Value::Null);
    assert_eq!(
        report["fundacao_paredes"][0]["reacoes_pilares"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn an_assistant_lays_out_and_checks_the_structure() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 20 } },
            { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 4 } },
            { "ferramenta": "criar_comodo", "argumentos": { "nome": "Quarto", "x": 5, "y": 1, "largura": 3, "comprimento": 4 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);
    let wall = app.project.walls[0].id;
    let opening = app.run_mcp_tool(
        "inserir_abertura",
        &json!({ "parede_id": wall, "tipo": "janela", "posicao": 0.5, "largura": 1.2 }),
    );
    assert!(!opening.is_error, "{}", opening.text);
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert_eq!(report["pilares"].as_array().unwrap().len(), 6);
    assert_eq!(report["lajes"].as_array().unwrap().len(), 2);
    assert!(!report["armaduras"]["vigas"].as_array().unwrap().is_empty());
    assert!(
        report["armaduras"]["vigas"][0]["ancoragem_inf_cm"]
            .as_f64()
            .unwrap()
            >= 10.0
    );
    assert!(
        report["armaduras"]["pilares"][0]["traspasse_cm"]
            .as_f64()
            .unwrap()
            >= 40.0
    );
    assert!(
        !report["armaduras"]["tabela_aco"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !report["armaduras"]["resumo_bitolas"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(report["armaduras"]["massa_total_kg"].as_f64().unwrap() > 0.0);
    assert!(
        report["armaduras"]["massa_com_perda_10_kg"]
            .as_f64()
            .unwrap()
            > report["armaduras"]["massa_total_kg"].as_f64().unwrap()
    );
    assert!(report["armaduras"]["total_barras_12m"].as_u64().unwrap() > 0);
    assert!(
        report["vigas"][0]["vaos"][0]["momento_fissuracao_knm"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(report["vigas"][0]["vaos"][0]["estadio_ii"].is_boolean());
    assert!(
        !report["alvenaria"]["paineis"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        report["alvenaria"]["vergas_contravergas"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Every element gets its traffic light (#101).
    let lights = report["semaforo"].as_array().unwrap();
    assert!(!lights.is_empty());
    assert!(
        lights
            .iter()
            .all(|v| v["verificacoes"].as_array().is_some_and(|c| !c.is_empty()))
    );
    assert!(report["alertas"].as_array().unwrap().is_empty(), "{report}");
    let beam = report["vigas"][0]["vaos"][0]["id"].as_u64().unwrap();
    assert!(
        report["vigas"][0]["vaos"][0]["momento_vao_knm"]
            .as_f64()
            .unwrap()
            > 0.0
    );

    let load = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "inserir_carga",
            "tipo": "caixa_agua",
            "x": 3.0,
            "y": 3.0,
            "carga_kn": 12.0
        }),
    );
    assert!(!load.is_error, "{}", load.text);
    let load_id = load.structured.unwrap()["id"].as_u64().unwrap();
    let with_load = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert_eq!(
        with_load["cargas_concentradas"].as_array().unwrap().len(),
        1
    );
    assert_eq!(with_load["cargas_concentradas"][0]["carga_kn"], 12.0);
    let edited_load = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "editar_carga", "id": load_id, "carga_kn": 13.5 }),
    );
    assert!(!edited_load.is_error, "{}", edited_load.text);

    // A beam too shallow is flagged; a solid slab is set by room name.
    let edited = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "editar_viga", "id": beam, "altura_cm": 20 }),
    );
    assert!(!edited.is_error, "{}", edited.text);
    let slab = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "definir_laje", "comodo": "sala", "tipo": "macica" }),
    );
    assert!(!slab.is_error, "{}", slab.text);
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert!(
        report["alertas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("pede altura"))
    );
    // A column snaps onto the nearest junction.
    let column = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "inserir_pilar", "x": 1.1, "y": 9.1, "largura_cm": 20, "profundidade_cm": 20 }),
    );
    assert!(!column.is_error, "{}", column.text);
    assert!(
        app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "configurar", "fck_mpa": 99 })
        )
        .is_error
    );
    let view = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "vista", "disciplina": "estrutura" }),
    );
    assert!(!view.is_error, "{}", view.text);
}

#[test]
fn an_assistant_roofs_the_gourmet_area_and_the_columns_take_it() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 10, "comprimento": 50 } },
            { "ferramenta": "criar_comodo", "argumentos": { "nome": "Gourmet", "x": 0, "y": 25.75, "largura": 5, "comprimento": 14.75 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);
    let before = app.project.structure_report();
    let load_before: f64 = before.columns.iter().map(|c| c.load_kn).sum();
    let roofed = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "definir_telhado",
            "x": 0, "y": 25.75, "largura_m": 5, "comprimento_m": 14.75,
            "lado_alto": "oeste", "altura_alta_m": 4.3, "altura_baixa_m": 2.6,
            "telha": "ceramica", "madeira": "c30",
        }),
    );
    assert!(!roofed.is_error, "{}", roofed.text);
    let data = roofed.structured.unwrap();
    assert_eq!(data["pecas"].as_array().unwrap().len(), 4, "{data}");
    let roof_id = app.project.structure.roofs[0].id;
    let bracing = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "definir_telhado", "id": roof_id, "contraventamento_kn": 100.0,
            "ajustar_paredes": false
        }),
    );
    assert!(!bracing.is_error, "{}", bracing.text);
    assert_eq!(
        app.project.structure.roofs[0].lateral_bracing_kn,
        Some(100.0)
    );
    let wind_report = app
        .run_mcp_tool(
            "projeto_estrutural",
            &json!({
                "acao": "configurar_vento", "v0_ms": 40.0
            }),
        )
        .structured
        .unwrap();
    assert_eq!(
        wind_report["estabilidade_global"].as_array().unwrap().len(),
        4
    );
    assert_eq!(wind_report["contraventamento"].as_array().unwrap().len(), 1);
    assert!((data["inclinacao_pct"].as_f64().unwrap() - 34.0).abs() < 0.01);
    // The west wall now stands 4.30 m tall.
    let west = app
        .project
        .walls
        .iter()
        .find(|w| w.start.x.abs() < 0.01 && w.end.x.abs() < 0.01)
        .unwrap();
    assert_eq!(west.heights(app.project.wall_height), (4.3, 4.3));
    // The roof weight reaches the columns.
    let after = app.project.structure_report();
    let load_after: f64 = after.columns.iter().map(|c| c.load_kn).sum();
    let carried: f64 = after.roofs[0]
        .reactions
        .iter()
        .filter(|r| r.support.as_deref().is_some_and(|s| s.starts_with('P')))
        .map(|r| r.load_kn)
        .sum();
    assert!(carried > 0.0);
    assert!(
        load_after > load_before + carried * 0.99,
        "{load_before} → {load_after}"
    );
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert_eq!(report["telhados"].as_array().unwrap().len(), 1);
    let id = data["id"].as_u64().unwrap();
    let edited = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "definir_telhado", "id": id, "telha": "metalica", "ajustar_paredes": false }),
    );
    assert!(!edited.is_error, "{}", edited.text);
    assert_eq!(
        edited.structured.unwrap()["pecas"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let removed = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "remover_telhado", "id": id }),
    );
    assert!(!removed.is_error, "{}", removed.text);
    assert!(app.project.structure.roofs.is_empty());
}

#[test]
fn timber_posts_carry_the_low_edge_of_a_roof() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 10, "comprimento": 50 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 25.83, "diametro_cm": 15 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 33.1, "diametro_cm": 15, "madeira": "itauba", "tratada": false } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 40.42, "diametro_cm": 15 } },
            { "ferramenta": "projeto_estrutural", "argumentos": {
                "acao": "definir_telhado", "x": 0, "y": 25.75, "largura_m": 5, "comprimento_m": 14.75,
                "lado_alto": "oeste", "altura_alta_m": 4.3, "altura_baixa_m": 2.6,
            } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);
    assert_eq!(
        app.project.structure.post_species,
        crate::domain::roof::TimberSpecies::Itauba
    );
    assert!(!app.project.structure.post_treated);
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    let posts = report["mouroes"].as_array().unwrap();
    assert_eq!(posts.len(), 3, "{report}");
    assert!(posts.iter().all(|p| p["carga_kn"].as_f64().unwrap() > 0.0));
    assert!(posts.iter().all(|p| p["especie"] == "itauba"));
    assert!(posts.iter().all(|p| p["classe"] == "c30"));
    assert!(posts.iter().all(|p| p["tratada"] == false));
    assert!(
        posts
            .iter()
            .all(|p| p["momento_vento_knm"].as_f64().unwrap() > 0.0)
    );
    assert!(posts.iter().all(|p| p["atende"] == false));
    // Two timber beams, post to post, carry the beam ends in between.
    assert_eq!(report["vigas_madeira"].as_array().unwrap().len(), 2);
    let low_loose = report["telhados"][0]["apoios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["lado"] == "baixo" && a["apoio"].is_null())
        .count();
    assert_eq!(low_loose, 0, "{report}");
    let bad = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "inserir_mourao", "x": 2, "y": 2, "diametro_cm": 60 }),
    );
    assert!(bad.is_error);
}

#[test]
fn the_wind_checks_the_tall_wall_and_lifts_an_open_roof() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 10, "comprimento": 50 } },
            { "ferramenta": "criar_parede", "argumentos": { "x1": 0.07, "y1": 25.75, "x2": 0.07, "y2": 40.5 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 25.83 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 40.42 } },
            { "ferramenta": "projeto_estrutural", "argumentos": {
                "acao": "definir_telhado", "x": 0, "y": 25.75, "largura_m": 5, "comprimento_m": 14.75,
                "lado_alto": "oeste", "altura_alta_m": 4.3, "altura_baixa_m": 2.6, "telha": "metalica",
            } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "configurar_vento", "v0_ms": 40, "categoria_terreno": 2 } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);
    assert_eq!(app.project.structure.wind.category, 2);
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    let roof = &report["vento"]["telhados"][0];
    // A light metal roof with an open side lifts.
    assert_eq!(roof["lado_aberto"], true, "{report}");
    assert_eq!(roof["levanta"], true, "{report}");
    assert!(roof["ancoragem_por_ponta_kn"].as_f64().unwrap() > 0.0);
    let bad = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "configurar_vento", "v0_ms": 90 }),
    );
    assert!(bad.is_error);
}

#[test]
fn a_soil_survey_sets_the_soil_and_compares_the_foundations() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 20 } },
            { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 4 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);
    let defined = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "definir_sondagem",
            "nome": "SP-01",
            "nivel_agua_m": 5.2,
            "leituras": [
                { "profundidade_m": 1, "n": 6, "solo": "argila" },
                { "profundidade_m": 2, "n": 7, "solo": "argila" },
                { "profundidade_m": 3, "n": 8, "solo": "silte_argiloso" },
                { "profundidade_m": 4, "n": 12, "solo": "silte_arenoso" },
                { "profundidade_m": 5, "n": 15, "solo": "areia" },
                { "profundidade_m": 6, "n": 18, "solo": "areia" },
            ],
        }),
    );
    assert!(!defined.is_error, "{}", defined.text);
    // N̄ from 1 to 3 m = 7 → 140 kPa.
    assert!((app.project.structure.settings.soil_kpa - 140.0).abs() < 1e-9);
    let study = app
        .run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "simular_fundacoes" }),
        )
        .structured
        .unwrap();
    let options = study["opcoes"].as_array().unwrap();
    assert_eq!(options.len(), 4);
    assert!(study["recomendada"].is_string(), "{study}");
    // Brocas stop above the water table at 5.2 m.
    assert!(study["broca"]["comprimento_m"].as_f64().unwrap() <= 5.0);
    // Deep CFA piles are also evaluated (6 m survey reaches MIN_DEEP_PILE_M).
    assert!(study["estaca_profunda"].is_object(), "{study}");
    assert!(options[0]["recalque_mm"].as_f64().unwrap() > 0.0);

    // Multi-borehole SPT uses the weaker borehole envelope.
    let multi = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "definir_sondagem",
            "cota_sapata_m": 1.0,
            "furos": [
                {
                    "nome": "SP-01",
                    "nivel_agua_m": 6.0,
                    "leituras": [
                        { "profundidade_m": 1, "n": 12, "solo": "areia" },
                        { "profundidade_m": 2, "n": 14, "solo": "areia" },
                        { "profundidade_m": 3, "n": 16, "solo": "areia" },
                        { "profundidade_m": 6, "n": 20, "solo": "areia" },
                        { "profundidade_m": 8, "n": 24, "solo": "areia" },
                    ]
                },
                {
                    "nome": "SP-02",
                    "nivel_agua_m": 2.0,
                    "leituras": [
                        { "profundidade_m": 1, "n": 6, "solo": "argila" },
                        { "profundidade_m": 2, "n": 6, "solo": "argila" },
                        { "profundidade_m": 3, "n": 6, "solo": "argila" },
                        { "profundidade_m": 6, "n": 12, "solo": "silte_arenoso" },
                        { "profundidade_m": 8, "n": 18, "solo": "areia" },
                    ]
                }
            ]
        }),
    );
    assert!(!multi.is_error, "{}", multi.text);
    let multi_study = multi.structured.unwrap();
    assert_eq!(multi_study["furos"].as_u64(), Some(2));
    assert_eq!(multi_study["furo_critico"].as_str(), Some("SP-02"));
    assert!((app.project.structure.settings.soil_kpa - 120.0).abs() < 1e-9);
    let bad = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "definir_sondagem", "leituras": [{ "profundidade_m": 1, "n": 5, "solo": "turfa" }] }),
    );
    assert!(bad.is_error);
    let removed = app.run_mcp_tool("projeto_estrutural", &json!({ "acao": "remover_sondagem" }));
    assert!(!removed.is_error, "{}", removed.text);
    assert!(app.project.structure.soil_survey.is_none());
}

#[test]
fn exporting_reinforcement_detail_svg_and_pdf_succeeds() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({ "acoes": [
            { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 20 } },
            { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 4 } },
            { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
        ] }),
    );
    assert!(!built.is_error, "{}", built.text);

    let dir = std::env::temp_dir().join("trazo_test_armaduras_110");
    let _ = std::fs::create_dir_all(&dir);
    let svg_path = dir.join("detalhamento.svg");
    let pdf_path = dir.join("detalhamento.pdf");
    let csv_path = dir.join("tabela-aco.csv");

    let svg_res = app.run_mcp_tool(
        "exportar_planta",
        &json!({ "formato": "armaduras_svg", "caminho": svg_path.to_string_lossy() }),
    );
    assert!(!svg_res.is_error, "{}", svg_res.text);
    let svg = std::fs::read_to_string(&svg_path).unwrap();
    assert!(svg.contains("DETALHAMENTO ESQUEMÁTICO DE ARMADURAS"));

    let pdf_res = app.run_mcp_tool(
        "exportar_planta",
        &json!({ "formato": "armaduras_pdf", "caminho": pdf_path.to_string_lossy() }),
    );
    assert!(!pdf_res.is_error, "{}", pdf_res.text);
    let pdf = std::fs::read(&pdf_path).unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    let csv_res = app.run_mcp_tool(
        "exportar_planta",
        &json!({ "formato": "tabela_aco_csv", "caminho": csv_path.to_string_lossy() }),
    );
    assert!(!csv_res.is_error, "{}", csv_res.text);
    let csv = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv.contains("Resumo por bitola (CA-50)"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_reports_2d_frames_and_exports_frame_diagrams() {
    let context = eframe::egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({
            "acoes": [
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 0, "y": 0, "largura": 4.0, "comprimento": 3.5 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Quarto", "x": 4.0, "y": 0, "largura": 3.5, "comprimento": 3.5 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
            ]
        }),
    );
    assert!(!built.is_error, "{}", built.text);

    let rigid = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    let frames = rigid["porticos_2d"].as_array().unwrap();
    assert!(!frames.is_empty());
    assert_eq!(frames[0]["nos_rigidos"], true);
    assert_eq!(frames[0]["bases_engastadas"], true);
    assert!(frames[0]["residuo_equilibrio_kn"].as_f64().unwrap() < 1e-6);
    for beam in rigid["armaduras"]["vigas"].as_array().unwrap() {
        let frame_beam = frames
            .iter()
            .flat_map(|f| f["vigas"].as_array().unwrap())
            .find(|b| b["id"] == beam["id"])
            .unwrap();
        assert_eq!(beam["origem_esforcos"], "portico_2d_elu");
        assert_eq!(beam["superior_continua"], true);
        assert_eq!(beam["envoltoria_elu"], frame_beam["envoltoria_elu"]);
        assert!(
            beam["envoltoria_elu"]["combinacao_momento_positivo"]
                .as_str()
                .unwrap()
                .starts_with("ELU")
        );
    }

    // Switch to pinned column-top joints via MCP configurar.
    let cfg = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "configurar", "portico_nos_rigidos": false, "portico_bases_engastadas": true }),
    );
    assert!(!cfg.is_error, "{}", cfg.text);
    let pinned = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    let p_frame = &pinned["porticos_2d"][0];
    assert_eq!(p_frame["nos_rigidos"], false);
    assert_eq!(p_frame["bases_engastadas"], true);
    assert_eq!(p_frame["contraventado"], false);
    for col in p_frame["pilares"].as_array().unwrap() {
        assert!(col["momento_topo_knm"].as_f64().unwrap().abs() < 1e-3);
    }

    let cfg = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "configurar", "portico_bases_engastadas": false }),
    );
    assert!(!cfg.is_error, "{}", cfg.text);
    let unbraced = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert!(unbraced["porticos_2d"].as_array().unwrap().is_empty());
    assert!(
        unbraced["armaduras"]["tabela_aco"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        unbraced["armaduras"]["avisos"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("análise do pórtico falhou"))
    );
    assert!(
        unbraced["falhas_porticos_2d"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| {
                f["motivo"]
                    .as_str()
                    .unwrap()
                    .contains("sem contraventamento físico modelado")
            })
    );

    let cfg = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({ "acao": "configurar", "portico_nos_rigidos": true, "portico_bases_engastadas": true }),
    );
    assert!(!cfg.is_error, "{}", cfg.text);

    // Export 2D frame diagram SVG.
    let dir = std::env::temp_dir().join("trazo_test_porticos_94");
    let _ = std::fs::create_dir_all(&dir);
    let svg_path = dir.join("porticos.svg");
    let svg_res = app.run_mcp_tool(
        "exportar_planta",
        &json!({ "formato": "porticos_svg", "caminho": svg_path.to_string_lossy() }),
    );
    assert!(!svg_res.is_error, "{}", svg_res.text);
    let svg = std::fs::read_to_string(&svg_path).unwrap();
    assert!(svg.contains("Análise de pórticos planos 2D"));
    let _ = std::fs::remove_dir_all(&dir);

    // A solver failure remains explicit in the machine-readable contract
    // and in the same safety traffic light consumed by assistants.
    app.project.structure.settings.fck_mpa = f64::INFINITY;
    let failed_output = app.run_mcp_tool("pre_dimensionamento", &json!({}));
    assert!(!failed_output.is_error, "{}", failed_output.text);
    assert!(failed_output.text.contains("falha(s) de pórtico"));
    let failed = failed_output.structured.unwrap();
    assert!(failed["porticos_2d"].as_array().unwrap().is_empty());
    let failures = failed["falhas_porticos_2d"].as_array().unwrap();
    assert_eq!(failures.len(), frames.len());
    assert!(failures.iter().all(|failure| {
        failure["rotulo"]
            .as_str()
            .is_some_and(|label| !label.is_empty())
            && failure["motivo"]
                .as_str()
                .is_some_and(|reason| reason.contains("rigidez inválida"))
            && failure["atende"] == false
    }));
    let lights = failed["semaforo"].as_array().unwrap();
    assert!(failures.iter().all(|failure| {
        lights
            .iter()
            .any(|light| light["elemento"] == failure["rotulo"] && light["semaforo"] == "vermelho")
    }));
}

#[test]
fn mcp_reports_nbr6120_nbr8681_combinations_and_nbr6123_building_wind() {
    let context = eframe::egui::Context::default();
    let mut app = FloorPlanApp::new(&context);
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({
            "acoes": [
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 0, "y": 0, "largura": 5.0, "comprimento": 4.0 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Garagem", "x": 5.0, "y": 0, "largura": 3.5, "comprimento": 4.0 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "configurar", "ocupacao": "escritorio", "sobrecarga_por_comodo": true } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "configurar_vento", "v0_ms": 40.0, "categoria_terreno": 3, "s1": 1.0, "s3": 1.0 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
            ]
        }),
    );
    assert!(!built.is_error, "{}", built.text);

    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert_eq!(report["materiais"]["ocupacao"], "escritorio");

    let loads = &report["cargas_e_combinacoes"];
    assert_eq!(loads["nbr_6120"]["ocupacao"], "escritorio");
    assert!((loads["nbr_6120"]["psi0_q"].as_f64().unwrap() - 0.70).abs() < 1e-6);
    assert!((loads["nbr_6120"]["psi1_q"].as_f64().unwrap() - 0.60).abs() < 1e-6);
    assert!((loads["nbr_6120"]["psi2_q"].as_f64().unwrap() - 0.40).abs() < 1e-6);
    assert!(loads["nbr_6120"]["g1_total_kn"].as_f64().unwrap() > 0.0);
    assert!(loads["nbr_6120"]["g2_total_kn"].as_f64().unwrap() > 0.0);
    assert!(loads["nbr_6120"]["q_total_kn"].as_f64().unwrap() > 0.0);

    let wind = &loads["vento_edificacao_nbr_6123"];
    assert_eq!(wind["v0_ms"], 40.0);
    assert_eq!(wind["direcoes"].as_array().unwrap().len(), 4);
    assert!(wind["direcoes"][0]["cortante_base_kn"].as_f64().unwrap() > 0.0);
    assert_eq!(wind["direcoes"][0]["atende_tombamento"], true);

    let combos = loads["combinacoes_nbr_8681"].as_array().unwrap();
    assert_eq!(combos.len(), 17);
    assert!(combos.iter().any(|c| c["codigo"] == "ELU-1"));
    assert!(combos.iter().any(|c| c["codigo"] == "ELU-EQU+"));
    assert!(combos.iter().any(|c| c["codigo"] == "ELS-QP"));
    assert!(combos.iter().any(|c| c["codigo"] == "ELS-F2"));

    let frame0 = &report["porticos_2d"][0];
    assert_eq!(frame0["casos_de_carga"].as_array().unwrap().len(), 5);
    assert_eq!(frame0["combinacoes"].as_array().unwrap().len(), 17);
    assert!(frame0["vento_caracteristico_kn"].as_f64().unwrap() > 0.0);
    assert!(
        frame0["envoltoria_elu"]["momento_pos_viga_knm"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

#[test]
fn mcp_covers_plan_overlay_whatif_and_grid_analysis() {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    let built = app.run_mcp_tool(
        "executar_lote",
        &json!({
            "acoes": [
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 0, "y": 0, "largura": 5.0, "comprimento": 4.0 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Quarto", "x": 5.0, "y": 0, "largura": 3.5, "comprimento": 4.0 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
                {
                    "ferramenta": "projeto_estrutural",
                    "argumentos": {
                        "acao": "configurar_sobreposicao",
                        "cores_aproveitamento": true,
                        "diagrama_planta": "momento",
                        "deformada_planta": true,
                        "caminho_cargas": true
                    }
                },
            ]
        }),
    );
    assert!(!built.is_error, "{}", built.text);

    // #106 overlay + #109 grid analysis exposed in pre_dimensionamento.
    let report = app
        .run_mcp_tool("pre_dimensionamento", &json!({}))
        .structured
        .unwrap();
    assert_eq!(report["sobreposicao_planta"]["cores_aproveitamento"], true);
    assert_eq!(report["sobreposicao_planta"]["diagrama_planta"], "momento");
    let grid = &report["grelha_lajes_vigas"];
    assert!(grid["nos"].as_u64().unwrap() > 10);
    assert!(grid["erro_equilibrio_kn"].as_f64().unwrap() < 1e-2);
    assert!(!grid["lajes"].as_array().unwrap().is_empty());
    assert!(!grid["vigas"].as_array().unwrap().is_empty());

    // #107 What-if non-destructive evaluation and apply.
    let whatif = app.run_mcp_tool(
        "projeto_estrutural",
        &json!({
            "acao": "simular_whatif",
            "altura_viga_cm": 50.0,
            "fck_mpa": 30.0,
            "aplicar": false
        }),
    );
    assert!(!whatif.is_error, "{}", whatif.text);
    let wi = whatif.structured.unwrap();
    assert!(wi["elapsed_ms"].as_f64().unwrap() < 250.0);
    assert_eq!(wi["safe_ranges"].as_array().unwrap().len(), 5);

    // Export both plan SVG (with #106 overlay) and #109 grid SVG/PDF.
    let tmp = std::env::temp_dir().join(format!("trazo-mcp-sim-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let sheet_svg = tmp.join("prancha.svg");
    let grid_svg = tmp.join("grelha.svg");
    let grid_pdf = tmp.join("grelha.pdf");
    for (fmt, path) in [
        ("prancha_svg", &sheet_svg),
        ("grelha_svg", &grid_svg),
        ("grelha_pdf", &grid_pdf),
    ] {
        let res = app.run_mcp_tool(
            "exportar_planta",
            &json!({
                "formato": fmt,
                "disciplina": "estrutura",
                "caminho": path.display().to_string()
            }),
        );
        assert!(!res.is_error, "{fmt}: {}", res.text);
        assert!(path.is_file());
    }
    assert!(
        std::fs::read_to_string(&sheet_svg)
            .unwrap()
            .contains("simulacao-estrutural")
    );
    assert!(
        std::fs::read_to_string(&grid_svg)
            .unwrap()
            .contains("Analogia de Grelha")
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
