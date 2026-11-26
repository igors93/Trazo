use serde_json::{Value, json};

use crate::app::FloorPlanApp;
use crate::domain::{geometry::Point, sheet_set::SheetStatus};
use crate::mcp::ToolOutput;
use eframe::egui;

fn app() -> FloorPlanApp {
    FloorPlanApp::new(&egui::Context::default())
}

fn ok(app: &mut FloorPlanApp, name: &str, args: Value) -> ToolOutput {
    let out = app.run_mcp_tool(name, &args);
    assert!(!out.is_error, "{name}: {}", out.text);
    out
}

#[test]
fn sheet_release_gate_is_enforced_through_mcp_and_export() {
    let mut app = app();
    app.project
        .set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
    app.project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    app.project.generate_structure();
    let beam = app.project.structure.beams[0].id;
    app.project.update_beam(beam, None, Some(0.20)).unwrap();
    let out = ok(
        &mut app,
        "prancha",
        json!({"acao": "adicionar_ao_conjunto", "titulo": "Forma"}),
    );
    let id = out.structured.unwrap()["id"].as_u64().unwrap();
    let denied = app.run_mcp_tool(
        "prancha",
        &json!({
            "acao": "definir_situacao", "id": id, "situacao": "liberado_obra"
        }),
    );
    assert!(denied.is_error);
    assert_eq!(
        app.project.sheet_set.sheets[0].status,
        SheetStatus::EmElaboracao
    );

    ok(
        &mut app,
        "prancha",
        json!({
            "acao": "definir_situacao", "id": id, "situacao": "liberado_obra",
            "revisor": "Ana Souza", "registro_revisor": "CREA 12345", "data_revisao": "23/11/2026"
        }),
    );
    let listed = ok(&mut app, "prancha", json!({"acao": "listar_conjunto"}));
    assert_eq!(
        listed.structured.unwrap()["pranchas"][0]["situacao"],
        "Liberado para obra"
    );
    let path = std::env::temp_dir().join(format!("trazo-gate-{}.svg", std::process::id()));
    ok(
        &mut app,
        "prancha",
        json!({
            "acao": "exportar_conjunto", "id": id, "formato": "svg", "caminho": path.display().to_string()
        }),
    );
    let markup = std::fs::read_to_string(&path).unwrap();
    assert!(markup.contains("VERMELHOS REVISADOS"));
    assert!(markup.contains("Ana Souza"));

    app.project.update_beam(beam, None, Some(0.18)).unwrap();
    ok(
        &mut app,
        "prancha",
        json!({
            "acao": "exportar_conjunto", "id": id, "formato": "svg", "caminho": path.display().to_string()
        }),
    );
    let markup = std::fs::read_to_string(&path).unwrap();
    assert!(markup.contains("Para aprovação"));
    assert!(!markup.contains("VERMELHOS REVISADOS"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn quantities_ergonomics_finishes_setbacks_and_openings_work_via_mcp() {
    let mut app = app();
    ok(
        &mut app,
        "definir_terreno",
        json!({ "largura": 12, "comprimento": 20, "recuo_frente": 2.0, "recuo_fundos": 1.5, "recuo_lateral": 1.0 }),
    );
    // Update only setbacks without respecifying width/length.
    ok(&mut app, "definir_terreno", json!({ "recuo_frente": 1.5 }));
    assert!((app.project.setbacks.front - 1.5).abs() < 1e-9);

    let room = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Sala", "x": 2, "y": 3, "largura": 5, "comprimento": 4 }),
    );
    let room_id = room.structured.as_ref().unwrap()["id"].as_u64().unwrap();
    let wall_id = room.structured.as_ref().unwrap()["paredes"][0]
        .as_u64()
        .unwrap();

    // Finishes preset + custom override.
    ok(
        &mut app,
        "editar_comodo",
        json!({
            "id": room_id,
            "preset_acabamento": "social_porcelanato",
            "piso": "Porcelanato 80x80",
            "rodape_cm": 12,
            "impermeabilizacao": true
        }),
    );

    // Door & window typologies.
    let door = ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall_id, "tipo": "porta", "posicao": 0.3 }),
    );
    let door_id = door.structured.as_ref().unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "editar_abertura",
        json!({ "id": door_id, "tipo_porta": "pivotante" }),
    );
    let win = ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall_id, "tipo": "janela", "posicao": 0.75 }),
    );
    let win_id = win.structured.as_ref().unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "editar_abertura",
        json!({ "id": win_id, "tipo_janela": "maxim_ar" }),
    );

    // Free angle on fixture.
    let table = ok(
        &mut app,
        "inserir_objeto",
        json!({ "tipo": "mesa", "x": 4.5, "y": 5.0 }),
    );
    let table_id = table.structured.as_ref().unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "editar_objeto",
        json!({ "id": table_id, "angulo_graus": 30 }),
    );

    let plan = ok(&mut app, "obter_planta", json!({})).structured.unwrap();
    assert_eq!(plan["terreno"]["recuos"]["frente"], 1.5);
    assert_eq!(
        plan["comodos"][0]["acabamentos"]["piso"],
        "Porcelanato 80x80"
    );
    assert_eq!(plan["comodos"][0]["acabamentos"]["impermeabilizacao"], true);
    assert_eq!(plan["aberturas"][0]["tipologia"], "Pivotante");
    assert_eq!(plan["aberturas"][1]["tipologia"], "Maxim-ar");
    assert_eq!(plan["objetos"][0]["angulo_graus"], 30.0);

    // Structure + roof + soil survey + quantities.
    ok(
        &mut app,
        "projeto_estrutural",
        json!({ "acao": "gerar_estrutura" }),
    );
    ok(
        &mut app,
        "projeto_estrutural",
        json!({ "acao": "configurar", "peso_alvenaria_kn_m3": 15.0, "sobrecarga_por_comodo": false }),
    );
    let roof = ok(
        &mut app,
        "projeto_estrutural",
        json!({
            "acao": "definir_telhado",
            "x": 2, "y": 3, "largura_m": 5, "comprimento_m": 4,
            "lado_alto": "oeste", "altura_alta_m": 3.8, "altura_baixa_m": 2.6,
            "ajustar_paredes": false
        }),
    );
    let roof_id = roof.structured.unwrap()["id"].as_u64().unwrap();
    let fitted = ok(
        &mut app,
        "projeto_estrutural",
        json!({ "acao": "ajustar_paredes_telhado", "id": roof_id }),
    );
    assert!(
        fitted.structured.unwrap()["paredes_ajustadas"]
            .as_u64()
            .unwrap()
            > 0
    );

    ok(
        &mut app,
        "projeto_estrutural",
        json!({
            "acao": "definir_sondagem",
            "nome": "SP-01",
            "leituras": [
                { "profundidade_m": 1, "n": 8, "solo": "argila" },
                { "profundidade_m": 2, "n": 10, "solo": "argila" },
                { "profundidade_m": 3, "n": 12, "solo": "areia" }
            ]
        }),
    );
    let presizing = ok(&mut app, "pre_dimensionamento", json!({}))
        .structured
        .unwrap();
    assert_eq!(presizing["materiais"]["peso_alvenaria_kn_m3"], 15.0);
    assert_eq!(presizing["materiais"]["sobrecarga_por_comodo"], false);
    assert_eq!(presizing["sondagem"]["nome"], "SP-01");
    assert!(presizing["estudo_fundacoes"]["opcoes"].is_array());

    let qty = ok(&mut app, "quantitativos", json!({})).structured.unwrap();
    assert!(qty["totais"]["alvenaria_liquida_m2"].as_f64().unwrap() > 0.0);
    assert!(qty["totais"]["aco_preliminar_kg"].as_f64().unwrap() > 0.0);
    assert!(qty["telhados"]["madeira_m3"].as_f64().unwrap() > 0.0);
    assert!(!qty["vergas_contravergas"].as_array().unwrap().is_empty());

    let ergo = ok(&mut app, "ergonomia", json!({ "passagem_minima_m": 0.90 }))
        .structured
        .unwrap();
    assert_eq!(ergo["passagem_minima_m"], 0.9);
    assert!(ergo["ocorrencias"].is_array());
}

#[test]
fn layers_notes_dimensions_circulation_views_sheet_and_exports_work_via_mcp() {
    let mut app = app();
    ok(
        &mut app,
        "definir_terreno",
        json!({ "largura": 14, "comprimento": 18 }),
    );
    let room = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Sala", "x": 1, "y": 1, "largura": 6, "comprimento": 5 }),
    );
    let wall_id = room.structured.unwrap()["paredes"][0].as_u64().unwrap();

    // Layers.
    let layer = ok(
        &mut app,
        "camadas",
        json!({ "acao": "criar", "nome": "Demolir", "cor": "#c62828", "espessura_mm": 0.30, "tipo_linha": "tracejada", "ativar": true }),
    );
    let layer_id = layer.structured.unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "camadas",
        json!({ "acao": "atribuir", "id": layer_id, "parede_id": wall_id }),
    );
    ok(
        &mut app,
        "camadas",
        json!({ "acao": "editar", "id": layer_id, "visivel": false, "bloqueada": true }),
    );
    let layers = ok(&mut app, "camadas", json!({ "acao": "listar" }))
        .structured
        .unwrap();
    assert!(
        layers["camadas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["id"] == layer_id && l["visivel"] == false && l["bloqueada"] == true)
    );
    ok(
        &mut app,
        "camadas",
        json!({ "acao": "excluir", "id": layer_id }),
    );
    // Deleting a built-in layer must fail.
    assert!(
        app.run_mcp_tool("camadas", &json!({ "acao": "excluir", "id": 1 }))
            .is_error
    );

    // Notes & symbols.
    let note = ok(
        &mut app,
        "anotacoes",
        json!({ "acao": "adicionar", "tipo": "chamada", "x": 3, "y": 3, "alvo_x": 2, "alvo_y": 2, "texto": "Ralo linear" }),
    );
    let note_id = note.structured.unwrap()["id"].as_u64().unwrap();
    ok(
        &mut app,
        "anotacoes",
        json!({ "acao": "adicionar", "tipo": "cota_nivel", "x": 4, "y": 4, "nivel_m": 0.15 }),
    );
    ok(
        &mut app,
        "anotacoes",
        json!({ "acao": "editar", "id": note_id, "texto": "Ralo oculto", "dx": 0.5, "dy": 0.2 }),
    );
    let listed_notes = ok(&mut app, "anotacoes", json!({ "acao": "listar" }))
        .structured
        .unwrap();
    assert_eq!(listed_notes["anotacoes"].as_array().unwrap().len(), 2);
    ok(
        &mut app,
        "anotacoes",
        json!({ "acao": "remover", "id": note_id }),
    );
    assert_eq!(app.project.notes.len(), 1);

    // Dimensions.
    let generated = ok(&mut app, "cotas", json!({ "acao": "gerar_internas" }))
        .structured
        .unwrap();
    assert_eq!(generated["adicionadas"], 2);
    // Opening dimension chains (#53): add a window on wall_id first.
    app.project
        .add_opening(wall_id, crate::domain::model::OpeningKind::Window, 0.5)
        .unwrap();
    let chains = ok(
        &mut app,
        "cotas",
        json!({ "acao": "gerar_aberturas", "deslocamento": 0.50 }),
    )
    .structured
    .unwrap();
    let chain_count = chains["adicionadas"].as_u64().unwrap();
    assert!(chain_count >= 3, "expected opening chain dimensions");
    assert!(chains["paredes"].as_u64().unwrap() >= 1);
    let wall_dim = ok(
        &mut app,
        "cotas",
        json!({ "acao": "adicionar_parede", "parede_id": wall_id, "deslocamento": 0.6 }),
    )
    .structured
    .unwrap()["id"]
        .as_u64()
        .unwrap();
    ok(
        &mut app,
        "cotas",
        json!({ "acao": "editar", "id": wall_dim, "texto": "6,00 m (eixo)" }),
    );
    let dims = ok(&mut app, "cotas", json!({ "acao": "listar" }))
        .structured
        .unwrap();
    assert_eq!(
        dims["cotas"].as_array().unwrap().len(),
        2 + chain_count as usize + 1
    );
    ok(
        &mut app,
        "cotas",
        json!({ "acao": "remover", "id": wall_dim }),
    );
    assert_eq!(app.project.dimensions.len(), 2 + chain_count as usize);

    // Circulation.
    let stair = ok(
        &mut app,
        "circulacao",
        json!({ "acao": "adicionar_escada", "x": 2, "y": 2, "formato": "em_l", "largura_m": 1.1, "desnivel_m": 2.8, "piso_m": 0.28 }),
    )
    .structured
    .unwrap();
    assert_eq!(stair["blondel_ok"], true);
    let ramp = ok(
        &mut app,
        "circulacao",
        json!({ "acao": "adicionar_rampa", "x": 8, "y": 2, "largura_m": 1.2, "comprimento_m": 6.0, "desnivel_m": 0.45 }),
    )
    .structured
    .unwrap();
    assert_eq!(ramp["nbr_9050_ok"], true);
    ok(
        &mut app,
        "circulacao",
        json!({ "acao": "adicionar_patamar", "x": 8, "y": 8, "largura_m": 1.5, "profundidade_m": 1.5, "nivel_m": 0.45 }),
    );
    ok(
        &mut app,
        "circulacao",
        json!({ "acao": "adicionar_guarda_corpo", "x1": 8, "y1": 8, "x2": 9.5, "y2": 8, "altura_m": 1.1 }),
    );
    let dc_id = ok(
        &mut app,
        "circulacao",
        json!({ "acao": "adicionar_desnivel", "x1": 7, "y1": 1, "x2": 7, "y2": 6, "nivel_de_m": 0.15, "nivel_para_m": 0.0 }),
    )
    .structured
    .unwrap()["id"]
        .as_u64()
        .unwrap();
    assert_eq!(
        ok(&mut app, "circulacao", json!({ "acao": "listar" }))
            .structured
            .unwrap()["circulacoes"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    ok(
        &mut app,
        "circulacao",
        json!({ "acao": "remover", "id": dc_id }),
    );
    assert_eq!(app.project.circulations.len(), 4);

    // Technical views & sheet configuration.
    let sec_id = ok(
        &mut app,
        "vistas_tecnicas",
        json!({ "acao": "adicionar_corte_padrao", "horizontal": true }),
    )
    .structured
    .unwrap()["id"]
        .as_u64()
        .unwrap();
    ok(
        &mut app,
        "vistas_tecnicas",
        json!({ "acao": "adicionar_fachada", "lado": "sul", "prancha_ref": "PR-02" }),
    );
    ok(
        &mut app,
        "vistas_tecnicas",
        json!({ "acao": "adicionar_detalhe", "x": 3, "y": 3, "raio_m": 1.2, "escala": "1:20" }),
    );
    let views = ok(&mut app, "vistas_tecnicas", json!({ "acao": "listar" }))
        .structured
        .unwrap();
    assert_eq!(views["cortes"].as_array().unwrap().len(), 1);
    assert_eq!(views["fachadas"].as_array().unwrap().len(), 1);
    assert_eq!(views["detalhes"].as_array().unwrap().len(), 1);

    let sheet = ok(
        &mut app,
        "prancha",
        json!({
            "acao": "configurar",
            "formato": "A2",
            "orientacao": "paisagem",
            "escala": "melhor",
            "titulo": "Planta Arquitetônica",
            "cliente": "Cliente Teste",
            "autor": "Arq. Trazo",
            "registro": "CAU A12345-6",
            "folha": "01/02",
            "revisao": "R01"
        }),
    )
    .structured
    .unwrap();
    assert_eq!(sheet["formato"], "A2");
    assert_eq!(sheet["carimbo"]["cliente"], "Cliente Teste");

    // Exports (view SVG + plan SVG + scaled sheet PDF + report PDF + CSVs).
    ok(
        &mut app,
        "projeto_estrutural",
        json!({ "acao": "gerar_estrutura" }),
    );
    let tmp = std::env::temp_dir().join(format!("trazo-mcp-exports-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let sec_svg = tmp.join("corte.svg");
    ok(
        &mut app,
        "vistas_tecnicas",
        json!({ "acao": "exportar", "id": sec_id, "formato": "svg", "caminho": sec_svg.display().to_string() }),
    );
    assert!(sec_svg.is_file());

    for (fmt, fname) in [
        ("svg", "planta.svg"),
        ("prancha_svg", "prancha.svg"),
        ("prancha_pdf", "prancha.pdf"),
        ("memorial_pdf", "memorial.pdf"),
        ("unifilar_svg", "unifilar.svg"),
        ("esforcos_svg", "esforcos.svg"),
        ("quantitativos_csv", "quantitativos.csv"),
        ("tabela_aco_csv", "tabela_aco.csv"),
        ("csv", "quadros.csv"),
    ] {
        let p = tmp.join(fname);
        let res = ok(
            &mut app,
            "exportar_planta",
            json!({ "formato": fmt, "caminho": p.display().to_string() }),
        );
        assert!(p.is_file(), "missing export for {fmt}");
        assert!(
            std::fs::metadata(&p).unwrap().len() > 0,
            "empty export for {fmt}"
        );
        if fmt == "memorial_pdf" {
            assert!(res.structured.unwrap()["paginas"].as_u64().unwrap() >= 1);
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    // Undo works cleanly across new tools.
    let before_undo = app.project.structure.beams.len();
    assert!(before_undo > 0);
    ok(&mut app, "desfazer", json!({ "passos": 1 }));
    assert_eq!(app.project.structure.beams.len(), 0);
}

#[test]
fn multi_floor_quantities_and_sheet_exports_work_via_mcp() {
    let mut app = app();
    ok(
        &mut app,
        "definir_terreno",
        json!({ "largura": 12, "comprimento": 16 }),
    );
    let sala = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Sala", "x": 1, "y": 1, "largura": 5, "comprimento": 4 }),
    )
    .structured
    .unwrap();
    let sala_id = sala["id"].as_u64().unwrap();
    let wall_id = sala["paredes"][0].as_u64().unwrap();
    ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall_id, "tipo": "porta", "posicao": 0.3 }),
    );
    ok(
        &mut app,
        "inserir_abertura",
        json!({ "parede_id": wall_id, "tipo": "janela", "posicao": 0.7 }),
    );
    ok(
        &mut app,
        "editar_comodo",
        json!({ "id": sala_id, "preset_acabamento": "social_porcelanato" }),
    );
    let banho = ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Banheiro", "x": 6, "y": 1, "largura": 2.5, "comprimento": 3 }),
    )
    .structured
    .unwrap();
    let banho_id = banho["id"].as_u64().unwrap();
    ok(
        &mut app,
        "editar_comodo",
        json!({ "id": banho_id, "preset_acabamento": "area_molhada_banheiro" }),
    );

    // Add upper floor copying walls and add a bedroom there.
    ok(
        &mut app,
        "pavimentos",
        json!({ "acao": "adicionar", "nome": "1º Pavimento", "copiar_paredes": true }),
    );
    ok(
        &mut app,
        "criar_comodo",
        json!({ "nome": "Quarto", "x": 1, "y": 6, "largura": 4, "comprimento": 3.5 }),
    );

    let single = ok(&mut app, "quantitativos", json!({})).structured.unwrap();
    assert_eq!(single["todos_pavimentos"], false);
    assert_eq!(single["por_pavimento"].as_array().unwrap().len(), 1);

    let all = ok(
        &mut app,
        "quantitativos",
        json!({ "todos_pavimentos": true, "perda_blocos_pct": 15.0 }),
    )
    .structured
    .unwrap();
    assert_eq!(all["todos_pavimentos"], true);
    assert_eq!(all["perda_percentual"], 15.0);
    assert_eq!(all["por_pavimento"].as_array().unwrap().len(), 2);
    assert!(
        all["totais"]["alvenaria_liquida_m2"].as_f64().unwrap()
            > single["totais"]["alvenaria_liquida_m2"].as_f64().unwrap()
    );
    assert!(all["totais"]["graute_canaletas_m3"].as_f64().unwrap() > 0.0);
    assert!(
        all["totais"]["paredes_internas_com_perda_m2"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(all["totais"]["impermeabilizacao_m2"].as_f64().unwrap() > 0.0);

    let first_mat = &all["alvenaria"].as_array().unwrap()[0];
    assert!(
        first_mat["canaletas_com_perda"].as_u64().unwrap()
            >= first_mat["canaletas"].as_u64().unwrap()
    );
    assert_eq!(
        first_mat["canaletas"].as_u64().unwrap(),
        first_mat["canaletas_cinta"].as_u64().unwrap()
            + first_mat["canaletas_vergas"].as_u64().unwrap()
    );
    let lintels = all["vergas_contravergas"].as_array().unwrap();
    assert!(!lintels.is_empty());
    assert!(
        lintels[0]["codigo"].as_str().unwrap().starts_with('P')
            || lintels[0]["codigo"].as_str().unwrap().starts_with('J')
    );
    assert!(lintels[0]["transpasse_lado_m"].as_f64().unwrap() >= 0.30);

    let rooms = all["acabamentos"].as_array().unwrap();
    assert!(
        rooms
            .iter()
            .any(|r| r["impermeabilizacao_m2"].as_f64().unwrap() > 0.0)
    );
    assert!(rooms.iter().any(|r| {
        r["acabamento_piso"]
            .as_str()
            .unwrap()
            .contains("Porcelanato")
    }));

    // Export SVG and PDF takeoff sheets with todos_pavimentos=true.
    let tmp = std::env::temp_dir().join(format!("trazo-mcp-qty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    for (fmt, fname) in [
        ("quantitativos_svg", "quantitativos.svg"),
        ("quantitativos_pdf", "quantitativos.pdf"),
        ("quantitativos_csv", "quantitativos.csv"),
    ] {
        let p = tmp.join(fname);
        ok(
            &mut app,
            "exportar_planta",
            json!({
                "formato": fmt,
                "todos_pavimentos": true,
                "caminho": p.display().to_string()
            }),
        );
        assert!(p.is_file(), "missing export for {fmt}");
        assert!(std::fs::metadata(&p).unwrap().len() > 0);
    }
    let svg_text = std::fs::read_to_string(tmp.join("quantitativos.svg")).unwrap();
    assert!(svg_text.contains("Resumo por pavimento"));
    assert!(svg_text.contains("Todos os pavimentos"));
    let _ = std::fs::remove_dir_all(&tmp);
}
