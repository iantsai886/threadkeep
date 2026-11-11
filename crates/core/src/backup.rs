//! Backup and restore of one space: a plain folder copy plus a manifest of SHA-256 hashes.
//! No archive format, so a backup can be opened and checked without Threadkeep.

use crate::home::{now_iso, sha256_hex, write_new, Space};
use crate::{bail, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "threadkeep-backup.json";

fn list_files(root: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(name) = e.file_name().into_string() else { continue };
            let kind = e.file_type()?;
            if matches!(name.as_str(), ".git" | "node_modules" | ".trash") || kind.is_symlink() {
                continue;
            }
            let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if kind.is_dir() {
                walk(&e.path(), &r, out)?;
            } else if kind.is_file() {
                out.push(r);
            }
        }
        Ok(())
    }
    let mut out = vec![];
    walk(root, "", &mut out)?;
    Ok(out)
}

fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(base.to_path_buf(), |p, part| p.join(part))
}

/// A manifest path is trusted only if it is made of plain names: no "..", no drive, no root.
fn safe_rel(rel: &str) -> bool {
    !rel.is_empty() && rel.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && !p.contains(['\\', ':']))
}

fn hash_file(file: &Path) -> Result<String> {
    Ok(sha256_hex(&fs::read(file)?))
}

fn inside(child: &Path, parent: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase();
    let (c, p) = (norm(child), norm(parent));
    c == p || c.starts_with(&format!("{p}/"))
}

pub fn backup_space(space: &Space, dest_root: &Path) -> Result<Value> {
    if !dest_root.is_absolute() {
        bail!("Choose a backup folder (absolute path).");
    }
    let src = Path::new(&space.path);
    if inside(dest_root, src) {
        bail!("The backup folder must be outside the space being backed up.");
    }
    let created = now_iso();
    let safe_name: String = space.name.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    let dest = dest_root.join(format!("{safe_name}-{}", created.replace([':', '.'], "-")));
    fs::create_dir_all(dest_root)?;
    fs::create_dir(&dest)?;
    let mut files = vec![];
    let mut bytes = 0u64;
    for rel in list_files(src)? {
        let to = join_rel(&dest.join("files"), &rel);
        fs::create_dir_all(to.parent().unwrap())?;
        fs::copy(join_rel(src, &rel), &to)?;
        let size = fs::metadata(&to)?.len();
        bytes += size;
        files.push(json!({ "path": rel, "size": size, "sha256": hash_file(&to)? }));
    }
    let count = files.len();
    let manifest = json!({ "product": "Threadkeep", "format": 1, "space": space.name, "kind": space.kind, "created": created, "files": files });
    write_new(&dest.join(MANIFEST), &(serde_json::to_string_pretty(&manifest)? + "\n"))?;
    let check = verify_backup(&dest);
    if check["ok"] != true {
        bail!("The backup was written but did not pass its own check.");
    }
    Ok(json!({ "backup": dest.to_string_lossy(), "files": count, "bytes": bytes, "verified": true }))
}

fn read_manifest(dir: &Path) -> Option<Value> {
    let m: Value = serde_json::from_str(&fs::read_to_string(dir.join(MANIFEST)).ok()?).ok()?;
    (m["product"] == "Threadkeep" && m["files"].is_array()).then_some(m)
}

/// Backups directly inside one folder, newest first. Reads each manifest only; it does not
/// re-hash the files, so "listed" does not mean "intact".
pub fn list_backups(root: &Path) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(root) else { return vec![] };
    let mut out: Vec<Value> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let m = read_manifest(&e.path())?;
            let files = m["files"].as_array()?;
            Some(json!({
                "dir": e.path().to_string_lossy(), "space": m["space"].as_str().unwrap_or(""), "kind": m["kind"].as_str().unwrap_or(""),
                "created": m["created"].as_str().unwrap_or(""), "files": files.len(), "bytes": files.iter().filter_map(|f| f["size"].as_u64()).sum::<u64>(),
            }))
        })
        .collect();
    out.sort_by(|a, b| b["created"].as_str().cmp(&a["created"].as_str()));
    out
}

pub fn verify_backup(dir: &Path) -> Value {
    let Some(manifest) = read_manifest(dir) else {
        return json!({ "ok": false, "problems": ["This folder is not a Threadkeep backup (no readable manifest)."] });
    };
    let mut problems = vec![];
    let files = manifest["files"].as_array().cloned().unwrap_or_default();
    for f in &files {
        let rel = f["path"].as_str().unwrap_or("");
        if !safe_rel(rel) {
            problems.push(format!("unsafe path in manifest: {rel}"));
            continue;
        }
        match hash_file(&join_rel(&dir.join("files"), rel)) {
            Ok(h) if Some(h.as_str()) == f["sha256"].as_str() => {}
            Ok(_) => problems.push(format!("changed since backup: {rel}")),
            Err(_) => problems.push(format!("missing: {rel}")),
        }
    }
    json!({ "ok": problems.is_empty(), "problems": problems, "space": manifest["space"], "created": manifest["created"], "files": files.len() })
}

/// Restore into a new or empty folder only. An existing space is never overwritten.
pub fn restore_backup(dir: &Path, target: &Path) -> Result<Value> {
    if !target.is_absolute() {
        bail!("Choose a destination folder (absolute path).");
    }
    let check = verify_backup(dir);
    if check["ok"] != true {
        let first: Vec<&str> = check["problems"].as_array().map(|p| p.iter().filter_map(Value::as_str).take(3).collect()).unwrap_or_default();
        bail!("This backup failed its check and was not restored: {}", first.join("; "));
    }
    if fs::read_dir(target).map(|mut d| d.next().is_some()).unwrap_or(false) {
        bail!("The destination folder is not empty. Restore into a new or empty folder; nothing was changed.");
    }
    fs::create_dir_all(target)?;
    let manifest = read_manifest(dir).unwrap_or_default();
    let files = manifest["files"].as_array().cloned().unwrap_or_default();
    for f in &files {
        let rel = f["path"].as_str().unwrap_or("");
        let to = join_rel(target, rel);
        fs::create_dir_all(to.parent().unwrap())?;
        fs::copy(join_rel(&dir.join("files"), rel), &to)?;
        if Some(hash_file(&to)?.as_str()) != f["sha256"].as_str() {
            bail!("Restore stopped: {rel} did not copy correctly.");
        }
    }
    Ok(json!({ "restored_to": target.to_string_lossy(), "files": files.len(), "space": manifest["space"], "next": "Add this folder as a space in Threadkeep to use it." }))
}
