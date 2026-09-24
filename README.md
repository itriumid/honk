# Honk

A lightweight, cross-platform soundboard built with Tauri. Import your sounds, bind them to global hotkeys, and fire them from a macOS menu bar popover without leaving whatever you're doing.

> Early development. No release has been published yet; when one is, it'll be on the [Releases page](https://github.com/muhammad-zakir/honk/releases).

## Install

Download the installer for your system from the latest release on the [Releases page](https://github.com/muhammad-zakir/honk/releases).

| System | File | Notes |
| --- | --- | --- |
| macOS (Apple Silicon and Intel) | `Honk_<version>_universal.dmg` | One download for every Mac |
| Windows 10 or 11 | `Honk_<version>_x64-setup.exe` | Or the `.msi`, if you'd rather manage it with Group Policy |
| Linux (Debian, Ubuntu and derivatives) | `Honk_<version>_amd64.deb` | |
| Linux (Fedora, openSUSE and derivatives) | `Honk-<version>-1.x86_64.rpm` | |
| Linux (anything else) | `Honk_<version>_amd64.AppImage` | `chmod +x` it, then run it |

### Honk isn't signed, so your system will warn you the first time

Code signing certificates cost money every year, and Honk is a free hobby project, so the
installers aren't signed by a verified developer. The app is built from this repository's source
by [the release workflow](.github/workflows/release.yml), in public, on GitHub's own machines —
but your operating system has no way to know that, so it asks you to confirm once.

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
  on every change, but get far less hands-on use — reports are welcome.
- The popover is designed for the macOS menu bar. On Windows and Linux it opens from the tray icon
  as a plain window, without the translucent background. Some Linux desktops don't deliver tray
  icon clicks to apps at all; use the popover's hotkey there instead.
- Global hotkeys on Linux need an X11 session. Under Wayland most compositors don't let an app
  register system-wide shortcuts, so pad hotkeys may not fire outside the Honk window.

## Planned features (v1)

- **Local library**: drag and drop audio files; they are copied into the app's data directory so moving the originals never breaks a pad
- **Pads** with per-sound volume, overlapping playback, and a global **Stop all**; drag a pad (or press Alt and an arrow key) to reorder
- **Global hotkeys** per pad, plus one to open the popover
- **Menu bar popover** (macOS): search, favorites grid, volume, stop all
- **Share libraries**: export a whole library (or one category) as a single `.honk` file, a zip holding a `manifest.json` plus the audio files, and import one to merge it into your own
  - Exports always include hotkeys; importing them is opt-in (off by default), and conflicts with existing bindings are shown for you to skip or replace
  - Duplicate sounds are detected by content hash, not filename
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

On Linux, install Tauri's [system dependencies](https://tauri.app/start/prerequisites/#linux)
plus the ALSA headers (`libasound2-dev` on Debian and Ubuntu, `alsa-lib-devel` on Fedora).

### Continuous integration

Every pull request is type-checked, tested, and bundled on macOS, Linux and Windows by
[`build.yml`](.github/workflows/build.yml). All three are required checks, so nothing reaches
`main` without compiling everywhere.

### Releasing

1. Set the new version in all three places: `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`. Land that through a pull request, like any other change.
2. Tag the merge commit on `main` and push the tag:

   ```sh
   git switch main && git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. [`release.yml`](.github/workflows/release.yml) checks that the tag matches all three versions,
   creates a **draft** release with notes generated from the merged pull requests' labels, and
   builds the installers on each operating system, attaching them to that draft as they finish.
4. When all three builds are green, open the draft on the Releases page, check the notes and the
   attached files, and click **Publish release**.

To try the release builds without making a release, run the **Release** workflow manually from
the Actions tab. It builds the same installers and keeps them as workflow artifacts instead.

## License

[MIT](LICENSE)
