import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile, writeFile, rm, readdir } from 'node:fs/promises';
import { world } from './helpers.mjs';

test('backup verifies itself, restores into an empty folder, and the restored space works', async () => {
  const w = await world();
  await w.ops.remember({ space: 'Northwind', statement: 'Northwind closes its books on the 5th.', source: 'finance lead' });
  const dest = path.join(w.base, 'backups');
  const b = await w.ops.backup({ space: 'Northwind', dest });
  assert.equal(b.verified, true);
  assert.ok(b.files >= 6);
  assert.equal((await w.ops.verifyBackup({ dir: b.backup })).ok, true);

  const target = path.join(w.base, 'Restored');
  const r = await w.ops.restore({ dir: b.backup, target });
  assert.equal(r.files, b.files);
  assert.equal(await readFile(path.join(target, 'Clients', 'Riverbend.md'), 'utf8'), await readFile(path.join(w.company, 'Clients', 'Riverbend.md'), 'utf8'));
  // The ignore file and private notes come back too: a backup is the owner's full copy.
  assert.ok((await readdir(target)).includes('.threadkeepignore'));

  await w.ops.addSpace({ path: target, name: 'Restored', kind: 'company' });
  const found = await w.ops.ask({ question: 'When does Northwind close its books?', space: 'Restored' });
  assert.equal(found.sources[0].kind, 'memory');
});

test('backups in a folder are listed newest first, and the last backup folder is remembered', async () => {
  const w = await world();
  assert.deepEqual(await w.ops.listBackups(), { dir: null, remembered: false, backups: [] });
  const dest = path.join(w.base, 'backups');
  const first = await w.ops.backup({ space: 'Northwind', dest });
  await new Promise((r) => setTimeout(r, 5));
  const second = await w.ops.backup({ space: 'Mira', dest });
  await writeFile(path.join(dest, 'stray.txt'), 'not a backup');
  const { mkdir } = await import('node:fs/promises');
  await mkdir(path.join(dest, 'Holiday photos'));

  const remembered = await w.ops.listBackups();
  assert.equal(remembered.dir, dest);
  assert.equal(remembered.remembered, true);
  assert.deepEqual(remembered.backups.map((b) => b.dir), [second.backup, first.backup]);
  assert.deepEqual(remembered.backups.map((b) => b.space), ['Mira', 'Northwind']);
  assert.equal(remembered.backups[1].files, first.files);
  assert.equal(remembered.backups[1].bytes, first.bytes);
  assert.equal(remembered.backups[1].kind, 'company');

  const named = await w.ops.listBackups({ dir: dest });
  assert.equal(named.remembered, false);
  assert.equal(named.backups.length, 2);
  assert.deepEqual((await w.ops.listBackups({ dir: path.join(w.base, 'nowhere') })).backups, []);
});

test('a changed or incomplete backup is detected and never restored', async () => {
  const w = await world();
  const b = await w.ops.backup({ space: 'Northwind', dest: path.join(w.base, 'backups') });
  await writeFile(path.join(b.backup, 'files', 'Clients', 'Riverbend.md'), 'tampered');
  await rm(path.join(b.backup, 'files', 'Playbook', 'Onboarding.md'));
  const check = await w.ops.verifyBackup({ dir: b.backup });
  assert.equal(check.ok, false);
  assert.ok(check.problems.some((p) => p.startsWith('changed since backup: Clients/Riverbend.md')));
  assert.ok(check.problems.some((p) => p.startsWith('missing: Playbook/Onboarding.md')));
  const target = path.join(w.base, 'Restored');
  await assert.rejects(w.ops.restore({ dir: b.backup, target }), /failed its check/);
  assert.deepEqual(await readdir(target).catch(() => []), []);
});

test('restore refuses a folder that already has files; backup refuses a folder inside the space', async () => {
  const w = await world();
  const b = await w.ops.backup({ space: 'Northwind', dest: path.join(w.base, 'backups') });
  await assert.rejects(w.ops.restore({ dir: b.backup, target: w.personal }), /not empty/);
  assert.match(await readFile(path.join(w.personal, 'Health.md'), 'utf8'), /PERSONAL_CANARY/);
  await assert.rejects(w.ops.backup({ space: 'Northwind', dest: path.join(w.company, 'backups') }), /outside the space/);
  await assert.rejects(w.ops.verifyBackup({ dir: w.personal }).then((r) => { if (!r.ok) throw new Error(r.problems[0]); }), /not a Threadkeep backup/);
});
