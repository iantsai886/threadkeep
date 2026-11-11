//! Adds Threadkeep to an AI app's own MCP settings file, and removes it again.
//! Every change keeps a timestamped copy of the previous file, and a file that cannot be
//! understood is left untouched rather than rewritten.

use crate::home::{atomic_write, now_iso};
use crate::{bail, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const SERVER_KEY: &str = "threadkeep";
const BEGIN: &str = "# >>> threadkeep (managed block; edit through Threadkeep)";
const END: &str = "# <<< threadkeep";

/// Where the user's own folders are. Passed in so tests can use an invented home.
#[derive(Debug, Clone)]
pub struct UserEnv {
    pub home: PathBuf,
    /// "windows", "macos" or "linux".
    pub platform: String,
    pub appdata: Option<PathBuf>,
    pub xdg_config: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
}

impl UserEnv {
    pub fn from_system() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).map(PathBuf::from);
        UserEnv {
            home: var("USERPROFILE").or_else(|| var("HOME")).unwrap_or_else(|| PathBuf::from(".")),
            platform: if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" }.to_string(),
            appdata: var("APPDATA"),
            xdg_config: var("XDG_CONFIG_HOME"),
            codex_home: var("CODEX_HOME"),
        }
    }
}

pub struct Client {
    pub id: &'static str,
    pub title: &'static str,
    pub toml: bool,
    /// What the user must do before the app sees the change. Shown as it is.
    pub reload: &'static str,
}

pub const CLIENTS: [Client; 6] = [
    Client { id: "claude-desktop", title: "Claude Desktop", toml: false, reload: "Quit Claude Desktop completely (also from the system tray or menu bar), open it again and start a new chat." },
    Client { id: "claude-code", title: "Claude Code", toml: false, reload: "Start a new Claude Code session, then run /mcp to check." },
    Client { id: "cursor", title: "Cursor", toml: false, reload: "Open Cursor Settings, MCP, and switch Threadkeep on if it is off." },
    Client { id: "codex", title: "Codex CLI", toml: true, reload: "Start a new Codex session." },
    Client { id: "gemini", title: "Gemini CLI", toml: false, reload: "Start a new Gemini CLI session, then run /mcp to check." },
    Client { id: "windsurf", title: "Windsurf", toml: false, reload: "Refresh MCP servers in Windsurf settings." },
];

pub fn client(id: &str) -> Option<&'static Client> {
    CLIENTS.iter().find(|c| c.id == id)
}

pub fn client_ids() -> String {
    CLIENTS.iter().map(|c| c.id).collect::<Vec<_>>().join(", ")
}

pub fn settings_file(c: &Client, u: &UserEnv) -> PathBuf {
    let h = &u.home;
    match c.id {
        "claude-desktop" => match u.platform.as_str() {
            "macos" => h.join("Library").join("Application Support").join("Claude"),
            "windows" => u.appdata.clone().unwrap_or_else(|| h.join("AppData").join("Roaming")).join("Claude"),
            _ => u.xdg_config.clone().unwrap_or_else(|| h.join(".config")).join("Claude"),
        }
        .join("claude_desktop_config.json"),
        "claude-code" => h.join(".claude.json"),
        "cursor" => h.join(".cursor").join("mcp.json"),
        "codex" => u.codex_home.clone().unwrap_or_else(|| h.join(".codex")).join("config.toml"),
        "gemini" => h.join(".gemini").join("settings.json"),
        _ => h.join(".codeium").join("windsurf").join("mcp_config.json"),
    }
}

/// The command an AI app runs. Absolute paths, because desktop apps do not inherit a shell PATH.
#[derive(Debug, Clone)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
}

/// The program AI apps should launch, given the path of the running program. When the running
/// program is the desktop app (or anything not called `threadkeep`), AI apps must still start the
/// small `threadkeep` command-line program that sits beside it (or in `../Resources` on macOS),
/// never the desktop window.
pub fn cli_beside(current: &Path) -> PathBuf {
    let name = if cfg!(windows) { "threadkeep.exe" } else { "threadkeep" };
    let stem = current.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    if stem == "threadkeep" {
        return current.to_path_buf();
    }
    let dir = current.parent().unwrap_or(Path::new("."));
    [dir.join(name), dir.join("..").join("Resources").join(name)].into_iter().find(|p| p.is_file()).unwrap_or_else(|| current.to_path_buf())
}

impl Launch {
    /// The `threadkeep` program, started as `threadkeep mcp`.
    pub fn this_program() -> Self {
        let exe = std::env::current_exe().map(|p| cli_beside(&p).to_string_lossy().into_owned()).unwrap_or_else(|_| "threadkeep".into());
        Launch { command: exe, args: vec!["mcp".into()] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_apps_launch_the_command_line_program_not_the_desktop_window() {
        let dir = std::env::temp_dir().join(format!("threadkeep-launch-{}", crate::home::random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let cli = dir.join(if cfg!(windows) { "threadkeep.exe" } else { "threadkeep" });
        let desktop = dir.join(if cfg!(windows) { "threadkeep-desktop.exe" } else { "threadkeep-desktop" });
        assert_eq!(cli_beside(&cli), cli, "the command-line program launches itself");
        assert_eq!(cli_beside(&desktop), desktop, "no sibling present: falls back to the running program");
        std::fs::write(&cli, "x").unwrap();
        assert_eq!(cli_beside(&desktop), cli, "desktop app points AI apps at the sibling command-line program");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

pub fn server_entry(token: &str, app_home: &Path, launch: &Launch) -> Value {
    json!({ "command": launch.command, "args": launch.args, "env": { "THREADKEEP_TOKEN": token, "THREADKEEP_HOME": app_home.to_string_lossy() } })
}

fn has_managed_entry(c: &Client, raw: &str) -> bool {
    if c.toml {
        raw.contains(BEGIN)
    } else {
        serde_json::from_str::<Value>(raw).map(|j| j["mcpServers"][SERVER_KEY].is_object()).unwrap_or(false)
    }
}

pub fn detect_clients(u: &UserEnv) -> Value {
    let mut out = Map::new();
    for c in &CLIENTS {
        let file = settings_file(c, u);
        let present = file.exists() || file.parent().map_or(false, Path::exists);
        let configured = fs::read_to_string(&file).map(|raw| has_managed_entry(c, &raw)).unwrap_or(false);
        out.insert(c.id.into(), json!({ "id": c.id, "title": c.title, "file": file.to_string_lossy(), "present": present, "configured": configured, "reload": c.reload }));
    }
    Value::Object(out)
}

fn backup(file: &Path) -> Result<Option<String>> {
    if !file.exists() {
        return Ok(None);
    }
    let dest = PathBuf::from(format!("{}.threadkeep-backup-{}", file.display(), now_iso().replace([':', '.'], "-")));
    fs::copy(file, &dest)?;
    Ok(Some(dest.to_string_lossy().into_owned()))
}

fn strip_toml_block(raw: &str) -> Result<String> {
    let Some(a) = raw.find(BEGIN) else { return Ok(raw.to_string()) };
    let Some(b) = raw[a..].find(END) else { bail!("The Threadkeep block in this file is damaged. Fix or remove it by hand, then try again.") };
    let mut out = format!("{}{}", &raw[..a], &raw[a + b + END.len()..]);
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    Ok(out)
}

fn toml_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

fn toml_block(token: &str, app_home: &Path, launch: &Launch) -> String {
    format!(
        "{BEGIN}\n[mcp_servers.{SERVER_KEY}]\ncommand = {}\nargs = [{}]\n\n[mcp_servers.{SERVER_KEY}.env]\nTHREADKEEP_TOKEN = {}\nTHREADKEEP_HOME = {}\n{END}",
        toml_str(&launch.command),
        launch.args.iter().map(|a| toml_str(a)).collect::<Vec<_>>().join(", "),
        toml_str(token),
        toml_str(&app_home.to_string_lossy())
    )
}

fn has_foreign_toml_entry(text: &str) -> bool {
    let header = format!("[mcp_servers.{SERVER_KEY}]");
    text.lines().any(|l| l.trim() == header)
}

pub fn connect_client(id: &str, token: &str, app_home: &Path, launch: &Launch, u: &UserEnv) -> Result<Value> {
    let Some(c) = client(id) else { bail!("Unknown app \"{id}\". Known: {}, or \"custom\".", client_ids()) };
    let file = settings_file(c, u);
    let dir = file.parent().unwrap_or(Path::new("."));
    if !file.exists() && !dir.exists() {
        bail!("{} does not appear to be installed on this computer (no settings folder at {}).", c.title, dir.display());
    }
    let raw = fs::read_to_string(&file).unwrap_or_default();
    let next = if c.toml {
        let rest = strip_toml_block(&raw)?;
        if has_foreign_toml_entry(&rest) {
            bail!("{} already has a \"{SERVER_KEY}\" entry that Threadkeep did not write. Remove it by hand, then try again.", c.title);
        }
        let sep = if rest.trim().is_empty() { "" } else { "\n\n" };
        format!("{}{sep}{}\n", rest.trim_end(), toml_block(token, app_home, launch))
    } else {
        let mut json: Value = if raw.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
                Ok(v) => v,
                Err(_) => bail!("{}'s settings file could not be read as JSON, so it was left untouched: {}", c.title, file.display()),
            }
        };
        let Some(obj) = json.as_object_mut() else { bail!("{}'s settings file has an unexpected shape, so it was left untouched: {}", c.title, file.display()) };
        if !obj.get("mcpServers").map_or(false, Value::is_object) {
            obj.insert("mcpServers".into(), json!({}));
        }
        obj["mcpServers"][SERVER_KEY] = server_entry(token, app_home, launch);
        serde_json::to_string_pretty(&json)? + "\n"
    };
    let saved = backup(&file)?;
    atomic_write(&file, next.as_bytes())?;
    Ok(json!({ "client": id, "title": c.title, "file": file.to_string_lossy(), "backup": saved, "reload": c.reload }))
}

pub fn disconnect_client(id: &str, u: &UserEnv) -> Result<Value> {
    let Some(c) = client(id) else { bail!("Unknown app \"{id}\".") };
    let file = settings_file(c, u);
    let Ok(raw) = fs::read_to_string(&file) else { return Ok(json!({ "client": id, "removed": false })) };
    let next = if c.toml {
        if !raw.contains(BEGIN) {
            return Ok(json!({ "client": id, "removed": false }));
        }
        format!("{}\n", strip_toml_block(&raw)?.trim_end())
    } else {
        let mut json: Value = match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
            Ok(v) => v,
            Err(_) => bail!("{}'s settings file could not be read as JSON, so it was left untouched: {}", c.title, file.display()),
        };
        match json.get_mut("mcpServers").and_then(Value::as_object_mut) {
            Some(servers) if servers.contains_key(SERVER_KEY) => {
                servers.remove(SERVER_KEY);
            }
            _ => return Ok(json!({ "client": id, "removed": false })),
        }
        serde_json::to_string_pretty(&json)? + "\n"
    };
    let saved = backup(&file)?;
    atomic_write(&file, next.as_bytes())?;
    Ok(json!({ "client": id, "removed": true, "file": file.to_string_lossy(), "backup": saved }))
}

/// For apps Threadkeep does not know: the exact entry to paste into that app's MCP settings.
pub fn custom_snippet(token: &str, app_home: &Path, launch: &Launch) -> String {
    serde_json::to_string_pretty(&json!({ "mcpServers": { SERVER_KEY: server_entry(token, app_home, launch) } })).unwrap_or_default()
}
