<!--
  The main window's category chips: pick which pads the grid shows, and create, rename or delete
  categories. Deleting a category keeps its sounds, in no category.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { library } from "$lib/library.svelte";

  let { onerror }: { onerror: (message: string) => void } = $props();

  /** What the text field is for, if it's open: a new category, or renaming the active one. */
  let editing = $state<"new" | "rename" | null>(null);
  let draft = $state("");
  let field: HTMLInputElement | undefined = $state();
  let confirmingDelete = $state(false);

  const active = $derived(
    library.categories.find((category) => category.id === library.activeCategoryId) ?? null,
  );

  // Asking to confirm a delete is about the category being shown; switching away cancels it.
  $effect(() => {
    void library.activeCategoryId;
    confirmingDelete = false;
  });

  async function attempt(action: () => Promise<void>) {
    try {
      await action();
    } catch (caught) {
      onerror(String(caught));
    }
  }

  async function startEditing(purpose: "new" | "rename") {
    editing = purpose;
    draft = purpose === "rename" ? (active?.name ?? "") : "";
    await tick();
    field?.select();
  }

  function finishEditing() {
    const purpose = editing;
    const name = draft.trim();
    editing = null;
    if (!name) return;
    if (purpose === "new") {
      attempt(async () => {
        library.activeCategoryId = (await library.createCategory(name)).id;
      });
    } else if (purpose === "rename" && active && name !== active.name) {
      const id = active.id;
      attempt(() => library.renameCategory(id, name));
    }
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") field?.blur();
    else if (event.key === "Escape") {
      draft = "";
      field?.blur();
    }
  }

  function deleteActive() {
    if (!active) return;
    if (!confirmingDelete) {
      confirmingDelete = true;
      setTimeout(() => (confirmingDelete = false), 3000);
      return;
    }
    const id = active.id;
    attempt(() => library.deleteCategory(id));
  }
</script>

{#snippet nameField(label: string)}
  <input
    bind:this={field}
    bind:value={draft}
    class="chip field"
    aria-label={label}
    placeholder="Category name"
    maxlength="40"
    onblur={finishEditing}
    {onkeydown}
  />
{/snippet}

<nav class="categories" aria-label="Categories">
  <div class="chips">
    <button
      class="chip"
      aria-pressed={library.activeCategoryId === null}
      onclick={() => (library.activeCategoryId = null)}>All</button
    >
    {#each library.categories as category (category.id)}
      {#if editing === "rename" && category.id === active?.id}
        {@render nameField(`Rename ${category.name}`)}
      {:else}
        <button
          class="chip"
          aria-pressed={library.activeCategoryId === category.id}
          onclick={() => (library.activeCategoryId = category.id)}>{category.name}</button
        >
      {/if}
    {/each}
    {#if editing === "new"}
      {@render nameField("New category")}
    {:else}
      <button class="chip add" onclick={() => startEditing("new")}>+ New</button>
    {/if}
  </div>

  {#if active && editing === null}
    <div class="manage">
      <button onclick={() => startEditing("rename")}>Rename</button>
      <button class:danger={confirmingDelete} onclick={deleteActive}>
        {confirmingDelete ? "Confirm delete" : "Delete"}
      </button>
    </div>
  {/if}
</nav>

<style>
  .categories {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4) 0;
  }

  .chips,
  .manage {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .chip {
    padding: var(--space-1) var(--space-3);
    background: var(--elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 999px;
    font: inherit;
  }

  .chip[aria-pressed="true"] {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }

  .chip.add {
    color: var(--muted);
    background: transparent;
    border-style: dashed;
  }

  .chip.field {
    width: 14ch;
    outline: none;
    border-color: var(--accent);
  }

  .manage button {
    padding: var(--space-1) var(--space-2);
    background: transparent;
    color: var(--muted);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    font: inherit;
  }

  .manage button:hover {
    color: var(--text);
  }

  .manage button.danger {
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
