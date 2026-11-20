//! Canonical sample projects stored in `examples/` (#81).
//!
//! Each sample is shipped as a standalone `.json` file in `examples/` for docs,
//! screenshots, tests and double-click opening, and is also embedded so users
//! can load it directly from *Arquivo → Novo a partir de modelo...*.

use super::{
    circulation::StairShape,
    doc_style::{DocStyle, DocStylePreset},
    fixtures::FixtureKind,
    geometry::Point,
    hatch::{HatchPattern, HatchStyle},
    model::{CURRENT_PROJECT_VERSION, OpeningKind, Project},
    notes::{NoteKind, TechnicalSymbol},
    setbacks::Setbacks,
    sheet::{DrawingScale, Orientation, PaperSize, SheetLayout},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleProject {
    CasaTerrea,
    Sobrado,
    Apartamento,
}

impl SampleProject {
    pub const ALL: [SampleProject; 3] = [
        SampleProject::CasaTerrea,
        SampleProject::Sobrado,
        SampleProject::Apartamento,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::CasaTerrea => "casa-terrea.json",
            Self::Sobrado => "sobrado.json",
            Self::Apartamento => "apartamento.json",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::CasaTerrea => "Casa térrea (2 quartos + suíte)",
            Self::Sobrado => "Sobrado (2 pavimentos, 3 quartos)",
            Self::Apartamento => "Apartamento (2 quartos com varanda)",
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
        }
    }

    /// Loads the sample from its embedded `examples/*.json` file (falling back
    /// to the programmatic builder only if needed).
    pub fn load_embedded(self) -> Project {
        let json = match self {
            Self::CasaTerrea => include_str!("../../examples/casa-terrea.json"),
            Self::Sobrado => include_str!("../../examples/sobrado.json"),
            Self::Apartamento => include_str!("../../examples/apartamento.json"),
        };
        serde_json::from_str::<Project>(json).unwrap_or_else(|_| self.build())
    }

    /// Programmatic builder used to produce and validate the canonical sample files.
    pub fn build(self) -> Project {
        let mut project = match self {
            Self::CasaTerrea => build_casa_terrea(),
            Self::Sobrado => build_sobrado(),
            Self::Apartamento => build_apartamento(),
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
