import { invoke } from "@tauri-apps/api/core";

export interface Sound {
  id: number;
  name: string;
  /** Linear, 0 to 1. */
  volume: number;
  favorite: boolean;
  /** A global-shortcut string such as `alt+Digit1`; see `$lib/hotkey`. */
  hotkey: string | null;
}

interface ImportResult {
  path: string;
  sound: Sound | null;
  duplicate: boolean;
  error: string | null;
}

export interface ImportSummary {
  imported: number;
  duplicates: number;
  failed: { path: string; error: string }[];
}

class Library {
  sounds = $state<Sound[]>([]);
  selectedId = $state<number | null>(null);
  stopAllHotkey = $state<string | null>(null);
  /** Saved hotkeys the system refused to register, with why. */
  hotkeyFailures = $state<Record<string, string>>({});

  selected = $derived(this.sounds.find((sound) => sound.id === this.selectedId) ?? null);

  async refresh() {
    [this.sounds, this.stopAllHotkey, this.hotkeyFailures] = await Promise.all([
      invoke<Sound[]>("list_sounds"),
      invoke<string | null>("stop_all_hotkey"),
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

  async setStopAllHotkey(hotkey: string | null) {
    this.stopAllHotkey = await invoke<string | null>("set_stop_all_hotkey", { hotkey });
    this.hotkeyFailures = await invoke<Record<string, string>>("hotkey_failures");
  }

  async import(paths: string[]): Promise<ImportSummary> {
    const results = await invoke<ImportResult[]>("import_sounds", { paths });
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
