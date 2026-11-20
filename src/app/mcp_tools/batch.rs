//! `executar_lote`: many tools in one call, all or nothing, as one undo
//! step — a whole kitchen or floor drawn in a single round trip.

use serde_json::{Value, json};

use super::ToolOutput;
use crate::app::FloorPlanApp;
use crate::mcp::protocol;

/// The most actions one batch may hold.
const MAX_ACTIONS: usize = 200;

/// Tools that act on files, history or the whole project, which a batch
/// cannot undo as one step.
const OUTSIDE_BATCH: [&str; 7] = [
    "executar_lote",
    "novo_projeto",
    "abrir_projeto",
    "salvar_projeto",
    "exportar_planta",
    "desfazer",
    "refazer",
];

impl FloorPlanApp {
    pub(super) fn tool_batch(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let actions = args
            .get("acoes")
            .and_then(Value::as_array)
            .ok_or("Informe 'acoes': uma lista de {\"ferramenta\": ..., \"argumentos\": {...}}.")?;
        if actions.is_empty() || actions.len() > MAX_ACTIONS {
            return Err(format!("Use de 1 a {MAX_ACTIONS} ações por lote."));
        }
        // Check every action before running any.
        let known = protocol::tool_names();
        let mut plan = Vec::with_capacity(actions.len());
        for (index, action) in actions.iter().enumerate() {
            let name = action
                .get("ferramenta")
                .and_then(Value::as_str)
                .ok_or(format!("Ação {}: falta 'ferramenta'.", index + 1))?;
            if !known.contains(&name) {
                return Err(format!(
                    "Ação {}: ferramenta desconhecida '{name}'.",
                    index + 1
                ));
            }
            if OUTSIDE_BATCH.contains(&name) {
                return Err(format!(
                    "Ação {}: '{name}' não pode fazer parte de um lote.",
                    index + 1
                ));
            }
            let arguments = match action.get("argumentos") {
                None | Some(Value::Null) => json!({}),
                Some(arguments) if arguments.is_object() => arguments.clone(),
                Some(_) => {
                    return Err(format!(
                        "Ação {}: 'argumentos' deve ser um objeto.",
                        index + 1
                    ));
                }
            };
            plan.push((name, arguments));
        }

        let before = self.project.clone();
        let (dirty, recovery) = (self.dirty, self.recovery_pending);
        let selection = self.selected();
        self.batch_depth += 1;
        let mut results = Vec::with_capacity(plan.len());
        for (index, (name, arguments)) in plan.iter().enumerate() {
            let outcome = self.dispatch_tool(name, arguments);
            let failure = match &outcome {
                Ok(output) if output.is_error => Some(output.text.clone()),
                Ok(_) => None,
                Err(error) => Some(error.clone()),
            };
            if let Some(error) = failure {
                // All or nothing: the plan goes back to how it was.
                self.batch_depth -= 1;
                self.project = before;
                self.dirty = dirty;
                self.recovery_pending = recovery;
                self.set_selection_set(selection);
                return Err(format!(
                    "Ação {} ({name}) falhou: {error} Nenhuma ação do lote foi aplicada.",
                    index + 1
                ));
            }
            if let Ok(output) = outcome {
                results.push(json!({
                    "ferramenta": name,
                    "mensagem": output.text,
                    "dados": output.structured,
                }));
            }
        }
        self.batch_depth -= 1;
        if self.project != before {
            self.history.record(before);
            self.mark_edited_outside_history();
        }
        Ok(ToolOutput::ok(format!(
            "{} ações executadas como um único passo de desfazer.",
            results.len()
        ))
        .with_structured(json!({ "resultados": results })))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn a_batch_is_all_or_nothing_and_one_undo_step() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let output = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 12, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 3 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Cozinha", "x": 5, "y": 1, "largura": 3, "comprimento": 3 } },
            ] }),
        );
        assert!(!output.is_error, "{}", output.text);
        assert_eq!(app.project.rooms.len(), 2);
        assert_eq!(
            output.structured.unwrap()["resultados"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        // One Ctrl+Z takes the whole batch back.
        app.undo();
        assert!(app.project.rooms.is_empty() && app.project.terrain.is_none());
        app.redo();

        // A failing action leaves no trace of the ones before it.
        let before = app.project.clone();
        let failed = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "criar_parede", "argumentos": { "x1": 2, "y1": 8, "x2": 6, "y2": 8 } },
                { "ferramenta": "excluir_elemento", "argumentos": { "id": 9999 } },
            ] }),
        );
        assert!(failed.is_error && failed.text.contains("Ação 2"));
        assert_eq!(app.project, before);
        // Files and history stay outside batches.
        let refused = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [{ "ferramenta": "desfazer" }] }),
        );
        assert!(refused.is_error);
        // Normal edits go on recording undo steps afterwards.
        app.run_mcp_tool(
            "criar_parede",
            &json!({ "x1": 2, "y1": 8, "x2": 6, "y2": 8 }),
        );
        app.undo();
        assert_eq!(app.project, before);
    }
}
