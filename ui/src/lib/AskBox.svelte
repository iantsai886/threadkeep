<script lang="ts">
  import { call } from '../backend';
  import { app, message } from './store.svelte';
  import type { AskResult, Opened } from './types';

  // With `as`, the answer is exactly what that connection would receive.
  let { as }: { as?: string } = $props();

  const STATUS = {
    found: { label: 'Found', tone: 'ok' },
    partial: { label: 'Partly covered', tone: 'warn' },
    none: { label: 'Nothing found', tone: '' },
  } as const;

  let question = $state('');
  let space = $state('');
  let history = $state(false);
  let busy = $state(false);
  let error = $state('');
  let result = $state<AskResult | null>(null);
  let opened = $state<Record<string, string>>({});

  // A different AI sees different things: drop the old answer.
  $effect(() => {
    as;
    result = null;
    opened = {};
  });

  async function ask(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    opened = {};
    try {
      result = await call<AskResult>('ask', { question, space: space || undefined, history: history || undefined, as });
    } catch (e) {
      result = null;
      error = message(e);
    } finally {
      busy = false;
    }
  }

  async function open(ref: string) {
    try {
      const o = await call<Opened>('open', { ref, as });
      opened[ref] = o.text + (o.truncated ? '\n\n(Cut at 8,000 characters.)' : '');
    } catch (e) {
      opened[ref] = message(e);
    }
  }
</script>

<form class="stack" onsubmit={ask}>
  <label class="field">
    Question
    <input type="text" bind:value={question} required placeholder="When does Riverbend renew?" autocomplete="off" />
  </label>
  <div class="row">
    <label class="row small">
      Space
      <select bind:value={space} style="width:auto">
        <option value="">All permitted spaces</option>
        {#each app.state?.spaces ?? [] as s}<option value={s.name}>{s.name}</option>{/each}
      </select>
    </label>
    <label class="check small"><input type="checkbox" bind:checked={history} /> Include outdated memories</label>
    <button class="primary" disabled={busy || !question.trim()}>Ask</button>
  </div>
</form>

{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if result}
  <div class="stack" aria-live="polite">
    <div class="row">
      <span class="badge {STATUS[result.status]?.tone}">{STATUS[result.status]?.label ?? result.status}</span>
      {#if result.missing?.length}<span class="small muted">Not covered: {result.missing.join(', ')}</span>{/if}
      {#if result.more_available}<span class="small muted">More is available.</span>{/if}
    </div>
    {#if result.sources.length}
      <ul class="list">
        {#each result.sources as s}
          <li>
            <div class="row">
              <strong>{s.title ?? (s.kind === 'memory' ? 'Memory' : 'Note')}{s.heading ? ` · ${s.heading}` : ''}</strong>
              <span class="badge">{s.space}</span>
              {#if s.outdated}<span class="badge warn">Outdated</span>{/if}
              {#if s.needs_review}<span class="badge warn">Needs review</span>{/if}
              {#if s.confidence}<span class="small muted">{s.confidence}</span>{/if}
              {#if s.date}<span class="small muted">{s.date}</span>{/if}
            </div>
            <p>{s.excerpt}</p>
            <div class="row">
              <span class="path muted">{s.ref}</span>
              {#if !(s.ref in opened)}<button class="small" onclick={() => open(s.ref)}>Open in full</button>{/if}
            </div>
            {#if s.ref in opened}<pre class="text">{opened[s.ref]}</pre>{/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">{as ? 'This AI would receive nothing for that question.' : 'Nothing in your spaces matched that question.'}</p>
    {/if}
  </div>
{/if}
