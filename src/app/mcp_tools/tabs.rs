//! The `abas` tool: several projects open in one window, as tabs. Every
//! other tool works on the active tab.

use serde_json::{Value, json};

use super::{ToolOutput, number, text};
use crate::app::FloorPlanApp;

impl FloorPlanApp {
    pub(super) fn tool_tabs(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".to_owned());
        let done = match action.as_str() {
            "listar" => None,
            "trocar" => {
                let tab =
                    number(args, "aba")?.ok_or("Informe 'aba': o número da aba (1, 2...).")?;
                let count = self.tab_count();
                if tab.fract() != 0.0 || tab < 1.0 || tab > count as f64 {
                    return Err(format!("Não há a aba {tab}: as abas vão de 1 a {count}."));
                }
                self.switch_tab(tab as usize - 1);
                Some("Aba trocada.")
            }
            "nova" => {
                self.new_tab();
                Some("Nova aba com uma planta em branco.")
            }
            "duplicar" => {
                self.duplicate_tab();
                Some("Projeto copiado numa nova aba, que ficou ativa.")
            }
            _ => return Err("Informe 'acao': listar, trocar, nova ou duplicar.".to_owned()),
        };
        let tabs: Vec<Value> = self
            .tab_infos()
            .into_iter()
            .enumerate()
            .map(|(index, info)| {
                json!({
                    "aba": index + 1,
                    "nome": info.title,
                    "ativa": info.active,
                    "alteracoes_nao_salvas": info.dirty,
                })
            })
            .collect();
        let active = self.active_tab() + 1;
        let summary = format!("{} aba(s); a ativa é a {active}.", tabs.len());
        let message = match done {
            Some(done) => format!("{done} {summary}"),
            None => summary,
        };
        Ok(ToolOutput::ok(message).with_structured(json!({ "abas": tabs, "ativa": active })))
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
}
