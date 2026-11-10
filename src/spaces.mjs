// A space is one customer-owned folder of Markdown notes. Spaces are never merged: every result
// names its space, and every write names exactly one.
import path from 'node:path';
import { readdir, readFile, stat, lstat } from 'node:fs/promises';

export const IGNORE_FILE = '.threadkeepignore';
const ALWAYS_SKIP = new Set(['node_modules']);
const MAX_NOTE_BYTES = 1_000_000;
const MAX_FILES = 50_000;

export const toPosix = (p) => p.split(path.sep).join('/');

// Every path that came from outside (an AI, the UI, a file) goes through here.
export function safeJoin(root, rel) {
  const base = path.resolve(root);
  const full = path.resolve(base, String(rel || ''));
  if (full !== base && !full.startsWith(base + path.sep)) throw new Error('That path is outside the space.');
  return full;
}

// One folder per line; "#" starts a comment. A folder listed here is never read, searched,
// answered from or backed up into an AI's view.
export async function readIgnore(root) {
  try {
    return (await readFile(path.join(root, IGNORE_FILE), 'utf8')).split(/\r?\n/)
      .map((l) => l.replace(/#.*$/, '').trim().replace(/\\/g, '/').replace(/^\/+|\/+$/g, '').toLowerCase()).filter(Boolean);
  } catch { return []; }
}

export const isIgnored = (rel, ignore) => { const r = rel.toLowerCase(); return ignore.some((p) => r === p || r.startsWith(p + '/')); };

// Markdown files in a space. Hidden folders, symlinks, ignored folders and oversized files are skipped.
export async function walkNotes(root) {
  const ignore = await readIgnore(root);
  const out = [];
  async function walk(dir, relDir) {
    let entries;
    try { entries = await readdir(dir, { withFileTypes: true }); } catch { return; }
    entries.sort((a, b) => a.name.localeCompare(b.name));
    for (const e of entries) {
      if (out.length >= MAX_FILES) return;
      if (e.name.startsWith('.') || ALWAYS_SKIP.has(e.name) || e.isSymbolicLink()) continue;
      const rel = relDir ? `${relDir}/${e.name}` : e.name;
      if (isIgnored(rel, ignore)) continue;
      const full = path.join(dir, e.name);
      if (e.isDirectory()) { await walk(full, rel); continue; }
      if (!e.isFile() || !/\.md$/i.test(e.name)) continue;
      const st = await stat(full).catch(() => null);
      if (!st || st.size > MAX_NOTE_BYTES) continue;
      out.push({ rel, full, mtimeMs: st.mtimeMs, size: st.size });
    }
  }
  await walk(path.resolve(root), '');
  return out;
}

export async function spaceExists(space) {
  try { return (await lstat(space.path)).isDirectory(); } catch { return false; }
}

// Read one note by a path that came from outside. Refuses ignored folders and non-Markdown.
export async function readSpaceFile(space, rel) {
  rel = toPosix(String(rel || '')).replace(/^\/+/, '');
  if (!/\.md$/i.test(rel)) throw new Error('Only Markdown notes can be opened.');
  const full = safeJoin(space.path, rel);
  if (rel.split('/').some((part) => part.startsWith('.')) || isIgnored(rel, await readIgnore(space.path))) throw new Error('That note is not available.');
  const st = await lstat(full).catch(() => null);
  if (!st || !st.isFile() || st.size > MAX_NOTE_BYTES) throw new Error('That note is not available.');
  return { rel, full, raw: await readFile(full, 'utf8'), mtimeMs: st.mtimeMs };
}
