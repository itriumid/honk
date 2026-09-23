# Honk

A lightweight, cross-platform soundboard built with Tauri. Import your sounds, bind them to global hotkeys, and fire them from a macOS menu bar popover without leaving whatever you're doing.

> Early development. Nothing to download yet.

## Planned features (v1)

- **Local library**: drag and drop audio files; they are copied into the app's data directory so moving the originals never breaks a pad
- **Pads** with per-sound volume, overlapping playback, and a global **Stop all**
- **Global hotkeys** per pad, plus one to open the popover
- **Menu bar popover** (macOS): search, favorites grid, volume, stop all
- **Share libraries**: export a whole library (or one category) as a single `.honk` file, a zip holding a `manifest.json` plus the audio files, and import one to merge it into your own
- **Output device picker** with dual output (e.g. headphones + a virtual cable like BlackHole or VB-Cable)

## Architecture

| Concern | Choice |
| --- | --- |
| Shell | Tauri 2 |
| Frontend | SvelteKit (static adapter) + TypeScript |
| Audio | Rust (`rodio` / `cpal`), not the webview, for consistent latency and device control on every OS |
| Storage | SQLite for metadata; audio files in the app data directory |
| Hotkeys | `tauri-plugin-global-shortcut` |
| Menu bar | Tray icon + positioned borderless window, hidden on blur |

## Design

Minimal, with one accent color that always means something: playing, focus, or the primary action.

| Token | Dark | Light |
| --- | --- | --- |
| Background | `#2B2B2B` | `#FAFAFA` |
| Surface | `#343434` | `#FFFFFF` |
| Elevated | `#3E3E3E` | `#F1F1F1` |
| Border | `#474747` | `#E4E4E4` |
| Text | `#F2F2F2` | `#2B2B2B` |
| Muted | `#9A9A9A` | `#6B6B6B` |
| Accent | `#FEBFCA` | `#FEBFCA` |

- Text on the accent is always graphite (`#2B2B2B`), never white.
- In light mode the accent is used for fills only, never as text color.
- The popover uses native macOS vibrancy; the main window stays solid.
- System font (SF Pro on macOS). Motion is limited to the playback fill and a subtle press.

## Development

Requires [Rust](https://rustup.rs), Node.js, and pnpm.

```sh
pnpm install
pnpm tauri dev
```

## License

[MIT](LICENSE)
