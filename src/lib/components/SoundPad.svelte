<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import type { Sound } from "$lib/library.svelte";
  import { formatHotkey } from "$lib/hotkey";
  import type { ActivePlayback } from "$lib/playback.svelte";

  let {
    sound,
    selected,
    playing,
    hotkeyBroken = false,
    lifted = false,
    onplay,
    ...attributes
  }: HTMLButtonAttributes & {
    sound: Sound;
    selected: boolean;
    playing: ActivePlayback | null;
    /** The hotkey is saved but the system refused to register it. */
    hotkeyBroken?: boolean;
    /** Being dragged to a new place; left dimmed where it will land. */
    lifted?: boolean;
    onplay: () => void;
  } = $props();
</script>

<button
  {...attributes}
  class="pad"
  class:selected
  class:lifted
  aria-pressed={selected}
  onclick={onplay}
  title={sound.name}
>
  {#if playing}
    <!-- Keyed so a replay restarts the fill instead of continuing the old one. -->
    {#key playing.playbackId}
      <span
        class="fill"
        class:unknown={playing.durationMilliseconds === null}
        style:animation-duration={playing.durationMilliseconds === null
          ? null
          : `${playing.durationMilliseconds}ms`}
        aria-hidden="true"
      ></span>
    {/key}
  {/if}
  <span class="name">{sound.name}</span>
  {#if sound.hotkey}
    <kbd class="hotkey" class:broken={hotkeyBroken} title={hotkeyBroken ? "This hotkey isn't working" : undefined}
      >{formatHotkey(sound.hotkey)}</kbd
    >
  {/if}
  {#if sound.favorite}
    <span class="favorite" aria-label="Favorite">★</span>
  {/if}
</button>

<style>
  .pad {
    position: relative;
    overflow: hidden;
    isolation: isolate;
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

  .pad.lifted {
    opacity: 0.4;
  }

  /* Sits under the text, so the name stays readable while it plays. */
  .fill {
    position: absolute;
    inset: 0;
    z-index: -1;
    background: color-mix(in srgb, var(--accent) 45%, transparent);
    transform-origin: left;
    animation: fill linear forwards;
  }

  .fill.unknown {
    animation: pulse 1.2s var(--ease) infinite alternate;
  }

  @keyframes fill {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }

  @keyframes pulse {
    from {
      opacity: 0.35;
    }
    to {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .fill,
    .fill.unknown {
      animation: none;
      opacity: 0.6;
    }
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

  .hotkey {
    position: absolute;
    top: var(--space-2);
    left: var(--space-3);
    color: var(--muted);
    font: inherit;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .hotkey.broken {
    text-decoration: line-through;
    text-decoration-color: var(--accent);
  }

  .favorite {
    position: absolute;
    top: var(--space-2);
    right: var(--space-3);
    color: var(--muted);
    font-size: 11px;
  }
</style>
