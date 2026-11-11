<script lang="ts">
  import { call } from '../backend';
  import Confirm from './Confirm.svelte';
  import { KINDS, message, refresh } from './store.svelte';
  import type { Kind } from './types';

  // `path` is filled in when the owner arrives from a restored backup.
  let { path: preset = '', ondone, oncancel }: { path?: string; ondone: () => void; oncancel?: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let path = $state(preset);
  let name = $state('');
  let kind = $state<Kind>('company');
  let create = $state(false);
  // Follows the kind (contract rule 5) until the owner chooses for themselves.
  let sensitiveChoice = $state<boolean | null>(null);
  const sensitive = $derived(sensitiveChoice ?? kind !== 'company');

  let confirming = $state(false);
  let busy = $state(false);
  let error = $state('');

  function submit(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    if (create) confirming = true;
    else save();
  }

  async function save() {
    confirming = false;
    busy = true;
    try {
      await call('addSpace', { path: path.trim(), name: name.trim() || undefined, kind, sensitive, create });
      await refresh();
      ondone();
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<form class="stack" onsubmit={submit}>
  <label class="field">
    Folder of Markdown notes
    <input type="text" class="path" bind:value={path} required placeholder="/Users/you/Notes/Acme Tools" autocomplete="off" spellcheck="false" />
    <span class="hint small muted">Threadkeep writes only inside a “Threadkeep” folder there. It never edits your own notes.</span>
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={create} />
    This folder does not exist yet. Create it.
  </label>
  <label class="field">
    Name <span class="hint small muted">Optional. Left empty, the folder's name is used.</span>
    <input type="text" bind:value={name} autocomplete="off" />
  </label>
  <label class="field">
    Kind
    <select bind:value={kind}>
      {#each KINDS as k}<option value={k.value}>{k.label}</option>{/each}
    </select>
  </label>
  <label class="check">
    <input type="checkbox" checked={sensitive} onchange={(e) => (sensitiveChoice = e.currentTarget.checked)} />
    <span>Sensitive: everything an AI saves in this space waits for your approval.</span>
  </label>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="row">
    <button class="primary" disabled={busy || !path.trim()}>Add space</button>
    {#if oncancel}<button type="button" onclick={oncancel}>Cancel</button>{/if}
  </div>
</form>

{#if confirming}
  <Confirm title="Create this folder?" confirmLabel="Create folder" onconfirm={save} oncancel={() => (confirming = false)}>
    <p>Threadkeep will make this folder:</p>
    <p class="path">{path.trim()}</p>
  </Confirm>
{/if}
