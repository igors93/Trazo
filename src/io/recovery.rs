use std::{fs, path::{Path, PathBuf}};

use crate::domain::model::Project;

use super::project_file;

fn recovery_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Trazo")
        .join("recovery.json")
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

pub fn load() -> Option<Project> {
    load_from(&recovery_path())
}

fn load_from(path: &Path) -> Option<Project> {
    project_file::load(path)
        .ok()
        .or_else(|| project_file::load(&backup_path(path)).ok())
}

pub fn save(project: &Project) -> Result<(), String> {
    save_to(project, &recovery_path())
}

fn save_to(project: &Project, path: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or("Caminho de recuperação inválido")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = temporary_path(path);
    project_file::save(project, &temporary)?;
    if path.exists() {
        let backup = backup_path(path);
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| error.to_string())?;
        }
        fs::rename(path, &backup).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

pub fn clear() {
    let path = recovery_path();
    for file in [&path, &backup_path(&path), &temporary_path(&path)] {
        if let Err(error) = fs::remove_file(file) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Could not clear Trazo recovery file: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn damaged_latest_recovery_falls_back_to_previous_snapshot() {
        let path = std::env::temp_dir().join(format!("trazo-recovery-{}.json", std::process::id()));
        let mut project = Project::default();
        project.add_wall(Point::new(0.0, 0.0), Point::new(2.0, 0.0));
        save_to(&project, &path).unwrap();
        project.add_wall(Point::new(0.0, 1.0), Point::new(2.0, 1.0));
        save_to(&project, &path).unwrap();
        assert_eq!(load_from(&path).unwrap().walls.len(), 2);
        fs::write(&path, "incomplete json").unwrap();
        assert_eq!(load_from(&path).unwrap().walls.len(), 1);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }
}
