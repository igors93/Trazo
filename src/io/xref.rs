//! Live loader and filesystem polling manager for external references (XREFs) (#44).

use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{Duration, Instant, SystemTime},
};

use crate::{
    domain::{model::Project, xref::ExternalReference},
    io::{dxf, project_file},
};

/// Cached runtime state of a single attached external reference.
pub struct LoadedXref {
    pub project: Project,
    pub modified: Option<SystemTime>,
    pub file_len: u64,
    pub error: Option<String>,
    pub reload_count: u32,
}

/// Manages loading and live change-detection for all attached XREFs.
pub struct XrefManager {
    entries: HashMap<u64, LoadedXref>,
    last_poll: Option<Instant>,
    poll_interval: Duration,
}

impl Default for XrefManager {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            last_poll: None,
            poll_interval: Duration::from_secs(1),
        }
    }
}

impl XrefManager {
    pub fn get(&self, id: u64) -> Option<&LoadedXref> {
        self.entries.get(&id)
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Loads a `.trazo`, `.json`, or `.dxf` file into a `Project` snapshot.
    pub fn load_external_project(path: &Path) -> Result<Project, String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "dxf" {
            let mut project = Project::default();
            dxf::import_file(&mut project, path)?;
            return Ok(project);
        }
        project_file::load(path).map(|(project, _)| project)
    }

    fn read_file_stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
        let meta = fs::metadata(path).ok()?;
        Some((meta.modified().ok(), meta.len()))
    }

    /// Forces an immediate load/reload of a specific XREF.
    pub fn reload_one(&mut self, xref: &ExternalReference) -> bool {
        let path = Path::new(&xref.path);
        let (modified, file_len) = Self::read_file_stamp(path).unwrap_or((None, 0));
        let prev_count = self
            .entries
            .get(&xref.id)
            .map(|e| e.reload_count)
            .unwrap_or(0);
        match Self::load_external_project(path) {
            Ok(project) => {
                self.entries.insert(
                    xref.id,
                    LoadedXref {
                        project,
                        modified,
                        file_len,
                        error: None,
                        reload_count: prev_count + 1,
                    },
                );
                true
            }
            Err(err) => {
                self.entries.insert(
                    xref.id,
                    LoadedXref {
                        project: Project::default(),
                        modified,
                        file_len,
                        error: Some(err),
                        reload_count: prev_count,
                    },
                );
                false
            }
        }
    }

    /// Polls all attached XREFs (throttled to `poll_interval`, or immediately
    /// when `force == true` or an XREF is not yet cached) and returns the names
    /// of any references that were updated live from disk.
    pub fn sync_live(&mut self, xrefs: &[ExternalReference], force: bool) -> Vec<String> {
        let active_ids: Vec<u64> = xrefs.iter().map(|x| x.id).collect();
        self.entries.retain(|id, _| active_ids.contains(id));

        let now = Instant::now();
        let poll_due = force
            || self
                .last_poll
                .is_none_or(|last| now.duration_since(last) >= self.poll_interval);
        let mut updated = Vec::new();

        for xref in xrefs {
            let needs_initial = !self.entries.contains_key(&xref.id);
            if !needs_initial && !poll_due {
                continue;
            }
            if needs_initial {
                self.reload_one(xref);
                continue;
            }
            let path = Path::new(&xref.path);
            let Some((modified, file_len)) = Self::read_file_stamp(path) else {
                continue;
            };
            let changed = self
                .entries
                .get(&xref.id)
                .is_some_and(|entry| entry.modified != modified || entry.file_len != file_len);
            if changed && self.reload_one(xref) {
                updated.push(xref.name.clone());
            }
        }

        if poll_due {
            self.last_poll = Some(now);
        }
        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn xref_manager_loads_trazo_and_dxf_and_reloads_live_on_file_change() {
        let dir = std::env::temp_dir().join(format!("trazo-xref-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let trazo_path = dir.join("base.trazo");
        let dxf_path = dir.join("site.dxf");

        // Save initial Trazo project with 1 wall.
        let mut ext = Project::default();
        ext.add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        project_file::save(&ext, &trazo_path).unwrap();
        dxf::export(&ext, &dxf_path).unwrap();

        let mut host = Project::default();
        let id_trazo = host.add_xref(
            "Base".to_owned(),
            trazo_path.to_string_lossy().into_owned(),
            Point::new(0.0, 0.0),
        );
        let id_dxf = host.add_xref(
            "Terreno DXF".to_owned(),
            dxf_path.to_string_lossy().into_owned(),
            Point::new(1.0, 0.0),
        );

        let mut mgr = XrefManager::default();
        let initial = mgr.sync_live(&host.xrefs, true);
        assert!(initial.is_empty());
        assert_eq!(mgr.get(id_trazo).unwrap().project.walls.len(), 1);
        assert_eq!(mgr.get(id_dxf).unwrap().project.walls.len(), 1);

        // Update the external .trazo file with a second wall and verify live reload.
        ext.add_wall(Point::new(4.0, 0.0), Point::new(4.0, 3.0))
            .unwrap();
        project_file::save(&ext, &trazo_path).unwrap();

        let updated = mgr.sync_live(&host.xrefs, true);
        assert_eq!(updated, vec!["Base".to_owned()]);
        assert_eq!(mgr.get(id_trazo).unwrap().project.walls.len(), 2);
        assert_eq!(mgr.get(id_trazo).unwrap().reload_count, 2);

        let _ = fs::remove_file(&trazo_path);
        let _ = fs::remove_file(&dxf_path);
        let _ = fs::remove_dir(&dir);
    }
}
