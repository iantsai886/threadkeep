// Backup and restore of one space: a plain folder copy plus a manifest of SHA-256 hashes.
// No archive format, so a backup can be opened and checked without Threadkeep.
import path from 'node:path';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, copyFile, stat, writeFile } from 'node:fs/promises';
import { toPosix } from './spaces.mjs';

export const MANIFEST = 'threadkeep-backup.json';
const SKIP = new Set(['.git', 'node_modules', '.trash']);

async function listFiles(root) {
  const out = [];
  async function walk(dir, rel) {
    for (const e of (await readdir(dir, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
      if (SKIP.has(e.name) || e.isSymbolicLink()) continue;
      const r = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) await walk(path.join(dir, e.name), r);
      else if (e.isFile()) out.push(r);
    }
  }
  await walk(root, '');
  return out;
}

const sha256 = async (file) => createHash('sha256').update(await readFile(file)).digest('hex');

export async function backupSpace(space, destRoot, now = new Date()) {
  if (!destRoot || !path.isAbsolute(destRoot)) throw new Error('Choose a backup folder (absolute path).');
  const src = path.resolve(space.path), root = path.resolve(destRoot);
  if (root === src || root.startsWith(src + path.sep)) throw new Error('The backup folder must be outside the space being backed up.');
  const dest = path.join(root, `${space.name.replace(/[^\w.-]+/g, '_')}-${now.toISOString().replace(/[:.]/g, '-')}`);
  await mkdir(dest, { recursive: false }).catch(async (e) => { if (e.code === 'ENOENT') { await mkdir(root, { recursive: true }); await mkdir(dest); } else throw e; });
  const files = [];
  for (const rel of await listFiles(src)) {
    const to = path.join(dest, 'files', ...rel.split('/'));
    await mkdir(path.dirname(to), { recursive: true });
    await copyFile(path.join(src, ...rel.split('/')), to);
    files.push({ path: rel, size: (await stat(to)).size, sha256: await sha256(to) });
  }
  await writeFile(path.join(dest, MANIFEST), JSON.stringify({ product: 'Threadkeep', format: 1, space: space.name, kind: space.kind, created: now.toISOString(), files }, null, 2) + '\n', { flag: 'wx' });
  const check = await verifyBackup(dest);
  if (!check.ok) throw new Error(`The backup was written but did not pass its own check: ${check.problems.slice(0, 3).join('; ')}`);
  return { backup: dest, files: files.length, bytes: files.reduce((n, f) => n + f.size, 0), verified: true };
}

// Backups directly inside one folder, newest first. Reads each manifest only; it does not
// re-hash the files, so "listed" does not mean "intact". Use verifyBackup for that.
export async function listBackups(root) {
  let entries;
  try { entries = await readdir(root, { withFileTypes: true }); } catch { return []; }
  const out = [];
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const dir = path.join(root, e.name);
    try {
      const m = JSON.parse(await readFile(path.join(dir, MANIFEST), 'utf8'));
      if (m?.product !== 'Threadkeep' || !Array.isArray(m.files)) continue;
      out.push({ dir, space: String(m.space || ''), kind: String(m.kind || ''), created: String(m.created || ''), files: m.files.length, bytes: m.files.reduce((n, f) => n + (Number(f.size) || 0), 0) });
    } catch { /* not a backup folder */ }
  }
  return out.sort((a, b) => b.created.localeCompare(a.created));
}

export async function verifyBackup(dir) {
  let manifest;
  try { manifest = JSON.parse(await readFile(path.join(dir, MANIFEST), 'utf8')); } catch { return { ok: false, problems: ['This folder is not a Threadkeep backup (no readable manifest).'] }; }
  const problems = [];
  for (const f of manifest.files || []) {
    const file = path.join(dir, 'files', ...String(f.path).split('/'));
    if (toPosix(path.relative(path.join(dir, 'files'), file)).startsWith('..')) { problems.push(`unsafe path in manifest: ${f.path}`); continue; }
    try { if ((await sha256(file)) !== f.sha256) problems.push(`changed since backup: ${f.path}`); } catch { problems.push(`missing: ${f.path}`); }
  }
  return { ok: problems.length === 0, problems, space: manifest.space, created: manifest.created, files: (manifest.files || []).length };
}

// Restore into a new or empty folder only. An existing space is never overwritten.
export async function restoreBackup(dir, target) {
  if (!target || !path.isAbsolute(target)) throw new Error('Choose a destination folder (absolute path).');
  const check = await verifyBackup(dir);
  if (!check.ok) throw new Error(`This backup failed its check and was not restored: ${check.problems.slice(0, 3).join('; ')}`);
  const existing = await readdir(target).catch(() => null);
  if (existing && existing.length) throw new Error('The destination folder is not empty. Restore into a new or empty folder; nothing was changed.');
  await mkdir(target, { recursive: true });
  const manifest = JSON.parse(await readFile(path.join(dir, MANIFEST), 'utf8'));
  for (const f of manifest.files) {
    const to = path.join(target, ...f.path.split('/'));
    await mkdir(path.dirname(to), { recursive: true });
    await copyFile(path.join(dir, 'files', ...f.path.split('/')), to);
    if ((await sha256(to)) !== f.sha256) throw new Error(`Restore stopped: ${f.path} did not copy correctly.`);
  }
  return { restored_to: target, files: manifest.files.length, space: manifest.space, next: 'Add this folder as a space in Threadkeep to use it.' };
}
