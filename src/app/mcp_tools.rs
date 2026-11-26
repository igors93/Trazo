//! The tools an AI assistant runs through MCP (see `crate::mcp`). They act
//! on the open project the way the interface does: each change is one undo
//! step, marks the project unsaved and feeds the recovery draft.
//!
//! The assistant sees plan coordinates in metres with X to the right and Y
//! up, as the status bar shows them; the model's Y grows downwards on
//! screen, so Y is flipped at this boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

use super::{FloorPlanApp, OpenResult, Selection};

mod args;
mod batch;
mod codes;
mod drawings;
mod editing;
mod electrical;
mod exporting;
#[cfg(test)]
mod gate_tests;
mod markups;
mod openings_fixtures;
mod paths;
mod project_docs;
mod protection;
mod resources;
mod rooms_walls;
mod shapes;
mod structure;
mod summary;
mod tabs;
mod terrain;
#[cfg(test)]
mod tests;

use crate::{
    domain::{geometry::format_meters, model::Project},
    io::export,
    mcp::{ToolOutput, protocol},
};
pub(super) use args::optional_count;
use args::{
    element_id, flag, from_model, number, quarter_turns, required_number, round, square_metres,
    text, to_model, xy,
};
use paths::{documents_dir, ensure_parent, file_stem, resolve_path, unique_path};

/// What a successful change reports.
struct Change {
    message: String,
    selection: Option<Selection>,
    data: Option<Value>,
}

impl Change {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            selection: None,
            data: None,
        }
    }

    fn select(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }

    fn data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

impl FloorPlanApp {
    /// Runs one tool for the assistant. A panic inside a tool leaves the
    /// plan as it was instead of taking the app down.
    pub(super) fn run_mcp_tool(&mut self, name: &str, args: &Value) -> ToolOutput {
        self.sync_markup_context();
        self.commit_item_drag();
        let before = self.project.clone();
        let before_selection = self.selected();
        // Changes made now are the assistant's, in the plan's log.
        self.mcp_running = Some(name.to_owned());
        let outcome = catch_unwind(AssertUnwindSafe(|| self.dispatch_tool(name, args)));
        let output = match outcome {
            Ok(Ok(output)) => output,
            Ok(Err(message)) => ToolOutput::error(message),
            Err(_) => {
                // Putting the plan back is a change too, for whoever follows it.
                let broken = std::mem::replace(&mut self.project, before.clone());
                self.batch_depth = 0;
                self.note_plan_change(&broken, false);
                self.set_selection_set(before_selection);
                ToolOutput::error(
                    "Erro interno do Trazo ao executar a ferramenta; a planta foi mantida como estava.",
                )
            }
        };
        let output = self.report_protection(&before, output);
        self.revalidate_selection();
        self.sync_inspector_fields();
        self.mcp_running = None;
        self.mcp_actions += 1;
        // Shown on the plan: the highlight, the badge and the log.
        self.note_ai_action(name, &output, &before);
        self.message = if output.is_error {
            format!("IA · {}", output.text)
        } else if protocol::is_read_only(name) {
            "IA consultou a planta.".to_owned()
        } else {
            format!("IA · {}", output.text.lines().next().unwrap_or_default())
        };
        output
    }

    fn dispatch_tool(&mut self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        // Optional safety guard: when an assistant passes `projeto_id` on a
        // project-scoped tool, refuse if it does not match the active project
        // so edits never land in another project by mistake.
        if !matches!(name, "abas" | "novo_projeto" | "abrir_projeto")
            && let Some(target_pid) = optional_count(args, "projeto_id")?
            && target_pid != self.active_project_id()
        {
            return Err(format!(
                "O projeto ativo é {} (aba {}), mas a ferramenta pediu projeto_id={target_pid}. Troque primeiro com abas(acao='trocar', projeto_id={target_pid}).",
                self.active_project_id(),
                self.active_tab() + 1
            ));
        }
        match name {
            "obter_planta" => {
                let summary = self.plan_summary();
                let text = serde_json::to_string_pretty(&summary).unwrap_or_default();
                Ok(ToolOutput::ok(text).with_structured(summary))
            }
            "imagem_da_planta" => {
                let side = number(args, "tamanho_max")?
                    .unwrap_or(1024.0)
                    .clamp(256.0, 2048.0);
                let discipline = match text(args, "disciplina") {
                    Some(raw) => crate::domain::discipline::Discipline::from_key(&raw).ok_or(
                        "'disciplina' deve ser tudo, arquitetura, eletrica, hidraulica, estrutura ou fundacoes.",
                    )?,
                    None => self.discipline,
                };
                let view = if discipline == self.discipline {
                    self.active_export_view()
                } else {
                    export::ExportView::for_discipline(&self.project, discipline)
                };
                let mut preview_project = self.project.clone();
                preview_project.markup_settings.export = args
                    .get("incluir_marcacoes")
                    .map(|_| flag(args, "incluir_marcacoes"))
                    .unwrap_or(true);
                let png = export::png_preview_for_view(&preview_project, &view, side as u32)?;
                Ok(ToolOutput::ok(format!(
                    "Imagem da planta \"{}\" (vista {}) com {} cômodos, {} paredes e {} aberturas.",
                    self.project.name,
                    discipline.label().to_lowercase(),
                    self.project.rooms.len(),
                    self.project.walls.len(),
                    self.project.openings.len()
                ))
                .with_image(png))
            }
            "definir_terreno" => self.tool_terrain(args),
            "criar_comodo" => self.tool_create_room(args),
            "criar_parede" => self.tool_create_wall(args),
            "inserir_abertura" => self.tool_insert_opening(args),
            "editar_comodo" => self.tool_edit_room(args),
            "editar_parede" => {
                let id = element_id(args, "id")?;
                let length = number(args, "comprimento")?;
                let height = number(args, "altura_m")?;
                let height_end = number(args, "altura_fim_m")?;
                let storey_height = args
                    .get("altura_padrao")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if length.is_none() && height.is_none() && height_end.is_none() && !storey_height {
                    return Err(
                        "Informe 'comprimento', 'altura_m' e/ou 'altura_fim_m', ou 'altura_padrao': true."
                            .to_owned(),
                    );
                }
                self.require(
                    id,
                    |selection| matches!(selection, Selection::Wall(_)),
                    "parede",
                )?;
                self.apply_change(None, |project| {
                    let mut said = Vec::new();
                    if let Some(length) = length {
                        project.resize_wall_length(id, length)?;
                        said.push(format!("tem {}", format_meters(length)));
                    }
                    if storey_height {
                        project.set_wall_height(id, None, None)?;
                        said.push("segue o pé-direito".to_owned());
                    } else if height.is_some() || height_end.is_some() {
                        // Only the end given: the start keeps its height.
                        let start = height.or_else(|| {
                            project
                                .walls
                                .iter()
                                .find(|w| w.id == id)
                                .map(|w| w.heights(project.wall_height).0)
                        });
                        project.set_wall_height(id, start, height_end)?;
                    }
                    let storey = project.wall_height;
                    let (start, end) = project
                        .walls
                        .iter()
                        .find(|w| w.id == id)
                        .map_or((storey, storey), |w| w.heights(storey));
                    if height.is_some() || height_end.is_some() {
                        said.push(if (start - end).abs() < 0.005 {
                            format!("tem {} de altura", format_meters(start))
                        } else {
                            format!(
                                "tem topo inclinado de {} a {}",
                                format_meters(start),
                                format_meters(end)
                            )
                        });
                    }
                    Ok(
                        Change::new(format!("Parede {id} agora {}.", said.join(" e ")))
                            .select(Selection::Wall(id))
                            .data(json!({
                                "altura_inicio_m": start,
                                "altura_fim_m": end,
                            })),
                    )
                })
            }
            "editar_abertura" => self.tool_edit_opening(args),
            "inserir_objeto" => self.tool_insert_fixture(args),
            "inserir_ponto_eletrico" => self.tool_insert_electrical(args),
            "editar_ponto_eletrico" => self.tool_edit_electrical(args),
            "projeto_eletrico" => self.tool_electrical_design(args),
            "quadro_de_cargas" => self.tool_load_schedule(),
            "projeto_estrutural" => self.tool_structural_design(args),
            "pre_dimensionamento" => self.tool_presizing(),
            "abas" => self.tool_tabs(args),
            "pavimentos" => self.tool_floors(args),
            "dependencias" => self.tool_dependencies(args),
            "quantitativos" => self.tool_quantities(args),
            "ergonomia" => self.tool_ergonomics(args),
            "camadas" => self.tool_layers(args),
            "anotacoes" => self.tool_notes(args),
            "obter_marcacoes" => self.tool_read_markups(args),
            "marcacoes" => self.tool_markups(args),
            "biblioteca" => self.tool_library(args),
            "blocos" => self.tool_blocks(args),
            "referencias" => self.tool_references(args),
            "documentacao" => self.tool_documentation(args),
            "modelos" => self.tool_templates(args),
            "acabamentos" => self.tool_finishes(args),
            "terreno" => self.tool_site(args),
            "cotas" => self.tool_dimensions(args),
            "circulacao" => self.tool_circulation(args),
            "codigos_normativos" => self.tool_building_codes(args),
            "local_clima" => self.tool_site_climate(args),
            "verificar_normas" => self.tool_check_codes(args),
            "vistas_tecnicas" => self.tool_views(args),
            "prancha" => self.tool_sheet(args),
            "desenhar" => self.tool_draw(args),
            "definir_alvenaria" => self.tool_masonry(args),
            "editar_objeto" => self.tool_edit_fixture(args),
            "mover_elemento" => self.tool_move(args),
            "selecionar" => self.tool_select(args),
            "executar_lote" => self.tool_batch(args),
            "mudancas_desde" => {
                let data = self.plan_changes_since(args)?;
                let count = data["mudancas"].as_array().map_or(0, Vec::len);
                Ok(ToolOutput::ok(format!(
                    "Revisão atual {}: {count} mudança(s) depois da revisão {} ({} do usuário).",
                    data["revisao_atual"], data["desde"], data["mudancas_do_usuario"]
                ))
                .with_structured(data))
            }
            "transformar" => self.tool_transform(args),
            "alinhar" => self.tool_arrange(args, false),
            "distribuir" => self.tool_arrange(args, true),
            "agrupar" => self.tool_group(args, false),
            "desagrupar" => self.tool_group(args, true),
            "repetir" => self.tool_repeat(args),
            "editar_geometria" => self.tool_geometry(args),
            "unir" => self.tool_join(args),
            "restringir" => self.tool_constrain(args),
            "remover_restricao" => self.tool_remove_constraint(args),
            "excluir_elemento" => {
                let id = element_id(args, "id")?;
                let selection = self.find_element(id)?;
                let label = Self::element_label(selection);
                self.apply_change(Some("Nada foi excluído."), |project| {
                    match selection {
                        Selection::Wall(id) => project.delete_wall(id),
                        Selection::Room(id) => {
                            project.delete_room(id);
                        }
                        Selection::Opening(id) => {
                            project.delete_opening(id);
                        }
                        Selection::Terrain(id) => {
                            project.delete_terrain(id);
                        }
                        Selection::Fixture(id) => {
                            project.delete_fixture(id);
                        }
                        Selection::Drawing(id) => {
                            project.delete_drawing(id);
                        }
                    }
                    Ok(Change::new(format!("{label} {id} excluído(a).")))
                })
            }
            "desfazer" | "refazer" => {
                let steps = number(args, "passos")?.unwrap_or(1.0).clamp(1.0, 50.0) as usize;
                let undo = name == "desfazer";
                let mut done = 0;
                for _ in 0..steps {
                    let possible = if undo {
                        self.history.can_undo()
                    } else {
                        self.history.can_redo()
                    };
                    if !possible {
                        break;
                    }
                    if undo {
                        self.undo();
                    } else {
                        self.redo();
                    }
                    done += 1;
                }
                if done == 0 {
                    return Err(if undo {
                        "Não há ações para desfazer.".to_owned()
                    } else {
                        "Não há ações para refazer.".to_owned()
                    });
                }
                Ok(ToolOutput::ok(format!(
                    "{} {done} ação(ões).",
                    if undo { "Desfeita(s)" } else { "Refeita(s)" }
                )))
            }
            "centralizar_planta" => {
                if self.project.terrain.is_none() {
                    return Err("Não há terreno: crie um com definir_terreno.".to_owned());
                }
                if !self.project.content_fits_terrain() {
                    return Err(
                        "A planta não cabe no terreno; aumente o terreno primeiro.".to_owned()
                    );
                }
                let output =
                    self.apply_change(Some("A planta já está centralizada."), |project| {
                        project.center_content_on_terrain();
                        Ok(Change::new("Planta centralizada no terreno."))
                    })?;
                self.fit_view_requested();
                Ok(output)
            }
            "novo_projeto" => {
                self.check_discard(args)?;
                self.new_project();
                if let Some(name) = text(args, "nome")
                    && self.project.name != name
                {
                    self.project.name = name;
                    self.mark_edited_outside_history();
                }
                self.fit_view_requested();
                Ok(ToolOutput::ok(format!(
                    "Nova planta \"{}\" criada.",
                    self.project.name
                )))
            }
            "abrir_projeto" => {
                let input =
                    text(args, "caminho").ok_or("Informe 'caminho' do arquivo .trazo ou .json.")?;
                let path = resolve_path(&input, "trazo");
                if !path.is_file() {
                    return Err(format!("Arquivo não encontrado: {}", path.display()));
                }
                let already_open = self.find_open_project_by_path(&path).is_some();
                if !already_open && self.active_is_blank() {
                    self.check_discard(args)?;
                }
                match self.open_in_tab(&path) {
                    OpenResult::Success => {
                        let pid = self.active_project_id();
                        let linked_tabs: Vec<usize> = (0..self.tab_count())
                            .filter(|&i| self.tab_project_id(i) == Some(pid))
                            .map(|i| i + 1)
                            .collect();
                        let msg = if already_open {
                            format!(
                                "Projeto \"{}\" já estava aberto (projeto {pid}, aba {}).",
                                self.project.name,
                                self.active_tab() + 1
                            )
                        } else {
                            format!(
                                "Projeto \"{}\" aberto de {} (projeto {pid}, {} aba(s) vinculada(s)).",
                                self.project.name,
                                path.display(),
                                linked_tabs.len()
                            )
                        };
                        Ok(ToolOutput::ok(msg).with_structured(json!({
                            "projeto_id": pid,
                            "aba_ativa": self.active_tab() + 1,
                            "abas_vinculadas": linked_tabs,
                            "ja_estava_aberto": already_open,
                            "caminho": path.display().to_string(),
                        })))
                    }
                    _ => Err(self.message.clone()),
                }
            }
            "salvar_projeto" => {
                let path = match text(args, "caminho") {
                    Some(input) => resolve_path(&input, "trazo"),
                    None => match &self.file_path {
                        Some(path) => path.clone(),
                        None => unique_path(
                            documents_dir()
                                .join(format!("{}.trazo", file_stem(&self.project.name))),
                        ),
                    },
                };
                ensure_parent(&path)?;
                if self.write_project(path.clone()) {
                    Ok(
                        ToolOutput::ok(format!("Projeto salvo em {}", path.display()))
                            .with_structured(json!({
                                "projeto_id": self.active_project_id(),
                                "caminho": path.display().to_string(),
                            })),
                    )
                } else {
                    Err(self.message.clone())
                }
            }
            "exportar_planta" => self.tool_export(args),
            "importar_dxf" => {
                let input = text(args, "caminho").ok_or("Informe 'caminho' do arquivo DXF.")?;
                let path = resolve_path(&input, "dxf");
                let previous = self.project.clone();
                match crate::io::dxf::import_file(&mut self.project, &path) {
                    Ok(report) => {
                        if report.total_imported() > 0 || report.layers > 0 {
                            self.record_change(previous);
                            self.fit_view_requested();
                        }
                        self.message = report.summary_pt();
                        Ok(ToolOutput::ok(report.summary_pt()).with_structured(json!({
                            "caminho": path.display().to_string(),
                            "paredes": report.walls,
                            "desenhos": report.drawings,
                            "textos": report.notes,
                            "cotas": report.dimensions,
                            "camadas": report.layers,
                            "terreno": report.terrain_imported,
                            "unidades": report.units_label,
                        })))
                    }
                    Err(error) => {
                        self.project = previous;
                        Err(error)
                    }
                }
            }
            _ => Err(format!("Ferramenta desconhecida: {name}")),
        }
    }

    /// Runs `change` on a copy of the project and keeps the result only when
    /// it succeeds, as one undo step. When nothing changed, the change is
    /// either reported as such or, with `unchanged`, turned into that error.
    fn apply_change(
        &mut self,
        unchanged: Option<&str>,
        change: impl FnOnce(&mut Project) -> Result<Change, String>,
    ) -> Result<ToolOutput, String> {
        let mut draft = self.project.clone();
        let outcome = change(&mut draft)?;
        if draft == self.project {
            return match unchanged {
                Some(message) => Err(message.to_owned()),
                None => Ok(ToolOutput::ok(format!(
                    "{} (nenhuma alteração era necessária)",
                    outcome.message
                ))),
            };
        }
        let was_empty = self.project.walls.is_empty()
            && self.project.rooms.is_empty()
            && self.project.terrain.is_none();
        let previous = std::mem::replace(&mut self.project, draft);
        // The plan's constraints hold for the assistant's edits too.
        if !self.record_change(previous) {
            return Err(self.message.clone());
        }
        if was_empty {
            self.fit_view_requested();
        }
        if let Some(selection) = outcome.selection {
            self.set_selection(Some(selection));
        }
        let mut output = ToolOutput::ok(outcome.message);
        if let Some(data) = outcome.data {
            output = output.with_structured(data);
        }
        Ok(output)
    }

    fn check_discard(&self, args: &Value) -> Result<(), String> {
        if self.dirty && !flag(args, "descartar_alteracoes") {
            return Err("A planta atual tem alterações não salvas. Salve com salvar_projeto ou repita com descartar_alteracoes = true.".to_owned());
        }
        Ok(())
    }

    fn find_element(&self, id: u64) -> Result<Selection, String> {
        let project = &self.project;
        if project.walls.iter().any(|wall| wall.id == id) {
            Ok(Selection::Wall(id))
        } else if project.rooms.iter().any(|room| room.id == id) {
            Ok(Selection::Room(id))
        } else if project.openings.iter().any(|opening| opening.id == id) {
            Ok(Selection::Opening(id))
        } else if project.fixtures.iter().any(|fixture| fixture.id == id) {
            Ok(Selection::Fixture(id))
        } else if project.drawing(id).is_some() {
            Ok(Selection::Drawing(id))
        } else if project
            .terrain
            .as_ref()
            .is_some_and(|terrain| terrain.id == id)
        {
            Ok(Selection::Terrain(id))
        } else {
            Err(format!(
                "Não existe elemento com id {id}. Use obter_planta para ver os ids."
            ))
        }
    }

    fn require(
        &self,
        id: u64,
        is_kind: impl Fn(Selection) -> bool,
        kind: &str,
    ) -> Result<Selection, String> {
        let selection = self.find_element(id)?;
        if is_kind(selection) {
            Ok(selection)
        } else {
            Err(format!(
                "O elemento {id} é um(a) {}, não um(a) {kind}.",
                Self::element_label(selection).to_lowercase()
            ))
        }
    }

    fn element_label(selection: Selection) -> &'static str {
        match selection {
            Selection::Wall(_) => "Parede",
            Selection::Room(_) => "Cômodo",
            Selection::Opening(_) => "Abertura",
            Selection::Terrain(_) => "Terreno",
            Selection::Fixture(_) => "Objeto",
            Selection::Drawing(_) => "Desenho",
        }
    }
}
