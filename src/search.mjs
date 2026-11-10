// ask: one question in, a small cited packet out. Filtering by permission and privacy happens
// before ranking, so nothing an AI may not see can influence or appear in its results.
import { readFile } from 'node:fs/promises';
import { MANAGED_DIR, estTokens } from './home.mjs';
import { walkNotes, readSpaceFile } from './spaces.mjs';
import { parseNote, isPrivate, noteTitle, chunkBody } from './markdown.mjs';
import { spaceFor } from './access.mjs';

const STOP = new Set(('a an and are as at be but by do does did for from had has have how i if in into is it its me my of on or our ' +
  'so than that the their them then there these they this to was we were what when where which who whom why will with would you your ' +
  'can could should about any all not no yes').split(' '));
const CJK = /[぀-ヿ㐀-鿿가-힯]/;

function stem(w) {
  if (w.length > 5 && w.endsWith('ing')) return w.slice(0, -3);
  if (w.length > 4 && w.endsWith('ed')) return w.slice(0, -2);
  // "boxes" -> "box", but "takes" -> "take": only strip "es" after a sibilant.
  if (w.length > 4 && /(s|x|z|ch|sh)es$/.test(w)) return w.slice(0, -2);
  if (w.length > 3 && w.endsWith('s') && !w.endsWith('ss')) return w.slice(0, -1);
  return w;
}

// Words for Latin-style scripts, overlapping pairs for Chinese, Japanese and Korean.
export function tokenize(text, { dropStop = false } = {}) {
  const out = [];
  for (const m of String(text).toLowerCase().matchAll(/[\p{L}\p{N}]+/gu)) {
    const w = m[0];
    if (CJK.test(w)) {
      const chars = [...w];
      if (chars.length === 1) out.push(w);
      for (let i = 0; i < chars.length - 1; i++) out.push(chars[i] + chars[i + 1]);
    } else if (!(dropStop && STOP.has(w))) out.push(stem(w));
  }
  return out;
}

const MEMORIES = `${MANAGED_DIR}/Memories/`;
const THREADS = `${MANAGED_DIR}/Threads/`;
const CACHE = new Map(); // space folder -> Map(rel -> { sig, docs })

function buildDocs(space, file, raw) {
  if (file.rel.startsWith(THREADS)) return []; // work threads are reached through resume, not ask
  const note = parseNote(raw);
  const priv = isPrivate(note);
  const memory = file.rel.startsWith(MEMORIES);
  const title = memory ? '' : noteTitle(note.meta, note.body, file.rel);
  const date = String(note.meta.date || note.meta.created || new Date(file.mtimeMs).toISOString()).slice(0, 10);
  const base = {
    space: space.name, path: file.rel, kind: memory ? 'memory' : 'note', private: priv, date,
    id: memory ? String(note.meta.id || '') : undefined,
    status: String(note.meta.status || (memory ? 'active' : '')),
    confidence: note.meta.confidence ? String(note.meta.confidence) : undefined,
    superseded_by: note.meta.superseded_by ? String(note.meta.superseded_by) : undefined,
  };
  const chunks = memory ? [{ heading: '', text: note.body.trim() }] : chunkBody(note.body);
  return chunks.filter((c) => c.text).map((c) => {
    const tf = new Map();
    for (const t of tokenize(c.text)) tf.set(t, (tf.get(t) || 0) + 1);
    for (const t of tokenize(`${title} ${c.heading}`)) tf.set(t, (tf.get(t) || 0) + 2);
    return { ...base, title, heading: c.heading, text: c.text, tf, len: c.text.length };
  });
}

async function spaceDocs(space) {
  const prev = CACHE.get(space.path) || new Map();
  const next = new Map();
  for (const f of await walkNotes(space.path)) {
    const sig = `${f.mtimeMs}:${f.size}`;
    const hit = prev.get(f.rel);
    if (hit && hit.sig === sig) { next.set(f.rel, hit); continue; }
    let docs = [];
    try { docs = buildDocs(space, f, await readFile(f.full, 'utf8')); } catch { /* unreadable file: skip */ }
    next.set(f.rel, { sig, docs });
  }
  CACHE.set(space.path, next);
  return [...next.values()].flatMap((x) => x.docs);
}

export function clearSearchCache() { CACHE.clear(); }

function visible(ctx, doc, history) {
  if (doc.private && !ctx.owner) return false;
  if (doc.kind === 'memory') {
    if (doc.status === 'retired' || doc.status === 'candidate') return false;
    if (doc.status === 'superseded' && !history) return false;
  }
  return true;
}

function excerpt(text, terms, max = 300) {
  if (text.length <= max) return text;
  const lower = text.toLowerCase();
  let best = 0, bestScore = -1;
  for (let start = 0; start < text.length; start += 80) {
    const window = lower.slice(start, start + max);
    const score = terms.reduce((n, t) => n + (window.includes(t) ? 1 : 0), 0);
    if (score > bestScore) { bestScore = score; best = start; }
  }
  const cut = text.slice(best, best + max).trim();
  return `${best > 0 ? '…' : ''}${cut}${best + max < text.length ? '…' : ''}`;
}

export const makeRef = (space, path, heading) => `${space}::${path}${heading ? `#${heading}` : ''}`;

export function parseRef(ref) {
  const m = String(ref || '').match(/^(.+?)::([^#]+)(?:#(.*))?$/);
  if (!m) throw new Error('A reference looks like "Space::path/to/note.md#Heading".');
  return { space: m[1], path: m[2], heading: m[3] || '' };
}

export async function ask(ctx, { question, space, limit = 4, history = false } = {}) {
  const q = String(question || '').trim();
  if (!q) throw new Error('Ask a question.');
  if (q.length > 1000) throw new Error('Keep the question under 1,000 characters.');
  limit = Math.max(1, Math.min(8, Number.isInteger(limit) ? limit : 4));
  const spaces = space ? [spaceFor(ctx, space)] : ctx.spaces;
  if (!spaces.length) return finish({ status: 'none', covered: [], missing: [], more_available: 0, sources: [], note: 'No space is available to this connection.' });

  const docs = [];
  for (const s of spaces) for (const d of await spaceDocs(s)) if (visible(ctx, d, history)) docs.push(d);

  // Original word for each search term, so "covered" and "missing" read naturally.
  const words = new Map();
  for (const m of q.toLowerCase().matchAll(/[\p{L}\p{N}]+/gu)) for (const t of tokenize(m[0], { dropStop: true })) if (!words.has(t)) words.set(t, CJK.test(m[0]) ? t : m[0]);
  if (!words.size) for (const t of tokenize(q)) words.set(t, t);
  const terms = [...words.keys()];

  const df = new Map(terms.map((t) => [t, 0]));
  let totalLen = 0;
  for (const d of docs) { totalLen += d.len; for (const t of terms) if (d.tf.has(t)) df.set(t, df.get(t) + 1); }
  const avg = totalLen / (docs.length || 1) || 1;
  const k1 = 1.4, b = 0.6;
  const scored = [];
  for (const d of docs) {
    let score = 0;
    for (const t of terms) {
      const tf = d.tf.get(t);
      if (!tf) continue;
      const idf = Math.log(1 + (docs.length - df.get(t) + 0.5) / (df.get(t) + 0.5));
      score += idf * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * d.len / avg));
    }
    if (score <= 0) continue;
    if (d.kind === 'memory') score *= d.status === 'superseded' ? 0.3 : 1.15; // curated statements edge out prose
    if (d.kind === 'note' && (d.status === 'superseded' || d.superseded_by)) score *= 0.3;
    scored.push({ d, score });
  }
  scored.sort((x, y) => y.score - x.score || x.d.path.localeCompare(y.d.path));

  // One passage per note, so four results are four different sources.
  const picked = [], seen = new Set();
  for (const s of scored) {
    const key = `${s.d.space}::${s.d.path}`;
    if (seen.has(key)) continue;
    seen.add(key);
    picked.push(s.d);
  }
  const top = picked.slice(0, limit);
  const covered = terms.filter((t) => top.some((d) => d.tf.has(t)));
  const missing = terms.filter((t) => !covered.includes(t));
  const ratio = terms.length ? covered.length / terms.length : 0;
  const literal = [...words.values()];
  return finish({
    // found = the returned passages cover most of the question. It is about these passages,
    // not about which words happen to exist elsewhere in the space.
    status: !top.length ? 'none' : ratio >= 0.6 ? 'found' : 'partial',
    covered: covered.map((t) => words.get(t)), missing: missing.map((t) => words.get(t)),
    more_available: Math.max(0, picked.length - top.length),
    sources: top.map((d) => ({
      ref: makeRef(d.space, d.path, d.heading), space: d.space, kind: d.kind,
      ...(d.title ? { title: d.title } : {}), ...(d.heading ? { heading: d.heading } : {}), date: d.date,
      ...(d.id ? { id: d.id } : {}), ...(d.confidence ? { confidence: d.confidence } : {}),
      ...(d.status === 'superseded' || d.superseded_by ? { outdated: true, ...(d.superseded_by ? { replaced_by: d.superseded_by } : {}) } : {}),
      ...(d.status === 'due' ? { needs_review: true } : {}),
      excerpt: excerpt(d.text, literal),
    })),
  });
}

function finish(packet) { packet.est_tokens = estTokens(packet); return packet; }

// open: one section (or one short note) by reference. Same checks as ask.
export async function open(ctx, { ref } = {}) {
  const r = parseRef(ref);
  const space = spaceFor(ctx, r.space);
  const file = await readSpaceFile(space, r.path);
  const note = parseNote(file.raw);
  if (isPrivate(note) && !ctx.owner) throw new Error('That note is not available.');
  let text = note.body.trim(), heading = '';
  if (r.heading) {
    const hit = chunkBody(note.body, 1e9).find((c) => c.heading.toLowerCase() === r.heading.toLowerCase());
    if (hit) { text = hit.text; heading = hit.heading; }
  }
  const max = 8000;
  return finish({ ref: makeRef(space.name, file.rel, heading), space: space.name, title: noteTitle(note.meta, note.body, file.rel), ...(heading ? { heading } : {}), text: text.slice(0, max), ...(text.length > max ? { truncated: true } : {}) });
}

export async function countNotes(space) { return (await walkNotes(space.path)).filter((f) => !f.rel.startsWith(`${MANAGED_DIR}/`)).length; }
