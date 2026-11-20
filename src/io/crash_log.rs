//! Crash logging and problem-reporting helpers (#79).
//!
//! Installs a panic hook that writes diagnostics to a rotating log file under
//! `<data_dir>/logs/crash.log` and builds pre-filled GitHub bug-report URLs
//! matching `.github/ISSUE_TEMPLATE/bug_report.yml`.

use std::{
    backtrace::Backtrace,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as _,
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

const ISSUE_NEW_URL: &str = "https://github.com/igors93/Trazo/issues/new";
const MAX_LOG_BYTES: u64 = 256 * 1024;
const MAX_ARCHIVES: usize = 3;
const MAX_URL_LOG_CHARS: usize = 2_400;

static LOG_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Version from `Cargo.toml`, formatted for display and issue templates.
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Human-readable operating system and architecture summary.
pub fn os_summary() -> String {
    let os = match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        other => other,
    };
    format!("{os} ({})", std::env::consts::ARCH)
}

/// Directory where Trazo stores rotating crash logs.
pub fn log_dir() -> PathBuf {
    super::data_dir().join("logs")
}

/// Active crash log file path.
pub fn crash_log_path() -> PathBuf {
    log_dir().join("crash.log")
}

/// Installs a global panic hook that records crash reports to the rotating
/// crash log before invoking the previously registered hook.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = format_panic_info(info);
        let _ = write_entry_locked(&crash_log_path(), &report, MAX_LOG_BYTES, MAX_ARCHIVES);
        previous(info);
    }));
}

fn format_panic_info(info: &PanicHookInfo<'_>) -> String {
    let payload = if let Some(text) = info.payload().downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = info.payload().downcast_ref::<String>() {
        text.clone()
    } else {
        "panic sem mensagem textual".to_owned()
    };
    let location = info
        .location()
        .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned();
    let backtrace = Backtrace::force_capture().to_string();
    format_crash_entry(
        unix_timestamp_secs(),
        &thread_name,
        &payload,
        location.as_deref(),
        &backtrace,
    )
}

fn format_crash_entry(
    timestamp_secs: u64,
    thread_name: &str,
    message: &str,
    location: Option<&str>,
    backtrace: &str,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== Trazo crash report ===");
    let _ = writeln!(out, "timestamp_unix: {timestamp_secs}");
    let _ = writeln!(out, "version: v{}", app_version());
    let _ = writeln!(out, "os: {}", os_summary());
    let _ = writeln!(out, "thread: {thread_name}");
    if let Some(loc) = location {
        let _ = writeln!(out, "location: {loc}");
    }
    let _ = writeln!(out, "message: {message}");
    let trimmed_bt = backtrace.trim();
    if !trimmed_bt.is_empty() {
        let _ = writeln!(out, "backtrace:\n{trimmed_bt}");
    }
    let _ = writeln!(out);
    out
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn archive_path(path: &Path, index: usize) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("crash");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("log");
    path.with_file_name(format!("{stem}.{index}.{ext}"))
}

fn write_entry_locked(
    path: &Path,
    entry: &str,
    max_bytes: u64,
    max_archives: usize,
) -> Result<(), String> {
    let _guard = LOG_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    append_rotating(path, entry, max_bytes, max_archives)
}

/// Appends `entry` to `path`, rotating `path -> path.1 -> ... -> path.<max_archives>`
/// whenever the current file would exceed `max_bytes`.
pub fn append_rotating(
    path: &Path,
    entry: &str,
    max_bytes: u64,
    max_archives: usize,
) -> Result<(), String> {
    let parent = path.parent().ok_or("Caminho de log inválido")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let existing_len = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if existing_len > 0 && existing_len.saturating_add(entry.len() as u64) > max_bytes {
        rotate_files(path, max_archives)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(entry.as_bytes())
        .map_err(|error| error.to_string())
}

fn rotate_files(path: &Path, max_archives: usize) -> Result<(), String> {
    if max_archives == 0 {
        let _ = fs::remove_file(path);
        return Ok(());
    }

    let oldest = archive_path(path, max_archives);
    if oldest.exists() {
        let _ = fs::remove_file(&oldest);
    }

    for index in (1..max_archives).rev() {
        let from = archive_path(path, index);
        let to = archive_path(path, index + 1);
        if from.exists() {
            fs::rename(&from, &to).map_err(|error| error.to_string())?;
        }
    }

    if path.exists() {
        fs::rename(path, archive_path(path, 1)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Returns true when the active crash log exists and is non-empty.
pub fn has_crash_log() -> bool {
    fs::metadata(crash_log_path())
        .map(|meta| meta.len() > 0)
        .unwrap_or(false)
}

/// Reads up to the last `max_chars` characters from the active crash log.
pub fn read_recent_log(max_chars: usize) -> Option<String> {
    read_recent_from(&crash_log_path(), max_chars)
}

fn read_recent_from(path: &Path, max_chars: usize) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= max_chars {
        return Some(trimmed.to_owned());
    }
    let keep = max_chars.saturating_sub(4);
    let tail: String = trimmed
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Some(format!("...\n{tail}"))
}

/// Deletes the active crash log and all rotated archives.
pub fn clear_crash_logs() -> Result<(), String> {
    clear_logs_at(&crash_log_path(), MAX_ARCHIVES)
}

fn clear_logs_at(path: &Path, max_archives: usize) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    for index in 1..=max_archives {
        let archived = archive_path(path, index);
        if archived.exists() {
            fs::remove_file(&archived).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Builds a GitHub issue URL pre-filled with Trazo version, OS and optional
/// crash log excerpt, targeting `.github/ISSUE_TEMPLATE/bug_report.yml`.
pub fn build_bug_report_url(summary: &str, crash_excerpt: Option<&str>) -> String {
    let mut params: Vec<(&str, String)> = vec![
        ("template", "bug_report.yml".to_owned()),
        ("version", format!("v{}", app_version())),
        ("os", os_summary()),
    ];

    let trimmed_summary = summary.trim();
    if !trimmed_summary.is_empty() {
        params.push(("what-happened", trimmed_summary.to_owned()));
    }

    if let Some(log) = crash_excerpt.map(str::trim).filter(|s| !s.is_empty()) {
        let bounded = truncate_tail(log, MAX_URL_LOG_CHARS);
        params.push(("evidence", format!("```\n{bounded}\n```")));
    }

    let query = params
        .into_iter()
        .map(|(key, value)| format!("{key}={}", percent_encode(&value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{ISSUE_NEW_URL}?{query}")
}

fn truncate_tail(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let keep = max_chars.saturating_sub(4);
    let tail: String = text
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("...\n{tail}")
}

/// Plain-text diagnostic report ready to paste into an issue or support message.
pub fn format_diagnostic_bundle(summary: &str, crash_excerpt: Option<&str>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Trazo: v{}", app_version());
    let _ = writeln!(out, "Sistema: {}", os_summary());
    let _ = writeln!(out, "Log: {}", crash_log_path().display());
    let trimmed_summary = summary.trim();
    if !trimmed_summary.is_empty() {
        let _ = writeln!(out, "\nDescrição:\n{trimmed_summary}");
    }
    if let Some(log) = crash_excerpt.map(str::trim).filter(|s| !s.is_empty()) {
        let _ = writeln!(out, "\nÚltimo registro de falha:\n```\n{log}\n```");
    }
    out
}

/// Opens the crash log folder in the operating system's file manager.
pub fn open_log_folder() -> Result<(), String> {
    let dir = log_dir();
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    open_path_in_os(&dir)
}

fn open_path_in_os(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("explorer");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Não foi possível abrir a pasta de logs: {error}"))
}

fn percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("trazo-crash-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rotation_moves_existing_logs_and_bounds_archive_count() {
        let dir = temp_log_dir("rotation");
        let path = dir.join("crash.log");

        for i in 1..=6 {
            let entry = format!("entry-{i}: 0123456789abcdef\n");
            append_rotating(&path, &entry, 30, 2).unwrap();
        }

        let current = fs::read_to_string(&path).unwrap();
        let archive_1 = fs::read_to_string(archive_path(&path, 1)).unwrap();
        let archive_2 = fs::read_to_string(archive_path(&path, 2)).unwrap();
        assert!(current.contains("entry-6"));
        assert!(archive_1.contains("entry-5"));
        assert!(archive_2.contains("entry-4"));
        assert!(!archive_path(&path, 3).exists());

        clear_logs_at(&path, 2).unwrap();
        assert!(!path.exists());
        assert!(!archive_path(&path, 1).exists());
        assert!(!archive_path(&path, 2).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_recent_truncates_utf8_safely() {
        let dir = temp_log_dir("utf8");
        let path = dir.join("crash.log");
        let entry = "ação em cômodo — parede não fechou corretamente no pavimento térreo";
        append_rotating(&path, entry, 1024, 2).unwrap();

        let excerpt = read_recent_from(&path, 24).unwrap();
        assert!(excerpt.starts_with("...\n"));
        assert!(excerpt.chars().count() <= 24);
        assert!(excerpt.ends_with("térreo"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bug_report_url_encodes_fields_matching_github_issue_template() {
        let url = build_bug_report_url(
            "Porta sumiu após mover parede",
            Some("panic at src/domain/model.rs:42"),
        );
        assert!(url.starts_with(ISSUE_NEW_URL));
        assert!(url.contains("template=bug_report.yml"));
        assert!(url.contains(&format!("version=v{}", app_version())));
        assert!(url.contains("os="));
        assert!(url.contains("what-happened=Porta%20sumiu%20ap%C3%B3s%20mover%20parede"));
        assert!(url.contains(
            "evidence=%60%60%60%0Apanic%20at%20src%2Fdomain%2Fmodel.rs%3A42%0A%60%60%60"
        ));
    }

    #[test]
    fn diagnostic_bundle_contains_version_os_and_excerpt() {
        let bundle = format_diagnostic_bundle("Erro ao exportar", Some("panic: boom"));
        assert!(bundle.contains(app_version()));
        assert!(bundle.contains(&os_summary()));
        assert!(bundle.contains("Erro ao exportar"));
        assert!(bundle.contains("panic: boom"));
    }
}
