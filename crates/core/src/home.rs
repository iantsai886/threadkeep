//! App data: Threadkeep's own settings, approval queue and activity log.
//! Customer knowledge never lives here; it stays in the customer's own space folders.

use crate::{bail, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PRODUCT: &str = "Threadkeep";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Folder Threadkeep manages inside each space. Everything it writes goes under here.
pub const MANAGED_DIR: &str = "Threadkeep";
pub const SPACE_KINDS: [&str; 3] = ["personal", "company", "client"];
pub const LEVELS: [&str; 3] = ["read", "propose", "write"];
pub const CONFIG_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub sensitive: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub space: String,
    pub level: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub client: String,
    #[serde(rename = "tokenHash")]
    pub token_hash: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub revoked: bool,
    pub grants: Vec<Grant>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "config_version")]
    pub version: u32,
    #[serde(default)]
    pub spaces: Vec<Space>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

fn config_version() -> u32 {
    CONFIG_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Config { version: CONFIG_VERSION, spaces: vec![], connections: vec![] }
    }
}

/// Where app data lives on this computer. `THREADKEEP_HOME` overrides it.
pub fn app_home() -> PathBuf {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(h) = var("THREADKEEP_HOME") {
        return PathBuf::from(h);
    }
    let user = var("USERPROFILE").or_else(|| var("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        user.join("Library").join("Application Support").join(PRODUCT)
    } else if cfg!(windows) {
        var("APPDATA").map(PathBuf::from).unwrap_or_else(|| user.join("AppData").join("Roaming")).join(PRODUCT)
    } else {
        var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| user.join(".config")).join("threadkeep")
    }
}

pub fn config_path(home: &Path) -> PathBuf {
    home.join("config.json")
}

pub fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn valid_connection_id(id: &str) -> bool {
    id.strip_prefix("c-").map_or(false, |h| is_hex(h, 8))
}

pub fn validate_config(cfg: &Config) -> Vec<String> {
    let mut errors = vec![];
    let mut names = HashSet::new();
    for s in &cfg.spaces {
        let name = s.name.trim();
        if name.is_empty() || s.name.chars().count() > 80 || s.name.contains(['/', '\\', ':', '#']) {
            errors.push("each space needs a name without / \\ : #".to_string());
            continue;
        }
        if !names.insert(s.name.to_lowercase()) {
            errors.push(format!("duplicate space \"{}\"", s.name));
        }
        if !Path::new(&s.path).is_absolute() {
            errors.push(format!("space \"{}\": folder must be an absolute path", s.name));
        }
        if !SPACE_KINDS.contains(&s.kind.as_str()) {
            errors.push(format!("space \"{}\": kind must be {}", s.name, SPACE_KINDS.join(", ")));
        }
    }
    let mut ids = HashSet::new();
    for c in &cfg.connections {
        if !valid_connection_id(&c.id) || !ids.insert(c.id.clone()) {
            errors.push("each connection needs a unique id".to_string());
            continue;
        }
        if c.name.trim().is_empty() || c.name.chars().count() > 80 {
            errors.push(format!("connection {}: name is required", c.id));
        }
        if !is_hex(&c.token_hash, 64) {
            errors.push(format!("connection {}: missing key fingerprint", c.id));
        }
        let mut seen = HashSet::new();
        for g in &c.grants {
            if !names.contains(&g.space.to_lowercase()) {
                errors.push(format!("connection {}: grant names an unknown space", c.id));
            } else if !seen.insert(g.space.to_lowercase()) {
                errors.push(format!("connection {}: space \"{}\" granted twice", c.id, g.space));
            }
            if !LEVELS.contains(&g.level.as_str()) {
                errors.push(format!("connection {}: level must be {}", c.id, LEVELS.join(", ")));
            }
        }
    }
    errors
}

pub fn load_config(home: &Path) -> Result<Config> {
    let file = config_path(home);
    let raw = match fs::read_to_string(&file) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e.into()),
    };
    let cfg: Config = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|_| Error(format!("Threadkeep settings are unreadable: {}", file.display())))?;
    let errors = validate_config(&cfg);
    if !errors.is_empty() {
        bail!("Threadkeep settings are invalid ({}):\n- {}", file.display(), errors.join("\n- "));
    }
    Ok(cfg)
}

pub fn save_config(home: &Path, cfg: &Config) -> Result<()> {
    let errors = validate_config(cfg);
    if !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }
    let mut out = cfg.clone();
    out.version = CONFIG_VERSION;
    atomic_write(&config_path(home), (serde_json::to_string_pretty(&out)? + "\n").as_bytes())
}

/// Write to a unique temp file, then rename, so a crash never leaves half a file. Windows can
/// briefly lock the destination (antivirus, another reader), so the rename is retried.
pub fn atomic_write(file: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = PathBuf::from(format!("{}.tmp-{}-{}", file.display(), std::process::id(), random_hex(5)));
    let write = || -> Result<()> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all().ok();
        drop(f);
        let mut attempt = 0;
        loop {
            match fs::rename(&tmp, file) {
                Ok(()) => return Ok(()),
                Err(e) if attempt < 19 && e.kind() == std::io::ErrorKind::PermissionDenied => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(e.into()),
            }
        }
    };
    let result = write();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Create a new file; fail if it already exists. Records are never replaced by accident.
pub fn write_new(file: &Path, text: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(file)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

// ---- small utilities ----

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).expect("the operating system could not supply random bytes");
    to_hex(&buf)
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(data: &[u8]) -> String {
    to_hex(&Sha256::digest(data))
}

/// UTC timestamp like 2026-11-10T09:12:44.120Z.
pub fn iso(time: SystemTime) -> String {
    let d = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", rem / 3600, rem % 3600 / 60, rem % 60, d.subsec_millis())
}

pub fn now_iso() -> String {
    iso(SystemTime::now())
}

pub fn est_tokens(value: &Value) -> u64 {
    let len = match value {
        Value::String(s) => s.chars().count(),
        other => other.to_string().chars().count(),
    };
    ((len + 3) / 4) as u64
}

// ---- activity: names, counts and outcomes only. Never questions, statements or note text. ----

const LOG_MAX: u64 = 512 * 1024;

pub fn log_activity(home: &Path, mut entry: Value) {
    let mut run = || -> Result<()> {
        fs::create_dir_all(home)?;
        let file = home.join("activity.log");
        if fs::metadata(&file).map(|m| m.len() > LOG_MAX).unwrap_or(false) {
            let _ = fs::rename(&file, home.join("activity.log.1"));
        }
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("at".into(), json!(now_iso()));
        }
        let mut f = fs::OpenOptions::new().append(true).create(true).open(&file)?;
        f.write_all(format!("{entry}\n").as_bytes())?;
        Ok(())
    };
    let _ = run(); // logging must never break a request
}

pub fn read_activity(home: &Path, n: usize) -> Vec<Value> {
    let raw = fs::read_to_string(home.join("activity.log")).unwrap_or_default();
    raw.lines().rev().filter_map(|l| serde_json::from_str(l).ok()).take(n).collect()
}

// "Seen" proves an AI app really called Threadkeep with this connection's key. Kept outside
// config.json so the many short-lived server processes never rewrite settings.

fn seen_file(home: &Path, id: &str) -> PathBuf {
    home.join("seen").join(format!("{id}.json"))
}

pub fn read_seen(home: &Path, id: &str) -> Option<Value> {
    if !valid_connection_id(id) {
        return None;
    }
    serde_json::from_str(&fs::read_to_string(seen_file(home, id)).ok()?).ok()
}

pub fn mark_seen(home: &Path, id: &str, tool: &str) {
    if !valid_connection_id(id) {
        return;
    }
    let prev = read_seen(home, id);
    let now = now_iso();
    let first = prev.as_ref().and_then(|p| p["first"].as_str()).unwrap_or(&now).to_string();
    let calls = prev.as_ref().and_then(|p| p["calls"].as_u64()).unwrap_or(0) + 1;
    let _ = atomic_write(&seen_file(home, id), json!({ "first": first, "last": now, "tool": tool, "calls": calls }).to_string().as_bytes());
}

pub fn clear_seen(home: &Path, id: &str) {
    if valid_connection_id(id) {
        let _ = fs::remove_file(seen_file(home, id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_known_instants() {
        assert_eq!(iso(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(UNIX_EPOCH + Duration::from_millis(1_794_302_564_120)), "2026-11-10T09:22:44.120Z");
        assert_eq!(iso(UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29T00:00:00.000Z");
    }
}
