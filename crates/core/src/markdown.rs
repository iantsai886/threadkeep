//! Minimal Markdown + frontmatter handling. Records stay plain files a person can read and edit.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct Note {
    /// Header keys in file order, values as written (quotes removed).
    pub meta: Vec<(String, MetaValue)>,
    pub header: String,
    pub body: String,
    /// A header was opened and never closed. Such a note is treated as private: its privacy
    /// marker may sit in the part that could not be read.
    pub malformed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MetaValue {
    Text(String),
    List(Vec<String>),
}

impl Note {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.meta.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
            MetaValue::Text(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        })
    }
    pub fn text(&self, key: &str) -> String {
        self.get(key).unwrap_or("").to_string()
    }
}

fn split_lines(s: &str) -> impl Iterator<Item = &str> {
    s.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

fn is_fence(line: &str) -> bool {
    line.strip_prefix("---").map_or(false, |rest| rest.chars().all(|c| c == ' ' || c == '\t'))
}

fn scalar(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('"') {
        if let Ok(s) = serde_json::from_str::<String>(v) {
            return s;
        }
        return v.trim_matches('"').to_string();
    }
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        return v[1..v.len() - 1].replace("''", "'");
    }
    v.to_string()
}

fn parse_key(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':')?;
    let key = line[..colon].trim_end_matches([' ', '\t']);
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((key, line[colon + 1..].trim()))
}

fn parse_header(text: &str) -> Vec<(String, MetaValue)> {
    let mut meta: Vec<(String, MetaValue)> = vec![];
    let mut list_open = false;
    for line in split_lines(text) {
        let trimmed = line.trim_start();
        if list_open {
            if let Some(item) = trimmed.strip_prefix("- ") {
                if let Some((_, MetaValue::List(items))) = meta.last_mut() {
                    items.push(scalar(item));
                }
                continue;
            }
        }
        match parse_key(line) {
            Some((key, value)) if value.is_empty() => {
                meta.push((key.to_string(), MetaValue::List(vec![])));
                list_open = true;
            }
            Some((key, value)) => {
                let v = if value.starts_with('[') && value.ends_with(']') {
                    MetaValue::List(value[1..value.len() - 1].split(',').map(scalar).filter(|s| !s.is_empty()).collect())
                } else {
                    MetaValue::Text(scalar(value))
                };
                meta.push((key.to_string(), v));
                list_open = false;
            }
            None => {
                if !line.trim().is_empty() {
                    list_open = false;
                }
            }
        }
    }
    for (_, v) in meta.iter_mut() {
        if matches!(v, MetaValue::List(items) if items.is_empty()) {
            *v = MetaValue::Text(String::new());
        }
    }
    meta
}

pub fn parse_note(raw: &str) -> Note {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = raw.split_inclusive('\n');
    let first = match lines.next() {
        Some(l) => l,
        None => return Note::default(),
    };
    if !is_fence(first.trim_end_matches(['\n', '\r'])) {
        return Note { body: raw.to_string(), ..Note::default() };
    }
    let mut offset = first.len();
    let header_start = offset;
    for line in lines {
        if is_fence(line.trim_end_matches(['\n', '\r'])) {
            let header = raw[header_start..offset].trim_end_matches(['\n', '\r']).to_string();
            return Note { meta: parse_header(&header), header, body: raw[offset + line.len()..].to_string(), malformed: false };
        }
        offset += line.len();
    }
    Note { body: raw.to_string(), malformed: true, ..Note::default() }
}

fn falsy(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "false" | "no" | "0" | "off" | "")
}

/// Deliberately loose: indented, quoted or oddly-cased `private:` keys still count.
pub fn is_private(note: &Note) -> bool {
    if note.malformed {
        return true;
    }
    for line in split_lines(&note.header) {
        let t = line.trim_start().trim_start_matches(['"', '\'']);
        if t.len() < 7 || !t.is_char_boundary(7) || !t[..7].eq_ignore_ascii_case("private") {
            continue;
        }
        let rest = t[7..].trim_start_matches(['"', '\'']).trim_start();
        if let Some(value) = rest.strip_prefix(':') {
            let value = value.split('#').next().unwrap_or("").replace(['"', '\''], "");
            if !falsy(&value) {
                return true;
            }
        }
    }
    false
}

fn emit(v: &str) -> String {
    let lower = v.to_lowercase();
    let special = v.is_empty()
        || v.contains([':', '#', '[', ']', '{', '}', ',', '"', '\'', '\n', '\r'])
        || v.starts_with(char::is_whitespace)
        || v.ends_with(char::is_whitespace)
        || matches!(lower.as_str(), "true" | "false" | "null" | "~" | "-");
    if special {
        serde_json::to_string(v).unwrap_or_else(|_| format!("\"{v}\""))
    } else {
        v.to_string()
    }
}

/// Serialise a header and body. Empty values are left out.
pub fn serialize_note(meta: &[(&str, &str)], body: &str) -> String {
    let lines: Vec<String> = meta.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}: {}", emit(v))).collect();
    format!("---\n{}\n---\n\n{}\n", lines.join("\n"), body.trim())
}

fn heading_of(line: &str, max_level: usize) -> Option<(usize, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if hashes == 0 || hashes > max_level {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim();
    if text.is_empty() {
        None
    } else {
        Some((hashes, text))
    }
}

pub fn note_title(note: &Note, rel: &str) -> String {
    if let Some(t) = note.get("title") {
        return t.trim().to_string();
    }
    for line in split_lines(&note.body) {
        if let Some((1, text)) = heading_of(line, 1) {
            return text.to_string();
        }
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".md").or_else(|| name.strip_suffix(".MD")).unwrap_or(name).to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub heading: String,
    pub text: String,
}

fn char_prefix(s: &str, max_chars: usize) -> (&str, &str) {
    match s.char_indices().nth(max_chars) {
        Some((i, _)) => s.split_at(i),
        None => (s, ""),
    }
}

/// Split by headings so an answer can cite, and later open, one section instead of a whole note.
pub fn chunk_body(body: &str, max_chars: usize) -> Vec<Chunk> {
    let mut chunks = vec![];
    let mut heading = String::new();
    let mut buf: Vec<&str> = vec![];
    let mut flush = |heading: &str, buf: &mut Vec<&str>| {
        let joined = buf.join("\n");
        buf.clear();
        let text = joined.trim();
        if text.is_empty() {
            return;
        }
        if text.chars().count() <= max_chars {
            chunks.push(Chunk { heading: heading.to_string(), text: text.to_string() });
            return;
        }
        let mut part = String::new();
        for para in text.split("\n\n") {
            if !part.is_empty() && part.chars().count() + para.chars().count() > max_chars {
                chunks.push(Chunk { heading: heading.to_string(), text: part.trim().to_string() });
                part.clear();
            }
            if !part.is_empty() {
                part.push_str("\n\n");
            }
            part.push_str(para);
            while part.chars().count() > max_chars * 2 {
                let (head, tail) = char_prefix(&part, max_chars);
                chunks.push(Chunk { heading: heading.to_string(), text: head.to_string() });
                part = tail.to_string();
            }
        }
        if !part.trim().is_empty() {
            chunks.push(Chunk { heading: heading.to_string(), text: part.trim().to_string() });
        }
    };
    for line in split_lines(body) {
        if let Some((_, text)) = heading_of(line, 6) {
            flush(&heading, &mut buf);
            heading = text.to_string();
        } else {
            buf.push(line);
        }
    }
    flush(&heading, &mut buf);
    chunks
}

/// Sections of a Threadkeep-written file: "## Name" -> text, keyed in lower case.
pub fn sections(body: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut name: Option<String> = None;
    for line in split_lines(body) {
        match heading_of(line, 2) {
            Some((2, text)) => {
                let key = text.to_lowercase();
                out.entry(key.clone()).or_default();
                name = Some(key);
            }
            _ => {
                if let Some(n) = &name {
                    out.get_mut(n).unwrap().push(line);
                }
            }
        }
    }
    out.into_iter().map(|(k, v)| (k, v.join("\n").trim().to_string())).collect()
}

pub fn bullets(text: &str) -> Vec<String> {
    split_lines(text)
        .filter_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_body_round_trip() {
        let text = serialize_note(&[("id", "m-1"), ("source", "owner said: yes"), ("empty", "")], "Hello.\n");
        let note = parse_note(&text);
        assert_eq!(note.get("id"), Some("m-1"));
        assert_eq!(note.get("source"), Some("owner said: yes"));
        assert_eq!(note.get("empty"), None);
        assert_eq!(note.body.trim(), "Hello.");
        assert!(!note.malformed);
    }

    #[test]
    fn privacy_marker_is_detected_loosely() {
        for header in ["private: true", "Private: yes", "  private: true", "\"private\": true", "private: true # keep out", "PRIVATE: 1"] {
            assert!(is_private(&parse_note(&format!("---\n{header}\n---\nbody"))), "{header}");
        }
        for header in ["private: false", "private:", "privately: true", "title: private"] {
            assert!(!is_private(&parse_note(&format!("---\n{header}\n---\nbody"))), "{header}");
        }
        assert!(is_private(&parse_note("---\ntitle: never closed\n\nbody")));
        assert!(!is_private(&parse_note("no header at all")));
        assert!(!is_private(&parse_note("---\n---\nempty header")));
    }

    #[test]
    fn chunks_follow_headings() {
        let chunks = chunk_body("intro\n\n## One\n\nalpha\n\n## Two\n\nbeta", 1500);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[1], Chunk { heading: "One".into(), text: "alpha".into() });
    }
}
