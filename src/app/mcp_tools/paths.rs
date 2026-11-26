//! Where the files an assistant names are written: Trazo's folder in
//! Documents, the expected extension and names that do not overwrite.

use std::path::{Path, PathBuf};

/// Trazo's folder inside the person's Documents (OneDrive's when Windows
/// keeps Documents there), where files go when no folder is given.
pub(super) fn documents_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let one_drive = std::env::var_os("OneDrive").map(PathBuf::from);
    let candidates = one_drive
        .iter()
        .flat_map(|folder| [folder.join("Documentos"), folder.join("Documents")])
        .chain([home.join("Documents"), home.join("Documentos")]);
    candidates
        .into_iter()
        .find(|folder| folder.is_dir())
        .unwrap_or(home)
        .join("Trazo")
}

/// A path given by the assistant: relative ones are taken from Trazo's
/// Documents folder, and a missing extension is added.
pub(super) fn resolve_path(input: &str, extension: &str) -> PathBuf {
    let mut path = PathBuf::from(input.trim().trim_matches('"'));
    if path.is_relative() {
        path = documents_dir().join(path);
    }
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}

/// `path`, or the first "name (2).ext", "name (3).ext"... that is free.
pub(super) fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|index| path.with_file_name(format!("{stem} ({index}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(path)
}

/// The project name as a safe file name.
pub(super) fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "planta".to_owned()
    } else {
        cleaned
    }
}

pub(super) fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Não foi possível criar a pasta {}: {error}",
                parent.display()
            )
        })?;
    }
    Ok(())
}
