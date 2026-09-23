<script lang="ts">
  import { theme, type ThemePreference } from "$lib/theme.svelte";

  const options: { value: ThemePreference; label: string }[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];
</script>

<div class="switcher" role="radiogroup" aria-label="Theme">
  {#each options as option}
    <button
      role="radio"
      aria-checked={theme.preference === option.value}
      onclick={() => theme.set(option.value)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .switcher {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    background: var(--elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  button {
    padding: var(--space-1) var(--space-3);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    font: inherit;
    transition:
      background var(--duration) var(--ease),
      color var(--duration) var(--ease);
  }

  button:hover {
    color: var(--text);
  }

  button[aria-checked="true"] {
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
