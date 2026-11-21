//! External references (XREFs): other Trazo or DXF drawings linked into a
//! project, displayed live and reloaded automatically when updated on disk (#44).

use serde::{Deserialize, Serialize};

use super::{geometry::Point, model::Project};

fn default_xref_opacity() -> f32 {
    0.55
}

fn default_true() -> bool {
    true
}

fn default_xref_tint() -> [u8; 3] {
    [76, 114, 160]
}

/// A linked external drawing shown live inside the current project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalReference {
    pub id: u64,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub offset: Point,
    #[serde(default = "default_xref_opacity")]
    pub opacity: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_true")]
    pub locked: bool,
    #[serde(default = "default_xref_tint")]
    pub tint: [u8; 3],
}

impl Project {
    /// Attaches an external drawing reference (`.trazo`, `.json`, or `.dxf`).
    pub fn add_xref(&mut self, name: String, path: String, offset: Point) -> u64 {
        let id = self.xrefs.iter().map(|x| x.id).max().unwrap_or(0) + 1;
        let clean_name = if name.trim().is_empty() {
            "Referência externa".to_owned()
        } else {
            name.trim().to_owned()
        };
        self.xrefs.push(ExternalReference {
            id,
            name: clean_name,
            path,
            offset: offset.rounded(),
            opacity: default_xref_opacity(),
            visible: true,
            locked: true,
            tint: default_xref_tint(),
        });
        id
    }

    pub fn remove_xref(&mut self, id: u64) -> bool {
        let before = self.xrefs.len();
        self.xrefs.retain(|x| x.id != id);
        self.xrefs.len() < before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_references_round_trip_through_project_json() {
        let mut project = Project::default();
        let id = project.add_xref(
            "Estrutura".to_owned(),
            r"C:\Projetos\estrutura.trazo".to_owned(),
            Point::new(2.0, -1.5),
        );
        assert_eq!(project.xrefs.len(), 1);

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.xrefs.len(), 1);
        assert_eq!(loaded.xrefs[0].id, id);
        assert_eq!(loaded.xrefs[0].name, "Estrutura");
        assert!((loaded.xrefs[0].offset.x - 2.0).abs() < 1e-6);

        assert!(project.remove_xref(id));
        assert!(project.xrefs.is_empty());
    }
}
