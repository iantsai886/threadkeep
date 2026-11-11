//! Owner commands through `dispatch`: spaces, connecting apps (on invented settings files in a
//! temporary "user home"), backup and restore. Mirrors test/connectors.test.mjs and
//! test/backup.test.mjs. No real app settings are read or changed.
mod common;
use common::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use threadkeep_core::connectors::{settings_file, client, Launch, UserEnv};
use threadkeep_core::ops::Ops;

const CODEX_BEFORE: &str = "# my codex settings\nmodel = \"example-model\"\n\n[mcp_servers.other_tool]\ncommand = \"other\"\nargs = [\"serve\"]\n\n[mcp_servers.other_tool.env]\nOTHER_KEY = \"keep-me\"\n";

fn user_env(home: &Path, platform: &str) -> UserEnv {
    UserEnv { home: home.to_path_buf(), platform: platform.into(), appdata: None, xdg_config: None, codex_home: None }
}

fn ops(w: &World) -> (Ops, PathBuf) {
    let user = w.base.join("user");
    fs::create_dir_all(&user).unwrap();
    let o = Ops { home: w.home.clone(), user: user_env(&user, "linux"), launch: Launch { command: "threadkeep".into(), args: vec!["mcp".into()] } };
    (o, user)
}

fn read(user: &Path, rel: &str) -> String {
    fs::read_to_string(rel.split('/').fold(user.to_path_buf(), |p, part| p.join(part))).unwrap()
}

fn backups_of(user: &Path, dir: &str) -> Vec<String> {
    fs::read_dir(user.join(dir)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".threadkeep-backup-")).collect()
}

fn grants(level: &str) -> Value {
    json!([{ "space": "Northwind", "level": level }])
}

#[test]
fn json_app_connect_adds_one_entry_keeps_the_rest_and_the_key_works() {
    let w = world();
    let (o, user) = ops(&w);
    // Key order is deliberately not alphabetical: it must survive the edit.
    let before = "{\n  \"security\": {\n    \"auth\": {\n      \"selectedType\": \"example\"\n    }\n  },\n  \"mcpServers\": {\n    \"other\": {\n      \"command\": \"other\",\n      \"args\": [\n        \"x\"\n      ]\n    }\n  },\n  \"aLaterKey\": true\n}\n";
    write(&user, ".gemini/settings.json", before);
    let r = o.dispatch("addConnection", &json!({ "client": "gemini", "grants": grants("read") })).unwrap();
    assert_eq!(r["status"], "configured");
    assert!(r["token"].is_null(), "the key is written to the app, never returned for a known app");
    assert!(r["next"].as_str().unwrap().contains("Gemini CLI"));

    let after_text = read(&user, ".gemini/settings.json");
    let after: Value = serde_json::from_str(&after_text).unwrap();
    let was: Value = serde_json::from_str(before).unwrap();
    assert_eq!(after["security"], was["security"]);
    assert_eq!(after["mcpServers"]["other"], was["mcpServers"]["other"]);
    let order = |text: &str| ["\"security\"", "\"mcpServers\"", "\"aLaterKey\""].map(|k| text.find(k).unwrap());
    assert!(order(&after_text).windows(2).all(|p| p[0] < p[1]), "top-level key order is kept");
    let entry = &after["mcpServers"]["threadkeep"];
    assert_eq!(entry["command"], "threadkeep");
    assert_eq!(entry["env"]["THREADKEEP_HOME"], w.home.to_string_lossy().as_ref());
    let token = entry["env"]["THREADKEEP_TOKEN"].as_str().unwrap().to_string();
    let saved = backups_of(&user, ".gemini");
    assert_eq!(saved.len(), 1);
    assert_eq!(read(&user, &format!(".gemini/{}", saved[0])), before, "the backup is the previous file, byte for byte");

    // The key in the app's file is a working key with exactly the granted access.
    let status = call_with_token(&w.home, Some(&token), "status", json!({}));
    assert_eq!(status.data["spaces"].as_array().unwrap().len(), 1);
    assert_eq!(status.data["spaces"][0]["level"], "read");
    assert_eq!(o.dispatch("connections", &json!({})).unwrap()[0]["status"], "verified");
    // Settings keep only a fingerprint of the key.
    assert!(!fs::read_to_string(w.home.join("config.json")).unwrap().contains(&token));
}

#[test]
fn json_app_reconnect_replaces_the_key_and_revoke_removes_only_our_entry() {
    let w = world();
    let (o, user) = ops(&w);
    write(&user, ".gemini/settings.json", "{\"mcpServers\":{\"other\":{\"command\":\"other\"}}}");
    o.dispatch("addConnection", &json!({ "client": "gemini", "grants": grants("read") })).unwrap();
    let key = |user: &Path| serde_json::from_str::<Value>(&read(user, ".gemini/settings.json")).unwrap()["mcpServers"]["threadkeep"]["env"]["THREADKEEP_TOKEN"].as_str().unwrap().to_string();
    let first = key(&user);
    let second = o.dispatch("addConnection", &json!({ "client": "gemini", "grants": grants("propose") })).unwrap();
    let now = key(&user);
    assert_ne!(now, first);
    assert!(call_with_token(&w.home, Some(&first), "status", json!({})).error.is_some(), "the replaced key is refused");
    assert_eq!(o.dispatch("connections", &json!({})).unwrap().as_array().unwrap().len(), 1);

    let r = o.dispatch("revokeConnection", &json!({ "id": second["id"] })).unwrap();
    assert_eq!(r["settings_entry_removed"], true);
    assert!(r["note"].as_str().unwrap().contains("can no longer use Threadkeep"));
    assert_eq!(serde_json::from_str::<Value>(&read(&user, ".gemini/settings.json")).unwrap(), json!({ "mcpServers": { "other": { "command": "other" } } }));
    assert!(call_with_token(&w.home, Some(&now), "status", json!({})).error.is_some());
}

#[test]
fn codex_managed_block_is_added_after_existing_settings_and_removed_cleanly() {
    let w = world();
    let (o, user) = ops(&w);
    write(&user, ".codex/config.toml", CODEX_BEFORE);
    o.dispatch("addConnection", &json!({ "client": "codex", "grants": grants("propose") })).unwrap();
    let after = read(&user, ".codex/config.toml");
    assert!(after.starts_with(CODEX_BEFORE.trim_end()), "existing settings are untouched and come first");
    assert!(after.contains("[mcp_servers.threadkeep]\ncommand = \"threadkeep\"\nargs = [\"mcp\"]"));
    assert!(after.contains("[mcp_servers.threadkeep.env]\nTHREADKEEP_TOKEN = \"tk_"));
    o.dispatch("addConnection", &json!({ "client": "codex", "grants": grants("read") })).unwrap();
    assert_eq!(read(&user, ".codex/config.toml").matches("[mcp_servers.threadkeep]").count(), 1, "reconnecting does not duplicate the block");
    o.dispatch("revokeConnection", &json!({ "id": "codex" })).unwrap();
    assert_eq!(read(&user, ".codex/config.toml"), CODEX_BEFORE);
}

#[test]
fn files_threadkeep_cannot_understand_or_did_not_write_are_left_as_they_were() {
    let w = world();
    let (o, user) = ops(&w);
    write(&user, ".gemini/settings.json", "{ \"mcpServers\": { broken json");
    let e = o.dispatch("addConnection", &json!({ "client": "gemini", "grants": grants("read") })).unwrap_err();
    assert!(e.0.contains("could not be read as JSON, so it was left untouched"));
    assert_eq!(read(&user, ".gemini/settings.json"), "{ \"mcpServers\": { broken json");
    assert!(backups_of(&user, ".gemini").is_empty());

    let foreign = "[mcp_servers.threadkeep]\ncommand = \"someone-elses\"\n";
    write(&user, ".codex/config.toml", foreign);
    assert!(o.dispatch("addConnection", &json!({ "client": "codex", "grants": grants("read") })).unwrap_err().0.contains("did not write"));
    assert_eq!(read(&user, ".codex/config.toml"), foreign);

    assert!(o.dispatch("addConnection", &json!({ "client": "cursor", "grants": grants("read") })).unwrap_err().0.contains("does not appear to be installed"));
    assert!(o.dispatch("addConnection", &json!({ "client": "gemini", "grants": [] })).unwrap_err().0.contains("at least one space"));
    assert_eq!(o.dispatch("connections", &json!({})).unwrap(), json!([]), "a failed connect leaves no connection behind");
}

#[test]
fn each_platform_gets_its_own_claude_desktop_settings_location() {
    let c = client("claude-desktop").unwrap();
    let file = |platform: &str| settings_file(c, &user_env(Path::new("/h"), platform)).to_string_lossy().replace('\\', "/");
    assert_eq!(file("macos"), "/h/Library/Application Support/Claude/claude_desktop_config.json");
    assert_eq!(file("linux"), "/h/.config/Claude/claude_desktop_config.json");
    assert_eq!(file("windows"), "/h/AppData/Roaming/Claude/claude_desktop_config.json");
}

#[test]
fn spaces_add_create_overlap_update_and_remove() {
    let w = world();
    let (o, _) = ops(&w);
    let inner = w.company.join("Clients");
    assert!(o.dispatch("addSpace", &json!({ "path": inner.to_string_lossy(), "name": "Inner" })).unwrap_err().0.contains("overlaps"));
    let fresh = w.base.join("Fresh");
    assert!(o.dispatch("addSpace", &json!({ "path": fresh.to_string_lossy() })).unwrap_err().0.contains("Folder not found"));
    let made = o.dispatch("addSpace", &json!({ "path": fresh.to_string_lossy(), "create": true, "kind": "client" })).unwrap();
    assert_eq!(made["created_folder"], true);
    assert_eq!(made["sensitive"], true, "client spaces start sensitive");
    assert_eq!(made["notes"], 1);
    assert!(fresh.join("Welcome.md").is_file());
    assert_eq!(o.dispatch("updateSpace", &json!({ "name": "Fresh", "sensitive": false })).unwrap()["sensitive"], false);

    let state = o.dispatch("state", &json!({})).unwrap();
    assert_eq!(state["spaces"].as_array().unwrap().len(), 3);
    // Five Markdown files exist; the one in the ignored HR folder is not counted.
    assert_eq!(state["spaces"][0]["notes"], 4);

    let r = o.dispatch("removeSpace", &json!({ "name": "Mira" })).unwrap();
    assert!(r["note"].as_str().unwrap().contains("left as they are"));
    assert!(w.personal.join("Health.md").is_file());
}

#[test]
fn doctor_reports_what_to_fix_and_see_as_never_writes() {
    let w = world();
    let (o, _) = ops(&w);
    let c = o.dispatch("addConnection", &json!({ "client": "custom", "name": "Helper", "grants": grants("propose") })).unwrap();
    assert!(c["token"].as_str().unwrap().starts_with("tk_"));
    assert!(c["snippet"].as_str().unwrap().contains("THREADKEEP_TOKEN"));
    let d = o.dispatch("doctor", &json!({})).unwrap();
    assert_eq!(d["ok"], false);
    let unverified = d["checks"].as_array().unwrap().iter().find(|x| x["what"].as_str().unwrap().contains("has not called")).unwrap();
    assert!(!unverified["fix"].as_str().unwrap().is_empty());

    let p = o.dispatch("preview", &json!({ "as": c["id"], "tool": "save", "args": { "statement": "Northwind ships on Tuesdays.", "source": "invented" } })).unwrap();
    assert_eq!(p["would"], "wait for owner approval");
    assert_eq!(o.dispatch("pending", &json!({})).unwrap(), json!([]));
    assert!(memory_files(&w.company).is_empty());

    call_with_token(&w.home, c["token"].as_str(), "status", json!({}));
    assert_eq!(o.dispatch("doctor", &json!({})).unwrap()["ok"], true);
}

#[test]
fn backup_verify_list_and_restore() {
    let w = world();
    let (o, _) = ops(&w);
    o.dispatch("remember", &json!({ "space": "Northwind", "statement": "Northwind closes its books on the 5th.", "source": "finance lead" })).unwrap();
    assert_eq!(o.dispatch("listBackups", &json!({})).unwrap(), json!({ "dir": null, "remembered": false, "backups": [] }));
    let dest = w.base.join("backups");
    let b = o.dispatch("backup", &json!({ "space": "Northwind", "dest": dest.to_string_lossy() })).unwrap();
    assert_eq!(b["verified"], true);
    let dir = b["backup"].as_str().unwrap().to_string();
    assert_eq!(o.dispatch("verifyBackup", &json!({ "dir": dir })).unwrap()["ok"], true);
    let listed = o.dispatch("listBackups", &json!({})).unwrap();
    assert_eq!(listed["remembered"], true);
    assert_eq!(listed["backups"][0]["dir"], dir.as_str());
    assert_eq!(listed["backups"][0]["files"], b["files"]);

    // Restore into an empty folder; the restored space answers from its memory.
    let target = w.base.join("Restored");
    let r = o.dispatch("restore", &json!({ "dir": dir, "target": target.to_string_lossy() })).unwrap();
    assert_eq!(r["files"], b["files"]);
    assert_eq!(fs::read(target.join("Clients").join("Riverbend.md")).unwrap(), fs::read(w.company.join("Clients").join("Riverbend.md")).unwrap());
    assert!(target.join(".threadkeepignore").is_file());
    o.dispatch("addSpace", &json!({ "path": target.to_string_lossy(), "name": "Restored", "kind": "company" })).unwrap();
    let found = o.dispatch("ask", &json!({ "question": "When does Northwind close its books?", "space": "Restored" })).unwrap();
    assert_eq!(found["sources"][0]["kind"], "memory");

    // Refusals: non-empty target, backup inside the space, a folder that is not a backup.
    assert!(o.dispatch("restore", &json!({ "dir": dir, "target": w.personal.to_string_lossy() })).unwrap_err().0.contains("not empty"));
    assert!(o.dispatch("backup", &json!({ "space": "Northwind", "dest": w.company.join("backups").to_string_lossy() })).unwrap_err().0.contains("outside the space"));
    assert_eq!(o.dispatch("verifyBackup", &json!({ "dir": w.personal.to_string_lossy() })).unwrap()["ok"], false);

    // A changed or incomplete backup is detected and never restored.
    let files = Path::new(&dir).join("files");
    fs::write(files.join("Clients").join("Riverbend.md"), "tampered").unwrap();
    fs::remove_file(files.join("Playbook").join("Onboarding.md")).unwrap();
    let check = o.dispatch("verifyBackup", &json!({ "dir": dir })).unwrap();
    let problems: Vec<&str> = check["problems"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(problems.contains(&"changed since backup: Clients/Riverbend.md"));
    assert!(problems.contains(&"missing: Playbook/Onboarding.md"));
    let again = w.base.join("Again");
    assert!(o.dispatch("restore", &json!({ "dir": dir, "target": again.to_string_lossy() })).unwrap_err().0.contains("failed its check"));
    assert!(!again.exists());
}
