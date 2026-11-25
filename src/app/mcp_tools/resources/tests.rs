use super::*;
use crate::domain::{geometry::Point, model::Project};
use eframe::egui;

fn app() -> FloorPlanApp {
    let mut app = FloorPlanApp::new(&egui::Context::default());
    app.project = Project::default();
    app
}
fn run(app: &mut FloorPlanApp, name: &str, args: Value) -> Value {
    let out = app.run_mcp_tool(name, &args);
    assert!(!out.is_error, "{name}: {}", out.text);
    out.structured.unwrap_or(Value::Null)
}

#[test]
fn library_models_and_blocks_are_accessible_with_dimensions_and_attributes() {
    let mut app = app();
    let catalog = run(
        &mut app,
        "biblioteca",
        json!({"acao":"listar","origem":"catalogo","busca":"sofa retratil"}),
    );
    assert_eq!(catalog["itens"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["itens"][0]["largura"], 2.5);
    let item = run(
        &mut app,
        "biblioteca",
        json!({"acao":"criar","nome":"Bancada especial","tipo":"bancada","largura":2.4,"profundidade":0.7,"categoria":"cozinha"}),
    );
    let id = item["id"].as_u64().unwrap();
    let placed = run(
        &mut app,
        "biblioteca",
        json!({"acao":"inserir","origem":"projeto","id":id,"x":3,"y":4,"rotacao":90}),
    );
    let fixture = placed["id"].as_u64().unwrap();
    assert_eq!(app.project.fixtures[0].center, to_model(3.0, 4.0));
    assert_eq!(app.project.fixtures[0].width, 2.4);
    let block = run(
        &mut app,
        "blocos",
        json!({"acao":"criar","nome":"Cozinha modular","ids":[fixture],"atributos":[{"chave":"ACAB","padrao":"Madeira"}]}),
    );
    let bid = block["id"].as_u64().unwrap();
    let placed = run(
        &mut app,
        "blocos",
        json!({"acao":"inserir","id":bid,"x":6,"y":4,"valores":{"ACAB":"Granito"}}),
    );
    let iid = placed["instancia_id"].as_u64().unwrap();
    run(
        &mut app,
        "blocos",
        json!({"acao":"atributo","id":iid,"chave":"ACAB","valor":"Quartzo"}),
    );
    assert_eq!(
        app.project.block_instances[0].attribute_value("ACAB"),
        Some("Quartzo")
    );
    assert_eq!(
        run(&mut app, "blocos", json!({"acao":"listar"}))["instancias"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let before = app.project.clone();
    assert!(
        app.run_mcp_tool("biblioteca", &json!({"acao":"editar","id":id,"largura":-1}))
            .is_error
    );
    assert_eq!(before, app.project);
    run(
        &mut app,
        "biblioteca",
        json!({"acao":"editar","id":id,"nome":"Bancada atualizada","largura":2.8}),
    );
    assert_eq!(app.project.library[0].width_m, 2.8);
    app.undo();
    assert_eq!(app.project.library[0].width_m, 2.4);
    run(&mut app, "biblioteca", json!({"acao":"excluir","id":id}));
    assert!(app.project.library.is_empty());
    run(&mut app, "blocos", json!({"acao":"excluir","id":bid}));
    assert!(app.project.blocks.is_empty());
    assert_eq!(app.project.fixtures.len(), 2);
}

#[test]
fn styles_hatches_wall_assemblies_and_polygon_lots_round_trip() {
    let mut app = app();
    let room = app
        .project
        .add_room(Point::ORIGIN, Point::new(4.0, 3.0), "Sala")
        .unwrap();
    let wall = app.project.walls[0].id;
    run(
        &mut app,
        "documentacao",
        json!({"acao":"configurar_estilo","estilo":"visual","unidade":"m","terminacao":"seta","pena_parede_mm":0.45}),
    );
    let d = run(&mut app, "documentacao", json!({"acao":"obter"}));
    assert_eq!(d["estilo"]["unidade"], "m");
    assert_eq!(d["estilo"]["pena_parede_mm"], 0.45);
    run(
        &mut app,
        "documentacao",
        json!({"acao":"configurar_tabelas","quantitativos":true,"perda_pct":15}),
    );
    assert!(app.project.schedules.show_quantities_schedule);
    assert_eq!(app.project.schedules.waste_percent, 15.0);
    run(
        &mut app,
        "acabamentos",
        json!({"acao":"hachura","id":room,"padrao_id":1,"espacamento_m":0.4,"cor":"#5588ff","angulo_graus":30}),
    );
    assert_eq!(app.project.rooms[0].hatch.unwrap().angle_deg, -30.0);
    run(
        &mut app,
        "acabamentos",
        json!({"acao":"composicao","id":wall,"nome":"Parede customizada","camadas":[{"nome":"Tijolo","funcao":"estrutura","espessura_m":0.14},{"nome":"Reboco","funcao":"reboco","espessura_m":0.02}]}),
    );
    assert!((app.project.walls[0].thickness() - 0.16).abs() < 1e-8);
    let before = app.project.clone();
    assert!(app.run_mcp_tool("acabamentos",&json!({"acao":"composicao","id":wall,"camadas":[{"nome":"Inválida","funcao":"estrutura","espessura_m":-1}]})).is_error);
    assert_eq!(app.project, before);
    run(
        &mut app,
        "terreno",
        json!({"acao":"definir_poligono","pontos":[{"x":-2,"y":2},{"x":10,"y":2},{"x":10,"y":-10},{"x":-2,"y":-10}]}),
    );
    let lots = run(
        &mut app,
        "terreno",
        json!({"acao":"subdividir","x1":6,"y1":3,"x2":6,"y2":-11}),
    );
    assert_eq!(lots["lotes"].as_array().unwrap().len(), 2);
    let id = lots["lotes"][0].as_u64().unwrap();
    run(
        &mut app,
        "terreno",
        json!({"acao":"renomear_lote","id":id,"nome":"Lote A"}),
    );
    assert_eq!(
        run(&mut app, "terreno", json!({"acao":"obter"}))["lotes"][0]["nome"],
        "Lote A"
    );
}

#[test]
fn references_calibration_comparison_and_export_use_real_files() {
    let mut app = app();
    let dir = std::env::temp_dir().join(format!("trazo-mcp-resources-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("fundo.png");
    image::RgbaImage::from_pixel(100, 50, image::Rgba([240, 240, 240, 255]))
        .save(&image)
        .unwrap();
    let data = run(
        &mut app,
        "referencias",
        json!({"acao":"adicionar","tipo":"fundo","caminho":image.display().to_string(),"x":2,"y":3,"largura":10}),
    );
    let id = data["id"].as_u64().unwrap();
    assert_eq!(app.project.underlays[0].origin, to_model(2.0, 3.0));
    run(
        &mut app,
        "referencias",
        json!({"acao":"calibrar","tipo":"fundo","id":id,"x1":2,"y1":3,"x2":12,"y2":3,"distancia_real":5}),
    );
    assert_eq!(app.project.underlays[0].width_m, 5.0);
    run(
        &mut app,
        "referencias",
        json!({"acao":"editar","id":id,"angulo_graus":30,"bloqueada":true}),
    );
    let before = app.project.clone();
    assert!(
        app.run_mcp_tool(
            "referencias",
            &json!({"acao":"calibrar","id":id,"x1":0,"y1":0,"x2":1,"y2":0,"distancia_real":2})
        )
        .is_error
    );
    assert_eq!(app.project, before);
    let baseline = dir.join("referencia.trazo");
    crate::io::project_file::save(&Project::default(), &baseline).unwrap();
    let xref = run(
        &mut app,
        "referencias",
        json!({"acao":"adicionar","tipo":"externa","caminho":baseline.display().to_string()}),
    );
    let xid = xref["id"].as_u64().unwrap();
    run(
        &mut app,
        "referencias",
        json!({"acao":"recarregar","tipo":"externa","id":xid}),
    );
    app.project
        .add_wall(Point::ORIGIN, Point::new(4.0, 0.0))
        .unwrap();
    let compare = run(
        &mut app,
        "referencias",
        json!({"acao":"comparar","caminho":baseline.display().to_string(),"exibir":true}),
    );
    assert_eq!(compare["adicionados"], 1);
    assert!(app.compare_state.overlay_active);
    let svg = dir.join("comparacao.svg");
    run(
        &mut app,
        "referencias",
        json!({"acao":"exportar_comparacao","caminho":svg.display().to_string()}),
    );
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));
    run(&mut app, "referencias", json!({"acao":"limpar_comparacao"}));
    assert!(!app.compare_state.overlay_active);
    run(
        &mut app,
        "referencias",
        json!({"acao":"editar","id":id,"bloqueada":false}),
    );
    run(&mut app, "referencias", json!({"acao":"remover","id":id}));
    assert!(app.project.underlays.is_empty());
    for path in [image, baseline, svg] {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(&dir).unwrap();
}

#[test]
fn templates_respect_unsaved_changes_and_every_new_tool_is_discoverable() {
    let mut app = app();
    let listing = run(&mut app, "modelos", json!({"acao":"listar"}));
    assert_eq!(listing["modelos"].as_array().unwrap().len(), 4);
    app.dirty = true;
    assert!(
        app.run_mcp_tool("modelos", &json!({"acao":"aplicar","id":1}))
            .is_error
    );
    run(
        &mut app,
        "modelos",
        json!({"acao":"aplicar","id":4,"descartar_alteracoes":true}),
    );
    assert!(!app.project.rooms.is_empty());
    assert!(app.file_path.is_none());
    let names = crate::mcp::protocol::tool_names();
    for (tool, action) in [
        ("biblioteca", "listar"),
        ("blocos", "listar"),
        ("referencias", "listar"),
        ("documentacao", "obter"),
        ("modelos", "listar"),
        ("acabamentos", "listar"),
        ("terreno", "obter"),
    ] {
        assert!(names.contains(&tool));
        run(&mut app, tool, json!({"acao":action}));
    }
    let before = app.project.clone();
    assert!(app.run_mcp_tool("executar_lote",&json!({"acoes":[{"ferramenta":"biblioteca","argumentos":{"acao":"compartilhar","origem":"catalogo","id":1}}]})).is_error);
    assert_eq!(before, app.project);
}
