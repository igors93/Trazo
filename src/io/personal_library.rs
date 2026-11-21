//! User's personal library shared across all Trazo projects (#43).
//!
//! Persisted as JSON in `<data_dir>/personal-library.json` so custom library
//! symbols and reusable blocks with attributes created in one project are
//! immediately available in every other project on the machine.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{blocks::BlockDefinition, library::LibraryItem};

const CURRENT_PERSONAL_LIBRARY_VERSION: u32 = 1;

fn default_version() -> u32 {
    CURRENT_PERSONAL_LIBRARY_VERSION
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonalLibrary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<LibraryItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<BlockDefinition>,
}

impl Default for PersonalLibrary {
    fn default() -> Self {
        Self {
            version: CURRENT_PERSONAL_LIBRARY_VERSION,
            items: Vec::new(),
            blocks: Vec::new(),
        }
    }
}

impl PersonalLibrary {
    pub fn add_item(&mut self, mut item: LibraryItem) -> u64 {
        let id = self.items.iter().map(|i| i.id).max().unwrap_or(50_000) + 1;
        item.id = id;
        self.items.push(item);
        id
    }

    pub fn remove_item(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < before
    }

    pub fn add_block(&mut self, mut block: BlockDefinition) -> u64 {
        let id = self.blocks.iter().map(|b| b.id).max().unwrap_or(80_000) + 1;
        block.id = id;
        self.blocks.push(block);
        id
    }

    pub fn remove_block(&mut self, id: u64) -> bool {
        let before = self.blocks.len();
        self.blocks.retain(|b| b.id != id);
        self.blocks.len() < before
    }
}

pub fn personal_library_path() -> PathBuf {
    crate::io::data_dir().join("personal-library.json")
}

pub fn load() -> PersonalLibrary {
    load_from(&personal_library_path()).unwrap_or_default()
}

pub fn load_from(path: &Path) -> Result<PersonalLibrary, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Não foi possível ler a biblioteca pessoal: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("Biblioteca pessoal inválida: {error}"))
}

pub fn save(library: &PersonalLibrary) -> Result<(), String> {
    save_to(library, &personal_library_path())
}

pub fn save_to(library: &PersonalLibrary, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("Não foi possível criar pasta da biblioteca pessoal: {error}")
        })?;
    }
    let json = serde_json::to_string_pretty(library)
        .map_err(|error| format!("Falha ao serializar biblioteca pessoal: {error}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)
        .map_err(|error| format!("Falha ao gravar biblioteca pessoal: {error}"))?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path)
        .map_err(|error| format!("Falha ao salvar biblioteca pessoal: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        blocks::BlockAttribute, element::Element, fixtures::FixtureKind, geometry::Point,
        library::LibraryCategory, model::Project,
    };

    #[test]
    fn personal_library_shares_items_and_blocks_between_separate_projects() {
        let dir =
            std::env::temp_dir().join(format!("trazo-personal-lib-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("personal-library.json");

        // Project A authors a custom item and a reusable block with attributes.
        let mut proj_a = Project::default();
        let (w, d) = FixtureKind::Desk.default_size();
        let desk = proj_a
            .add_fixture(FixtureKind::Desk, Point::new(1.0, 1.0), w, d, 0)
            .unwrap();
        let block = proj_a
            .define_block_from_elements(
                "Estação Home Office",
                "Escritório",
                "Mesa completa",
                &[Element::Fixture(desk)],
                vec![BlockAttribute::new("CODIGO", "Código", "HO-01")],
            )
            .unwrap();

        let mut lib = PersonalLibrary::default();
        lib.add_item(LibraryItem {
            id: 0,
            name: "Ilha gourmet pessoal".to_owned(),
            category: LibraryCategory::Kitchen,
            width_m: 2.2,
            depth_m: 0.9,
            base_kind: Some(FixtureKind::Counter),
            symbol_tag: None,
        });
        lib.add_block(block);
        save_to(&lib, &path).unwrap();

        // Project B (brand new, empty) loads the shared personal library and inserts the block.
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.blocks.len(), 1);

        let mut proj_b = Project::default();
        let inst_id = proj_b
            .insert_block(
                &loaded.blocks[0],
                Point::new(4.0, 2.0),
                &[("CODIGO".to_owned(), "HO-B".to_owned())],
            )
            .unwrap();
        assert_eq!(proj_b.fixtures.len(), 1);
        assert_eq!(
            proj_b
                .block_instances
                .iter()
                .find(|i| i.id == inst_id)
                .unwrap()
                .attribute_value("CODIGO"),
            Some("HO-B")
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
