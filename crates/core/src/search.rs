//! ask: one question in, a small cited packet out. Filtering by permission and privacy happens
//! before ranking, so nothing an AI may not see can influence or appear in its results.

use crate::access::Context;
use crate::home::{est_tokens, iso, Space, MANAGED_DIR};
use crate::markdown::{chunk_body, is_private, note_title, parse_note};
use crate::spaces::{read_space_file, walk_notes};
use crate::{bail, Result};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

const STOP: &str = " a an and are as at be but by do does did for from had has have how i if in into is it its me my of on or our so than that the their them then there these they this to was we were what when where which who whom why will with would you your can could should about any all not no yes ";

fn is_stop(word: &str) -> bool {
    STOP.contains(&format!(" {word} "))
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xac00..=0xd7af)
}

fn stem(w: &str) -> &str {
    let n = w.chars().count();
    if !w.is_ascii() {
        return w;
    }
    if n > 5 && w.ends_with("ing") {
        return &w[..w.len() - 3];
    }
    // "boxes" -> "box", but "takes" -> "take": only strip "es" after a sibilant.
    if n > 4 && (w.ends_with("ed") || ["ses", "xes", "zes", "ches", "shes"].iter().any(|s| w.ends_with(s))) {
        return &w[..w.len() - 2];
    }
    if n > 3 && w.ends_with('s') && !w.ends_with("ss") {
        return &w[..w.len() - 1];
    }
    w
}

/// Runs of letters and digits, lower-cased.
fn words(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Search terms of one word: its stem, or overlapping character pairs for Chinese, Japanese
/// and Korean. Returns nothing for a stop word when `drop_stop` is set.
fn terms_of(word: &str, drop_stop: bool) -> Vec<String> {
    if word.chars().any(is_cjk) {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() == 1 {
            return vec![word.to_string()];
        }
        return chars.windows(2).map(|p| p.iter().collect()).collect();
    }
    if drop_stop && is_stop(word) {
        return vec![];
    }
    vec![stem(word).to_string()]
}

pub fn tokenize(text: &str) -> Vec<String> {
    words(text).iter().flat_map(|w| terms_of(w, false)).collect()
}

#[derive(Debug, Clone)]
struct Doc {
    space: String,
    path: String,
    memory: bool,
    private: bool,
    date: String,
    id: String,
    status: String,
    confidence: String,
    superseded_by: String,
    title: String,
    heading: String,
    text: String,
    tf: HashMap<String, f64>,
    len: f64,
}

type FileEntry = (String, Vec<Doc>); // (signature, passages)
static CACHE: Mutex<Option<HashMap<String, HashMap<String, FileEntry>>>> = Mutex::new(None);

pub fn clear_search_cache() {
    *CACHE.lock().unwrap() = None;
}

fn build_docs(space: &Space, rel: &str, mtime_ms: u128, raw: &str) -> Vec<Doc> {
    let memories = format!("{MANAGED_DIR}/Memories/");
    if rel.starts_with(&format!("{MANAGED_DIR}/Threads/")) {
        return vec![]; // work threads are reached through resume, not ask
    }
    let note = parse_note(raw);
    let memory = rel.starts_with(&memories);
    let title = if memory { String::new() } else { note_title(&note, rel) };
    let date = note
        .get("date")
        .or_else(|| note.get("created"))
        .map(str::to_string)
        .unwrap_or_else(|| iso(UNIX_EPOCH + Duration::from_millis(mtime_ms as u64)));
    let date: String = date.chars().take(10).collect();
    let chunks = if memory { vec![crate::markdown::Chunk { heading: String::new(), text: note.body.trim().to_string() }] } else { chunk_body(&note.body, 1500) };
    let status = note.get("status").map(str::to_string).unwrap_or_else(|| if memory { "active".into() } else { String::new() });
    chunks
        .into_iter()
        .filter(|c| !c.text.is_empty())
        .map(|c| {
            let mut tf: HashMap<String, f64> = HashMap::new();
            for t in tokenize(&c.text) {
                *tf.entry(t).or_default() += 1.0;
            }
            for t in tokenize(&format!("{title} {}", c.heading)) {
                *tf.entry(t).or_default() += 2.0;
            }
            Doc {
                space: space.name.clone(),
                path: rel.to_string(),
                memory,
                private: is_private(&note),
                date: date.clone(),
                id: if memory { note.text("id") } else { String::new() },
                status: status.clone(),
                confidence: note.text("confidence"),
                superseded_by: note.text("superseded_by"),
                title: title.clone(),
                heading: c.heading,
                len: c.text.chars().count() as f64,
                text: c.text,
                tf,
            }
        })
        .collect()
}

fn space_docs(space: &Space) -> Vec<Doc> {
    let files = walk_notes(Path::new(&space.path));
    let mut guard = CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashMap::new);
    let mut prev = cache.remove(&space.path).unwrap_or_default();
    let mut next: HashMap<String, FileEntry> = HashMap::new();
    let mut all = vec![];
    for f in files {
        let sig = format!("{}:{}", f.mtime_ms, f.size);
        let entry = match prev.remove(&f.rel) {
            Some(hit) if hit.0 == sig => hit,
            _ => (sig, fs::read_to_string(&f.full).map(|raw| build_docs(space, &f.rel, f.mtime_ms, &raw)).unwrap_or_default()),
        };
        all.extend(entry.1.iter().cloned());
        next.insert(f.rel, entry);
    }
    cache.insert(space.path.clone(), next);
    all
}

fn visible(ctx: &Context, d: &Doc, history: bool) -> bool {
    if d.private && !ctx.owner {
        return false;
    }
    if d.memory {
        if d.status == "retired" || d.status == "candidate" {
            return false;
        }
        if d.status == "superseded" && !history {
            return false;
        }
    }
    true
}

fn excerpt(text: &str, literal: &[String], max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let (mut best, mut best_score) = (0usize, -1i32);
    let mut start = 0;
    while start < chars.len() {
        let window: String = chars[start..(start + max).min(chars.len())].iter().collect::<String>().to_lowercase();
        let score = literal.iter().filter(|t| window.contains(t.as_str())).count() as i32;
        if score > best_score {
            best_score = score;
            best = start;
        }
        start += 80;
    }
    let end = (best + max).min(chars.len());
    let cut: String = chars[best..end].iter().collect();
    format!("{}{}{}", if best > 0 { "…" } else { "" }, cut.trim(), if end < chars.len() { "…" } else { "" })
}

pub fn make_ref(space: &str, path: &str, heading: &str) -> String {
    if heading.is_empty() {
        format!("{space}::{path}")
    } else {
        format!("{space}::{path}#{heading}")
    }
}

/// "Space::path/to/note.md#Heading" -> (space, path, heading)
pub fn parse_ref(r: &str) -> Result<(String, String, String)> {
    let Some((space, rest)) = r.split_once("::") else { bail!("A reference looks like \"Space::path/to/note.md#Heading\".") };
    let (path, heading) = rest.split_once('#').unwrap_or((rest, ""));
    if space.is_empty() || path.is_empty() {
        bail!("A reference looks like \"Space::path/to/note.md#Heading\".");
    }
    Ok((space.to_string(), path.to_string(), heading.to_string()))
}

fn finish(mut packet: Map<String, Value>) -> Value {
    let size = est_tokens(&Value::Object(packet.clone()));
    packet.insert("est_tokens".into(), json!(size));
    Value::Object(packet)
}

#[derive(Debug, Default, Clone)]
pub struct AskArgs {
    pub question: String,
    pub space: Option<String>,
    pub limit: Option<i64>,
    pub history: bool,
}

pub fn ask(ctx: &Context, args: &AskArgs) -> Result<Value> {
    let q = args.question.trim();
    if q.is_empty() {
        bail!("Ask a question.");
    }
    if q.chars().count() > 1000 {
        bail!("Keep the question under 1,000 characters.");
    }
    let limit = args.limit.unwrap_or(4).clamp(1, 8) as usize;
    let spaces: Vec<&Space> = match args.space.as_deref().filter(|s| !s.is_empty()) {
        Some(name) => vec![&ctx.space_for(name)?.space],
        None => ctx.spaces.iter().map(|s| &s.space).collect(),
    };
    if spaces.is_empty() {
        let mut p = Map::new();
        p.insert("status".into(), json!("none"));
        p.insert("covered".into(), json!([]));
        p.insert("missing".into(), json!([]));
        p.insert("more_available".into(), json!(0));
        p.insert("sources".into(), json!([]));
        p.insert("note".into(), json!("No space is available to this connection."));
        return Ok(finish(p));
    }

    let mut docs: Vec<Doc> = vec![];
    for s in spaces {
        docs.extend(space_docs(s).into_iter().filter(|d| visible(ctx, d, args.history)));
    }

    // Original word for each search term, so "covered" and "missing" read naturally.
    let mut terms: Vec<String> = vec![];
    let mut original: HashMap<String, String> = HashMap::new();
    let all_words = words(q);
    for pass in [true, false] {
        for w in &all_words {
            for t in terms_of(w, pass) {
                if !original.contains_key(&t) {
                    original.insert(t.clone(), if w.chars().any(is_cjk) { t.clone() } else { w.clone() });
                    terms.push(t);
                }
            }
        }
        if !terms.is_empty() {
            break; // the second pass runs only when every word was a stop word
        }
    }

    let n = docs.len() as f64;
    let df: HashMap<&str, f64> = terms.iter().map(|t| (t.as_str(), docs.iter().filter(|d| d.tf.contains_key(t)).count() as f64)).collect();
    let avg = (docs.iter().map(|d| d.len).sum::<f64>() / n.max(1.0)).max(1.0);
    let (k1, b) = (1.4, 0.6);
    let mut scored: Vec<(f64, &Doc)> = vec![];
    for d in &docs {
        let mut score = 0.0;
        for t in &terms {
            if let Some(tf) = d.tf.get(t) {
                let dft = df[t.as_str()];
                let idf = (1.0 + (n - dft + 0.5) / (dft + 0.5)).ln();
                score += idf * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * d.len / avg));
            }
        }
        if score <= 0.0 {
            continue;
        }
        if d.memory {
            score *= if d.status == "superseded" { 0.3 } else { 1.15 }; // curated statements edge out prose
        } else if d.status == "superseded" || !d.superseded_by.is_empty() {
            score *= 0.3;
        }
        scored.push((score, d));
    }
    scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| x.1.path.cmp(&y.1.path)));

    // One passage per note, so four results are four different sources.
    let mut seen = HashSet::new();
    let picked: Vec<&Doc> = scored.iter().filter(|(_, d)| seen.insert((d.space.as_str(), d.path.as_str()))).map(|(_, d)| *d).collect();
    let top: Vec<&Doc> = picked.iter().take(limit).copied().collect();
    let covered: Vec<&String> = terms.iter().filter(|t| top.iter().any(|d| d.tf.contains_key(*t))).collect();
    let missing: Vec<&String> = terms.iter().filter(|t| !covered.contains(t)).collect();
    let ratio = if terms.is_empty() { 0.0 } else { covered.len() as f64 / terms.len() as f64 };
    let literal: Vec<String> = terms.iter().map(|t| original[t].clone()).collect();

    let sources: Vec<Value> = top
        .iter()
        .map(|d| {
            let mut s = Map::new();
            s.insert("ref".into(), json!(make_ref(&d.space, &d.path, &d.heading)));
            s.insert("space".into(), json!(d.space));
            s.insert("kind".into(), json!(if d.memory { "memory" } else { "note" }));
            if !d.title.is_empty() {
                s.insert("title".into(), json!(d.title));
            }
            if !d.heading.is_empty() {
                s.insert("heading".into(), json!(d.heading));
            }
            s.insert("date".into(), json!(d.date));
            if !d.id.is_empty() {
                s.insert("id".into(), json!(d.id));
            }
            if !d.confidence.is_empty() {
                s.insert("confidence".into(), json!(d.confidence));
            }
            if d.status == "superseded" || !d.superseded_by.is_empty() {
                s.insert("outdated".into(), json!(true));
                if !d.superseded_by.is_empty() {
                    s.insert("replaced_by".into(), json!(d.superseded_by));
                }
            }
            s.insert("excerpt".into(), json!(excerpt(&d.text, &literal, 300)));
            Value::Object(s)
        })
        .collect();

    let mut p = Map::new();
    // found = the returned passages cover most of the question. It is about these passages,
    // not about which words happen to exist elsewhere in the space.
    p.insert("status".into(), json!(if top.is_empty() { "none" } else if ratio >= 0.6 { "found" } else { "partial" }));
    p.insert("covered".into(), json!(covered.iter().map(|t| &original[*t]).collect::<Vec<_>>()));
    p.insert("missing".into(), json!(missing.iter().map(|t| &original[*t]).collect::<Vec<_>>()));
    p.insert("more_available".into(), json!(picked.len().saturating_sub(top.len())));
    p.insert("sources".into(), Value::Array(sources));
    Ok(finish(p))
}

/// open: one section (or one short note) by reference. Same checks as ask.
pub fn open(ctx: &Context, reference: &str) -> Result<Value> {
    let (space_name, path, heading) = parse_ref(reference)?;
    let space = &ctx.space_for(&space_name)?.space;
    let (rel, raw, _) = read_space_file(space, &path)?;
    let note = parse_note(&raw);
    if is_private(&note) && !ctx.owner {
        bail!("That note is not available.");
    }
    let mut text = note.body.trim().to_string();
    let mut found_heading = String::new();
    if !heading.is_empty() {
        if let Some(hit) = chunk_body(&note.body, usize::MAX / 4).into_iter().find(|c| c.heading.to_lowercase() == heading.to_lowercase()) {
            text = hit.text;
            found_heading = hit.heading;
        }
    }
    let max = 8000;
    let truncated = text.chars().count() > max;
    let mut p = Map::new();
    p.insert("ref".into(), json!(make_ref(&space.name, &rel, &found_heading)));
    p.insert("space".into(), json!(space.name));
    p.insert("title".into(), json!(note_title(&note, &rel)));
    if !found_heading.is_empty() {
        p.insert("heading".into(), json!(found_heading));
    }
    p.insert("text".into(), json!(text.chars().take(max).collect::<String>()));
    if truncated {
        p.insert("truncated".into(), json!(true));
    }
    Ok(finish(p))
}

/// The customer's own notes in a space (Threadkeep's managed folder is not counted).
pub fn count_notes(space: &Space) -> usize {
    walk_notes(Path::new(&space.path)).iter().filter(|f| !f.rel.starts_with(&format!("{MANAGED_DIR}/"))).count()
}
