// Minimal Markdown + frontmatter handling. Records stay plain files a person can read and edit.
const OPEN = /^﻿?---[ \t]*\r?\n/;
const BLOCK = /^﻿?---[ \t]*\r?\n(?:([\s\S]*?)\r?\n)?---[ \t]*(?:\r?\n|$)/;

function scalar(v) {
  v = v.trim();
  if (v === '') return '';
  if (v.startsWith('"')) { try { return JSON.parse(v); } catch { return v.replace(/^"|"$/g, ''); } }
  if (v.startsWith("'") && v.endsWith("'") && v.length >= 2) return v.slice(1, -1).replace(/''/g, "'");
  if (v === 'true') return true;
  if (v === 'false') return false;
  if (v.startsWith('[') && v.endsWith(']')) return v.slice(1, -1).split(',').map((x) => scalar(x)).filter((x) => x !== '');
  return v;
}

function parseHeader(text) {
  const meta = {};
  let listKey = null;
  for (const line of text.split(/\r?\n/)) {
    const item = line.match(/^\s*-\s+(.*)$/);
    if (item && listKey) { meta[listKey].push(scalar(item[1])); continue; }
    const kv = line.match(/^([A-Za-z_][\w-]*)[ \t]*:[ \t]*(.*)$/);
    if (!kv) { if (line.trim()) listKey = null; continue; }
    if (kv[2].trim() === '') { meta[kv[1]] = []; listKey = kv[1]; }
    else { meta[kv[1]] = scalar(kv[2]); listKey = null; }
  }
  for (const k of Object.keys(meta)) if (Array.isArray(meta[k]) && meta[k].length === 0) meta[k] = '';
  return meta;
}

// `malformed` means a header was opened and never closed. Such a note is treated as private:
// its privacy marker may sit in the part we could not read.
export function parseNote(raw) {
  raw = String(raw);
  const m = raw.match(BLOCK);
  if (!m) return { meta: {}, header: '', body: raw.replace(/^﻿/, ''), malformed: OPEN.test(raw) };
  return { meta: parseHeader(m[1] || ''), header: m[1] || '', body: raw.slice(m[0].length), malformed: false };
}

const FALSY = new Set(['false', 'no', '0', 'off', '']);
// Deliberately loose: indented, quoted or oddly-cased `private:` keys still count.
export function isPrivate(note) {
  if (note.malformed) return true;
  for (const [k, v] of Object.entries(note.meta)) {
    if (k.toLowerCase() === 'private' && !FALSY.has(String(v).trim().toLowerCase())) return true;
  }
  for (const line of note.header.split(/\r?\n/)) {
    const m = line.match(/^\s*["']?private["']?\s*:\s*(.*)$/i);
    if (m && !FALSY.has(m[1].replace(/#.*$/, '').replace(/["']/g, '').trim().toLowerCase())) return true;
  }
  return false;
}

function emit(v) {
  const s = String(v);
  return s === '' || /[:#\[\]{},"'\n\r]|^\s|\s$|^(true|false|null|~|-)$/i.test(s) ? JSON.stringify(s) : s;
}

export function serializeNote(meta, body) {
  const lines = [];
  for (const [k, v] of Object.entries(meta)) {
    if (v === undefined || v === null || v === '') continue;
    if (Array.isArray(v)) { if (v.length) lines.push(`${k}:`, ...v.map((x) => `  - ${emit(x)}`)); }
    else lines.push(`${k}: ${typeof v === 'boolean' ? v : emit(v)}`);
  }
  return `---\n${lines.join('\n')}\n---\n\n${String(body).trim()}\n`;
}

export function noteTitle(meta, body, rel) {
  if (typeof meta.title === 'string' && meta.title.trim()) return meta.title.trim();
  const h1 = body.match(/^#\s+(.+)$/m);
  return h1 ? h1[1].trim() : rel.split('/').pop().replace(/\.md$/i, '');
}

// Split by headings so an answer can cite, and later open, one section instead of a whole note.
export function chunkBody(body, maxChars = 1500) {
  const chunks = [];
  let heading = '', buf = [];
  const flush = () => {
    const text = buf.join('\n').trim();
    buf = [];
    if (!text) return;
    if (text.length <= maxChars) { chunks.push({ heading, text }); return; }
    let part = '';
    for (const para of text.split(/\n\s*\n/)) {
      if (part && part.length + para.length > maxChars) { chunks.push({ heading, text: part.trim() }); part = ''; }
      part += (part ? '\n\n' : '') + para;
      while (part.length > maxChars * 2) { chunks.push({ heading, text: part.slice(0, maxChars) }); part = part.slice(maxChars); }
    }
    if (part.trim()) chunks.push({ heading, text: part.trim() });
  };
  for (const line of String(body).split(/\r?\n/)) {
    const h = line.match(/^#{1,6}\s+(.+?)\s*#*$/);
    if (h) { flush(); heading = h[1].trim(); } else buf.push(line);
  }
  flush();
  return chunks;
}

// Sections of a Threadkeep-written file: "## Name" -> text.
export function sections(body) {
  const out = {};
  let name = null;
  for (const line of String(body).split(/\r?\n/)) {
    const h = line.match(/^##\s+(.+?)\s*$/);
    if (h) { name = h[1].toLowerCase(); out[name] = []; } else if (name) out[name].push(line);
  }
  for (const k of Object.keys(out)) out[k] = out[k].join('\n').trim();
  return out;
}

export const bullets = (text) => String(text || '').split(/\r?\n/).map((l) => l.match(/^\s*[-*]\s+(.*)$/)?.[1]?.trim()).filter(Boolean);
