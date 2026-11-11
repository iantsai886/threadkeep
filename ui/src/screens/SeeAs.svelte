<script lang="ts">
  import AskBox from '../lib/AskBox.svelte';
  import { app, link } from '../lib/store.svelte';

  const connections = $derived(app.state?.connections ?? []);
  let as = $state('');
</script>

<h1>See as an AI</h1>
<p class="muted">Ask a question the way one of your AI apps would, and see exactly what it would receive. Nothing is saved.</p>

{#if connections.length}
  <div class="card">
    <label class="field">
      See as
      <select bind:value={as}>
        <option value="">Yourself (everything you own)</option>
        {#each connections as c}<option value={c.id}>{c.name}</option>{/each}
      </select>
    </label>
    <AskBox as={as || undefined} />
  </div>
{:else}
  <div class="card">
    <p class="muted">No AI app is connected yet.</p>
    <div><a class="button" href={link('add-connection')}>Connect an AI app</a></div>
  </div>
{/if}
