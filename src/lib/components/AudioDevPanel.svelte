<!--
  Temporary: exercises the audio engine until the real pads exist. Delete it along with its use
  in +page.svelte once the library UI can play sounds.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    listOutputDevices,
    playSound,
    setOutputDevices,
    stopAll,
    type OutputDevice,
  } from "$lib/audio";

  let devices = $state<OutputDevice[]>([]);
  let primary = $state<string | null>(null);
  let secondary = $state<string | null>(null);
  let path = $state("");
  let volume = $state(1);
  let error = $state("");

  async function run(action: () => Promise<void>) {
    error = "";
    try {
      await action();
    } catch (caught) {
      error = String(caught);
    }
  }

  onMount(() =>
    run(async () => {
      devices = await listOutputDevices();
    }),
  );

  const applyDevices = () => run(() => setOutputDevices(primary, secondary));
</script>

<section class="panel" aria-label="Audio engine test panel">
  <h2>Audio engine</h2>

  <label>
    Primary output
    <select bind:value={primary} onchange={applyDevices}>
      <option value={null}>System default</option>
      {#each devices as device (device.id)}
        <option value={device.id}>{device.name}{device.is_default ? " (default)" : ""}</option>
      {/each}
    </select>
  </label>

  <label>
    Secondary output
    <select bind:value={secondary} onchange={applyDevices}>
      <option value={null}>None</option>
      {#each devices as device (device.id)}
        <option value={device.id}>{device.name}</option>
      {/each}
    </select>
  </label>

  <label>
    Sound file
    <input bind:value={path} placeholder="/path/to/sound.mp3" spellcheck="false" />
  </label>

  <label>
    Volume {Math.round(volume * 100)}%
    <input type="range" min="0" max="1" step="0.01" bind:value={volume} />
  </label>

  <div class="actions">
    <button class="primary" disabled={!path} onclick={() => run(() => playSound(path, volume))}>
      Play
    </button>
    <button onclick={() => run(stopAll)}>Stop all</button>
  </div>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .panel {
    display: grid;
    gap: var(--space-3);
    width: min(420px, 100%);
    padding: var(--space-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  label {
    display: grid;
    gap: var(--space-1);
    color: var(--muted);
  }

  select,
  input:not([type="range"]) {
    padding: var(--space-2);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font: inherit;
  }

  input[type="range"] {
    accent-color: var(--accent);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
  }

  button {
    padding: var(--space-2) var(--space-4);
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

  button:disabled {
    opacity: 0.5;
  }

  .error {
    margin: 0;
    color: var(--text);
    padding: var(--space-2);
    border-left: 2px solid var(--accent);
    background: var(--elevated);
  }
</style>
