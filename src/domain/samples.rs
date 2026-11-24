//! Canonical sample projects stored in `examples/` (#81).
//!
//! Each sample is shipped as a standalone `.json` file in `examples/` for docs,
//! screenshots, tests and double-click opening, and is also embedded so users
//! can load it directly from *Arquivo → Novo a partir de modelo...*.

use super::{
    circulation::StairShape,
    doc_style::{DocStyle, DocStylePreset},
    drawing::Shape,
    fixtures::FixtureKind,
    geometry::Point,
    hatch::{HatchPattern, HatchStyle},
    masonry::Masonry,
    model::{CURRENT_PROJECT_VERSION, OpeningKind, Project},
    notes::{NoteKind, TechnicalSymbol},
    roof::{Roof, RoofSide, TileKind, TimberClass, TimberSpecies},
    setbacks::Setbacks,
    sheet::{DrawingScale, Orientation, PaperSize, SheetLayout},
    soil::{SoilKind, SoilSurvey, SptReading},
    wind::WindSettings,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleProject {
    CasaTerrea,
    Sobrado,
    Apartamento,
    AreaGourmet,
}

impl SampleProject {
    pub const ALL: [SampleProject; 4] = [
        SampleProject::CasaTerrea,
        SampleProject::Sobrado,
        SampleProject::Apartamento,
        SampleProject::AreaGourmet,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::CasaTerrea => "casa-terrea.json",
            Self::Sobrado => "sobrado.json",
            Self::Apartamento => "apartamento.json",
            Self::AreaGourmet => "area-gourmet.json",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::CasaTerrea => "Casa térrea (2 quartos + suíte)",
            Self::Sobrado => "Sobrado (2 pavimentos, 3 quartos)",
            Self::Apartamento => "Apartamento (2 quartos com varanda)",
            Self::AreaGourmet => "Área gourmet semiaberta (5 × 14,75 m, ref. estrutural)",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::CasaTerrea => {
                "Lote 10 × 20 m com recuos, sala integrada, cozinha, área de serviço, suíte, dormitório, banheiro social e mobiliário completo."
            }
            Self::Sobrado => {
                "Residência em dois pavimentos com escada, área social no térreo e área íntima com 3 quartos e sacada no pavimento superior."
            }
            Self::Apartamento => {
                "Apartamento residencial com varanda, estar/jantar, cozinha e lavanderia, suíte, dormitório e banheiro social em prancha A3 1:50."
            }
            Self::AreaGourmet => {
                "Pavilhão gourmet 5 × 14,75 m na divisa esquerda com parede alta de 4,30 m, oitões inclinados, mourões tratados voltados para a piscina, telhado aparente meia-água (34 %), baldrames, vigas alavanca, sondagem SPT e verificação completa de vento."
            }
        }
    }

    /// Loads the sample from its embedded `examples/*.json` file (falling back
    /// to the programmatic builder only if needed).
    pub fn load_embedded(self) -> Project {
        let json = match self {
            Self::CasaTerrea => include_str!("../../examples/casa-terrea.json"),
            Self::Sobrado => include_str!("../../examples/sobrado.json"),
            Self::Apartamento => include_str!("../../examples/apartamento.json"),
            Self::AreaGourmet => include_str!("../../examples/area-gourmet.json"),
        };
        serde_json::from_str::<Project>(json).unwrap_or_else(|_| self.build())
    }

    /// Programmatic builder used to produce and validate the canonical sample files.
    pub fn build(self) -> Project {
        let mut project = match self {
            Self::CasaTerrea => build_casa_terrea(),
            Self::Sobrado => build_sobrado(),
            Self::Apartamento => build_apartamento(),
            Self::AreaGourmet => build_area_gourmet(),
        };
        project.version = CURRENT_PROJECT_VERSION;
        project.normalize_precision();
        project
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

fn apply_residential_hatches(project: &mut Project) {
    for room in &mut project.rooms {
        let lower = room.name.to_lowercase();
        if lower.contains("cozinha")
            || lower.contains("banheiro")
            || lower.contains("lavabo")
            || lower.contains("serviço")
            || lower.contains("varanda")
            || lower.contains("sacada")
        {
            room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
        } else if lower.contains("sala") || lower.contains("hall") || lower.contains("circulação")
        {
            room.hatch = Some(HatchStyle::for_pattern(HatchPattern::StoneGrid));
        } else if lower.contains("quarto") || lower.contains("suíte") {
            room.hatch = Some(HatchStyle::for_pattern(HatchPattern::WoodPlank));
        }
    }
}

fn build_casa_terrea() -> Project {
    let mut project = Project::default();
    project.name = "Casa Térrea — Exemplo Trazo".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_50;
    project.sheet.title_block.drawing_title = "Planta Baixa — Casa Térrea".to_owned();
    project.sheet.title_block.client = "Projeto de Exemplo".to_owned();
    project.sheet.title_block.author = "Trazo CAD".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    // 10 x 20 m urban lot with setbacks.
    let _ = project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    project.setbacks = Setbacks {
        front: 4.0,
        rear: 3.0,
        side: 1.5,
    };

    // Footprint positioned inside buildable area: x in [1.5, 8.5], y in [4.0, 15.0].
    let _ = project.add_room(
        Point::new(1.5, 4.0),
        Point::new(5.5, 8.5),
        "Sala de Estar e Jantar",
    );
    let _ = project.add_room(Point::new(5.5, 4.0), Point::new(8.5, 8.0), "Cozinha");
    let _ = project.add_room(
        Point::new(5.5, 8.0),
        Point::new(8.5, 10.0),
        "Área de Serviço",
    );
    let _ = project.add_room(Point::new(1.5, 8.5), Point::new(5.0, 12.0), "Suíte");
    let _ = project.add_room(
        Point::new(5.0, 10.0),
        Point::new(8.5, 12.0),
        "Banheiro Suíte",
    );
    let _ = project.add_room(Point::new(1.5, 12.0), Point::new(5.0, 15.0), "Quarto 2");
    let _ = project.add_room(
        Point::new(5.0, 12.0),
        Point::new(8.5, 15.0),
        "Banheiro Social",
    );
    apply_residential_hatches(&mut project);

    // Doors and windows.
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 4.0), Point::new(5.5, 4.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.28);
        let _ = project.add_opening(w, OpeningKind::Window, 0.72);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.0), Point::new(8.5, 4.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.0), Point::new(5.5, 8.0)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 8.0), Point::new(8.5, 8.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.45);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 8.0), Point::new(8.5, 10.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 8.5), Point::new(5.0, 8.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.80);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.0, 10.0), Point::new(5.0, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 12.0), Point::new(5.0, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.80);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.0, 12.0), Point::new(5.0, 15.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.35);
    }
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 8.5), Point::new(1.5, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 12.0), Point::new(1.5, 15.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 10.0), Point::new(8.5, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 12.0), Point::new(8.5, 15.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }

    // Furniture and sanitary fixtures.
    let _ = project.add_fixture(FixtureKind::Sofa, Point::new(2.7, 5.5), 2.1, 0.9, 1);
    let _ = project.add_fixture(FixtureKind::TvStand, Point::new(5.0, 5.5), 1.6, 0.45, 3);
    let _ = project.add_fixture(FixtureKind::DiningSet, Point::new(3.5, 7.5), 1.5, 0.9, 0);
    let _ = project.add_fixture(FixtureKind::KitchenSink, Point::new(6.9, 4.45), 1.5, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(8.0, 4.45), 0.6, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Fridge, Point::new(8.0, 6.0), 0.7, 0.7, 3);
    let _ = project.add_fixture(FixtureKind::Washer, Point::new(7.9, 9.4), 0.65, 0.65, 2);
    let _ = project.add_fixture(FixtureKind::BedDouble, Point::new(3.0, 10.3), 1.6, 2.0, 1);
    let _ = project.add_fixture(FixtureKind::Wardrobe, Point::new(3.2, 8.95), 2.0, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(5.8, 10.45), 0.7, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(6.8, 10.55), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(7.85, 11.0), 1.0, 1.5, 3);
    let _ = project.add_fixture(FixtureKind::BedSingle, Point::new(2.7, 13.7), 0.95, 1.95, 1);
    let _ = project.add_fixture(FixtureKind::Desk, Point::new(4.3, 14.4), 1.1, 0.55, 2);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(5.8, 12.5), 0.7, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(6.8, 12.6), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(7.85, 14.0), 1.0, 1.5, 3);

    // Annotations & symbols.
    let _ = project.add_note(
        Point::new(2.6, 3.2),
        "ACESSO PRINCIPAL",
        NoteKind::Symbol(TechnicalSymbol::EntryArrow),
    );
    let _ = project.add_note(
        Point::new(9.2, 1.5),
        "N",
        NoteKind::Symbol(TechnicalSymbol::NorthArrow),
    );
    let _ = project.add_note(
        Point::new(3.5, 6.2),
        "NA +0,20",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.20 }),
    );
    project
}

fn build_sobrado() -> Project {
    let mut project = Project::default();
    project.name = "Sobrado 2 Pavimentos — Exemplo Trazo".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_50;
    project.sheet.title_block.drawing_title =
        "Planta Baixa — Sobrado (Térreo e Superior)".to_owned();
    project.sheet.title_block.client = "Projeto de Exemplo".to_owned();
    project.sheet.title_block.author = "Trazo CAD".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    let _ = project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
    project.setbacks = Setbacks {
        front: 4.5,
        rear: 3.0,
        side: 1.5,
    };

    let ground_id = project.ensure_default_floor();

    // Ground floor (Pavimento Térreo): x in [1.5, 8.5], y in [4.5, 13.5].
    let _ = project.add_room(Point::new(1.5, 4.5), Point::new(5.5, 9.0), "Sala de Estar");
    let _ = project.add_room(Point::new(5.5, 4.5), Point::new(8.5, 8.5), "Sala de Jantar");
    let _ = project.add_room(Point::new(5.5, 8.5), Point::new(8.5, 12.0), "Cozinha");
    let _ = project.add_room(
        Point::new(5.5, 12.0),
        Point::new(8.5, 13.5),
        "Área de Serviço",
    );
    let _ = project.add_room(Point::new(1.5, 9.0), Point::new(3.5, 11.0), "Lavabo");
    let _ = project.add_room(
        Point::new(1.5, 11.0),
        Point::new(5.5, 13.5),
        "Hall da Escada",
    );
    apply_residential_hatches(&mut project);

    if let Some(w) = find_wall_near(&project, Point::new(1.5, 4.5), Point::new(5.5, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.25);
        let _ = project.add_opening(w, OpeningKind::Window, 0.72);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.5), Point::new(8.5, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.5), Point::new(5.5, 8.5)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 8.5), Point::new(8.5, 8.5)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 12.0), Point::new(8.5, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.45);
    }
    if let Some(w) = find_wall_near(&project, Point::new(3.5, 9.0), Point::new(3.5, 11.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 8.5), Point::new(8.5, 12.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }

    // Stair connecting ground floor to upper floor (rise 2.85 m).
    let _ = project.add_stair(Point::new(2.0, 11.4), StairShape::UShaped, 1.0, 2.85, 0.28);

    // Ground floor fixtures.
    let _ = project.add_fixture(FixtureKind::Sofa, Point::new(2.6, 6.7), 2.2, 0.9, 1);
    let _ = project.add_fixture(FixtureKind::TvStand, Point::new(4.9, 6.7), 1.6, 0.45, 3);
    let _ = project.add_fixture(FixtureKind::DiningSet, Point::new(7.0, 6.5), 1.8, 1.0, 0);
    let _ = project.add_fixture(FixtureKind::KitchenSink, Point::new(7.0, 11.4), 1.6, 0.6, 2);
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(8.0, 9.8), 0.6, 0.6, 3);
    let _ = project.add_fixture(FixtureKind::Fridge, Point::new(8.0, 8.95), 0.7, 0.7, 3);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(2.1, 9.45), 0.55, 0.4, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(2.9, 9.55), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Washer, Point::new(7.9, 12.9), 0.65, 0.65, 2);
    let _ = project.add_note(
        Point::new(2.5, 3.7),
        "ACESSO SOCIAL",
        NoteKind::Symbol(TechnicalSymbol::EntryArrow),
    );
    let _ = project.add_note(
        Point::new(9.2, 1.5),
        "N",
        NoteKind::Symbol(TechnicalSymbol::NorthArrow),
    );

    // Upper floor (Pavimento Superior).
    let upper_id = project.add_floor("Pavimento Superior", true, false);
    let _ = project.switch_floor(upper_id);

    let _ = project.add_room(Point::new(1.5, 4.5), Point::new(5.5, 8.0), "Suíte Master");
    let _ = project.add_room(Point::new(5.5, 4.5), Point::new(8.5, 7.0), "Banheiro Suíte");
    let _ = project.add_room(Point::new(5.5, 7.0), Point::new(8.5, 10.5), "Quarto 2");
    let _ = project.add_room(Point::new(1.5, 8.0), Point::new(4.5, 11.0), "Quarto 3");
    let _ = project.add_room(
        Point::new(5.5, 10.5),
        Point::new(8.5, 13.5),
        "Banheiro Social",
    );
    let _ = project.add_room(
        Point::new(1.5, 11.0),
        Point::new(5.5, 13.5),
        "Hall Superior",
    );
    apply_residential_hatches(&mut project);

    if let Some(w) = find_wall_near(&project, Point::new(1.5, 4.5), Point::new(5.5, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.5), Point::new(5.5, 7.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 4.5), Point::new(8.5, 7.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 7.0), Point::new(8.5, 10.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(1.5, 8.0), Point::new(1.5, 11.0)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.5, 10.5), Point::new(8.5, 13.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }

    let _ = project.add_fixture(FixtureKind::BedDouble, Point::new(3.3, 6.2), 1.6, 2.0, 0);
    let _ = project.add_fixture(FixtureKind::Wardrobe, Point::new(2.0, 6.2), 1.8, 0.6, 1);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(6.3, 5.0), 0.7, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(7.2, 5.1), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(7.85, 6.2), 1.0, 1.2, 3);
    let _ = project.add_fixture(FixtureKind::BedSingle, Point::new(7.2, 8.8), 0.95, 1.95, 3);
    let _ = project.add_fixture(FixtureKind::BedSingle, Point::new(2.8, 9.5), 0.95, 1.95, 1);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(6.3, 11.0), 0.7, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(7.2, 11.1), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(7.85, 12.6), 1.0, 1.4, 3);

    // Return to ground floor so opening the project starts on Pavimento Térreo.
    let _ = project.switch_floor(ground_id);
    project
}

fn build_apartamento() -> Project {
    let mut project = Project::default();
    project.name = "Apartamento 2 Quartos — Exemplo Trazo".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_50;
    project.sheet.title_block.drawing_title = "Planta Baixa — Apartamento Tipo".to_owned();
    project.sheet.title_block.client = "Projeto de Exemplo".to_owned();
    project.sheet.title_block.author = "Trazo CAD".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    // 9.5 x 7.5 m apartment unit with balcony.
    let _ = project.add_room(Point::new(0.0, 0.0), Point::new(5.0, 1.5), "Varanda");
    let _ = project.add_room(
        Point::new(0.0, 1.5),
        Point::new(5.0, 5.0),
        "Sala de Estar e Jantar",
    );
    let _ = project.add_room(Point::new(5.0, 1.5), Point::new(8.0, 4.5), "Cozinha");
    let _ = project.add_room(
        Point::new(8.0, 1.5),
        Point::new(9.5, 4.5),
        "Área de Serviço",
    );
    let _ = project.add_room(Point::new(0.0, 5.0), Point::new(3.5, 7.5), "Suíte");
    let _ = project.add_room(Point::new(3.5, 5.0), Point::new(5.5, 7.5), "Banheiro Suíte");
    let _ = project.add_room(
        Point::new(5.5, 4.5),
        Point::new(7.5, 7.5),
        "Banheiro Social",
    );
    let _ = project.add_room(Point::new(7.5, 4.5), Point::new(9.5, 7.5), "Quarto 2");
    apply_residential_hatches(&mut project);

    // Openings.
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 1.5), Point::new(5.0, 1.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.0, 1.5), Point::new(5.0, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Gap, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.0, 1.5), Point::new(8.0, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.0, 1.5), Point::new(8.0, 1.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(8.0, 1.5), Point::new(9.5, 1.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 5.0), Point::new(3.5, 5.0)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.80);
    }
    if let Some(w) = find_wall_near(&project, Point::new(3.5, 5.0), Point::new(3.5, 7.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.35);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 4.5), Point::new(7.5, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(7.5, 4.5), Point::new(9.5, 4.5)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.35);
    }
    if let Some(w) = find_wall_near(&project, Point::new(0.0, 7.5), Point::new(3.5, 7.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(3.5, 7.5), Point::new(5.5, 7.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(5.5, 7.5), Point::new(7.5, 7.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(7.5, 7.5), Point::new(9.5, 7.5)) {
        let _ = project.add_opening(w, OpeningKind::Window, 0.50);
    }

    // Furniture & fixtures.
    let _ = project.add_fixture(FixtureKind::Sofa, Point::new(1.5, 3.0), 2.0, 0.85, 0);
    let _ = project.add_fixture(FixtureKind::TvStand, Point::new(1.5, 4.55), 1.6, 0.45, 2);
    let _ = project.add_fixture(FixtureKind::DiningSet, Point::new(3.8, 3.3), 1.4, 0.9, 0);
    let _ = project.add_fixture(FixtureKind::KitchenSink, Point::new(6.3, 1.95), 1.4, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(7.45, 1.95), 0.6, 0.6, 0);
    let _ = project.add_fixture(FixtureKind::Fridge, Point::new(7.45, 3.2), 0.7, 0.7, 3);
    let _ = project.add_fixture(FixtureKind::Washer, Point::new(8.8, 2.2), 0.65, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::BedDouble, Point::new(1.6, 6.25), 1.5, 1.95, 1);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(4.1, 5.45), 0.6, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(4.95, 5.55), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(4.5, 6.9), 1.5, 0.9, 0);
    let _ = project.add_fixture(FixtureKind::Washbasin, Point::new(6.1, 5.0), 0.6, 0.45, 0);
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(6.95, 5.1), 0.4, 0.65, 0);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(6.5, 6.9), 1.5, 0.9, 0);
    let _ = project.add_fixture(FixtureKind::BedSingle, Point::new(8.5, 6.25), 0.95, 1.95, 3);

    let _ = project.add_note(
        Point::new(2.5, 3.2),
        "NA ±0,00",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.0 }),
    );
    project
}

/// Reference semi-open gourmet area (#150): 5.00 × 14.75 m pavilion on a
/// 10 × 50 m lot against the left boundary, with bathroom + pantry block,
/// 4.30 m tall boundary wall, sloped gables, treated timber posts and eave
/// beams facing the pool, exposed 34 % mono-pitch roof, SPT soil survey,
/// ground beams + strip footings, boundary balance beams and NBR 6123 wind.
fn build_area_gourmet() -> Project {
    let mut project = Project::default();
    project.name = "Área Gourmet Semiaberta (5 × 14,75 m) — Referência Trazo".to_owned();
    project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    project.sheet.scale = DrawingScale::S1_100;
    project.sheet.title_block.drawing_title =
        "Área Gourmet Semiaberta — Planta e Estrutura de Referência".to_owned();
    project.sheet.title_block.client = "Projeto de Referência (#150)".to_owned();
    project.sheet.title_block.author = "Trazo CAD".to_owned();
    project.doc_style = DocStyle::from_preset(DocStylePreset::AbntNbr6492);

    // 19 cm structural concrete block keeps 4.30 m boundary wall slenderness within NBR 16868.
    project.masonry = Masonry {
        name: "Bloco estrutural de concreto 19×19×39 cm".to_owned(),
        block_width: 0.19,
        block_length: 0.39,
        block_height: 0.19,
        joint: 0.01,
        plaster: 0.015,
    };

    // 10 × 50 m lot (pavilion sits at x ∈ [0, 5], y ∈ [25.75, 40.50]).
    let _ = project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 50.0));

    // Closed bathroom + pantry block inside the south end of the pavilion.
    let _ = project.add_room(
        Point::new(0.0, 25.75),
        Point::new(2.50, 28.25),
        "Banheiro Gourmet",
    );
    let _ = project.add_room(Point::new(0.0, 28.25), Point::new(2.50, 30.75), "Despensa");
    apply_residential_hatches(&mut project);

    // Continue left boundary wall to 14.75 m total length, plus south/north gables to x = 5.0 m.
    let _ = project.add_wall(Point::new(0.0, 30.75), Point::new(0.0, 40.50));
    let _ = project.add_wall(Point::new(2.50, 25.75), Point::new(5.00, 25.75));
    let _ = project.add_wall(Point::new(0.0, 40.50), Point::new(5.00, 40.50));

    // Doors on the interior east wall of the bathroom and pantry (keeping the tall boundary wall clear).
    if let Some(w) = find_wall_near(&project, Point::new(2.50, 25.75), Point::new(2.50, 28.25)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }
    if let Some(w) = find_wall_near(&project, Point::new(2.50, 28.25), Point::new(2.50, 30.75)) {
        let _ = project.add_opening(w, OpeningKind::Door, 0.50);
    }

    // SPT soil survey (SP-01, firm sandy/clayey silt, water table at 5.0 m).
    let survey = SoilSurvey::new(
        "SP-01".to_owned(),
        vec![
            SptReading {
                depth_m: 1.0,
                n: 8,
                soil: SoilKind::ClayeySilt,
            },
            SptReading {
                depth_m: 2.0,
                n: 9,
                soil: SoilKind::ClayeySilt,
            },
            SptReading {
                depth_m: 3.0,
                n: 11,
                soil: SoilKind::SandySilt,
            },
            SptReading {
                depth_m: 4.0,
                n: 14,
                soil: SoilKind::SandySilt,
            },
            SptReading {
                depth_m: 5.0,
                n: 18,
                soil: SoilKind::Sand,
            },
            SptReading {
                depth_m: 6.0,
                n: 22,
                soil: SoilKind::Sand,
            },
        ],
        Some(5.0),
        1.0,
    )
    .expect("valid reference SPT survey");
    project.set_soil_survey(Some(survey));
    let _ = project.apply_survey_to_soil();

    // Concrete columns along the tall left boundary wall (30 cm across the wall, flush inside lot)
    // and interior block corners (omitting the middle interior column so y = 28.25 gets a strip footing).
    let mut west_col_ids = Vec::new();
    for y in [25.75, 28.25, 30.75, 34.00, 37.25, 40.50] {
        let pt = Point::new(0.0, y);
        let (c_center, c_turns, _) =
            project.structural_placement_fitted(FixtureKind::Column, pt, 0.30, 0.20);
        if let Some(id) = project.add_fixture(FixtureKind::Column, c_center, 0.30, 0.20, c_turns) {
            west_col_ids.push(id);
        }
        let (f_center, f_turns, _) =
            project.structural_placement_fitted(FixtureKind::Footing, pt, 1.00, 1.00);
        let _ = project.add_fixture(FixtureKind::Footing, f_center, 1.00, 1.00, f_turns);
    }
    let mut inner_col_ids = Vec::new();
    for y in [25.75, 30.75] {
        let pt = Point::new(2.50, y);
        if let Some(id) = project.add_fixture(FixtureKind::Column, pt, 0.20, 0.20, 0) {
            inner_col_ids.push(id);
        }
        let _ = project.add_fixture(FixtureKind::Footing, pt, 0.90, 0.90, 0);
    }

    // Concrete tie beams along the boundary wall and around the bathroom/pantry block + slabs.
    for pair in west_col_ids.windows(2) {
        let _ = project.add_beam(pair[0], pair[1], 0.20, 0.40);
    }
    if west_col_ids.len() >= 3 && inner_col_ids.len() == 2 {
        let _ = project.add_beam(west_col_ids[0], inner_col_ids[0], 0.20, 0.40);
        let _ = project.add_beam(west_col_ids[2], inner_col_ids[1], 0.20, 0.40);
        let _ = project.add_beam(inner_col_ids[0], inner_col_ids[1], 0.20, 0.40);
    }
    let room_ids: Vec<u64> = project.rooms.iter().map(|r| r.id).collect();
    for room_id in room_ids {
        let _ = project.set_slab(room_id, super::structure::SlabKind::Solid, None);
    }

    // Treated Cumaru timber posts (mourões Ø 20 cm) along the open pool edge (x = 5.0 m).
    // Five posts carry the low edge of the roof directly and via timber eave beams between posts.
    project.structure.post_species = TimberSpecies::Cumaru;
    project.structure.post_treated = true;
    for y in [25.75, 29.45, 33.15, 36.85, 40.50] {
        let _ = project.add_fixture(FixtureKind::TimberPost, Point::new(5.00, y), 0.20, 0.20, 0);
    }

    // Exposed mono-pitch ceramic tile roof in C30 timber (4.30 m → 2.60 m = 34 % draining east to the pool).
    let mut roof = Roof::new(
        Point::new(0.0, 25.75),
        Point::new(5.00, 40.50),
        RoofSide::West,
        4.30,
        2.60,
    );
    roof.tile = TileKind::Ceramic;
    roof.timber = TimberClass::C30;
    roof.exposed = true;
    roof.overhang_m = 0.50;
    roof.beam_spacing_m = 3.25;
    roof.lateral_bracing_kn = Some(60.0);
    let roof_id = project.add_roof(roof).expect("valid gourmet roof");
    let _ = project.fit_walls_to_roof(roof_id);

    // Keep the enclosed bathroom/pantry interior walls at their 2.80 m slab ceiling height.
    for (a, b) in [
        (Point::new(2.50, 25.75), Point::new(2.50, 28.25)),
        (Point::new(2.50, 28.25), Point::new(2.50, 30.75)),
        (Point::new(0.0, 28.25), Point::new(2.50, 28.25)),
        (Point::new(0.0, 30.75), Point::new(2.50, 30.75)),
    ] {
        if let Some(wall_id) = find_wall_near(&project, a, b) {
            let _ = project.set_wall_height(wall_id, Some(2.80), None);
        }
    }

    // Balance beams on boundary footings (#144), ground beams + strip footings (#143), and footing sizing.
    let _ = project.generate_balance_beams();
    let _ = project.generate_wall_foundations();
    let _ = project.size_footings();

    // NBR 6123 site wind (V0 = 35 m/s, terrain category III).
    let _ = project.set_wind(WindSettings {
        v0_ms: 35.0,
        category: 3,
        s1: 1.0,
        s3: 1.0,
    });

    // Sanitary fixtures, gourmet counter/dining furniture and 2D pool outline.
    let _ = project.add_fixture(
        FixtureKind::Washbasin,
        Point::new(0.55, 26.40),
        0.65,
        0.45,
        1,
    );
    let _ = project.add_fixture(FixtureKind::Toilet, Point::new(0.60, 27.35), 0.40, 0.65, 1);
    let _ = project.add_fixture(FixtureKind::Shower, Point::new(1.70, 26.55), 0.90, 1.10, 0);
    let _ = project.add_fixture(FixtureKind::Fridge, Point::new(0.65, 29.50), 0.70, 0.70, 1);
    let _ = project.add_fixture(
        FixtureKind::KitchenSink,
        Point::new(0.55, 34.00),
        2.20,
        0.65,
        1,
    );
    let _ = project.add_fixture(FixtureKind::Stove, Point::new(0.55, 36.00), 0.80, 0.65, 1);
    let _ = project.add_fixture(
        FixtureKind::DiningSet,
        Point::new(2.90, 35.00),
        1.60,
        2.80,
        0,
    );

    // Swimming pool outline on the open east side (x ∈ [6.2, 9.2], y ∈ [27.5, 38.5]).
    let _ = project.add_drawing(Shape::Polyline {
        points: vec![
            Point::new(6.2, 27.5),
            Point::new(9.2, 27.5),
            Point::new(9.2, 38.5),
            Point::new(6.2, 38.5),
        ],
        closed: true,
    });
    let _ = project.add_note(
        Point::new(2.6, 38.2),
        "ÁREA GOURMET (SEMIABERTA)",
        NoteKind::Text,
    );
    let _ = project.add_note(Point::new(7.7, 33.0), "PISCINA", NoteKind::Text);
    let _ = project.add_note(
        Point::new(9.2, 25.0),
        "N",
        NoteKind::Symbol(TechnicalSymbol::NorthArrow),
    );
    let _ = project.add_note(
        Point::new(3.2, 32.0),
        "NA +0,15",
        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.15 }),
    );
    project.ensure_default_layers();
    project
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::io::{export, project_file};

    fn examples_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
    }

    #[test]
    fn sample_projects_in_examples_load_validate_and_export_cleanly() {
        let dir = examples_dir();
        let temp_dir = std::env::temp_dir().join(format!("trazo-samples-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        for sample in SampleProject::ALL {
            let path = dir.join(sample.file_name());
            assert!(path.is_file(), "sample project missing: {}", path.display());

            let (project, warnings) = project_file::load(&path)
                .unwrap_or_else(|err| panic!("failed to load {}: {err}", path.display()));
            assert!(
                warnings.is_empty(),
                "{} loaded with repair/migration warnings: {warnings:?}",
                sample.file_name()
            );
            assert_eq!(project.version, CURRENT_PROJECT_VERSION);
            assert!(
                !project.rooms.is_empty(),
                "{} has no rooms",
                sample.file_name()
            );
            assert!(
                !project.walls.is_empty(),
                "{} has no walls",
                sample.file_name()
            );
            assert!(
                !project.openings.is_empty(),
                "{} has no openings",
                sample.file_name()
            );
            assert!(
                !project.fixtures.is_empty(),
                "{} has no fixtures",
                sample.file_name()
            );

            if sample == SampleProject::Sobrado {
                assert_eq!(
                    project.floors.levels.len(),
                    2,
                    "sobrado.json must have 2 floors"
                );
                assert!(
                    !project.circulations.is_empty(),
                    "sobrado.json must include a stair"
                );
            }

            let svg_path = temp_dir.join(format!("{}.svg", sample.file_name()));
            export::svg(&project, &svg_path).unwrap();
            let svg = std::fs::read_to_string(&svg_path).unwrap();
            assert!(
                svg.contains("<svg"),
                "invalid SVG for {}",
                sample.file_name()
            );

            let csv_path = temp_dir.join(format!("{}.csv", sample.file_name()));
            export::csv(&project, &csv_path).unwrap();
            let csv = std::fs::read_to_string(&csv_path).unwrap();
            assert!(
                !csv.trim().is_empty(),
                "empty CSV for {}",
                sample.file_name()
            );

            // Embedded JSON must match the on-disk file.
            let embedded = sample.load_embedded();
            assert_eq!(embedded.name, project.name);
            assert_eq!(embedded.rooms.len(), project.rooms.len());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn gourmet_area_reference_project_computes_end_to_end_in_ci() {
        let path = examples_dir().join(SampleProject::AreaGourmet.file_name());
        let (project, warnings) = project_file::load(&path)
            .unwrap_or_else(|err| panic!("failed to load area-gourmet.json: {err}"));
        assert!(warnings.is_empty(), "load warnings: {warnings:?}");
        assert!(
            crate::domain::limits::structural_refusal(&project).is_none(),
            "reference gourmet area must stay inside applicability limits"
        );

        let report = project.structure_report();
        // #112: SPT soil survey & comparative foundation study.
        assert!(project.structure.soil_survey.is_some());
        let study = project.foundation_study();
        assert!(study.survey);
        assert!(study.recommended.is_some());

        // #143: Both ground beams (under columned walls) and strip footings (under uncolumned partition).
        let ground_beams = report
            .wall_foundations
            .iter()
            .filter(|f| f.kind == crate::domain::wall_foundations::WallFoundationKind::GroundBeam)
            .count();
        let strip_footings = report
            .wall_foundations
            .iter()
            .filter(|f| f.kind == crate::domain::wall_foundations::WallFoundationKind::StripFooting)
            .count();
        assert!(
            ground_beams >= 2,
            "expected ground beams, got {ground_beams}"
        );
        assert!(
            strip_footings >= 1,
            "expected at least one strip footing, got {strip_footings}"
        );

        // #144: Boundary columns flush inside lot + balance beams.
        assert!(
            !report.balance_beams.is_empty(),
            "expected boundary balance beams"
        );
        assert!(report.columns.len() >= 8);

        // #145: Tall 4.30 m left boundary wall and sloped gables.
        assert!(
            project
                .walls
                .iter()
                .any(|w| w.heights(project.wall_height).0 >= 4.25),
            "expected 4.30 m tall boundary wall"
        );
        assert!(
            project.walls.iter().any(|w| {
                let (h0, h1) = w.heights(project.wall_height);
                (h0 - h1).abs() > 0.5
            }),
            "expected sloped gable walls"
        );

        // #114: Exposed 34 % mono-pitch timber roof with zero unsupported beam ends.
        assert_eq!(report.roofs.len(), 1);
        let roof = &report.roofs[0];
        assert!(
            (roof.slope_percent - 34.0).abs() < 0.2,
            "slope={}",
            roof.slope_percent
        );
        assert_eq!(
            roof.unsupported(),
            0,
            "roof has unsupported beam ends: {:?}",
            roof.reactions
        );
        assert!(roof.slope_ok());
        assert!(roof.wall_plate.is_some());
        assert!(roof.overhang_m > 0.0);

        // #146: Treated Cumaru timber posts and eave beams carrying the open pool edge.
        assert_eq!(report.timber_posts.len(), 5);
        assert!(
            report.timber_posts.iter().all(|p| p.ok()),
            "timber posts must pass: {:?}",
            report.timber_posts
        );
        assert!(
            !report.eave_beams.is_empty(),
            "expected timber eave beams spanning between posts"
        );
        assert!(
            report.eave_beams.iter().all(|b| b.ok && b.treated),
            "timber eave beams must pass: {:?}",
            report.eave_beams
        );

        // #147: NBR 6123 wind on tall boundary wall, open-sided roof uplift + anchor bolts,
        // X-bracing tie rod, masonry ties and 4-direction global stability + soil sliding.
        assert!(!report.wind.columns.is_empty());
        assert_eq!(report.wind.roofs.len(), 1);
        assert!(report.wind.roofs[0].open);
        assert!(!report.wind.roofs[0].anchor_spec.is_empty());
        assert_eq!(report.wind.bracing.len(), 1);
        assert!(report.wind.bracing[0].ok(), "{:?}", report.wind.bracing[0]);
        assert_eq!(report.wind.global.len(), 4);
        assert!(report.wind.global.iter().all(|g| g.sliding_fs > 0.0));
        assert!(
            report
                .masonry
                .panels
                .iter()
                .any(|p| p.wind.as_ref().is_some_and(|w| w.bond_beam.is_some()))
        );

        // #105, #110, #101, #103: Combinations, reinforcement schedule, traffic lights and review PDF.
        assert_eq!(report.loads.combinations.len(), 17);
        let reinforcement =
            crate::domain::reinforcement::build_reinforcement_report(&project, &report);
        assert!(reinforcement.total_mass_kg() > 0.0);
        let verdicts = crate::domain::verdict::structure_verdicts(&project, &report);
        assert!(!verdicts.is_empty());
        let pages = crate::io::report::report_pages(&project, "2026-11-24");
        assert!(pages.len() >= 5, "review memorial pages: {}", pages.len());
        assert!(pages.iter().all(|p| p.contains("<svg")));
    }

    #[test]
    #[ignore = "utility to regenerate examples/area-gourmet.json from build_area_gourmet"]
    fn regenerate_area_gourmet_file() {
        let dir = examples_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let project = SampleProject::AreaGourmet.build();
        project_file::save(&project, &dir.join(SampleProject::AreaGourmet.file_name())).unwrap();
    }

    #[test]
    #[ignore = "utility to regenerate examples/*.json from the canonical builders"]
    fn regenerate_example_files() {
        let dir = examples_dir();
        std::fs::create_dir_all(&dir).unwrap();
        for sample in SampleProject::ALL {
            let project = sample.build();
            project_file::save(&project, &dir.join(sample.file_name())).unwrap();
        }
    }
}
