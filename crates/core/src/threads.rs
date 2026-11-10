//! Work threads: "carry on where the last AI stopped".
//! A thread is a folder under <space>/Threadkeep/Threads/<thread-id>/. Every checkpoint is a new
//! file; nothing is edited in place. Two computers that both checkpoint through a synced folder
//! therefore produce two files, never a conflict. The newest checkpoint is the thread's state.

use crate::home::{now_iso, random_hex, write_new, Space, MANAGED_DIR};
use crate::markdown::{bullets, parse_note, sections, serialize_note};
use crate::memory::{assert_no_secret, one_line};
use crate::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const THREAD_CONTRACT: &str = "threadkeep.thread/1";

/// t-word-word-…: lower-case letters, digits and single hyphens.
pub fn valid_thread_id(id: &str) -> bool {
    id.len() <= 80
        && id.strip_prefix("t-").map_or(false, |rest| {
            !rest.is_empty() && rest.split('-').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
        })
}

fn threads_dir(space: &Space) -> PathBuf {
    Path::new(&space.path).join(MANAGED_DIR).join("Threads")
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckpointInput {
    pub thread: String,
    pub title: String,
    pub goal: String,
    pub state: String,
    pub next_action: String,
    pub status: String,
    pub decisions: Vec<String>,
    pub sources: Vec<String>,
    pub open_questions: Vec<String>,
    pub by: String,
}

fn clean_list(items: &[String], name: &str) -> Result<Vec<String>> {
    if items.len() > 12 {
        bail!("Keep {name} to 12 items.");
    }
    let mut out = vec![];
    for item in items {
        let s = one_line(item);
        if s.chars().count() > 300 {
            bail!("Keep each item in {name} under 300 characters.");
        }
        if !s.is_empty() {
            out.push(s);
        }
    }
    Ok(out)
}

/// Check and normalise a checkpoint without writing. Used before queueing, too.
pub fn prepare_checkpoint(input: &CheckpointInput) -> Result<CheckpointInput> {
    let thread = input.thread.trim().to_string();
    if !thread.is_empty() && !valid_thread_id(&thread) {
        bail!("thread must be an id returned by resume or checkpoint, such as t-renewal-proposal-3fa2.");
    }
    let title: String = one_line(&input.title).chars().take(120).collect();
    if thread.is_empty() && title.is_empty() {
        bail!("A new thread needs a title. To continue an existing one, pass its thread id.");
    }
    let clean = |s: &str| s.replace('\r', "").trim().to_string();
    let (goal, state, next_action) = (clean(&input.goal), clean(&input.state), clean(&input.next_action));
    if state.is_empty() {
        bail!("Say where the work stands (state).");
    }
    if next_action.is_empty() {
        bail!("Say what should happen next (next_action).");
    }
    for (name, value) in [("goal", &goal), ("state", &state), ("next_action", &next_action)] {
        if value.chars().count() > 2000 {
            bail!("Keep {name} under 2,000 characters.");
        }
    }
    let status = if input.status.is_empty() { "open".to_string() } else { input.status.clone() };
    if status != "open" && status != "done" {
        bail!("status must be open or done.");
    }
    let out = CheckpointInput {
        thread, title, goal, state, next_action, status,
        decisions: clean_list(&input.decisions, "decisions")?,
        sources: clean_list(&input.sources, "sources")?,
        open_questions: clean_list(&input.open_questions, "open_questions")?,
        by: { let by: String = one_line(&input.by).chars().take(120).collect(); if by.is_empty() { "owner".into() } else { by } },
    };
    let mut texts: Vec<&str> = vec![&out.title, &out.goal, &out.state, &out.next_action];
    texts.extend(out.decisions.iter().chain(&out.sources).chain(&out.open_questions).map(String::as_str));
    assert_no_secret(&texts)?;
    Ok(out)
}

#[derive(Debug, Clone)]
struct Checkpoint {
    thread: String,
    title: String,
    status: String,
    goal: String,
    state: String,
    decisions: Vec<String>,
    sources: Vec<String>,
    open_questions: Vec<String>,
    next_action: String,
    by: String,
    at: String,
    file: String,
}

fn parse_checkpoint(raw: &str, file: &str) -> Option<Checkpoint> {
    let note = parse_note(raw);
    if note.get("contract") != Some(THREAD_CONTRACT) {
        return None;
    }
    let s = sections(&note.body);
    let text = |k: &str| s.get(k).cloned().unwrap_or_default();
    // "(none)" and "(not stated)" are written so the file reads well; they are not content.
    let items = |k: &str| bullets(&text(k)).into_iter().filter(|x| x != "(none)").collect::<Vec<_>>();
    let goal = text("goal");
    Some(Checkpoint {
        thread: note.get("thread")?.to_string(),
        title: note.text("title"),
        status: note.get("status").unwrap_or("open").to_string(),
        goal: if goal == "(not stated)" { String::new() } else { goal },
        state: text("state"),
        decisions: items("decisions"),
        sources: items("sources"),
        open_questions: items("open questions"),
        next_action: text("next action"),
        by: note.text("by"),
        at: note.text("at"),
        file: file.to_string(),
    })
}

fn read_thread(space: &Space, id: &str) -> Vec<Checkpoint> {
    if !valid_thread_id(id) {
        return vec![];
    }
    let dir = threads_dir(space).join(id);
    let Ok(entries) = fs::read_dir(&dir) else { return vec![] };
    let mut out: Vec<Checkpoint> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_lowercase().ends_with(".md"))
        .filter_map(|n| fs::read_to_string(dir.join(&n)).ok().and_then(|raw| parse_checkpoint(&raw, &n)))
        .filter(|c| c.thread == id)
        .collect();
    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.file.cmp(&b.file)));
    out
}

fn packet(space: &Space, history: &[Checkpoint]) -> Value {
    let last = &history[history.len() - 1];
    json!({
        "contract": THREAD_CONTRACT, "space": space.name, "thread": last.thread, "title": last.title, "status": last.status,
        "goal": last.goal, "state": last.state, "decisions": last.decisions, "sources": last.sources, "open_questions": last.open_questions,
        "next_action": last.next_action, "last_checkpoint": { "by": last.by, "at": last.at }, "checkpoints": history.len(),
    })
}

pub fn write_checkpoint(space: &Space, input: &CheckpointInput) -> Result<Value> {
    let p = prepare_checkpoint(input)?;
    let mut id = p.thread.clone();
    let mut history = vec![];
    if id.is_empty() {
        let mut slug = String::new();
        for c in p.title.to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
        let slug: String = slug.chars().take(40).collect();
        let slug = slug.trim_matches('-');
        id = format!("t-{}-{}", if slug.is_empty() { "work" } else { slug }, random_hex(2));
    } else {
        history = read_thread(space, &id);
        if history.is_empty() {
            bail!("No thread {id} in \"{}\". Leave thread empty and give a title to start a new one.", space.name);
        }
    }
    let prev = history.last();
    let title = if !p.title.is_empty() { p.title.clone() } else { prev.map(|c| c.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| id.clone()) };
    let goal = if !p.goal.is_empty() { p.goal.clone() } else { prev.map(|c| c.goal.clone()).unwrap_or_default() };
    let at = now_iso();
    let list = |xs: &[String]| if xs.is_empty() { "- (none)".to_string() } else { xs.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n") };
    let body = format!(
        "# {title}\n\n## Goal\n\n{}\n\n## State\n\n{}\n\n## Decisions\n\n{}\n\n## Sources\n\n{}\n\n## Open questions\n\n{}\n\n## Next action\n\n{}",
        if goal.is_empty() { "(not stated)" } else { &goal }, p.state, list(&p.decisions), list(&p.sources), list(&p.open_questions), p.next_action
    );
    let name = format!("{}-{}.md", at.replace([':', '.'], "-"), random_hex(2));
    let text = serialize_note(&[("contract", THREAD_CONTRACT), ("thread", &id), ("title", &title), ("status", &p.status), ("by", &p.by), ("at", &at)], &body);
    write_new(&threads_dir(space).join(&id).join(name), &text)?;
    Ok(packet(space, &read_thread(space, &id)))
}

/// Latest state of each thread, newest first. Finished threads are left out unless asked for.
pub fn list_threads(space: &Space, include_done: bool) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(threads_dir(space)) else { return vec![] };
    let mut out: Vec<Value> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .map(|id| read_thread(space, &id))
        .filter(|h| h.last().map_or(false, |c| include_done || c.status == "open"))
        .map(|h| packet(space, &h))
        .collect();
    out.sort_by(|a, b| b["last_checkpoint"]["at"].as_str().cmp(&a["last_checkpoint"]["at"].as_str()));
    out
}

pub fn get_thread(space: &Space, id: &str) -> Result<Value> {
    let history = read_thread(space, id);
    if history.is_empty() {
        bail!("No thread {id} in \"{}\".", space.name);
    }
    Ok(packet(space, &history))
}
