<script lang="ts">
  import { onMount } from 'svelte';
  import ConnectForm from './lib/ConnectForm.svelte';
  import SpaceForm from './lib/SpaceForm.svelte';
  import { app, go, link, refresh, refreshChecks, route, startRouter } from './lib/store.svelte';
  import Activity from './screens/Activity.svelte';
  import Approvals from './screens/Approvals.svelte';
  import Backup from './screens/Backup.svelte';
  import ConnectionScreen from './screens/Connection.svelte';
  import Home from './screens/Home.svelte';
  import Problems from './screens/Problems.svelte';
  import SeeAs from './screens/SeeAs.svelte';
  import Setup from './screens/Setup.svelte';
  import SpaceScreen from './screens/Space.svelte';

  const problems = $derived(app.checks.filter((c) => !c.ok).length);

  onMount(() => {
    const stopRouter = startRouter();
    let timer = 0;
    let stopped = false;

    // Poll faster while an AI app is set up but has not called Threadkeep yet.
    async function tick() {
      await refresh();
      if (stopped) return;
      const waiting = app.state?.connections.some((c) => c.status !== 'verified');
      timer = window.setTimeout(tick, waiting ? 2000 : 10000);
    }
    tick();
    refreshChecks();

    const onFocus = () => {
      refresh();
      refreshChecks();
    };
    window.addEventListener('focus', onFocus);
    window.addEventListener('threadkeep-reopened', onFocus);
    return () => {
      stopped = true;
      clearTimeout(timer);
      stopRouter();
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('threadkeep-reopened', onFocus);
    };
  });
</script>

<div class="page">
  <header class="topbar">
    <a class="brand" href={link('home')}>Threadkeep</a>
    {#if app.state && !app.setup}
      <nav aria-label="Main">
        <a href={link('home')} aria-current={route.name === 'home' ? 'page' : undefined}>Home</a>
        <a href={link('approvals')} aria-current={route.name === 'approvals' ? 'page' : undefined}>
          Waiting for approval{app.state.pending ? ` (${app.state.pending})` : ''}
        </a>
        <a href={link('see-as')} aria-current={route.name === 'see-as' ? 'page' : undefined}>See as an AI</a>
        <a href={link('activity')} aria-current={route.name === 'activity' ? 'page' : undefined}>Activity</a>
        <a href={link('backup')} aria-current={route.name === 'backup' ? 'page' : undefined}>Backup</a>
        <a href={link('problems')} aria-current={route.name === 'problems' ? 'page' : undefined}>
          Problems{problems ? ` (${problems})` : ''}
        </a>
      </nav>
    {/if}
  </header>

  <main class="stack">
    {#if app.error}<p class="error" role="alert">{app.error}</p>{/if}
    {#if app.notice}<p class="note plain" role="status">{app.notice}</p>{/if}

    {#if !app.state}
      {#if !app.error}<p class="muted">Loading…</p>{/if}
    {:else if app.setup}
      <Setup />
    {:else if route.name === 'space'}
      {#key route.arg}<SpaceScreen name={route.arg} />{/key}
    {:else if route.name === 'connection'}
      {#key route.arg}<ConnectionScreen id={route.arg} />{/key}
    {:else if route.name === 'add-space'}
      <h1>Add a space</h1>
      {#key route.arg}<div class="card"><SpaceForm path={route.arg} ondone={() => go('home')} oncancel={() => go('home')} /></div>{/key}
    {:else if route.name === 'add-connection'}
      <h1>Connect an AI app</h1>
      <div class="card"><ConnectForm ondone={() => go('home')} oncancel={() => go('home')} /></div>
    {:else if route.name === 'approvals'}
      <Approvals />
    {:else if route.name === 'see-as'}
      <SeeAs />
    {:else if route.name === 'activity'}
      <Activity />
    {:else if route.name === 'backup'}
      {#key route.arg}<Backup space={route.arg} />{/key}
    {:else if route.name === 'problems'}
      <Problems />
    {:else}
      <Home />
    {/if}
  </main>
</div>
