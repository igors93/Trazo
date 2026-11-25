//! Existing documentation tools also support editing their placed elements.
use super::super::project_docs::{parse_railing_side, parse_scale, parse_stair_shape};
use super::*;
use crate::domain::{circulation::CirculationKind, geometry::Point};

fn set_number(args: &Value, key: &str, value: &mut f64, min: f64, max: f64) -> Result<(), String> {
    if let Some(n) = number(args, key)? {
        if !(min..=max).contains(&n) {
            return Err(format!("{key} deve estar entre {min} e {max}."));
        }
        *value = n;
    }
    Ok(())
}
fn point(args: &Value, x: &str, y: &str) -> Result<Option<Point>, String> {
    match (number(args, x)?, number(args, y)?) {
        (Some(x), Some(y)) => Ok(Some(to_model(x, y))),
        (None, None) => Ok(None),
        _ => Err(format!("Informe {x} e {y} juntos.")),
    }
}

impl FloorPlanApp {
    pub(in crate::app::mcp_tools) fn tool_edit_circulation(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        self.apply_change(None, |p| {
            let c = p
                .circulations
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or("Elemento de circulação não encontrado.")?;
            if let Some(n) = text(args, "nome") {
                c.name = n;
            }
            let new_origin = point(args, "x", "y")?.or(point(args, "x1", "y1")?);
            if let Some(pos) = new_origin {
                let delta = pos.minus(c.origin);
                if let CirculationKind::Railing { end, .. }
                | CirculationKind::LevelChange { end, .. } = &mut c.kind
                {
                    *end = end.translated(delta);
                }
                c.origin = pos;
            }
            if let Some(n) = number(args, "rotacao_graus")? {
                c.rotation_deg = n;
            }
            if let Some(r) = text(args, "guarda_corpo") {
                c.railing = parse_railing_side(&r)
                    .ok_or("guarda_corpo: nenhum, esquerda, direita ou ambos.")?;
            }
            match &mut c.kind {
                CirculationKind::Stair {
                    shape,
                    width_m,
                    total_rise_m,
                    tread_m,
                    landing_depth_m,
                } => {
                    if let Some(raw) = text(args, "formato") {
                        *shape = parse_stair_shape(&raw).ok_or("formato: reta, em_l ou em_u.")?;
                    }
                    set_number(args, "largura_m", width_m, 0.6, 4.0)?;
                    set_number(args, "desnivel_m", total_rise_m, 0.3, 6.0)?;
                    set_number(args, "piso_m", tread_m, 0.22, 0.45)?;
                    set_number(args, "patamar_m", landing_depth_m, 0.3, 10.0)?;
                }
                CirculationKind::Ramp {
                    width_m,
                    length_m,
                    total_rise_m,
                    landing_depth_m,
                } => {
                    set_number(args, "largura_m", width_m, 0.8, 5.0)?;
                    set_number(args, "comprimento_m", length_m, 0.8, 30.0)?;
                    set_number(args, "desnivel_m", total_rise_m, 0.05, 4.0)?;
                    set_number(args, "patamar_m", landing_depth_m, 0.3, 10.0)?;
                }
                CirculationKind::Landing {
                    width_m,
                    depth_m,
                    elevation_m,
                } => {
                    set_number(args, "largura_m", width_m, 0.5, 10.0)?;
                    set_number(args, "profundidade_m", depth_m, 0.5, 10.0)?;
                    set_number(args, "nivel_m", elevation_m, -100.0, 100.0)?;
                }
                CirculationKind::Railing { end, height_m } => {
                    if let Some(pos) = point(args, "x2", "y2")? {
                        *end = pos;
                    }
                    set_number(args, "altura_m", height_m, 0.5, 2.0)?;
                    if c.origin.distance(*end) < 0.2 {
                        return Err("Guarda-corpo deve ter ao menos 20 cm.".into());
                    }
                }
                CirculationKind::LevelChange {
                    end,
                    from_elev_m,
                    to_elev_m,
                } => {
                    if let Some(pos) = point(args, "x2", "y2")? {
                        *end = pos;
                    }
                    set_number(args, "nivel_de_m", from_elev_m, -100.0, 100.0)?;
                    set_number(args, "nivel_para_m", to_elev_m, -100.0, 100.0)?;
                    if c.origin.distance(*end) < 0.2 {
                        return Err("Desnível deve ter ao menos 20 cm.".into());
                    }
                }
            }
            Ok(Change::new("Elemento de circulação atualizado.")
                .data(json!({"id":id,"origem":xy(c.origin),"resumo":c.summary()})))
        })
    }

    pub(in crate::app::mcp_tools) fn tool_edit_view(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        self.apply_change(None, |p| {
            let label = text(args, "rotulo");
            let target = if args.get("prancha_ref").is_some() {
                Some(text(args, "prancha_ref"))
            } else {
                None
            };
            if let Some(s) = p.views.sections.iter_mut().find(|s| s.id == id) {
                if let Some(v) = label {
                    s.label = v;
                }
                if let Some(v) = target {
                    s.target_sheet = v;
                }
                if args.get("olhar_esquerda").is_some() {
                    s.look_left = flag(args, "olhar_esquerda");
                }
                if let Some(v) = point(args, "x1", "y1")? {
                    s.start = v;
                }
                if let Some(v) = point(args, "x2", "y2")? {
                    s.end = v;
                }
                if s.start.distance(s.end) < 0.2 {
                    return Err("A linha de corte precisa ter pelo menos 20 cm.".into());
                }
            } else if let Some(e) = p.views.elevations.iter_mut().find(|e| e.id == id) {
                if let Some(v) = label {
                    e.label = v;
                }
                if let Some(v) = target {
                    e.target_sheet = v;
                }
                if let Some(v) = text(args, "lado") {
                    e.side = match v.as_str() {
                        "norte" => crate::domain::views::ElevationSide::North,
                        "sul" => crate::domain::views::ElevationSide::South,
                        "leste" => crate::domain::views::ElevationSide::East,
                        "oeste" => crate::domain::views::ElevationSide::West,
                        _ => return Err("lado: norte, sul, leste ou oeste.".into()),
                    };
                }
            } else if let Some(d) = p.views.details.iter_mut().find(|d| d.id == id) {
                if let Some(v) = label {
                    d.label = v;
                }
                if let Some(v) = target {
                    d.target_sheet = v;
                }
                if let Some(v) = point(args, "x", "y")? {
                    d.center = v;
                }
                set_number(args, "raio_m", &mut d.radius, 0.25, 10.0)?;
                if let Some(v) = text(args, "escala") {
                    d.scale = parse_scale(&v).ok_or("Escala inválida.")?;
                }
            } else {
                return Err("Vista técnica não encontrada.".into());
            }
            Ok(Change::new("Vista técnica atualizada.").data(json!({"id":id})))
        })
    }

    pub(in crate::app::mcp_tools) fn edit_floor(&mut self, args: &Value) -> Result<(), String> {
        let action = text(args, "acao").unwrap_or_default();
        self.apply_change(None, |p| {
            if action == "configurar" {
                if args.get("mostrar_inferior").is_some() {
                    p.floors.show_underlay = flag(args, "mostrar_inferior");
                }
                return Ok(Change::new("Referência do pavimento inferior configurada."));
            }
            let id = optional_count(args, "pavimento_id")?.ok_or("Informe pavimento_id.")?;
            if action == "remover" {
                p.remove_floor(id)?;
            } else {
                let floor = p
                    .floors
                    .levels
                    .iter()
                    .find(|f| f.id == id)
                    .ok_or("Pavimento não encontrado.")?
                    .clone();
                let mut elevation = floor.elevation_m;
                let mut height = floor.ceiling_height_m;
                let mut thickness = floor.slab_thickness_m;
                set_number(args, "nivel_m", &mut elevation, -1000.0, 1000.0)?;
                set_number(args, "pe_direito_m", &mut height, 2.0, 12.0)?;
                set_number(args, "espessura_laje_m", &mut thickness, 0.08, 0.8)?;
                p.update_floor_metadata(
                    id,
                    &text(args, "nome").unwrap_or(floor.name),
                    elevation,
                    height,
                    thickness,
                )?;
            }
            Ok(Change::new("Pavimentos atualizados."))
        })?;
        self.active_tab_floor_id = self.project.floors.active_floor_id;
        self.clear_selection();
        self.markup_state.selected = None;
        self.sync_inspector_fields();
        self.fit_view_requested();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Project;
    #[test]
    fn existing_tools_edit_floor_metadata_circulation_and_technical_views() {
        let mut app = FloorPlanApp::new(&eframe::egui::Context::default());
        app.project = Project::default();
        let ground = app.project.ensure_default_floor();
        let upper = app.project.add_floor("Superior", true, false);
        let out=app.run_mcp_tool("pavimentos",&json!({"acao":"editar","pavimento_id":upper,"nome":"Primeiro","nivel_m":3.2,"pe_direito_m":3.0}));
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(app.project.wall_height, 3.0);
        let stair = app
            .project
            .add_stair(
                Point::ORIGIN,
                crate::domain::circulation::StairShape::Straight,
                1.0,
                3.0,
                0.28,
            )
            .unwrap();
        let out=app.run_mcp_tool("circulacao",&json!({"acao":"editar","id":stair,"x":2,"y":3,"formato":"em_l","largura_m":1.2,"rotacao_graus":90}));
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(app.project.circulations[0].origin, to_model(2.0, 3.0));
        let before = app.project.clone();
        assert!(
            app.run_mcp_tool(
                "circulacao",
                &json!({"acao":"editar","id":stair,"largura_m":-1})
            )
            .is_error
        );
        assert_eq!(app.project, before);
        let section = app
            .project
            .add_section_line("AA", Point::ORIGIN, Point::new(4.0, 0.0), true, None)
            .unwrap();
        let out=app.run_mcp_tool("vistas_tecnicas",&json!({"acao":"editar","id":section,"rotulo":"BB","prancha_ref":"PR-02","olhar_esquerda":false}));
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(app.project.views.sections[0].label, "BB");
        assert!(!app.project.views.sections[0].look_left);
        let out = app.run_mcp_tool(
            "pavimentos",
            &json!({"acao":"remover","pavimento_id":upper}),
        );
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(app.project.floors.active_floor_id, Some(ground));
    }
}
