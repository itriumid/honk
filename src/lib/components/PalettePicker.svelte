<script lang="ts">
  import { PALETTES, type PaletteId } from "$lib/palettes";
  import { theme } from "$lib/theme.svelte";
</script>

<label class="palette">
  Colors
  <select
    value={theme.palette}
    onchange={(event) => theme.setPalette(event.currentTarget.value as PaletteId)}
  >
    {#each PALETTES as palette (palette.id)}
      <option value={palette.id}>{palette.label}</option>
    {/each}
  </select>
</label>

<style>
  .palette {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--muted);
  }

  select {
    padding: var(--space-1) var(--space-2);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font: inherit;
  }
</style>
