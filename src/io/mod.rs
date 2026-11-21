pub mod crash_log;
pub mod dxf;
pub mod export;
pub mod instance;
pub mod personal_library;
pub mod print;
pub mod project_file;
pub mod recovery;
pub mod settings;
pub mod underlay;
pub mod updates;

use std::path::PathBuf;

/// Where Trazo keeps its own files (crash-recovery draft, preferences).
///
/// Tests get a folder of their own: flows such as "new project" clear the
/// recovery draft, and a test run must never delete the person's real one.
pub fn data_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("trazo-tests-{}", std::process::id()));
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Trazo")
}

#[cfg(test)]
mod tests {
    #[test]
    fn tests_never_touch_the_real_data_folder() {
        let folder = super::data_dir();
        assert!(folder.starts_with(std::env::temp_dir()));
        assert!(folder.to_string_lossy().contains("trazo-tests-"));
    }
}
