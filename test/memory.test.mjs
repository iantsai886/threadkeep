import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { world, connectAi } from './helpers.mjs';

const memoryFiles = async (root) => readdir(path.join(root, 'Threadkeep', 'Memories')).catch(() => []);

test('propose level: a save waits for the owner and writes nothing until approved', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'propose' }]);
  const r = await ai.call('save', { statement: 'Riverbend wants invoices on the first of the month.', source: 'client call, fictional' });
  assert.equal(r.data.saved, false);
  assert.equal(r.data.waiting_for_approval, true);
  assert.deepEqual(await memoryFiles(w.company), []);
  assert.equal((await ai.call('ask', { question: 'Riverbend invoices first of the month' })).data.sources.some((s) => s.kind === 'memory'), false);
  assert.equal((await ai.call('status')).data.waiting_for_approval, 1);

  const [p] = await w.ops.pending();
  assert.equal(p.connection.name, 'Test AI');
  const done = await w.ops.approve({ id: p.id });
  assert.equal(done.result.saved, true);
  assert.equal((await memoryFiles(w.company)).length, 1);
  const found = await ai.call('ask', { question: 'When does Riverbend want invoices?' });
  assert.equal(found.data.sources[0].kind, 'memory');
  assert.equal(found.data.sources[0].confidence, 'reported');
  assert.deepEqual(await w.ops.pending(), []);
});

test('a rejected proposal leaves no trace in the space', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'propose' }]);
  await ai.call('save', { statement: 'A wrong guess about Riverbend.', source: 'inference' });
  const [p] = await w.ops.pending();
  await w.ops.reject({ id: p.id });
  assert.deepEqual(await memoryFiles(w.company), []);
  assert.deepEqual(await w.ops.pending(), []);
});

test('a sensitive space needs approval even at write level, for memories and checkpoints', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Mira', level: 'write' }]);
  const status = await ai.call('status');
  assert.equal(status.data.spaces[0].sensitive, true);
  assert.equal(status.data.spaces[0].save, 'waits for owner approval');
  assert.equal((await ai.call('save', { statement: 'Mira likes window seats.', source: 'said in chat' })).data.waiting_for_approval, true);
  assert.equal((await ai.call('checkpoint', { title: 'Trip plan', state: 'Flights compared', next_action: 'Book the morning flight' })).data.waiting_for_approval, true);
  assert.deepEqual(await readdir(path.join(w.personal, 'Threadkeep')).catch(() => []), []);
  for (const p of await w.ops.pending()) await w.ops.approve({ id: p.id });
  assert.equal((await w.ops.memories({ space: 'Mira' })).length, 1);
  assert.equal((await w.ops.threads({ space: 'Mira' })).length, 1);
});

test('a correction replaces the old memory, which stays as history', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }]);
  const first = await ai.call('save', { statement: 'The Riverbend account lead is Dana Whitfield.', source: 'kickoff notes' });
  const fixed = await ai.call('save', { statement: 'The Riverbend account lead is Priya Raman.', source: 'owner correction in chat', replaces: first.data.id });
  assert.equal(fixed.data.replaced, first.data.id);

  const now = await ai.call('ask', { question: 'Who is the Riverbend account lead?' });
  const memories = now.data.sources.filter((s) => s.kind === 'memory');
  assert.deepEqual(memories.map((s) => s.id), [fixed.data.id]);
  assert.match(memories[0].excerpt, /Priya Raman/);

  const past = await ai.call('ask', { question: 'Who is the Riverbend account lead?', history: true });
  const old = past.data.sources.find((s) => s.id === first.data.id);
  assert.equal(old.outdated, true);
  assert.equal(old.replaced_by, fixed.data.id);

  // Correcting the already-replaced record is refused instead of forking history.
  assert.match((await ai.call('save', { statement: 'Someone else.', source: 'x', replaces: first.data.id })).error, /superseded/);
});

test('duplicates are skipped, secrets are refused, kind and source are checked', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }]);
  const a = await ai.call('save', { statement: 'Northwind closes its books on the 5th.', source: 'finance lead' });
  const b = await ai.call('save', { statement: 'northwind closes its books on the 5th', source: 'again' });
  assert.equal(b.data.already_known, a.data.id);
  for (const statement of ['The portal password is hunter2-swordfish', 'Use key sk-proj-abcdefghijklmnopqrstuvwxyz123456', 'Card 4111 1111 1111 1111 is on file']) {
    assert.match((await ai.call('save', { statement, source: 'chat' })).error, /does not store credentials/);
  }
  assert.match((await ai.call('save', { statement: 'No source given.' })).error, /source/);
  assert.match((await ai.call('save', { statement: 'Bad kind.', source: 'x', kind: 'rumour' })).error, /kind must be/);
  assert.equal((await memoryFiles(w.company)).length, 1);
});

test('saving needs a named space when more than one accepts saves', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }, { space: 'Mira', level: 'propose' }]);
  assert.match((await ai.call('save', { statement: 'Ambiguous.', source: 'x' })).error, /Say which space/);
  assert.equal((await ai.call('save', { statement: 'Goes to the company.', source: 'x', space: 'Northwind' })).data.saved, true);
});

test('retire hides a memory; forget deletes it and says what it could not remove', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }]);
  const m = await ai.call('save', { statement: 'Zephyr Labs is a prospect for the spring campaign.', source: 'sales note' });
  await w.ops.retire({ space: 'Northwind', id: m.data.id });
  assert.equal((await ai.call('ask', { question: 'Zephyr Labs prospect' })).data.status, 'none');
  assert.equal((await ai.call('ask', { question: 'Zephyr Labs prospect', history: true })).data.status, 'none');
  const r = await w.ops.forget({ space: 'Northwind', id: m.data.id });
  assert.deepEqual(await memoryFiles(w.company), []);
  assert.ok(r.not_removed.some((x) => /Backups/.test(x)));
});

test('a memory file edited by hand or duplicated by a sync tool does not break anything', async () => {
  const w = await world();
  const saved = await w.ops.remember({ space: 'Northwind', statement: 'The office is closed on Fridays.', source: 'owner' });
  const dir = path.join(w.company, 'Threadkeep', 'Memories');
  const raw = await readFile(path.join(dir, `${saved.id}.md`), 'utf8');
  await writeFile(path.join(dir, `${saved.id} (conflicted copy).md`), raw);
  await writeFile(path.join(dir, 'notes.txt'), 'stray');
  await writeFile(path.join(dir, `${saved.id}.md`), raw.replace('Fridays', 'Mondays'));
  const list = await w.ops.memories({ space: 'Northwind' });
  assert.equal(list.length, 1);
  assert.match(list[0].statement, /Mondays/);
});
