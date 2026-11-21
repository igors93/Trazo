//! Discipline views (#96): architecture, electrical and structure are views
//! of the same model. Each shows what it needs, in its own colour, on
//! screen (the rest faded behind it) and on its own sheets.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::layers::LayerCategory;
use super::model::Project;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discipline {
    /// Everything at once.
    #[default]
    All,
    Architecture,
    Electrical,
    Structure,
}

impl Discipline {
    pub const ALL: [Discipline; 4] = [
        Discipline::All,
        Discipline::Architecture,
        Discipline::Electrical,
        Discipline::Structure,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Tudo",
            Self::Architecture => "Arquitetura",
            Self::Electrical => "Elétrica",
            Self::Structure => "Estrutura",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::All => "tudo",
            Self::Architecture => "arquitetura",
            Self::Electrical => "eletrica",
            Self::Structure => "estrutura",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.key() == key || d.label().to_lowercase() == key)
    }

    pub fn is_all(&self) -> bool {
        *self == Self::All
    }

    /// The layers this discipline leaves out.
    pub fn hidden(self) -> &'static [LayerCategory] {
        match self {
            Self::All => &[],
            Self::Architecture => &[LayerCategory::Electrical, LayerCategory::Structure],
            Self::Electrical => &[LayerCategory::Structure],
            Self::Structure => &[LayerCategory::Electrical, LayerCategory::Fixtures],
        }
    }

    /// The layer drawn on top, with the architecture faded behind it.
    pub fn focus(self) -> Option<LayerCategory> {
        match self {
            Self::Electrical => Some(LayerCategory::Electrical),
            Self::Structure => Some(LayerCategory::Structure),
            _ => None,
        }
    }

    /// What a sheet of this discipline is called.
    pub fn sheet_title(self) -> &'static str {
        match self {
            Self::All | Self::Architecture => "Planta Baixa",
            Self::Electrical => "Planta de Instalações Elétricas",
            Self::Structure => "Planta de Forma",
        }
    }
}

impl Project {
    /// The project as `discipline` shows it: its hidden layers turned off.
    pub fn for_discipline(&self, discipline: Discipline) -> Cow<'_, Project> {
        let hidden = discipline.hidden();
        if hidden.is_empty() {
            return Cow::Borrowed(self);
        }
        let mut shown = self.clone();
        shown.ensure_default_layers();
        for layer in &mut shown.layers {
            if layer.category.is_some_and(|c| hidden.contains(&c)) {
                layer.visible = false;
            }
        }
        Cow::Owned(shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_discipline_hides_the_others() {
        let project = Project::default();
        assert!(matches!(
            project.for_discipline(Discipline::All),
            Cow::Borrowed(_)
        ));
        let architecture = project.for_discipline(Discipline::Architecture);
        assert!(!architecture.category_visible(LayerCategory::Electrical));
        assert!(!architecture.category_visible(LayerCategory::Structure));
        assert!(architecture.category_visible(LayerCategory::Walls));
        let structure = project.for_discipline(Discipline::Structure);
        assert!(structure.category_visible(LayerCategory::Structure));
        assert!(!structure.category_visible(LayerCategory::Fixtures));
        assert_eq!(
            Discipline::from_key("estrutura"),
            Some(Discipline::Structure)
        );
        assert_eq!(
            Discipline::Structure.focus(),
            Some(LayerCategory::Structure)
        );
    }
}
