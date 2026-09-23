import { listen } from "@tauri-apps/api/event";
import { SvelteMap } from "svelte/reactivity";

type PlaybackEvent =
  | {
      kind: "started";
      playback_id: number;
      sound_id: number;
      duration_milliseconds: number | null;
    }
  | { kind: "finished"; playback_id: number; sound_id: number };

export interface ActivePlayback {
  playbackId: number;
  /** `null` when the file doesn't say how long it is. */
  durationMilliseconds: number | null;
}

/** What's playing right now, as reported by the audio engine. */
class Playback {
  /** The most recent playback of each sound that is still playing. */
  private latest = new SvelteMap<number, ActivePlayback>();
  private listening: Promise<() => void> | null = null;

  latestFor(soundId: number): ActivePlayback | null {
    return this.latest.get(soundId) ?? null;
  }

  /** Starts following engine events; returns a function that stops. */
  follow(): () => void {
    this.listening ??= listen<PlaybackEvent>("playback", ({ payload }) => {
      if (payload.kind === "started") {
        this.latest.set(payload.sound_id, {
          playbackId: payload.playback_id,
          durationMilliseconds: payload.duration_milliseconds,
        });
      } else if (this.latest.get(payload.sound_id)?.playbackId === payload.playback_id) {
        // An earlier, overlapping play finishing doesn't end the one shown.
        this.latest.delete(payload.sound_id);
      }
    });
    return () => {
      this.listening?.then((unlisten) => unlisten());
      this.listening = null;
    };
  }
}

export const playback = new Playback();
