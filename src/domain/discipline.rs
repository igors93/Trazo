//! Discipline views (#96) and synchronized workspace tabs: architecture,
//! electrical, plumbing (prepared view for #72), structure and foundations
//! are views of the same central model. Each shows what it needs, in its own
//! colour, on screen (the rest faded behind it) and on its own sheets.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::layers::LayerCategory;
use super::model::Project;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Discipline {
    /// Everything at once.
    #[default]
    All,
    Architecture,
    Electrical,
    /// Prepared view highlighting sanitary/wet fixtures (hydraulic piping
    /// sizing joins in #72).
    Plumbing,
    Structure,
    /// Columns and footings (foundation plan view).
    Foundations,
}

impl Discipline {
    pub const ALL: [Discipline; 6] = [
        Discipline::All,
        Discipline::Architecture,
        Discipline::Electrical,
        Discipline::Plumbing,
        Discipline::Structure,
        Discipline::Foundations,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Visão geral",
            Self::Architecture => "Arquitetura",
            Self::Electrical => "Elétrica",
            Self::Plumbing => "Hidráulica",
            Self::Structure => "Estrutura",
            Self::Foundations => "Fundações",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::All => "tudo",
            Self::Architecture => "arquitetura",
            Self::Electrical => "eletrica",
            Self::Plumbing => "hidraulica",
            Self::Structure => "estrutura",
            Self::Foundations => "fundacoes",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        let normalized = key.trim().to_lowercase();
        match normalized.as_str() {
            "tudo" | "geral" | "visao_geral" | "visão geral" => Some(Self::All),
            "arquitetura" | "arq" => Some(Self::Architecture),
            "eletrica" | "elétrica" => Some(Self::Electrical),
            "hidraulica" | "hidráulica" | "hidrossanitaria" | "hidrossanitária" => {
                Some(Self::Plumbing)
            }
            "estrutura" | "estrutural" => Some(Self::Structure),
            "fundacoes" | "fundações" | "fundacao" | "fundação" => Some(Self::Foundations),
            _ => Self::ALL
                .into_iter()
                .find(|d| d.key() == normalized || d.label().to_lowercase() == normalized),
        }
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
            Self::Plumbing => &[LayerCategory::Electrical, LayerCategory::Structure],
            Self::Structure | Self::Foundations => {
                &[LayerCategory::Electrical, LayerCategory::Fixtures]
            }
        }
    }

    /// The layer drawn on top, with the architecture faded behind it.
    pub fn focus(self) -> Option<LayerCategory> {
        match self {
            Self::Electrical => Some(LayerCategory::Electrical),
            Self::Plumbing => Some(LayerCategory::Fixtures),
            Self::Structure | Self::Foundations => Some(LayerCategory::Structure),
            _ => None,
        }
    }

    /// What a sheet of this discipline is called.
    pub fn sheet_title(self) -> &'static str {
        match self {
            Self::All | Self::Architecture => "Planta Baixa",
            Self::Electrical => "Planta de Instalações Elétricas",
            Self::Plumbing => "Planta Hidrossanitária",
            Self::Structure => "Planta de Forma",
            Self::Foundations => "Planta de Fundações",
        }
    }
}

/// Which side of a split view holds a persisted workspace tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePane {
    #[default]
    Left,
    Right,
}

impl WorkspacePane {
    pub fn is_left(&self) -> bool {
        *self == Self::Left
    }
}

/// Persisted configuration of one synchronized view tab within a project file.
///
/// Tabs saved here do NOT duplicate the model: they all open as linked views
/// of the single [`Project`] loaded from the file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceTab {
    /// Optional custom tab label; when absent, derived from project name + discipline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub discipline: Discipline,
    /// Floor level shown in this view (`None` uses the project's active floor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_id: Option<u64>,
    /// Optional layer IDs hidden specifically in this view tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_layers: Vec<u64>,
    /// Whether this tab was focused when saved.
    #[serde(default, skip_serializing_if = "is_false")]
    pub active: bool,
    /// Whether this tab was displayed as the companion split-view pane.
    #[serde(default, skip_serializing_if = "is_false")]
    pub split_companion: bool,
    #[serde(default, skip_serializing_if = "WorkspacePane::is_left")]
    pub pane: WorkspacePane,
    /// Saved zoom scale in pixels/metre (when positive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    /// Saved camera pan offset `[x, y]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan: Option<[f32; 2]>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Default for WorkspaceTab {
    fn default() -> Self {
        Self {
            title: None,
            discipline: Discipline::All,
            floor_id: None,
            hidden_layers: Vec::new(),
            active: false,
            split_companion: false,
            pane: WorkspacePane::Left,
            scale: None,
            pan: None,
        }
    }
}

impl Project {
    /// The project as `discipline` shows it: its hidden layers turned off.
    pub fn for_discipline(&self, discipline: Discipline) -> Cow<'_, Project> {
        self.for_view(discipline, &[])
    }

    /// The project filtered for a view tab: discipline layer rules plus any
    /// per-tab hidden layer overrides (`hidden_layers`).
    pub fn for_view(&self, discipline: Discipline, hidden_layers: &[u64]) -> Cow<'_, Project> {
        let hidden = discipline.hidden();
        if hidden.is_empty() && hidden_layers.is_empty() {
            return Cow::Borrowed(self);
        }
        let mut shown = self.clone();
        shown.ensure_default_layers();
        for layer in &mut shown.layers {
            if layer.category.is_some_and(|c| hidden.contains(&c))
                || hidden_layers.contains(&layer.id)
            {
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
        let foundations = project.for_discipline(Discipline::Foundations);
        assert!(foundations.category_visible(LayerCategory::Structure));
        assert!(!foundations.category_visible(LayerCategory::Electrical));
        let plumbing = project.for_discipline(Discipline::Plumbing);
        assert!(plumbing.category_visible(LayerCategory::Fixtures));
        assert!(!plumbing.category_visible(LayerCategory::Electrical));
        assert_eq!(
            Discipline::from_key("estrutura"),
            Some(Discipline::Structure)
        );
        assert_eq!(
            Discipline::from_key("fundações"),
            Some(Discipline::Foundations)
        );
        assert_eq!(
            Discipline::from_key("hidráulica"),
            Some(Discipline::Plumbing)
        );
        assert_eq!(
            Discipline::Structure.focus(),
            Some(LayerCategory::Structure)
        );
    }
}
