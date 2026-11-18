use std::{fs, path::Path};

use crate::domain::model::Project;

pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    let content = serde_json::to_string_pretty(project).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

pub fn load(path: &Path) -> Result<Project, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let project: Project = serde_json::from_str(&content).map_err(|error| error.to_string())?;
    if project.version != 1 {
        return Err(format!(
            "Versão de projeto não suportada: {}",
            project.version
        ));
    }
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn round_trip_keeps_editable_walls() {
        let mut project = Project::default();
        project.add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
        let path = std::env::temp_dir().join(format!("trazo-project-{}.json", std::process::id()));
        save(&project, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.walls.len(), 1);
        assert_eq!(loaded.walls[0].length(), 2.0);
        let _ = fs::remove_file(path);
    }
}
