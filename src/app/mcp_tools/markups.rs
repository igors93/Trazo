//! Structured access to review marks, with explicit and geometric references.

use super::{Change, element_id, flag, optional_count, required_number, text, to_model, xy};
use crate::{
    app::{FloorPlanApp, RibbonTab, Tool},
    domain::{
        markups::{Markup, MarkupKind},
        model::Project,
    },
    mcp::ToolOutput,
};
use serde_json::{Value, json};

pub(super) fn color(raw: &str) -> Result<[u8; 3], String> {
    match raw.trim().to_lowercase().as_str() {
        "vermelho" | "vermelha" => Ok([220, 65, 65]),
        "azul" => Ok([38, 112, 216]),
        "verde" => Ok([35, 142, 86]),
        "laranja" => Ok([231, 145, 32]),
        "roxo" | "roxa" => Ok([143, 80, 191]),
        "amarelo" | "amarela" => Ok([240, 194, 35]),
        _ => super::project_docs::parse_hex_color(raw),
    }
}

pub(super) fn markup_json(m: &Markup, p: &Project) -> Value {
    let (a, b) = m.bounds();
    let refs = |items: Vec<crate::domain::element::Element>| {
        items.into_iter().map(|e|json!({"id":e.id(),"tipo":e.kind().singular().to_lowercase(),"existe":p.element_exists(e)})).collect::<Vec<_>>()
    };
    let mut related = refs(m.related_elements(p));
    related.extend(
        m.related_resources(p)
            .into_iter()
            .map(|r| json!({"id":r.id,"tipo":r.kind,"existe":true})),
    );
    json!({"id":m.id,"referencia":m.label(),"tipo":m.kind.key(),"cor":format!("#{:02x}{:02x}{:02x}",m.color[0],m.color[1],m.color[2]),"comentario":m.comment,"resolvida":m.resolved,"pontos":m.points.iter().copied().map(xy).collect::<Vec<_>>(),"limites":{"min":xy(crate::domain::geometry::Point::new(a.x,b.y)),"max":xy(crate::domain::geometry::Point::new(b.x,a.y))},"vinculos":refs(m.linked_elements.clone()),"elementos_relacionados":related,"pavimento_id":p.floors.active_floor_id,"pavimento":p.active_floor().map(|f|f.name.as_str()).unwrap_or("Térreo")})
}

fn points(args: &Value) -> Result<Vec<crate::domain::geometry::Point>, String> {
    let list = args
        .get("pontos")
        .and_then(Value::as_array)
        .ok_or("Informe 'pontos': lista de {x,y} em metros.")?;
    if !(2..=crate::domain::markups::MAX_POINTS).contains(&list.len()) {
        return Err("Use de 2 a 2048 pontos.".into());
    }
    list.iter()
        .map(|p| Ok(to_model(required_number(p, "x")?, required_number(p, "y")?)))
        .collect()
}
fn kind(raw: &str) -> Result<MarkupKind, String> {
    match raw {
        "retangulo" | "retângulo" => Ok(MarkupKind::Rectangle),
        "circulo" | "círculo" => Ok(MarkupKind::Circle),
        "seta" => Ok(MarkupKind::Arrow),
        "traco_livre" | "traço_livre" => Ok(MarkupKind::Freehand),
        _ => Err("Tipo: retangulo, circulo, seta ou traco_livre.".into()),
    }
}

impl FloorPlanApp {
    pub(super) fn markup_summary(&self) -> Value {
        json!({"configuracao":self.project.markup_settings,"marcacoes":self.project.markups.iter().map(|m|markup_json(m,&self.project)).collect::<Vec<_>>()})
    }
    pub(super) fn tool_read_markups(&self, args: &Value) -> Result<ToolOutput, String> {
        let wanted = optional_count(args, "id")?;
        let wanted_floor = optional_count(args, "pavimento_id")?;
        let filter_color = text(args, "cor").map(|c| color(&c)).transpose()?;
        let resolved = args.get("resolvida").map(|_| flag(args, "resolvida"));
        let mut data = Vec::new();
        let mut collect = |p: &Project| {
            data.extend(
                p.markups
                    .iter()
                    .filter(|m| {
                        wanted.is_none_or(|id| id == m.id)
                            && filter_color.is_none_or(|c| c == m.color)
                            && resolved.is_none_or(|r| r == m.resolved)
                    })
                    .map(|m| markup_json(m, p)),
            );
        };
        if let Some(id) = wanted_floor {
            let mut p = self.project.clone();
            if p.floors.active_floor_id != Some(id) {
                p.switch_floor(id)?;
            }
            collect(&p);
        } else {
            collect(&self.project);
            if flag(args, "todos_pavimentos") {
                for floor in &self.project.floors.levels {
                    if Some(floor.id) != self.project.floors.active_floor_id {
                        let mut p = self.project.clone();
                        p.switch_floor(floor.id)?;
                        collect(&p);
                    }
                }
            }
        }
        Ok(ToolOutput::ok(format!("{} marcações encontradas.",data.len())).with_structured(json!({"marcacoes":data,"revisao":self.plan_log.revision,"configuracao":self.project.markup_settings})))
    }
    pub(super) fn tool_markups(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            return self.tool_read_markups(args);
        }
        if let Some(id) = optional_count(args, "pavimento_id")?
            && self.project.floors.active_floor_id != Some(id)
        {
            return Err("Ative o pavimento com pavimentos antes de editar suas marcações.".into());
        }
        if action == "selecionar" {
            let id = element_id(args, "id")?;
            let m = self
                .project
                .markups
                .iter()
                .find(|m| m.id == id)
                .cloned()
                .ok_or("Marcação não encontrada neste pavimento.")?;
            self.set_tool(Tool::Markup(None));
            self.ribbon_tab = RibbonTab::Markups;
            self.ribbon_collapsed = false;
            self.markup_state.selected = Some(id);
            self.markup_state.edit = Some(m.clone());
            let before = self.project.clone();
            self.project.markup_settings.visible = true;
            if m.resolved {
                self.project.markup_settings.show_resolved = true;
            }
            self.record_change(before);
            let (a, b) = m.bounds();
            self.zoom_target = Some((
                a.translated(crate::domain::geometry::Point::new(-1.0, -1.0)),
                b.translated(crate::domain::geometry::Point::new(1.0, 1.0)),
            ));
            let data = markup_json(&m, &self.project);
            return Ok(ToolOutput::ok(format!("Marcação M{id} selecionada.")).with_structured(data));
        }
        if action == "configurar" {
            return self.apply_change(None, |p| {
                if args.get("visivel").is_some() {
                    p.markup_settings.visible = flag(args, "visivel");
                }
                if args.get("mostrar_resolvidas").is_some() {
                    p.markup_settings.show_resolved = flag(args, "mostrar_resolvidas");
                }
                if args.get("exportar").is_some() {
                    p.markup_settings.export = flag(args, "exportar");
                }
                Ok(
                    Change::new("Exibição das marcações atualizada.")
                        .data(json!(p.markup_settings)),
                )
            });
        }
        let parsed_points = if args.get("pontos").is_some() {
            Some(points(args)?)
        } else {
            None
        };
        let parsed_kind = text(args, "tipo").map(|t| kind(&t)).transpose()?;
        let parsed_color = text(args, "cor").map(|t| color(&t)).transpose()?;
        if args.get("comentario").is_some_and(|v| !v.is_string()) {
            return Err("comentario deve ser texto.".into());
        }
        let links = if args.get("ids").is_some() {
            Some(self.elements_of(args)?)
        } else {
            None
        };
        self.apply_change(None,|p| {
            if action=="criar" {
                let m=Markup {id:0,kind:parsed_kind.ok_or("Informe 'tipo'.")?,points:parsed_points.ok_or("Informe 'pontos'.")?,color:parsed_color.unwrap_or([220,65,65]),comment:args.get("comentario").and_then(Value::as_str).unwrap_or("").to_owned(),resolved:flag(args,"resolvida"),linked_elements:links.unwrap_or_default()};
                let id=p.add_markup(m)?; let m=p.markups.iter().find(|m|m.id==id).ok_or("Não foi possível localizar a marcação criada.")?;
                return Ok(Change::new(format!("Marcação M{id} criada.")).data(markup_json(m,p)));
            }
            let id=element_id(args,"id")?;
            let mut m=p.markups.iter().find(|m|m.id==id).cloned().ok_or("Marcação não encontrada neste pavimento.")?;
            match action.as_str() {
                "excluir"=>{p.delete_markup(id);return Ok(Change::new(format!("Marcação M{id} excluída.")));},
                "resolver"=>m.resolved=true,
                "reabrir"=>m.resolved=false,
                "mover"=>{let d=to_model(required_number(args,"dx")?,required_number(args,"dy")?);m.points.iter_mut().for_each(|v|*v=v.translated(d));},
                "editar"=>{
                    if let Some(points)=parsed_points {m.points=points;}
                    if let Some(kind)=parsed_kind {m.kind=kind;}
                    if let Some(c)=parsed_color {m.color=c;}
                    if let Some(v)=args.get("comentario") {m.comment=v.as_str().ok_or("comentario deve ser texto.")?.to_owned();}
                    if args.get("resolvida").is_some(){m.resolved=flag(args,"resolvida");}
                    if let Some(links)=links {m.linked_elements=links;}
                },
                _=>return Err("Ação: listar, criar, editar, mover, excluir, resolver, reabrir, selecionar ou configurar.".into()),
            }
            let data=markup_json(&m,p);p.update_markup(m)?;
            Ok(Change::new(format!("Marcação M{id} atualizada.")).data(data))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    fn run(app: &mut FloorPlanApp, name: &str, args: Value) -> Value {
        let out = app.run_mcp_tool(name, &args);
        assert!(!out.is_error, "{}: {}", name, out.text);
        out.structured.unwrap_or(Value::Null)
    }

    #[test]
    fn mcp_marks_cover_crud_coordinates_revisions_filters_and_undo() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = Project::default();
        let wall = app
            .project
            .add_wall(to_model(0.0, 2.0), to_model(4.0, 2.0))
            .unwrap();
        let data = run(
            &mut app,
            "marcacoes",
            json!({"acao":"criar","tipo":"retangulo","cor":"azul","comentario":"Mover esta parede","pontos":[{"x":1,"y":1},{"x":3,"y":3}],"ids":[wall]}),
        );
        let id = data["id"].as_u64().unwrap();
        assert_eq!(app.project.markups[0].points[0], to_model(1.0, 1.0));
        assert_eq!(data["pontos"][0], json!({"x":1.0,"y":1.0}));
        assert_eq!(data["cor"], "#2670d8");
        assert!(
            data["elementos_relacionados"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["id"] == wall)
        );
        let before = app.project.clone();
        run(&mut app, "obter_marcacoes", json!({"cor":"azul"}));
        assert_eq!(before, app.project);
        let rev = run(&mut app, "mudancas_desde", json!({"revisao":0}));
        assert!(rev.to_string().contains("marcacoes_adicionadas"));
        run(
            &mut app,
            "marcacoes",
            json!({"acao":"editar","id":id,"comentario":"","cor":"#ff7733"}),
        );
        assert!(app.project.markups[0].comment.is_empty());
        run(
            &mut app,
            "marcacoes",
            json!({"acao":"mover","id":id,"dx":2,"dy":-1}),
        );
        assert_eq!(app.project.markups[0].points[0], to_model(3.0, 0.0));
        run(&mut app, "marcacoes", json!({"acao":"resolver","id":id}));
        assert!(!app.project.markup_shown(&app.project.markups[0]));
        assert_eq!(
            run(&mut app, "obter_marcacoes", json!({"resolvida":true}))["marcacoes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        run(&mut app, "marcacoes", json!({"acao":"reabrir","id":id}));
        run(&mut app, "marcacoes", json!({"acao":"selecionar","id":id}));
        assert_eq!(app.markup_state.selected, Some(id));
        run(&mut app, "marcacoes", json!({"acao":"excluir","id":id}));
        assert!(app.project.markups.is_empty());
        app.undo();
        assert_eq!(app.project.markups[0].id, id);
    }

    #[test]
    fn markups_reject_bad_input_and_batch_rolls_back() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = Project::default();
        let before = app.project.clone();
        for args in [
            json!({"acao":"criar","tipo":"seta","cor":"€abc","pontos":[{"x":0,"y":0},{"x":1,"y":1}]}),
            json!({"acao":"criar","tipo":"retangulo","comentario":3,"pontos":[{"x":0,"y":0},{"x":1,"y":1}]}),
            json!({"acao":"criar","tipo":"circulo","pontos":[{"x":0,"y":0},{"x":0,"y":0}]}),
        ] {
            let output = app.run_mcp_tool("marcacoes", &args);
            assert!(output.is_error);
            assert!(!output.text.contains("Erro interno"), "{}", output.text);
            assert_eq!(app.project, before);
        }
        let failed=app.run_mcp_tool("executar_lote",&json!({"acoes":[{"ferramenta":"marcacoes","argumentos":{"acao":"criar","tipo":"seta","pontos":[{"x":0,"y":0},{"x":1,"y":1}]}},{"ferramenta":"marcacoes","argumentos":{"acao":"excluir","id":999}}]}));
        assert!(failed.is_error);
        assert_eq!(app.project, before);
        assert!(crate::mcp::protocol::is_read_only("obter_marcacoes"));
        assert!(!crate::mcp::protocol::is_read_only("marcacoes"));
    }

    #[test]
    fn all_floor_queries_do_not_change_the_active_floor_or_geometry() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = Project::default();
        app.project.ensure_default_floor();
        let base = app.project.floors.active_floor_id.unwrap();
        run(
            &mut app,
            "marcacoes",
            json!({"acao":"criar","tipo":"seta","pontos":[{"x":0,"y":0},{"x":1,"y":1}]}),
        );
        let upper = app.project.add_floor("Superior", true, false);
        run(
            &mut app,
            "marcacoes",
            json!({"acao":"criar","tipo":"circulo","pontos":[{"x":0,"y":0},{"x":1,"y":1}]}),
        );
        let before = app.project.clone();
        let data = run(
            &mut app,
            "obter_marcacoes",
            json!({"todos_pavimentos":true}),
        );
        assert_eq!(data["marcacoes"].as_array().unwrap().len(), 2);
        assert_eq!(before, app.project);
        let data = run(&mut app, "obter_marcacoes", json!({"pavimento_id":base}));
        assert_eq!(data["marcacoes"][0]["pavimento_id"], base);
        assert_eq!(app.project.floors.active_floor_id, Some(upper));
        assert!(app.run_mcp_tool("marcacoes",&json!({"acao":"criar","pavimento_id":base,"tipo":"seta","pontos":[{"x":0,"y":0},{"x":1,"y":1}]})).is_error);
    }
}
