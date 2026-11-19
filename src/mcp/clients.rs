//! How each AI client connects to Trazo: ready-to-paste commands and
//! configuration, and the one-click setup of Claude Desktop.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The key Trazo uses in a client's list of MCP servers.
const SERVER_KEY: &str = "trazo";

/// Claude Code: one command adds the local HTTP server.
pub fn claude_code_command(url: &str) -> String {
    format!("claude mcp add --transport http {SERVER_KEY} {url}")
}

/// A public HTTPS address for clients that run in the cloud (ChatGPT,
/// Claude on the web), through a free Cloudflare quick tunnel.
pub fn tunnel_command(port: u16) -> String {
    format!("cloudflared tunnel --url http://127.0.0.1:{port}")
}

fn claude_desktop_entry(executable: &Path) -> Value {
    json!({
        "command": executable.display().to_string(),
        "args": ["--mcp-stdio"],
    })
}

/// The `claude_desktop_config.json` content that adds Trazo.
pub fn claude_desktop_snippet(executable: &Path) -> String {
    let config = json!({ "mcpServers": { SERVER_KEY: claude_desktop_entry(executable) } });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// Where Claude Desktop keeps its configuration on this computer.
pub fn claude_desktop_config_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Claude"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/Claude"))
    } else {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/Claude"))
    }
    .map(|folder| folder.join("claude_desktop_config.json"))
}

/// Adds (or updates) Trazo in Claude Desktop's configuration, keeping every
/// other setting and server. A file that is not valid JSON is left alone;
/// the previous file is kept as `.bak`.
pub fn install_claude_desktop(executable: &Path) -> Result<PathBuf, String> {
    let path = claude_desktop_config_path()
        .ok_or("Não foi possível localizar a pasta de configuração do Claude Desktop.")?;
    install_claude_desktop_at(&path, executable)?;
    Ok(path)
}

fn install_claude_desktop_at(path: &Path, executable: &Path) -> Result<(), String> {
    let mut config = if path.exists() {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("Não foi possível ler {}: {error}", path.display()))?;
        if content.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str::<Value>(&content).map_err(|_| {
                format!(
                    "{} não é um JSON válido; ele não foi alterado. Corrija-o ou cole a configuração manualmente.",
                    path.display()
                )
            })?
        }
    } else {
        json!({})
    };
    let root = config
        .as_object_mut()
        .ok_or("A configuração do Claude Desktop não é um objeto JSON; ela não foi alterada.")?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("\"mcpServers\" não é um objeto na configuração do Claude Desktop; ela não foi alterada.")?;
    servers.insert(SERVER_KEY.to_owned(), claude_desktop_entry(executable));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak")).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    fs::write(&temporary, content).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("trazo-clients-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn commands_and_snippet_point_at_trazo() {
        assert_eq!(
            claude_code_command("http://127.0.0.1:7799/mcp"),
            "claude mcp add --transport http trazo http://127.0.0.1:7799/mcp"
        );
        assert_eq!(
            tunnel_command(7799),
            "cloudflared tunnel --url http://127.0.0.1:7799"
        );
        let snippet: Value =
            serde_json::from_str(&claude_desktop_snippet(Path::new(r"C:\Apps\trazo.exe"))).unwrap();
        assert_eq!(
            snippet["mcpServers"]["trazo"]["command"],
            r"C:\Apps\trazo.exe"
        );
        assert_eq!(snippet["mcpServers"]["trazo"]["args"][0], "--mcp-stdio");
    }

    #[test]
    fn installing_keeps_other_settings_and_servers() {
        let folder = folder("merge");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("claude_desktop_config.json");
        fs::write(
            &path,
            r#"{ "theme": "light", "mcpServers": { "files": { "command": "files.exe" } } }"#,
        )
        .unwrap();
        install_claude_desktop_at(&path, Path::new("trazo.exe")).unwrap();
        let config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["theme"], "light");
        assert_eq!(config["mcpServers"]["files"]["command"], "files.exe");
        assert_eq!(config["mcpServers"]["trazo"]["args"][0], "--mcp-stdio");
        assert!(path.with_extension("json.bak").exists());

        // Installing again only updates Trazo's entry.
        install_claude_desktop_at(&path, Path::new("novo.exe")).unwrap();
        let config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["trazo"]["command"], "novo.exe");
        assert_eq!(config["mcpServers"].as_object().unwrap().len(), 2);
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_missing_file_is_created_and_a_broken_one_is_left_alone() {
        let folder = folder("create");
        let path = folder.join("Claude").join("claude_desktop_config.json");
        install_claude_desktop_at(&path, Path::new("trazo.exe")).unwrap();
        let config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["trazo"].is_object());

        fs::write(&path, "{ quebrado").unwrap();
        assert!(install_claude_desktop_at(&path, Path::new("trazo.exe")).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ quebrado");
        let _ = fs::remove_dir_all(folder);
    }
}
