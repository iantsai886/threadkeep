// Adds Threadkeep to an AI app's own MCP settings file, and removes it again.
// Every change keeps a timestamped copy of the previous file, and a file that cannot be
// understood is left untouched rather than rewritten.
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile, copyFile, stat } from 'node:fs/promises';
import { atomicWrite } from './home.mjs';

export const SERVER_KEY = 'threadkeep';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const appData = (home, env) => env.APPDATA || path.join(home, 'AppData', 'Roaming');

// reload: what the user must do before the app sees the change. Shown verbatim in the UI.
export const CLIENTS = {
  'claude-desktop': {
    title: 'Claude Desktop', format: 'json',
    file: ({ home, platform, env }) => platform === 'darwin' ? path.join(home, 'Library', 'Application Support', 'Claude', 'claude_desktop_config.json')
      : platform === 'win32' ? path.join(appData(home, env), 'Claude', 'claude_desktop_config.json') : path.join(env.XDG_CONFIG_HOME || path.join(home, '.config'), 'Claude', 'claude_desktop_config.json'),
    reload: 'Quit Claude Desktop completely (also from the system tray or menu bar), open it again and start a new chat.',
  },
  'claude-code': { title: 'Claude Code', format: 'json', file: ({ home }) => path.join(home, '.claude.json'), reload: 'Start a new Claude Code session, then run /mcp to check.' },
  cursor: { title: 'Cursor', format: 'json', file: ({ home }) => path.join(home, '.cursor', 'mcp.json'), reload: 'Open Cursor Settings, MCP, and switch Threadkeep on if it is off.' },
  codex: { title: 'Codex CLI', format: 'toml', file: ({ home, env }) => path.join(env.CODEX_HOME || path.join(home, '.codex'), 'config.toml'), reload: 'Start a new Codex session.' },
  gemini: { title: 'Gemini CLI', format: 'json', file: ({ home }) => path.join(home, '.gemini', 'settings.json'), reload: 'Start a new Gemini CLI session, then run /mcp to check.' },
  windsurf: { title: 'Windsurf', format: 'json', file: ({ home }) => path.join(home, '.codeium', 'windsurf', 'mcp_config.json'), reload: 'Refresh MCP servers in Windsurf settings.' },
};

const opts = (o = {}) => ({ home: o.home || os.homedir(), platform: o.platform || process.platform, env: o.env || process.env });
const exists = (p) => stat(p).then(() => true, () => false);

// The command an AI app runs. Absolute paths, because desktop apps do not inherit a shell PATH.
export function launchSpec(o = {}) {
  return { command: o.nodePath || process.execPath, args: [o.cliPath || path.join(ROOT, 'bin', 'threadkeep.mjs'), 'mcp'] };
}

export function serverEntry({ token, appHome, launch }) {
  return { command: launch.command, args: [...launch.args], env: { THREADKEEP_TOKEN: token, THREADKEEP_HOME: appHome } };
}

export async function detectClients(o) {
  const x = opts(o);
  const out = {};
  for (const [id, c] of Object.entries(CLIENTS)) {
    const file = c.file(x);
    const present = (await exists(file)) || (await exists(path.dirname(file)));
    let configured = false;
    if (await exists(file)) {
      const raw = await readFile(file, 'utf8').catch(() => '');
      configured = c.format === 'toml' ? raw.includes(BEGIN) : (() => { try { return Boolean(JSON.parse(raw || '{}').mcpServers?.[SERVER_KEY]); } catch { return false; } })();
    }
    out[id] = { id, title: c.title, file, present, configured, reload: c.reload };
  }
  return out;
}

async function backup(file) {
  if (!(await exists(file))) return null;
  const dest = `${file}.threadkeep-backup-${new Date().toISOString().replace(/[:.]/g, '-')}`;
  await copyFile(file, dest);
  return dest;
}

const BEGIN = '# >>> threadkeep (managed block; edit through Threadkeep)';
const END = '# <<< threadkeep';
const tomlStr = (s) => JSON.stringify(String(s));

function stripTomlBlock(raw) {
  const a = raw.indexOf(BEGIN);
  if (a < 0) return raw;
  const b = raw.indexOf(END, a);
  if (b < 0) throw new Error('The Threadkeep block in this file is damaged. Fix or remove it by hand, then try again.');
  return (raw.slice(0, a) + raw.slice(b + END.length)).replace(/\n{3,}/g, '\n\n');
}

function tomlBlock(entry) {
  return [BEGIN, `[mcp_servers.${SERVER_KEY}]`, `command = ${tomlStr(entry.command)}`, `args = [${entry.args.map(tomlStr).join(', ')}]`, '',
    `[mcp_servers.${SERVER_KEY}.env]`, ...Object.entries(entry.env).map(([k, v]) => `${k} = ${tomlStr(v)}`), END].join('\n');
}

export async function connectClient(id, { token, appHome, launch, ...rest }) {
  const c = CLIENTS[id];
  if (!c) throw new Error(`Unknown app "${id}". Known: ${Object.keys(CLIENTS).join(', ')}, or "custom".`);
  const x = opts(rest);
  const file = c.file(x);
  if (!(await exists(file)) && !(await exists(path.dirname(file)))) throw new Error(`${c.title} does not appear to be installed on this computer (no settings folder at ${path.dirname(file)}).`);
  const entry = serverEntry({ token, appHome, launch: launch || launchSpec() });
  const raw = (await exists(file)) ? await readFile(file, 'utf8') : '';
  let next;
  if (c.format === 'toml') {
    const rest_ = stripTomlBlock(raw);
    if (new RegExp(`^\\s*\\[mcp_servers\\.${SERVER_KEY}\\]`, 'm').test(rest_)) throw new Error(`${c.title} already has a "${SERVER_KEY}" entry that Threadkeep did not write. Remove it by hand, then try again.`);
    next = `${rest_.trimEnd()}${rest_.trim() ? '\n\n' : ''}${tomlBlock(entry)}\n`;
  } else {
    let json;
    try { json = raw.trim() ? JSON.parse(raw) : {}; } catch { throw new Error(`${c.title}'s settings file could not be read as JSON, so it was left untouched: ${file}`); }
    if (!json || typeof json !== 'object' || Array.isArray(json)) throw new Error(`${c.title}'s settings file has an unexpected shape, so it was left untouched: ${file}`);
    json.mcpServers = { ...(json.mcpServers && typeof json.mcpServers === 'object' ? json.mcpServers : {}), [SERVER_KEY]: entry };
    next = JSON.stringify(json, null, 2) + '\n';
  }
  const saved = await backup(file);
  await atomicWrite(file, next, 0o600);
  return { client: id, title: c.title, file, backup: saved, reload: c.reload };
}

export async function disconnectClient(id, rest) {
  const c = CLIENTS[id];
  if (!c) throw new Error(`Unknown app "${id}".`);
  const file = c.file(opts(rest));
  if (!(await exists(file))) return { client: id, removed: false };
  const raw = await readFile(file, 'utf8');
  let next;
  if (c.format === 'toml') {
    if (!raw.includes(BEGIN)) return { client: id, removed: false };
    next = stripTomlBlock(raw).trimEnd() + '\n';
  } else {
    let json;
    try { json = JSON.parse(raw || '{}'); } catch { throw new Error(`${c.title}'s settings file could not be read as JSON, so it was left untouched: ${file}`); }
    if (!json?.mcpServers?.[SERVER_KEY]) return { client: id, removed: false };
    delete json.mcpServers[SERVER_KEY];
    next = JSON.stringify(json, null, 2) + '\n';
  }
  const saved = await backup(file);
  await atomicWrite(file, next, 0o600);
  return { client: id, removed: true, file, backup: saved };
}

// For apps Threadkeep does not know: the exact entry to paste into that app's MCP settings.
export function customSnippet({ token, appHome, launch }) {
  return JSON.stringify({ mcpServers: { [SERVER_KEY]: serverEntry({ token, appHome, launch: launch || launchSpec() }) } }, null, 2);
}
