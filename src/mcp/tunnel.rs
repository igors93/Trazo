//! A public HTTPS address for the local MCP server, for assistants that run
//! in the cloud (ChatGPT, Claude on the web). It uses a free Cloudflare
//! "quick tunnel": `cloudflared` prints a random https://….trycloudflare.com
//! address that forwards to this computer while it runs.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum TunnelState {
    Starting,
    /// The public address, without the `/mcp` path.
    Ready(String),
    Failed(String),
}

/// A running `cloudflared`; it stops when this is dropped.
pub struct Tunnel {
    child: Child,
    state: Arc<Mutex<TunnelState>>,
}

impl Tunnel {
    /// Starts a quick tunnel to `127.0.0.1:port`.
    pub fn start(port: u16) -> Result<Self, String> {
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
        Ok(Self { child, state })
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
            let failed = TunnelState::Failed(format!(
                "O cloudflared foi encerrado ({status}). Verifique a conexão com a internet e tente de novo."
            ));
            if let Ok(mut shared) = self.state.lock() {
                *shared = failed.clone();
            }
            return failed;
        }
        state
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
}
