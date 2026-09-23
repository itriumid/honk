import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemePreference = "system" | "light" | "dark";

const STORAGE_KEY = "honk:theme";

function load(): ThemePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value === "light" || value === "dark") return value;
  } catch {}
  return "system";
}

class Theme {
  preference = $state<ThemePreference>(load());

  set(preference: ThemePreference) {
    this.preference = preference;
    try {
      localStorage.setItem(STORAGE_KEY, preference);
    } catch {}
    this.apply();
  }

  apply() {
    const root = document.documentElement;
    if (this.preference === "system") delete root.dataset.theme;
    else root.dataset.theme = this.preference;

    // Keeps the native title bar in sync; null follows the OS.
    getCurrentWindow()
      .setTheme(this.preference === "system" ? null : this.preference)
      .catch(() => {});
  }
}

export const theme = new Theme();
