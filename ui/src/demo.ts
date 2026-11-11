// In-memory stand-in for the backend, used by `npm run dev:demo`.
// Every company, person and note here is invented.

import type { Connection, Grant, KnownApp, Memory, Proposal, Space, Thread } from './lib/types';

type Args = Record<string, any>;

const spaces: Space[] = [
  { name: 'Acme Tools', path: '/Users/demo/Notes/Acme Tools', kind: 'company', sensitive: false, exists: true, notes: 128 },
  { name: 'Riverbend Co.', path: '/Users/demo/Notes/Riverbend Co.', kind: 'client', sensitive: true, exists: true, notes: 41 },
];

// Same shape as state.apps: keyed by id.
const apps: Record<string, KnownApp> = {
  'claude-desktop': { id: 'claude-desktop', title: 'Claude Desktop', file: '/Users/demo/Library/Application Support/Claude/claude_desktop_config.json', present: true, configured: true, reload: 'Quit Claude Desktop completely, open it again and start a new chat.' },
  'claude-code': { id: 'claude-code', title: 'Claude Code', file: '/Users/demo/.claude.json', present: true, configured: false, reload: 'Start a new Claude Code session, then run /mcp to check.' },
  codex: { id: 'codex', title: 'Codex CLI', file: '/Users/demo/.codex/config.toml', present: true, configured: true, reload: 'Start a new Codex session.' },
  cursor: { id: 'cursor', title: 'Cursor', file: '/Users/demo/.cursor/mcp.json', present: false, configured: false, reload: 'Open Cursor Settings, MCP, and switch Threadkeep on if it is off.' },
};

// polls: how many more `state` calls until the demo marks the connection verified
const connections: (Connection & { polls: number })[] = [
  {
    id: 'c-1a2b', name: 'Claude Desktop', client: 'claude-desktop', status: 'verified', polls: 0, created: '2026-11-02T08:00:00.000Z',
    grants: [{ space: 'Acme Tools', level: 'write' }, { space: 'Riverbend Co.', level: 'propose' }],
  },
  {
    id: 'c-3c4d', name: 'Codex CLI', client: 'codex', status: 'configured', polls: 1e9, created: '2026-11-10T07:30:00.000Z',
    next: 'Start a new Codex session.',
    grants: [{ space: 'Acme Tools', level: 'read' }],
  },
];

let proposals: Proposal[] = [
  {
    id: 'p-01', type: 'memory', space: 'Riverbend Co.', created: '2026-11-10T09:12:44.120Z',
    connection: { id: 'c-1a2b', name: 'Claude Desktop' },
    summary: 'Riverbend Co. renews its support contract every March.',
    payload: { statement: 'Riverbend Co. renews its support contract every March.', source: 'owner said so in chat, 2026-11-10', kind: 'fact', confidence: 'reported' },
  },
  {
    id: 'p-02', type: 'checkpoint', space: 'Riverbend Co.', created: '2026-11-10T09:20:00.000Z',
    connection: { id: 'c-1a2b', name: 'Claude Desktop' },
    summary: 'Renewal proposal',
    payload: {
      title: 'Renewal proposal', goal: 'Send Riverbend a renewal proposal before the end of the month.',
      state: 'Outline agreed. Pricing section is still empty.', decisions: ['Offer a two-year term'],
      open_questions: ['Does Riverbend want on-site training again?'], next_action: 'Draft the pricing section.',
    },
  },
];

const memories: Record<string, Memory[]> = {
  'Acme Tools': [
    { id: 'm-20261101-a41c9e', kind: 'decision', status: 'active', confidence: 'verified', source: 'Planning meeting, 2026-11-01', created: '2026-11-01T10:00:00.000Z', by: 'owner', statement: 'Acme Tools ships orders on Tuesdays and Fridays.' },
    { id: 'm-20260302-9be0c1', kind: 'fact', status: 'superseded', superseded_by: 'm-20261101-a41c9e', confidence: 'reported', source: 'Old handbook', created: '2026-03-02T10:00:00.000Z', by: 'Claude Desktop', statement: 'Acme Tools ships orders on Mondays.' },
    { id: 'm-20261105-77d2aa', kind: 'preference', status: 'active', confidence: 'reported', source: 'owner said so in chat, 2026-11-05', created: '2026-11-05T14:20:00.000Z', by: 'Claude Desktop', statement: 'Quotes are written in plain language, with prices excluding tax.' },
  ],
  'Riverbend Co.': [],
};

const threads: Thread[] = [
  { thread: 't-price-list-update-3fa2', title: 'Price list update', status: 'open', space: 'Acme Tools', state: 'New prices entered for 40 of 43 items.', next_action: 'Check the three items marked "ask supplier".', last_checkpoint: { by: 'Claude Desktop', at: '2026-11-09T16:05:00.000Z' }, checkpoints: 3 },
];

// Backups made in this demo session, by folder.
const backups: Record<string, { space: string; files: number }> = {};

const fail = (sentence: string): never => {
  throw new Error(sentence);
};
const newId = (prefix: string) => `${prefix}-${Math.random().toString(16).slice(2, 8)}`;
const findConnection = (id: string) => connections.find((c) => c.id === id) ?? fail('That connection does not exist.');

const commands: Record<string, (a: Args) => unknown> = {
  state() {
    for (const c of connections) if (c.status === 'configured' && --c.polls <= 0) c.status = 'verified';
    return {
      spaces,
      connections: connections.map(({ polls, ...c }) => (c.status === 'verified' ? { ...c, next: undefined } : c)),
      pending: proposals.length,
      apps,
    };
  },
  doctor: () => ({
    ok: false,
    checks: [
      { ok: true, what: 'Settings file is readable.' },
      { ok: true, what: '2 space(s) set up.' },
      { ok: true, what: '"Claude Desktop" has really called Threadkeep (last: 2026-11-10T08:55:00.000Z).' },
      { ok: false, what: '"Codex CLI" is set up but has not called Threadkeep yet.', fix: 'Start a new Codex session.' },
    ],
  }),
  addSpace(a) {
    if (!a.path) fail('Choose a folder for the space.');
    const name = a.name || String(a.path).split(/[\\/]/).filter(Boolean).pop();
    if (spaces.some((s) => s.name === name)) fail(`A space called ${name} already exists.`);
    const kind = a.kind ?? 'personal';
    spaces.push({ name, path: a.path, kind, sensitive: a.sensitive ?? kind !== 'company', exists: true, notes: 0 });
    memories[name] = [];
    return { name };
  },
  updateSpace(a) {
    const s = spaces.find((x) => x.name === a.name) ?? fail('That space does not exist.');
    if (typeof a.sensitive === 'boolean') s.sensitive = a.sensitive;
    return { name: s.name };
  },
  removeSpace(a) {
    const i = spaces.findIndex((x) => x.name === a.name);
    if (i >= 0) spaces.splice(i, 1);
    for (const c of connections) c.grants = c.grants.filter((g) => g.space !== a.name);
    return { removed: a.name, note: `"${a.name}" was removed from Threadkeep. Its folder and every file in it were left as they are.` };
  },
  addConnection(a) {
    if (!a.grants?.length) fail('Choose at least one space for this AI app.');
    const known = apps[a.client];
    const old = known ? connections.findIndex((c) => c.client === a.client) : -1;
    if (old >= 0) connections.splice(old, 1);
    const id = newId('c');
    const name = a.name || known?.title || 'Custom AI';
    const next = known ? known.reload : 'Paste this entry into the app\'s MCP settings. The key is shown only once.';
    connections.push({ id, name, client: a.client, status: 'configured', polls: 4, next, grants: a.grants, created: new Date().toISOString() });
    const verify = 'After that, ask the AI: "What can you do in Threadkeep?" This connection shows as verified once the app really calls Threadkeep.';
    return known
      ? { id, name, file: known.file, backup: `${known.file}.threadkeep-backup-2026-11-10T09-30-00-000Z`, next, verify }
      : { id, name, token: 'demo-key-not-real', snippet: JSON.stringify({ mcpServers: { threadkeep: { command: 'threadkeep', args: ['serve'], env: { THREADKEEP_KEY: 'demo-key-not-real' } } } }, null, 2), next, verify };
  },
  setGrants(a) {
    if (!a.grants?.length) fail('Choose at least one space for this AI app.');
    findConnection(a.id).grants = a.grants as Grant[];
    return { id: a.id };
  },
  revokeConnection(a) {
    const i = connections.findIndex((c) => c.id === a.id);
    const [gone] = i >= 0 ? connections.splice(i, 1) : [];
    return { revoked: a.id, note: `"${gone?.name ?? a.id}" can no longer use Threadkeep, starting with its next request.` };
  },
  pending: () => proposals,
  approve(a) {
    const p = proposals.find((x) => x.id === a.id);
    if (p?.type === 'memory') {
      (memories[p.space] ??= []).unshift({
        id: newId('m-20261110'), kind: String(p.payload.kind ?? 'fact'), status: 'active', confidence: String(p.payload.confidence ?? 'reported'),
        source: String(p.payload.source ?? ''), created: new Date().toISOString(), by: p.connection.name, statement: String(p.payload.statement),
      });
    }
    proposals = proposals.filter((x) => x.id !== a.id);
    return { approved: true };
  },
  reject(a) {
    proposals = proposals.filter((x) => x.id !== a.id);
    return { rejected: true };
  },
  ask(a) {
    const allowed = a.as ? findConnection(a.as).grants.map((g) => g.space) : spaces.map((s) => s.name);
    const words = String(a.question ?? '').toLowerCase().split(/\W+/).filter((w) => w.length > 3);
    const sources = Object.entries(memories)
      .filter(([space]) => allowed.includes(space) && (!a.space || a.space === space))
      .flatMap(([space, list]) =>
        list
          .filter((m) => m.status === 'active' || (a.history && m.status === 'superseded'))
          .filter((m) => words.some((w) => m.statement.toLowerCase().includes(w)))
          .map((m) => ({
            ref: `${space}::Threadkeep/Memories/${m.id}.md`, space, kind: 'memory', id: m.id, date: m.created?.slice(0, 10),
            confidence: m.confidence, outdated: m.status === 'superseded' || undefined, replaced_by: m.superseded_by, excerpt: m.statement,
          })),
      );
    const covered = words.filter((w) => sources.some((s) => s.excerpt.toLowerCase().includes(w)));
    const status = !sources.length ? 'none' : covered.length >= words.length * 0.6 ? 'found' : 'partial';
    return { status, covered, missing: words.filter((w) => !covered.includes(w)), more_available: false, sources, est_tokens: sources.length * 30 };
  },
  open: (a) => ({ ref: a.ref, text: 'In the demo, the full section is the same as the excerpt above.' }),
  memories: (a) => memories[a.space] ?? [],
  remember(a) {
    if (!a.statement) fail('Write the statement to remember.');
    const id = newId('m-20261110');
    const list = (memories[a.space] ??= []);
    const same = !a.replaces && list.find((m) => m.status === 'active' && m.statement.trim().toLowerCase() === String(a.statement).trim().toLowerCase());
    if (same) return { saved: false, duplicate_of: same.id };
    const old = list.find((m) => m.id === a.replaces);
    if (old) Object.assign(old, { status: 'superseded', superseded_by: id });
    list.unshift({ id, kind: a.kind ?? 'fact', status: 'active', confidence: a.confidence ?? 'verified', source: a.source, created: new Date().toISOString(), by: 'owner', statement: a.statement });
    return { saved: true, id };
  },
  retire(a) {
    const m = (memories[a.space] ?? []).find((x) => x.id === a.id);
    if (m) m.status = 'retired';
    return { retired: true };
  },
  forget(a) {
    memories[a.space] = (memories[a.space] ?? []).filter((x) => x.id !== a.id);
    return { forgotten: a.id, not_removed: ['Backups made before now still contain it.', 'Any AI conversation that already received it keeps its own copy.'] };
  },
  backup(a) {
    const s = spaces.find((x) => x.name === a.space) ?? fail(`No space called "${a.space}".`);
    if (!/^(\/|~|[A-Za-z]:\\)/.test(String(a.dest ?? ''))) fail('Choose a backup folder (absolute path).');
    const backup = `${String(a.dest).replace(/[\\/]$/, '')}/${s.name.replace(/[^\w.-]+/g, '_')}-2026-11-10T09-30-00-000Z`;
    backups[backup] = { space: s.name, files: (s.notes ?? 0) + 4 };
    return { backup, files: backups[backup].files, bytes: backups[backup].files * 2140, verified: true };
  },
  activity: (a) =>
    [
      { at: '2026-11-10T09:20:00.000Z', tool: 'checkpoint', connection: 'c-1a2b', name: 'Claude Desktop', ok: true, ms: 14, est_tokens: 61, queued: true },
      { at: '2026-11-10T09:12:44.120Z', tool: 'save', connection: 'c-1a2b', name: 'Claude Desktop', ok: true, ms: 11, est_tokens: 58, queued: true },
      { at: '2026-11-10T09:10:02.000Z', tool: 'ask', connection: 'c-1a2b', name: 'Claude Desktop', ok: true, ms: 37, est_tokens: 412, status: 'found' },
      { at: '2026-11-10T09:09:40.000Z', tool: 'ask', connection: 'c-1a2b', name: 'Claude Desktop', ok: true, ms: 29, est_tokens: 44, status: 'none' },
      { at: '2026-11-10T08:55:00.000Z', tool: 'status', connection: 'c-1a2b', name: 'Claude Desktop', ok: true, ms: 6, est_tokens: 120 },
      { at: '2026-11-09T17:30:00.000Z', tool: 'ask', connection: 'c-9z9z', name: 'Old Laptop AI', ok: false, ms: 2 },
      { at: '2026-11-09T17:29:00.000Z', tool: 'status', connection: null, name: null, ok: false, ms: 1 },
    ].slice(0, a.n ?? 30),
  overhead: () => ({ tools: 6, total_est_tokens: 700, method: 'characters / 4; an estimate, not a provider-reported count' }),
  listBackups(a) {
    const all = Object.entries(backups).map(([dir, b]) => ({ dir, space: b.space, kind: 'company', created: '2026-11-10T09:30:00.000Z', files: b.files, bytes: b.files * 2140 }));
    const last = all.length ? all[all.length - 1].dir.replace(/\/[^/]+$/, '') : null;
    const dir = a.dir || last;
    return { dir, remembered: !a.dir && Boolean(dir), backups: all.filter((b) => b.dir.startsWith(`${dir}/`)).reverse() };
  },
  verifyBackup(a) {
    const b = backups[a.dir];
    if (!b) return { ok: false, problems: ['This folder is not a Threadkeep backup (no readable manifest).'] };
    return { ok: true, problems: [], space: b.space, created: '2026-11-10T09:30:00.000Z', files: b.files };
  },
  restore(a) {
    const b = backups[a.dir] ?? fail('This backup failed its check and was not restored: This folder is not a Threadkeep backup (no readable manifest).');
    if (spaces.some((s) => s.path === a.target)) fail('The destination folder is not empty. Restore into a new or empty folder; nothing was changed.');
    return { restored_to: a.target, files: b.files, space: b.space, next: 'Add this folder as a space in Threadkeep to use it.' };
  },
  checkpoint(a) {
    if (!a.state) fail('Say where the work stands (state).');
    if (!a.next_action) fail('Say what should happen next (next_action).');
    let t = threads.find((x) => x.thread === a.thread && x.space === a.space);
    if (a.thread && !t) fail(`No thread ${a.thread} in "${a.space}". Leave thread empty and give a title to start a new one.`);
    if (!t) {
      if (!a.title) fail('A new thread needs a title. To continue an existing one, pass its thread id.');
      t = { thread: newId('t-demo'), title: a.title, status: 'open', space: a.space, checkpoints: 0 };
      threads.unshift(t);
    }
    Object.assign(t, {
      goal: a.goal || t.goal, state: a.state, next_action: a.next_action, status: a.status ?? 'open',
      decisions: a.decisions ?? [], sources: a.sources ?? [], open_questions: a.open_questions ?? [],
      last_checkpoint: { by: 'owner', at: new Date().toISOString() }, checkpoints: (t.checkpoints ?? 0) + 1,
    });
    return t;
  },
  threads: (a) => threads.filter((t) => (!a.space || t.space === a.space) && (a.all || t.status === 'open')),
};

export async function demoCall(command: string, args: object): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 120));
  const run = commands[command] ?? fail(`"${command}" is not part of the demo.`);
  // Hand back a copy, as a real transport would.
  return JSON.parse(JSON.stringify(run(args as Args) ?? null));
}
