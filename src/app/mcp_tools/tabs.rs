//! Assistant tools for synchronized workspace tabs (`abas`), multi-storey
//! floors (`pavimentos`), and cross-discipline dependencies (`dependencias`).

use serde_json::{Value, json};

use super::{ToolOutput, flag, number, optional_count, text, xy};
use crate::app::{FloorPlanApp, tabs::TabPane};
use crate::domain::dependencies::ReviewSeverity;
use crate::domain::discipline::Discipline;

fn parse_discipline(args: &Value, key: &str) -> Result<Option<Discipline>, String> {
    match text(args, key) {
        None => Ok(None),
        Some(raw) => Discipline::from_key(&raw).map(Some).ok_or_else(|| {
            format!(
                "'{key}' deve ser tudo, arquitetura, eletrica, hidraulica, estrutura ou fundacoes."
            )
        }),
    }
}

impl FloorPlanApp {
    pub(super) fn tool_tabs(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".to_owned());
        let done = match action.as_str() {
            "listar" => None,
            "trocar" => {
                if let Some(tab) = number(args, "aba")? {
                    let count = self.tab_count();
                    if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                        return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                    }
                    self.switch_tab(tab as usize - 1);
                    Some("Aba trocada.".to_owned())
                } else if let Some(pid) = optional_count(args, "projeto_id")? {
                    let index = (0..self.tab_count())
                        .find(|&i| self.tab_project_id(i) == Some(pid))
                        .ok_or_else(|| {
                            format!("Projeto {pid} não encontrado entre as abas abertas.")
                        })?;
                    self.switch_tab(index);
                    Some(format!("Focado no projeto {pid} (aba {}).", index + 1))
                } else {
                    return Err("Informe 'aba' (1, 2...) ou 'projeto_id'.".to_owned());
                }
            }
            "nova" => {
                self.new_tab();
                Some("Nova aba com um projeto independente em branco.".to_owned())
            }
            "nova_visao" | "vincular" => {
                let source = if let Some(tab) = number(args, "aba")? {
                    let count = self.tab_count();
                    if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                        return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                    }
                    tab as usize - 1
                } else if let Some(pid) = optional_count(args, "projeto_id")? {
                    (0..self.tab_count())
                        .find(|&i| self.tab_project_id(i) == Some(pid))
                        .ok_or_else(|| {
                            format!("Projeto {pid} não encontrado entre as abas abertas.")
                        })?
                } else {
                    self.active_tab()
                };
                let discipline = parse_discipline(args, "disciplina")?.unwrap_or(Discipline::All);
                let floor_id = optional_count(args, "pavimento_id")?;
                if let Some(fid) = floor_id {
                    let pid = self
                        .tab_project_id(source)
                        .ok_or("Projeto de origem não encontrado.")?;
                    let valid = self
                        .project_by_id(pid)
                        .is_some_and(|p| p.floors.levels.iter().any(|l| l.id == fid));
                    if !valid {
                        return Err(format!("Pavimento {fid} não encontrado neste projeto."));
                    }
                }
                let title = text(args, "titulo");
                let created = self
                    .new_linked_tab_in_pane(source, discipline, floor_id, title, self.active_pane())
                    .ok_or("Não foi possível criar a visão vinculada.")?;
                if flag(args, "lado_a_lado") && source != created {
                    self.toggle_side_by_side(Some(source));
                }
                Some(format!(
                    "Nova visão vinculada ({}) aberta na aba {}, compartilhando o projeto {}.",
                    discipline.label(),
                    created + 1,
                    self.active_project_id()
                ))
            }
            "configurar_visao" => {
                let index = match number(args, "aba")? {
                    Some(tab) => {
                        let count = self.tab_count();
                        if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                            return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                        }
                        tab as usize - 1
                    }
                    None => self.active_tab(),
                };
                let discipline = parse_discipline(args, "disciplina")?;
                let floor_id = optional_count(args, "pavimento_id")?;
                let title = args.get("titulo").map(|v| {
                    v.as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                });
                if discipline.is_none() && floor_id.is_none() && title.is_none() {
                    return Err("Informe 'disciplina', 'pavimento_id' ou 'titulo'.".to_owned());
                }
                self.configure_view_tab(index, discipline, floor_id, title)?;
                Some(format!("Visão da aba {} atualizada.", index + 1))
            }
            "lado_a_lado" => match number(args, "aba")? {
                None | Some(0.0) => {
                    self.toggle_side_by_side(None);
                    Some("Vista lado a lado desativada.".to_owned())
                }
                Some(tab) => {
                    let count = self.tab_count();
                    if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                        return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                    }
                    let idx = tab as usize - 1;
                    if idx == self.active_tab() {
                        return Err(
                            "Escolha uma aba diferente da ativa para mostrar ao lado.".to_owned()
                        );
                    }
                    self.toggle_side_by_side(Some(idx));
                    Some(format!("Aba {} exibida lado a lado.", idx + 1))
                }
            },
            "fechar" => {
                let index = match number(args, "aba")? {
                    Some(tab) => {
                        let count = self.tab_count();
                        if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                            return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                        }
                        tab as usize - 1
                    }
                    None => self.active_tab(),
                };
                let pid = self.tab_project_id(index).ok_or("Aba não encontrada.")?;
                let is_last_link = self.project_tab_count(pid) <= 1;
                if is_last_link && self.project_dirty(pid) && !flag(args, "descartar_alteracoes") {
                    return Err("Esta é a última aba do projeto e há alterações não salvas. Salve antes ou informe descartar_alteracoes = true.".to_owned());
                }
                if index != self.active_tab() {
                    self.switch_tab(index);
                }
                self.close_active_tab();
                Some(if is_last_link {
                    "Aba fechada.".to_owned()
                } else {
                    "Visão fechada (o projeto permanece aberto nas outras abas vinculadas)."
                        .to_owned()
                })
            }
            "duplicar" => {
                self.duplicate_tab();
                Some(
                    "Cópia independente do projeto aberta numa nova aba, que ficou ativa."
                        .to_owned(),
                )
            }
            _ => {
                return Err(
                    "Informe 'acao': listar, trocar, nova, nova_visao, configurar_visao, lado_a_lado, fechar ou duplicar."
                        .to_owned(),
                );
            }
        };

        let infos = self.tab_infos();
        let projects: Vec<Value> = self
            .open_project_ids()
            .into_iter()
            .filter_map(|pid| {
                let project = self.project_by_id(pid)?;
                let linked_tabs: Vec<usize> = infos
                    .iter()
                    .enumerate()
                    .filter(|(_, info)| info.project_id == pid)
                    .map(|(i, _)| i + 1)
                    .collect();
                Some(json!({
                    "projeto_id": pid,
                    "nome": project.name,
                    "arquivo": self.project_file_path(pid).map(|p| p.display().to_string()),
                    "alteracoes_nao_salvas": self.project_dirty(pid),
                    "revisao": self.project_revision(pid),
                    "abas_vinculadas": linked_tabs,
                }))
            })
            .collect();
        let tabs: Vec<Value> = infos
            .iter()
            .enumerate()
            .map(|(index, info)| {
                json!({
                    "aba": index + 1,
                    "tab_id": info.tab_id,
                    "projeto_id": info.project_id,
                    "nome": info.title,
                    "disciplina": info.discipline.key(),
                    "pavimento_id": info.floor_id,
                    "pavimento": info.floor_name,
                    "ativa": info.active,
                    "visivel": info.shown,
                    "vinculada": info.linked,
                    "painel": match info.pane {
                        TabPane::Left => "esquerda",
                        TabPane::Right => "direita",
                    },
                    "alteracoes_nao_salvas": info.dirty,
                })
            })
            .collect();
        let active = self.active_tab() + 1;
        let active_pid = self.active_project_id();
        let summary = format!(
            "{} projeto(s), {} aba(s); ativa: aba {active} (projeto {active_pid}).",
            projects.len(),
            tabs.len()
        );
        let message = match done {
            Some(done) => format!("{done} {summary}"),
            None => summary,
        };
        Ok(ToolOutput::ok(message).with_structured(json!({
            "projeto_ativo": active_pid,
            "ativa": active,
            "projetos": projects,
            "abas": tabs,
        })))
    }

    pub(super) fn tool_floors(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".to_owned());
        let done = match action.as_str() {
            "listar" => None,
            "adicionar" => {
                let name = text(args, "nome").unwrap_or_default();
                let above = args.get("acima").and_then(Value::as_bool).unwrap_or(true);
                let copy_walls = args
                    .get("copiar_paredes")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let previous = self.project.clone();
                let id = self.project.add_floor(&name, above, copy_walls);
                self.active_tab_floor_id = Some(id);
                if !self.record_change(previous) {
                    return Err(self.message.clone());
                }
                self.clear_selection();
                self.sync_inspector_fields();
                let label = self
                    .project
                    .active_floor()
                    .map(|l| l.label_with_elevation())
                    .unwrap_or_else(|| format!("Pavimento {id}"));
                Some(format!(
                    "Pavimento adicionado e ativado: {label} (id {id})."
                ))
            }
            "trocar" => {
                self.project.ensure_default_floor();
                let target_id = if let Some(id) = optional_count(args, "pavimento_id")? {
                    id
                } else if let Some(name) = text(args, "nome") {
                    self.project
                        .floors
                        .levels
                        .iter()
                        .find(|l| l.name.eq_ignore_ascii_case(&name))
                        .map(|l| l.id)
                        .ok_or_else(|| format!("Nenhum pavimento chamado '{name}'."))?
                } else {
                    return Err("Informe 'pavimento_id' ou 'nome'.".to_owned());
                };
                self.project.switch_floor(target_id)?;
                self.active_tab_floor_id = self.project.floors.active_floor_id;
                self.clear_selection();
                self.sync_inspector_fields();
                let label = self
                    .project
                    .active_floor()
                    .map(|l| l.label_with_elevation())
                    .unwrap_or_else(|| format!("Pavimento {target_id}"));
                Some(format!("Pavimento ativo nesta aba: {label}."))
            }
            _ => return Err("Informe 'acao': listar, adicionar ou trocar.".to_owned()),
        };

        let active_id = self.project.floors.active_floor_id;
        let floors: Vec<Value> = if self.project.floors.levels.is_empty() {
            vec![json!({
                "id": 1,
                "nome": "Térreo",
                "nivel_m": 0.0,
                "pe_direito_m": self.project.wall_height,
                "espessura_laje_m": 0.15,
                "ativo": true,
                "paredes": self.project.walls.len(),
                "comodos": self.project.rooms.len(),
            })]
        } else {
            self.project
                .floors
                .levels
                .iter()
                .map(|l| {
                    let is_active = Some(l.id) == active_id;
                    let (walls, rooms) = if is_active {
                        (self.project.walls.len(), self.project.rooms.len())
                    } else {
                        (l.data.walls.len(), l.data.rooms.len())
                    };
                    json!({
                        "id": l.id,
                        "nome": l.name,
                        "nivel_m": l.elevation_m,
                        "pe_direito_m": if is_active { self.project.wall_height } else { l.ceiling_height_m },
                        "espessura_laje_m": l.slab_thickness_m,
                        "ativo": is_active,
                        "paredes": walls,
                        "comodos": rooms,
                    })
                })
                .collect()
        };
        let summary = format!("{} pavimento(s) no projeto.", floors.len());
        let message = match done {
            Some(done) => format!("{done} {summary}"),
            None => summary,
        };
        Ok(ToolOutput::ok(message).with_structured(json!({
            "projeto_id": self.active_project_id(),
            "aba_ativa": self.active_tab() + 1,
            "pavimento_ativo_id": active_id.unwrap_or(1),
            "pavimentos": floors,
        })))
    }

    pub(super) fn tool_dependencies(&self, args: &Value) -> Result<ToolOutput, String> {
        let filter = parse_discipline(args, "disciplina")?;
        let audit = self.project.cached_dependency_audit();
        let links: Vec<Value> = audit
            .links
            .iter()
            .filter(|l| filter.is_none_or(|d| d.is_all() || l.target_discipline == d))
            .map(|l| {
                json!({
                    "origem_tipo": l.source_kind,
                    "origem_id": l.source_id,
                    "destino_tipo": l.target_kind,
                    "destino_id": l.target_id,
                    "disciplina": l.target_discipline.key(),
                    "relacao": l.relation,
                })
            })
            .collect();
        let reviews: Vec<Value> = audit
            .review_items
            .iter()
            .filter(|r| filter.is_none_or(|d| d.is_all() || r.discipline == d))
            .map(|r| {
                json!({
                    "disciplina": r.discipline.key(),
                    "severidade": r.severity.key(),
                    "tipo": r.kind.key(),
                    "elemento_id": r.element_id,
                    "relacionados": r.related_ids,
                    "posicao": r.position.map(xy),
                    "mensagem": r.message,
                    "sugestao": r.suggestion,
                })
            })
            .collect();
        let critical = audit
            .review_items
            .iter()
            .filter(|r| {
                r.severity == ReviewSeverity::Critical
                    && filter.is_none_or(|d| d.is_all() || r.discipline == d)
            })
            .count();
        let warnings = audit
            .review_items
            .iter()
            .filter(|r| {
                r.severity == ReviewSeverity::Warning
                    && filter.is_none_or(|d| d.is_all() || r.discipline == d)
            })
            .count();

        let last_auto: Vec<Value> = self
            .sync_report
            .auto_updated
            .iter()
            .map(|u| {
                json!({
                    "disciplina": u.discipline.key(),
                    "elemento_id": u.element_id,
                    "origem_id": u.source_id,
                    "descricao": u.description,
                })
            })
            .collect();
        let last_blocked: Vec<Value> = self
            .sync_report
            .blocked_updates
            .iter()
            .map(|r| {
                json!({
                    "disciplina": r.discipline.key(),
                    "severidade": r.severity.key(),
                    "tipo": r.kind.key(),
                    "elemento_id": r.element_id,
                    "mensagem": r.message,
                    "sugestao": r.suggestion,
                })
            })
            .collect();
        let recalc: Vec<&str> = self
            .sync_report
            .recalculated_disciplines
            .iter()
            .map(|d| d.key())
            .collect();

        let msg = format!(
            "{} vínculo(s) entre disciplinas, {} item(ns) para revisão manual ({critical} crítico(s), {warnings} aviso(s)).",
            links.len(),
            reviews.len()
        );
        Ok(ToolOutput::ok(msg).with_structured(json!({
            "projeto_id": self.active_project_id(),
            "revisao": self.plan_log.revision,
            "aba_ativa": self.active_tab() + 1,
            "disciplina_ativa": self.discipline.key(),
            "vinculos": links,
            "revisao_manual": reviews,
            "total_revisao_manual": reviews.len(),
            "criticos": critical,
            "avisos": warnings,
            "ultima_sincronizacao": {
                "vazia": self.sync_report.is_empty(),
                "atualizados_automaticamente": last_auto,
                "bloqueados": last_blocked,
                "disciplinas_recalculadas": recalc,
            },
        })))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn an_assistant_works_with_several_projects_in_tabs() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.run_mcp_tool(
            "criar_parede",
            &json!({ "x1": 0, "y1": 0, "x2": 4, "y2": 0 }),
        );
        let duplicated = app.run_mcp_tool("abas", &json!({ "acao": "duplicar" }));
        assert!(!duplicated.is_error, "{}", duplicated.text);
        assert_eq!(duplicated.structured.unwrap()["ativa"], 2);
        // The other tools work on the active tab.
        app.run_mcp_tool(
            "criar_parede",
            &json!({ "x1": 0, "y1": 3, "x2": 4, "y2": 3 }),
        );
        assert_eq!(app.project.walls.len(), 2);
        let back = app.run_mcp_tool("abas", &json!({ "acao": "trocar", "aba": 1 }));
        assert!(!back.is_error);
        assert_eq!(app.project.walls.len(), 1);
        let listed = app.run_mcp_tool("abas", &json!({}));
        let data = listed.structured.unwrap();
        assert_eq!(data["abas"].as_array().unwrap().len(), 2);
        assert_eq!(data["projetos"].as_array().unwrap().len(), 2);
        assert_eq!(data["abas"][0]["ativa"], true);
        assert!(
            app.run_mcp_tool("abas", &json!({ "acao": "trocar", "aba": 9 }))
                .is_error
        );
        // Tabs cannot be switched inside a batch, which undoes as one step.
        assert!(
            app.run_mcp_tool(
                "executar_lote",
                &json!({ "acoes": [{ "ferramenta": "abas", "argumentos": { "acao": "nova" } }] }),
            )
            .is_error
        );
    }

    #[test]
    fn an_assistant_creates_linked_discipline_views_and_inspects_dependencies() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 15 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 3 } },
            ] }),
        );
        let linked = app.run_mcp_tool(
            "abas",
            &json!({ "acao": "nova_visao", "disciplina": "eletrica", "lado_a_lado": true }),
        );
        assert!(!linked.is_error, "{}", linked.text);
        let data = linked.structured.unwrap();
        assert_eq!(data["projetos"].as_array().unwrap().len(), 1);
        assert_eq!(data["abas"].as_array().unwrap().len(), 2);
        assert_eq!(data["abas"][0]["projeto_id"], data["abas"][1]["projeto_id"]);
        assert_eq!(data["abas"][1]["disciplina"], "eletrica");
        assert_eq!(data["abas"][1]["vinculada"], true);

        // Place an outlet in Electrical tab, then switch to Architecture tab and verify it exists.
        let outlet = app.run_mcp_tool(
            "inserir_ponto_eletrico",
            &json!({ "tipo": "tomada_media", "x": 2.5, "y": 1.2 }),
        );
        assert!(!outlet.is_error, "{}", outlet.text);
        let deps = app
            .run_mcp_tool("dependencias", &json!({}))
            .structured
            .unwrap();
        assert!(!deps["vinculos"].as_array().unwrap().is_empty());

        // Switching back to tab 1 sees the same shared project and its outlet.
        app.run_mcp_tool("abas", &json!({ "acao": "trocar", "aba": 1 }));
        assert_eq!(app.project.electrical_devices().count(), 1);
    }
}
