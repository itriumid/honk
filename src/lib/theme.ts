import { getCurrentWindow } from "@tauri-apps/api/window";
import { Theme } from "@itrium/palettes";

/**
 * The chosen theme and palette, shared by the main window and the popover. app.html reads the
 * same keys before the first paint.
 */
export const theme = new Theme({
  storagePrefix: "honk",
  // Keeps the native title bar in step; null follows the system.
  onApply: (choice) => {
    getCurrentWindow()
      .setTheme(choice)
      .catch(() => {});
  },
});
