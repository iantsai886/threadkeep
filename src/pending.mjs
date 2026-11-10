// Approval queue. When an AI's write needs the owner's approval, the proposal is stored here,
// in app data, and nothing is written to the space. Only the owner (CLI or local app) can
// approve or reject. An AI cannot approve its own proposal: there is no tool for it.
import path from 'node:path';
import { mkdir, readdir, readFile, unlink } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { atomicWrite } from './home.mjs';
import { prepareMemory, saveMemory } from './memory.mjs';
import { prepareCheckpoint, writeCheckpoint } from './threads.mjs';

const ID = /^p-[a-f0-9]{10}$/;
const dir = (home) => path.join(home, 'pending');
const file = (home, id) => { if (!ID.test(String(id))) throw new Error('That is not a proposal id.'); return path.join(dir(home), `${id}.json`); };
const MAX_PENDING = 200;

export async function listPending(home) {
  let names;
  try { names = await readdir(dir(home)); } catch { return []; }
  const out = [];
  for (const name of names) {
    if (!name.endsWith('.json')) continue;
    try { out.push(JSON.parse(await readFile(path.join(dir(home), name), 'utf8'))); } catch { /* skip */ }
  }
  return out.sort((a, b) => a.created.localeCompare(b.created));
}

// type: 'memory' | 'checkpoint'. The payload is validated now, so the owner never reviews
// something that could not be saved.
export async function queueProposal(home, { type, space, connection, payload }) {
  const prepared = type === 'memory' ? prepareMemory(payload) : type === 'checkpoint' ? prepareCheckpoint(payload) : null;
  if (!prepared) throw new Error('Unknown proposal type.');
  if ((await listPending(home)).length >= MAX_PENDING) throw new Error('Too many proposals are waiting for review. Ask the owner to review them in Threadkeep first.');
  const id = `p-${randomBytes(5).toString('hex')}`;
  const proposal = {
    id, type, space, created: new Date().toISOString(), connection: { id: connection.id, name: connection.name },
    summary: type === 'memory' ? `${prepared.replaces ? `Correct ${prepared.replaces}: ` : ''}${prepared.statement}` : `Checkpoint: ${prepared.title || prepared.thread} — next: ${prepared.next_action}`.slice(0, 400),
    payload: prepared,
  };
  await mkdir(dir(home), { recursive: true });
  await atomicWrite(file(home, id), JSON.stringify(proposal, null, 2) + '\n');
  return proposal;
}

export async function getPending(home, id) {
  try { return JSON.parse(await readFile(file(home, id), 'utf8')); } catch { throw new Error(`No proposal ${id} is waiting.`); }
}

export async function approvePending(home, cfg, id) {
  const p = await getPending(home, id);
  const space = cfg.spaces.find((s) => s.name === p.space);
  if (!space) throw new Error(`The space "${p.space}" no longer exists. Reject this proposal.`);
  const result = p.type === 'memory' ? await saveMemory(space, p.payload) : await writeCheckpoint(space, p.payload);
  await unlink(file(home, id));
  return { approved: id, type: p.type, space: p.space, result };
}

export async function rejectPending(home, id) {
  await getPending(home, id);
  await unlink(file(home, id));
  return { rejected: id };
}
