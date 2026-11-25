//! File-backed tracing references and visual project comparison.
use super::*;

impl FloorPlanApp {
    pub(in crate::app::mcp_tools) fn references_summary(&self) -> Value {
        json!({"fundos":self.project.underlays.iter().map(|u|json!({"id":u.id,"nome":u.name,"caminho":u.source_path,"pagina":u.page,"origem":xy(u.origin),"largura":u.width_m,"altura":u.height_m(),"angulo_graus":-u.rotation_deg,"opacidade":u.opacity,"visivel":u.visible,"bloqueada":u.locked,"imagem_incorporada":u.embedded_png_base64.is_some()})).collect::<Vec<_>>(),"externas":self.project.xrefs.iter().map(|x|json!({"id":x.id,"nome":x.name,"caminho":x.path,"deslocamento":xy(x.offset),"opacidade":x.opacity,"visivel":x.visible,"bloqueada":x.locked,"cor":format!("#{:02x}{:02x}{:02x}",x.tint[0],x.tint[1],x.tint[2]),"erro":self.xref_manager.get(x.id).and_then(|v|v.error.as_deref())})).collect::<Vec<_>>()})
    }
    pub(in crate::app::mcp_tools) fn tool_references(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".into());
        if action == "listar" {
            return Ok(ToolOutput::ok("Imagens de fundo e referências externas.")
                .with_structured(self.references_summary()));
        }
        if action == "comparar" {
            let (baseline, label) = if let Some(tab) = optional_count(args, "aba")? {
                let index = tab.checked_sub(1).ok_or("aba deve ser maior que zero.")? as usize;
                (
                    self.tab_project(index)
                        .ok_or("Aba não encontrada.")?
                        .clone(),
                    format!("Aba {tab}"),
                )
            } else {
                let path = resolve_path(
                    &text(args, "caminho").ok_or("Informe caminho ou aba.")?,
                    "trazo",
                );
                (
                    crate::io::xref::XrefManager::load_external_project(&path)?,
                    path.display().to_string(),
                )
            };
            let diff = crate::domain::compare::compare_projects(&baseline, &self.project, &label);
            let counts = diff.counts();
            let data = json!({"referencia":label,"adicionados":counts.added,"removidos":counts.removed,"alterados":counts.modified,"area_antes_m2":diff.total_area_before,"area_depois_m2":diff.total_area_after,"paredes_adicionadas":diff.walls_added.iter().map(|w|w.id).collect::<Vec<_>>(),"paredes_removidas":diff.walls_removed.iter().map(|w|w.id).collect::<Vec<_>>(),"paredes_alteradas":diff.walls_modified.iter().map(|(_,w)|w.id).collect::<Vec<_>>(),"marcacoes_alteradas":baseline.markups!=self.project.markups});
            self.compare_state.baseline = Some(baseline);
            self.compare_state.baseline_label = label;
            self.compare_state.diff = Some(diff);
            self.compare_state.last_revision = self.plan_log.revision;
            self.compare_state.overlay_active = flag(args, "exibir");
            return Ok(ToolOutput::ok("Comparação calculada.").with_structured(data));
        }
        if action == "limpar_comparacao" {
            self.compare_state = Default::default();
            return Ok(ToolOutput::ok("Comparação encerrada."));
        }
        if action == "exportar_comparacao" {
            let baseline = self
                .compare_state
                .baseline
                .as_ref()
                .ok_or("Execute comparar primeiro.")?;
            let diff = crate::domain::compare::compare_projects(
                baseline,
                &self.project,
                &self.compare_state.baseline_label,
            );
            let path = resolve_path(&text(args, "caminho").ok_or("Informe caminho.")?, "svg");
            if path.exists() && !flag(args, "sobrescrever") {
                return Err("Arquivo já existe; use sobrescrever: true.".into());
            }
            super::super::ensure_parent(&path)?;
            std::fs::write(
                &path,
                crate::domain::compare::export_diff_svg(baseline, &self.project, &diff),
            )
            .map_err(|e| e.to_string())?;
            return Ok(ToolOutput::ok("Comparação exportada.")
                .with_structured(json!({"caminho":path.display().to_string()})));
        }
        let typ = text(args, "tipo").unwrap_or_else(|| "fundo".into());
        if !["fundo", "externa"].contains(&typ.as_str()) {
            return Err("tipo: fundo ou externa.".into());
        }
        let x = number(args, "x")?;
        let y = number(args, "y")?;
        if x.is_some() != y.is_some() {
            return Err("Informe x e y juntos.".into());
        }
        let at = to_model(x.unwrap_or(0.0), y.unwrap_or(0.0));
        if action == "adicionar" {
            let path = resolve_path(
                &text(args, "caminho").ok_or("Informe caminho.")?,
                if typ == "fundo" { "png" } else { "trazo" },
            );
            let name = text(args, "nome").unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            if typ == "externa" {
                crate::io::xref::XrefManager::load_external_project(&path)?;
                return self.apply_change(None, |p| {
                    let id = p.add_xref(name, path.display().to_string(), at);
                    Ok(Change::new("Referência externa vinculada.")
                        .data(json!({"id":id,"tipo":"externa"})))
                });
            }
            let page = optional_count(args, "pagina")?.unwrap_or(1);
            if !(1..=u32::MAX as u64).contains(&page) {
                return Err("Página inválida.".into());
            }
            let decoded = crate::io::underlay::load_underlay_file(&path, page as u32)?;
            let width = positive(args, "largura", 15.0)?;
            return self.apply_change(None, |p| {
                let id = p.add_underlay(
                    name,
                    path.display().to_string(),
                    page as u32,
                    decoded.width,
                    decoded.height,
                    at,
                    width,
                    Some(decoded.png_base64),
                );
                Ok(
                    Change::new("Imagem de fundo adicionada.")
                        .data(json!({"id":id,"tipo":"fundo"})),
                )
            });
        }
        let id = element_id(args, "id")?;
        if action == "recarregar" && typ == "externa" {
            let xref = self
                .project
                .xrefs
                .iter()
                .find(|x| x.id == id)
                .ok_or("Referência não encontrada.")?
                .clone();
            self.xref_manager.reload_one(&xref);
            if let Some(error) = self.xref_manager.get(id).and_then(|s| s.error.as_ref()) {
                return Err(error.clone());
            }
            return Ok(ToolOutput::ok("Referência recarregada."));
        }
        self.apply_change(None, |p| {
            if typ == "fundo" {
                if action == "remover" {
                    p.remove_underlay(id)?;
                    return Ok(Change::new("Imagem de fundo removida."));
                }
                if action == "calibrar" {
                    if p.underlays
                        .iter()
                        .find(|u| u.id == id)
                        .is_some_and(|u| u.locked)
                    {
                        return Err("Desbloqueie a imagem antes de calibrar.".into());
                    }
                    let a = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                    let b = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                    let factor =
                        p.calibrate_underlay(id, a, b, required_number(args, "distancia_real")?)?;
                    let width = p
                        .underlays
                        .iter()
                        .find(|u| u.id == id)
                        .ok_or("Imagem não encontrada.")?
                        .width_m;
                    return Ok(Change::new("Escala da imagem calibrada.")
                        .data(json!({"largura":width,"fator":factor})));
                }
                if action != "editar" {
                    return Err("Ação de referência inválida.".into());
                }
                let u = p
                    .underlays
                    .iter_mut()
                    .find(|u| u.id == id)
                    .ok_or("Imagem não encontrada.")?;
                // An unlocked image may be edited and locked in one transaction.
                let geometry_locked =
                    u.locked && (args.get("bloqueada").is_none() || flag(args, "bloqueada"));
                if args.get("bloqueada").is_some() {
                    u.locked = flag(args, "bloqueada");
                }
                if geometry_locked
                    && (x.is_some()
                        || args.get("largura").is_some()
                        || args.get("angulo_graus").is_some())
                {
                    return Err("Desbloqueie a imagem antes de alterar sua geometria.".into());
                }
                if x.is_some() {
                    u.origin = at;
                }
                if let Some(w) = number(args, "largura")? {
                    if w <= 0.0 {
                        return Err("largura deve ser positiva.".into());
                    }
                    u.width_m = w;
                }
                if let Some(a) = number(args, "angulo_graus")? {
                    u.rotation_deg = -a;
                }
                if let Some(name) = text(args, "nome") {
                    u.name = name;
                }
                if let Some(o) = number(args, "opacidade")? {
                    if !(0.0..=1.0).contains(&o) {
                        return Err("opacidade deve estar entre 0 e 1.".into());
                    }
                    u.opacity = o as f32;
                }
                if args.get("visivel").is_some() {
                    u.visible = flag(args, "visivel");
                }
            } else {
                if action == "remover" {
                    if !p.remove_xref(id) {
                        return Err("Referência não encontrada.".into());
                    }
                    return Ok(Change::new("Referência externa desvinculada."));
                }
                if action != "editar" {
                    return Err("Ação de referência externa inválida.".into());
                }
                let u = p
                    .xrefs
                    .iter_mut()
                    .find(|u| u.id == id)
                    .ok_or("Referência não encontrada.")?;
                let geometry_locked =
                    u.locked && (args.get("bloqueada").is_none() || flag(args, "bloqueada"));
                if args.get("bloqueada").is_some() {
                    u.locked = flag(args, "bloqueada");
                }
                if x.is_some() {
                    if geometry_locked {
                        return Err("Desbloqueie a referência para mover.".into());
                    }
                    u.offset = at;
                }
                if let Some(name) = text(args, "nome") {
                    u.name = name;
                }
                if let Some(c) = text(args, "cor") {
                    u.tint = super::super::markups::color(&c)?;
                }
                if let Some(o) = number(args, "opacidade")? {
                    if !(0.0..=1.0).contains(&o) {
                        return Err("opacidade deve estar entre 0 e 1.".into());
                    }
                    u.opacity = o as f32;
                }
                if args.get("visivel").is_some() {
                    u.visible = flag(args, "visivel");
                }
            }
            Ok(Change::new("Referência atualizada."))
        })
    }
}
