<script lang="ts">
  import ThemeSwitcher from "$lib/components/ThemeSwitcher.svelte";
</script>

<main>
  <h1>Honk</h1>
  <ThemeSwitcher />
</main>

<style>
  main {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: var(--space-4);
    height: 100%;
  }

  h1 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
</style>
