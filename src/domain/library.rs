//! Customizable furniture, equipment and technical symbol library.
//!
//! Ships with a rich built-in Brazilian catalog (living, bedroom, kitchen,
//! bathroom, office, laundry, electrical/plumbing symbols) and lets users
//! author and save their own custom items inside the project.

use serde::{Deserialize, Serialize};

use super::{
    fixtures::{FixtureKind, valid_size},
    geometry::Point,
    model::Project,
    notes::{NoteKind, TechnicalSymbol},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryCategory {
    #[default]
    Living,
    Bedroom,
    Kitchen,
    Bathroom,
    Office,
    Laundry,
    Outdoor,
    Symbols,
    Custom,
}

impl LibraryCategory {
    pub const ALL: [LibraryCategory; 9] = [
        LibraryCategory::Living,
        LibraryCategory::Bedroom,
        LibraryCategory::Kitchen,
        LibraryCategory::Bathroom,
        LibraryCategory::Office,
        LibraryCategory::Laundry,
        LibraryCategory::Outdoor,
        LibraryCategory::Symbols,
        LibraryCategory::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Living => "Sala",
            Self::Bedroom => "Quarto",
            Self::Kitchen => "Cozinha",
            Self::Bathroom => "Banheiro",
            Self::Office => "Escritório",
            Self::Laundry => "Área de serviço",
            Self::Outdoor => "Garagem e lazer",
            Self::Symbols => "Símbolos técnicos",
            Self::Custom => "Personalizados",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryItem {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub category: LibraryCategory,
    pub width_m: f64,
    pub depth_m: f64,
    /// Built-in fixture symbol used when placed on plan (`None` for text/technical symbols).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_kind: Option<FixtureKind>,
    /// Optional short technical symbol tag (e.g. `"TOM"`, `"QDC"`, `"RS"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_tag: Option<String>,
}

impl LibraryItem {
    /// Curated built-in catalog of common Brazilian furniture, appliances & technical symbols.
    pub fn builtin_catalog() -> Vec<LibraryItem> {
        let mut id = 10_000;
        let mut mk = |name: &str,
                      category: LibraryCategory,
                      width_m: f64,
                      depth_m: f64,
                      base_kind: Option<FixtureKind>,
                      symbol_tag: Option<&str>| {
            id += 1;
            LibraryItem {
                id,
                name: name.to_owned(),
                category,
                width_m,
                depth_m,
                base_kind,
                symbol_tag: symbol_tag.map(|s| s.to_owned()),
            }
        };
        vec![
            mk(
                "Sofá 3 lugares",
                LibraryCategory::Living,
                2.10,
                0.85,
                Some(FixtureKind::Sofa),
                None,
            ),
            mk(
                "Sofá retrátil (2,50 m)",
                LibraryCategory::Living,
                2.50,
                1.10,
                Some(FixtureKind::Sofa),
                None,
            ),
            mk(
                "Poltrona",
                LibraryCategory::Living,
                0.85,
                0.85,
                Some(FixtureKind::Armchair),
                None,
            ),
            mk(
                "Rack / Painel TV",
                LibraryCategory::Living,
                1.80,
                0.45,
                Some(FixtureKind::TvStand),
                None,
            ),
            mk(
                "Mesa de jantar 6 lugares",
                LibraryCategory::Living,
                1.80,
                0.90,
                Some(FixtureKind::DiningSet),
                None,
            ),
            mk(
                "Cama casal padrão (1,38 × 1,88)",
                LibraryCategory::Bedroom,
                1.38,
                1.88,
                Some(FixtureKind::BedDouble),
                None,
            ),
            mk(
                "Cama Queen (1,58 × 1,98)",
                LibraryCategory::Bedroom,
                1.58,
                1.98,
                Some(FixtureKind::BedDouble),
                None,
            ),
            mk(
                "Cama King (1,93 × 2,03)",
                LibraryCategory::Bedroom,
                1.93,
                2.03,
                Some(FixtureKind::BedDouble),
                None,
            ),
            mk(
                "Cama solteiro",
                LibraryCategory::Bedroom,
                0.90,
                1.95,
                Some(FixtureKind::BedSingle),
                None,
            ),
            mk(
                "Guarda-roupa 6 portas",
                LibraryCategory::Bedroom,
                2.40,
                0.60,
                Some(FixtureKind::Wardrobe),
                None,
            ),
            mk(
                "Bancada de cozinha",
                LibraryCategory::Kitchen,
                1.80,
                0.60,
                Some(FixtureKind::Counter),
                None,
            ),
            mk(
                "Pia inox",
                LibraryCategory::Kitchen,
                1.20,
                0.60,
                Some(FixtureKind::KitchenSink),
                None,
            ),
            mk(
                "Fogão 4 bocas",
                LibraryCategory::Kitchen,
                0.60,
                0.60,
                Some(FixtureKind::Stove),
                None,
            ),
            mk(
                "Geladeira Duplex",
                LibraryCategory::Kitchen,
                0.72,
                0.72,
                Some(FixtureKind::Fridge),
                None,
            ),
            mk(
                "Churrasqueira",
                LibraryCategory::Kitchen,
                1.00,
                0.65,
                Some(FixtureKind::Grill),
                None,
            ),
            mk(
                "Lavatório com bancada",
                LibraryCategory::Bathroom,
                0.80,
                0.50,
                Some(FixtureKind::Washbasin),
                None,
            ),
            mk(
                "Vaso sanitário",
                LibraryCategory::Bathroom,
                0.40,
                0.65,
                Some(FixtureKind::Toilet),
                None,
            ),
            mk(
                "Box chuveiro",
                LibraryCategory::Bathroom,
                0.90,
                0.90,
                Some(FixtureKind::Shower),
                None,
            ),
            mk(
                "Banheira de imersão",
                LibraryCategory::Bathroom,
                1.70,
                0.80,
                Some(FixtureKind::Bathtub),
                None,
            ),
            mk(
                "Escrivaninha home office",
                LibraryCategory::Office,
                1.40,
                0.65,
                Some(FixtureKind::Desk),
                None,
            ),
            mk(
                "Estante / Prateleira",
                LibraryCategory::Office,
                1.20,
                0.40,
                Some(FixtureKind::Shelves),
                None,
            ),
            mk(
                "Máquina de lavar",
                LibraryCategory::Laundry,
                0.60,
                0.60,
                Some(FixtureKind::Washer),
                None,
            ),
            mk(
                "Carro (passeio médio)",
                LibraryCategory::Outdoor,
                1.85,
                4.50,
                Some(FixtureKind::Car),
                None,
            ),
            mk(
                "Carro compacto",
                LibraryCategory::Outdoor,
                1.75,
                4.00,
                Some(FixtureKind::Car),
                None,
            ),
            mk(
                "SUV / Caminhonete",
                LibraryCategory::Outdoor,
                2.00,
                5.10,
                Some(FixtureKind::Car),
                None,
            ),
            mk(
                "Guarda-sol",
                LibraryCategory::Outdoor,
                2.40,
                2.40,
                Some(FixtureKind::Umbrella),
                None,
            ),
            mk(
                "Cadeira de sol (espreguiçadeira)",
                LibraryCategory::Outdoor,
                0.75,
                1.95,
                Some(FixtureKind::SunLounger),
                None,
            ),
            mk(
                "Ponto de luz no teto",
                LibraryCategory::Symbols,
                0.30,
                0.30,
                None,
                Some("LUZ"),
            ),
            mk(
                "Tomada baixa (h=30 cm)",
                LibraryCategory::Symbols,
                0.25,
                0.25,
                None,
                Some("TOM 30"),
            ),
            mk(
                "Quadro de distribuição (QDC)",
                LibraryCategory::Symbols,
                0.40,
                0.20,
                None,
                Some("QDC"),
            ),
            mk(
                "Ralo seco (RS)",
                LibraryCategory::Symbols,
                0.20,
                0.20,
                None,
                Some("RS"),
            ),
        ]
    }
}

impl Project {
    pub fn add_custom_library_item(
        &mut self,
        name: &str,
        category: LibraryCategory,
        width_m: f64,
        depth_m: f64,
        base_kind: Option<FixtureKind>,
        symbol_tag: Option<String>,
    ) -> Result<u64, String> {
        let clean = name.trim();
        if clean.is_empty() {
            return Err("Informe um nome para o item da biblioteca.".to_owned());
        }
        if base_kind.is_some() && !valid_size(width_m, depth_m) {
            return Err("Dimensões fora dos limites permitidos (0,20 a 8,00 m).".to_owned());
        }
        let id = self.library.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        self.library.push(LibraryItem {
            id,
            name: clean.to_owned(),
            category,
            width_m,
            depth_m,
            base_kind,
            symbol_tag: symbol_tag.and_then(|s| {
                let t = s.trim();
                (!t.is_empty()).then(|| t.to_owned())
            }),
        });
        Ok(id)
    }

    pub fn remove_library_item(&mut self, id: u64) -> bool {
        let before = self.library.len();
        self.library.retain(|i| i.id != id);
        self.library.len() != before
    }

    /// Places a catalog or custom library item onto the active floor plan.
    pub fn place_library_item(
        &mut self,
        item: &LibraryItem,
        center: Point,
        quarter_turns: u8,
    ) -> Result<u64, String> {
        if let Some(kind) = item.base_kind {
            self.add_fixture(kind, center, item.width_m, item.depth_m, quarter_turns)
                .ok_or_else(|| "Não foi possível posicionar o móvel dentro do terreno.".to_owned())
        } else {
            let tag = item.symbol_tag.as_deref().unwrap_or(item.name.as_str());
            self.add_note(
                center,
                tag,
                if tag == "N" {
                    NoteKind::Symbol(TechnicalSymbol::NorthArrow)
                } else {
                    NoteKind::Note
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_fixture_catalog_and_custom_library_place_and_round_trip() {
        assert_eq!(FixtureKind::ALL.len(), 24);
        for kind in FixtureKind::ALL {
            let (w, d) = kind.default_size();
            assert!(!crate::domain::fixtures::symbol(kind, w, d).is_empty());
            assert!(kind.front_clearance_m() >= 0.45);
        }

        let catalog = LibraryItem::builtin_catalog();
        assert!(catalog.len() >= 25);

        let mut project = Project::default();
        let custom_id = project
            .add_custom_library_item(
                "Cama Super King",
                LibraryCategory::Custom,
                2.03,
                2.03,
                Some(FixtureKind::BedDouble),
                None,
            )
            .unwrap();
        let custom = project.library[0].clone();
        let placed_fixture = project
            .place_library_item(&custom, Point::new(2.0, 2.0), 0)
            .unwrap();
        assert!(project.fixtures.iter().any(|f| f.id == placed_fixture));

        // Technical symbol item places as annotation note.
        let sym_item = catalog
            .iter()
            .find(|i| i.category == LibraryCategory::Symbols)
            .unwrap();
        let note_id = project
            .place_library_item(sym_item, Point::new(1.0, 1.0), 0)
            .unwrap();
        assert!(project.notes.iter().any(|n| n.id == note_id));

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.library.len(), 1);
        assert!(project.remove_library_item(custom_id));
        assert!(project.library.is_empty());
    }
}
