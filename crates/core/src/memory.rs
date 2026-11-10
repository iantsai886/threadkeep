//! Memory records: one small Markdown file per remembered statement, under
//! <space>/Threadkeep/Memories/. A record has an identity, a source and a status, so it can be
//! corrected, retired or forgotten instead of silently lingering.
//!
//! Status:  active -> superseded (a correction replaced it; kept as history)
//!          active -> retired    (kept on disk, never retrieved)
//!          any    -> forgotten  (file deleted)

use crate::home::{atomic_write, now_iso, random_hex, write_new, Space, MANAGED_DIR};
use crate::markdown::{parse_note, serialize_note};
use crate::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const MEMORY_KINDS: [&str; 5] = ["fact", "decision", "preference", "procedure", "episode"];
pub const CONFIDENCE: [&str; 4] = ["verified", "reported", "inferred", "unknown"];
pub const MEMORY_CONTRACT: &str = "threadkeep.memory/1";

/// m-YYYYMMDD-xxxxxx
pub fn valid_memory_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 17 && id.starts_with("m-") && b[2..10].iter().all(u8::is_ascii_digit) && b[10] == b'-' && crate::home::is_hex(&id[11..], 6)
}

fn memory_dir(space: &Space) -> PathBuf {
    Path::new(&space.path).join(MANAGED_DIR).join("Memories")
}

fn memory_file(space: &Space, id: &str) -> Result<PathBuf> {
    if !valid_memory_id(id) {
        bail!("That is not a memory id.");
    }
    Ok(memory_dir(space).join(format!("{id}.md")))
}

pub fn memory_rel(id: &str) -> String {
    format!("{MANAGED_DIR}/Memories/{id}.md")
}

// ---- secret screening: a safety net, not a guarantee ----

fn run_len(s: &str, ok: impl Fn(char) -> bool) -> usize {
    s.chars().take_while(|c| ok(*c)).count()
}

fn word_start(text: &str, at: usize) -> bool {
    at == 0 || !text[..at].chars().next_back().map_or(false, |c| c.is_alphanumeric() || c == '_')
}

/// Finds `prefix` at a word start followed by at least `min` characters accepted by `ok`.
fn has_token(text: &str, prefix: &str, min: usize, ok: impl Fn(char) -> bool + Copy) -> bool {
    text.match_indices(prefix).any(|(i, _)| word_start(text, i) && run_len(&text[i + prefix.len()..], ok) >= min)
}

fn luhn(digits: &[u32]) -> bool {
    let mut sum = 0;
    for (i, d) in digits.iter().rev().enumerate() {
        let mut n = *d;
        if i % 2 == 1 {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
    }
    sum % 10 == 0
}

fn has_card_number(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let mut digits = vec![];
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || ((chars[j] == ' ' || chars[j] == '-') && j + 1 < chars.len() && chars[j + 1].is_ascii_digit())) {
                if let Some(d) = chars[j].to_digit(10) {
                    digits.push(d);
                }
                j += 1;
            }
            if (13..=19).contains(&digits.len()) && luhn(&digits) {
                return true;
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    false
}

fn has_labelled_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    for label in ["password", "passwd", "pwd", "passphrase", "secret", "api key", "api_key", "api-key", "apikey", "access token", "access_token", "access-token"] {
        for (i, _) in lower.match_indices(label) {
            if !word_start(&lower, i) {
                continue;
            }
            let rest = lower[i + label.len()..].trim_start();
            // "password: x" and "password = x" always count. "password is x" counts only when x
            // contains a digit, so "the secret is patience" is not mistaken for a credential.
            let (value, needs_digit) = match rest.strip_prefix(':').or_else(|| rest.strip_prefix('=')) {
                Some(v) => (Some(v), false),
                None => (rest.strip_prefix("is "), true),
            };
            if let Some(v) = value {
                let token: String = v.trim_start().chars().take_while(|c| !c.is_whitespace()).collect();
                if token.chars().count() >= 6 && (!needs_digit || token.chars().any(|c| c.is_ascii_digit())) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn find_secret(text: &str) -> Option<&'static str> {
    let key = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY-----") {
        return Some("a private key");
    }
    for prefix in ["sk-", "sk_", "pk-", "pk_", "rk-", "rk_"] {
        if has_token(text, prefix, 20, key) {
            return Some("an API key");
        }
    }
    if has_token(text, "AKIA", 16, |c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Some("a cloud access key");
    }
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        if has_token(text, prefix, 30, |c| c.is_ascii_alphanumeric()) {
            return Some("a GitHub token");
        }
    }
    for prefix in ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"] {
        if has_token(text, prefix, 10, key) {
            return Some("a Slack token");
        }
    }
    if has_token(text, "tk_", 48, |c| c.is_ascii_hexdigit()) {
        return Some("a Threadkeep connection key");
    }
    if text.match_indices("eyJ").any(|(i, _)| {
        let parts: Vec<&str> = text[i..].split(|c: char| !(key(c) || c == '.')).next().unwrap_or("").split('.').collect();
        parts.len() >= 3 && parts.iter().take(3).all(|p| p.len() >= 10)
    }) {
        return Some("a signed token");
    }
    if has_labelled_secret(text) {
        return Some("a password or key");
    }
    if has_card_number(text) {
        return Some("a card number");
    }
    None
}

pub fn assert_no_secret(texts: &[&str]) -> Result<()> {
    if let Some(hit) = find_secret(&texts.join("\n")) {
        bail!("This looks like it contains {hit}. Threadkeep does not store credentials or account numbers; remove it and try again.");
    }
    Ok(())
}

fn norm(s: &str) -> String {
    let mapped: String = s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// A proposed memory as it arrives from an AI, the command line or the interface.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryInput {
    pub statement: String,
    pub kind: String,
    pub confidence: String,
    pub source: String,
    pub review_after: String,
    pub replaces: String,
    pub by: String,
}

/// Check and normalise a proposed memory without writing anything. Used before queueing, too.
pub fn prepare_memory(input: &MemoryInput) -> Result<MemoryInput> {
    let statement = one_line(&input.statement);
    if statement.is_empty() {
        bail!("A memory needs a statement: one or two plain sentences.");
    }
    if statement.chars().count() > 600 {
        bail!("Keep a memory under 600 characters. Longer material belongs in a note.");
    }
    let kind = if input.kind.is_empty() { "fact".to_string() } else { input.kind.clone() };
    if !MEMORY_KINDS.contains(&kind.as_str()) {
        bail!("kind must be one of: {}.", MEMORY_KINDS.join(", "));
    }
    let confidence = if input.confidence.is_empty() { "reported".to_string() } else { input.confidence.clone() };
    if !CONFIDENCE.contains(&confidence.as_str()) {
        bail!("confidence must be one of: {}.", CONFIDENCE.join(", "));
    }
    let source = one_line(&input.source);
    if source.is_empty() {
        bail!("Say where this came from (source), for example \"owner said so in chat on 2026-11-10\".");
    }
    if source.chars().count() > 300 {
        bail!("Keep the source under 300 characters.");
    }
    if !input.review_after.is_empty() && !is_date(&input.review_after) {
        bail!("review_after must be a date like 2027-03-01.");
    }
    if !input.replaces.is_empty() && !valid_memory_id(&input.replaces) {
        bail!("replaces must be a memory id such as m-20261110-a41c9e.");
    }
    assert_no_secret(&[&statement, &source])?;
    let by: String = one_line(&input.by).chars().take(120).collect();
    Ok(MemoryInput { statement, kind, confidence, source, review_after: input.review_after.clone(), replaces: input.replaces.clone(), by: if by.is_empty() { "owner".into() } else { by } })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MemoryRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub confidence: String,
    pub source: String,
    pub created: String,
    pub by: String,
    pub supersedes: String,
    pub superseded_by: String,
    pub review_after: String,
    pub statement: String,
    pub path: String,
    /// Active and past its review date.
    pub due: bool,
}

pub fn read_memory(space: &Space, id: &str) -> Result<MemoryRecord> {
    let Ok(raw) = fs::read_to_string(memory_file(space, id)?) else { bail!("No memory {id} in \"{}\".", space.name) };
    let note = parse_note(&raw);
    let or = |key: &str, default: &str| note.get(key).unwrap_or(default).to_string();
    let status = or("status", "active");
    let review_after = note.text("review_after");
    let today: String = now_iso().chars().take(10).collect();
    Ok(MemoryRecord {
        id: id.to_string(),
        kind: or("kind", "fact"),
        due: status == "active" && !review_after.is_empty() && review_after <= today,
        status,
        confidence: or("confidence", "unknown"),
        source: note.text("source"),
        created: note.text("created"),
        by: note.text("created_by"),
        supersedes: note.text("supersedes"),
        superseded_by: note.text("superseded_by"),
        review_after,
        statement: note.body.trim().to_string(),
        path: memory_rel(id),
    })
}

/// Records in a space, newest first. Files that do not carry a memory id (sync conflict
/// copies, stray files) are ignored, never guessed at.
pub fn list_memories(space: &Space, status: Option<&str>) -> Vec<MemoryRecord> {
    let Ok(entries) = fs::read_dir(memory_dir(space)) else { return vec![] };
    let mut out: Vec<MemoryRecord> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".md").map(str::to_string))
        .filter(|id| valid_memory_id(id))
        .filter_map(|id| read_memory(space, &id).ok())
        .filter(|r| status.map_or(true, |s| r.status == s))
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
    out
}

fn record_text(r: &MemoryRecord) -> String {
    serialize_note(
        &[
            ("contract", MEMORY_CONTRACT), ("id", &r.id), ("kind", &r.kind), ("status", &r.status), ("confidence", &r.confidence), ("source", &r.source),
            ("created", &r.created), ("created_by", &r.by), ("supersedes", &r.supersedes), ("superseded_by", &r.superseded_by), ("review_after", &r.review_after),
        ],
        &r.statement,
    )
}

/// Store a memory. With `replaces`, this is a correction: the old record stays as history.
/// Returns {saved, id, replaced?} or {saved: false, duplicate_of}.
pub fn save_memory(space: &Space, input: &MemoryInput) -> Result<Value> {
    let p = prepare_memory(input)?;
    let mut old = None;
    if !p.replaces.is_empty() {
        let rec = read_memory(space, &p.replaces)?;
        if rec.status != "active" {
            bail!("Memory {} is {}; only an active memory can be corrected.", rec.id, rec.status);
        }
        old = Some(rec);
    } else if let Some(same) = list_memories(space, Some("active")).into_iter().find(|r| norm(&r.statement) == norm(&p.statement)) {
        return Ok(json!({ "saved": false, "duplicate_of": same.id }));
    }
    let created = now_iso();
    let id = format!("m-{}-{}", created[..10].replace('-', ""), random_hex(3));
    let record = MemoryRecord {
        id: id.clone(), kind: p.kind, status: "active".into(), confidence: p.confidence, source: p.source, created, by: p.by,
        supersedes: p.replaces, superseded_by: String::new(), review_after: p.review_after, statement: p.statement, path: memory_rel(&id), due: false,
    };
    write_new(&memory_file(space, &id)?, &record_text(&record))?; // never replaces an existing record
    let mut result = json!({ "saved": true, "id": id });
    if let Some(mut o) = old {
        o.status = "superseded".into();
        o.superseded_by = id;
        atomic_write(&memory_file(space, &o.id)?, record_text(&o).as_bytes())?;
        result["replaced"] = json!(o.id);
    }
    Ok(result)
}

pub fn retire_memory(space: &Space, id: &str) -> Result<MemoryRecord> {
    let mut rec = read_memory(space, id)?;
    if rec.status != "retired" {
        rec.status = "retired".into();
        rec.due = false;
        atomic_write(&memory_file(space, id)?, record_text(&rec).as_bytes())?;
    }
    Ok(rec)
}

/// Forget: delete the record file. Search keeps no copy on disk, so nothing else needs purging
/// inside Threadkeep. Copies outside its reach are named so the owner is not misled.
pub fn forget_memory(space: &Space, id: &str) -> Result<Value> {
    let rec = read_memory(space, id)?;
    fs::remove_file(memory_file(space, id)?)?;
    let mut not_removed = vec!["Backups made before now still contain it.".to_string(), "Any AI conversation that already received it keeps its own copy.".to_string()];
    if !rec.supersedes.is_empty() {
        not_removed.push(format!("The older record {} it replaced is still stored; forget that one too if needed.", rec.supersedes));
    }
    Ok(json!({ "forgotten": id, "removed": [format!("{}/{}", space.name, rec.path)], "not_removed": not_removed }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_spotted_and_ordinary_text_is_not() {
        for bad in [
            "The portal password is hunter2-swordfish",
            "Use key sk-proj-abcdefghijklmnopqrstuvwxyz123456",
            "Card 4111 1111 1111 1111 is on file",
            "api_key=abcdef123456",
            "token ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "AKIAABCDEFGHIJKLMNOP",
        ] {
            assert!(find_secret(bad).is_some(), "{bad}");
        }
        for fine in ["Riverbend renews every March.", "The password policy is reviewed yearly.", "Order 1234 5678 shipped.", "Ask Priya about the risk register."] {
            assert_eq!(find_secret(fine), None, "{fine}");
        }
    }

    #[test]
    fn memory_ids() {
        assert!(valid_memory_id("m-20261110-a41c9e"));
        for bad in ["m-20261110-A41C9E", "m-2026111-a41c9e", "../m-20261110-a41c9e", "m-20261110-a41c9e (conflicted copy)"] {
            assert!(!valid_memory_id(bad), "{bad}");
        }
    }
}
