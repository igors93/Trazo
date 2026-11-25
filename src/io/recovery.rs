use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::domain::model::Project;

use super::project_file;

fn recovery_path() -> PathBuf {
    // Each open window keeps its own draft (see `instance`).
    super::data_dir().join(super::instance::recovery_file_name())
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
    project_file::load_project(path)
        .ok()
        .or_else(|| project_file::load_project(&backup_path(path)).ok())
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

/// The draft of the `n`-th waiting tab (from 1), next to this window's draft.
fn tab_path(main: &Path, n: usize) -> PathBuf {
    let stem = main
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "recovery".to_owned());
    main.with_file_name(format!("{stem}-aba-{n}.json"))
}

/// The most waiting tabs a window keeps drafts for.
const MAX_TAB_DRAFTS: usize = 64;

/// Keeps a draft of each tab with unsaved changes other than the active one
/// (whose draft is `save`'s), replacing the previous set.
pub fn save_tabs(projects: &[&Project]) -> Result<(), String> {
    save_tabs_at(projects, &recovery_path())
}

fn save_tabs_at(projects: &[&Project], main: &Path) -> Result<(), String> {
    for (index, project) in projects.iter().take(MAX_TAB_DRAFTS).enumerate() {
        save_to(project, &tab_path(main, index + 1))?;
    }
    // Keep the previous drafts until every replacement has been saved. A failed
    // write must not erase the only recovery copy of the remaining projects.
    clear_tabs_from(main, projects.len().min(MAX_TAB_DRAFTS) + 1);
    Ok(())
}

/// The waiting tabs' drafts, in order.
pub fn load_tabs() -> Vec<Project> {
    load_tabs_at(&recovery_path())
}

fn load_tabs_at(main: &Path) -> Vec<Project> {
    (1..=MAX_TAB_DRAFTS)
        .filter_map(|n| load_from(&tab_path(main, n)))
        .collect()
}

pub fn clear_tabs() {
    clear_tabs_from(&recovery_path(), 1);
}

fn clear_tabs_from(main: &Path, first: usize) {
    for n in first..=MAX_TAB_DRAFTS {
        let path = tab_path(main, n);
        for file in [&path, &backup_path(&path), &temporary_path(&path)] {
            let _ = fs::remove_file(file);
        }
    }
}

pub fn clear() {
    let path = recovery_path();
    for file in [&path, &backup_path(&path), &temporary_path(&path)] {
        if let Err(error) = fs::remove_file(file)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Could not clear Trazo recovery file: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn tab_recovery_skips_corrupt_drafts_and_cleans_up_after_gaps() {
        let folder =
            std::env::temp_dir().join(format!("trazo-recovery-gaps-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let main = folder.join("recovery.json");
        fs::write(tab_path(&main, 1), "broken json").unwrap();
        save_to(&Project::default(), &tab_path(&main, 3)).unwrap();
        assert_eq!(load_tabs_at(&main).len(), 1);
        clear_tabs_from(&main, 1);
        assert!(!tab_path(&main, 3).exists());
        fs::remove_dir(&folder).unwrap();
    }

    #[test]
    fn failed_tab_save_preserves_remaining_recovery_drafts() {
        let folder =
            std::env::temp_dir().join(format!("trazo-recovery-failed-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let main = folder.join("recovery.json");
        let original = Project::default();
        save_tabs_at(&[&original, &original, &original], &main).unwrap();
        let blocked = temporary_path(&tab_path(&main, 2));
        fs::create_dir(&blocked).unwrap();
        let mut edited = original.clone();
        edited.name = "Alterado".to_owned();
        assert!(save_tabs_at(&[&edited, &edited, &edited], &main).is_err());
        assert!(load_from(&tab_path(&main, 2)).is_some());
        assert!(load_from(&tab_path(&main, 3)).is_some());
        fs::remove_dir(blocked).unwrap();
        save_tabs_at(&[&edited], &main).unwrap();
        assert_eq!(load_tabs_at(&main).len(), 1);
        clear_tabs_from(&main, 1);
        fs::remove_dir(&folder).unwrap();
    }

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
