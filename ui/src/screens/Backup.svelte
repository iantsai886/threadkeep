<script lang="ts">
  import { call } from '../backend';
  import Confirm from '../lib/Confirm.svelte';
  import { app, count, link, message, size, when } from '../lib/store.svelte';
  import type { BackupCheck, BackupEntry, BackupList, BackupResult, RestoreResult } from '../lib/types';

  // `space` is set when the owner comes from a space's own screen.
  let { space: preset = '' }: { space?: string } = $props();

  const spaces = $derived(app.state?.spaces ?? []);

  // --- back up a space ---
  // svelte-ignore state_referenced_locally
  let space = $state(preset);
  let dest = $state('');
  let confirmingBackup = $state(false);
  let backingUp = $state(false);
  let backupError = $state('');
  let made = $state<BackupResult | null>(null);

  const chosen = $derived(spaces.find((s) => s.name === space));

  async function backUp() {
    confirmingBackup = false;
    backingUp = true;
    backupError = '';
    made = null;
    try {
      made = await call<BackupResult>('backup', { space, dest: dest.trim() });
      // Ready to be checked or restored below.
      dir = made.backup;
      checked = null;
      restored = null;
      folder = dest.trim();
      list();
    } catch (e) {
      backupError = message(e);
    } finally {
      backingUp = false;
    }
  }

  // --- check or restore a backup ---
  // Backups found in one folder. Listed does not mean intact: a backup is still checked before restore.
  let folder = $state('');
  let found = $state<BackupList | null>(null);
  let listing = $state(false);
  let listError = $state('');

  async function list(e?: SubmitEvent) {
    e?.preventDefault();
    listing = true;
    listError = '';
    try {
      found = await call<BackupList>('listBackups', { dir: folder.trim() });
      if (found.dir && !folder.trim()) folder = found.dir;
      // Offer the folder of the last backup as the place for the next one.
      if (found.remembered && found.dir && !dest) dest = found.dir;
    } catch (e) {
      found = null;
      listError = message(e);
    } finally {
      listing = false;
    }
  }
  list();

  function use(b: BackupEntry) {
    dir = b.dir;
    check();
  }

  let dir = $state('');
  let target = $state('');
  let checking = $state(false);
  let checked = $state<BackupCheck | null>(null);
  let checkedDir = $state('');
  let confirmingRestore = $state(false);
  let restoring = $state(false);
  let restoreError = $state('');
  let restored = $state<RestoreResult | null>(null);

  // A check only counts for the folder it was run on.
  const current = $derived(checked && checkedDir === dir.trim() ? checked : null);

  async function check(e?: SubmitEvent) {
    e?.preventDefault();
    checking = true;
    restoreError = '';
    restored = null;
    try {
      checkedDir = dir.trim();
      checked = await call<BackupCheck>('verifyBackup', { dir: checkedDir });
    } catch (e) {
      checked = null;
      restoreError = message(e);
    } finally {
      checking = false;
    }
  }

  async function restore() {
    confirmingRestore = false;
    restoring = true;
    restoreError = '';
    try {
      restored = await call<RestoreResult>('restore', { dir: dir.trim(), target: target.trim() });
    } catch (e) {
      restoreError = message(e);
    } finally {
      restoring = false;
    }
  }
</script>

<h1>Backup</h1>
<p class="muted">A backup is a plain copy of one space's folder, with a list of its files so the copy can be checked later.</p>

<section class="card">
  <h2>Back up a space</h2>
  {#if spaces.length}
    <form
      class="stack"
      onsubmit={(e) => {
        e.preventDefault();
        confirmingBackup = true;
      }}
    >
      <label class="field" style="max-width:320px">
        Space
        <select bind:value={space} required>
          <option value="" disabled>Choose a space</option>
          {#each spaces as s}<option value={s.name}>{s.name}</option>{/each}
        </select>
      </label>
      <label class="field">
        Backup folder
        <input type="text" class="path" bind:value={dest} required placeholder="/Volumes/Backup Drive/Threadkeep" autocomplete="off" spellcheck="false" />
        <span class="hint small muted">
          Each backup gets its own new folder inside this one. It must be outside the space. Another drive is safest.
        </span>
      </label>
      <div><button class="primary" disabled={backingUp || !space || !dest.trim()}>{backingUp ? 'Backing up…' : 'Back up'}</button></div>
    </form>
  {:else}
    <p class="muted">There are no spaces yet. <a href={link('add-space')}>Add a space</a></p>
  {/if}

  {#if backupError}<p class="error" role="alert">{backupError}</p>{/if}
  {#if made}
    <div class="note plain" role="status">
      <p><strong>Backup made and checked.</strong> {count(made.files, 'file')}, {size(made.bytes)}. Every copied file matches the original.</p>
      <p class="path">{made.backup}</p>
    </div>
  {/if}
</section>

<section class="card">
  <h2>Check or restore a backup</h2>
  <form class="stack" onsubmit={list}>
    <label class="field">
      Folder that holds your backups
      <input type="text" class="path" bind:value={folder} required placeholder="/Volumes/Backup Drive/Threadkeep" autocomplete="off" spellcheck="false" />
    </label>
    <div><button disabled={listing || !folder.trim()}>{listing ? 'Looking…' : 'List backups'}</button></div>
  </form>
  {#if listError}<p class="error" role="alert">{listError}</p>{/if}
  {#if found?.dir}
    {#if found.backups.length}
      <p class="small muted">{found.remembered ? 'Where your last backup went. ' : ''}Newest first. A backup is checked before it can be restored.</p>
      <ul class="list">
        {#each found.backups as b (b.dir)}
          <li>
            <div class="row spread">
              <span><strong>{b.space}</strong> · {when(b.created)} · {count(b.files, 'file')}, {size(b.bytes)}</span>
              <button class="small" disabled={checking} onclick={() => use(b)}>Check this one</button>
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No Threadkeep backups in this folder.</p>
    {/if}
  {/if}

  <form class="stack" style="border-top:1px solid var(--line);padding-top:14px" onsubmit={check}>
    <label class="field">
      Backup to use
      <input type="text" class="path" bind:value={dir} required placeholder="/Volumes/Backup Drive/Threadkeep/Acme_Tools-2026-11-10T09-30-00-000Z" autocomplete="off" spellcheck="false" />
      <span class="hint small muted">The folder of one backup: the one that contains “threadkeep-backup.json”.</span>
    </label>
    <div><button disabled={checking || !dir.trim()}>{checking ? 'Checking…' : 'Check this backup'}</button></div>
  </form>

  {#if current}
    {#if current.ok}
      <div class="note plain" role="status">
        <p>
          <span class="badge ok">Intact</span>
          Backup of <strong>{current.space}</strong>, made {when(current.created)}. {count(current.files ?? 0, 'file')}, none changed or missing.
        </p>
      </div>
    {:else}
      <div class="error" role="alert">
        <p>
          This backup has problems and cannot be restored{current.space ? ` (backup of ${current.space}, made ${when(current.created)})` : ''}:
        </p>
        <ul style="margin:6px 0 0;padding-left:20px">
          {#each current.problems.slice(0, 20) as p}<li>{p}</li>{/each}
        </ul>
        {#if current.problems.length > 20}<p>and {current.problems.length - 20} more.</p>{/if}
      </div>
    {/if}
  {/if}

  {#if current?.ok && !restored}
    <form
      class="stack"
      style="border-top:1px solid var(--line);padding-top:14px"
      onsubmit={(e) => {
        e.preventDefault();
        confirmingRestore = true;
      }}
    >
      <h3>Restore it</h3>
      <label class="field">
        Restore into
        <input type="text" class="path" bind:value={target} required placeholder="/Users/you/Notes/Acme Tools restored" autocomplete="off" spellcheck="false" />
        <span class="hint small muted">A new folder, or an empty one. Threadkeep never restores over existing files.</span>
      </label>
      <div><button class="primary" disabled={restoring || !target.trim()}>{restoring ? 'Restoring…' : 'Restore'}</button></div>
    </form>
  {/if}

  {#if restoreError}<p class="error" role="alert">{restoreError}</p>{/if}
  {#if restored}
    <div class="note plain" role="status">
      <p><strong>Restored.</strong> {count(restored.files, 'file')} from the backup of {restored.space}, each checked after copying.</p>
      <p class="path">{restored.restored_to}</p>
      <p>{restored.next}</p>
      <p><a class="button" href={link('add-space', restored.restored_to)}>Add it as a space</a></p>
    </div>
  {/if}
</section>

{#if confirmingBackup && chosen}
  <Confirm title={`Back up ${chosen.name}?`} confirmLabel="Back up" onconfirm={backUp} oncancel={() => (confirmingBackup = false)}>
    <p>Threadkeep will copy every file in this space:</p>
    <p class="path">{chosen.path}</p>
    <p>into a new folder inside:</p>
    <p class="path">{dest.trim()}</p>
    <p>Nothing in the space is changed. {chosen.sensitive ? 'This space is sensitive, and the copy is not encrypted, so keep it somewhere only you can reach.' : 'The copy is not encrypted.'}</p>
  </Confirm>
{/if}

{#if confirmingRestore && current}
  <Confirm title="Restore this backup?" confirmLabel="Restore" onconfirm={restore} oncancel={() => (confirmingRestore = false)}>
    <p>Threadkeep will copy the {count(current.files ?? 0, 'file')} of the backup of {current.space} into:</p>
    <p class="path">{target.trim()}</p>
    <p>The folder is made if it does not exist. If it already has files in it, nothing is changed and you are told so. No existing space is touched.</p>
  </Confirm>
{/if}
