<!-- Where Honk shows up: the Dock (macOS only) and the menu bar, or the system tray elsewhere. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let {
    popoverHotkey,
    onerror
  }: { popoverHotkey: string | null; onerror: (message: string) => void } = $props();

  let dock = $state(true);
  let menuBar = $state(true);
  let hasDock = $state(false);
  // macOS ignores hiding the Dock icon within a second of showing it, so don't offer to.
  let settling = $state(false);

  const menuBarName = $derived(hasDock ? "menu bar" : "system tray");
  // Honk always keeps a way to open it: the last icon, or the popover's shortcut, can't go.
  const keepsMenuBar = $derived(menuBar && !(hasDock && dock) && !popoverHotkey);
  const keepsDock = $derived(dock && !menuBar && !popoverHotkey);
  const iconsHidden = $derived(!menuBar && !(hasDock && dock));

  onMount(async () => {
    try {
      ({ showInDock: dock, showInMenuBar: menuBar, hasDock } = await invoke<{
        showInDock: boolean;
        showInMenuBar: boolean;
        hasDock: boolean;
      }>("presence"));
    } catch (caught) {
      onerror(String(caught));
    }
  });

  async function changeDock() {
    try {
      await invoke("set_show_in_dock", { show: dock });
      if (dock) {
        settling = true;
        setTimeout(() => (settling = false), 1200);
      }
    } catch (caught) {
      dock = !dock;
      onerror(String(caught));
    }
  }

  async function changeMenuBar() {
    try {
      await invoke("set_show_in_menu_bar", { show: menuBar });
    } catch (caught) {
      menuBar = !menuBar;
      onerror(String(caught));
    }
  }
</script>

<div class="presence">
  <label>
    <input type="checkbox" bind:checked={menuBar} onchange={changeMenuBar} disabled={keepsMenuBar} />
    Show in {menuBarName}
  </label>
  {#if hasDock}
    <label>
      <input type="checkbox" bind:checked={dock} onchange={changeDock} disabled={settling || keepsDock} />
      Show in Dock
    </label>
  {/if}
  {#if keepsMenuBar || keepsDock}
    <span class="hint">
      · {hasDock ? "Turn on the other icon or" : "Set"} a popover shortcut first, so you can still open Honk
    </span>
  {:else if iconsHidden}
    <span class="hint">
      · open Honk with its popover shortcut, or by opening it again.
      <button type="button" onclick={() => invoke("quit")}>Quit Honk</button>
    </span>
  {:else if hasDock && !dock}
    <span class="hint">· quit from the {menuBarName} icon</span>
  {/if}
</div>

<style>
  .presence {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    color: var(--muted);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  input {
    accent-color: var(--accent-edge);
  }

  .hint {
    font-size: 12px;
  }

  .hint button {
    margin-left: var(--space-2);
    font: inherit;
    color: var(--text);
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0 var(--space-2);
    cursor: pointer;
  }

  .hint button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
</style>
