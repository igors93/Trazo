//! MCP prompts (#166): the guided workflow an assistant follows to turn a
//! family's description into a checked house and clear building
//! instructions, without the family touching a drawing tool.

use serde_json::{Value, json};

/// The one prompt Trazo offers.
pub const DESIGN_HOUSE: &str = "projetar_casa";

const WORKFLOW: &str = "\
Você vai projetar uma casa para uma família que provavelmente não estudou engenharia e tem poucos recursos. \
Você faz o trabalho de projetar, comparar, revisar e organizar; o Trazo confere e contesta cada decisão; a família decide e recebe orientações claras para construir. \
Fale sempre em português simples e explique qualquer termo técnico.

1. Converse. Pergunte o tamanho do terreno, a cidade, quantos quartos e banheiros, o que é mais importante para a família, quanto pode gastar e se pretende subir outro andar depois. \
Chame projeto_estrutural com acao riscos_terreno (sem outros argumentos) para ver as perguntas sobre o terreno, faça cada uma à pessoa e registre só o que ela responder.

2. Projete. Use definir_terreno, criar_comodo (nomeie cada cômodo pelo uso: Sala, Quarto, Cozinha, Banheiro), inserir_abertura e inserir_objeto; executar_lote faz várias mudanças de uma vez. \
Se a família vai subir outro andar depois, registre com projeto_estrutural acao etapas (andares_futuros).

3. Compare. Monte pelo menos duas opções de planta em abas diferentes (ferramenta abas) e explique as diferenças em palavras simples: área, luz e ventilação, circulação e quantidade de material. Deixe a família escolher.

4. Estrutura. Na opção escolhida, chame projeto_estrutural com gerar_estrutura, depois gerar_fundacao_paredes e ajustar_sapatas.

5. Revise. Chame revisar_projeto. Para cada ponto vermelho, explique o risco em uma frase e corrija o que puder. \
Nunca mude uma hipótese de cálculo (solo, concreto, cargas, vento) sem um dado real informado pela pessoa: se o Trazo segurar uma hipótese para a pessoa confirmar, explique o motivo e peça a prova. Repita a revisão até não sobrar vermelho que você possa resolver.

6. Entregue. Chame guia_de_obra e explique as etapas na ordem da obra. Exporte o caderno da família (exportar_planta com formato caderno_familia_pdf) e o memorial (memorial_pdf). \
Diga com clareza o que precisa de um profissional e onde encontrar ajuda gratuita (assistência técnica gratuita pela Lei 11.888/2008, prefeitura, CAU ou CREA; Defesa Civil 199 em risco de desabamento).

Nunca minimize um vermelho, nunca invente uma prova e nunca diga que a casa está garantida: o Trazo faz pré-dimensionamento, não projeto final.";

/// `prompts/list`.
pub(super) fn prompts_list() -> Value {
    json!({
        "prompts": [{
            "name": DESIGN_HOUSE,
            "title": "Projetar uma casa com segurança",
            "description": "Roteiro para transformar a descrição de uma família numa casa desenhada, comparada, conferida pelo Trazo e num guia de obra em linguagem simples.",
            "arguments": [{
                "name": "descricao",
                "description": "O que a família quer: terreno, cômodos, orçamento, andares futuros, cidade e qualquer risco do terreno que já tenha contado.",
                "required": false,
            }],
        }],
    })
}

/// `prompts/get`.
pub(super) fn prompts_get(params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if name != DESIGN_HOUSE {
        return Err((-32602, format!("Prompt desconhecido: {name}")));
    }
    let description = params
        .get("arguments")
        .and_then(|a| a.get("descricao"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty());
    let text = match description {
        Some(described) => format!("{WORKFLOW}\n\nO que a família contou até agora: {described}"),
        None => WORKFLOW.to_owned(),
    };
    Ok(json!({
        "description": "Projetar uma casa com segurança, da conversa ao guia de obra.",
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": text },
        }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_prompt_is_listed_and_carries_the_family_description() {
        let listed = prompts_list();
        assert_eq!(listed["prompts"][0]["name"], DESIGN_HOUSE);
        let got = prompts_get(&json!({
            "name": DESIGN_HOUSE,
            "arguments": { "descricao": "terreno de 5 por 25 metros, dois quartos" }
        }))
        .unwrap();
        let text = got["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("revisar_projeto"));
        assert!(text.contains("5 por 25"));
        assert!(prompts_get(&json!({ "name": "outro" })).is_err());
    }
}
