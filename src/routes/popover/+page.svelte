<!--
  The menu bar popover: search, then play. Rendered in its own transparent window, so the page
  background stays clear and macOS's blur shows through.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { playSound, stopAll } from "$lib/audio";
  import { library, type Sound } from "$lib/library.svelte";
  import { playback } from "$lib/playback.svelte";
  import SoundPad from "$lib/components/SoundPad.svelte";

  /** Which pads the popover shows when not searching: favorites, everything, or a category. */
  type View = "favorites" | "all" | number;
  const VIEW_KEY = "honk.popover.view";

  let query = $state("");
  let error = $state("");
  let search: HTMLInputElement | undefined = $state();
  let chosen = $state<View | null>(readView());

  const favorites = $derived(library.sounds.filter((sound) => sound.favorite));
  const searching = $derived(query.trim() !== "");
  // The chosen view, unless it no longer applies: a deleted category, or favorites once there
  // are none. Until then it's kept, so the choice survives the library loading.
  const view: View = $derived.by(() => {
    if (typeof chosen === "number") {
      if (library.categories.some((category) => category.id === chosen)) return chosen;
    } else if (chosen === "all") return "all";
    return favorites.length ? "favorites" : "all";
  });
  const shown: Sound[] = $derived.by(() => {
    if (searching) {
      // Search always covers the whole library, whatever view is picked.
      const words = query.trim().toLowerCase().split(/\s+/);
      return library.sounds.filter((sound) =>
        words.every((word) => sound.name.toLowerCase().includes(word)),
      );
    }
    if (view === "favorites") return favorites;
    if (view === "all") return library.sounds;
    return library.sounds.filter((sound) => sound.category_id === view);
  });
  const heading = $derived.by(() => {
    if (searching) return "Results";
    if (view === "favorites") return "Favorites";
    if (view === "all") return "All sounds";
    return library.categories.find((category) => category.id === view)?.name ?? "All sounds";
  });

  // Storage can be unavailable or cleared; the popover works the same without it.
  function readView(): View | null {
    try {
      const stored = localStorage.getItem(VIEW_KEY);
      if (stored === "favorites" || stored === "all") return stored;
      return stored && /^\d+$/.test(stored) ? Number(stored) : null;
    } catch {
      return null;
    }
  }

  function choose(next: View) {
    chosen = next;
    query = "";
    try {
      localStorage.setItem(VIEW_KEY, String(next));
    } catch {
      // Remembering the choice is a convenience; losing it is fine.
    }
  }

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
    // The window is created once and shown and hidden, so refresh each time it opens — the main
    // window may have changed the library meanwhile. Rust announces each opening: on macOS the
    // popover is a panel, which doesn't get the usual focus events.
    const stopWatchingOpens = listen("popover-shown", () => {
      query = "";
      search?.focus();
      attempt(() => library.refresh());
    });
    return () => {
      stopFollowingPlayback();
      stopWatchingOpens.then((unlisten) => unlisten());
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

  <nav class="views" aria-label="Show">
    {#if favorites.length}
      <button class="chip" aria-pressed={!searching && view === "favorites"} onclick={() => choose("favorites")}
        >★ Favorites</button
      >
    {/if}
    <button class="chip" aria-pressed={!searching && view === "all"} onclick={() => choose("all")}
      >All</button
    >
    {#each library.categories as category (category.id)}
      <button
        class="chip"
        aria-pressed={!searching && view === category.id}
        onclick={() => choose(category.id)}>{category.name}</button
      >
    {/each}
  </nav>

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
    {:else if library.sounds.length}
      <p class="empty">Nothing in {heading} yet.</p>
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
    grid-template-rows: auto auto 1fr auto auto;
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

  .views {
    display: flex;
    gap: var(--space-1);
    overflow-x: auto;
    scrollbar-width: none;
  }

  .chip {
    flex: none;
    padding: 2px var(--space-2);
    background: var(--glass);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 999px;
    font: inherit;
    font-size: 12px;
  }

  .chip[aria-pressed="true"] {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
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
