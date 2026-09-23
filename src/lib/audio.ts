import { invoke } from "@tauri-apps/api/core";

export interface OutputDevice {
  id: string;
  name: string;
  is_default: boolean;
}

export const listOutputDevices = () => invoke<OutputDevice[]>("list_output_devices");

/** `primary: null` means the system default; `secondary: null` means none. */
export const setOutputDevices = (primary: string | null, secondary: string | null) =>
  invoke<void>("set_output_devices", { primary, secondary });

/** Plays a library sound at its saved volume. */
export const playSound = (id: number) => invoke<void>("play_sound", { id });

export const stopAll = () => invoke<void>("stop_all");
