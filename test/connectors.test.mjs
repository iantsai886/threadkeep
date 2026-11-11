// Connecting known AI apps, against invented settings files in a temporary "user home".
// No real app settings are read or changed.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile, readdir } from 'node:fs/promises';
import { world, write } from './helpers.mjs';
import { connectionContext } from '../src/access.mjs';
import { loadConfig } from '../src/home.mjs';
import { createMcp } from '../src/mcp.mjs';

const GEMINI = '.gemini/settings.json';
const CODEX = '.codex/config.toml';
const CODEX_BEFORE = '# my codex settings\nmodel = "example-model"\n\n[mcp_servers.other_tool]\ncommand = "other"\nargs = ["serve"]\n\n[mcp_servers.other_tool.env]\nOTHER_KEY = "keep-me"\n';
const read = (w, rel) => readFile(path.join(w.userHome, ...rel.split('/')), 'utf8');
const backups = async (w, rel) => (await readdir(path.join(w.userHome, path.dirname(rel)))).filter((n) => n.includes('.threadkeep-backup-'));

async function callAs(w, token, tool, args = {}) {
  const mcp = createMcp(async () => connectionContext(await loadConfig(w.home), w.home, token));
  const r = await mcp.handle({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: tool, arguments: args } });
  return r.result;
}

test('JSON app: connect adds one entry, keeps everything else, backs up, and the written key works', async () => {
  const w = await world();
  const before = { security: { auth: { selectedType: 'example' } }, mcpServers: { other: { command: 'other', args: ['x'], env: { OTHER_KEY: 'keep-me' } } } };
  await write(w.userHome, GEMINI, JSON.stringify(before, null, 2));
  const r = await w.ops.addConnection({ client: 'gemini', grants: [{ space: 'Northwind', level: 'read' }] });
  assert.equal(r.status, 'configured');
  assert.equal(r.token, undefined, 'the key is written to the app, never returned for a known app');
  assert.match(r.next, /Gemini CLI/);

  const after = JSON.parse(await read(w, GEMINI));
  assert.deepEqual(after.security, before.security);
  assert.deepEqual(after.mcpServers.other, before.mcpServers.other);
  const entry = after.mcpServers.threadkeep;
  assert.deepEqual(Object.keys(entry.env).sort(), ['THREADKEEP_HOME', 'THREADKEEP_TOKEN']);
  assert.equal(entry.env.THREADKEEP_HOME, w.home);
  const [backup] = await backups(w, GEMINI);
  assert.deepEqual(JSON.parse(await read(w, `.gemini/${backup}`)), before);

  // The key in the app's file is a working key with exactly the granted access.
  const status = await callAs(w, entry.env.THREADKEEP_TOKEN, 'status');
  assert.equal(status.structuredContent.spaces.length, 1);
  assert.equal(status.structuredContent.spaces[0].level, 'read');
  assert.equal((await w.ops.connections())[0].status, 'verified');
  // Settings keep only a fingerprint of the key.
  assert.ok(!(await readFile(path.join(w.home, 'config.json'), 'utf8')).includes(entry.env.THREADKEEP_TOKEN));
});

test('JSON app: connecting again replaces the key; revoke removes only our entry', async () => {
  const w = await world();
  await write(w.userHome, GEMINI, JSON.stringify({ mcpServers: { other: { command: 'other' } } }));
  await w.ops.addConnection({ client: 'gemini', grants: [{ space: 'Northwind', level: 'read' }] });
  const first = JSON.parse(await read(w, GEMINI)).mcpServers.threadkeep.env.THREADKEEP_TOKEN;
  const second = await w.ops.addConnection({ client: 'gemini', grants: [{ space: 'Northwind', level: 'propose' }] });
  const now = JSON.parse(await read(w, GEMINI)).mcpServers.threadkeep.env.THREADKEEP_TOKEN;
  assert.notEqual(now, first);
  assert.equal((await callAs(w, first, 'status')).isError, true, 'the replaced key is refused');
  assert.equal((await w.ops.connections()).length, 1);

  const r = await w.ops.revokeConnection({ id: second.id });
  assert.equal(r.settings_entry_removed, true);
  assert.match(r.note, /can no longer use Threadkeep/);
  assert.deepEqual(JSON.parse(await read(w, GEMINI)), { mcpServers: { other: { command: 'other' } } });
  assert.equal((await callAs(w, now, 'status')).isError, true);
});

test('Codex: a managed block is added after existing settings and removed cleanly', async () => {
  const w = await world();
  await write(w.userHome, CODEX, CODEX_BEFORE);
  const c = await w.ops.addConnection({ client: 'codex', grants: [{ space: 'Northwind', level: 'propose' }] });
  const after = await read(w, CODEX);
  assert.ok(after.startsWith(CODEX_BEFORE.trimEnd()), 'existing settings are untouched and come first');
  assert.match(after, /\[mcp_servers\.threadkeep\]\ncommand = ".+"\nargs = \[".+", "mcp"\]/);
  assert.match(after, /\[mcp_servers\.threadkeep\.env\]\nTHREADKEEP_TOKEN = "tk_[a-f0-9]{48}"\nTHREADKEEP_HOME = /);
  assert.equal((after.match(/\[mcp_servers\.threadkeep\]/g) || []).length, 1);

  await w.ops.addConnection({ client: 'codex', grants: [{ space: 'Northwind', level: 'read' }] });
  assert.equal(((await read(w, CODEX)).match(/\[mcp_servers\.threadkeep\]/g) || []).length, 1, 'reconnecting does not duplicate the block');

  await w.ops.revokeConnection({ id: 'codex' });
  assert.equal(await read(w, CODEX), CODEX_BEFORE);
  void c;
});

test('files Threadkeep cannot understand, or did not write, are left exactly as they were', async () => {
  const w = await world();
  const grants = [{ space: 'Northwind', level: 'read' }];
  await write(w.userHome, GEMINI, '{ "mcpServers": { broken json');
  await assert.rejects(w.ops.addConnection({ client: 'gemini', grants }), /could not be read as JSON, so it was left untouched/);
  assert.equal(await read(w, GEMINI), '{ "mcpServers": { broken json');
  assert.deepEqual(await backups(w, GEMINI), []);

  const foreign = '[mcp_servers.threadkeep]\ncommand = "someone-elses"\n';
  await write(w.userHome, CODEX, foreign);
  await assert.rejects(w.ops.addConnection({ client: 'codex', grants }), /did not write/);
  assert.equal(await read(w, CODEX), foreign);

  // An app that is not installed is refused instead of getting a settings file nobody reads.
  await assert.rejects(w.ops.addConnection({ client: 'cursor', grants }), /does not appear to be installed/);
  assert.deepEqual(await w.ops.connections(), [], 'a failed connect leaves no connection behind');
});

test('each platform gets its own Claude Desktop settings location', async () => {
  const { CLIENTS } = await import('../src/connectors.mjs');
  const file = (platform, env = {}) => CLIENTS['claude-desktop'].file({ home: '/h', platform, env }).replace(/\\/g, '/');
  assert.equal(file('darwin'), '/h/Library/Application Support/Claude/claude_desktop_config.json');
  assert.equal(file('linux'), '/h/.config/Claude/claude_desktop_config.json');
  assert.equal(file('win32', { APPDATA: '/h/AppData/Roaming' }), '/h/AppData/Roaming/Claude/claude_desktop_config.json');
});
