pub mod export;
pub mod project_file;
pub mod recovery;
pub mod settings;

use std::path::PathBuf;

/// Where Trazo keeps its own files (crash-recovery draft, preferences).
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Trazo")
}
