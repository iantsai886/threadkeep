// App data: where Threadkeep keeps its own settings, approval queue and activity log.
// Customer knowledge never lives here; it stays in the customer's own space folders.
import os from 'node:os';
import path from 'node:path';
import { mkdir, readFile, writeFile, rename, unlink, appendFile, stat } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';

export const PRODUCT = 'Threadkeep';
export const VERSION = '0.1.0';
// Folder Threadkeep manages inside each space. Everything it writes goes under here.
export const MANAGED_DIR = 'Threadkeep';
export const SPACE_KINDS = ['personal', 'company', 'client'];
export const LEVELS = ['read', 'propose', 'write'];
export const CONFIG_VERSION = 1;

export function appHome(env = process.env, platform = process.platform, home = os.homedir()) {
  if (env.THREADKEEP_HOME) return path.resolve(env.THREADKEEP_HOME);
  if (platform === 'darwin') return path.join(home, 'Library', 'Application Support', PRODUCT);
  if (platform === 'win32') return path.join(env.APPDATA || path.join(home, 'AppData', 'Roaming'), PRODUCT);
  return path.join(env.XDG_CONFIG_HOME || path.join(home, '.config'), 'threadkeep');
}

export const configPath = (home) => path.join(home, 'config.json');
export const emptyConfig = () => ({ version: CONFIG_VERSION, spaces: [], connections: [] });

export function validateConfig(cfg) {
  const errors = [];
  if (!cfg || typeof cfg !== 'object' || Array.isArray(cfg)) return ['settings must be an object'];
  if (!Array.isArray(cfg.spaces)) errors.push('spaces must be a list');
  if (!Array.isArray(cfg.connections)) errors.push('connections must be a list');
  if (errors.length) return errors;
  const names = new Set();
  for (const s of cfg.spaces) {
    if (!s || typeof s.name !== 'string' || !s.name.trim() || s.name.length > 80 || /[\/\\:#]/.test(s.name)) { errors.push('each space needs a name without / \\ : #'); continue; }
    const key = s.name.toLowerCase();
    if (names.has(key)) errors.push(`duplicate space "${s.name}"`);
    names.add(key);
    if (typeof s.path !== 'string' || !path.isAbsolute(s.path)) errors.push(`space "${s.name}": folder must be an absolute path`);
    if (!SPACE_KINDS.includes(s.kind)) errors.push(`space "${s.name}": kind must be ${SPACE_KINDS.join(', ')}`);
    if (typeof s.sensitive !== 'boolean') errors.push(`space "${s.name}": sensitive must be true or false`);
  }
  const ids = new Set();
  for (const c of cfg.connections) {
    if (!c || !/^c-[a-f0-9]{8}$/.test(c.id || '') || ids.has(c.id)) { errors.push('each connection needs a unique id'); continue; }
    ids.add(c.id);
    if (typeof c.name !== 'string' || !c.name.trim() || c.name.length > 80) errors.push(`connection ${c.id}: name is required`);
    if (!/^[a-f0-9]{64}$/.test(c.tokenHash || '')) errors.push(`connection ${c.id}: missing key fingerprint`);
    if (!Array.isArray(c.grants)) { errors.push(`connection ${c.id}: grants must be a list`); continue; }
    const seen = new Set();
    for (const g of c.grants) {
      if (!g || !names.has(String(g.space || '').toLowerCase())) errors.push(`connection ${c.id}: grant names an unknown space`);
      else if (seen.has(g.space.toLowerCase())) errors.push(`connection ${c.id}: space "${g.space}" granted twice`);
      else seen.add(g.space.toLowerCase());
      if (!LEVELS.includes(g?.level)) errors.push(`connection ${c.id}: level must be ${LEVELS.join(', ')}`);
    }
  }
  return errors;
}

export async function loadConfig(home) {
  let raw;
  try { raw = await readFile(configPath(home), 'utf8'); }
  catch (e) { if (e.code === 'ENOENT') return emptyConfig(); throw e; }
  let cfg;
  try { cfg = JSON.parse(raw); } catch { throw new Error(`Threadkeep settings are unreadable: ${configPath(home)}`); }
  cfg = { ...emptyConfig(), ...cfg };
  const errors = validateConfig(cfg);
  if (errors.length) throw new Error(`Threadkeep settings are invalid (${configPath(home)}):\n- ${errors.join('\n- ')}`);
  return cfg;
}

// Write to a unique temp file, then rename, so a crash never leaves half a file. Windows can
// briefly lock the destination (antivirus, another reader), so the rename is retried.
export async function atomicWrite(file, text, mode = 0o600) {
  await mkdir(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp-${process.pid}-${randomBytes(5).toString('hex')}`;
  try {
    await writeFile(tmp, text, { flag: 'wx', mode });
    for (let attempt = 0; ; attempt++) {
      try { await rename(tmp, file); return; }
      catch (e) {
        if (!['EPERM', 'EACCES', 'EBUSY'].includes(e.code) || attempt >= 19) throw e;
        await delay(25);
      }
    }
  } catch (e) { await unlink(tmp).catch(() => {}); throw e; }
}

export async function saveConfig(home, cfg) {
  const errors = validateConfig(cfg);
  if (errors.length) throw new Error(errors.join('; '));
  await atomicWrite(configPath(home), JSON.stringify({ ...cfg, version: CONFIG_VERSION }, null, 2) + '\n');
}

// ---- activity: names, counts and outcomes only. Never questions, statements or note text. ----
const LOG_MAX = 512 * 1024;

export async function logActivity(home, entry) {
  try {
    await mkdir(home, { recursive: true });
    const file = path.join(home, 'activity.log');
    const st = await stat(file).catch(() => null);
    if (st && st.size > LOG_MAX) await rename(file, `${file}.1`).catch(() => {});
    await appendFile(file, JSON.stringify({ at: new Date().toISOString(), ...entry }) + '\n', { mode: 0o600 });
  } catch { /* logging must never break a request */ }
}

export async function readActivity(home, n = 30) {
  try {
    const lines = (await readFile(path.join(home, 'activity.log'), 'utf8')).trim().split('\n');
    return lines.slice(-n).reverse().map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
  } catch { return []; }
}

// "Seen" proves an AI app really called Threadkeep with this connection's key. Kept outside
// config.json so the many short-lived server processes never rewrite settings.
const seenFile = (home, id) => path.join(home, 'seen', `${id}.json`);

export async function markSeen(home, id, tool) {
  try {
    const prev = await readSeen(home, id);
    await atomicWrite(seenFile(home, id), JSON.stringify({ first: prev?.first || new Date().toISOString(), last: new Date().toISOString(), tool, calls: (prev?.calls || 0) + 1 }));
  } catch { /* best effort */ }
}

export async function readSeen(home, id) {
  try { return JSON.parse(await readFile(seenFile(home, id), 'utf8')); } catch { return null; }
}

export async function clearSeen(home, id) { await unlink(seenFile(home, id)).catch(() => {}); }

export const estTokens = (value) => Math.ceil((typeof value === 'string' ? value : JSON.stringify(value)).length / 4);
