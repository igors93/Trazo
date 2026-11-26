//! The family booklet (#163): a few A4 pages to take to the hardware store,
//! the builder and the reviewer. The traffic light in one sentence, what
//! must stop, the shopping list, the building guide, when to stop and where
//! to get help, and the plan.

use std::path::Path;

use super::report::{INK, MUTED, Pages, pages_to_pdf};
use crate::domain::discipline::Discipline;
use crate::domain::model::Project;
use crate::domain::protection::glossary::family_summary;
use crate::domain::protection::guide::building_guide;
use crate::domain::protection::help::help_lines;
use crate::domain::verdict::{Light, safety_summary};

/// The booklet's pages as SVG: the A4 text pages, then the plan.
pub fn booklet_pages(project: &Project, date: &str) -> Vec<String> {
    let summary = safety_summary(project);
    let mut pages = Pages::new(&project.name);

    pages.heading("Caderno da família");
    pages.paragraph(&format!("{} · {date}", project.name), 3.5, MUTED);
    pages.paragraph(
        "O Trazo existe para que a falta de dinheiro e de conhecimento técnico não deixe uma família desamparada diante dos riscos de construir a própria casa. Este caderno resume o que o Trazo conferiu. Ele não substitui um profissional: leve-o junto com o memorial quando pedir ajuda.",
        3.0,
        INK,
    );

    pages.heading("1. Como está a sua casa");
    pages.paragraph(&family_summary(&summary), 3.4, INK);
    let red: Vec<_> = summary
        .verdicts
        .iter()
        .filter(|v| v.light() == Light::Red)
        .collect();
    if !red.is_empty() {
        pages.subheading("Não construa assim");
        for verdict in red.iter().take(15) {
            let why = verdict
                .checks
                .iter()
                .find(|c| c.light == Light::Red)
                .map_or(String::new(), |c| c.simple.clone());
            pages.paragraph(&format!("• {}: {why}", verdict.element), 3.0, INK);
        }
        if red.len() > 15 {
            pages.paragraph(
                &format!("… e mais {} ponto(s) no memorial.", red.len() - 15),
                3.0,
                MUTED,
            );
        }
    }

    pages.heading("2. Lista de compras");
    let takeoff = project.construction_takeoff();
    for masonry in &takeoff.masonry {
        pages.paragraph(
            &format!(
                "• {}: {:.0} blocos inteiros e {:.0} meios-blocos (com perda).",
                masonry.masonry, masonry.blocks_with_waste, masonry.half_blocks_with_waste
            ),
            3.0,
            INK,
        );
    }
    if takeoff.mortar_m3 > 0.0 {
        pages.paragraph(
            &format!(
                "• Argamassa de assentamento: cerca de {:.2} m³. Reboco: {:.0} m².",
                takeoff.mortar_m3, takeoff.plaster_m2
            ),
            3.0,
            INK,
        );
    }
    let reinforcement = project.cached_reinforcement_report();
    if reinforcement.total_mass_kg() > 0.0 {
        pages.paragraph(
            &format!(
                "• Aço CA-50: cerca de {:.0} kg com perda ({} barras de 12 m). As bitolas estão na tabela de aço do memorial.",
                reinforcement.total_mass_with_waste_kg(),
                reinforcement.total_bars_12m()
            ),
            3.0,
            INK,
        );
    }
    if takeoff.floor_with_waste_m2 > 0.0 {
        pages.paragraph(
            &format!("• Piso: {:.0} m² com perda.", takeoff.floor_with_waste_m2),
            3.0,
            INK,
        );
    }
    pages.paragraph(
        "Compre o material certo: trocar por um mais fraco para economizar põe a casa em risco.",
        3.0,
        MUTED,
    );

    pages.heading("3. Guia de obra");
    for stage in building_guide(project) {
        pages.subheading(&stage.title);
        for step in &stage.steps {
            pages.paragraph(&format!("• {step}"), 3.0, INK);
        }
        if let Some(list) = &stage.checklist {
            pages.paragraph(&format!("{}:", list.title), 3.0, MUTED);
            for item in &list.items {
                pages.paragraph(&format!("[  ] {item}"), 3.0, INK);
            }
        }
        for point in &stage.professional {
            pages.paragraph(&format!("! {point}"), 3.0, INK);
        }
    }

    pages.heading("4. Quando parar e procurar ajuda");
    for line in [
        "Algum ponto ficou vermelho e você não conseguiu resolver, ou o Trazo recusou um cálculo.",
        "O terreno fica em morro, perto de córrego, em aterro, ou brota água do chão.",
        "As casas vizinhas têm rachaduras, ou você vai escavar perto da fundação de um vizinho.",
        "Vai construir em cima de uma casa que já existe e não sabe como a fundação dela foi feita.",
        "A casa mostra rachaduras, portas que emperram, ferro aparecendo ou laje com barriga.",
    ] {
        pages.paragraph(&format!("• {line}"), 3.0, INK);
    }
    for line in help_lines(&project.protection.help_contacts) {
        pages.paragraph(&format!("• {line}"), 3.0, INK);
    }

    let mut out = pages.finish();
    if !project.walls.is_empty() {
        out.push(super::export::sheet::scaled_sheet_svg_for_view(
            project,
            &super::export::ExportView::for_discipline(project, Discipline::Architecture),
            None,
        ));
    }
    out
}

/// Writes the booklet as one PDF; returns the number of pages.
pub fn export_family_booklet(project: &Project, date: &str, path: &Path) -> Result<usize, String> {
    let pages = booklet_pages(project, date);
    let pdf = pages_to_pdf(&pages)?;
    std::fs::write(path, pdf).map_err(|e| format!("Não foi possível gravar o caderno: {e}"))?;
    Ok(pages.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn the_booklet_has_the_summary_the_guide_and_the_help() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        let pages = booklet_pages(&project, "26/11/2026");
        assert!(pages.len() >= 2);
        let text = pages.join("");
        assert!(text.contains("Caderno da fam"));
        assert!(text.contains("Guia de obra"));
        assert!(text.contains("11.888"));
    }
}
