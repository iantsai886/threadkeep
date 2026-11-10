// Memory records: one small Markdown file per remembered statement, under
// <space>/Threadkeep/Memories/. A record has an identity, a source and a status, so it can be
// corrected, retired or forgotten instead of silently lingering.
//
// Status:  active -> superseded (a correction replaced it; kept as history)
//          active -> retired    (kept on disk, never retrieved)
//          any    -> forgotten  (file deleted)
import path from 'node:path';
import { mkdir, readdir, readFile, writeFile, unlink } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { MANAGED_DIR, atomicWrite } from './home.mjs';
import { parseNote, serializeNote } from './markdown.mjs';

export const MEMORY_KINDS = ['fact', 'decision', 'preference', 'procedure', 'episode'];
export const CONFIDENCE = ['verified', 'reported', 'inferred', 'unknown'];
export const MEMORY_CONTRACT = 'threadkeep.memory/1';
const ID = /^m-\d{8}-[a-f0-9]{6}$/;

const memoryDir = (space) => path.join(space.path, MANAGED_DIR, 'Memories');
const memoryFile = (space, id) => { if (!ID.test(String(id))) throw new Error('That is not a memory id.'); return path.join(memoryDir(space), `${id}.md`); };
export const memoryRel = (id) => `${MANAGED_DIR}/Memories/${id}.md`;

// ---- secret screening: a safety net, not a guarantee ----
const SECRET_PATTERNS = [
  [/-----BEGIN [A-Z ]*PRIVATE KEY-----/, 'a private key'],
  [/\b(?:sk|pk|rk)[-_](?:live|test|proj|ant)?[-_]?[A-Za-z0-9_-]{20,}/, 'an API key'],
  [/\bAKIA[0-9A-Z]{16}\b/, 'a cloud access key'],
  [/\bgh[pousr]_[A-Za-z0-9]{30,}\b/, 'a GitHub token'],
  [/\bxox[baprs]-[A-Za-z0-9-]{10,}/, 'a Slack token'],
  [/\btk_[a-f0-9]{48}\b/, 'a Threadkeep connection key'],
  [/\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/, 'a signed token'],
  [/\b(?:password|passwd|pwd|passphrase|secret|api[ _-]?key|access[ _-]?token)\b\s*(?:is|[:=])\s*\S{6,}/i, 'a password or key'],
];

function luhn(digits) {
  let sum = 0, alt = false;
  for (let i = digits.length - 1; i >= 0; i--) { let n = +digits[i]; if (alt) { n *= 2; if (n > 9) n -= 9; } sum += n; alt = !alt; }
  return sum % 10 === 0;
}

export function findSecret(text) {
  text = String(text || '');
  for (const [re, label] of SECRET_PATTERNS) if (re.test(text)) return label;
  for (const m of text.matchAll(/\b(?:\d[ -]?){13,19}\b/g)) { const d = m[0].replace(/\D/g, ''); if (d.length >= 13 && d.length <= 19 && luhn(d)) return 'a card number'; }
  return null;
}

export function assertNoSecret(...texts) {
  const hit = findSecret(texts.filter(Boolean).join('\n'));
  if (hit) throw new Error(`This looks like it contains ${hit}. Threadkeep does not store credentials or account numbers; remove it and try again.`);
}

const norm = (s) => String(s).toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
const clean = (s, max) => String(s ?? '').replace(/\s+/g, ' ').trim().slice(0, max);

// Check and normalise a proposed memory without writing anything. Used before queueing, too.
export function prepareMemory(input = {}) {
  const statement = clean(input.statement, 2000);
  if (!statement) throw new Error('A memory needs a statement: one or two plain sentences.');
  if (statement.length > 600) throw new Error('Keep a memory under 600 characters. Longer material belongs in a note.');
  const kind = input.kind || 'fact';
  if (!MEMORY_KINDS.includes(kind)) throw new Error(`kind must be one of: ${MEMORY_KINDS.join(', ')}.`);
  const confidence = input.confidence || 'reported';
  if (!CONFIDENCE.includes(confidence)) throw new Error(`confidence must be one of: ${CONFIDENCE.join(', ')}.`);
  const source = clean(input.source, 400);
  if (!source) throw new Error('Say where this came from (source), for example "owner said so in chat on 2026-11-10".');
  if (source.length > 300) throw new Error('Keep the source under 300 characters.');
  const review_after = input.review_after ? String(input.review_after) : '';
  if (review_after && !/^\d{4}-\d{2}-\d{2}$/.test(review_after)) throw new Error('review_after must be a date like 2027-03-01.');
  const replaces = input.replaces ? String(input.replaces) : '';
  if (replaces && !ID.test(replaces)) throw new Error('replaces must be a memory id such as m-20261110-a41c9e.');
  assertNoSecret(statement, source);
  return { statement, kind, confidence, source, review_after, replaces, by: clean(input.by, 120) || 'owner' };
}

function toRecord(note, rel) {
  const m = note.meta;
  return {
    id: String(m.id || ''), kind: String(m.kind || 'fact'), status: String(m.status || 'active'), confidence: String(m.confidence || 'unknown'),
    source: String(m.source || ''), created: String(m.created || ''), by: String(m.created_by || ''),
    supersedes: m.supersedes ? String(m.supersedes) : '', superseded_by: m.superseded_by ? String(m.superseded_by) : '',
    review_after: m.review_after ? String(m.review_after) : '', statement: note.body.trim(), path: rel,
  };
}

export async function readMemory(space, id) {
  let raw;
  try { raw = await readFile(memoryFile(space, id), 'utf8'); } catch { throw new Error(`No memory ${id} in "${space.name}".`); }
  return toRecord(parseNote(raw), memoryRel(id));
}

export async function listMemories(space, { status } = {}) {
  let names;
  try { names = await readdir(memoryDir(space)); } catch { return []; }
  const out = [];
  for (const name of names.sort()) {
    const id = name.replace(/\.md$/i, '');
    if (!ID.test(id)) continue; // sync conflict copies and stray files are ignored, never guessed at
    try { out.push(await readMemory(space, id)); } catch { /* skip unreadable */ }
  }
  const today = new Date().toISOString().slice(0, 10);
  for (const r of out) if (r.status === 'active' && r.review_after && r.review_after <= today) r.due = true;
  return (status ? out.filter((r) => r.status === status) : out).sort((a, b) => b.created.localeCompare(a.created));
}

async function writeRecord(space, rec, { create }) {
  const meta = {
    contract: MEMORY_CONTRACT, id: rec.id, kind: rec.kind, status: rec.status, confidence: rec.confidence, source: rec.source,
    created: rec.created, created_by: rec.by, supersedes: rec.supersedes, superseded_by: rec.superseded_by, review_after: rec.review_after,
  };
  const text = serializeNote(meta, rec.statement);
  const file = memoryFile(space, rec.id);
  await mkdir(path.dirname(file), { recursive: true });
  if (create) await writeFile(file, text, { flag: 'wx' }); // never replaces an existing record
  else await atomicWrite(file, text, 0o644);
}

// Store a memory. With `replaces`, this is a correction: the old record stays as history.
export async function saveMemory(space, input, now = new Date()) {
  const p = prepareMemory(input);
  let old = null;
  if (p.replaces) {
    old = await readMemory(space, p.replaces);
    if (old.status !== 'active') throw new Error(`Memory ${old.id} is ${old.status}; only an active memory can be corrected.`);
  } else {
    const same = (await listMemories(space, { status: 'active' })).find((r) => norm(r.statement) === norm(p.statement));
    if (same) return { saved: false, duplicate_of: same.id, record: same };
  }
  const id = `m-${now.toISOString().slice(0, 10).replace(/-/g, '')}-${randomBytes(3).toString('hex')}`;
  const record = { id, kind: p.kind, status: 'active', confidence: p.confidence, source: p.source, created: now.toISOString(), by: p.by, supersedes: p.replaces, superseded_by: '', review_after: p.review_after, statement: p.statement, path: memoryRel(id) };
  await writeRecord(space, record, { create: true });
  if (old) await writeRecord(space, { ...old, status: 'superseded', superseded_by: id }, { create: false });
  return { saved: true, id, record, ...(old ? { replaced: old.id } : {}) };
}

export async function retireMemory(space, id) {
  const rec = await readMemory(space, id);
  if (rec.status === 'retired') return rec;
  const next = { ...rec, status: 'retired' };
  await writeRecord(space, next, { create: false });
  return next;
}

// Forget: delete the record file. Search keeps no copy on disk, so nothing else needs purging
// inside Threadkeep. Copies outside its reach are named so the owner is not misled.
export async function forgetMemory(space, id) {
  const rec = await readMemory(space, id);
  await unlink(memoryFile(space, id));
  return {
    forgotten: id, removed: [`${space.name}/${rec.path}`],
    not_removed: ['Backups made before now still contain it.', 'Any AI conversation that already received it keeps its own copy.', ...(rec.supersedes ? [`The older record ${rec.supersedes} it replaced is still stored; forget that one too if needed.`] : [])],
  };
}
