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

/** `volume` is linear, 0 to 1. */
export const playSound = (path: string, volume: number) =>
  invoke<void>("play_sound", { path, volume });

export const stopAll = () => invoke<void>("stop_all");
