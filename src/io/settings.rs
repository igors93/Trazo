//! Interface preferences kept between sessions: grid, drafting aids and
//! which panels are open. They belong to the person, not to a project.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{element::SelectionFilter, snap::OsnapSettings, wall_reference::WallReference};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_grid: bool,
    pub snap_enabled: bool,
    pub osnap: OsnapSettings,
    pub wall_reference: WallReference,
    pub ortho_enabled: bool,
    pub show_browser: bool,
    pub show_properties: bool,
    pub ribbon_collapsed: bool,
    /// Wall lengths and room sides follow the block module.
    pub modulation: bool,
    /// Dimension chains around the building.
    pub dimensions: bool,
    /// Which kinds a selection box or "select all" picks.
    pub selection_filter: SelectionFilter,
    /// The view follows what a connected AI assistant changes.
    pub follow_ai: bool,
    /// Check GitHub Releases for a newer version at startup.
    pub check_updates: bool,
    /// Light mode for modest computers (#164); `None` lets Trazo decide by
    /// the computer's memory.
    pub light_mode: Option<bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_grid: true,
            snap_enabled: true,
            osnap: OsnapSettings::default(),
            wall_reference: WallReference::default(),
            ortho_enabled: false,
            show_browser: true,
            show_properties: true,
            ribbon_collapsed: false,
            modulation: true,
            dimensions: true,
            selection_filter: SelectionFilter::default(),
            follow_ai: true,
            check_updates: true,
            light_mode: None,
        }
    }
}

fn settings_path() -> PathBuf {
    super::data_dir().join("settings.json")
}

/// The saved preferences, or the defaults when there are none or the file
/// cannot be read (a missing or damaged file never blocks the app).
pub fn load() -> Settings {
    load_from(&settings_path())
}

fn load_from(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> Result<(), String> {
    save_to(settings, &settings_path())
}

/// Writes through a temporary file and a rename, so an interrupted write
/// leaves the previous preferences intact.
fn save_to(settings: &Settings, path: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or("Caminho de preferências inválido")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let content = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, content).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("trazo-settings-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn preferences_round_trip_and_replace_the_previous_file() {
        let path = temp_path("round-trip");
        let mut settings = Settings {
            show_grid: false,
            ortho_enabled: true,
            ..Default::default()
        };
        save_to(&settings, &path).unwrap();
        assert_eq!(load_from(&path), settings);
        settings.ribbon_collapsed = true;
        save_to(&settings, &path).unwrap();
        assert_eq!(load_from(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_damaged_or_older_files_fall_back_to_defaults() {
        let missing = temp_path("missing");
        assert_eq!(load_from(&missing), Settings::default());

        let damaged = temp_path("damaged");
        fs::write(&damaged, "{ not json").unwrap();
        assert_eq!(load_from(&damaged), Settings::default());
        let _ = fs::remove_file(&damaged);

        // A file from a version that knew fewer preferences keeps what it has.
        let older = temp_path("older");
        fs::write(&older, r#"{ "show_grid": false }"#).unwrap();
        let loaded = load_from(&older);
        assert!(!loaded.show_grid);
        assert!(loaded.snap_enabled && loaded.show_browser);
        assert_eq!(loaded.osnap, OsnapSettings::default());
        assert_eq!(loaded.wall_reference, WallReference::Center);
        let _ = fs::remove_file(&older);
    }

    #[test]
    fn osnap_and_wall_reference_preferences_persist_across_saves() {
        let path = temp_path("osnap");
        let settings = Settings {
            osnap: OsnapSettings {
                midpoint: false,
                tangent: false,
                ..Default::default()
            },
            wall_reference: WallReference::Internal,
            ..Default::default()
        };
        save_to(&settings, &path).unwrap();
        let loaded = load_from(&path);
        assert!(!loaded.osnap.midpoint);
        assert!(!loaded.osnap.tangent);
        assert!(loaded.osnap.endpoint && loaded.osnap.intersection && loaded.osnap.center);
        assert_eq!(loaded.wall_reference, WallReference::Internal);
        let _ = fs::remove_file(&path);
    }
}
