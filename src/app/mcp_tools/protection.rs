//! What the protection layer adds to every assistant answer (roadmap M0):
//! the hypotheses Trazo held back for the person (#171) and what the action
//! made worse (#170); the building guide (#165) and the plain-language
//! review (#166) tools.

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

    /// `revisar_projeto`: the project reviewed for the family, in plain
    /// words, with what needs a professional and where to get help.
    pub(super) fn tool_review_project(&self) -> Result<ToolOutput, String> {
        use crate::domain::protection::{glossary, help, hypotheses};
        use crate::domain::verdict::{Light, safety_summary};
        let project = &self.project;
        let summary = safety_summary(project);
        let pick = |light: Light| -> Vec<Value> {
            summary
                .verdicts
                .iter()
                .filter(|v| v.light() == light)
                .map(|v| {
                    let check = v.checks.iter().find(|c| c.light == light);
                    json!({
                        "elemento": v.element,
                        "o_que_fazer": check.map(|c| c.simple.clone()),
                    })
                })
                .collect()
        };
        let red = pick(Light::Red);
        let yellow = pick(Light::Yellow);
        let contested: Vec<Value> = hypotheses::contests(project)
            .iter()
            .map(|c| json!({ "hipotese": c.key.label(), "motivo": c.reason, "o_que_fazer": c.fix }))
            .collect();
        let waiting: Vec<Value> = project
            .protection
            .hypotheses
            .proposals()
            .iter()
            .map(|p| json!({ "hipotese": p.key.label(), "valor": p.key.format(project, p.value) }))
            .collect();
        let risks = &project.protection.site_risks;
        let headline = glossary::family_summary(&summary);
        let mut text = vec![headline.clone()];
        if !red.is_empty() {
            text.push(format!(
                "{} ponto(s) vermelho(s): explique cada um à família e corrija o que puder.",
                red.len()
            ));
        }
        if !contested.is_empty() || !waiting.is_empty() {
            text.push(format!(
                "{} hipótese(s) contestada(s) e {} esperando a confirmação da pessoa: peça a prova, não contorne.",
                contested.len(),
                waiting.len()
            ));
        }
        if !risks.unanswered().is_empty() {
            text.push(format!(
                "{} pergunta(s) sobre o terreno sem resposta: faça-as à família (projeto_estrutural acao riscos_terreno).",
                risks.unanswered().len()
            ));
        }
        if !risks.risky().is_empty() || risks.unknown_existing_base() {
            text.push(
                "O terreno ou a casa existente tem risco que o Trazo não calcula: as fundações ficam paradas até um profissional avaliar."
                    .to_owned(),
            );
        }
        text.push(help::HELP_SENTENCE.to_owned());
        Ok(ToolOutput::ok(text.join("\n")).with_structured(json!({
            "resumo_para_a_familia": headline,
            "nao_construir_assim": red,
            "no_limite": yellow,
            "hipoteses_contestadas": contested,
            "hipoteses_aguardando_pessoa": waiting,
            "perguntas_do_terreno_sem_resposta": risks.unanswered().iter().map(|r| r.question()).collect::<Vec<_>>(),
            "riscos_do_terreno": risks.risky().iter().map(|r| r.danger()).collect::<Vec<_>>(),
            "base_existente_desconhecida": risks.unknown_existing_base(),
            "onde_buscar_ajuda": help::help_lines(&project.protection.help_contacts),
        })))
    }
}
