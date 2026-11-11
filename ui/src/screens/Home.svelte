<script lang="ts">
  import { call } from '../backend';
  import { app, count, link, when } from '../lib/store.svelte';
  import type { Proposal } from '../lib/types';

  const data = $derived(app.state!);
  let waiting = $state<Proposal[]>([]);

  $effect(() => {
    if (!data.pending) {
      waiting = [];
      return;
    }
    call<Proposal[]>('pending')
      .then((list) => (waiting = list ?? []))
      .catch(() => (waiting = []));
  });
</script>

<h1>Home</h1>

<div class="grid3">
  <section class="card" aria-labelledby="h-spaces">
    <h2 id="h-spaces">Spaces</h2>
    {#if data.spaces.length}
      <ul class="list">
        {#each data.spaces as s}
          <li>
            <a class="title" href={link('space', s.name)}>{s.name}</a>
            <div class="row small muted">
              <span>{s.kind}</span>
              {#if s.exists === false}
                <span class="badge danger">Folder missing</span>
              {:else if typeof s.notes === 'number'}
                <span>{count(s.notes, 'note')}</span>
              {/if}
              {#if s.sensitive}<span class="badge warn">Sensitive</span>{/if}
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No spaces yet. A space is one folder of your Markdown notes.</p>
    {/if}
    <div><a class="button" href={link('add-space')}>Add a space</a></div>
  </section>

  <section class="card" aria-labelledby="h-apps">
    <h2 id="h-apps">Connected AI apps</h2>
    {#if data.connections.length}
      <ul class="list">
        {#each data.connections as c}
          <li>
            <div class="row spread">
              <a class="title" href={link('connection', c.id)}>{c.name}</a>
              {#if c.status === 'verified'}
                <span class="badge ok">Verified</span>
              {:else}
                <span class="badge warn">Not verified yet</span>
              {/if}
            </div>
            {#if c.status !== 'verified'}
              <p class="small">{c.next ?? 'Restart the app. It is verified once it has really called Threadkeep.'}</p>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No AI app is connected yet.</p>
    {/if}
    <div><a class="button" href={link('add-connection')}>Connect an AI app</a></div>
  </section>

  <section class="card" aria-labelledby="h-waiting">
    <h2 id="h-waiting">Waiting for approval</h2>
    {#if data.pending}
      <p class="big">{data.pending}</p>
      <ul class="list">
        {#each waiting.slice(0, 3) as p}
          <li>
            <span>{p.summary}</span>
            <span class="small muted">{p.connection.name} → {p.space} · {when(p.created)}</span>
          </li>
        {/each}
      </ul>
      <div><a class="button primary" href={link('approvals')}>Review</a></div>
    {:else}
      <p class="muted">Nothing is waiting for you.</p>
    {/if}
  </section>
</div>
