//! A public HTTPS address for the local MCP server, for assistants that run
//! in the cloud (ChatGPT, Claude on the web). It uses a free Cloudflare
//! "quick tunnel": `cloudflared` prints a random https://….trycloudflare.com
//! address that forwards to this computer while it runs.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine as _;

use super::server::TunnelToken;

#[derive(Clone, Debug, PartialEq)]
pub enum TunnelState {
    Starting,
    /// The public address, without the `/mcp/<token>` path.
    Ready(String),
    Failed(String),
}

/// A running `cloudflared`; it stops and revokes its secret token when dropped.
pub struct Tunnel {
    child: Child,
    state: Arc<Mutex<TunnelState>>,
    token: String,
    shared_token: TunnelToken,
}

impl Tunnel {
    /// Starts a quick tunnel to `127.0.0.1:port` and activates a fresh secret
    /// token on `shared_token` so only holders of the full URL can call MCP.
    pub fn start(port: u16, shared_token: TunnelToken) -> Result<Self, String> {
        let program = find_cloudflared().ok_or(
            "O cloudflared não está instalado. Clique em Instalar cloudflared e depois em Abrir túnel.",
        )?;
        let mut command = Command::new(program);
        command
            .args([
                "tunnel",
                "--no-autoupdate",
                "--url",
                &format!("http://127.0.0.1:{port}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| format!("Não foi possível iniciar o cloudflared ({error})."))?;
        let token = generate_token();
        shared_token.set(Some(token.clone()));
        let state = Arc::new(Mutex::new(TunnelState::Starting));
        // cloudflared logs to stderr; read both streams so neither fills up.
        let streams: Vec<Box<dyn std::io::Read + Send>> = [
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .collect();
        for stream in streams {
            let state = Arc::clone(&state);
            std::thread::spawn(move || watch_output(stream, &state));
        }
        Ok(Self {
            child,
            state,
            token,
            shared_token,
        })
    }

    /// Full connector address including the per-session secret token.
    pub fn connector_url(&self, base_address: &str) -> String {
        format!("{}/mcp/{}", base_address.trim_end_matches('/'), self.token)
    }

    pub fn state(&mut self) -> TunnelState {
        let state = self
            .state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_else(|_| TunnelState::Failed("estado do túnel indisponível".to_owned()));
        // A tunnel that stopped on its own is reported, not shown as ready.
        if let Ok(Some(status)) = self.child.try_wait()
            && !matches!(state, TunnelState::Failed(_))
        {
            self.shared_token.clear();
            let failed = TunnelState::Failed(format!(
                "O cloudflared foi encerrado ({status}). Verifique a conexão com a internet e tente de novo."
            ));
            if let Ok(mut shared) = self.state.lock() {
                *shared = failed.clone();
            }
            return failed;
        }
        if matches!(state, TunnelState::Failed(_)) {
            self.shared_token.clear();
        }
        state
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.shared_token.clear();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Generates a URL-safe random token (192 bits of entropy, 32 characters).
pub fn generate_token() -> String {
    let mut bytes = [0u8; 24];
    if !fill_os_random(&mut bytes) {
        for chunk in bytes.chunks_mut(8) {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_usize(chunk.as_ptr() as usize);
            let value = hasher.finish().to_le_bytes();
            chunk.copy_from_slice(&value[..chunk.len()]);
        }
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(windows)]
fn fill_os_random(buffer: &mut [u8]) -> bool {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            algorithm: *mut core::ffi::c_void,
            buffer: *mut u8,
            count: u32,
            flags: u32,
        ) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    let Ok(len) = u32::try_from(buffer.len()) else {
        return false;
    };
    // SAFETY: `buffer` is valid for `len` writable bytes and `algorithm` is null
    // as required by BCRYPT_USE_SYSTEM_PREFERRED_RNG.
    let status = unsafe {
        BCryptGenRandom(
            core::ptr::null_mut(),
            buffer.as_mut_ptr(),
            len,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    status == 0
}

#[cfg(unix)]
fn fill_os_random(buffer: &mut [u8]) -> bool {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(buffer))
        .is_ok()
}

#[cfg(not(any(windows, unix)))]
fn fill_os_random(_buffer: &mut [u8]) -> bool {
    false
}

fn watch_output(stream: Box<dyn std::io::Read + Send>, state: &Mutex<TunnelState>) {
    let mut recent = Vec::new();
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        if let Some(url) = tunnel_url(&line) {
            if let Ok(mut state) = state.lock() {
                *state = TunnelState::Ready(url);
            }
            continue;
        }
        recent.push(line);
        if recent.len() > 4 {
            recent.remove(0);
        }
    }
    // The stream ended: the process stopped. Keep a hint of why.
    if let Ok(mut state) = state.lock()
        && *state == TunnelState::Starting
    {
        *state = TunnelState::Failed(format!(
            "O cloudflared parou sem criar o túnel. {}",
            recent.join(" ")
        ));
    }
}

/// The quick-tunnel address in a line of cloudflared's log, if any.
pub fn tunnel_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let url: String = line[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '|' && *c != '"')
        .collect();
    let host = url.trim_start_matches("https://");
    (host.ends_with(".trycloudflare.com") && host.len() > ".trycloudflare.com".len()).then_some(url)
}

/// `cloudflared` on the PATH or where its installers put it.
pub fn find_cloudflared() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "cloudflared.exe"
    } else {
        "cloudflared"
    };
    let on_path: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(name))
                .collect()
        })
        .unwrap_or_default();
    let mut known = Vec::new();
    if cfg!(windows) {
        for folder in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(base) = std::env::var_os(folder) {
                known.push(PathBuf::from(base).join("cloudflared").join(name));
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let winget = PathBuf::from(local).join("Microsoft").join("WinGet");
            known.push(winget.join("Links").join(name));
            // winget may not have refreshed this app's PATH yet.
            if let Ok(entries) = std::fs::read_dir(winget.join("Packages")) {
                known.extend(
                    entries
                        .flatten()
                        .filter(|entry| {
                            entry
                                .file_name()
                                .to_string_lossy()
                                .starts_with("Cloudflare.cloudflared")
                        })
                        .map(|entry| entry.path().join(name)),
                );
            }
        }
    }
    on_path.into_iter().chain(known).find(|path| path.is_file())
}

/// Installs cloudflared with winget in a visible console, so the person sees
/// the progress and any question the installer asks.
pub fn install_cloudflared() -> Result<(), String> {
    if !cfg!(windows) {
        return Err(
            "Instale o cloudflared pelo gerenciador de pacotes do sistema (veja developers.cloudflare.com)."
                .to_owned(),
        );
    }
    Command::new("cmd")
        .args([
            "/C",
            "start",
            "Instalar cloudflared",
            "cmd",
            "/K",
            "winget install --id Cloudflare.cloudflared -e --accept-source-agreements --accept-package-agreements && echo. && echo Pronto. Volte ao Trazo e clique em Abrir tunel.",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Não foi possível abrir o instalador ({error})."))
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_address_in_cloudflared_logs() {
        let banner = "2026-11-19T10:00:00Z INF |  https://quiet-river-lamp-moon.trycloudflare.com                          |";
        assert_eq!(
            tunnel_url(banner).as_deref(),
            Some("https://quiet-river-lamp-moon.trycloudflare.com")
        );
        assert_eq!(
            tunnel_url("INF Requesting new quick Tunnel on trycloudflare.com..."),
            None
        );
        assert_eq!(
            tunnel_url("INF Visit https://developers.cloudflare.com/ for docs"),
            None
        );
        assert_eq!(tunnel_url("https://.trycloudflare.com"), None);
    }

    #[test]
    fn a_dropped_watch_without_an_address_reports_failure() {
        let state = Mutex::new(TunnelState::Starting);
        let log = "ERR failed to connect\nERR giving up\n";
        watch_output(
            Box::new(std::io::Cursor::new(log.as_bytes().to_vec())),
            &state,
        );
        match state.into_inner().unwrap() {
            TunnelState::Failed(message) => assert!(message.contains("giving up")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_address_in_the_log_makes_the_tunnel_ready() {
        let state = Mutex::new(TunnelState::Starting);
        let log = "INF starting\nINF |  https://a-b-c.trycloudflare.com  |\nINF registered\n";
        watch_output(
            Box::new(std::io::Cursor::new(log.as_bytes().to_vec())),
            &state,
        );
        assert_eq!(
            state.into_inner().unwrap(),
            TunnelState::Ready("https://a-b-c.trycloudflare.com".to_owned())
        );
    }

    #[test]
    fn generated_tokens_are_url_safe_and_unique() {
        let first = generate_token();
        let second = generate_token();
        assert_eq!(first.len(), 32);
        assert_ne!(first, second);
        assert!(
            first
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
    }
}
