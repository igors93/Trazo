//! The tools an AI assistant runs through MCP (see `crate::mcp`). They act
//! on the open project the way the interface does: each change is one undo
//! step, marks the project unsaved and feeds the recovery draft.
//!
//! The assistant sees plan coordinates in metres with X to the right and Y
//! up, as the status bar shows them; the model's Y grows downwards on
//! screen, so Y is flipped at this boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{FloorPlanApp, OpenResult, Selection};

mod batch;
mod drawings;
mod editing;
mod electrical;
mod project_docs;
mod shapes;
mod structure;
mod tabs;
use crate::{
    domain::{
        fixtures::FixtureKind,
        geometry::{Point, format_meters},
        model::{OpeningKind, Project},
    },
    io::export,
    mcp::{ToolOutput, protocol},
};

/// Largest coordinate or length accepted, in metres.
const LIMIT: f64 = 10_000.0;

fn to_model(x: f64, y: f64) -> Point {
    Point::new(x, -y)
}

fn from_model(point: Point) -> (f64, f64) {
    // Adding 0.0 turns -0.0 into 0.0.
    (point.x + 0.0, -point.y + 0.0)
}

fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0 + 0.0
}

fn xy(point: Point) -> Value {
    let (x, y) = from_model(point);
    json!({ "x": round(x), "y": round(y) })
}

fn square_metres(value: f64) -> String {
    format!("{value:.2} m²").replace('.', ",")
}

/// An optional number argument; strings such as "3,5" are accepted too.
fn number(args: &Value, key: &str) -> Result<Option<f64>, String> {
    let value = match args.get(key) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().replace(',', ".").parse::<f64>().ok(),
        Some(_) => None,
    };
    match value {
        Some(value) if value.is_finite() && value.abs() <= LIMIT => Ok(Some(value)),
        _ => Err(format!(
            "'{key}' deve ser um número entre -10000 e 10000 (metros)."
        )),
    }
}

/// A whole number from 0 up given as `key`, if any.
pub(super) fn optional_count(args: &Value, key: &str) -> Result<Option<u64>, String> {
    let wrong = || format!("'{key}' deve ser um número inteiro de 0 em diante.");
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .or_else(|| {
                number
                    .as_f64()
                    .filter(|value| *value >= 0.0 && value.fract() == 0.0)
                    .map(|value| value as u64)
            })
            .map(Some)
            .ok_or_else(wrong),
        Some(Value::String(text)) => text.trim().parse::<u64>().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn required_number(args: &Value, key: &str) -> Result<f64, String> {
    number(args, key)?.ok_or_else(|| format!("Informe '{key}' (número, em metros)."))
}

fn element_id(args: &Value, key: &str) -> Result<u64, String> {
    let id = match args.get(key) {
        Some(Value::Number(number)) => number.as_u64().or_else(|| {
            number
                .as_f64()
                .filter(|v| v.fract() == 0.0 && *v >= 0.0)
                .map(|v| v as u64)
        }),
        Some(Value::String(text)) => text.trim().parse::<u64>().ok(),
        _ => None,
    };
    id.ok_or_else(|| format!("Informe '{key}': o id numérico do elemento (veja obter_planta)."))
}

/// `rotacao` in degrees (a multiple of 90) as clockwise quarter turns.
fn quarter_turns(args: &Value) -> Result<Option<u8>, String> {
    let Some(degrees) = number(args, "rotacao")? else {
        return Ok(None);
    };
    let turns = degrees / 90.0;
    if (turns - turns.round()).abs() > 0.01 {
        return Err("'rotacao' deve ser 0, 90, 180 ou 270 graus.".to_owned());
    }
    Ok(Some((turns.round() as i32).rem_euclid(4) as u8))
}

fn text(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn flag(args: &Value, key: &str) -> bool {
    match args.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(text)) => {
            matches!(text.trim().to_lowercase().as_str(), "true" | "sim" | "1")
        }
        _ => false,
    }
}

/// Trazo's folder inside the person's Documents (OneDrive's when Windows
/// keeps Documents there), where files go when no folder is given.
fn documents_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let one_drive = std::env::var_os("OneDrive").map(PathBuf::from);
    let candidates = one_drive
        .iter()
        .flat_map(|folder| [folder.join("Documentos"), folder.join("Documents")])
        .chain([home.join("Documents"), home.join("Documentos")]);
    candidates
        .into_iter()
        .find(|folder| folder.is_dir())
        .unwrap_or(home)
        .join("Trazo")
}

/// A path given by the assistant: relative ones are taken from Trazo's
/// Documents folder, and a missing extension is added.
fn resolve_path(input: &str, extension: &str) -> PathBuf {
    let mut path = PathBuf::from(input.trim().trim_matches('"'));
    if path.is_relative() {
        path = documents_dir().join(path);
    }
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}

/// `path`, or the first "name (2).ext", "name (3).ext"... that is free.
fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|index| path.with_file_name(format!("{stem} ({index}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(path)
}

/// The project name as a safe file name.
fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "planta".to_owned()
    } else {
        cleaned
    }
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Não foi possível criar a pasta {}: {error}",
                parent.display()
            )
        })?;
    }
    Ok(())
}

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
                let png = export::png_preview_for_view(&self.project, &view, side as u32)?;
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
            "cotas" => self.tool_dimensions(args),
            "circulacao" => self.tool_circulation(args),
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

    fn tool_export(&self, args: &Value) -> Result<ToolOutput, String> {
        let format = text(args, "formato")
            .map(|format| format.to_lowercase())
            .ok_or("Informe 'formato': pdf, pdf_abas, png, dxf, svg, prancha_pdf, prancha_pdf_abas, prancha_svg, memorial_pdf, unifilar_svg, unifilar_pdf, esforcos_svg, esforcos_pdf, porticos_svg, porticos_pdf, grelha_svg, grelha_pdf, armaduras_svg, armaduras_pdf, quantitativos_csv, quantitativos_svg, quantitativos_pdf, tabela_aco_csv ou csv.")?;
        let (ext, suffix) = match format.as_str() {
            "pdf" => ("pdf", ""),
            "pdf_abas" => ("pdf", "-abas"),
            "png" => ("png", ""),
            "dxf" => ("dxf", ""),
            "svg" => ("svg", ""),
            "prancha_pdf" => ("pdf", "-prancha"),
            "prancha_pdf_abas" => ("pdf", "-pranchas-abas"),
            "prancha_svg" => ("svg", "-prancha"),
            "memorial_pdf" => ("pdf", "-memorial"),
            "unifilar_svg" => ("svg", "-unifilar"),
            "unifilar_pdf" => ("pdf", "-unifilar"),
            "esforcos_svg" => ("svg", "-esforcos"),
            "esforcos_pdf" => ("pdf", "-esforcos"),
            "porticos_svg" => ("svg", "-porticos"),
            "porticos_pdf" => ("pdf", "-porticos"),
            "grelha_svg" => ("svg", "-grelha"),
            "grelha_pdf" => ("pdf", "-grelha"),
            "armaduras_svg" => ("svg", "-armaduras"),
            "armaduras_pdf" => ("pdf", "-armaduras"),
            "quantitativos_csv" => ("csv", "-quantitativos"),
            "quantitativos_svg" => ("svg", "-quantitativos"),
            "quantitativos_pdf" => ("pdf", "-quantitativos"),
            "tabela_aco_csv" => ("csv", "-tabela-aco"),
            "csv" => ("csv", "-quadros"),
            _ => {
                return Err("Formato inválido. Use: pdf, pdf_abas, png, dxf, svg, prancha_pdf, prancha_pdf_abas, prancha_svg, memorial_pdf, unifilar_svg, unifilar_pdf, esforcos_svg, esforcos_pdf, porticos_svg, porticos_pdf, grelha_svg, grelha_pdf, armaduras_svg, armaduras_pdf, quantitativos_csv, quantitativos_svg, quantitativos_pdf, tabela_aco_csv ou csv.".to_owned());
            }
        };
        let explicit_discipline = text(args, "disciplina");
        let discipline = match explicit_discipline.as_deref() {
            Some(raw) => crate::domain::discipline::Discipline::from_key(raw).ok_or(
                "'disciplina' deve ser tudo, arquitetura, eletrica, hidraulica, estrutura ou fundacoes.",
            )?,
            None => self.discipline,
        };
        let path = match text(args, "caminho") {
            Some(input) => resolve_path(&input, ext),
            None => unique_path(
                documents_dir().join(format!("{}{suffix}.{ext}", file_stem(&self.project.name))),
            ),
        };
        ensure_parent(&path)?;
        let view = if explicit_discipline.is_none() {
            self.active_export_view()
        } else {
            export::ExportView::for_discipline(&self.project, discipline)
        };
        let shown = self.project.for_discipline(discipline);
        let mut extra_pages: Option<usize> = None;
        match format.as_str() {
            "pdf" => export::pdf_for_view(&self.project, &view, &path)?,
            "pdf_abas" => {
                let views = self.project_tab_export_views();
                extra_pages = Some(export::pdf_views(&self.project, &views, &path)?);
            }
            "png" => export::png_for_view(&self.project, &view, &path)?,
            "dxf" => crate::io::dxf::export(&shown, &path)?,
            "svg" => export::svg_for_view(&self.project, &view, &path)?,
            "prancha_pdf" => export::export_scaled_pdf_for_view(&self.project, &view, &path)?,
            "prancha_pdf_abas" => {
                let views = self.project_tab_export_views();
                extra_pages = Some(export::export_scaled_pdf_views(
                    &self.project,
                    &views,
                    &path,
                )?);
            }
            "prancha_svg" => export::export_scaled_svg_for_view(&self.project, &view, &path)?,
            "memorial_pdf" => {
                let date = crate::io::report::today();
                let pages =
                    crate::io::report::export_presizing_report(&self.project, &date, &path)?;
                extra_pages = Some(pages);
            }
            "unifilar_svg" => {
                let markup = crate::domain::distribution::single_line_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "unifilar_pdf" => {
                let markup = crate::domain::distribution::single_line_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "esforcos_svg" => {
                let markup = crate::domain::structure::beam_diagrams_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "esforcos_pdf" => {
                let markup = crate::domain::structure::beam_diagrams_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "porticos_svg" => {
                let markup = crate::domain::frame2d::frame_diagrams_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "porticos_pdf" => {
                let markup = crate::domain::frame2d::frame_diagrams_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "grelha_svg" => {
                let grid = self.project.cached_grid_analysis();
                let markup = crate::domain::grid_analysis::grid_analysis_svg(&self.project, &grid);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "grelha_pdf" => {
                let grid = self.project.cached_grid_analysis();
                let markup = crate::domain::grid_analysis::grid_analysis_svg(&self.project, &grid);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "armaduras_svg" => {
                let structure = self.project.cached_structure_report();
                let reinforcement = self.project.cached_reinforcement_report();
                let markup = crate::domain::reinforcement::reinforcement_detail_svg(
                    &self.project,
                    &structure,
                    &reinforcement,
                );
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "armaduras_pdf" => {
                let structure = self.project.cached_structure_report();
                let reinforcement = self.project.cached_reinforcement_report();
                let markup = crate::domain::reinforcement::reinforcement_detail_svg(
                    &self.project,
                    &structure,
                    &reinforcement,
                );
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "quantitativos_csv" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let takeoff = self
                    .project
                    .construction_takeoff_with_options(all_floors, None);
                let csv = crate::domain::quantities::takeoff_csv_for(&takeoff);
                std::fs::write(&path, format!("\u{FEFF}{csv}"))
                    .map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "quantitativos_svg" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let markup =
                    crate::domain::quantities::takeoff_sheet_svg(&self.project, all_floors);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "quantitativos_pdf" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let markup =
                    crate::domain::quantities::takeoff_sheet_svg(&self.project, all_floors);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "tabela_aco_csv" => {
                let reinforcement = self.project.cached_reinforcement_report();
                let csv = crate::domain::reinforcement::steel_schedule_csv(&reinforcement);
                std::fs::write(&path, csv).map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "csv" => export::csv(&shown, &path)?,
            other => return Err(format!("Formato de exportação desconhecido: '{other}'.")),
        }
        let msg = match (format.as_str(), extra_pages) {
            ("memorial_pdf", Some(pages)) => format!(
                "Memorial técnico ({pages} páginas) exportado em {}",
                path.display()
            ),
            ("pdf_abas" | "prancha_pdf_abas", Some(pages)) => format!(
                "PDF com todas as abas ({pages} página(s)) exportado em {}",
                path.display()
            ),
            _ => format!("Exportado ({format}) em {}", path.display()),
        };
        Ok(ToolOutput::ok(msg).with_structured(json!({
            "caminho": path.display().to_string(),
            "formato": format,
            "disciplina": discipline.key(),
            "paginas": extra_pages,
        })))
    }

    fn tool_terrain(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let recuo_frente = number(args, "recuo_frente")?;
        let recuo_fundos = number(args, "recuo_fundos")?;
        let recuo_lateral = number(args, "recuo_lateral")?;
        let has_setbacks =
            recuo_frente.is_some() || recuo_fundos.is_some() || recuo_lateral.is_some();
        let width_opt = number(args, "largura")?;
        let length_opt = number(args, "comprimento")?;

        if width_opt.is_none() && length_opt.is_none() && has_setbacks {
            if self.project.terrain.is_none() {
                return Err(
                    "Defina 'largura' e 'comprimento' do terreno antes de configurar recuos."
                        .to_owned(),
                );
            }
            let mut setbacks = self.project.setbacks;
            if let Some(v) = recuo_frente {
                setbacks.front = v;
            }
            if let Some(v) = recuo_fundos {
                setbacks.rear = v;
            }
            if let Some(v) = recuo_lateral {
                setbacks.side = v;
            }
            setbacks.validate()?;
            let mut output =
                self.apply_change(Some("Os recuos já tinham esses valores."), |project| {
                    project.set_setbacks(setbacks)?;
                    Ok(Change::new(format!(
                        "Recuos definidos: frente {}, fundos {}, lateral {}.",
                        format_meters(setbacks.front),
                        format_meters(setbacks.rear),
                        format_meters(setbacks.side)
                    ))
                    .data(json!({
                        "recuos": {
                            "frente": round(setbacks.front),
                            "fundos": round(setbacks.rear),
                            "lateral": round(setbacks.side),
                        }
                    })))
                })?;
            if !self.project.content_inside_buildable_limit() {
                output
                    .text
                    .push_str(" Atenção: há paredes invadindo a faixa de recuo configurada.");
            }
            return Ok(output);
        }

        let width = required_number(args, "largura")?;
        let length = required_number(args, "comprimento")?;
        if width < 0.25 || length < 0.25 {
            return Err(
                "O terreno precisa ter pelo menos 0,25 m de largura e de comprimento.".to_owned(),
            );
        }
        let mut setbacks = self.project.setbacks;
        if let Some(v) = recuo_frente {
            setbacks.front = v;
        }
        if let Some(v) = recuo_fundos {
            setbacks.rear = v;
        }
        if let Some(v) = recuo_lateral {
            setbacks.side = v;
        }
        if has_setbacks {
            setbacks.validate()?;
        }
        let (min, max) = match (number(args, "x")?, number(args, "y")?) {
            (Some(x), Some(y)) => (to_model(x, y + length), to_model(x + width, y)),
            (None, None) => match (&self.project.terrain, self.project.content_bounds()) {
                // Keep the lower left corner the assistant already knows.
                (Some(terrain), _) => (
                    Point::new(terrain.min.x, terrain.max.y - length),
                    Point::new(terrain.min.x + width, terrain.max.y),
                ),
                (None, Some((low, high))) => {
                    let origin = Point::new(
                        (low.x + high.x - width) / 2.0,
                        (low.y + high.y - length) / 2.0,
                    );
                    (origin, Point::new(origin.x + width, origin.y + length))
                }
                (None, None) => (to_model(0.0, length), to_model(width, 0.0)),
            },
            _ => return Err("Informe x e y juntos, ou nenhum dos dois.".to_owned()),
        };
        let mut output = self.apply_change(None, |project| {
            let id = project
                .set_terrain(min, max)
                .ok_or("Medidas de terreno inválidas.")?;
            if has_setbacks {
                project.set_setbacks(setbacks)?;
            }
            Ok(Change::new(format!(
                "Terreno de {} × {} ({}) definido.",
                format_meters(width),
                format_meters(length),
                square_metres(width * length)
            ))
            .select(Selection::Terrain(id))
            .data(json!({
                "id": id,
                "recuos": project.setbacks.is_active().then(|| json!({
                    "frente": round(project.setbacks.front),
                    "fundos": round(project.setbacks.rear),
                    "lateral": round(project.setbacks.side),
                })),
            })))
        })?;
        if !self.project.content_inside_terrain() {
            output.text.push_str(
                " Atenção: há paredes ou cômodos fora do terreno; use centralizar_planta ou mova-os.",
            );
        } else if self.project.setbacks.is_active()
            && !self.project.content_inside_buildable_limit()
        {
            output
                .text
                .push_str(" Atenção: há paredes invadindo a faixa de recuo configurada.");
        }
        self.sync_terrain_fields();
        self.fit_view_requested();
        Ok(output)
    }

    fn tool_create_room(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let name = text(args, "nome").ok_or("Informe 'nome' do cômodo.")?;
        if let Some(corners) = args.get("pontos").filter(|value| !value.is_null()) {
            return self.create_polygon_room(&name, corners);
        }
        let x = required_number(args, "x")?;
        let y = required_number(args, "y")?;
        let width = required_number(args, "largura")?;
        let length = required_number(args, "comprimento")?;
        if width < 0.5 || length < 0.5 {
            return Err(
                "O cômodo precisa ter pelo menos 0,50 m de largura e de comprimento.".to_owned(),
            );
        }
        self.apply_change(None, |project| {
            let id = project
                .add_room(to_model(x, y), to_model(x + width, y + length), &name)
                .ok_or(
                    "Não foi possível criar o cômodo: ele precisa ficar inteiro dentro do terreno.",
                )?;
            let walls = project
                .rooms
                .iter()
                .find(|room| room.id == id)
                .map(|room| room.wall_ids.clone())
                .unwrap_or_default();
            Ok(Change::new(format!(
                "Cômodo \"{name}\" criado (id {id}), {} × {}.",
                format_meters(width),
                format_meters(length)
            ))
            .select(Selection::Room(id))
            .data(json!({ "id": id, "paredes": walls })))
        })
    }

    fn tool_create_wall(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
        let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
        self.apply_change(None, |project| {
            let id = project.add_wall(start, end).ok_or(
                "Não foi possível criar a parede: ela precisa ter pelo menos 0,25 m e ficar dentro do terreno.",
            )?;
            Ok(Change::new(format!(
                "Parede criada (id {id}), {}.",
                format_meters(start.distance(end))
            ))
            .select(Selection::Wall(id))
            .data(json!({ "id": id })))
        })
    }

    fn tool_insert_opening(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let wall_id = element_id(args, "parede_id")?;
        let kind = match text(args, "tipo")
            .map(|kind| kind.to_lowercase().replace(['-', ' '], "_"))
            .as_deref()
        {
            Some("porta") => OpeningKind::Door,
            Some("janela") => OpeningKind::Window,
            Some("porta_vai_e_vem" | "vai_e_vem" | "porta_faroeste") => OpeningKind::SaloonDoor,
            Some("vao_livre" | "vao" | "vão_livre" | "vão") => OpeningKind::Gap,
            Some("portao" | "portão") => OpeningKind::Gate,
            _ => {
                return Err(
                    "Informe 'tipo': porta, janela, porta_vai_e_vem, vao_livre ou portao."
                        .to_owned(),
                );
            }
        };
        let position = number(args, "posicao")?.unwrap_or(0.5);
        if !(0.0..=1.0).contains(&position) {
            return Err(
                "'posicao' é uma fração de 0 a 1 ao longo da parede (0,5 = meio).".to_owned(),
            );
        }
        let width = number(args, "largura")?;
        if let Some(width) = width
            && !(kind.min_width()..=kind.max_width()).contains(&width)
        {
            return Err(format!(
                "A largura de {} deve estar entre {} e {}.",
                kind.label().to_lowercase(),
                format_meters(kind.min_width()),
                format_meters(kind.max_width())
            ));
        }
        self.require(
            wall_id,
            |selection| matches!(selection, Selection::Wall(_)),
            "parede",
        )?;
        self.apply_change(None, |project| {
            let id = project.add_opening_sized(wall_id, kind, position, width).ok_or(format!(
                "Não há espaço para a {} nessa posição: ela não pode sobrepor outra abertura nem ficar perto demais das pontas da parede.",
                kind.label().to_lowercase()
            ))?;
            let width = project
                .openings
                .iter()
                .find(|opening| opening.id == id)
                .map(|opening| opening.width())
                .unwrap_or_default();
            Ok(Change::new(format!(
                "{} inserida na parede {wall_id} (id {id}), {}.",
                kind.label(),
                format_meters(width)
            ))
            .select(Selection::Opening(id))
            .data(json!({ "id": id })))
        })
    }

    fn tool_masonry(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let mut material = self.project.masonry.clone();
        if let Some(name) = text(args, "material") {
            material = crate::domain::masonry::Masonry::presets()
                .into_iter()
                .find(|preset| preset.name.to_lowercase() == name.to_lowercase())
                .unwrap_or(crate::domain::masonry::Masonry { name, ..material });
        }
        let cm = |key: &str| number(args, key).map(|value| value.map(|cm| cm / 100.0));
        if let Some(width) = cm("largura_bloco_cm")? {
            material.block_width = width;
        }
        if let Some(length) = cm("comprimento_bloco_cm")? {
            material.block_length = length;
        }
        if let Some(joint) = cm("junta_cm")? {
            material.joint = joint;
        }
        if let Some(plaster) = cm("reboco_cm")? {
            material.plaster = plaster;
        }
        if !material.valid() {
            return Err("Medidas de bloco inválidas: largura de 4 a 50 cm, comprimento de 5 a 100 cm, junta até 5 cm e reboco até 8 cm.".to_owned());
        }
        let height = number(args, "pe_direito")?;
        if height.is_some_and(|height| !(1.5..=10.0).contains(&height)) {
            return Err("pe_direito deve estar entre 1,50 e 10,00 m.".to_owned());
        }
        let all = flag(args, "aplicar_em_todas");
        let label = material.label();
        let thickness = material.thickness();
        let output = self.apply_change(None, |project| {
            project.masonry = material.clone();
            if all {
                project.set_all_walls_masonry(&material);
            }
            if let Some(height) = height {
                project.wall_height = height;
            }
            Ok(Change::new(format!(
                "Alvenaria: {label}, parede acabada de {}{}.",
                format_meters(thickness),
                if all {
                    ", aplicada a todas as paredes"
                } else {
                    " para as novas paredes"
                }
            )))
        })?;
        self.sync_masonry_fields();
        Ok(output)
    }

    fn tool_insert_fixture(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let keys: Vec<&str> = FixtureKind::ALL.iter().map(|kind| kind.key()).collect();
        let kind = text(args, "tipo")
            .and_then(|kind| FixtureKind::from_key(&kind.to_lowercase()))
            .ok_or_else(|| format!("Informe 'tipo': {}.", keys.join(", ")))?;
        let (default_width, default_depth) = kind.default_size();
        let width = number(args, "largura")?.unwrap_or(default_width);
        let depth = number(args, "profundidade")?.unwrap_or(default_depth);
        let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
        let turns = quarter_turns(args)?.unwrap_or(0);
        let against_wall = flag(args, "encostar_na_parede");
        let placed = if against_wall {
            self.place_against_wall(kind, point, width, depth, turns)
        } else {
            crate::domain::fixtures::Fixture {
                id: 0,
                kind,
                center: point,
                width,
                depth,
                quarter_turns: turns,
                angle_rad: 0.0,
                electrical: kind.default_electrical(),
                structural_load_kn: kind.default_structural_load_kn(),
            }
        };
        let moved_to_wall = against_wall && placed.center != point;
        let mut output = self.apply_change(None, |project| {
            let id = project
                .add_fixture(kind, placed.center, width, depth, placed.quarter_turns)
                .ok_or(format!(
                    "Não foi possível inserir: o objeto precisa ficar dentro do terreno e medir entre {:.2} e {:.2} m.",
                    kind.min_size(),
                    crate::domain::fixtures::MAX_SIZE
                ).replace('.', ","))?;
            Ok(Change::new(format!(
                "{} inserido(a) (id {id}), {} × {}, rotação {}°.",
                kind.label(),
                format_meters(width),
                format_meters(depth),
                placed.quarter_turns as u32 * 90
            ))
            .select(Selection::Fixture(id))
            .data(json!({ "id": id, "centro": xy(placed.center), "rotacao": placed.quarter_turns as u32 * 90 })))
        })?;
        if against_wall && !moved_to_wall {
            output
                .text
                .push_str(" Nenhuma parede reta a menos de 0,8 m: ficou no ponto informado.");
        }
        Ok(output)
    }

    fn tool_edit_fixture(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Fixture(_)),
            "objeto",
        )?;
        let current = self
            .project
            .fixtures
            .iter()
            .find(|fixture| fixture.id == id)
            .copied()
            .ok_or("Objeto não encontrado.")?;
        let x = number(args, "x")?;
        let y = number(args, "y")?;
        let width = number(args, "largura")?;
        let depth = number(args, "profundidade")?;
        let turns = quarter_turns(args)?;
        let free_angle = number(args, "angulo_graus")?;
        if x.is_none()
            && y.is_none()
            && width.is_none()
            && depth.is_none()
            && turns.is_none()
            && free_angle.is_none()
        {
            return Err("Informe x, y, rotacao, angulo_graus, largura ou profundidade.".to_owned());
        }
        let (cx, cy) = from_model(current.center);
        let center =
            (x.is_some() || y.is_some()).then(|| to_model(x.unwrap_or(cx), y.unwrap_or(cy)));
        let size = (width.is_some() || depth.is_some()).then(|| {
            (
                width.unwrap_or(current.width),
                depth.unwrap_or(current.depth),
            )
        });
        let angle_update = if let Some(deg) = free_angle {
            let norm = deg.rem_euclid(360.0);
            let q = ((norm / 90.0).floor() as u8) % 4;
            let rem_deg = norm - f64::from(q) * 90.0;
            let rem_rad = if rem_deg.abs() < 1e-6 {
                0.0
            } else {
                rem_deg.to_radians()
            };
            Some((q, rem_rad))
        } else {
            turns.map(|q| (q, 0.0))
        };
        self.apply_change(None, |project| {
            if center.is_some() || size.is_some() {
                project.update_fixture(id, center, size, None)?;
            }
            if let Some((q, rad)) = angle_update {
                project.set_fixture_angle(id, q, rad)?;
            }
            Ok(Change::new(format!("Objeto {id} atualizado.")).select(Selection::Fixture(id)))
        })
    }

    fn tool_edit_room(&mut self, args: &Value) -> Result<ToolOutput, String> {
        use crate::domain::finishes::{RoomFinishPreset, RoomFinishes};
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Room(_)),
            "cômodo",
        )?;
        let name = text(args, "nome");
        let width = number(args, "largura")?;
        let length = number(args, "comprimento")?;
        let finish_preset = match text(args, "preset_acabamento") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let preset = match key.as_str() {
                    "social_porcelanato" | "social" => RoomFinishPreset::SocialPorcelanato,
                    "dormitorio_madeira" | "dormitorio" | "quarto" => {
                        RoomFinishPreset::DormitorioMadeira
                    }
                    "area_molhada_banheiro" | "banheiro" => RoomFinishPreset::AreaMolhadaBanheiro,
                    "cozinha_area_servico" | "cozinha" => RoomFinishPreset::CozinhaAreaServico,
                    "area_externa_varanda" | "varanda" | "externa" => {
                        RoomFinishPreset::AreaExternaVaranda
                    }
                    _ => {
                        return Err(
                            "'preset_acabamento' deve ser social_porcelanato, dormitorio_madeira, banheiro, cozinha ou varanda."
                                .to_owned(),
                        );
                    }
                };
                Some(preset)
            }
            None => None,
        };
        let floor_fin = text(args, "piso");
        let wall_fin = text(args, "parede");
        let ceiling_fin = text(args, "teto");
        let baseboard_fin = text(args, "rodape");
        let baseboard_cm = number(args, "rodape_cm")?;
        let waterproofing = args.get("impermeabilizacao").and_then(Value::as_bool);
        let finish_notes = text(args, "notas_acabamento");
        let has_finish_fields = floor_fin.is_some()
            || wall_fin.is_some()
            || ceiling_fin.is_some()
            || baseboard_fin.is_some()
            || baseboard_cm.is_some()
            || waterproofing.is_some()
            || finish_notes.is_some();

        if name.is_none()
            && width.is_none()
            && length.is_none()
            && finish_preset.is_none()
            && !has_finish_fields
        {
            return Err(
                "Informe nome, largura, comprimento, preset_acabamento ou campos de acabamento (piso, parede, teto, rodape, rodape_cm, impermeabilizacao)."
                    .to_owned(),
            );
        }
        if let Some(cm) = baseboard_cm
            && !(0.0..=50.0).contains(&cm)
        {
            return Err("'rodape_cm' deve ficar entre 0 e 50 cm.".to_owned());
        }
        let current_bounds = self
            .project
            .rooms
            .iter()
            .find(|room| room.id == id)
            .and_then(|room| self.project.room_bounds(room))
            .map(|(min, max)| (max.x - min.x, max.y - min.y));
        if (width.is_some() || length.is_some()) && current_bounds.is_none() {
            return Err("Cômodo sem paredes válidas para medir.".to_owned());
        }
        self.apply_change(None, |project| {
            if let Some(name) = &name {
                project.rename_room(id, name)?;
            }
            if (width.is_some() || length.is_some())
                && let Some((current_width, current_length)) = current_bounds
            {
                project.resize_room_dimensions(
                    id,
                    width.unwrap_or(current_width),
                    length.unwrap_or(current_length),
                )?;
            }
            if let Some(preset) = finish_preset {
                project.apply_room_finish_preset(id, preset)?;
            }
            if has_finish_fields {
                let mut finishes = project
                    .rooms
                    .iter()
                    .find(|r| r.id == id)
                    .and_then(|r| r.finishes.clone())
                    .unwrap_or_else(|| RoomFinishes {
                        baseboard_height_cm: 10.0,
                        ..RoomFinishes::default()
                    });
                if let Some(v) = &floor_fin {
                    finishes.floor = v.clone();
                }
                if let Some(v) = &wall_fin {
                    finishes.wall = v.clone();
                }
                if let Some(v) = &ceiling_fin {
                    finishes.ceiling = v.clone();
                }
                if let Some(v) = &baseboard_fin {
                    finishes.baseboard = v.clone();
                }
                if let Some(cm) = baseboard_cm {
                    finishes.baseboard_height_cm = cm;
                }
                if let Some(w) = waterproofing {
                    finishes.waterproofing = w;
                }
                if let Some(v) = &finish_notes {
                    finishes.notes = v.clone();
                }
                project.set_room_finishes(id, Some(finishes))?;
            }
            let room = project.rooms.iter().find(|room| room.id == id);
            let summary = room
                .and_then(|room| {
                    let (min, max) = project.room_bounds(room)?;
                    Some(format!(
                        "\"{}\", {} × {}",
                        room.display_name(),
                        format_meters(max.x - min.x),
                        format_meters(max.y - min.y)
                    ))
                })
                .unwrap_or_default();
            Ok(Change::new(format!("Cômodo {id} atualizado: {summary}."))
                .select(Selection::Room(id)))
        })
    }

    fn tool_edit_opening(&mut self, args: &Value) -> Result<ToolOutput, String> {
        use crate::domain::openings::{DoorType, WindowType};
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Opening(_)),
            "porta ou janela",
        )?;
        let width = number(args, "largura")?;
        let position = number(args, "posicao")?;
        let flip_side = flag(args, "inverter_lado");
        let flip_hinge = flag(args, "inverter_dobradica");
        let height = number(args, "altura")?;
        let sill = number(args, "peitoril")?;
        let door_type = match text(args, "tipo_porta") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let dt = match key.as_str() {
                    "giro" | "swing" => DoorType::Swing,
                    "dupla" | "double_swing" => DoorType::DoubleSwing,
                    "correr" | "sliding" => DoorType::Sliding,
                    "correr_embutida" | "pocket_sliding" => DoorType::PocketSliding,
                    "pivotante" | "pivot" => DoorType::Pivot,
                    "sanfonada" | "camarao" | "bifold" => DoorType::Bifold,
                    "giratoria" | "revolving" => DoorType::Revolving,
                    _ => {
                        return Err(
                            "'tipo_porta' deve ser giro, dupla, correr, correr_embutida, pivotante, sanfonada ou giratoria."
                                .to_owned(),
                        );
                    }
                };
                Some(dt)
            }
            None => None,
        };
        let window_type = match text(args, "tipo_janela") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let wt = match key.as_str() {
                    "correr" | "sliding" => WindowType::Sliding,
                    "basculante" => WindowType::Basculante,
                    "maxim_ar" | "maximar" => WindowType::MaximAr,
                    "abrir" | "casement" => WindowType::Casement,
                    "oscilo_batente" | "tilt_turn" => WindowType::TiltTurn,
                    "fixa" | "fixed" => WindowType::Fixed,
                    "bay_window" => WindowType::BayWindow,
                    _ => {
                        return Err(
                            "'tipo_janela' deve ser correr, basculante, maxim_ar, abrir, oscilo_batente, fixa ou bay_window."
                                .to_owned(),
                        );
                    }
                };
                Some(wt)
            }
            None => None,
        };
        if width.is_none()
            && position.is_none()
            && !flip_side
            && !flip_hinge
            && height.is_none()
            && sill.is_none()
            && door_type.is_none()
            && window_type.is_none()
        {
            return Err(
                "Informe largura, posicao, altura, peitoril, tipo_porta, tipo_janela, inverter_lado ou inverter_dobradica."
                    .to_owned(),
            );
        }
        if position.is_some_and(|position| !(0.0..=1.0).contains(&position)) {
            return Err("'posicao' é uma fração de 0 a 1 ao longo da parede.".to_owned());
        }
        self.apply_change(None, |project| {
            if let Some(width) = width {
                project.update_opening_width(id, width)?;
            }
            if let Some(position) = position {
                let wall = project
                    .openings
                    .iter()
                    .find(|opening| opening.id == id)
                    .and_then(|opening| {
                        project.walls.iter().find(|wall| wall.id == opening.wall_id)
                    })
                    .cloned()
                    .ok_or("A parede dessa abertura não existe.")?;
                let target = Point::new(
                    wall.start.x + (wall.end.x - wall.start.x) * position,
                    wall.start.y + (wall.end.y - wall.start.y) * position,
                );
                if !project.move_opening_along_wall(id, target) {
                    return Err("Não há espaço para a abertura nessa posição.".to_owned());
                }
            }
            if flip_side || flip_hinge {
                project.flip_opening(id, flip_side, flip_hinge);
            }
            if height.is_some() || sill.is_some() {
                project.set_opening_heights(id, height, sill)?;
            }
            if let Some(dt) = door_type {
                project.set_opening_door_type(id, dt)?;
            }
            if let Some(wt) = window_type {
                project.set_opening_window_type(id, wt)?;
            }
            Ok(Change::new(format!("Abertura {id} atualizada.")).select(Selection::Opening(id)))
        })
    }

    fn tool_move(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        let dx = required_number(args, "dx")?;
        let dy = required_number(args, "dy")?;
        let selection = self.find_element(id)?;
        let delta = Point::new(dx, -dy);
        let opening_target = self
            .opening_center(id)
            .map(|center| Point::new(center.x + delta.x, center.y + delta.y));
        let label = Self::element_label(selection);
        self.apply_change(
            Some("Não foi possível mover: o terreno ou as paredes vizinhas não deixam espaço nessa direção."),
            |project| {
                match selection {
                    Selection::Wall(id) => project.move_wall(id, delta),
                    Selection::Room(id) => {
                        if !project.move_room(id, delta) {
                            return Err("Esse cômodo compartilha paredes com outro e não pode ser movido sozinho; mova as paredes individualmente.".to_owned());
                        }
                    }
                    Selection::Terrain(id) => project.move_terrain(id, delta),
                    Selection::Fixture(id) => project.move_fixture(id, delta),
                    Selection::Drawing(id) => project.move_drawing(id, delta),
                    Selection::Opening(id) => {
                        if let Some(target) = opening_target {
                            project.move_opening_along_wall(id, target);
                        }
                    }
                }
                Ok(Change::new(format!("{label} {id} movido(a).")).select(selection))
            },
        )
    }

    /// The plan as the assistant sees it, in its own coordinates.
    pub(super) fn plan_summary(&self) -> Value {
        let project = &self.project;
        let terrain = project.terrain.as_ref().map(|terrain| {
            let (x, y) = from_model(Point::new(terrain.min.x, terrain.max.y));
            json!({
                "id": terrain.id,
                "forma": if terrain.vertices.len() >= 3 { "poligono" } else { "retangulo" },
                "divisas": terrain.outline().into_iter().map(xy).collect::<Vec<_>>(),
                "x": round(x),
                "y": round(y),
                "largura": round(terrain.width()),
                "comprimento": round(terrain.height()),
                "area_m2": round(terrain.area()),
                "recuos": project.setbacks.is_active().then(|| json!({
                    "frente": round(project.setbacks.front),
                    "fundos": round(project.setbacks.rear),
                    "lateral": round(project.setbacks.side),
                })),
            })
        });
        let rooms: Vec<Value> = project
            .rooms
            .iter()
            .map(|room| {
                let bounds = project.room_bounds(room);
                let rectangle = project.room_is_rectangle(room);
                let finishes = room.finishes.as_ref().map(|f| {
                    json!({
                        "piso": f.floor,
                        "parede": f.wall,
                        "teto": f.ceiling,
                        "rodape": f.baseboard,
                        "rodape_cm": round(f.baseboard_height_cm),
                        "impermeabilizacao": f.waterproofing,
                        "notas": f.notes,
                    })
                });
                json!({
                    "id": room.id,
                    "nome": room.display_name(),
                    "forma": if rectangle { "retangulo" } else { "poligono" },
                    "centro": project.room_centroid(room).map(xy),
                    "cantos": (!rectangle).then(|| {
                        project
                            .room_outline(room)
                            .unwrap_or_default()
                            .into_iter()
                            .map(xy)
                            .collect::<Vec<_>>()
                    }),
                    "x": bounds.map(|(min, _)| round(min.x + 0.0)),
                    "y": bounds.map(|(_, max)| round(-max.y + 0.0)),
                    "largura": bounds.map(|(min, max)| round(max.x - min.x)),
                    "comprimento": bounds.map(|(min, max)| round(max.y - min.y)),
                    "area_m2": project.room_area(room).map(round),
                    "paredes": room.wall_ids,
                    "acabamentos": finishes,
                })
            })
            .collect();
        let walls: Vec<Value> = project
            .walls
            .iter()
            .map(|wall| {
                let rooms: Vec<&str> = project
                    .rooms
                    .iter()
                    .filter(|room| room.wall_ids.contains(&wall.id))
                    .map(|room| room.display_name())
                    .collect();
                let openings: Vec<u64> = project
                    .openings
                    .iter()
                    .filter(|opening| opening.wall_id == wall.id)
                    .map(|opening| opening.id)
                    .collect();
                json!({
                    "id": wall.id,
                    "espessura": round(wall.thickness()),
                    "material": wall.masonry.as_ref().map(|material| material.name.clone()),
                    "inicio": xy(wall.start),
                    "fim": xy(wall.end),
                    "comprimento": round(wall.length()),
                    "altura": wall.has_own_height().then(|| {
                        let (start, end) = wall.heights(project.wall_height);
                        json!({ "inicio_m": round(start), "fim_m": round(end) })
                    }),
                    "comodos": rooms,
                    "aberturas": openings,
                })
            })
            .collect();
        let frame_types = crate::domain::annotations::frame_types(project);
        let openings: Vec<Value> = project
            .openings
            .iter()
            .map(|opening| {
                json!({
                    "id": opening.id,
                    "tipo": match opening.kind {
                        OpeningKind::Door => "porta",
                        OpeningKind::Window => "janela",
                        OpeningKind::SaloonDoor => "porta_vai_e_vem",
                        OpeningKind::Gap => "vao_livre",
                        OpeningKind::Gate => "portao",
                    },
                    "parede_id": opening.wall_id,
                    "codigo": crate::domain::annotations::frame_of(&frame_types, opening)
                        .map(|frame| frame.code.clone()),
                    "tipologia": opening
                        .door_type
                        .map(|dt| dt.short_label())
                        .or_else(|| opening.window_type.map(|wt| wt.short_label())),
                    "altura": opening.height().map(round),
                    "peitoril": round(opening.sill()),
                    "lado_invertido": opening.flip_side,
                    "dobradica_no_fim": opening.flip_hinge,
                    "posicao": round(opening.t),
                    "largura": round(opening.width()),
                    "centro": self.opening_center(opening.id).map(xy),
                })
            })
            .collect();
        let fixtures: Vec<Value> = project
            .fixtures
            .iter()
            .map(|fixture| {
                let (width, depth) = fixture.extent();
                json!({
                    "id": fixture.id,
                    "tipo": fixture.kind.key(),
                    "nome": fixture.kind.label(),
                    "centro": xy(fixture.center),
                    "largura": round(fixture.width),
                    "profundidade": round(fixture.depth),
                    "rotacao": fixture.quarter_turns as u32 * 90,
                    "angulo_graus": (fixture.angle_rad.abs() >= 1e-6)
                        .then(|| round(fixture.rotation_rad().to_degrees().rem_euclid(360.0))),
                    "ocupa_x": round(width),
                    "ocupa_y": round(depth),
                })
            })
            .collect();
        let areas = project.area_summary();
        let pid = self.active_project_id();
        let linked_tabs: Vec<usize> = (0..self.tab_count())
            .filter(|&i| self.tab_project_id(i) == Some(pid))
            .map(|i| i + 1)
            .collect();
        let active_floor = project.active_floor().map(|f| {
            json!({
                "id": f.id,
                "nome": f.name,
                "nivel_m": round(f.elevation_m),
                "pe_direito_m": round(project.wall_height),
            })
        });
        let floors: Vec<Value> = project
            .floors
            .levels
            .iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "nome": f.name,
                    "nivel_m": round(f.elevation_m),
                    "ativo": Some(f.id) == project.floors.active_floor_id,
                })
            })
            .collect();
        let audit = project.cached_dependency_audit();
        let pending_reviews = audit
            .review_items
            .iter()
            .filter(|r| r.severity != crate::domain::dependencies::ReviewSeverity::Info)
            .count();
        json!({
            "espessura_parede_m": round(self.project.masonry.thickness()),
            "alvenaria": {
                "material": self.project.masonry.name,
                "bloco_largura_cm": round(self.project.masonry.block_width * 100.0),
                "bloco_comprimento_cm": round(self.project.masonry.block_length * 100.0),
                "junta_cm": round(self.project.masonry.joint * 100.0),
                "reboco_cm": round(self.project.masonry.plaster * 100.0),
                "modulo_m": round(self.project.masonry.module()),
                "pe_direito_m": round(self.project.wall_height),
            },
            "revisao": self.plan_log.revision,
            "aba_ativa": self.active_tab() + 1,
            "disciplina": self.discipline.key(),
            "abas_vinculadas": linked_tabs,
            "pavimento_ativo": active_floor,
            "pavimentos": floors,
            "revisoes_pendentes": pending_reviews,
            "ultima_sincronizacao": {
                "atualizados_automaticamente": self.sync_report.auto_updated.len(),
                "bloqueados": self.sync_report.blocked_updates.len(),
            },
            "objetos": fixtures,
            "desenhos": drawings::drawings_summary(project),
            "estrutura_resumo": {
                "pilares": project.fixtures.iter().filter(|f| f.kind == FixtureKind::Column).count(),
                "sapatas": project.fixtures.iter().filter(|f| f.kind == FixtureKind::Footing).count(),
                "mouroes": project.fixtures.iter().filter(|f| f.kind == FixtureKind::TimberPost).count(),
                "vigas": project.structure.beams.len(),
                "lajes": project.structure.slabs.len(),
                "telhados": project.structure.roofs.len(),
                "vigas_alavanca": project.structure.balance_beams.len(),
                "fundacao_paredes": project.structure.wall_foundations.len(),
                "sondagem": project.structure.soil_survey.is_some(),
            },
            "documentacao": {
                "camadas": project.layers.len(),
                "anotacoes": project.notes.len(),
                "cotas": project.dimensions.len(),
                "circulacoes": project.circulations.len(),
                "cortes": project.views.sections.len(),
                "fachadas": project.views.elevations.len(),
                "detalhes": project.views.details.len(),
                "prancha": {
                    "formato": project.sheet.size.short_name(),
                    "orientacao": project.sheet.orientation.label(),
                    "escala": project.sheet.scale.label(),
                },
            },
            // What the person has selected on screen, so "this one" makes sense.
            "selecao": self.selected().iter().map(|item| item.id()).collect::<Vec<_>>(),
            "grupos": project
                .groups
                .iter()
                .map(|group| json!({
                    "id": group.id,
                    "nome": group.name,
                    "membros": group.members.iter().map(|member| member.id()).collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
            "restricoes": project
                .constraints
                .iter()
                .map(|constraint| {
                    let [a, b] = constraint.kind.entities();
                    json!({
                        "id": constraint.id,
                        "tipo": constraint.kind.label(),
                        "a": a.element().id(),
                        "b": b.element().id(),
                        "distancia": constraint.kind.distance().map(round),
                    })
                })
                .collect::<Vec<_>>(),
            "projeto": {
                "id": pid,
                "nome": project.name,
                "arquivo": self.file_path.as_ref().map(|path| path.display().to_string()),
                "alteracoes_nao_salvas": self.dirty,
                "unidade": "metro",
                "coordenadas": "X para a direita, Y para cima",
            },
            "terreno": terrain,
            "comodos": rooms,
            "paredes": walls,
            "aberturas": openings,
            "quadro_esquadrias": frame_types
                .iter()
                .map(|frame| json!({
                    "codigo": frame.code,
                    "tipo": frame.kind.label(),
                    "largura": round(frame.width),
                    "altura": frame.height.map(round),
                    "peitoril": round(frame.sill),
                    "quantidade": frame.ids.len(),
                }))
                .collect::<Vec<_>>(),
            "areas": {
                "util_m2": round(areas.usable),
                "paredes_m2": round(areas.walls),
                "construida_m2": round(areas.constructed),
                "projecao_m2": round(areas.projection),
                "terreno_m2": round(areas.terrain),
            },
            "area_util_m2": round(areas.usable),
            "area_construida_m2": round(areas.constructed),
            "area_projecao_m2": round(areas.projection),
            "area_terreno_m2": round(areas.terrain),
            "taxa_ocupacao_percentual": (areas.terrain > 0.0)
                .then(|| round(areas.projection / areas.terrain * 100.0)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

#[cfg(test)]
mod gate_tests {
    use crate::app::FloorPlanApp;
    use crate::domain::model::OpeningKind;
    use crate::domain::openings::DoorType;
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
        let dxf_path =
            std::env::temp_dir().join(format!("trazo-mcp-dxf-{}.dxf", std::process::id()));
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
}
