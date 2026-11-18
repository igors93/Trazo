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
        project.add_room(Point::new(3.0, 1.0), Point::new(6.0, 4.0), "Ateliê");
        let path = std::env::temp_dir().join(format!("trazo-project-{}.json", std::process::id()));
        save(&project, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.walls.len(), 5);
        assert_eq!(loaded.walls[0].length(), 2.0);
        assert_eq!(loaded.rooms[0].display_name(), "Ateliê");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn older_room_types_remain_readable() {
        let content = r#"{"version":1,"name":"Antiga","walls":[],"rooms":[{"id":1,"kind":"Bedroom","wall_ids":[2,3,4,5]}],"openings":[],"next_id":6}"#;
        let project: Project = serde_json::from_str(content).unwrap();
        assert_eq!(project.rooms[0].display_name(), "Quarto");
        assert!(project.terrain.is_none());
    }

    #[test]
    fn terrain_survives_project_round_trip() {
        let mut project = Project::default();
        project.set_terrain(Point::new(2.0, 3.0), Point::new(7.0, 13.0));
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.terrain.unwrap().area(), 50.0);
    }
}
