//! Reusable user-authored CAD blocks with structured attributes (#43).
//!
//! A `BlockDefinition` captures any selection of walls, fixtures and drawings
//! normalized around an insertion anchor together with named attributes
//! (code, manufacturer, finish, power, notes). Blocks can live inside a
//! project or in the user's shared personal library and be inserted multiple
//! times with independent per-instance attribute values.

use serde::{Deserialize, Serialize};

use super::{
    element::Element,
    geometry::Point,
    model::{Fragment, Project},
    transform::Transform,
};

/// A named attribute schema attached to a reusable block definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockAttribute {
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub default_value: String,
}

impl BlockAttribute {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        default_value: impl Into<String>,
    ) -> Self {
        let key = key.into().trim().to_uppercase();
        let label_raw = label.into();
        let label = if label_raw.trim().is_empty() {
            key.clone()
        } else {
            label_raw.trim().to_owned()
        };
        Self {
            key,
            label,
            default_value: default_value.into().trim().to_owned(),
        }
    }
}

/// Reusable CAD block definition containing normalized geometry and attributes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockDefinition {
    pub id: u64,
    pub name: String,
    #[serde(default = "default_block_category")]
    pub category: String,
    #[serde(default)]
    pub description: String,
    pub fragment: Fragment,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<BlockAttribute>,
}

fn default_block_category() -> String {
    "Blocos".to_owned()
}

impl BlockDefinition {
    /// Returns the `(width, height)` span of the block's geometry in metres.
    pub fn size_m(&self) -> (f64, f64) {
        self.fragment
            .bounds()
            .map(|(min, max)| ((max.x - min.x).abs(), (max.y - min.y).abs()))
            .unwrap_or((0.0, 0.0))
    }

    pub fn element_count(&self) -> usize {
        self.fragment.walls.len()
            + self.fragment.openings.len()
            + self.fragment.fixtures.len()
            + self.fragment.drawings.len()
    }
}

/// A placed instance of a `BlockDefinition` inside a project, with its own
/// attribute values and references to the inserted elements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockInstance {
    pub id: u64,
    pub block_id: u64,
    pub block_name: String,
    pub origin: Point,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Element>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<(String, String)>,
}

impl BlockInstance {
    #[allow(dead_code)]
    pub fn attribute_value(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

impl Project {
    /// Creates a reusable `BlockDefinition` from the given project elements,
    /// normalizing their coordinates so the block's centre sits at `(0, 0)`.
    pub fn define_block_from_elements(
        &mut self,
        name: &str,
        category: &str,
        description: &str,
        elements: &[Element],
        attributes: Vec<BlockAttribute>,
    ) -> Result<BlockDefinition, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Informe um nome para o bloco.".to_owned());
        }
        let mut fragment = self.fragment(elements);
        if fragment.is_empty() {
            return Err(
                "Selecione ao menos uma parede, móvel ou desenho para criar o bloco.".to_owned(),
            );
        }
        if let Some(center) = fragment.center() {
            let _ = fragment.transform(&Transform::Translate(Point::new(-center.x, -center.y)));
        }
        let id = self.blocks.iter().map(|b| b.id).max().unwrap_or(0) + 1;
        let block = BlockDefinition {
            id,
            name: trimmed.to_owned(),
            category: if category.trim().is_empty() {
                default_block_category()
            } else {
                category.trim().to_owned()
            },
            description: description.trim().to_owned(),
            fragment,
            attributes: attributes
                .into_iter()
                .filter(|a| !a.key.trim().is_empty())
                .collect(),
        };
        self.blocks.push(block.clone());
        Ok(block)
    }

    /// Inserts a `BlockDefinition` (either project-local or from the shared
    /// personal library) at `position`, grouping its inserted elements and
    /// recording a `BlockInstance` with per-instance attribute values.
    pub fn insert_block(
        &mut self,
        block: &BlockDefinition,
        position: Point,
        overrides: &[(String, String)],
    ) -> Result<u64, String> {
        if block.fragment.is_empty() {
            return Err("O bloco selecionado não possui geometria.".to_owned());
        }
        let moved = block
            .fragment
            .transformed(&Transform::Translate(position))?;
        let members = self.insert_fragment(&moved)?;
        if members.len() >= 2 {
            let _ = self.group_elements(&members, &block.name);
        }

        // Resolve final attributes: start from definition defaults, apply overrides.
        let mut resolved: Vec<(String, String)> = block
            .attributes
            .iter()
            .map(|attr| {
                let value = overrides
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&attr.key))
                    .map(|(_, v)| v.trim().to_owned())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| attr.default_value.clone());
                (attr.key.clone(), value)
            })
            .collect();
        for (k, v) in overrides {
            let key = k.trim().to_uppercase();
            if !key.is_empty() && !resolved.iter().any(|(rk, _)| rk.eq_ignore_ascii_case(&key)) {
                resolved.push((key, v.trim().to_owned()));
            }
        }

        let id = self.block_instances.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        self.block_instances.push(BlockInstance {
            id,
            block_id: block.id,
            block_name: block.name.clone(),
            origin: position.rounded(),
            members,
            attributes: resolved,
        });
        Ok(id)
    }

    /// Updates (or adds) an attribute value on an existing placed block instance.
    #[allow(dead_code)]
    pub fn set_block_instance_attribute(
        &mut self,
        instance_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let key = key.trim().to_uppercase();
        if key.is_empty() {
            return Err("Chave do atributo não pode estar vazia.".to_owned());
        }
        let instance = self
            .block_instances
            .iter_mut()
            .find(|i| i.id == instance_id)
            .ok_or_else(|| "Instância de bloco não encontrada.".to_owned())?;
        if let Some((_, existing)) = instance
            .attributes
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        {
            *existing = value.trim().to_owned();
        } else {
            instance.attributes.push((key, value.trim().to_owned()));
        }
        Ok(())
    }

    pub fn remove_block_definition(&mut self, block_id: u64) -> bool {
        let before = self.blocks.len();
        self.blocks.retain(|b| b.id != block_id);
        self.blocks.len() < before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    #[test]
    fn defines_reusable_block_with_attributes_and_inserts_independent_instances() {
        let mut project = Project::default();
        let (w1, d1) = FixtureKind::Washbasin.default_size();
        let f1 = project
            .add_fixture(FixtureKind::Washbasin, Point::new(2.0, 2.0), w1, d1, 0)
            .unwrap();
        let (w2, d2) = FixtureKind::Toilet.default_size();
        let f2 = project
            .add_fixture(FixtureKind::Toilet, Point::new(2.8, 2.0), w2, d2, 0)
            .unwrap();

        let block = project
            .define_block_from_elements(
                "Kit Lavabo",
                "Banheiro",
                "Lavatório + bacia",
                &[Element::Fixture(f1), Element::Fixture(f2)],
                vec![
                    BlockAttribute::new("CODIGO", "Código", "LAV-01"),
                    BlockAttribute::new("FABRICANTE", "Fabricante", "Deca"),
                ],
            )
            .unwrap();
        assert_eq!(block.element_count(), 2);
        let (w, h) = block.size_m();
        assert!(w > 0.5 && h > 0.3);

        // Insert two instances at different locations with different attribute values.
        let i1 = project
            .insert_block(
                &block,
                Point::new(6.0, 3.0),
                &[("CODIGO".to_owned(), "LAV-A".to_owned())],
            )
            .unwrap();
        let i2 = project
            .insert_block(
                &block,
                Point::new(10.0, 3.0),
                &[
                    ("CODIGO".to_owned(), "LAV-B".to_owned()),
                    ("FABRICANTE".to_owned(), "Roca".to_owned()),
                ],
            )
            .unwrap();

        assert_eq!(project.fixtures.len(), 6);
        assert_eq!(project.block_instances.len(), 2);
        assert_eq!(
            project.block_instances[0].attribute_value("CODIGO"),
            Some("LAV-A")
        );
        assert_eq!(
            project.block_instances[0].attribute_value("FABRICANTE"),
            Some("Deca")
        );
        assert_eq!(
            project.block_instances[1].attribute_value("FABRICANTE"),
            Some("Roca")
        );

        project
            .set_block_instance_attribute(i1, "OBSERVACAO", "Padrão acessível")
            .unwrap();
        assert_eq!(
            project
                .block_instances
                .iter()
                .find(|i| i.id == i1)
                .unwrap()
                .attribute_value("OBSERVACAO"),
            Some("Padrão acessível")
        );
        assert_ne!(i1, i2);

        // Round-trip via JSON.
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.blocks.len(), 1);
        assert_eq!(loaded.block_instances.len(), 2);
    }
}
