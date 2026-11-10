//! A space is one customer-owned folder of Markdown notes. Spaces are never merged: every
//! result names its space, and every write names exactly one.

use crate::home::Space;
use crate::{bail, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const IGNORE_FILE: &str = ".threadkeepignore";
const MAX_NOTE_BYTES: u64 = 1_000_000;
const MAX_FILES: usize = 50_000;

#[derive(Debug, Clone)]
pub struct NoteFile {
    /// Path inside the space, with forward slashes.
    pub rel: String,
    pub full: PathBuf,
    pub mtime_ms: u128,
    pub size: u64,
}

/// Every path that came from outside (an AI, the interface, a file) goes through here.
/// Only plain relative segments are accepted, so the result cannot leave the space.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        let mut comps = Path::new(part).components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(_)), None) if part != ".." && !part.contains(':') => out.push(part),
            _ => bail!("That path is outside the space."),
        }
    }
    Ok(out)
}

/// One folder per line; "#" starts a comment. A folder listed here is never read, searched or
/// answered from.
pub fn read_ignore(root: &Path) -> Vec<String> {
    let raw = fs::read_to_string(root.join(IGNORE_FILE)).unwrap_or_default();
    raw.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().replace('\\', "/").trim_matches('/').to_lowercase())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn is_ignored(rel: &str, ignore: &[String]) -> bool {
    let r = rel.to_lowercase();
    ignore.iter().any(|p| r == *p || r.starts_with(&format!("{p}/")))
}

fn mtime_ms(meta: &fs::Metadata) -> u128 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis())
}

fn is_markdown(name: &str) -> bool {
    name.len() > 3 && name[name.len() - 3..].eq_ignore_ascii_case(".md")
}

/// Markdown files in a space. Hidden entries, symlinks, ignored folders and oversized files
/// are skipped.
pub fn walk_notes(root: &Path) -> Vec<NoteFile> {
    let ignore = read_ignore(root);
    let mut out = vec![];
    fn walk(dir: &Path, rel_dir: &str, ignore: &[String], out: &mut Vec<NoteFile>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if out.len() >= MAX_FILES {
                return;
            }
            let Ok(name) = e.file_name().into_string() else { continue };
            let Ok(kind) = e.file_type() else { continue };
            if name.starts_with('.') || name == "node_modules" || kind.is_symlink() {
                continue;
            }
            let rel = if rel_dir.is_empty() { name.clone() } else { format!("{rel_dir}/{name}") };
            if is_ignored(&rel, ignore) {
                continue;
            }
            if kind.is_dir() {
                walk(&e.path(), &rel, ignore, out);
            } else if kind.is_file() && is_markdown(&name) {
                if let Ok(meta) = e.metadata() {
                    if meta.len() <= MAX_NOTE_BYTES {
                        out.push(NoteFile { rel, full: e.path(), mtime_ms: mtime_ms(&meta), size: meta.len() });
                    }
                }
            }
        }
    }
    walk(root, "", &ignore, &mut out);
    out
}

pub fn space_exists(space: &Space) -> bool {
    fs::symlink_metadata(&space.path).map(|m| m.is_dir()).unwrap_or(false)
}

/// Read one note by a path that came from outside. Refuses hidden paths, ignored folders,
/// symlinks and anything that is not Markdown.
pub fn read_space_file(space: &Space, rel: &str) -> Result<(String, String, u128)> {
    let rel = rel.replace('\\', "/").trim_start_matches('/').to_string();
    if !is_markdown(&rel) {
        bail!("Only Markdown notes can be opened.");
    }
    let root = Path::new(&space.path);
    let full = safe_join(root, &rel)?;
    if rel.split('/').any(|p| p.starts_with('.')) || is_ignored(&rel, &read_ignore(root)) {
        bail!("That note is not available.");
    }
    match fs::symlink_metadata(&full) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_NOTE_BYTES => match fs::read_to_string(&full) {
            Ok(raw) => Ok((rel, raw, mtime_ms(&meta))),
            Err(_) => bail!("That note is not available."),
        },
        _ => bail!("That note is not available."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_refuses_escapes() {
        let root = Path::new(if cfg!(windows) { "C:\\space" } else { "/space" });
        assert!(safe_join(root, "a/b.md").is_ok());
        assert!(safe_join(root, "./a//b.md").is_ok());
        for bad in ["../x.md", "a/../../x.md", "a\\..\\..\\x.md", "C:/Windows/x.md", "a/C:evil.md"] {
            assert!(safe_join(root, bad).is_err(), "{bad}");
        }
        // A leading slash is treated as relative to the space, never as the filesystem root.
        assert_eq!(safe_join(root, "/etc/passwd.md").unwrap(), root.join("etc").join("passwd.md"));
    }
}
