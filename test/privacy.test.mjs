import { test } from 'node:test';
import assert from 'node:assert/strict';
import { world, connectAi } from './helpers.mjs';

const CANARIES = ['COMPANY_PRIVATE_CANARY', 'MALFORMED_CANARY', 'IGNORED_FOLDER_CANARY', 'PERSONAL_CANARY'];

test('an AI never receives private notes, malformed-header notes, ignored folders or ungranted spaces', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }]);
  for (const question of ['Dana salary confidential amount', 'never closed hidden', 'performance reviews notes', 'Mira morning appointments', 'canary']) {
    const r = await ai.call('ask', { question, limit: 8 });
    assert.equal(r.error, null);
    for (const c of CANARIES) assert.ok(!r.text.includes(c), `${c} leaked for "${question}"`);
    assert.ok(r.data.sources.every((s) => s.space === 'Northwind'));
  }
  for (const ref of ['Northwind::Finance/Salaries.md', 'Northwind::Broken.md', 'Northwind::HR/Reviews.md', 'Mira::Health.md', 'Northwind::../Mira/Health.md', 'Northwind::.threadkeepignore']) {
    const r = await ai.call('open', { ref });
    assert.ok(r.error, `open ${ref} should be refused`);
    for (const c of CANARIES) assert.ok(!r.text.includes(c));
  }
  const named = await ai.call('ask', { question: 'appointments', space: 'Mira' });
  assert.match(named.error, /No space named "Mira" is available here/);
});

test('the owner can still search their own private notes', async () => {
  const w = await world();
  const r = await w.ops.ask({ question: 'Dana confidential amount' });
  assert.ok(JSON.stringify(r).includes('COMPANY_PRIVATE_CANARY'));
  // ...but an ignored folder stays out for everyone: it is the owner's "never read this" switch.
  assert.ok(!JSON.stringify(await w.ops.ask({ question: 'performance reviews' })).includes('IGNORED_FOLDER_CANARY'));
});

test('"see as this AI" shows exactly what the connection would get', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'read' }]);
  const preview = await w.ops.ask({ question: 'Dana confidential amount salary', as: ai.id });
  assert.ok(!JSON.stringify(preview).includes('COMPANY_PRIVATE_CANARY'));
  const real = await ai.call('ask', { question: 'Dana confidential amount salary' });
  assert.deepEqual(preview.sources.map((s) => s.ref), real.data.sources.map((s) => s.ref));
});

test('no key, a wrong key or a revoked key gets nothing', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'write' }]);
  assert.equal((await ai.call('ask', { question: 'Riverbend contract' })).error, null);
  await w.ops.revokeConnection({ id: ai.id });
  // Same server object, no restart: the next request is refused.
  const refused = await ai.call('ask', { question: 'Riverbend contract' });
  assert.match(refused.error, /not connected/);
  assert.ok(!refused.text.includes('Riverbend support contract'));
  const { createMcp } = await import('../src/mcp.mjs');
  const { connectionContext } = await import('../src/access.mjs');
  const { loadConfig } = await import('../src/home.mjs');
  for (const token of [undefined, '', 'tk_' + 'a'.repeat(48)]) {
    const mcp = createMcp(async () => connectionContext(await loadConfig(w.home), w.home, token));
    const r = await mcp.handle({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'ask', arguments: { question: 'Riverbend' } } });
    assert.equal(r.result.isError, true);
  }
});

test('changing access takes effect on the next request', async () => {
  const w = await world();
  const ai = await connectAi(w, [{ space: 'Northwind', level: 'read' }]);
  assert.match((await ai.call('save', { statement: 'Riverbend prefers email.', source: 'test' })).error, /read-only/);
  await w.ops.setGrants({ id: ai.id, grants: [{ space: 'Northwind', level: 'write' }] });
  assert.equal((await ai.call('save', { statement: 'Riverbend prefers email.', source: 'test' })).data.saved, true);
});

test('spaces cannot overlap, and removing a space keeps its folder', async () => {
  const w = await world();
  await assert.rejects(w.ops.addSpace({ path: w.company + '/Clients', name: 'Inner' }), /overlaps/);
  const r = await w.ops.removeSpace({ name: 'Mira' });
  assert.equal(r.folder_kept, w.personal);
  const { stat } = await import('node:fs/promises');
  assert.ok((await stat(w.personal + '/Health.md')).isFile());
});
