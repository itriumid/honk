<!--
  The menu bar popover: search, then play. Rendered in its own transparent window, so the page
  background stays clear and macOS's blur shows through.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { playSound, stopAll } from "$lib/audio";
  import { library, type Sound } from "$lib/library.svelte";
  import { playback } from "$lib/playback.svelte";
  import SoundPad from "$lib/components/SoundPad.svelte";

  let query = $state("");
  let error = $state("");
  let search: HTMLInputElement | undefined = $state();

  const favorites = $derived(library.sounds.filter((sound) => sound.favorite));
  const searching = $derived(query.trim() !== "");
  const shown: Sound[] = $derived.by(() => {
    if (!searching) return favorites.length ? favorites : library.sounds;
    const words = query.trim().toLowerCase().split(/\s+/);
    return library.sounds.filter((sound) =>
      words.every((word) => sound.name.toLowerCase().includes(word)),
    );
  });
  const heading = $derived(searching ? "Results" : favorites.length ? "Favorites" : "All sounds");

  async function attempt(action: () => Promise<void>) {
    error = "";
    try {
      await action();
    } catch (caught) {
      error = String(caught);
    }
  }

  const hide = () => invoke("hide_popover");

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && shown.length) {
      attempt(() => playSound(shown[0].id));
    } else if (event.key === "Escape") {
      if (query) query = "";
      else hide();
    }
  }

  onMount(() => {
    document.documentElement.classList.add("popover");
    attempt(() => library.refresh());
    const stopFollowingPlayback = playback.follow();
    // The window is created once and shown and hidden, so refresh each time it opens —
    // the main window may have changed the library meanwhile.
    const stopWatchingFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) return;
      query = "";
      search?.focus();
      attempt(() => library.refresh());
    });
    return () => {
      stopFollowingPlayback();
      stopWatchingFocus.then((unlisten) => unlisten());
    };
  });
</script>

<div class="popover">
  <input
    bind:this={search}
    bind:value={query}
    {onkeydown}
    class="search"
    type="search"
    placeholder="Search sounds"
    aria-label="Search sounds"
    spellcheck="false"
    autocomplete="off"
  />

  <section aria-label={heading}>
    <h2>{heading}</h2>
    {#if shown.length}
      <div class="grid">
        {#each shown as sound (sound.id)}
          <SoundPad
            {sound}
            selected={false}
            playing={playback.latestFor(sound.id)}
            hotkeyBroken={library.failureFor(sound.hotkey) !== null}
            onplay={() => attempt(() => playSound(sound.id))}
          />
        {/each}
      </div>
    {:else if searching}
      <p class="empty">No sounds match “{query.trim()}”.</p>
    {:else}
      <p class="empty">No sounds yet. Open Honk to import some.</p>
    {/if}
  </section>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <footer>
    <button onclick={() => attempt(stopAll)}>■ Stop all</button>
    <button onclick={() => invoke("show_main_window")}>Open Honk</button>
  </footer>
</div>

<style>
  .popover {
    display: grid;
    grid-template-rows: auto 1fr auto auto;
    gap: var(--space-3);
    height: 100%;
    padding: var(--space-3);
  }

  .search {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: var(--glass);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font: inherit;
  }

  .search::placeholder {
    color: var(--muted);
  }

  section {
    overflow-y: auto;
    min-height: 0;
  }

  h2 {
    margin: 0 0 var(--space-2);
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-2);
  }

  .empty {
    margin: var(--space-6) 0;
    color: var(--muted);
    text-align: center;
  }

  .error {
    margin: 0;
    padding: var(--space-2);
    border-left: 2px solid var(--accent);
    background: var(--glass);
    font-size: 12px;
  }

  footer {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
  }

  footer button {
    flex: 1;
    padding: var(--space-2);
    background: var(--glass);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font: inherit;
  }
</style>
