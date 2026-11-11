<script lang="ts">
  import { call } from '../backend';
  import { app, link, message, when } from '../lib/store.svelte';
  import type { ActivityEntry, Overhead } from '../lib/types';

  const FIRST = 30;
  const MOST = 200; // the backend returns no more than this

  let entries = $state<ActivityEntry[] | null>(null);
  let overhead = $state<Overhead | null>(null);
  let n = $state(FIRST);
  let error = $state('');
  let busy = $state(false);

  async function load() {
    busy = true;
    try {
      entries = (await call<ActivityEntry[]>('activity', { n })) ?? [];
      error = '';
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
  load();
  call<Overhead>('overhead')
    .then((o) => (overhead = o))
    .catch(() => {});

  function more() {
    n = MOST;
    load();
  }

  // What each AI-facing tool does, in the product's words.
  const DID: Record<string, string> = {
    status: 'Checked what it may do',
    ask: 'Asked a question',
    open: 'Opened a cited section',
    save: 'Saved a memory',
    resume: 'Looked up a thread',
    checkpoint: 'Saved a thread checkpoint',
  };
  const FOUND: Record<string, string> = { found: 'Found', partial: 'Partly covered', none: 'Nothing found' };

  const connected = (id: string | null) => Boolean(id && app.state?.connections.some((c) => c.id === id));
</script>

<div class="row spread">
  <h1>Activity</h1>
  <button onclick={load} disabled={busy}>Refresh</button>
</div>
<p class="muted">
  Each time an AI app calls Threadkeep, one line is recorded here: which app, what kind of call, and whether it worked. The question, the answer and
  the text of your notes are never recorded.
</p>

{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if entries === null}
  {#if !error}<p class="muted">Loading…</p>{/if}
{:else if !entries.length}
  <div class="card"><p class="muted">No AI app has called Threadkeep yet.</p></div>
{:else}
  <div class="card">
    <ul class="list">
      {#each entries as e}
        <li>
          <div class="row spread">
            <span>
              {#if connected(e.connection)}
                <a class="title" href={link('connection', e.connection ?? '')}>{e.name}</a>
              {:else}
                <strong>{e.name ?? 'An app without a valid key'}</strong>
              {/if}
              · {DID[e.tool] ?? e.tool}
            </span>
            {#if !e.ok}
              <span class="badge danger">Refused or failed</span>
            {:else if e.queued}
              <span class="badge warn">Sent for your approval</span>
            {:else if e.status && FOUND[e.status]}
              <span class="badge" class:ok={e.status === 'found'}>{FOUND[e.status]}</span>
            {:else}
              <span class="badge ok">OK</span>
            {/if}
          </div>
          <div class="row small muted">
            <span>{when(e.at)}</span>
            {#if typeof e.ms === 'number'}<span>{e.ms} ms</span>{/if}
            {#if typeof e.est_tokens === 'number'}<span>about {e.est_tokens} tokens returned (estimate)</span>{/if}
            {#if e.connection && !connected(e.connection)}<span>This connection has since been revoked.</span>{/if}
          </div>
        </li>
      {/each}
    </ul>
    {#if n < MOST && entries.length >= n}
      <div><button onclick={more} disabled={busy}>Show up to {MOST}</button></div>
    {:else}
      <p class="small muted">Showing the latest {entries.length}.</p>
    {/if}
  </div>
{/if}

{#if overhead}
  <section class="card">
    <h2>What Threadkeep adds to each AI conversation</h2>
    <p>
      A connected AI app carries Threadkeep's {overhead.tools} tool descriptions and its instructions in every conversation: about
      {overhead.total_est_tokens} tokens.
    </p>
    <p class="small muted">An estimate ({overhead.method}).</p>
  </section>
{/if}
