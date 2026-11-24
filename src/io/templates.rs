//! User project templates (`.trazot`) and recent-files registry (T-06).
//!
//! Opening a `.trazot` file always starts a fresh, untitled project without
//! binding `file_path` so a template is never accidentally overwritten on Save.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_TEMPLATES_SHOWN: usize = 10;
const MAX_TEMPLATES_KEPT: usize = 200;
const MAX_RECENT_FILES: usize = 12;
const MAX_CONFIG_FILE_BYTES: u64 = 256 << 10;

/// Returns `true` when `path` carries the `.trazot` project-template extension.
pub fn is_template_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("trazot"))
}

fn normalize_key(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn templates_file() -> PathBuf {
    super::data_dir().join("templates.json")
}

fn recent_file() -> PathBuf {
    super::data_dir().join("recent.json")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserTemplates {
    pub files: Vec<PathBuf>,
}

impl UserTemplates {
    pub fn load() -> Self {
        Self::load_from(&templates_file())
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(meta) = fs::metadata(path) else {
            return Self::default();
        };
        if !meta.is_file() || meta.len() > MAX_CONFIG_FILE_BYTES {
            return Self::default();
        }
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&templates_file())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("Caminho de modelos inválido.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let part = path.with_extension("json.part");
        fs::write(&part, text).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&part, path).map_err(|e| e.to_string())
    }

    /// Promotes or inserts `path` at the top of the user template list.
    pub fn pick(&mut self, path: &Path) -> bool {
        let key = normalize_key(path);
        if self.files.first() == Some(&key) {
            return false;
        }
        self.files.retain(|p| *p != key);
        self.files.insert(0, key);
        self.files.truncate(MAX_TEMPLATES_KEPT);
        true
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, path: &Path) {
        let key = normalize_key(path);
        self.files.retain(|p| *p != key);
    }

    /// Up to [`MAX_TEMPLATES_SHOWN`] templates displayed in New / Start Page.
    pub fn shown(&self) -> &[PathBuf] {
        &self.files[..self.files.len().min(MAX_TEMPLATES_SHOWN)]
    }
}

/// One entry in the Start Page recent-projects list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub title: String,
    #[serde(default)]
    pub wall_count: usize,
    #[serde(default)]
    pub room_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentFiles {
    pub entries: Vec<RecentEntry>,
}

impl RecentFiles {
    pub fn load() -> Self {
        Self::load_from(&recent_file())
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(meta) = fs::metadata(path) else {
            return Self::default();
        };
        if !meta.is_file() || meta.len() > MAX_CONFIG_FILE_BYTES {
            return Self::default();
        }
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&recent_file())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("Caminho de recentes inválido.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let part = path.with_extension("json.part");
        fs::write(&part, text).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&part, path).map_err(|e| e.to_string())
    }

    /// Inserts or promotes a recently opened/saved project file (ignoring `.trazot` templates).
    pub fn record(&mut self, path: &Path, title: &str, wall_count: usize, room_count: usize) {
        if is_template_path(path) {
            return;
        }
        let key = normalize_key(path);
        self.entries.retain(|e| e.path != key);
        let clean_title = if title.trim().is_empty() {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Projeto".to_owned())
        } else {
            title.trim().to_owned()
        };
        self.entries.insert(
            0,
            RecentEntry {
                path: key,
                title: clean_title,
                wall_count,
                room_count,
            },
        );
        self.entries.truncate(MAX_RECENT_FILES);
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, path: &Path) {
        let key = normalize_key(path);
        self.entries.retain(|e| e.path != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_templates_and_recent_files_round_trip_cleanly() {
        let dir = std::env::temp_dir().join(format!("trazo-tpl-test-{}", std::process::id()));
        let tpl_cfg = dir.join("templates.json");
        let rec_cfg = dir.join("recent.json");

        assert!(is_template_path(Path::new("residencial.trazot")));
        assert!(is_template_path(Path::new("PADRAO.TRAZOT")));
        assert!(!is_template_path(Path::new("casa.trazo")));

        let mut templates = UserTemplates::default();
        let t1 = dir.join("residencial.trazot");
        let t2 = dir.join("comercial.trazot");
        assert!(templates.pick(&t1));
        assert!(templates.pick(&t2));
        assert_eq!(templates.shown().len(), 2);
        assert_eq!(templates.shown()[0], normalize_key(&t2));
        templates.save_to(&tpl_cfg).unwrap();
        let reloaded = UserTemplates::load_from(&tpl_cfg);
        assert_eq!(reloaded, templates);

        let mut recents = RecentFiles::default();
        let p1 = dir.join("casa.trazo");
        recents.record(&t1, "Modelo", 4, 1); // .trazot ignored in recents
        assert!(recents.entries.is_empty());
        recents.record(&p1, "Casa Principal", 12, 4);
        assert_eq!(recents.entries.len(), 1);
        recents.save_to(&rec_cfg).unwrap();
        let reloaded_rec = RecentFiles::load_from(&rec_cfg);
        assert_eq!(reloaded_rec, recents);

        let _ = fs::remove_dir_all(&dir);
    }
}
