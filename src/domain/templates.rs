//! Ready-to-use architectural project templates for floor plans, site plans
//! (implantação) and wet-area detailing, preconfigured with sheet size,
//! scale, hatches, fixtures, notes and ABNT NBR 6492 documentation styles.

use super::{
    doc_style::{DocStyle, DocStylePreset},
    fixtures::FixtureKind,
    geometry::Point,
    hatch::{HatchPattern, HatchStyle},
    model::{OpeningKind, Project},
    notes::{NoteKind, TechnicalSymbol},
    setbacks::Setbacks,
    sheet::{DrawingScale, Orientation, PaperSize, SheetLayout},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectTemplate {
    PlantaBaixaResidencial,
    ImplantacaoLote,
    DetalhamentoBanheiro,
    ApartamentoCompacto,
}

impl ProjectTemplate {
    pub const ALL: [ProjectTemplate; 4] = [
        ProjectTemplate::PlantaBaixaResidencial,
        ProjectTemplate::ImplantacaoLote,
        ProjectTemplate::DetalhamentoBanheiro,
        ProjectTemplate::ApartamentoCompacto,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::PlantaBaixaResidencial => "Planta baixa residencial (2 quartos)",
            Self::ImplantacaoLote => "Implantação no lote urbano (12 × 25 m)",
            Self::DetalhamentoBanheiro => "Detalhamento de banheiro (1:25)",
            Self::ApartamentoCompacto => "Apartamento compacto / Studio",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::PlantaBaixaResidencial | Self::ApartamentoCompacto => "Planta baixa",
            Self::ImplantacaoLote => "Implantação",
            Self::DetalhamentoBanheiro => "Detalhamento",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::PlantaBaixaResidencial => {
                "Prancha A3 1:50 (NBR 6492) com sala, cozinha, banheiro, 2 quartos, esquadrias, louças, hachuras e cotas de nível."
            }
            Self::ImplantacaoLote => {
                "Prancha A3 1:100 com lote 12 × 25 m, recuos urbanísticos configurados, edificação posicionada, norte e acessos."
            }
            Self::DetalhamentoBanheiro => {
                "Prancha A4 1:25 com banheiro equipado (lavatório, vaso, chuveiro), piso 30×30, caimento, cota de nível e chamada técnica."
            }
            Self::ApartamentoCompacto => {
                "Prancha A3 1:50 (Trazo Visual) com estar/dormitório integrado, cozinha americana e banheiro."
            }
        }
    }

    pub fn build(self) -> Project {
        match self {
            Self::PlantaBaixaResidencial => build_planta_baixa_residencial(),
            Self::ImplantacaoLote => build_implantacao_lote(),
            Self::DetalhamentoBanheiro => build_detalhamento_banheiro(),
            Self::ApartamentoCompacto => build_apartamento_compacto(),
        }
    }
}

fn find_wall_near(project: &Project, a: Point, b: Point) -> Option<u64> {
    project
        .walls
        .iter()
        .find(|w| {
            (w.start.near(a, 0.15) && w.end.near(b, 0.15))
                || (w.start.near(b, 0.15) && w.end.near(a, 0.15))
        })
        .map(|w| w.id)
}

fn build_planta_baixa_residencial() -> Project {
    let mut project = Project::default();
    project.name = "Residência Térrea — 2 Quartos".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_50;
    project.sheet.title_block.drawing_title = "Planta Baixa — Pavimento Térreo".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    // Rooms: 8.0 x 6.5 m footprint.
    let _ = project.add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.5), "Sala de Estar");
    let _ = project.add_room(Point::new(4.5, 0.0), Point::new(8.0, 3.5), "Cozinha");
    let _ = project.add_room(Point::new(0.0, 3.5), Point::new(3.5, 6.5), "Quarto 1");
    let _ = project.add_room(Point::new(3.5, 3.5), Point::new(5.5, 6.5), "Banheiro");
    let _ = project.add_room(Point::new(5.5, 3.5), Point::new(8.0, 6.5), "Quarto 2");

    // Material hatches for wet areas and social area.
    for room in &mut project.rooms {
        match room.name.as_str() {
            "Cozinha" | "Banheiro" => {
                room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
            }
            "Sala de Estar" => {
                room.hatch = Some(HatchStyle::for_pattern(HatchPattern::StoneGrid));
            }
            "Quarto 1" | "Quarto 2" => {
                room.hatch = Some(HatchStyle::for_pattern(HatchPattern::WoodPlank));
            }
            _ => {}
        }
    }

    // Doors and windows.
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 0.0), Point::new(4.5, 0.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.25);
        let _ = project.add_opening(w, OpeningKind::Window, 0.72);
    }
    if let Some(w) = find_wall_near(&project, Point::new(4.5, 0.0), Point::new(8.0, 0.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.55);
    }
    if let Some(w) = find_wall_near(&project, Point::new(4.5, 0.0), Point::new(4.5, 3.5)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 3.5), Point::new(3.5, 3.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.80);
    }
    if let Some(w) = find_wall_near(&project, Point::new(3.5, 3.5), Point::new(5.5, 3.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 3.5), Point::new(8.0, 3.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.25);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 6.5), Point::new(3.5, 6.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(3.5, 6.5), Point::new(5.5, 6.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 6.5), Point::new(8.0, 6.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }

    // Fixtures in kitchen and bathroom.
    let _ = project.add_fixture(FixtureKind::KitchenSink, Point::new(6.2, 0.45), 1.4, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(7.45, 0.45), 0.6, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Fridge, Point::new(7.45, 1.55), 0.7, 0.7, 3);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(4.1, 4.1), 0.6, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(4.95, 4.15), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(4.5, 5.85), 1.6, 0.9, 0);

    // Technical annotations & symbols.
    let _ = project.add_note(
        Point::new(1.1, -0.7),
        "ACESSO PRINCIPAL",
        NoteKind::Symbol(TechnicalSymbol::EntryArrow),
    );
    let _ = project.add_note(
        Point::new(2.2, 1.8),
        "",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.15 }),
    );
    let _ = project.add_note(
        Point::new(4.5, 5.0),
        "",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.13 }),
    );

    let _ = project.validate_and_repair();
    project
}

fn build_implantacao_lote() -> Project {
    let mut project = Project::default();
    project.name = "Implantação no Lote — 12 × 25 m".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_100;
    project.sheet.title_block.drawing_title = "Planta de Implantação e Cobertura".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    project.set_terrain(Point::new(0.0, 0.0), Point::new(12.0, 25.0));
    project.setbacks = Setbacks {
        front: 4.0,
        rear: 3.0,
        side: 1.5,
    };

    // Main residence volume inside buildable envelope.
    let _ = project.add_room(
        Point::new(1.5, 4.0),
        Point::new(10.5, 13.0),
        "Edificação Principal",
    );
    let _ = project.add_room(
        Point::new(1.5, 13.0),
        Point::new(6.0, 16.5),
        "Varanda / Garagem",
    );
    if let Some(room) = project.rooms.get_mut(0) {
        room.hatch = Some(HatchStyle::for_pattern(HatchPattern::Diagonal45));
    }
    if let Some(room) = project.rooms.get_mut(1) {
        room.hatch = Some(HatchStyle::for_pattern(HatchPattern::StoneGrid));
    }

    let _ = project.add_note(
        Point::new(6.0, -1.2),
        "RUA / ACESSO",
        NoteKind::Symbol(TechnicalSymbol::EntryArrow),
    );
    let _ = project.add_note(
        Point::new(10.5, 1.8),
        "",
        NoteKind::Symbol(TechnicalSymbol::NorthArrow),
    );
    let _ = project.add_note(Point::new(6.0, 2.0), "Recuo frontal 4,00 m", NoteKind::Note);
    let _ = project.add_note(
        Point::new(6.0, 8.5),
        "",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.30 }),
    );
    let _ = project.add_note(Point::new(6.0, 19.5), "Quintal permeável", NoteKind::Text);

    let _ = project.validate_and_repair();
    project
}

fn build_detalhamento_banheiro() -> Project {
    let mut project = Project::default();
    project.name = "Detalhamento — Banheiro Social".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A4, Orientation::Portrait);
    project.sheet.scale = DrawingScale::S1_25;
    project.sheet.title_block.drawing_title = "Detalhamento de Área Molhada — Banheiro".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    let _ = project.add_room(
        Point::new(0.0, 0.0),
        Point::new(2.6, 1.8),
        "Banheiro Social",
    );
    if let Some(room) = project.rooms.first_mut() {
        room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
    }

    if let Some(w) = find_wall_near(&project, Point::new(0.0, 0.0), Point::new(2.6, 0.0))
        && let Some(door_id) = project.add_opening(w, OpeningKind::Door, 0.22)
        && let Some(op) = project.openings.iter_mut().find(|o| o.id == door_id)
    {
        op.width = Some(0.70);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 1.8), Point::new(2.6, 1.8))
        && let Some(win_id) = project.add_opening(w, OpeningKind::Window, 0.72)
        && let Some(op) = project.openings.iter_mut().find(|o| o.id == win_id)
    {
        op.width = Some(0.80);
        op.sill = Some(1.60);
        op.height = Some(0.60);
    }

    let _ = project.add_fixture(
        FixtureKind::Washbasin,
        Point::new(0.55, 1.45),
        0.70,
        0.45,
        2,
    );
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(1.30, 1.38), 0.42, 0.65, 2);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(2.10, 0.90), 0.90, 1.50, 3);

    let _ = project.add_note(
        Point::new(2.10, 0.45),
        "",
        NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent: 1.5 }),
    );
    let _ = project.add_note(
        Point::new(1.0, 0.75),
        "",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: -0.02 }),
    );
    let _ = project.add_note(
        Point::new(3.0, 1.5),
        "Impermeabilização até h = 1,80 m no box",
        NoteKind::Callout {
            target: Point::new(2.1, 1.2),
        },
    );

    let _ = project.validate_and_repair();
    project
}

fn build_apartamento_compacto() -> Project {
    let mut project = Project::default();
    project.name = "Apartamento Studio — 36 m²".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_50;
    project.sheet.title_block.drawing_title = "Planta Baixa — Unidade Tipo".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::TrazoVisual);

    let _ = project.add_room(
        Point::new(0.0, 0.0),
        Point::new(4.5, 5.5),
        "Estar / Dormitório",
    );
    let _ = project.add_room(Point::new(4.5, 0.0), Point::new(7.0, 3.0), "Cozinha");
    let _ = project.add_room(Point::new(4.5, 3.0), Point::new(7.0, 5.5), "Banheiro");

    for room in &mut project.rooms {
        match room.name.as_str() {
            "Estar / Dormitório" => {
                room.hatch = Some(HatchStyle::for_pattern(HatchPattern::WoodPlank));
            }
            "Cozinha" | "Banheiro" => {
                room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
            }
            _ => {}
        }
    }

    if let Some(w) = find_wall_near(&project, Point::new(0.0, 0.0), Point::new(4.5, 0.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.25);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 5.5), Point::new(4.5, 5.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(4.5, 0.0), Point::new(4.5, 3.0)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(4.5, 3.0), Point::new(4.5, 5.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.45);
    }

    let _ = project.add_fixture(FixtureKind::KitchenSink, Point::new(5.8, 0.45), 1.4, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(6.55, 1.4), 0.6, 0.6, 3);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(5.4, 3.45), 0.6, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(6.3, 3.5), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(5.8, 4.9), 1.5, 0.9, 0);

    let _ = project.validate_and_repair();
    project
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_build_valid_architectural_projects() {
        for template in ProjectTemplate::ALL {
            let mut project = template.build();
            assert!(!project.rooms.is_empty(), "{template:?} should have rooms");
            assert!(!project.walls.is_empty(), "{template:?} should have walls");
            let warnings = project.validate_and_repair().unwrap();
            assert!(
                warnings.is_empty(),
                "{template:?} produced repair warnings: {warnings:?}"
            );
            assert!(project.sheet.validate().is_ok());
        }
    }

    #[test]
    fn site_plan_template_sets_terrain_and_setbacks() {
        let project = ProjectTemplate::ImplantacaoLote.build();
        let terrain = project.terrain.as_ref().expect("site plan has terrain");
        assert!((terrain.area() - 300.0).abs() < 1e-6);
        assert!((project.setbacks.front - 4.0).abs() < 1e-6);
        assert_eq!(project.sheet.scale, DrawingScale::S1_100);
    }

    #[test]
    fn detailing_template_uses_1_25_scale_and_wet_area_hatch() {
        let project = ProjectTemplate::DetalhamentoBanheiro.build();
        assert_eq!(project.sheet.scale, DrawingScale::S1_25);
        assert_eq!(project.sheet.size, PaperSize::A4);
        assert_eq!(
            project.rooms[0].hatch.map(|h| h.pattern),
            Some(HatchPattern::CeramicGrid)
        );
        assert!(!project.fixtures.is_empty());
        assert!(!project.notes.is_empty());
    }
}
