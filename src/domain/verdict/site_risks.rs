//! Site risks in the traffic light (#156): unanswered questions are yellow,
//! and a risky answer stops the foundations, which show the reason and
//! where to get help instead of numbers.

use super::{Check, Discipline, Verdict};
use crate::domain::model::Project;
use crate::domain::protection::help::HELP_SENTENCE;
use crate::domain::protection::site_risks::{SiteRisk, is_foundation_element};

/// The element name of the site-risk verdict.
pub const SITE_RISKS_ELEMENT: &str = "Riscos do terreno";

pub(super) fn site_risks(project: &Project, verdicts: &mut Vec<Verdict>) {
    if verdicts.is_empty() {
        return;
    }
    let risks = &project.protection.site_risks;
    let risky = risks.risky();
    let unanswered = risks.unanswered();
    let mut checks = Vec::new();
    if !risky.is_empty() {
        let names: Vec<&str> = risky.iter().map(|r| r.question()).collect();
        let stop = format!(
            "Não dimensione esta fundação: o terreno tem risco que o Trazo não sabe calcular ({}). Pare e procure um profissional. {HELP_SENTENCE}",
            names.join(" ")
        );
        for verdict in verdicts.iter_mut() {
            if is_foundation_element(&verdict.element) {
                verdict.checks = vec![Check::fail(
                    "Riscos do terreno",
                    stop.clone(),
                    "Regra 9 do contrato de segurança: o que o Trazo não vê é perguntado, nunca suposto (#156).".to_owned(),
                )];
            }
        }
        for risk in risky {
            checks.push(Check::fail(
                risk.question(),
                format!("{} Pare e procure ajuda. {HELP_SENTENCE}", risk.danger()),
                format!("Resposta: sim ({}).", risk.key()),
            ));
        }
    }
    if !unanswered.is_empty() {
        let assistant_said_no = unanswered
            .iter()
            .any(|&r| risks.answer(r).is_some_and(|a| !a.yes));
        checks.push(Check::new(
            "Perguntas sem resposta",
            0.95,
            format!(
                "Responda às perguntas sobre o terreno na janela Segurança ({} sem resposta{}): o Trazo não vê encostas, córregos, aterros nem trincas nos vizinhos.",
                unanswered.len(),
                if assistant_said_no {
                    "; a IA respondeu \"não\" por você em alguma, confirme"
                } else {
                    ""
                }
            ),
            unanswered
                .iter()
                .map(|r| r.question())
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }
    if checks.is_empty() {
        checks.push(Check::new(
            "Perguntas respondidas",
            0.5,
            "Você respondeu que o terreno não tem os riscos que o Trazo pergunta.".to_owned(),
            SiteRisk::ALL
                .iter()
                .map(|r| format!("{} Não.", r.question()))
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }
    verdicts.push(Verdict {
        discipline: Discipline::Structure,
        element: SITE_RISKS_ELEMENT.to_owned(),
        checks,
    });
}
