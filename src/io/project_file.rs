use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::domain::model::Project;

pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    let content = serde_json::to_string_pretty(project)
        .map_err(|error| format!("Falha na serialização do projeto: {}", error))?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("projeto");
    let pid = std::process::id();
    let tmp_path = parent.join(format!("{}.{}.trazo.tmp", file_stem, pid));

    let write_res = (|| -> Result<(), std::io::Error> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Erro ao gravar arquivo temporário: {}", err));
    }

    if let Err(err) = safe_replace(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Erro ao salvar projeto: {}", err));
    }

    Ok(())
}

fn safe_replace(tmp_path: &Path, target_path: &Path) -> Result<(), std::io::Error> {
    if !target_path.exists() {
        return fs::rename(tmp_path, target_path);
    }

    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("projeto");
    let pid = std::process::id();
    let backup_path: PathBuf = parent.join(format!("{}.{}.trazo.bak", file_stem, pid));

    if backup_path.exists() {
        let _ = fs::remove_file(&backup_path);
    }

    fs::rename(target_path, &backup_path)?;

    if let Err(err) = fs::rename(tmp_path, target_path) {
        let _ = fs::rename(&backup_path, target_path);
        return Err(err);
    }

    let _ = fs::remove_file(&backup_path);
    Ok(())
}

pub fn load(path: &Path) -> Result<Project, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Não foi possível ler o arquivo: {}", error))?;
    let mut project: Project = serde_json::from_str(&content)
        .map_err(|error| format!("Formato de arquivo inválido: {}", error))?;
    if project.version != 1 {
        return Err(format!(
            "Versão de projeto não suportada: {}",
            project.version
        ));
    }
    project
        .validate_and_repair()
        .map_err(|error| format!("Erro de validação: {}", error))?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{geometry::Point, model::OpeningKind};

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
    fn safe_atomic_save_overwrites_existing_file() {
        let path = std::env::temp_dir().join(format!("trazo-atomic-{}.json", std::process::id()));
        let mut proj1 = Project::default();
        proj1.name = "Versao 1".to_owned();
        save(&proj1, &path).unwrap();

        let mut proj2 = Project::default();
        proj2.name = "Versao 2".to_owned();
        save(&proj2, &path).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.name, "Versao 2");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn older_room_types_remain_readable() {
        let content = r#"{"version":1,"name":"Antiga","walls":[],"rooms":[{"id":1,"kind":"Bedroom","wall_ids":[2,3,4,5]}],"openings":[],"next_id":6}"#;
        let mut project: Project = serde_json::from_str(content).unwrap();
        assert_eq!(project.rooms[0].display_name(), "Quarto");
        assert!(project.terrain.is_none());
        // Validation should clean rooms with non-existent wall IDs
        let _ = project.validate_and_repair();
        assert_eq!(project.rooms.len(), 0);
    }

    #[test]
    fn terrain_survives_project_round_trip() {
        let mut project = Project::default();
        project.set_terrain(Point::new(2.0, 3.0), Point::new(7.0, 13.0));
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.terrain.unwrap().area(), 50.0);
    }

    #[test]
    fn load_rejects_non_finite_coordinates() {
        let path = std::env::temp_dir().join(format!("trazo-corrupt-{}.json", std::process::id()));
        let json = r#"{"version":1,"name":"Corrupto","walls":[{"id":1,"start":{"x":0.0,"y":0.0},"end":{"x":"NaN","y":5.0}}],"rooms":[],"openings":[],"next_id":2}"#;
        fs::write(&path, json).unwrap();
        assert!(load(&path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_repairs_referential_integrity_and_deduplicates_walls() {
        let mut project = Project::default();
        let w1 = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        // Duplicate wall
        let _w2 = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        // Opening referencing non-existent wall
        project.add_opening(999, OpeningKind::Door, 0.5);
        // Opening on valid wall
        project.add_opening(w1, OpeningKind::Window, 0.5);

        let path = std::env::temp_dir().join(format!("trazo-repair-{}.json", std::process::id()));
        save(&project, &path).unwrap();

        let loaded = load(&path).unwrap();
        // Duplicate wall deduplicated
        assert_eq!(loaded.walls.len(), 1);
        // Orphan opening removed, valid opening kept
        assert_eq!(loaded.openings.len(), 1);
        assert_eq!(loaded.openings[0].wall_id, loaded.walls[0].id);
        let _ = fs::remove_file(path);
    }
}
