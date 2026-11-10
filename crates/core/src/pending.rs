//! Approval queue. When an AI's write needs the owner's approval, the proposal is stored here,
//! in app data, and nothing is written to the space. Only the owner (command line or desktop
//! app) can approve or reject. An AI cannot approve its own proposal: there is no tool for it.

use crate::home::{atomic_write, is_hex, now_iso, random_hex, Config, Connection};
use crate::memory::{prepare_memory, save_memory, MemoryInput};
use crate::threads::{prepare_checkpoint, write_checkpoint, CheckpointInput};
use crate::{bail, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_PENDING: usize = 200;

pub enum Proposal {
    Memory(MemoryInput),
    Checkpoint(CheckpointInput),
}

fn dir(home: &Path) -> PathBuf {
    home.join("pending")
}

fn file(home: &Path, id: &str) -> Result<PathBuf> {
    if !id.strip_prefix("p-").map_or(false, |h| is_hex(h, 10)) {
        bail!("That is not a proposal id.");
    }
    Ok(dir(home).join(format!("{id}.json")))
}

pub fn list_pending(home: &Path) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir(home)) else { return vec![] };
    let mut out: Vec<Value> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|raw| serde_json::from_str(&raw).ok())
        .collect();
    out.sort_by(|a: &Value, b: &Value| a["created"].as_str().cmp(&b["created"].as_str()));
    out
}

/// The payload is validated now, so the owner never reviews something that could not be saved.
pub fn queue_proposal(home: &Path, space: &str, connection: &Connection, proposal: Proposal) -> Result<Value> {
    let (kind, summary, payload) = match proposal {
        Proposal::Memory(input) => {
            let p = prepare_memory(&input)?;
            let summary = if p.replaces.is_empty() { p.statement.clone() } else { format!("Correct {}: {}", p.replaces, p.statement) };
            ("memory", summary, serde_json::to_value(&p)?)
        }
        Proposal::Checkpoint(input) => {
            let p = prepare_checkpoint(&input)?;
            let label = if p.title.is_empty() { &p.thread } else { &p.title };
            let summary: String = format!("Checkpoint: {label} — next: {}", p.next_action).chars().take(400).collect();
            ("checkpoint", summary, serde_json::to_value(&p)?)
        }
    };
    if list_pending(home).len() >= MAX_PENDING {
        bail!("Too many proposals are waiting for review. Ask the owner to review them in Threadkeep first.");
    }
    let id = format!("p-{}", random_hex(5));
    let proposal = json!({
        "id": id, "type": kind, "space": space, "created": now_iso(),
        "connection": { "id": connection.id, "name": connection.name }, "summary": summary, "payload": payload,
    });
    atomic_write(&file(home, &id)?, (serde_json::to_string_pretty(&proposal)? + "\n").as_bytes())?;
    Ok(proposal)
}

pub fn get_pending(home: &Path, id: &str) -> Result<Value> {
    match fs::read_to_string(file(home, id)?).ok().and_then(|raw| serde_json::from_str(&raw).ok()) {
        Some(v) => Ok(v),
        None => bail!("No proposal {id} is waiting."),
    }
}

pub fn approve_pending(home: &Path, cfg: &Config, id: &str) -> Result<Value> {
    let p = get_pending(home, id)?;
    let space_name = p["space"].as_str().unwrap_or("");
    let Some(space) = cfg.spaces.iter().find(|s| s.name == space_name) else { bail!("The space \"{space_name}\" no longer exists. Reject this proposal.") };
    let result = match p["type"].as_str() {
        Some("memory") => save_memory(space, &serde_json::from_value(p["payload"].clone())?)?,
        Some("checkpoint") => write_checkpoint(space, &serde_json::from_value(p["payload"].clone())?)?,
        _ => bail!("This proposal has an unknown type. Reject it."),
    };
    fs::remove_file(file(home, id)?)?;
    Ok(json!({ "approved": id, "type": p["type"], "space": space_name, "result": result }))
}

pub fn reject_pending(home: &Path, id: &str) -> Result<Value> {
    get_pending(home, id)?;
    fs::remove_file(file(home, id)?)?;
    Ok(json!({ "rejected": id }))
}
