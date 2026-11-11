<script lang="ts">
  import ConnectForm from '../lib/ConnectForm.svelte';
  import SpaceForm from '../lib/SpaceForm.svelte';
  import { app, go } from '../lib/store.svelte';

  const STEPS = ['Add your notes', 'Connect an AI app', 'Check it works'];
  let step = $state(0);

  const connections = $derived(app.state?.connections ?? []);
  const allVerified = $derived(connections.length > 0 && connections.every((c) => c.status === 'verified'));

  function finish() {
    app.setup = false;
    go('home');
  }
</script>

<h1>Set up Threadkeep</h1>

<ol class="steps">
  {#each STEPS as s, i}
    <li aria-current={i === step ? 'step' : undefined}>{i + 1}. {s}</li>
  {/each}
</ol>

<div class="card">
  {#if step === 0}
    <h2>Add your notes</h2>
    <p class="muted">A space is one folder of Markdown notes that you own. You can add more later.</p>
    <SpaceForm ondone={() => (step = 1)} />
  {:else if step === 1}
    <h2>Connect an AI app</h2>
    <p class="muted">Each AI app gets its own key and its own access. Nothing is shared until you choose it here.</p>
    <ConnectForm ondone={() => (step = 2)} />
  {:else}
    <h2>Check it works</h2>
    <ul class="list">
      {#each connections as c}
        <li>
          <div class="row spread">
            <strong>{c.name}</strong>
            {#if c.status === 'verified'}
              <span class="badge ok">Verified</span>
            {:else}
              <span class="badge warn">Not verified yet</span>
            {/if}
          </div>
          {#if c.status !== 'verified'}
            <p>{c.next ?? 'Restart the app.'}</p>
            <p class="small muted">Waiting for the app to call Threadkeep. This page updates by itself.</p>
          {/if}
        </li>
      {/each}
    </ul>
    <div class="row">
      <button class="primary" onclick={finish}>{allVerified ? 'Go to Home' : 'Continue without waiting'}</button>
    </div>
  {/if}
</div>

{#if step < 2}
  <div><button onclick={finish}>Skip setup</button></div>
{/if}
