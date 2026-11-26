//! What the protection layer adds to every assistant answer (roadmap M0):
//! the hypotheses Trazo held back for the person (#171) and what the action
//! made worse (#170); and the building guide tool (#165).

use serde_json::{Value, json};

use crate::app::FloorPlanApp;
use crate::domain::model::Project;
use crate::domain::protection::action_report::action_report;
use crate::domain::protection::guide::{building_guide, guide_text};
use crate::domain::protection::hypotheses::HypothesisProposal;
use crate::mcp::ToolOutput;

impl FloorPlanApp {
    /// Tells the assistant, in its tool answer, what Trazo did not let it
    /// change: favourable hypotheses kept for the person to confirm.
    pub(super) fn report_protection(
        &mut self,
        before: &Project,
        mut output: ToolOutput,
    ) -> ToolOutput {
        if output.is_error {
            return output;
        }
        output = self.report_action(before, output);
        let earlier = before.protection.hypotheses.proposals();
        let held: Vec<&HypothesisProposal> = self
            .project
            .protection
            .hypotheses
            .proposals()
            .iter()
            .filter(|proposal| !earlier.contains(proposal))
            .collect();
        if held.is_empty() {
            return output;
        }
        let list = held
            .iter()
            .map(|p| {
                format!(
                    "{} = {}",
                    p.key.label().to_lowercase(),
                    p.key.format(&self.project, p.value)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        output.text.push_str(&format!(
            "\nHipótese não aplicada: {list}. Um valor mais favorável que o seguro precisa de prova e da confirmação da pessoa na janela Segurança › Hipóteses de cálculo. Explique isso a ela e peça o laudo, a nota fiscal ou a fonte do valor; não tente contornar."
        ));
        let held_json = Value::Array(
            held.iter()
                .map(|p| {
                    json!({
                        "chave": p.key.key(),
                        "valor": p.key.format(&self.project, p.value),
                        "prova_citada": p.evidence,
                    })
                })
                .collect(),
        );
        output.structured = Some(match output.structured.take() {
            Some(Value::Object(mut map)) => {
                map.insert("hipoteses_aguardando_pessoa".to_owned(), held_json);
                Value::Object(map)
            }
            Some(other) => json!({ "resultado": other, "hipoteses_aguardando_pessoa": held_json }),
            None => json!({ "hipoteses_aguardando_pessoa": held_json }),
        });
        output
    }

    /// Adds what the action made worse to the answer and shows it on screen
    /// (#170): worse checks, supporting elements removed, hypotheses changed.
    fn report_action(&mut self, before: &Project, mut output: ToolOutput) -> ToolOutput {
        let report = action_report(before, &self.project);
        if report.is_empty() {
            return output;
        }
        let summary = report.summary();
        output.text.push('\n');
        output.text.push_str(&summary);
        output.text.push_str(
            " Explique isso à pessoa em palavras simples e proponha como corrigir antes de seguir.",
        );
        let worsened: Vec<Value> = report
            .worsened
            .iter()
            .map(|w| {
                json!({
                    "elemento": w.element,
                    "verificacao": w.check,
                    "antes": w.before.map(|l| l.key()),
                    "depois": w.after.key(),
                    "o_que_fazer": w.simple,
                })
            })
            .collect();
        let report_json = json!({
            "verificacoes_piores": worsened,
            "elementos_removidos": report.removed,
            "hipoteses_alteradas": report.hypotheses,
        });
        output.structured = Some(match output.structured.take() {
            Some(Value::Object(mut map)) => {
                map.insert("seguranca_piorou".to_owned(), report_json);
                Value::Object(map)
            }
            Some(other) => json!({ "resultado": other, "seguranca_piorou": report_json }),
            None => json!({ "seguranca_piorou": report_json }),
        });
        // On screen too: the status bar shows it over the AI's message.
        self.edit_notice = Some(format!("IA · {summary}"));
        output
    }

    /// `guia_de_obra`: the checked house as ordered instructions to build.
    pub(super) fn tool_building_guide(&self) -> Result<ToolOutput, String> {
        let guide = building_guide(&self.project);
        let stages: Vec<Value> = guide
            .iter()
            .map(|stage| {
                json!({
                    "etapa": stage.title,
                    "passos": stage.steps,
                    "materiais": stage.materials,
                    "conferir_antes_de_concretar": stage.checklist.as_ref().map(|list| json!({
                        "titulo": list.title,
                        "elementos": list.elements,
                        "itens": list.items,
                    })),
                    "onde_um_profissional_deve_olhar": stage.professional,
                })
            })
            .collect();
        Ok(ToolOutput::ok(guide_text(&guide)).with_structured(json!({ "etapas": stages })))
    }
}
