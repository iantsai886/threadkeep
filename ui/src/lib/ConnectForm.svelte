<script lang="ts">
  import { call } from '../backend';
  import Confirm from './Confirm.svelte';
  import LevelPicker from './LevelPicker.svelte';
  import { app, message, refresh, withLevel } from './store.svelte';
  import type { AddConnectionResult, Grant } from './types';

  let { ondone, oncancel }: { ondone: () => void; oncancel?: () => void } = $props();

  const CUSTOM = 'custom';
  const installed = $derived((app.state?.apps ?? []).filter((a) => a.present));
  const spaces = $derived(app.state?.spaces ?? []);

  let client = $state('');
  let name = $state('');
  let grants = $state<Grant[] | null>(null);
  let confirming = $state(false);
  let busy = $state(false);
  let error = $state('');
  let result = $state<AddConnectionResult | null>(null);
  let copied = $state(false);

  const chosen = $derived(installed.find((a) => a.id === client));
  // One app holds one key: connecting it again replaces the earlier connection.
  const existing = $derived(app.state?.connections.find((c) => c.client === client && client !== CUSTOM));

  // Start every space on Read; the owner raises or removes it.
  $effect(() => {
    if (grants === null && spaces.length) grants = spaces.map((s) => ({ space: s.name, level: 'read' }));
  });

  async function save() {
    confirming = false;
    busy = true;
    error = '';
    try {
      result = await call<AddConnectionResult>('addConnection', { client, name: name.trim() || undefined, grants: grants ?? [] });
      await refresh();
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

{#if result}
  <div class="stack">
    {#if result.snippet || result.token}
      <p class="note">This key is shown only once. Copy it now.</p>
      <pre class="text path">{result.snippet ?? result.token}</pre>
      <div class="row">
        <button onclick={() => copy(result?.snippet ?? result?.token ?? '')}>{copied ? 'Copied' : 'Copy'}</button>
      </div>
    {:else if result.file}
      <p>Changed: <span class="path">{result.file}</span></p>
      {#if result.backup}<p>Backup of the earlier file: <span class="path">{result.backup}</span></p>{/if}
    {/if}
    <div class="note plain stack" style="gap:6px">
      <strong>Not verified yet</strong>
      {#if result.next}<p>{result.next}</p>{/if}
      {#if result.verify}<p>{result.verify}</p>{/if}
    </div>
    <div class="row"><button class="primary" onclick={ondone}>Done</button></div>
  </div>
{:else}
  <form
    class="stack"
    onsubmit={(e) => {
      e.preventDefault();
      confirming = true;
    }}
  >
    <label class="field">
      AI app
      <select bind:value={client} required>
        <option value="" disabled>Choose an app</option>
        {#each installed as a}<option value={a.id}>{a.title}</option>{/each}
        <option value={CUSTOM}>Another AI app (set up by hand)</option>
      </select>
      <span class="hint small muted">Only apps found on this computer are listed.</span>
    </label>
    <label class="field">
      Name <span class="hint small muted">Optional. How this connection is shown to you.</span>
      <input type="text" bind:value={name} autocomplete="off" />
    </label>

    {#if spaces.length}
      <div class="stack">
        <h3>What may it do in each space?</h3>
        {#each spaces as s}
          <LevelPicker
            label={s.name}
            sensitive={s.sensitive}
            value={grants?.find((g) => g.space === s.name)?.level ?? 'none'}
            onchange={(v) => (grants = withLevel(grants ?? [], s.name, v))}
          />
        {/each}
      </div>
    {:else}
      <p class="note">Add a space first. An AI app needs access to at least one space.</p>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="row">
      <button class="primary" disabled={busy || !client || !grants?.length}>Connect</button>
      {#if oncancel}<button type="button" onclick={oncancel}>Cancel</button>{/if}
    </div>
  </form>
{/if}

{#if confirming}
  <Confirm
    title={chosen ? `Change ${chosen.title}'s settings?` : 'Make a key for this app?'}
    confirmLabel="Connect"
    onconfirm={save}
    oncancel={() => (confirming = false)}
  >
    {#if chosen}
      <p>Threadkeep will change this settings file, and keep a backup of it first:</p>
      <p class="path">{chosen.file}</p>
      {#if existing}<p>{existing.name} is already connected. Its earlier key stops working and this connection replaces it.</p>{/if}
    {:else}
      <p>Threadkeep will make a one-time key and show you the entry to paste into your AI app's settings. No file is changed for you.</p>
    {/if}
  </Confirm>
{/if}
