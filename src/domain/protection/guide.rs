//! The building guide (#165): what the family takes to the site. The
//! checked house becomes clear, ordered instructions, stage by stage, each
//! with its materials, its checklist and the points where a professional
//! should look.

use super::checklists::{Checklist, pour_checklists};
use super::help::HELP_SENTENCE;
use super::site_risks::is_foundation_element;
use crate::domain::model::Project;
use crate::domain::verdict::{Discipline, Light, safety_summary};

/// One stage of the work.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideStage {
    pub title: String,
    /// What to do, in order.
    pub steps: Vec<String>,
    /// What to buy for this stage.
    pub materials: Vec<String>,
    /// What to check before pouring, when the stage has a pour.
    pub checklist: Option<Checklist>,
    /// Where a professional should look before going on.
    pub professional: Vec<String>,
}

/// The red results of the elements a stage covers.
fn red_results(project: &Project, matches: impl Fn(&str, Discipline) -> bool) -> Vec<String> {
    safety_summary(project)
        .verdicts
        .into_iter()
        .filter(|v| matches(&v.element, v.discipline) && v.light() == Light::Red)
        .map(|v| {
            let why = v
                .checks
                .iter()
                .find(|c| c.light == Light::Red)
                .map(|c| c.simple.clone())
                .unwrap_or_default();
            format!("{}: {why}", v.element)
        })
        .collect()
}

fn take(lists: &mut Vec<Checklist>, title_start: &str) -> Option<Checklist> {
    let index = lists.iter().position(|l| l.title.contains(title_start))?;
    Some(lists.remove(index))
}

/// The building guide of the project, in the order of the work. Stages
/// with nothing to build are left out.
pub fn building_guide(project: &Project) -> Vec<GuideStage> {
    let report = project.structure_report();
    let reinforcement = project.cached_reinforcement_report();
    let mut lists = pour_checklists(&reinforcement);
    let takeoff = project.construction_takeoff();
    let risks = &project.protection.site_risks;
    let mut guide = Vec::new();

    // 1. The site and the setting out.
    let mut professional: Vec<String> = risks
        .risky()
        .iter()
        .map(|r| format!("{} {}", r.question(), r.danger()))
        .collect();
    if risks.unknown_existing_base() {
        professional.push(super::site_risks::EXISTING_BASE_GUIDANCE.to_owned());
    }
    let mut steps = vec![
        "Confira as medidas do terreno e as divisas com os vizinhos antes de começar.".to_owned(),
        "Marque a casa no chão com gabarito (tábuas e linhas esticadas), conferindo os esquadros (o 3-4-5 dá o canto reto) e as distâncias das divisas.".to_owned(),
        "Limpe o terreno e tire a camada de terra com raízes e entulho onde vai a casa.".to_owned(),
    ];
    if !risks.unanswered().is_empty() {
        steps.push(
            "Responda às perguntas sobre o terreno na janela Segurança antes de cavar.".to_owned(),
        );
    }
    guide.push(GuideStage {
        title: "1. Terreno e marcação da casa".to_owned(),
        steps,
        materials: vec!["Tábuas, pontaletes, pregos e linha de náilon para o gabarito.".to_owned()],
        checklist: None,
        professional,
    });

    // 2. The foundations.
    if !report.footings.is_empty()
        || !report.pile_caps.is_empty()
        || !report.wall_foundations.is_empty()
    {
        let mut steps = Vec::new();
        if !report.footings.is_empty() {
            steps.push(format!(
                "Cave e faça {} sapata(s): {}.",
                report.footings.len(),
                report
                    .footings
                    .iter()
                    .map(|f| format!(
                        "{} de {:.0} × {:.0} cm",
                        f.label,
                        f.width_m * 100.0,
                        f.depth_m * 100.0
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !report.pile_caps.is_empty() {
            steps.push(format!(
                "Faça {} bloco(s) sobre broca, com as brocas no diâmetro e na profundidade do projeto.",
                report.pile_caps.len()
            ));
        }
        if !report.wall_foundations.is_empty() {
            let length: f64 = report.wall_foundations.iter().map(|w| w.length_m).sum();
            steps.push(format!(
                "Faça os baldrames e sapatas corridas sob as paredes do térreo ({length:.1} m no total), ligando as sapatas."
            ));
        }
        steps.push(
            "Impermeabilize o topo dos baldrames antes de subir as paredes, para a umidade do chão não subir."
                .to_owned(),
        );
        let mut professional = red_results(project, |e, d| {
            d == Discipline::Structure && (is_foundation_element(e) || e == "Riscos do terreno")
        });
        if project.structure.soil_survey.is_none() {
            professional.push(
                "Não há sondagem do terreno: as fundações foram calculadas com um solo fraco, do lado seguro. Uma sondagem pode mostrar que o solo é ainda pior; peça uma se puder."
                    .to_owned(),
            );
        }
        guide.push(GuideStage {
            title: "2. Fundação".to_owned(),
            steps,
            materials: vec![
                "Cimento, areia, brita e ferro conforme a tabela de aço do memorial.".to_owned(),
                "Espaçadores (pastilhas) para o cobrimento, arame recozido e lona plástica."
                    .to_owned(),
            ],
            checklist: take(&mut lists, "sapatas"),
            professional,
        });
    }

    // 3. Columns and beams.
    if !report.columns.is_empty() {
        let mut steps = vec![format!(
            "Levante {} pilar(es) a partir das esperas das sapatas: {}.",
            report.columns.len(),
            report
                .columns
                .iter()
                .map(|c| format!(
                    "{} de {:.0} × {:.0} cm",
                    c.label,
                    c.width_m * 100.0,
                    c.depth_m * 100.0
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )];
        if !report.beams.is_empty() {
            steps.push(format!(
                "Faça {} viga(s) ligando os pilares, escoradas até o concreto ficar forte.",
                report.beams.len()
            ));
        }
        let mut checklist = take(&mut lists, "pilares");
        if let Some(beams) = take(&mut lists, "vigas") {
            match &mut checklist {
                Some(list) => {
                    list.title.push_str(" e as vigas");
                    list.elements.extend(beams.elements);
                    list.items.extend(beams.items.into_iter().filter(|i| {
                        i.contains("escora") || i.contains("ancoragem") || i.contains("de baixo")
                    }));
                }
                None => checklist = Some(beams),
            }
        }
        guide.push(GuideStage {
            title: "3. Pilares e vigas".to_owned(),
            steps,
            materials: vec![
                "Fôrmas de madeira, escoras, ferro, cimento, areia e brita conforme o memorial."
                    .to_owned(),
            ],
            checklist,
            professional: red_results(project, |e, d| {
                d == Discipline::Structure
                    && (e.starts_with("Pilar ")
                        || (e.starts_with("Viga ")
                            && !e.starts_with("Viga baldrame")
                            && !e.starts_with("Viga alavanca")))
            }),
        });
    }

    // 4. Walls.
    if !takeoff.masonry.is_empty() {
        let mut materials: Vec<String> = takeoff
            .masonry
            .iter()
            .map(|m| {
                format!(
                    "{}: {:.0} blocos inteiros e {:.0} meios-blocos, já com perda.",
                    m.masonry, m.blocks_with_waste, m.half_blocks_with_waste
                )
            })
            .collect();
        materials.push(format!(
            "Argamassa de assentamento: cerca de {:.2} m³; reboco: {:.0} m².",
            takeoff.mortar_m3, takeoff.plaster_m2
        ));
        guide.push(GuideStage {
            title: "4. Paredes".to_owned(),
            steps: vec![
                "Suba as paredes em fiadas niveladas e no prumo, amarradas aos pilares (ferro-cabelo a cada duas ou três fiadas).".to_owned(),
                "Faça verga em cima de toda porta e janela e contraverga embaixo de toda janela, entrando pelo menos 30 cm de cada lado.".to_owned(),
                "Deixe os eletrodutos e as caixas de luz no lugar antes de rebocar.".to_owned(),
            ],
            materials,
            checklist: None,
            professional: red_results(project, |e, d| {
                d == Discipline::Structure && (e.starts_with("Alvenaria ") || e.starts_with("Verga") || e.starts_with("Contraverga"))
            }),
        });
    }

    // 5. The slab.
    if !report.slabs.is_empty() {
        let tanks = project
            .fixtures
            .iter()
            .filter(|f| f.kind == crate::domain::fixtures::FixtureKind::WaterTank)
            .count();
        let mut steps = vec![format!(
            "Monte {} laje(s) com as escoras alinhadas e apoiadas em chão firme.",
            report.slabs.len()
        )];
        if tanks > 0 {
            steps.push("Coloque a caixa-d'água só no lugar calculado no projeto.".to_owned());
        }
        guide.push(GuideStage {
            title: "5. Laje".to_owned(),
            steps,
            materials: vec![
                "Vigotas e enchimentos (laje treliçada) ou fôrma (laje maciça), ferro, concreto e escoras.".to_owned(),
            ],
            checklist: take(&mut lists, "lajes"),
            professional: red_results(project, |e, d| {
                d == Discipline::Structure && (e.starts_with("Laje ") || e.starts_with("Carga ") || e.starts_with("Balanço") || e.starts_with("Escada"))
            }),
        });
    }

    // 6. The roof.
    if !report.roofs.is_empty() {
        guide.push(GuideStage {
            title: "6. Telhado".to_owned(),
            steps: vec![
                "Monte a estrutura de madeira com as peças e os espaçamentos do projeto, presa às paredes ou vigas com as ancoragens indicadas contra o vento.".to_owned(),
                "Use madeira tratada para uso externo e telhas na inclinação mínima do fabricante.".to_owned(),
            ],
            materials: vec!["Madeiramento, telhas, pregos e ancoragens conforme o memorial.".to_owned()],
            checklist: None,
            professional: red_results(project, |e, d| d == Discipline::Structure && e.starts_with("Telhado")),
        });
    }

    // 7. The electrical installation.
    let schedule = project.load_schedule();
    if !schedule.circuits.is_empty() {
        let rcd = schedule.circuits.iter().filter(|c| c.needs_rcd).count();
        guide.push(GuideStage {
            title: "7. Instalação elétrica".to_owned(),
            steps: vec![
                format!(
                    "Passe os fios de {} circuito(s) nos eletrodutos, com a bitola e o disjuntor de cada um no quadro de cargas.",
                    schedule.circuits.len()
                ),
                format!(
                    "Instale o DR nos {rcd} circuito(s) de áreas molhadas e externas, e ligue o fio terra em todas as tomadas."
                ),
                "Nunca use fio mais fino que o do projeto: fio fino esquenta e pode pegar fogo.".to_owned(),
            ],
            materials: vec!["Fios, eletrodutos, caixas, disjuntores, DR e quadro conforme o quadro de cargas.".to_owned()],
            checklist: None,
            professional: red_results(project, |_, d| d == Discipline::Electrical),
        });
    }

    for stage in &mut guide {
        if !stage.professional.is_empty() {
            stage.professional.push(HELP_SENTENCE.to_owned());
        }
    }
    guide
}

/// The guide as plain text, for MCP and for printing.
pub fn guide_text(guide: &[GuideStage]) -> String {
    let mut out = String::new();
    for stage in guide {
        out.push_str(&stage.title);
        out.push('\n');
        for step in &stage.steps {
            out.push_str(&format!("  • {step}\n"));
        }
        if !stage.materials.is_empty() {
            out.push_str("  Materiais:\n");
            for material in &stage.materials {
                out.push_str(&format!("    - {material}\n"));
            }
        }
        if let Some(list) = &stage.checklist {
            out.push_str(&format!("  {}:\n", list.title));
            for element in &list.elements {
                out.push_str(&format!("    • {element}\n"));
            }
            for item in &list.items {
                out.push_str(&format!("    [ ] {item}\n"));
            }
        }
        if !stage.professional.is_empty() {
            out.push_str("  Onde um profissional deve olhar:\n");
            for point in &stage.professional {
                out.push_str(&format!("    ! {point}\n"));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn a_house_gets_ordered_stages_with_the_foundation_first() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        let guide = building_guide(&project);
        assert!(guide.len() >= 3);
        assert!(guide[0].title.starts_with("1."));
        assert!(guide.iter().any(|s| s.title.contains("Fundação")));
        let text = guide_text(&guide);
        assert!(text.contains("Materiais"));
    }
}
