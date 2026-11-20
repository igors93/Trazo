//! Update check against GitHub Releases and startup file argument parsing (#63).

use std::{cmp::Ordering, ffi::OsString, path::PathBuf, process::Command, sync::mpsc, thread};

use serde::Deserialize;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/igors93/Trazo/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/igors93/Trazo/releases";

/// Parsed semantic version (`MAJOR.MINOR.PATCH[-PRE]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        let without_v = trimmed
            .strip_prefix('v')
            .or_else(|| trimmed.strip_prefix('V'))
            .unwrap_or(trimmed);
        // Strip optional build metadata (`+...`).
        let without_build = without_v.split('+').next()?;
        let (core, pre) = match without_build.split_once('-') {
            Some((core, pre)) if valid_prerelease(pre) => (core, Some(pre.to_owned())),
            Some(_) => return None,
            None => (without_build, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

fn valid_prerelease(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn compare_prerelease(left: &str, right: &str) -> Ordering {
    for (left, right) in left.split('.').zip(right.split('.')) {
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            // SemVer numeric identifiers have lower precedence than non-numeric ones.
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.split('.').count().cmp(&right.split('.').count())
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                // A stable release has higher precedence than a pre-release of the same version.
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_prerelease(a, b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Returns true when `candidate` is strictly newer than `current`.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (Version::parse(current), Version::parse(candidate)) {
        (Some(cur), Some(cand)) => cand > cur,
        _ => false,
    }
}

/// Metadata extracted from a GitHub Release response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub url: String,
    pub notes: String,
}

/// State of the update check displayed in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Failed(String),
}

#[derive(Deserialize)]
struct GithubReleasePayload {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    draft: bool,
}

/// Parses a GitHub `/releases/latest` JSON payload into a `ReleaseInfo`.
pub fn parse_latest_release(json: &str) -> Result<ReleaseInfo, String> {
    let payload: GithubReleasePayload =
        serde_json::from_str(json).map_err(|error| format!("Resposta inválida: {error}"))?;
    if payload.draft {
        return Err("Nenhuma versão publicada ainda.".to_owned());
    }
    let version = payload
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_owned();
    if Version::parse(&version).is_none() {
        return Err(format!("Tag de versão inválida: {}", payload.tag_name));
    }
    Ok(ReleaseInfo {
        tag: payload.tag_name,
        version,
        url: if payload.html_url.trim().is_empty() {
            RELEASES_PAGE_URL.to_owned()
        } else {
            payload.html_url
        },
        notes: payload.body.trim().to_owned(),
    })
}

/// Runs a non-blocking update check on a background thread and wakes the UI
/// when a result arrives.
pub fn spawn_check(
    current_version: &'static str,
    wake: impl Fn() + Send + 'static,
) -> mpsc::Receiver<UpdateStatus> {
    let (sender, receiver) = mpsc::channel();
    let _ = thread::Builder::new()
        .name("trazo-update-check".to_owned())
        .spawn(move || {
            let status = check_now(current_version);
            let _ = sender.send(status);
            wake();
        });
    receiver
}

fn check_now(current_version: &str) -> UpdateStatus {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--max-time",
            "8",
            "-H",
            &format!("User-Agent: Trazo/{current_version}"),
            "-H",
            "Accept: application/vnd.github+json",
            LATEST_RELEASE_API,
        ])
        .output();

    let output = match output {
        Ok(out) => out,
        Err(_) => {
            return UpdateStatus::Failed(
                "Não foi possível consultar atualizações neste computador.".to_owned(),
            );
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") {
            return UpdateStatus::UpToDate;
        }
        return UpdateStatus::Failed(
            "Sem conexão com o GitHub para verificar atualizações.".to_owned(),
        );
    }

    let body = String::from_utf8_lossy(&output.stdout);
    match parse_latest_release(&body) {
        Ok(release) if is_newer(current_version, &release.version) => {
            UpdateStatus::Available(release)
        }
        Ok(_) => UpdateStatus::UpToDate,
        Err(error) => UpdateStatus::Failed(error),
    }
}

/// Extracts an optional project file path passed on the command line
/// (for example when Windows opens a `.trazo` or legacy `.trazo.json` file by
/// double-clicking it). Flags starting with `-` are ignored.
pub fn parse_startup_file<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = OsString>,
{
    for arg in args.into_iter().skip(1) {
        let text = arg.to_string_lossy();
        if text.is_empty() || text.starts_with('-') {
            continue;
        }
        return Some(PathBuf::from(arg));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_handles_prefixes_and_prereleases() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("0.1.0-rc.1", "0.1.0"));
        assert!(is_newer("0.1.0-rc.2", "0.1.0-rc.10"));
        assert!(is_newer("0.1.0-alpha", "0.1.0-alpha.1"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0-beta.1"));
        assert!(!is_newer("0.1.0", "not-a-version"));
        assert!(!is_newer("0.1.0", "0.2.0-alpha..1"));
    }

    #[test]
    fn github_release_payload_parses_and_rejects_drafts() {
        let payload = r##"{
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/igors93/Trazo/releases/tag/v0.2.0",
            "body": "Added: Windows installer",
            "draft": false
        }"##;
        let info = parse_latest_release(payload).unwrap();
        assert_eq!(info.tag, "v0.2.0");
        assert_eq!(info.version, "0.2.0");
        assert_eq!(
            info.url,
            "https://github.com/igors93/Trazo/releases/tag/v0.2.0"
        );
        assert!(info.notes.contains("Windows installer"));

        let draft = r#"{ "tag_name": "v0.3.0", "html_url": "", "draft": true }"#;
        assert!(parse_latest_release(draft).is_err());
    }

    #[test]
    fn startup_file_parser_skips_flags_and_preserves_spaces() {
        let args = vec![
            OsString::from("trazo.exe"),
            OsString::from("--some-flag"),
            OsString::from(r"C:\Projetos\Casa da Praia.trazo"),
        ];
        assert_eq!(
            parse_startup_file(args),
            Some(PathBuf::from(r"C:\Projetos\Casa da Praia.trazo"))
        );

        let no_file = vec![OsString::from("trazo.exe"), OsString::from("--mcp-stdio")];
        assert_eq!(parse_startup_file(no_file), None);
    }
}
