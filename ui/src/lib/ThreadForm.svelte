<script lang="ts">
  import { call } from '../backend';
  import { message } from './store.svelte';
  import type { Thread } from './types';

  // With `thread`, adds a checkpoint to it; without, starts a new thread.
  let { space, thread, ondone, oncancel }: { space: string; thread?: Thread; ondone: (saved: Thread) => void; oncancel: () => void } = $props();

  const lines = (list?: string[]) => (list ?? []).join('\n');
  const items = (text: string) =>
    text
      .split('\n')
      .map((x) => x.replace(/^\s*-\s*/, '').trim())
      .filter(Boolean);

  // A checkpoint replaces the lists, so they start from what the thread has now.
  // svelte-ignore state_referenced_locally
  const from = thread;
  let title = $state('');
  let goal = $state(from?.goal ?? '');
  let standing = $state(from?.state ?? '');
  let nextAction = $state(from?.next_action ?? '');
  let decisions = $state(lines(from?.decisions));
  let openQuestions = $state(lines(from?.open_questions));
  let sources = $state(lines(from?.sources));
  let done = $state(false);

  let busy = $state(false);
  let error = $state('');

  async function save(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const saved = await call<Thread>('checkpoint', {
        space,
        thread: from?.thread,
        title: from ? undefined : title.trim(),
        goal: goal.trim(),
        state: standing.trim(),
        next_action: nextAction.trim(),
        decisions: items(decisions),
        open_questions: items(openQuestions),
        sources: items(sources),
        status: done ? 'done' : 'open',
      });
      ondone(saved);
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<form class="stack" onsubmit={save}>
  <h3>{from ? `Add a checkpoint to “${from.title}”` : 'Start a thread'}</h3>
  {#if from}
    <p class="note plain small">
      A checkpoint is added as a new file; earlier checkpoints are kept. The thread then shows what you save here, so edit the lines below rather than
      clearing them.
    </p>
  {:else}
    <label class="field">
      Title
      <input type="text" bind:value={title} required maxlength="120" autocomplete="off" />
    </label>
  {/if}
  <label class="field">
    Goal <span class="hint small muted">What this work is for.</span>
    <textarea bind:value={goal} maxlength="2000"></textarea>
  </label>
  <label class="field">
    State <span class="hint small muted">Where the work stands now.</span>
    <textarea bind:value={standing} required maxlength="2000"></textarea>
  </label>
  <label class="field">
    Next action <span class="hint small muted">What whoever picks this up should do first.</span>
    <textarea bind:value={nextAction} required maxlength="2000"></textarea>
  </label>
  <label class="field">
    Decisions <span class="hint small muted">One per line. Up to 12.</span>
    <textarea bind:value={decisions}></textarea>
  </label>
  <label class="field">
    Open questions <span class="hint small muted">One per line. Up to 12.</span>
    <textarea bind:value={openQuestions}></textarea>
  </label>
  <label class="field">
    Sources <span class="hint small muted">One per line: notes, files or links the work relies on.</span>
    <textarea bind:value={sources}></textarea>
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={done} />
    This work is finished. Mark the thread done.
  </label>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" disabled={busy || !standing.trim() || !nextAction.trim() || (!from && !title.trim())}>
      {from ? 'Save checkpoint' : 'Start thread'}
    </button>
    <button type="button" onclick={oncancel}>Cancel</button>
  </div>
</form>
