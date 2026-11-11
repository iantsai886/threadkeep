<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    confirmLabel = 'Continue',
    danger = false,
    onconfirm,
    oncancel,
    children,
  }: {
    title: string;
    confirmLabel?: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
    children: Snippet;
  } = $props();

  let confirmButton = $state<HTMLButtonElement>();
  $effect(() => confirmButton?.focus());
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && oncancel()} />

<div class="overlay">
  <div class="card" role="alertdialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <div class="stack">{@render children()}</div>
    <div class="row">
      <button class="primary" class:danger bind:this={confirmButton} onclick={onconfirm}>{confirmLabel}</button>
      <button onclick={oncancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  button.primary.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--surface);
  }
</style>
