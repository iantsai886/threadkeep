//! Owner commands. `dispatch` is the single entry point: the command line calls it today and
//! the desktop app will call it with the same names and JSON (docs/CONTRACTS.md section 6).

use crate::access::{find_space, hash_token, new_connection_id, new_token, owner_context, preview_context, Context};
use crate::backup::{backup_space, list_backups, restore_backup, verify_backup};
use crate::connectors::{client, client_ids, connect_client, custom_snippet, detect_clients, disconnect_client, Launch, UserEnv};
use crate::home::{atomic_write, clear_seen, config_path, load_config, now_iso, read_activity, read_seen, save_config, Config, Connection, Grant, Space, LEVELS, MANAGED_DIR, PRODUCT, SPACE_KINDS, VERSION};
use crate::memory::{forget_memory, list_memories, retire_memory, save_memory, MemoryInput};
use crate::pending::{approve_pending, list_pending, reject_pending};
use crate::search::{ask, count_notes, open, AskArgs};
use crate::spaces::{space_exists, IGNORE_FILE};
use crate::threads::{list_threads, write_checkpoint, CheckpointInput};
use crate::{bail, mcp, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Ops {
    pub home: PathBuf,
    pub user: UserEnv,
    pub launch: Launch,
}

fn text(a: &Value, k: &str) -> String {
    a.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn opt(a: &Value, k: &str) -> Option<String> {
    Some(text(a, k)).filter(|s| !s.is_empty())
}

fn flag(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

fn need_space<'a>(cfg: &'a Config, name: &str) -> Result<&'a Space> {
    match find_space(cfg, name) {
        Some(s) => Ok(s),
        None => bail!("No space named \"{name}\"."),
    }
}

/// Lower-cased, forward-slash form with no trailing slash, for comparing folders.
fn folder_key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

impl Ops {
    pub fn from_system(home: PathBuf) -> Self {
        Ops { home, user: UserEnv::from_system(), launch: Launch::this_program() }
    }

    fn load(&self) -> Result<Config> {
        load_config(&self.home)
    }

    /// "~" and "~/..." mean the user's home folder.
    fn resolve(&self, p: &str) -> PathBuf {
        match p.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => self.user.home.join(rest.trim_start_matches(['/', '\\'])),
            _ => PathBuf::from(p),
        }
    }

    /// The space a command means: the named one, or the only one.
    fn space(&self, cfg: &Config, name: Option<String>) -> Result<Space> {
        match name {
            Some(n) => need_space(cfg, &n).cloned(),
            None if cfg.spaces.len() == 1 => Ok(cfg.spaces[0].clone()),
            None => bail!("Say which space: {}.", if cfg.spaces.is_empty() { "none set up yet".to_string() } else { cfg.spaces.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ") }),
        }
    }

    fn context(&self, cfg: &Config, as_connection: Option<String>) -> Result<Context> {
        match as_connection {
            Some(id) => preview_context(cfg, &self.home, &id),
            None => Ok(owner_context(cfg, &self.home)),
        }
    }

    // ---- spaces ----

    fn add_space(&self, a: &Value) -> Result<Value> {
        let Some(folder) = opt(a, "path") else { bail!("Choose a folder for the space.") };
        let full = self.resolve(&folder);
        if !full.is_absolute() {
            bail!("Give the full path of the folder.");
        }
        let kind = opt(a, "kind").unwrap_or_else(|| "personal".into());
        if !SPACE_KINDS.contains(&kind.as_str()) {
            bail!("kind must be one of: {}.", SPACE_KINDS.join(", "));
        }
        let mut cfg = self.load()?;
        let existed = match fs::metadata(&full) {
            Ok(m) if !m.is_dir() => bail!("That path is a file, not a folder."),
            Ok(_) => true,
            Err(_) => false,
        };
        if !existed && flag(a, "create") != Some(true) {
            bail!("Folder not found: {}. Use \"create\" to make a new empty knowledge base there.", full.display());
        }
        let key = folder_key(&full);
        for s in &cfg.spaces {
            let other = folder_key(Path::new(&s.path));
            if other == key || other.starts_with(&format!("{key}/")) || key.starts_with(&format!("{other}/")) {
                bail!("That folder overlaps the space \"{}\". Spaces must be separate folders so their contents never mix.", s.name);
            }
        }
        fs::create_dir_all(&full)?;
        if fs::read_dir(&full)?.next().is_none() {
            let welcome = format!("# Welcome\n\nThis folder is your knowledge base. It belongs to you: plain Markdown files you can read, edit, move or back up with any tool.\n\n- Write notes anywhere in this folder.\n- {PRODUCT} keeps what your AIs remember in the `{MANAGED_DIR}` folder, as plain files too.\n- To hide one note from every AI, put `private: true` at the top of it between two `---` lines.\n- To hide a whole folder, list it in `{IGNORE_FILE}`, one folder per line.\n");
            let _ = fs::write(full.join("Welcome.md"), welcome);
        }
        let name = opt(a, "name").unwrap_or_else(|| full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        // Personal and client spaces start sensitive: every AI write waits for the owner.
        let space = Space { name, path: full.to_string_lossy().into_owned(), sensitive: flag(a, "sensitive").unwrap_or(kind != "company"), kind };
        cfg.spaces.push(space.clone());
        save_config(&self.home, &cfg)?;
        Ok(json!({ "name": space.name, "path": space.path, "kind": space.kind, "sensitive": space.sensitive, "created_folder": !existed, "notes": count_notes(&space) }))
    }

    fn update_space(&self, a: &Value) -> Result<Value> {
        let mut cfg = self.load()?;
        let name = need_space(&cfg, &text(a, "name"))?.name.clone();
        let s = cfg.spaces.iter_mut().find(|s| s.name == name).unwrap();
        if let Some(kind) = opt(a, "kind") {
            if !SPACE_KINDS.contains(&kind.as_str()) {
                bail!("kind must be one of: {}.", SPACE_KINDS.join(", "));
            }
            s.kind = kind;
        }
        if let Some(sensitive) = flag(a, "sensitive") {
            s.sensitive = sensitive;
        }
        let out = serde_json::to_value(&*s)?;
        save_config(&self.home, &cfg)?;
        Ok(out)
    }

    /// Removing a space only forgets it in Threadkeep. The folder and its files are not touched.
    fn remove_space(&self, a: &Value) -> Result<Value> {
        let mut cfg = self.load()?;
        let s = need_space(&cfg, &text(a, "name"))?.clone();
        cfg.spaces.retain(|x| x.name != s.name);
        for c in cfg.connections.iter_mut() {
            c.grants.retain(|g| g.space.to_lowercase() != s.name.to_lowercase());
        }
        save_config(&self.home, &cfg)?;
        Ok(json!({ "removed": s.name, "folder_kept": s.path, "note": format!("\"{}\" was removed from {PRODUCT}. Its folder and every file in it were left as they are: {}", s.name, s.path) }))
    }

    // ---- connections ----

    fn grants(&self, cfg: &Config, a: &Value) -> Result<Vec<Grant>> {
        let list = a.get("grants").and_then(Value::as_array).cloned().unwrap_or_default();
        if list.is_empty() {
            bail!("Choose at least one space for this AI app.");
        }
        let mut out = vec![];
        for g in &list {
            let space = need_space(cfg, &text(g, "space"))?.name.clone();
            let level = opt(g, "level").unwrap_or_else(|| "propose".into());
            if !LEVELS.contains(&level.as_str()) {
                bail!("Access level must be one of: {}.", LEVELS.join(", "));
            }
            out.push(Grant { space, level });
        }
        Ok(out)
    }

    fn add_connection(&self, a: &Value) -> Result<Value> {
        let client_id = text(a, "client");
        let known = client(&client_id);
        if known.is_none() && client_id != "custom" {
            bail!("Unknown app \"{client_id}\". Known: {}, custom.", client_ids());
        }
        let mut cfg = self.load()?;
        let grants = self.grants(&cfg, a)?;
        let token = new_token();
        let name: String = opt(a, "name").unwrap_or_else(|| known.map_or("Custom AI", |c| c.title).to_string()).chars().take(80).collect();
        let connection = Connection { id: new_connection_id(), name, client: client_id.clone(), token_hash: hash_token(&token), created: now_iso(), revoked: false, grants: grants.clone() };
        // The app's settings are written first: if that is refused, no connection is left behind.
        let written = match known {
            Some(_) => Some(connect_client(&client_id, &token, &self.home, &self.launch, &self.user)?),
            None => None,
        };
        // One app holds one key. Connecting it again replaces the old key instead of leaving a stale one.
        let mut replaced = vec![];
        if known.is_some() {
            for c in cfg.connections.iter_mut().filter(|c| c.client == client_id && !c.revoked) {
                c.revoked = true;
                replaced.push(c.id.clone());
            }
        }
        cfg.connections.push(connection.clone());
        save_config(&self.home, &cfg)?;
        for id in &replaced {
            clear_seen(&self.home, id);
        }
        let mut out = json!({
            "id": connection.id, "name": connection.name, "client": client_id, "grants": grants, "status": "configured",
            "verify": format!("After that, ask the AI: \"What can you do in {PRODUCT}?\" This connection shows as verified once the app really calls {PRODUCT}."),
        });
        match written {
            Some(w) => {
                out["file"] = w["file"].clone();
                out["backup"] = w["backup"].clone();
                out["next"] = w["reload"].clone();
            }
            None => {
                // The key is shown once, here, and stored only as a fingerprint.
                out["token"] = json!(token);
                out["snippet"] = json!(custom_snippet(&token, &self.home, &self.launch));
                out["next"] = json!("Paste this entry into the app's MCP settings. The key is shown only once.");
            }
        }
        Ok(out)
    }

    fn set_grants(&self, a: &Value) -> Result<Value> {
        let mut cfg = self.load()?;
        let grants = self.grants(&cfg, a)?;
        let id = text(a, "id");
        let Some(c) = cfg.connections.iter_mut().find(|c| c.id == id && !c.revoked) else { bail!("Unknown connection.") };
        c.grants = grants.clone();
        save_config(&self.home, &cfg)?;
        Ok(json!({ "id": id, "grants": grants, "note": "Takes effect on the app's next request; no restart needed." }))
    }

    fn revoke_connection(&self, a: &Value) -> Result<Value> {
        let mut cfg = self.load()?;
        let id = text(a, "id");
        let Some(c) = cfg.connections.iter_mut().find(|c| !c.revoked && (c.id == id || c.client == id)) else { bail!("Unknown connection.") };
        c.revoked = true;
        let (cid, name, client_id) = (c.id.clone(), c.name.clone(), c.client.clone());
        save_config(&self.home, &cfg)?;
        clear_seen(&self.home, &cid);
        let known = client(&client_id).is_some();
        let removal = if known { Some(disconnect_client(&client_id, &self.user)) } else { None };
        let removed = matches!(&removal, Some(Ok(v)) if v["removed"] == true);
        let entry = if removed {
            " Its entry was removed from the app's settings (a copy of the previous file was kept).".to_string()
        } else if known {
            " Its entry could not be removed from the app's settings; the key no longer works either way.".to_string()
        } else {
            format!(" Remove the {PRODUCT} entry from that app's settings yourself; the key no longer works either way.")
        };
        let mut out = json!({ "revoked": cid, "name": name, "key_refused_from_now": true, "settings_entry_removed": removed, "note": format!("\"{name}\" can no longer use {PRODUCT}, starting with its next request.{entry}") });
        if let Some(Err(e)) = removal {
            out["warning"] = json!(e.0);
        }
        Ok(out)
    }

    fn connections(&self, cfg: &Config) -> Value {
        Value::Array(
            cfg.connections
                .iter()
                .filter(|c| !c.revoked)
                .map(|c| {
                    let seen = read_seen(&self.home, &c.id);
                    let mut out = json!({
                        "id": c.id, "name": c.name, "client": c.client, "grants": c.grants, "created": c.created,
                        "status": if seen.is_some() { "verified" } else { "configured" },
                        "first_call": seen.as_ref().map(|s| s["first"].clone()), "last_call": seen.as_ref().map(|s| s["last"].clone()),
                        "calls": seen.as_ref().and_then(|s| s["calls"].as_u64()).unwrap_or(0),
                    });
                    if seen.is_none() {
                        out["next"] = json!(client(&c.client).map_or("Add the entry to the app, then ask it to use Threadkeep.", |k| k.reload));
                    }
                    out
                })
                .collect(),
        )
    }

    fn state(&self) -> Result<Value> {
        let cfg = self.load()?;
        let spaces: Vec<Value> = cfg
            .spaces
            .iter()
            .map(|s| {
                let exists = space_exists(s);
                json!({ "name": s.name, "path": s.path, "kind": s.kind, "sensitive": s.sensitive, "exists": exists, "notes": if exists { json!(count_notes(s)) } else { Value::Null } })
            })
            .collect();
        Ok(json!({
            "product": PRODUCT, "version": VERSION, "settings_file": config_path(&self.home).to_string_lossy(), "spaces": spaces,
            "connections": self.connections(&cfg), "pending": list_pending(&self.home).len(), "apps": detect_clients(&self.user), "levels": LEVELS, "kinds": SPACE_KINDS,
        }))
    }

    /// Problems the owner can act on, each with a sentence saying how.
    fn doctor(&self) -> Value {
        let mut checks: Vec<Value> = vec![];
        let mut add = |ok: bool, what: String, fix: &str| checks.push(if ok { json!({ "ok": true, "what": what }) } else { json!({ "ok": false, "what": what, "fix": fix }) });
        let cfg = match self.load() {
            Ok(c) => {
                add(true, "Settings file is readable.".into(), "");
                c
            }
            Err(e) => {
                add(false, e.0, "Restore config.json from a copy, or move it aside to start fresh.");
                return json!({ "ok": false, "checks": checks });
            }
        };
        add(!cfg.spaces.is_empty(), format!("{} space(s) set up.", cfg.spaces.len()), "Add a space: threadkeep space add <folder> --create");
        for s in &cfg.spaces {
            add(space_exists(s), format!("Space \"{}\" folder is reachable.", s.name), &format!("Folder missing: {}. Reconnect the drive or sync folder, or remove the space.", s.path));
        }
        let apps = detect_clients(&self.user);
        for c in self.connections(&cfg).as_array().cloned().unwrap_or_default() {
            let name = c["name"].as_str().unwrap_or("");
            let client_id = c["client"].as_str().unwrap_or("");
            if c["grants"].as_array().map_or(true, Vec::is_empty) {
                add(false, format!("\"{name}\" has no space granted."), "Give it access to a space, or disconnect it.");
            }
            if client(client_id).is_some() {
                add(apps[client_id]["configured"] == true, format!("\"{name}\" entry is present in the app's settings."), &format!("The entry is missing from {}. Connect the app again.", apps[client_id]["file"].as_str().unwrap_or("")));
            }
            let verified = c["status"] == "verified";
            let what = if verified { format!("\"{name}\" has really called {PRODUCT} (last: {}).", c["last_call"].as_str().unwrap_or("")) } else { format!("\"{name}\" is set up but has not called {PRODUCT} yet.") };
            add(verified, what, c["next"].as_str().unwrap_or("Restart the app and start a new chat."));
        }
        let pending = list_pending(&self.home).len();
        if pending > 0 {
            add(false, format!("{pending} proposal(s) are waiting for your approval."), "Review them: threadkeep review");
        }
        let ok = checks.iter().all(|c| c["ok"] == true);
        json!({ "ok": ok, "checks": checks })
    }

    fn last_backup_file(&self) -> PathBuf {
        self.home.join("last-backup.json")
    }

    /// Run one owner command by its contract name.
    pub fn dispatch(&self, command: &str, a: &Value) -> Result<Value> {
        match command {
            "state" => self.state(),
            "doctor" => Ok(self.doctor()),
            "addSpace" => self.add_space(a),
            "updateSpace" => self.update_space(a),
            "removeSpace" => self.remove_space(a),
            "connections" => Ok(self.connections(&self.load()?)),
            "addConnection" => self.add_connection(a),
            "setGrants" => self.set_grants(a),
            "revokeConnection" => self.revoke_connection(a),
            "pending" => Ok(Value::Array(list_pending(&self.home))),
            "approve" => approve_pending(&self.home, &self.load()?, &text(a, "id")),
            "reject" => reject_pending(&self.home, &text(a, "id")),
            "ask" => {
                let ctx = self.context(&self.load()?, opt(a, "as"))?;
                ask(&ctx, &AskArgs { question: text(a, "question"), space: opt(a, "space"), limit: a.get("limit").and_then(Value::as_i64), history: flag(a, "history").unwrap_or(false) })
            }
            "open" => open(&self.context(&self.load()?, opt(a, "as"))?, &text(a, "ref")),
            // Run any AI tool exactly as one connection would, without writing anything.
            "preview" => {
                let Some(id) = opt(a, "as") else { bail!("Choose which connection to preview.") };
                mcp::run_tool(&preview_context(&self.load()?, &self.home, &id)?, &text(a, "tool"), a.get("args").unwrap_or(&json!({})))
            }
            "memories" => {
                let cfg = self.load()?;
                let spaces: Vec<Space> = match opt(a, "space") {
                    Some(n) => vec![need_space(&cfg, &n)?.clone()],
                    None => cfg.spaces.clone(),
                };
                let status = opt(a, "status");
                let mut out = vec![];
                for s in &spaces {
                    for r in list_memories(s, status.as_deref()) {
                        let mut v = serde_json::to_value(&r)?;
                        v["space"] = json!(s.name);
                        out.push(v);
                    }
                }
                Ok(Value::Array(out))
            }
            "remember" => {
                let space = self.space(&self.load()?, opt(a, "space"))?;
                let mut input: MemoryInput = serde_json::from_value(a.clone()).unwrap_or_default();
                input.by = "owner".into();
                save_memory(&space, &input)
            }
            "retire" => Ok(serde_json::to_value(retire_memory(&self.space(&self.load()?, opt(a, "space"))?, &text(a, "id"))?)?),
            "forget" => forget_memory(&self.space(&self.load()?, opt(a, "space"))?, &text(a, "id")),
            "threads" => {
                let cfg = self.load()?;
                let spaces: Vec<Space> = match opt(a, "space") {
                    Some(n) => vec![need_space(&cfg, &n)?.clone()],
                    None => cfg.spaces.clone(),
                };
                Ok(Value::Array(spaces.iter().flat_map(|s| list_threads(s, flag(a, "all").unwrap_or(false))).collect()))
            }
            "checkpoint" => {
                let space = self.space(&self.load()?, opt(a, "space"))?;
                let mut input: CheckpointInput = match serde_json::from_value(a.clone()) {
                    Ok(i) => i,
                    Err(_) => bail!("decisions, sources and open_questions must be lists of short lines."),
                };
                input.by = "owner".into();
                write_checkpoint(&space, &input)
            }
            "backup" => {
                let Some(dest) = opt(a, "dest") else { bail!("Choose a backup folder (absolute path).") };
                let folder = self.resolve(&dest);
                let result = backup_space(&self.space(&self.load()?, opt(a, "space"))?, &folder)?;
                // Remembered so the owner need not retype it.
                let _ = atomic_write(&self.last_backup_file(), json!({ "dir": folder.to_string_lossy() }).to_string().as_bytes());
                Ok(result)
            }
            "listBackups" => {
                let given = opt(a, "dir");
                let folder = match &given {
                    Some(d) => Some(self.resolve(d)),
                    None => fs::read_to_string(self.last_backup_file()).ok().and_then(|raw| serde_json::from_str::<Value>(&raw).ok()).and_then(|v| v["dir"].as_str().map(PathBuf::from)),
                };
                Ok(json!({
                    "dir": folder.as_ref().map(|f| f.to_string_lossy().into_owned()), "remembered": given.is_none() && folder.is_some(),
                    "backups": folder.as_ref().map(|f| list_backups(f)).unwrap_or_default(),
                }))
            }
            "verifyBackup" => Ok(verify_backup(&self.resolve(&text(a, "dir")))),
            "restore" => restore_backup(&self.resolve(&text(a, "dir")), &self.resolve(&text(a, "target"))),
            "activity" => Ok(Value::Array(read_activity(&self.home, a.get("n").and_then(Value::as_u64).unwrap_or(30).min(200) as usize))),
            "overhead" => Ok(mcp::overhead()),
            other => bail!("Unknown command \"{other}\"."),
        }
    }
}
