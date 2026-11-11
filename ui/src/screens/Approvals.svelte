<script lang="ts">
  import { call } from '../backend';
  import { app, message, refresh, when } from '../lib/store.svelte';
  import type { Proposal } from '../lib/types';

  let list = $state<Proposal[] | null>(null);
  let error = $state('');
  let busy = $state('');

  async function load() {
    try {
      list = (await call<Proposal[]>('pending')) ?? [];
      error = '';
    } catch (e) {
      error = message(e);
    }
  }

  // Reload when the count changes, for example when an AI proposes something while this is open.
  $effect(() => {
    app.state?.pending;
    load();
  });

  async function decide(command: 'approve' | 'reject', id: string) {
    busy = id;
    try {
      await call(command, { id });
      await refresh();
      await load();
    } catch (e) {
      error = message(e);
    } finally {
      busy = '';
    }
  }

  const SECTIONS: [string, string][] = [
    ['goal', 'Goal'],
    ['state', 'State'],
    ['decisions', 'Decisions'],
    ['sources', 'Sources'],
    ['open_questions', 'Open questions'],
    ['next_action', 'Next action'],
  ];

  // The full text of what would be stored.
  function fullText(p: Proposal): string {
    const d = p.payload ?? {};
    if (typeof d.statement === 'string') return d.statement;
    const parts = SECTIONS.filter(([key]) => d[key] !== undefined && d[key] !== '' && !(Array.isArray(d[key]) && !(d[key] as unknown[]).length)).map(
      ([key, label]) => `${label}\n${Array.isArray(d[key]) ? (d[key] as unknown[]).map((x) => `- ${x}`).join('\n') : d[key]}`,
    );
    return parts.length ? parts.join('\n\n') : JSON.stringify(d, null, 2);
  }
</script>

<h1>Waiting for approval</h1>
<p class="muted">Only you can approve or reject. Nothing below is stored in a space until you approve it.</p>

{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if list === null}
  {#if !error}<p class="muted">Loading…</p>{/if}
{:else if !list.length}
  <div class="card"><p class="muted">Nothing is waiting for you.</p></div>
{:else}
  {#each list as p (p.id)}
    <article class="card">
      <div class="row">
        <span class="badge">{p.type === 'memory' ? 'Memory' : 'Thread checkpoint'}</span>
        <span><strong>{p.connection.name}</strong> proposed this for <strong>{p.space}</strong></span>
        <span class="small muted">{when(p.created)}</span>
      </div>
      {#if p.type === 'checkpoint'}<h2>{p.payload?.title || p.payload?.thread || p.summary}</h2>{/if}
      <pre class="text">{fullText(p)}</pre>
      {#if p.type === 'memory' && p.payload?.source}
        <p class="small muted">Source: {p.payload.source}{p.payload.replaces ? ` · Replaces ${p.payload.replaces}` : ''}</p>
      {/if}
      <div class="row">
        <button class="primary" disabled={busy === p.id} onclick={() => decide('approve', p.id)}>Approve</button>
        <button disabled={busy === p.id} onclick={() => decide('reject', p.id)}>Reject</button>
      </div>
    </article>
  {/each}
{/if}
