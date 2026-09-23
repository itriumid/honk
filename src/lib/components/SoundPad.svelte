<script lang="ts">
  import type { Sound } from "$lib/library.svelte";

  let {
    sound,
    selected,
    onplay,
  }: { sound: Sound; selected: boolean; onplay: () => void } = $props();
</script>

<button class="pad" class:selected aria-pressed={selected} onclick={onplay} title={sound.name}>
  <span class="name">{sound.name}</span>
  {#if sound.favorite}
    <span class="favorite" aria-label="Favorite">★</span>
  {/if}
</button>

<style>
  .pad {
    position: relative;
    display: flex;
    align-items: flex-end;
    aspect-ratio: 1;
    padding: var(--space-3);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    font: inherit;
    text-align: left;
    transition:
      transform var(--duration) var(--ease),
      background var(--duration) var(--ease),
      border-color var(--duration) var(--ease);
  }

  .pad:hover {
    background: var(--elevated);
  }

  .pad:active {
    transform: scale(0.97);
  }

  .pad.selected {
    border-color: var(--accent);
  }

  .name {
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    font-weight: 500;
    word-break: break-word;
  }

  .favorite {
    position: absolute;
    top: var(--space-2);
    right: var(--space-3);
    color: var(--muted);
    font-size: 11px;
  }
</style>
