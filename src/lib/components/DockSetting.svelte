<!-- macOS only: whether Honk shows in the Dock, or lives in the menu bar alone. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { detectPlatform } from "$lib/hotkey";

  let { onerror }: { onerror: (message: string) => void } = $props();

  const available = detectPlatform() === "macos";
  let show = $state(true);
  // macOS ignores hiding the Dock icon within a second of showing it, so don't offer to.
  let settling = $state(false);

  onMount(async () => {
    if (!available) return;
    try {
      show = await invoke<boolean>("show_in_dock");
    } catch (caught) {
      onerror(String(caught));
    }
  });

  async function change() {
    try {
      await invoke("set_show_in_dock", { show });
      if (show) {
        settling = true;
        setTimeout(() => (settling = false), 1200);
      }
    } catch (caught) {
      show = !show;
      onerror(String(caught));
    }
  }
</script>

{#if available}
  <label class="dock" title={show ? undefined : "Quit from the menu bar icon — ⌘Q needs the Dock icon"}>
    <input type="checkbox" bind:checked={show} onchange={change} disabled={settling} />
    Show in Dock
    {#if !show}
      <span class="hint">· quit from the menu bar icon</span>
    {/if}
  </label>
{/if}

<style>
  .dock {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--muted);
  }

  input {
    accent-color: var(--accent);
  }

  .hint {
    font-size: 12px;
  }
</style>
