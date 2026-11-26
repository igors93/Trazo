//! What the protection layer adds to every assistant answer (roadmap M0):
//! the hypotheses Trazo held back for the person (#171).

use serde_json::{Value, json};

use crate::app::FloorPlanApp;
use crate::domain::model::Project;
use crate::domain::protection::hypotheses::HypothesisProposal;
use crate::mcp::ToolOutput;

impl FloorPlanApp {
    /// Tells the assistant, in its tool answer, what Trazo did not let it
    /// change: favourable hypotheses kept for the person to confirm.
    pub(super) fn report_protection(&self, before: &Project, mut output: ToolOutput) -> ToolOutput {
        if output.is_error {
            return output;
        }
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
}
