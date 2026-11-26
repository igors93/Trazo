//! Plain language (#161): the technical words Trazo's results use, each
//! explained for someone who never studied engineering, and a one-sentence
//! summary of the traffic light for the family.

use crate::domain::verdict::{Light, SafetySummary};

/// One technical term and what it means in plain words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Term {
    /// The word as it appears in the results (matched without accents and
    /// case).
    pub word: &'static str,
    pub plain: &'static str,
}

pub const TERMS: [Term; 36] = [
    Term {
        word: "pilar",
        plain: "Coluna de concreto armado que leva o peso das vigas até a fundação.",
    },
    Term {
        word: "viga",
        plain: "Peça de concreto armado deitada que apoia a laje e as paredes e leva o peso até os pilares.",
    },
    Term {
        word: "laje",
        plain: "O piso ou o teto de concreto. Também carrega peso: pessoas, móveis, caixa-d'água.",
    },
    Term {
        word: "sapata corrida",
        plain: "Fundação contínua, como uma faixa de concreto embaixo de uma parede.",
    },
    Term {
        word: "sapata",
        plain: "Base de concreto enterrada embaixo de um pilar, que espalha o peso no chão.",
    },
    Term {
        word: "baldrame",
        plain: "Viga enterrada embaixo das paredes do térreo, ligando os pilares.",
    },
    Term {
        word: "viga alavanca",
        plain: "Viga que equilibra a sapata de um pilar encostado na divisa, puxando-a para dentro.",
    },
    Term {
        word: "broca",
        plain: "Estaca curta cavada à mão (trado) e cheia de concreto, para levar o peso mais fundo.",
    },
    Term {
        word: "bloco sobre broca",
        plain: "Bloco de concreto em cima das brocas, onde nasce o pilar.",
    },
    Term {
        word: "fundacao",
        plain: "A parte enterrada da casa que passa todo o peso para o chão.",
    },
    Term {
        word: "fck",
        plain: "A resistência do concreto. C20 quer dizer 20 MPa; concreto feito na obra costuma ser mais fraco que o usinado.",
    },
    Term {
        word: "kpa",
        plain: "Unidade de pressão. Na tensão do solo, diz quanto peso cada metro quadrado do chão aguenta (100 kPa é cerca de 10 toneladas por m²).",
    },
    Term {
        word: "kn",
        plain: "Unidade de força. 10 kN é mais ou menos o peso de 1 tonelada.",
    },
    Term {
        word: "tensao do solo",
        plain: "Quanto peso o chão aguenta em cada metro quadrado sem afundar.",
    },
    Term {
        word: "sondagem",
        plain: "Ensaio feito no terreno (SPT) que mede a firmeza do chão em cada metro de profundidade.",
    },
    Term {
        word: "spt",
        plain: "O ensaio de sondagem mais comum: conta quantos golpes são precisos para o amostrador afundar no chão.",
    },
    Term {
        word: "sobrecarga",
        plain: "O peso das pessoas, móveis e objetos sobre a laje.",
    },
    Term {
        word: "flecha",
        plain: "O quanto a viga ou a laje enverga (faz barriga) com o peso.",
    },
    Term {
        word: "esbeltez",
        plain: "O quanto o pilar é fino para a altura dele. Pilar muito esbelto pode dobrar.",
    },
    Term {
        word: "puncao",
        plain: "Quando o pilar ou um peso concentrado fura a laje ou a sapata, como um dedo furando papel.",
    },
    Term {
        word: "armadura",
        plain: "O ferro (aço) dentro do concreto, que segura a tração.",
    },
    Term {
        word: "estribo",
        plain: "Os ferros dobrados em volta das barras da viga ou do pilar, que seguram o corte e amarram tudo.",
    },
    Term {
        word: "cobrimento",
        plain: "A camada de concreto entre o ferro e o lado de fora da peça, que protege o ferro da ferrugem.",
    },
    Term {
        word: "verga",
        plain: "Peça de concreto armado em cima da porta ou janela, que segura a parede de cima.",
    },
    Term {
        word: "contraverga",
        plain: "Peça de concreto armado embaixo da janela, que evita trincas nos cantos.",
    },
    Term {
        word: "cinta",
        plain: "Viga baixa dentro da parede que amarra os tijolos e ajuda contra o vento.",
    },
    Term {
        word: "recalque",
        plain: "Quando a fundação afunda com o tempo. Recalques diferentes trincam a casa.",
    },
    Term {
        word: "portico",
        plain: "Conjunto de vigas e pilares ligados, que trabalham juntos contra o peso e o vento.",
    },
    Term {
        word: "contraventamento",
        plain: "Peças que travam a estrutura contra o vento, como um X entre pilares.",
    },
    Term {
        word: "elu",
        plain: "Estado limite último: a conta que garante que a peça não quebra, com margem de segurança.",
    },
    Term {
        word: "els",
        plain: "Estado limite de serviço: a conta que garante que a peça não enverga nem trinca demais no uso.",
    },
    Term {
        word: "escoramento",
        plain: "As escoras de madeira ou metal que seguram a fôrma até o concreto ficar forte.",
    },
    Term {
        word: "eletroduto",
        plain: "O cano por onde passam os fios da instalação elétrica.",
    },
    Term {
        word: "disjuntor",
        plain: "A chave do quadro que desliga sozinha quando o circuito puxa corrente demais, evitando incêndio.",
    },
    Term {
        word: "dr",
        plain: "Dispositivo que desliga a energia quando há choque elétrico (obrigatório em banheiro, cozinha e área externa).",
    },
    Term {
        word: "queda de tensao",
        plain: "Perda de energia em fios longos ou finos demais: o chuveiro esquenta menos e o fio esquenta mais.",
    },
];

/// Lowercase without accents, for matching.
fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'Á' | 'À' | 'Â' | 'Ã' => 'a',
            'é' | 'ê' | 'É' | 'Ê' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'o',
            'ú' | 'Ú' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// The terms a text uses, longest match first (so "sapata corrida" wins
/// over "sapata"), each once.
pub fn terms_in(text: &str) -> Vec<Term> {
    let folded = fold(text);
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut found: Vec<Term> = Vec::new();
    for term in TERMS {
        let parts: Vec<&str> = term.word.split(' ').collect();
        // Short words must match exactly ("dr", "kn"); longer ones also
        // match their plurals ("pilares", "vigas").
        let hit = words.windows(parts.len()).any(|window| {
            window.iter().zip(&parts).all(|(w, p)| {
                if p.len() <= 3 {
                    w == p
                } else {
                    w.starts_with(p)
                }
            })
        });
        let covered = found
            .iter()
            .any(|f| f.word.len() > term.word.len() && f.word.starts_with(term.word));
        if hit && !covered {
            found.push(term);
        }
    }
    found
}

/// The explanation of every term in `text`, one per line.
pub fn explain(text: &str) -> String {
    terms_in(text)
        .iter()
        .map(|t| format!("{}: {}", t.word, t.plain))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One sentence on the whole traffic light, for the family.
pub fn family_summary(summary: &SafetySummary) -> String {
    match summary.worst() {
        None => "Ainda não há nada para conferir: desenhe a casa e gere a estrutura.".to_owned(),
        Some(Light::Red) => format!(
            "Pare: {} ponto(s) vermelho(s) não podem ser construídos assim. Corrija o que cada um pede ou procure ajuda antes de começar a obra.",
            summary.red
        ),
        Some(Light::Yellow) => format!(
            "Atenção: nada está reprovado, mas {} ponto(s) estão no limite ou dependem de uma resposta sua. Resolva-os antes de construir.",
            summary.yellow
        ),
        Some(Light::Green) => "Tudo o que o Trazo confere passou com folga. Isso não garante a casa: leve o memorial a um profissional antes de construir.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_found_without_accents_and_the_longest_wins() {
        let found = terms_in("A Sapata corrida SC2 tem punção e o fck é baixo.");
        let words: Vec<&str> = found.iter().map(|t| t.word).collect();
        assert!(words.contains(&"sapata corrida"));
        assert!(!words.contains(&"sapata"));
        assert!(words.contains(&"puncao"));
        assert!(words.contains(&"fck"));
    }

    #[test]
    fn the_family_summary_says_stop_when_something_is_red() {
        let summary = SafetySummary {
            red: 2,
            verdicts: vec![crate::domain::verdict::Verdict {
                discipline: crate::domain::verdict::Discipline::Structure,
                element: "Viga V1".to_owned(),
                checks: Vec::new(),
            }],
            ..SafetySummary::default()
        };
        assert!(family_summary(&summary).starts_with("Pare"));
    }
}
