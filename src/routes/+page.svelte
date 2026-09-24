<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";
  import { playSound, stopAll } from "$lib/audio";
  import { fileName, library, type AppShortcut, type ImportSummary } from "$lib/library.svelte";
  import { playback } from "$lib/playback.svelte";
  import DockSetting from "$lib/components/DockSetting.svelte";
  import HotkeyRecorder from "$lib/components/HotkeyRecorder.svelte";
  import OutputSettings from "$lib/components/OutputSettings.svelte";
  import SoundEditor from "$lib/components/SoundEditor.svelte";
  import SoundPad from "$lib/components/SoundPad.svelte";
  import ThemeSwitcher from "$lib/components/ThemeSwitcher.svelte";

  const APP_SHORTCUTS: [AppShortcut, string][] = [
    ["stop_all", "Stop all"],
    ["toggle_popover", "Popover"],
  ];

  const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "ogg", "oga", "m4a", "aac", "mp4"];

  let dragging = $state(false);
  let notice = $state("");

  // Pads are reordered with pointer events rather than HTML drag and drop: Tauri's native file
  // drop, which import relies on, stops HTML drag and drop from working in the Windows webview.
  /** How far the pointer moves before a press on a pad becomes a drag instead of a click. */
  const DRAG_THRESHOLD = 6;
  /** Within this distance of the list's top or bottom edge, a drag scrolls the list. */
  const SCROLL_EDGE = 32;

  let grid = $state<HTMLElement>();
  let list = $state<HTMLElement>();
  let reordering = $state<{
    id: number;
    pad: HTMLElement;
    pointerId: number;
    startX: number;
    startY: number;
    moved: boolean;
  } | null>(null);
  /** Set when a drag ends, so the click the browser fires on release doesn't play the pad. */
  let swallowClick = false;

  const padIdAt = (x: number, y: number) => {
    const slot = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-sound-id]");
    return slot ? Number(slot.dataset.soundId) : null;
  };

  function pressPad(id: number, event: PointerEvent & { currentTarget: HTMLElement }) {
    // A drag whose release never arrived, because it ended outside the window, is saved now.
    if (reordering?.moved) attempt(() => library.saveOrder());
    reordering = null;
    // Cleared on every press: a drag released outside the window fires no click to swallow.
    swallowClick = false;
    if (event.button !== 0) return;
    reordering = {
      id,
      pad: event.currentTarget,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      moved: false,
    };
  }

  // The drag is followed on the window, not the pad: reordering moves pads around the DOM,
  // which can drop a pad's pointer capture partway through.
  function dragPad(event: PointerEvent) {
    if (!reordering || event.pointerId !== reordering.pointerId) return;
    if (!reordering.moved) {
      const distance = Math.hypot(
        event.clientX - reordering.startX,
        event.clientY - reordering.startY,
      );
      if (distance < DRAG_THRESHOLD) return;
      reordering.moved = true;
      // Keeps the drag's events coming while the pointer is outside the window.
      reordering.pad.setPointerCapture(event.pointerId);
    }
    const target = padIdAt(event.clientX, event.clientY);
    if (target !== null && target !== reordering.id) {
      library.move(
        reordering.id,
        library.sounds.findIndex((sound) => sound.id === target),
      );
    }
    if (list) {
      const edges = list.getBoundingClientRect();
      if (event.clientY < edges.top + SCROLL_EDGE) list.scrollBy(0, -SCROLL_EDGE / 2);
      else if (event.clientY > edges.bottom - SCROLL_EDGE) list.scrollBy(0, SCROLL_EDGE / 2);
    }
  }

  function releasePad(event: PointerEvent) {
    if (!reordering || event.pointerId !== reordering.pointerId) return;
    const moved = reordering.moved;
    reordering = null;
    if (moved) {
      swallowClick = true;
      attempt(() => library.saveOrder());
    }
  }

  function describe(summary: ImportSummary) {
    const parts = [];
    if (summary.imported) parts.push(`Imported ${summary.imported}`);
    if (summary.duplicates) parts.push(`${summary.duplicates} already in the library`);
    for (const failure of summary.failed) {
      parts.push(`${fileName(failure.path)}: ${failure.error}`);
    }
    return parts.join(" · ");
  }

  async function attempt(action: () => Promise<void>) {
    try {
      await action();
    } catch (caught) {
      notice = String(caught);
    }
  }

  const importPaths = (paths: string[]) =>
    attempt(async () => {
      if (paths.length) notice = describe(await library.import(paths));
    });

  async function pickFiles() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    if (picked) await importPaths(picked);
  }

  /** Alt+Arrow moves the focused pad one place, for anyone not using a pointer. */
  async function movePadWithKeyboard(id: number, event: KeyboardEvent) {
    const step = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key];
    if (!event.altKey || step === undefined) return;
    event.preventDefault();
    library.move(id, library.sounds.findIndex((sound) => sound.id === id) + step);
    await tick();
    // Moving a focused element in the DOM can drop its focus; put it back.
    grid?.querySelector<HTMLElement>(`[data-sound-id="${id}"]`)?.focus();
    await attempt(() => library.saveOrder());
  }

  function play(id: number) {
    if (swallowClick) {
      swallowClick = false;
      return;
    }
    library.selectedId = id;
    attempt(() => playSound(id));
  }

  onMount(() => {
    attempt(() => library.refresh());
    const stopFollowingPlayback = playback.follow();
    const stopListening = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") dragging = true;
      else if (event.payload.type === "leave") dragging = false;
      else if (event.payload.type === "drop") {
        dragging = false;
        importPaths(event.payload.paths);
      }
    });
    return () => {
      stopFollowingPlayback();
      stopListening.then((unlisten) => unlisten());
    };
  });
</script>

<svelte:window onpointermove={dragPad} onpointerup={releasePad} onpointercancel={releasePad} />

<div class="app">
  <header>
    <h1>Honk</h1>
    <div class="actions">
      <button onclick={() => attempt(stopAll)}>■ Stop all</button>
      <button class="primary" onclick={pickFiles}>+ Import</button>
    </div>
  </header>

  {#if notice}
    <p class="notice" role="status">
      <span>{notice}</span>
      <button aria-label="Dismiss" onclick={() => (notice = "")}>×</button>
    </p>
  {/if}

  <main bind:this={list}>
    {#if library.sounds.length}
      <p id="reorder-hint" class="visually-hidden">
        Drag a pad, or press Alt and an arrow key, to move it.
      </p>
      <div
        class="grid"
        class:reordering={reordering?.moved}
        role="group"
        aria-label="Sounds"
        bind:this={grid}
      >
        {#each library.sounds as sound (sound.id)}
          <SoundPad
            {sound}
            selected={library.selectedId === sound.id}
            playing={playback.latestFor(sound.id)}
            hotkeyBroken={library.failureFor(sound.hotkey) !== null}
            onplay={() => play(sound.id)}
            data-sound-id={sound.id}
            lifted={reordering?.moved && reordering.id === sound.id}
            aria-describedby="reorder-hint"
            onpointerdown={(event) => pressPad(sound.id, event)}
            onkeydown={(event) => movePadWithKeyboard(sound.id, event)}
          />
        {/each}
      </div>
    {:else}
      <div class="empty">
        <p>No sounds yet.</p>
        <p class="hint">Drop audio files anywhere, or use <strong>Import</strong>.</p>
      </div>
    {/if}
  </main>

  {#if library.selected}
    <SoundEditor sound={library.selected} onerror={(message) => (notice = message)} />
  {/if}

  <footer>
    <OutputSettings onerror={(message) => (notice = message)} />
    {#each APP_SHORTCUTS as [shortcut, label] (shortcut)}
      <div class="app-hotkey">
        <span>{label}</span>
        <HotkeyRecorder
          hotkey={library.appHotkeys[shortcut]}
          failure={library.failureFor(library.appHotkeys[shortcut])}
          onsave={(hotkey) => library.setAppHotkey(shortcut, hotkey)}
        />
      </div>
    {/each}
    <DockSetting onerror={(message) => (notice = message)} />
    <ThemeSwitcher />
  </footer>

  {#if dragging}
    <div class="drop" aria-hidden="true">Drop to import</div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-rows: auto auto 1fr auto auto;
    height: 100%;
  }

  header,
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
  }

  header {
    border-bottom: 1px solid var(--border);
  }

  footer {
    border-top: 1px solid var(--border);
  }

  .app-hotkey {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    color: var(--muted);
  }

  h1 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: var(--space-2);
  }

  button {
    padding: var(--space-1) var(--space-3);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font: inherit;
  }

  button.primary {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }

  .notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin: var(--space-3) var(--space-4) 0;
    padding: var(--space-2) var(--space-3);
    background: var(--elevated);
    border-left: 2px solid var(--accent);
    border-radius: var(--radius-sm);
  }

  .notice button {
    padding: 0 var(--space-2);
    border: 0;
    background: transparent;
    color: var(--muted);
  }

  main {
    overflow-y: auto;
    padding: var(--space-4);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: var(--space-3);
  }

  .grid.reordering {
    cursor: grabbing;
    user-select: none;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .empty {
    display: grid;
    place-content: center;
    height: 100%;
    text-align: center;
  }

  .empty p {
    margin: 0;
    font-weight: 500;
  }

  .empty .hint {
    margin-top: var(--space-1);
    color: var(--muted);
    font-weight: 400;
  }

  .drop {
    position: fixed;
    inset: var(--space-3);
    display: grid;
    place-content: center;
    border: 2px dashed var(--accent);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    font-size: 15px;
    font-weight: 600;
    pointer-events: none;
  }
</style>
