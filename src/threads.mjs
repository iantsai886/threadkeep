// Work threads: "carry on where the last AI stopped".
// A thread is a folder under <space>/Threadkeep/Threads/<thread-id>/. Every checkpoint is a new
// file; nothing is edited in place. Two computers that both checkpoint through a synced folder
// therefore produce two files, never a conflict. The newest checkpoint is the thread's state.
import path from 'node:path';
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { MANAGED_DIR } from './home.mjs';
import { parseNote, serializeNote, sections, bullets } from './markdown.mjs';
import { assertNoSecret } from './memory.mjs';

export const THREAD_CONTRACT = 'threadkeep.thread/1';
const THREAD_ID = /^t-[a-z0-9]+(?:-[a-z0-9]+)*$/;
const threadsDir = (space) => path.join(space.path, MANAGED_DIR, 'Threads');

const clean = (s) => String(s ?? '').replace(/\r/g, '').trim();
const oneLine = (s) => clean(s).replace(/\s+/g, ' ');

function list(value, name) {
  if (value === undefined || value === null || value === '') return [];
  if (!Array.isArray(value)) throw new Error(`${name} must be a list of short lines.`);
  if (value.length > 12) throw new Error(`Keep ${name} to 12 items.`);
  return value.map((v) => { const s = oneLine(v); if (s.length > 300) throw new Error(`Keep each item in ${name} under 300 characters.`); return s; }).filter(Boolean);
}

// Check and normalise a checkpoint without writing. Used before queueing, too.
export function prepareCheckpoint(input = {}) {
  const thread = input.thread ? String(input.thread).trim() : '';
  if (thread && (!THREAD_ID.test(thread) || thread.length > 80)) throw new Error('thread must be an id returned by resume or checkpoint, such as t-renewal-proposal-3fa2.');
  const title = oneLine(input.title).slice(0, 120);
  if (!thread && !title) throw new Error('A new thread needs a title. To continue an existing one, pass its thread id.');
  const fields = { goal: clean(input.goal), state: clean(input.state), next_action: clean(input.next_action) };
  if (!fields.state) throw new Error('Say where the work stands (state).');
  if (!fields.next_action) throw new Error('Say what should happen next (next_action).');
  for (const [k, v] of Object.entries(fields)) if (v.length > 2000) throw new Error(`Keep ${k} under 2,000 characters.`);
  const status = input.status || 'open';
  if (!['open', 'done'].includes(status)) throw new Error('status must be open or done.');
  const out = { thread, title, ...fields, status, decisions: list(input.decisions, 'decisions'), sources: list(input.sources, 'sources'), open_questions: list(input.open_questions, 'open_questions'), by: oneLine(input.by).slice(0, 120) || 'owner' };
  assertNoSecret(out.title, out.goal, out.state, out.next_action, ...out.decisions, ...out.sources, ...out.open_questions);
  return out;
}

function parseCheckpoint(raw, file) {
  const note = parseNote(raw);
  if (note.meta.contract !== THREAD_CONTRACT || !note.meta.thread) return null;
  const s = sections(note.body);
  // "(none)" and "(not stated)" are written so the file reads well; they are not content.
  const items = (text) => bullets(text).filter((x) => x !== '(none)');
  return {
    thread: String(note.meta.thread), title: String(note.meta.title || ''), status: String(note.meta.status || 'open'),
    goal: s.goal === '(not stated)' ? '' : s.goal || '', state: s.state || '', decisions: items(s.decisions), sources: items(s.sources), open_questions: items(s['open questions']),
    next_action: s['next action'] || '', by: String(note.meta.by || ''), at: String(note.meta.at || ''), file,
  };
}

async function readThread(space, id) {
  if (!THREAD_ID.test(id)) return [];
  let names;
  try { names = await readdir(path.join(threadsDir(space), id)); } catch { return []; }
  const out = [];
  for (const name of names) {
    if (!/\.md$/i.test(name)) continue;
    try { const cp = parseCheckpoint(await readFile(path.join(threadsDir(space), id, name), 'utf8'), name); if (cp && cp.thread === id) out.push(cp); } catch { /* skip */ }
  }
  return out.sort((a, b) => a.at.localeCompare(b.at) || a.file.localeCompare(b.file));
}

const packet = (space, history) => {
  const last = history[history.length - 1];
  return {
    contract: THREAD_CONTRACT, space: space.name, thread: last.thread, title: last.title, status: last.status,
    goal: last.goal, state: last.state, decisions: last.decisions, sources: last.sources, open_questions: last.open_questions, next_action: last.next_action,
    last_checkpoint: { by: last.by, at: last.at }, checkpoints: history.length,
  };
};

export async function writeCheckpoint(space, input, now = new Date()) {
  const p = prepareCheckpoint(input);
  let id = p.thread, history = [];
  if (id) {
    history = await readThread(space, id);
    if (!history.length) throw new Error(`No thread ${id} in "${space.name}". Leave thread empty and give a title to start a new one.`);
  } else {
    const slug = p.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 40).replace(/-$/, '') || 'work';
    id = `t-${slug}-${randomBytes(2).toString('hex')}`;
  }
  const prev = history[history.length - 1];
  const title = p.title || prev?.title || id;
  const goal = p.goal || prev?.goal || '';
  const at = now.toISOString();
  const li = (xs) => (xs.length ? xs.map((x) => `- ${x}`).join('\n') : '- (none)');
  const body = [`# ${title}`, '', '## Goal', '', goal || '(not stated)', '', '## State', '', p.state, '', '## Decisions', '', li(p.decisions), '', '## Sources', '', li(p.sources), '', '## Open questions', '', li(p.open_questions), '', '## Next action', '', p.next_action].join('\n');
  const dir = path.join(threadsDir(space), id);
  await mkdir(dir, { recursive: true });
  const name = `${at.replace(/[:.]/g, '-')}-${randomBytes(2).toString('hex')}.md`;
  await writeFile(path.join(dir, name), serializeNote({ contract: THREAD_CONTRACT, thread: id, title, status: p.status, by: p.by, at }, body), { flag: 'wx' });
  return packet(space, await readThread(space, id));
}

export async function listThreads(space, { includeDone = false } = {}) {
  let ids;
  try { ids = (await readdir(threadsDir(space), { withFileTypes: true })).filter((e) => e.isDirectory()).map((e) => e.name); } catch { return []; }
  const out = [];
  for (const id of ids) {
    const history = await readThread(space, id);
    if (history.length && (includeDone || history[history.length - 1].status === 'open')) out.push(packet(space, history));
  }
  return out.sort((a, b) => b.last_checkpoint.at.localeCompare(a.last_checkpoint.at));
}

export async function getThread(space, id) {
  const history = await readThread(space, String(id || ''));
  if (!history.length) throw new Error(`No thread ${id} in "${space.name}".`);
  return packet(space, history);
}
