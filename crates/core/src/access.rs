//! Who may do what. One rule set, applied before anything is read:
//!   - A connection is one AI app with its own key. No key, unknown key or revoked key: nothing.
//!   - A connection sees only the spaces it was granted, at one level per space:
//!       read     ask, open, resume
//!       propose  + save and checkpoint; saved memories wait for the owner's approval
//!       write    + saved memories are stored at once
//!   - A space marked sensitive never accepts a direct write from an AI, whatever the level.

use crate::home::{random_hex, sha256_hex, Config, Connection, Space, LEVELS};
use crate::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GrantedSpace {
    pub space: Space,
    pub level: String,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub home: PathBuf,
    /// The owner at the keyboard (command line or desktop app).
    pub owner: bool,
    pub connection: Option<Connection>,
    pub spaces: Vec<GrantedSpace>,
    /// "See as this AI": behaves like the connection but never writes or queues anything.
    pub preview: bool,
}

pub fn hash_token(token: &str) -> String {
    sha256_hex(token.as_bytes())
}

pub fn new_token() -> String {
    format!("tk_{}", random_hex(24))
}

pub fn new_connection_id() -> String {
    format!("c-{}", random_hex(4))
}

fn rank(level: &str) -> Option<usize> {
    LEVELS.iter().position(|l| *l == level)
}

pub fn at_least(level: &str, wanted: &str) -> bool {
    matches!((rank(level), rank(wanted)), (Some(a), Some(b)) if a >= b)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn find_connection<'a>(cfg: &'a Config, token: Option<&str>) -> Option<&'a Connection> {
    let token = token.filter(|t| t.len() >= 20)?;
    let want = hash_token(token);
    cfg.connections.iter().find(|c| !c.revoked && constant_time_eq(c.token_hash.as_bytes(), want.as_bytes()))
}

pub fn find_space<'a>(cfg: &'a Config, name: &str) -> Option<&'a Space> {
    cfg.spaces.iter().find(|s| s.name.to_lowercase() == name.to_lowercase())
}

fn granted(cfg: &Config, connection: &Connection) -> Vec<GrantedSpace> {
    connection.grants.iter().filter_map(|g| find_space(cfg, &g.space).map(|s| GrantedSpace { space: s.clone(), level: g.level.clone() })).collect()
}

/// Context for an AI connection. Unknown callers get an empty one, never a default.
pub fn connection_context(cfg: &Config, home: &Path, token: Option<&str>) -> Context {
    let connection = find_connection(cfg, token).cloned();
    let spaces = connection.as_ref().map(|c| granted(cfg, c)).unwrap_or_default();
    Context { home: home.to_path_buf(), owner: false, connection, spaces, preview: false }
}

pub fn owner_context(cfg: &Config, home: &Path) -> Context {
    let spaces = cfg.spaces.iter().map(|s| GrantedSpace { space: s.clone(), level: "write".into() }).collect();
    Context { home: home.to_path_buf(), owner: true, connection: None, spaces, preview: false }
}

pub fn preview_context(cfg: &Config, home: &Path, connection_id: &str) -> Result<Context> {
    match cfg.connections.iter().find(|c| c.id == connection_id && !c.revoked) {
        Some(c) => Ok(Context { home: home.to_path_buf(), owner: false, connection: Some(c.clone()), spaces: granted(cfg, c), preview: true }),
        None => bail!("Unknown connection."),
    }
}

impl Context {
    pub fn space_names(&self) -> String {
        if self.spaces.is_empty() {
            "none".to_string()
        } else {
            self.spaces.iter().map(|s| s.space.name.as_str()).collect::<Vec<_>>().join(", ")
        }
    }

    /// Same message whether the space does not exist or is not granted: its existence is not confirmed.
    pub fn space_for(&self, name: &str) -> Result<&GrantedSpace> {
        match self.spaces.iter().find(|s| s.space.name.to_lowercase() == name.to_lowercase()) {
            Some(s) => Ok(s),
            None => bail!("No space named \"{name}\" is available here. Available: {}.", self.space_names()),
        }
    }

    /// Where a write goes when the caller did not name a space: only an unambiguous choice is accepted.
    pub fn writable_space(&self, name: Option<&str>) -> Result<&GrantedSpace> {
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let s = self.space_for(name)?;
            if !at_least(&s.level, "propose") {
                bail!("This connection can read \"{}\" but not add to it.", s.space.name);
            }
            return Ok(s);
        }
        let options: Vec<&GrantedSpace> = self.spaces.iter().filter(|s| at_least(&s.level, "propose")).collect();
        match options.len() {
            1 => Ok(options[0]),
            0 => bail!("This connection is read-only."),
            _ => bail!("Say which space this belongs to: {}.", options.iter().map(|s| s.space.name.as_str()).collect::<Vec<_>>().join(", ")),
        }
    }

    /// true = store now; false = queue for the owner's approval.
    pub fn writes_directly(&self, s: &GrantedSpace) -> bool {
        self.owner || (s.level == "write" && !s.space.sensitive)
    }

    /// Checkpoints are working notes for handover, so "propose" may store them, except in sensitive spaces.
    pub fn checkpoints_directly(&self, s: &GrantedSpace) -> bool {
        self.owner || (at_least(&s.level, "propose") && !s.space.sensitive)
    }

    pub fn describe_access(&self) -> Value {
        let describe = |allowed: bool, direct: bool| if !allowed { "not allowed" } else if direct { "stored at once" } else { "waits for owner approval" };
        Value::Array(
            self.spaces
                .iter()
                .map(|s| {
                    let can_add = at_least(&s.level, "propose");
                    let mut can = vec!["ask", "open", "resume"];
                    if can_add {
                        can.extend(["save", "checkpoint"]);
                    }
                    json!({
                        "space": s.space.name, "kind": s.space.kind, "sensitive": s.space.sensitive, "level": s.level, "can": can,
                        "save": describe(can_add, self.writes_directly(s)),
                        "checkpoint": describe(can_add, self.checkpoints_directly(s)),
                    })
                })
                .collect(),
        )
    }
}
