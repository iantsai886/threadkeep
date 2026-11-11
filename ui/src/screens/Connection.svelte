<script lang="ts">
  import { call } from '../backend';
  import AskBox from '../lib/AskBox.svelte';
  import Confirm from '../lib/Confirm.svelte';
  import LevelPicker from '../lib/LevelPicker.svelte';
  import type { RevokeResult } from '../lib/types';
  import { app, flash, go, levelOf, link, message, refresh, withLevel, type Choice } from '../lib/store.svelte';

  let { id }: { id: string } = $props();

  const connection = $derived(app.state?.connections.find((c) => c.id === id));
  const spaces = $derived(app.state?.spaces ?? []);
  const knownApp = $derived(app.state?.apps.find((a) => a.id === connection?.client));

  let error = $state('');
  let busy = $state(false);
  let revoking = $state(false);

  async function setLevel(space: string, level: Choice) {
    if (!connection) return;
    busy = true;
    error = '';
    try {
      await call('setGrants', { id, grants: withLevel(connection.grants, space, level) });
    } catch (e) {
      error = message(e);
    } finally {
      await refresh();
      busy = false;
    }
  }

  async function revoke() {
    revoking = false;
    try {
      const r = await call<RevokeResult>('revokeConnection', { id });
      await refresh();
      flash(r?.warning ? `${r.note ?? ''} ${r.warning}`.trim() : r?.note);
      go('home');
    } catch (e) {
      error = message(e);
    }
  }
</script>

{#if !connection}
  <h1>Connection not found</h1>
  <p><a href={link('home')}>Back to Home</a></p>
{:else}
  <div class="row">
    <h1>{connection.name}</h1>
    {#if connection.status === 'verified'}
      <span class="badge ok">Verified</span>
    {:else}
      <span class="badge warn">Not verified yet</span>
    {/if}
  </div>

  {#if connection.status !== 'verified'}
    <p class="note">
      {connection.next ?? 'Restart the app.'} It becomes “Verified” once the app has really called Threadkeep. This page updates by itself.
    </p>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <section class="card">
    <h2>Access</h2>
    {#if spaces.length}
      {#each spaces as s (s.name)}
        <LevelPicker label={s.name} sensitive={s.sensitive} disabled={busy} value={levelOf(connection, s.name)} onchange={(v) => setLevel(s.name, v)} />
      {/each}
      <p class="small muted">
        Changes apply to the app's next request. No restart is needed. An app keeps access to at least one space; to remove all access, revoke the connection.
      </p>
    {:else}
      <p class="muted">There are no spaces yet.</p>
    {/if}
  </section>

  <section class="card">
    <h2>See as this AI</h2>
    <p class="muted">Ask a question and see exactly what {connection.name} would receive. Nothing is saved.</p>
    <AskBox as={id} />
  </section>

  <section class="card">
    <h2>Disconnect</h2>
    <p class="muted">Revoking the key stops this app at its next request.</p>
    <div><button class="danger" onclick={() => (revoking = true)}>Revoke this connection</button></div>
  </section>

  {#if revoking}
    <Confirm title={`Revoke ${connection.name}?`} confirmLabel="Revoke" danger onconfirm={revoke} oncancel={() => (revoking = false)}>
      <p>{connection.name} will get nothing from Threadkeep from its next request on. To use it again you would connect it again.</p>
      {#if knownApp}
        <p>Threadkeep will also take its entry out of this settings file, and keep a backup of it first:</p>
        <p class="path">{knownApp.file}</p>
      {/if}
    </Confirm>
  {/if}
{/if}
