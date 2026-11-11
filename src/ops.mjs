// Owner operations, shared by the command line and the local app API so both behave the same.
import os from 'node:os';
import path from 'node:path';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { PRODUCT, VERSION, MANAGED_DIR, SPACE_KINDS, LEVELS, loadConfig, saveConfig, readSeen, clearSeen, readActivity, configPath, atomicWrite } from './home.mjs';
import { newToken, newConnectionId, hashToken, ownerContext, previewContext, spaceFor } from './access.mjs';
import { CLIENTS, connectClient, disconnectClient, detectClients, customSnippet, launchSpec } from './connectors.mjs';
import { spaceExists, IGNORE_FILE } from './spaces.mjs';
import { countNotes, ask, open } from './search.mjs';
import { listMemories, saveMemory, retireMemory, forgetMemory } from './memory.mjs';
import { listThreads, writeCheckpoint } from './threads.mjs';
import { listPending, approvePending, rejectPending } from './pending.mjs';
import { toolHandlers, overhead } from './mcp.mjs';
import { backupSpace, verifyBackup, restoreBackup, listBackups } from './backup.mjs';

const WELCOME = `# Welcome\n\nThis folder is your knowledge base. It belongs to you: plain Markdown files you can read, edit, move or back up with any tool.\n\n- Write notes anywhere in this folder.\n- ${PRODUCT} keeps what your AIs remember in the \`${MANAGED_DIR}\` folder, as plain files too.\n- To hide one note from every AI, put \`private: true\` at the top of it between two \`---\` lines.\n- To hide a whole folder, list it in \`${IGNORE_FILE}\`, one folder per line.\n`;

const findSpace = (cfg, name) => { const s = cfg.spaces.find((x) => x.name.toLowerCase() === String(name || '').toLowerCase()); if (!s) throw new Error(`No space named "${name}".`); return s; };
const resolveHomePath = (p, home = os.homedir()) => path.resolve(String(p || '').replace(/^~(?=$|[\/\\])/, home));

export function createOps({ home, env = process.env, userHome = os.homedir(), platform = process.platform, launch } = {}) {
  const clientOpts = { home: userHome, env, platform };
  const load = () => loadConfig(home);
  const lastBackupFile = path.join(home, 'last-backup.json');

  // ---- spaces ----
  async function addSpace({ path: folder, name, kind = 'personal', sensitive, create = false } = {}) {
    if (!folder) throw new Error('Choose a folder for the space.');
    const full = resolveHomePath(folder, userHome);
    const cfg = await load();
    if (!SPACE_KINDS.includes(kind)) throw new Error(`kind must be one of: ${SPACE_KINDS.join(', ')}.`);
    const st = await stat(full).catch(() => null);
    if (st && !st.isDirectory()) throw new Error('That path is a file, not a folder.');
    if (!st && !create) throw new Error(`Folder not found: ${full}. To start a new, empty knowledge base there, choose to create the folder (command line: add --create).`);
    for (const s of cfg.spaces) {
      const other = path.resolve(s.path);
      if (other === full || other.startsWith(full + path.sep) || full.startsWith(other + path.sep)) throw new Error(`That folder overlaps the space "${s.name}". Spaces must be separate folders so their contents never mix.`);
    }
    if (!st) await mkdir(full, { recursive: true });
    const empty = (await readdir(full)).length === 0;
    if (empty) await writeFile(path.join(full, 'Welcome.md'), WELCOME, { flag: 'wx' }).catch(() => {});
    const space = { name: String(name || path.basename(full)).trim(), path: full, kind, sensitive: sensitive === undefined ? kind !== 'company' : Boolean(sensitive) };
    await saveConfig(home, { ...cfg, spaces: [...cfg.spaces, space] });
    return { ...space, created_folder: !st, notes: await countNotes(space) };
  }

  async function updateSpace({ name, kind, sensitive } = {}) {
    const cfg = await load();
    const s = findSpace(cfg, name);
    if (kind !== undefined) { if (!SPACE_KINDS.includes(kind)) throw new Error(`kind must be one of: ${SPACE_KINDS.join(', ')}.`); s.kind = kind; }
    if (sensitive !== undefined) s.sensitive = Boolean(sensitive);
    await saveConfig(home, cfg);
    return s;
  }

  // Removing a space only forgets it in Threadkeep. The folder and its files are not touched.
  async function removeSpace({ name } = {}) {
    const cfg = await load();
    const s = findSpace(cfg, name);
    const connections = cfg.connections.map((c) => ({ ...c, grants: c.grants.filter((g) => g.space.toLowerCase() !== s.name.toLowerCase()) }));
    await saveConfig(home, { ...cfg, spaces: cfg.spaces.filter((x) => x !== s), connections });
    return { removed: s.name, folder_kept: s.path, note: `"${s.name}" was removed from ${PRODUCT}. Its folder and every file in it were left as they are: ${s.path}` };
  }

  // ---- connections ----
  function normaliseGrants(cfg, grants) {
    if (!Array.isArray(grants) || !grants.length) throw new Error('Choose at least one space for this AI app.');
    return grants.map((g) => {
      const s = findSpace(cfg, g.space);
      const level = g.level || 'propose';
      if (!LEVELS.includes(level)) throw new Error(`Access level must be one of: ${LEVELS.join(', ')}.`);
      return { space: s.name, level };
    });
  }

  async function addConnection({ client, name, grants } = {}) {
    if (client !== 'custom' && !CLIENTS[client]) throw new Error(`Unknown app "${client}". Known: ${Object.keys(CLIENTS).join(', ')}, custom.`);
    const cfg = await load();
    const clean = normaliseGrants(cfg, grants);
    // One app holds one key. Connecting it again replaces the old key instead of leaving a stale one.
    const previous = client === 'custom' ? [] : cfg.connections.filter((c) => c.client === client && !c.revoked);
    const token = newToken();
    const connection = { id: newConnectionId(), name: String(name || CLIENTS[client]?.title || 'Custom AI').trim().slice(0, 80), client, tokenHash: hashToken(token), created: new Date().toISOString(), revoked: false, grants: clean };
    let written = null;
    if (client !== 'custom') written = await connectClient(client, { token, appHome: home, launch: launch || launchSpec(), ...clientOpts });
    const connections = [...cfg.connections.map((c) => (previous.includes(c) ? { ...c, revoked: true } : c)), connection];
    await saveConfig(home, { ...cfg, connections });
    for (const p of previous) await clearSeen(home, p.id);
    return {
      id: connection.id, name: connection.name, client, grants: clean, status: 'configured',
      ...(written ? { file: written.file, backup: written.backup, next: written.reload } : { token, snippet: customSnippet({ token, appHome: home, launch: launch || launchSpec() }), next: 'Paste this entry into the app\'s MCP settings. The key is shown only once.' }),
      verify: `After that, ask the AI: "What can you do in ${PRODUCT}?" This connection shows as verified once the app really calls ${PRODUCT}.`,
    };
  }

  async function setGrants({ id, grants } = {}) {
    const cfg = await load();
    const c = cfg.connections.find((x) => x.id === id && !x.revoked);
    if (!c) throw new Error('Unknown connection.');
    c.grants = normaliseGrants(cfg, grants);
    await saveConfig(home, cfg);
    return { id, grants: c.grants, note: 'Takes effect on the app\'s next request; no restart needed.' };
  }

  async function revokeConnection({ id } = {}) {
    const cfg = await load();
    const c = cfg.connections.find((x) => x.id === id || (x.client === id && !x.revoked));
    if (!c) throw new Error('Unknown connection.');
    c.revoked = true;
    await saveConfig(home, cfg);
    await clearSeen(home, c.id);
    let removed = null;
    if (CLIENTS[c.client]) removed = await disconnectClient(c.client, clientOpts).catch((e) => ({ removed: false, error: e.message }));
    const entry = removed?.removed ? ` Its entry was removed from the app's settings (a copy of the previous file was kept).` : CLIENTS[c.client] ? ` Its entry could not be removed from the app's settings; the key no longer works either way.` : ` Remove the ${PRODUCT} entry from that app's settings yourself; the key no longer works either way.`;
    return { revoked: c.id, name: c.name, key_refused_from_now: true, settings_entry_removed: Boolean(removed?.removed), ...(removed?.error ? { warning: removed.error } : {}), note: `"${c.name}" can no longer use ${PRODUCT}, starting with its next request.${entry}` };
  }

  async function connections(cfg) {
    cfg = cfg || await load();
    const out = [];
    for (const c of cfg.connections.filter((x) => !x.revoked)) {
      const seen = await readSeen(home, c.id);
      out.push({ id: c.id, name: c.name, client: c.client, grants: c.grants, created: c.created, status: seen ? 'verified' : 'configured', first_call: seen?.first || null, last_call: seen?.last || null, calls: seen?.calls || 0, ...(seen ? {} : { next: CLIENTS[c.client]?.reload || 'Add the entry to the app, then ask it to use Threadkeep.' }) });
    }
    return out;
  }

  // ---- everything the home screen needs ----
  async function state() {
    const cfg = await load();
    const spaces = [];
    for (const s of cfg.spaces) { const exists = await spaceExists(s); spaces.push({ ...s, exists, notes: exists ? await countNotes(s) : null }); }
    return { product: PRODUCT, version: VERSION, settings_file: configPath(home), spaces, connections: await connections(cfg), pending: (await listPending(home)).length, apps: await detectClients(clientOpts), levels: LEVELS, kinds: SPACE_KINDS };
  }

  // ---- problems the owner can act on ----
  async function doctor() {
    const checks = [];
    const add = (ok, what, fix) => checks.push({ ok, what, ...(ok ? {} : { fix }) });
    let cfg;
    try { cfg = await load(); add(true, 'Settings file is readable.'); } catch (e) { add(false, e.message, 'Restore config.json from a copy, or move it aside to start fresh.'); return { ok: false, checks }; }
    add(Number(process.versions.node.split('.')[0]) >= 22, `Node.js ${process.versions.node}`, 'Install Node.js 22 or newer.');
    add(cfg.spaces.length > 0, `${cfg.spaces.length} space(s) set up.`, 'Add a space: threadkeep space add <folder> --create');
    for (const s of cfg.spaces) add(await spaceExists(s), `Space "${s.name}" folder is reachable.`, `Folder missing: ${s.path}. Reconnect the drive or sync folder, or remove the space.`);
    const apps = await detectClients(clientOpts);
    for (const c of await connections(cfg)) {
      if (!c.grants.length) add(false, `"${c.name}" has no space granted.`, 'Give it access to a space, or disconnect it.');
      if (CLIENTS[c.client]) add(apps[c.client].configured, `"${c.name}" entry is present in the app's settings.`, `The entry is missing from ${apps[c.client].file}. Connect the app again.`);
      add(c.status === 'verified', c.status === 'verified' ? `"${c.name}" has really called ${PRODUCT} (last: ${c.last_call}).` : `"${c.name}" is set up but has not called ${PRODUCT} yet.`, c.next || 'Restart the app and start a new chat.');
    }
    const pending = (await listPending(home)).length;
    if (pending) add(false, `${pending} proposal(s) are waiting for your approval.`, 'Review them: threadkeep review');
    return { ok: checks.every((c) => c.ok), checks };
  }

  // ---- owner's own reading and writing ----
  const ctxFor = async (as) => { const cfg = await load(); return as ? previewContext(cfg, home, as) : ownerContext(cfg, home); };
  const space = async (name) => {
    const ctx = ownerContext(await load(), home);
    if (!name && ctx.spaces.length === 1) return ctx.spaces[0];
    if (!name) throw new Error(`Say which space: ${ctx.spaces.map((s) => s.name).join(', ') || 'none set up yet'}.`);
    return spaceFor(ctx, name);
  };

  return {
    home, addSpace, updateSpace, removeSpace, addConnection, setGrants, revokeConnection, connections, state, doctor,
    ask: async ({ as, ...a }) => ask(await ctxFor(as), a),
    open: async ({ as, ...a }) => open(await ctxFor(as), a),
    // Run any AI tool exactly as one connection would, without writing anything.
    preview: async ({ as, tool, args }) => { const h = toolHandlers(await ctxFor(as)); if (!as) throw new Error('Choose which connection to preview.'); if (!Object.hasOwn(h, tool)) throw new Error('Unknown tool.'); return h[tool](args || {}); },
    memories: async ({ space: name, status } = {}) => { const cfg = await load(); const out = []; for (const s of name ? [findSpace(cfg, name)] : cfg.spaces) for (const r of await listMemories(s, { status })) out.push({ space: s.name, ...r }); return out; },
    remember: async ({ space: name, ...a }) => saveMemory(await space(name), { ...a, by: 'owner' }),
    retire: async ({ space: name, id }) => retireMemory(await space(name), id),
    forget: async ({ space: name, id }) => forgetMemory(await space(name), id),
    threads: async ({ space: name, all } = {}) => { const cfg = await load(); const out = []; for (const s of name ? [findSpace(cfg, name)] : cfg.spaces) out.push(...await listThreads(s, { includeDone: Boolean(all) })); return out; },
    checkpoint: async ({ space: name, ...a }) => writeCheckpoint(await space(name), { ...a, by: 'owner' }),
    pending: () => listPending(home),
    approve: async ({ id }) => approvePending(home, await load(), id),
    reject: ({ id }) => rejectPending(home, id),
    backup: async ({ space: name, dest }) => {
      const folder = resolveHomePath(dest, userHome);
      const result = await backupSpace(await space(name), folder);
      await atomicWrite(lastBackupFile, JSON.stringify({ dir: folder })).catch(() => {}); // remembered so the owner need not retype it
      return result;
    },
    // dir omitted: the folder used for the most recent backup made on this computer, if any.
    listBackups: async ({ dir } = {}) => {
      const folder = dir ? resolveHomePath(dir, userHome) : await readFile(lastBackupFile, 'utf8').then((raw) => String(JSON.parse(raw).dir || ''), () => '');
      return { dir: folder || null, remembered: !dir && Boolean(folder), backups: folder ? await listBackups(folder) : [] };
    },
    verifyBackup: ({ dir }) => verifyBackup(resolveHomePath(dir, userHome)),
    restore: ({ dir, target }) => restoreBackup(resolveHomePath(dir, userHome), resolveHomePath(target, userHome)),
    activity: ({ n } = {}) => readActivity(home, Math.min(200, Number(n) || 30)),
    overhead,
  };
}
