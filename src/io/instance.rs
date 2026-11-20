//! Several Trazo windows at once, each editing its own project.
//!
//! Every window takes the first free *slot*: slot 0 is the first window,
//! slot 1 the second, and so on. A slot is held by an operating-system lock
//! on a small file, so it is released the moment the window closes or
//! crashes. The slot gives each window its own crash-recovery draft and its
//! own MCP port, so two windows never overwrite each other's draft and an AI
//! assistant can work on several projects at the same time.

use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::OnceLock;

/// How many windows can run at once with their own draft and port.
pub const MAX_SLOTS: u16 = 16;

struct Slot {
    number: u16,
    /// Keeps the lock for the life of the process.
    _lock: Option<File>,
}

static SLOT: OnceLock<Slot> = OnceLock::new();

fn lock_path(folder: &std::path::Path, number: u16) -> PathBuf {
    folder.join(format!("instance-{number}.lock"))
}

/// Takes the first slot whose lock file is free in `folder`.
fn claim(folder: &std::path::Path) -> Slot {
    if fs::create_dir_all(folder).is_ok() {
        for number in 0..MAX_SLOTS {
            let Ok(file) = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(lock_path(folder, number))
            else {
                continue;
            };
            if file.try_lock().is_ok() {
                return Slot {
                    number,
                    _lock: Some(file),
                };
            }
        }
    }
    // No lock could be taken (read-only folder, too many windows): fall back
    // to a slot from the process id so drafts still do not collide.
    Slot {
        number: MAX_SLOTS + (std::process::id() % 1000) as u16,
        _lock: None,
    }
}

/// This window's slot number (0 for the first window).
pub fn slot() -> u16 {
    SLOT.get_or_init(|| claim(&super::data_dir())).number
}

/// The file name of this window's crash-recovery draft. The first window
/// keeps the historical name, `recovery.json`.
pub fn recovery_file_name() -> String {
    match slot() {
        0 => "recovery.json".to_owned(),
        n => format!("recovery-{n}.json"),
    }
}

/// The MCP port for this window: the configured port for the first window,
/// the next ones for the others.
pub fn mcp_port(base: u16) -> u16 {
    base.saturating_add(slot().min(MAX_SLOTS))
}

/// Opens another Trazo window, with an empty project.
pub fn open_new_window() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Não foi possível abrir outra janela: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_window_takes_the_next_free_slot_and_frees_it_on_close() {
        let folder =
            std::env::temp_dir().join(format!("trazo-instance-test-{}", std::process::id()));
        let first = claim(&folder);
        let second = claim(&folder);
        assert_eq!((first.number, second.number), (0, 1));
        // Closing the first window frees slot 0 for the next one.
        drop(first);
        let third = claim(&folder);
        assert_eq!(third.number, 0);
        drop((second, third));
        let _ = fs::remove_dir_all(folder);
    }
}
