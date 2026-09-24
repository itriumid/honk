import { invoke } from "@tauri-apps/api/core";

export interface Sound {
  id: number;
  name: string;
  /** Linear, 0 to 1. */
  volume: number;
  favorite: boolean;
  /** A global-shortcut string such as `alt+Digit1`; see `$lib/hotkey`. */
  hotkey: string | null;
  category_id: number | null;
}

export interface Category {
  id: number;
  name: string;
}

interface ImportResult {
  path: string;
  sound: Sound | null;
  duplicate: boolean;
  error: string | null;
}

/** App-wide shortcuts, as opposed to one per pad. */
export type AppShortcut = "stop_all" | "toggle_popover";

export interface ImportSummary {
  imported: number;
  duplicates: number;
  failed: { path: string; error: string }[];
}

class Library {
  sounds = $state<Sound[]>([]);
  categories = $state<Category[]>([]);
  selectedId = $state<number | null>(null);
  /** The category the main window is showing, or `null` for every sound. */
  activeCategoryId = $state<number | null>(null);
  appHotkeys = $state<Record<AppShortcut, string | null>>({
    stop_all: null,
    toggle_popover: null,
  });
  /** Saved hotkeys the system refused to register, with why. */
  hotkeyFailures = $state<Record<string, string>>({});

  selected = $derived(this.sounds.find((sound) => sound.id === this.selectedId) ?? null);
  /** What the main window's grid shows: the active category, or everything. */
  visible = $derived(
    this.activeCategoryId === null
      ? this.sounds
      : this.sounds.filter((sound) => sound.category_id === this.activeCategoryId),
  );

  async refresh() {
    [this.sounds, this.categories, this.appHotkeys, this.hotkeyFailures] = await Promise.all([
      invoke<Sound[]>("list_sounds"),
      invoke<Category[]>("list_categories"),
      invoke<Record<AppShortcut, string | null>>("app_hotkeys"),
      invoke<Record<string, string>>("hotkey_failures"),
    ]);
  }

  failureFor(hotkey: string | null): string | null {
    return hotkey ? (this.hotkeyFailures[hotkey] ?? null) : null;
  }

  async setHotkey(id: number, hotkey: string | null) {
    this.replace(await invoke<Sound>("set_sound_hotkey", { id, hotkey }));
    this.hotkeyFailures = await invoke<Record<string, string>>("hotkey_failures");
  }

  async setAppHotkey(shortcut: AppShortcut, hotkey: string | null) {
    this.appHotkeys[shortcut] = await invoke<string | null>("set_app_hotkey", { shortcut, hotkey });
    this.hotkeyFailures = await invoke<Record<string, string>>("hotkey_failures");
  }

  async import(paths: string[]): Promise<ImportSummary> {
    // New sounds go in the category being shown, so they don't vanish from view on import.
    const results = await invoke<ImportResult[]>("import_sounds", {
      paths,
      categoryId: this.activeCategoryId,
    });
    await this.refresh();
    return {
      imported: results.filter((result) => result.sound && !result.duplicate).length,
      duplicates: results.filter((result) => result.duplicate).length,
      failed: results
        .filter((result) => result.error !== null)
        .map((result) => ({ path: result.path, error: result.error ?? "" })),
    };
  }

  async rename(id: number, name: string) {
    this.replace(await invoke<Sound>("rename_sound", { id, name }));
  }

  async setVolume(id: number, volume: number) {
    this.replace(await invoke<Sound>("set_sound_volume", { id, volume }));
  }

  async setFavorite(id: number, favorite: boolean) {
    this.replace(await invoke<Sound>("set_sound_favorite", { id, favorite }));
  }

  async setCategory(id: number, categoryId: number | null) {
    this.replace(await invoke<Sound>("set_sound_category", { id, categoryId }));
  }

  async createCategory(name: string) {
    const category = await invoke<Category>("create_category", { name });
    this.categories = [...this.categories, category];
    return category;
  }

  async renameCategory(id: number, name: string) {
    const renamed = await invoke<Category>("rename_category", { id, name });
    this.categories = this.categories.map((category) => (category.id === id ? renamed : category));
  }

  /** Deletes the category; its sounds stay, in no category. */
  async deleteCategory(id: number) {
    await invoke<void>("delete_category", { id });
    this.categories = this.categories.filter((category) => category.id !== id);
    this.sounds = this.sounds.map((sound) =>
      sound.category_id === id ? { ...sound, category_id: null } : sound,
    );
    if (this.activeCategoryId === id) this.activeCategoryId = null;
  }

  /** Moves a pad to `index` in the list. Only local until `saveOrder`, so a drag can preview. */
  move(id: number, index: number) {
    const from = this.sounds.findIndex((sound) => sound.id === id);
    const to = Math.max(0, Math.min(index, this.sounds.length - 1));
    if (from === -1 || from === to) return;
    const sounds = [...this.sounds];
    const [sound] = sounds.splice(from, 1);
    sounds.splice(to, 0, sound);
    this.sounds = sounds;
  }

  /** Saves the current pad order, or puts the saved one back if that fails. */
  async saveOrder() {
    try {
      await invoke<void>("reorder_sounds", { ids: this.sounds.map((sound) => sound.id) });
    } catch (caught) {
      await this.refresh();
      throw caught;
    }
  }

  async delete(id: number) {
    await invoke<void>("delete_sound", { id });
    this.sounds = this.sounds.filter((sound) => sound.id !== id);
    if (this.selectedId === id) this.selectedId = null;
  }

  private replace(updated: Sound) {
    this.sounds = this.sounds.map((sound) => (sound.id === updated.id ? updated : sound));
  }
}

export const library = new Library();

/** The file name without its folder, for messages about a path. */
export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
