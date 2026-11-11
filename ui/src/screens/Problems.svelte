<script lang="ts">
  import { call } from '../backend';
  import { app, message } from '../lib/store.svelte';
  import type { Check } from '../lib/types';

  let error = $state('');
  let busy = $state(false);

  const failed = $derived(app.checks.filter((c) => !c.ok));
  const passed = $derived(app.checks.filter((c) => c.ok));

  async function check() {
    busy = true;
    try {
      app.checks = (await call<{ checks: Check[] }>('doctor')).checks ?? [];
      error = '';
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
  check();
</script>

<div class="row spread">
  <h1>Problems</h1>
  <button onclick={check} disabled={busy}>Check again</button>
</div>

{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if failed.length}
  {#each failed as c}
    <article class="card">
      <div class="row"><span class="badge danger">Problem</span><strong>{c.what}</strong></div>
      {#if c.fix}<p><span class="muted">To fix it:</span> {c.fix}</p>{/if}
    </article>
  {/each}
{:else if app.checks.length}
  <div class="card"><p>No problems found.</p></div>
{/if}

{#if passed.length}
  <details class="card">
    <summary>{passed.length} checks passed</summary>
    <ul class="list" style="margin-top:12px">
      {#each passed as c}<li><div class="row"><span class="badge ok">OK</span><span>{c.what}</span></div></li>{/each}
    </ul>
  </details>
{/if}
