//! Before-the-pour checklists (#162). Many failures happen on site, at the
//! pour, after a correct plan: the wrong bars, no spacers, a crooked form,
//! shoring removed too early. Each list comes from the bar schedule and is
//! written to be printed and read beside the form.

use crate::domain::reinforcement::ReinforcementReport;

/// One printable list for one kind of pour.
#[derive(Clone, Debug, PartialEq)]
pub struct Checklist {
    pub title: String,
    /// One line per element: what bars it takes.
    pub elements: Vec<String>,
    /// What to check before pouring, in plain words.
    pub items: Vec<String>,
}

const CURING: &str = "Depois de concretar, molhe o concreto (ou cubra com lona ou saco molhado) por pelo menos 7 dias.";
const NO_EXTRA_WATER: &str = "Não ponha água a mais no concreto para ficar mole: ele fica fraco. Use o traço indicado e adense bem (vibrador ou soquete), sem deixar buracos.";
const STEEL_CLEAN: &str = "Ferro sem ferrugem solta, sem barro e sem óleo; amarrado com arame recozido em todos os cruzamentos.";

/// The checklists of the project's pours, in building order. Empty lists
/// (no element of that kind) are left out.
pub fn pour_checklists(reinforcement: &ReinforcementReport) -> Vec<Checklist> {
    let mut lists = Vec::new();
    if !reinforcement.footings.is_empty() {
        lists.push(Checklist {
            title: "Antes de concretar as sapatas".to_owned(),
            elements: reinforcement
                .footings
                .iter()
                .map(|f| {
                    format!(
                        "{}: {:.0} × {:.0} × {:.0} cm; malha {} numa direção e {} na outra.",
                        f.label,
                        f.width_m * 100.0,
                        f.depth_m * 100.0,
                        f.height_m * 100.0,
                        f.bars_x.notation(),
                        f.bars_y.notation()
                    )
                })
                .collect(),
            items: vec![
                "A cava tem o tamanho e a profundidade do projeto, o fundo está firme, limpo e sem água ou lama.".to_owned(),
                "Há uma camada de concreto magro (lastro) de uns 5 cm no fundo antes do ferro.".to_owned(),
                "A malha fica a pelo menos 5 cm do chão, apoiada em espaçadores (pastilhas de concreto), nunca em pedras ou tijolos.".to_owned(),
                "Os ferros de espera do pilar estão no lugar, no prumo e com o comprimento do projeto.".to_owned(),
                STEEL_CLEAN.to_owned(),
                NO_EXTRA_WATER.to_owned(),
                CURING.to_owned(),
            ],
        });
    }
    if !reinforcement.columns.is_empty() {
        lists.push(Checklist {
            title: "Antes de concretar os pilares".to_owned(),
            elements: reinforcement
                .columns
                .iter()
                .map(|c| {
                    format!(
                        "{}: {:.0} × {:.0} cm; {} na vertical e estribos {}; emenda (transpasse) de {:.0} cm.",
                        c.label,
                        c.width_m * 100.0,
                        c.depth_m * 100.0,
                        c.longitudinal.notation(),
                        c.ties.notation(),
                        c.lap_length_m * 100.0
                    )
                })
                .collect(),
            items: vec![
                "A fôrma está no prumo nas duas direções, travada e sem frestas.".to_owned(),
                "Há espaçadores de 2,5 a 3 cm entre o ferro e a fôrma, em todas as faces.".to_owned(),
                "Os estribos têm o espaçamento do projeto, inclusive perto do topo e da base.".to_owned(),
                "A emenda dos ferros tem o comprimento indicado; não emende todos no mesmo ponto sem o profissional dizer que pode.".to_owned(),
                STEEL_CLEAN.to_owned(),
                "Concrete em camadas e adense cada uma; em pilares altos, use uma janela na fôrma para não jogar o concreto de muito alto.".to_owned(),
                NO_EXTRA_WATER.to_owned(),
                CURING.to_owned(),
            ],
        });
    }
    if !reinforcement.beams.is_empty() {
        lists.push(Checklist {
            title: "Antes de concretar as vigas".to_owned(),
            elements: reinforcement
                .beams
                .iter()
                .map(|b| {
                    format!(
                        "{} (vão {}): {:.0} × {:.0} cm; embaixo {}, em cima {}, estribos {}.",
                        b.label,
                        b.span_index + 1,
                        b.width_m * 100.0,
                        b.height_m * 100.0,
                        b.bottom.notation(),
                        b.top.left().notation(),
                        b.stirrups.notation()
                    )
                })
                .collect(),
            items: vec![
                "O fundo da fôrma está nivelado, escorado a cada 80 cm a 1 m, com escoras apoiadas em chão firme (tábua embaixo) e travadas.".to_owned(),
                "Os ferros de baixo e de cima estão do lado certo: os de baixo seguram o meio do vão, os de cima seguram em cima dos pilares.".to_owned(),
                "Há espaçadores de 2,5 a 3 cm embaixo e dos lados.".to_owned(),
                "Os ferros entram nos pilares com a ancoragem (dobra) do projeto.".to_owned(),
                STEEL_CLEAN.to_owned(),
                NO_EXTRA_WATER.to_owned(),
                CURING.to_owned(),
                "Só tire as escoras depois de pelo menos 21 dias, ou do prazo que o profissional indicar.".to_owned(),
            ],
        });
    }
    if !reinforcement.slabs.is_empty() {
        lists.push(Checklist {
            title: "Antes de concretar as lajes".to_owned(),
            elements: reinforcement
                .slabs
                .iter()
                .map(|s| {
                    format!(
                        "{} ({}): {} principal e {} de distribuição.",
                        s.label,
                        s.room,
                        s.main.notation(),
                        s.distribution.notation()
                    )
                })
                .collect(),
            items: vec![
                "As escoras estão alinhadas, apoiadas em chão firme e com a distância que o fabricante da laje indica (laje treliçada).".to_owned(),
                "A laje tem a contraflecha (leve curva para cima no meio) que o fabricante indica.".to_owned(),
                "A malha fica a uns 2 cm do fundo, sobre espaçadores; ninguém pisa direto nas vigotas ou na malha (use tábuas).".to_owned(),
                "Eletrodutos e caixas de luz estão presos antes de concretar.".to_owned(),
                "A caixa-d'água só vai sobre a laje que foi calculada para ela.".to_owned(),
                NO_EXTRA_WATER.to_owned(),
                CURING.to_owned(),
                "Só tire as escoras depois de pelo menos 21 dias, ou do prazo que o profissional ou o fabricante indicar.".to_owned(),
            ],
        });
    }
    lists
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;
    use crate::domain::model::Project;

    #[test]
    fn a_house_gets_a_list_per_pour_with_its_bars() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        let report = project.structure_report();
        let reinforcement =
            crate::domain::reinforcement::build_reinforcement_report(&project, &report);
        let lists = pour_checklists(&reinforcement);
        assert!(!lists.is_empty());
        assert!(lists.iter().all(|l| !l.items.is_empty()));
        assert!(
            lists
                .iter()
                .all(|l| l.items.iter().any(|i| i.contains("7 dias")))
        );
    }
}
