import { getCurrentWindow } from "@tauri-apps/api/window";
import { DEFAULT_PALETTE, isPaletteId, type PaletteId } from "$lib/palettes";

export type ThemePreference = "system" | "light" | "dark";

// app.html reads the same keys before the first paint; keep them in step.
const THEME_KEY = "honk:theme";
const PALETTE_KEY = "honk:palette";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}

function loadPreference(): ThemePreference {
  const value = read(THEME_KEY);
  return value === "light" || value === "dark" ? value : "system";
}

function loadPalette(): PaletteId {
  const value = read(PALETTE_KEY);
  return isPaletteId(value) ? value : DEFAULT_PALETTE;
}

class Theme {
  preference = $state<ThemePreference>(loadPreference());
  palette = $state<PaletteId>(loadPalette());

  constructor() {
    // The main window and the popover share storage, so a change in one reaches the other.
    if (typeof window !== "undefined") {
      window.addEventListener("storage", (event) => {
        if (event.key !== THEME_KEY && event.key !== PALETTE_KEY) return;
        this.preference = loadPreference();
        this.palette = loadPalette();
        this.apply();
      });
    }
  }

  set(preference: ThemePreference) {
    this.preference = preference;
    write(THEME_KEY, preference);
    this.apply();
  }

  setPalette(palette: PaletteId) {
    this.palette = palette;
    write(PALETTE_KEY, palette);
    this.apply();
  }

  apply() {
    const root = document.documentElement;
    if (this.preference === "system") delete root.dataset.theme;
    else root.dataset.theme = this.preference;

    if (this.palette === DEFAULT_PALETTE) delete root.dataset.palette;
    else root.dataset.palette = this.palette;

    // Keeps the native title bar in sync; null follows the OS.
    getCurrentWindow()
      .setTheme(this.preference === "system" ? null : this.preference)
      .catch(() => {});
  }
}

export const theme = new Theme();
