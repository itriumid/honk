<!--
  Shows what importing a .honk file will do, and imports it once confirmed. Nothing is written
  before that. Hotkeys are off unless asked for; one that's already taken is skipped unless it's
  switched to Replace.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { formatHotkey } from "$lib/hotkey";
  import {
    fileName,
    library,
    type LibraryFilePreview,
    type LibraryFileReport,
  } from "$lib/library.svelte";

  let {
    path,
    preview,
    onclose,
  }: {
    path: string;
    preview: LibraryFilePreview;
    /** With the result once imported, or `null` when cancelled. */
    onclose: (report: LibraryFileReport | null) => void;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let importHotkeys = $state(false);
  /** Taken hotkeys switched to Replace. */
  let replace = $state<Record<string, boolean>>({});
  let importing = $state(false);
  let error = $state("");

  const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;
  const destinations = $derived(
    preview.categories.filter((category) => category.sounds > 0 || !category.merges),
  );

  onMount(() => dialog?.showModal());

  async function confirm() {
    importing = true;
    error = "";
    try {
      const replacing = Object.keys(replace).filter((hotkey) => replace[hotkey]);
      onclose(await library.importFile(path, importHotkeys, replacing));
    } catch (caught) {
      error = String(caught);
      importing = false;
    }
  }
</script>

<dialog bind:this={dialog} oncancel={() => onclose(null)} aria-labelledby="import-title">
  <h2 id="import-title">Import “{fileName(path)}”</h2>

  <p>
    {#if preview.new_sounds}
      <strong>{plural(preview.new_sounds, "new sound")}</strong>
    {:else}
      <strong>Nothing new:</strong> every sound in this file is already in your library.
    {/if}
    {#if preview.duplicates && preview.new_sounds}
      · {plural(preview.duplicates, "sound")} already in your library, left as they are
    {/if}
  </p>

  {#if preview.new_sounds || destinations.length}
    <h3>Where they go</h3>
    <ul class="destinations">
      {#each destinations as category (category.name)}
        <li>
          <span>{category.name}</span>
          <span class="muted">
            {category.sounds ? plural(category.sounds, "sound") : "empty"} ·
            {category.merges ? "joins yours" : "new category"}
          </span>
        </li>
      {/each}
      {#if preview.uncategorized}
        <li>
          <span>No category</span>
          <span class="muted">{plural(preview.uncategorized, "sound")}</span>
        </li>
      {/if}
    </ul>
  {/if}

  {#if preview.hotkeys.length}
    <label class="toggle">
      <input type="checkbox" bind:checked={importHotkeys} />
      Import their hotkeys ({preview.hotkeys.length})
    </label>
    {#if importHotkeys}
      <ul class="hotkeys">
        {#each preview.hotkeys as entry, index (index)}
          <li>
            <kbd>{formatHotkey(entry.hotkey)}</kbd>
            <span>{entry.sound}</span>
            {#if entry.repeated}
              <span class="muted">Skipped: an earlier sound in this file has it</span>
            {:else if entry.taken_by}
              <span class="muted">Used by {entry.taken_by}</span>
              <span class="choice" role="radiogroup" aria-label="{formatHotkey(entry.hotkey)}">
                <button
                  role="radio"
                  aria-checked={!replace[entry.hotkey]}
                  onclick={() => (replace[entry.hotkey] = false)}>Skip</button
                >
                <button
                  role="radio"
                  aria-checked={!!replace[entry.hotkey]}
                  onclick={() => (replace[entry.hotkey] = true)}>Replace</button
                >
              </span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <p class="muted small">Made with Honk {preview.made_with}</p>

  <div class="buttons">
    <button onclick={() => onclose(null)} disabled={importing}>Cancel</button>
    <button class="primary" onclick={confirm} disabled={importing || !preview.new_sounds}>
      {importing ? "Importing…" : "Import"}
    </button>
  </div>
</dialog>

<style>
  dialog {
    width: min(460px, calc(100vw - 2 * var(--space-4)));
    max-height: calc(100vh - 2 * var(--space-4));
    padding: var(--space-4);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  dialog::backdrop {
    background: color-mix(in srgb, var(--bg) 70%, transparent);
  }

  h2 {
    margin: 0 0 var(--space-3);
    font-size: 15px;
    overflow-wrap: anywhere;
  }

  h3 {
    margin: var(--space-4) 0 var(--space-2);
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  p {
    margin: 0 0 var(--space-2);
  }

  ul {
    display: grid;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .destinations li {
    justify-content: space-between;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .toggle input {
    accent-color: var(--accent);
  }

  .hotkeys {
    margin-top: var(--space-3);
  }

  kbd {
    min-width: 4ch;
    color: var(--muted);
    font: inherit;
    font-weight: 600;
  }

  .muted {
    color: var(--muted);
  }

  .small {
    margin-top: var(--space-4);
    font-size: 12px;
  }

  .choice {
    display: inline-flex;
    margin-left: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .choice button {
    padding: 2px var(--space-2);
    background: transparent;
    color: var(--muted);
    border: 0;
    font: inherit;
  }

  .choice button[aria-checked="true"] {
    background: var(--accent);
    color: var(--on-accent);
  }

  .error {
    padding: var(--space-2);
    border-left: 2px solid var(--accent);
    background: var(--elevated);
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  .buttons button {
    padding: var(--space-1) var(--space-3);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font: inherit;
  }

  .buttons button.primary {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }

  .buttons button:disabled {
    opacity: 0.5;
  }
</style>
