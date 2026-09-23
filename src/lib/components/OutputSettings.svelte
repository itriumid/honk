<script lang="ts">
  import { onMount } from "svelte";
  import { listOutputDevices, setOutputDevices, type OutputDevice } from "$lib/audio";

  let { onerror }: { onerror: (message: string) => void } = $props();

  let devices = $state<OutputDevice[]>([]);
  let primary = $state<string | null>(null);
  let secondary = $state<string | null>(null);

  async function attempt(action: () => Promise<void>) {
    try {
      await action();
    } catch (caught) {
      onerror(String(caught));
    }
  }

  onMount(() =>
    attempt(async () => {
      devices = await listOutputDevices();
    }),
  );

  const apply = () => attempt(() => setOutputDevices(primary, secondary));
</script>

<div class="outputs">
  <label>
    Output
    <select bind:value={primary} onchange={apply}>
      <option value={null}>System default</option>
      {#each devices as device (device.id)}
        <option value={device.id}>{device.name}{device.is_default ? " (default)" : ""}</option>
      {/each}
    </select>
  </label>

  <label>
    Also play on
    <select bind:value={secondary} onchange={apply}>
      <option value={null}>Nothing</option>
      {#each devices as device (device.id)}
        <option value={device.id}>{device.name}</option>
      {/each}
    </select>
  </label>
</div>

<style>
  .outputs {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--muted);
  }

  select {
    max-width: 200px;
    padding: var(--space-1) var(--space-2);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font: inherit;
  }
</style>
