# Honk

A lightweight, cross-platform soundboard built with Tauri. Import your sounds, bind them to global hotkeys, and fire them from a macOS menu bar popover without leaving whatever you're doing.

> Early days: [the latest release](https://github.com/itriumid/honk/releases/latest) works, but expect rough edges. Made by [Itrium](https://github.com/itriumid).

## Privacy

Honk makes no network requests of its own: no accounts, no analytics, no telemetry, no update
checks. Your sounds and settings stay on your computer, and a `.honk` file only goes where you
send it. The code is all here, so you can check.

## Install

Download the installer for your system from the latest release on the [Releases page](https://github.com/itriumid/honk/releases).

| System | File | Notes |
| --- | --- | --- |
| macOS (Apple Silicon and Intel) | `Honk_<version>_universal.dmg` | One download for every Mac |
| Windows 10 or 11 | `Honk_<version>_x64-setup.exe` | Or the `.msi`, if you'd rather manage it with Group Policy |
| Windows 10, 32-bit | `Honk_<version>_x86-setup.exe` | Or the `x86` `.msi`. From the release after 0.1.0 |
| Windows on ARM | `Honk_<version>_arm64-setup.exe` | No `.msi` for ARM. From the release after 0.1.0 |
| Windows, without installing | `Honk_<version>_x64-portable.exe` (or `x86`, `arm64`) | Needs Microsoft Edge WebView2, which Windows 11 includes. Your library is still stored in your user folder. From the release after 0.1.0 |
| Linux (Debian, Ubuntu and derivatives) | `Honk_<version>_amd64.deb` | |
| Linux (Fedora, openSUSE and derivatives) | `Honk-<version>-1.x86_64.rpm` | |
| Linux (anything else) | `Honk_<version>_amd64.AppImage` | `chmod +x` it, then run it |
| Linux on ARM (Raspberry Pi 4/5, ARM laptops) | `Honk_<version>_arm64.deb`, `Honk-<version>-1.aarch64.rpm` or `Honk_<version>_aarch64.AppImage` | 64-bit ARM only. From the release after 0.1.0 |

Versions follow [Semantic Versioning](https://semver.org), and upgrading never loses your
library. From `1.0.0` on, only a new major version can break compatibility, such as older
`.honk` files no longer opening. Honk is still `0.x`, where a minor version may do that, so check
the release notes before updating.

### Honk isn't signed, so your system will warn you the first time

Code signing certificates cost money every year, and Honk is a free hobby project, so the
installers aren't signed by a verified developer. The app is built from this repository's source
by [the release workflow](.github/workflows/release.yml), in public, on GitHub's own machines —
but your operating system has no way to know that, so it asks you to confirm once. Signing is
tracked in [#12](https://github.com/itriumid/honk/issues/12).

**macOS.** The first time you open Honk, macOS says it "could not verify Honk is free of malware"
and only offers to move it to the Bin. To open it anyway:

1. Drag Honk from the `.dmg` into **Applications**, and try to open it once. Click **Done** on the
   warning.
2. Open **System Settings → Privacy & Security**, scroll to **Security**, and click **Open Anyway**
   next to the message about Honk.
3. Confirm with your password or Touch ID, then click **Open Anyway** once more.

macOS remembers the choice; you won't be asked again for that version. On macOS 14 and earlier,
Control-clicking the app and choosing **Open** does the same thing. From macOS 15 onwards that
shortcut no longer works, so use the steps above.

If macOS instead says Honk "is damaged and can't be opened", the download's quarantine flag is
the problem, not the app. Remove it in Terminal, then open Honk normally:

```sh
xattr -dr com.apple.quarantine /Applications/Honk.app
```

**Windows.** SmartScreen shows "Windows protected your PC". Click **More info**, then **Run
anyway**.

**Linux.** No warning. Package managers install `.deb` and `.rpm` files without a signature
check when you open the file directly.

### Platform notes

- Honk is built and tested mainly on macOS. The Windows and Linux builds compile and bundle in CI
  on every change, but haven't been tried by hand yet ([#11](https://github.com/itriumid/honk/issues/11)).
  Reports are welcome.
- The popover is designed for the macOS menu bar. On Windows it opens from the tray icon as a
  plain window, without the translucent background. On Linux, clicking the tray icon shows its
  menu instead, so the popover only opens from its hotkey ([#10](https://github.com/itriumid/honk/issues/10)).
- Global hotkeys on Linux need an X11 session. Under Wayland most compositors don't let an app
  register system-wide shortcuts, so pad hotkeys may not fire outside the Honk window
  ([#9](https://github.com/itriumid/honk/issues/9)).

## Features

- **Local library**: drag and drop audio files; they are copied into the app's data directory so moving the originals never breaks a pad
- **Pads** with per-sound volume, overlapping playback, and a global **Stop all**; drag a pad (or press Alt and an arrow key) to reorder
- **Categories**: put each pad in one category (or none), and filter the main window and the popover by it
- **Global hotkeys** per pad, plus one to open the popover
- **Menu bar popover** (macOS): search, favorites grid, volume, stop all
- **Share libraries**: export a whole library (or the category you're viewing) as a single `.honk` file, and import one, from **Import** or by dropping it on the window, to merge it into your own
  - A preview shows what's new, what you already have, and where everything goes before anything is added
  - Sounds keep the file's categories; one with the same name as yours, ignoring case, joins yours
  - Sounds already in your library are matched by their audio, not their name, and left as they are
  - Exports always include hotkeys; importing them is opt-in (off by default), and each one that's already taken is yours to skip or replace
  - A `.honk` is a zip holding a `manifest.json` plus the audio files. Imports treat it as untrusted: every sound is size-capped, checked against its checksum, and has to play, and a single bad sound stops the whole import before anything is added
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

Setting up, the pull request rules, and how releases are cut are all in
[CONTRIBUTING.md](CONTRIBUTING.md). The short version:

```sh
pnpm install
pnpm tauri dev
```

## License

[MIT](LICENSE)
