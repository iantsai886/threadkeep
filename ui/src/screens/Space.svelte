<script lang="ts">
  import { call } from '../backend';
  import Confirm from '../lib/Confirm.svelte';
  import LevelPicker from '../lib/LevelPicker.svelte';
  import ThreadForm from '../lib/ThreadForm.svelte';
  import { app, count, flash, go, levelOf, link, message, refresh, when, withLevel, type Choice } from '../lib/store.svelte';
  import type { Connection, ForgetResult, Memory, RememberResult, RemoveSpaceResult, Thread } from '../lib/types';

  let { name }: { name: string } = $props();

  const space = $derived(app.state?.spaces.find((s) => s.name === name));
  const connections = $derived(app.state?.connections ?? []);

  const MEMORY_KINDS = ['fact', 'decision', 'preference', 'procedure', 'episode'];

  let error = $state('');
  let busy = $state(false);
  let memories = $state<Memory[]>([]);
  let threads = $state<Thread[]>([]);
  let loaded = $state(false);
  let showOld = $state(false);
  let removing = $state(false);
  let forgetting = $state<Memory | null>(null);
  // The thread form: 'new', or the id of the thread getting a checkpoint.
  let checkpointing = $state('');

  async function checkpointed(saved: Thread) {
    checkpointing = '';
    error = '';
    notice = [saved.checkpoints === 1 ? `Thread “${saved.title}” started.` : `Checkpoint saved to “${saved.title}”${saved.status === 'done' ? ', and the thread is marked done' : ''}.`];
    await load();
  }

  // Add or correct a memory. `replaces` is set while correcting.
  let statement = $state('');
  let source = $state('');
  let kind = $state('fact');
  let replaces = $state<Memory | null>(null);
  let statementInput = $state<HTMLTextAreaElement>();

  const shown = $derived(memories.filter((m) => showOld || m.status === 'active'));
  const hidden = $derived(memories.length - memories.filter((m) => m.status === 'active').length);

  // Shown after a save or a forget: what did and did not happen.
  let notice = $state<string[]>([]);

  async function load() {
    try {
      [memories, threads] = await Promise.all([call<Memory[]>('memories', { space: name }), call<Thread[]>('threads', { space: name, all: true })]);
      loaded = true;
    } catch (e) {
      error = message(e);
    }
  }
  load();

  async function run(work: () => Promise<unknown>) {
    busy = true;
    error = '';
    notice = [];
    try {
      await work();
    } catch (e) {
      error = message(e);
    } finally {
      await Promise.all([refresh(), load()]);
      busy = false;
    }
  }

  const setSensitive = (sensitive: boolean) => run(() => call('updateSpace', { name, sensitive }));
  const setLevel = (c: Connection, level: Choice) => run(() => call('setGrants', { id: c.id, grants: withLevel(c.grants, name, level) }));
  const retire = (m: Memory) => run(() => call('retire', { space: name, id: m.id }));

  function forget() {
    const m = forgetting;
    forgetting = null;
    if (m)
      run(async () => {
        const r = await call<ForgetResult>('forget', { space: name, id: m.id });
        notice = ['Forgotten. What Threadkeep could not remove:', ...(r.not_removed ?? [])];
      });
  }

  function correct(m: Memory) {
    replaces = m;
    statement = m.statement;
    kind = m.kind;
    source = '';
    statementInput?.focus();
  }

  function clearForm() {
    replaces = null;
    statement = '';
    source = '';
  }

  function remember(e: SubmitEvent) {
    e.preventDefault();
    run(async () => {
      const r = await call<RememberResult>('remember', { space: name, statement: statement.trim(), source: source.trim(), kind, replaces: replaces?.id });
      if (!r.saved && r.duplicate_of) notice = [`Already remembered, so nothing was added (${r.duplicate_of}).`];
      clearForm();
    });
  }

  async function remove() {
    removing = false;
    try {
      const r = await call<RemoveSpaceResult>('removeSpace', { name });
      await refresh();
      flash(r?.note);
      go('home');
    } catch (e) {
      error = message(e);
    }
  }
</script>

{#if !space}
  <h1>Space not found</h1>
  <p><a href={link('home')}>Back to Home</a></p>
{:else}
  <div class="stack" style="gap:6px">
    <div class="row">
      <h1>{space.name}</h1>
      <span class="badge">{space.kind}</span>
      {#if space.sensitive}<span class="badge warn">Sensitive</span>{/if}
    </div>
    <p class="path muted">{space.path}</p>
    {#if typeof space.notes === 'number'}<p class="small muted">{count(space.notes, 'note')}</p>{/if}
  </div>

  {#if space.exists === false}
    <p class="error">This folder cannot be reached. Reconnect the drive or sync folder, or remove the space.</p>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice.length}
    <div class="note plain" role="status">
      <p>{notice[0]}</p>
      {#if notice.length > 1}
        <ul style="margin:6px 0 0;padding-left:20px">{#each notice.slice(1) as line}<li>{line}</li>{/each}</ul>
      {/if}
    </div>
  {/if}

  <section class="card">
    <h2>AI apps in this space</h2>
    <label class="check">
      <input type="checkbox" checked={space.sensitive} disabled={busy} onchange={(e) => setSensitive(e.currentTarget.checked)} />
      <span>Sensitive: everything an AI saves in this space waits for your approval.</span>
    </label>
    {#if connections.length}
      {#each connections as c (c.id)}
        <LevelPicker label={c.name} sensitive={space.sensitive} disabled={busy} value={levelOf(c, name)} onchange={(v) => setLevel(c, v)} />
      {/each}
    {:else}
      <p class="muted">No AI app is connected yet. <a href={link('add-connection')}>Connect an AI app</a></p>
    {/if}
  </section>

  <section class="card">
    <div class="row spread">
      <h2>Memories</h2>
      {#if hidden}
        <label class="check small"><input type="checkbox" bind:checked={showOld} /> Show superseded and retired ({hidden})</label>
      {/if}
    </div>

    {#if shown.length}
      <ul class="list">
        {#each shown as m (m.id)}
          <li>
            <p>{m.statement}</p>
            <div class="row small muted">
              <span class="badge" class:ok={m.status === 'active'}>{m.status}</span>
              <span>{m.kind}</span>
              {#if m.confidence}<span>{m.confidence}</span>{/if}
              {#if m.source}<span>Source: {m.source}</span>{/if}
              {#if m.by}<span>by {m.by}</span>{/if}
              <span>{when(m.created)}</span>
              {#if m.due}<span class="badge warn">Review due</span>{/if}
              {#if m.superseded_by}<span>Replaced by {m.superseded_by}</span>{/if}
            </div>
            <div class="row">
              {#if m.status === 'active'}
                <button class="small" disabled={busy} onclick={() => correct(m)}>Correct</button>
                <button class="small" disabled={busy} onclick={() => retire(m)}>Retire</button>
              {/if}
              <button class="small danger" disabled={busy} onclick={() => (forgetting = m)}>Forget</button>
            </div>
          </li>
        {/each}
      </ul>
    {:else if loaded}
      <p class="muted">No memories in this space yet.</p>
    {/if}

    <form class="stack" onsubmit={remember} style="border-top:1px solid var(--line);padding-top:14px">
      <h3>{replaces ? 'Correct a memory' : 'Remember something'}</h3>
      {#if replaces}
        <p class="note plain small">The earlier memory is kept and marked superseded: “{replaces.statement}”</p>
      {/if}
      <label class="field">
        Statement
        <textarea bind:value={statement} bind:this={statementInput} required></textarea>
      </label>
      <label class="field">
        Source <span class="hint small muted">Where this comes from, so it can be checked later.</span>
        <input type="text" bind:value={source} required autocomplete="off" />
      </label>
      <label class="field" style="max-width:220px">
        Kind
        <select bind:value={kind}>
          {#each MEMORY_KINDS as k}<option value={k}>{k}</option>{/each}
        </select>
      </label>
      <div class="row">
        <button class="primary" disabled={busy || !statement.trim() || !source.trim()}>{replaces ? 'Save correction' : 'Remember'}</button>
        {#if replaces}<button type="button" onclick={clearForm}>Cancel</button>{/if}
      </div>
    </form>
  </section>

  <section class="card">
    <h2>Threads</h2>
    {#if threads.length}
      <ul class="list">
        {#each threads as t (t.thread)}
          <li>
            <div class="row">
              <strong>{t.title}</strong>
              <span class="badge" class:ok={t.status === 'open'}>{t.status}</span>
            </div>
            {#if t.goal}<p><span class="muted">Goal:</span> {t.goal}</p>{/if}
            {#if t.state}<p><span class="muted">State:</span> {t.state}</p>{/if}
            {#if t.next_action}<p><span class="muted">Next action:</span> {t.next_action}</p>{/if}
            {#each [['Decisions', t.decisions], ['Open questions', t.open_questions], ['Sources', t.sources]] as const as [label, list]}
              {#if list?.length}
                <div>
                  <span class="muted">{label}:</span>
                  <ul style="margin:2px 0 0;padding-left:20px">{#each list as line}<li>{line}</li>{/each}</ul>
                </div>
              {/if}
            {/each}
            {#if t.last_checkpoint}
              <p class="small muted">
                {t.checkpoints ? `${count(t.checkpoints, 'checkpoint')} · ` : ''}Last by {t.last_checkpoint.by} · {when(t.last_checkpoint.at)}
              </p>
            {/if}
            {#if checkpointing === t.thread}
              <ThreadForm space={name} thread={t} ondone={checkpointed} oncancel={() => (checkpointing = '')} />
            {:else}
              <div class="row">
                <button class="small" disabled={busy} onclick={() => (checkpointing = t.thread)}>
                  {t.status === 'done' ? 'Reopen with a checkpoint' : 'Add a checkpoint'}
                </button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {:else if loaded}
      <p class="muted">No threads in this space yet. A thread is unfinished work that any permitted AI can resume.</p>
    {/if}
    {#if checkpointing === 'new'}
      <div style="border-top:1px solid var(--line);padding-top:14px">
        <ThreadForm space={name} ondone={checkpointed} oncancel={() => (checkpointing = '')} />
      </div>
    {:else}
      <div><button disabled={busy} onclick={() => (checkpointing = 'new')}>Start a thread</button></div>
    {/if}
  </section>

  <section class="card">
    <h2>Backup</h2>
    <p class="muted">Copy this space's folder to another place, so it can be restored if the folder is lost or damaged.</p>
    <div><a class="button" href={link('backup', space.name)}>Back up this space</a></div>
  </section>

  <section class="card">
    <h2>Remove this space</h2>
    <p class="muted">Threadkeep stops using the folder. The folder and everything in it stay where they are.</p>
    <div><button class="danger" onclick={() => (removing = true)}>Remove space</button></div>
  </section>

  {#if removing}
    <Confirm title={`Remove ${space.name}?`} confirmLabel="Remove" danger onconfirm={remove} oncancel={() => (removing = false)}>
      <p>No AI app will be able to use this space any more. The folder is not deleted:</p>
      <p class="path">{space.path}</p>
    </Confirm>
  {/if}

  {#if forgetting}
    <Confirm title="Forget this memory?" confirmLabel="Forget" danger onconfirm={forget} oncancel={() => (forgetting = null)}>
      <p>“{forgetting.statement}”</p>
      <p>This deletes the memory for good. To keep it on record but stop AIs receiving it, use Retire instead.</p>
    </Confirm>
  {/if}
{/if}
