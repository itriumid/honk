<script lang="ts">
  import { library, type Sound } from "$lib/library.svelte";

  let { sound, onerror }: { sound: Sound; onerror: (message: string) => void } = $props();

  let name = $state("");
  let volume = $state(1);
  let confirmingDelete = $state(false);

  // Re-sync the fields whenever a different sound is selected, or the saved one changes.
  $effect(() => {
    name = sound.name;
    volume = sound.volume;
    confirmingDelete = false;
  });

  async function attempt(action: () => Promise<void>) {
    try {
      await action();
    } catch (caught) {
      onerror(String(caught));
    }
  }

  function saveName() {
    if (name.trim() === sound.name) return;
    attempt(() => library.rename(sound.id, name));
  }

  function deleteSound() {
    if (!confirmingDelete) {
      confirmingDelete = true;
      setTimeout(() => (confirmingDelete = false), 3000);
      return;
    }
    attempt(() => library.delete(sound.id));
  }
</script>

<section class="editor" aria-label="Selected sound">
  <input
    class="name"
    aria-label="Name"
    bind:value={name}
    onblur={saveName}
    onkeydown={(event) => event.key === "Enter" && event.currentTarget.blur()}
  />

  <label class="volume">
    <span>Volume {Math.round(volume * 100)}%</span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.01"
      bind:value={volume}
      onchange={() => attempt(() => library.setVolume(sound.id, volume))}
    />
  </label>

  <button
    aria-pressed={sound.favorite}
    onclick={() => attempt(() => library.setFavorite(sound.id, !sound.favorite))}
  >
    {sound.favorite ? "★ Favorite" : "☆ Favorite"}
  </button>

  <button class:danger={confirmingDelete} onclick={deleteSound}>
    {confirmingDelete ? "Confirm delete" : "Delete"}
  </button>
</section>

<style>
  .editor {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border-top: 1px solid var(--border);
  }

  .name {
    flex: 1 1 160px;
    padding: var(--space-2);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font: inherit;
    font-weight: 500;
  }

  .volume {
    display: grid;
    gap: var(--space-1);
    flex: 1 1 160px;
    color: var(--muted);
  }

  input[type="range"] {
    accent-color: var(--accent);
  }

  button {
    padding: var(--space-2) var(--space-3);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font: inherit;
  }

  button.danger {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }
</style>
