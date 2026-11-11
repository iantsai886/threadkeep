<script lang="ts">
  import { LEVELS, type Choice } from './store.svelte';

  let {
    value,
    label,
    sensitive = false,
    disabled = false,
    onchange,
  }: { value: Choice; label: string; sensitive?: boolean; disabled?: boolean; onchange: (v: Choice) => void } = $props();

  const group = `level-${Math.random().toString(36).slice(2)}`;
</script>

<fieldset class="levels" {disabled}>
  <legend class="small muted" style="padding:0;margin-bottom:6px">{label}</legend>
  {#each LEVELS as l}
    <label>
      <input type="radio" name={group} value={l.value} checked={value === l.value} onchange={() => onchange(l.value)} />
      {l.label}
    </label>
  {/each}
</fieldset>
{#if sensitive && (value === 'propose' || value === 'write')}
  <p class="small muted">This space is sensitive: everything an AI saves here waits for your approval.</p>
{/if}
