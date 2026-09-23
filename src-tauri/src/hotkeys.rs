//! Global shortcuts: one per pad, plus the app shortcuts (Stop all, the popover). Registered from Rust, so they work while
//! the window is hidden or another app has focus.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::audio_engine::AudioEngine;
use crate::commands::{self, SoundCache};
use crate::library::{AppShortcut, Library};
use crate::popover;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    PlaySound(i64),
    StopAll,
    TogglePopover,
}

/// What each registered shortcut does, and which saved hotkeys the system refused.
#[derive(Default)]
pub struct Hotkeys {
    actions: Mutex<HashMap<u32, Action>>,
    /// Hotkey string to the registration error, for hotkeys saved but not currently working.
    failures: Mutex<HashMap<String, String>>,
    /// Held for a whole re-registration, so two can't interleave their unregister and register.
    registering: Mutex<()>,
}

/// How the modifiers a hotkey needs read on this platform; `super` is ⌘, Win or Super.
#[cfg(target_os = "macos")]
const REQUIRED_MODIFIERS: &str = "⌘, ⌥ or ⌃";
#[cfg(target_os = "windows")]
const REQUIRED_MODIFIERS: &str = "Ctrl, Alt or Win";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const REQUIRED_MODIFIERS: &str = "Ctrl, Alt or Super";

/// Parses a shortcut into its canonical form (`shift+control+alt+super+Key`), which is what gets
/// stored. Requires a modifier other than Shift, so ordinary typing can never trigger a sound.
pub fn normalize(hotkey: &str) -> Result<String, String> {
    let shortcut =
        Shortcut::from_str(hotkey).map_err(|error| format!("not a valid shortcut: {error}"))?;
    if !shortcut
        .mods
        .intersects(Modifiers::SUPER | Modifiers::ALT | Modifiers::CONTROL)
    {
        return Err(format!(
            "a shortcut needs {} — otherwise typing would trigger it",
            REQUIRED_MODIFIERS
        ));
    }
    Ok(shortcut.into_string())
}

/// Replaces every registration with the library's saved hotkeys. A hotkey the system refuses is
/// recorded in the failures rather than stopping the rest.
pub fn register_all(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Hotkeys>();
    let _registering = state.registering.lock().map_err(|_| poisoned())?;
    let library = app.state::<Library>();
    let mut bindings: Vec<(String, Action)> = library
        .list()?
        .into_iter()
        .filter_map(|sound| Some((sound.hotkey?, Action::PlaySound(sound.id))))
        .collect();
    for shortcut in AppShortcut::ALL {
        if let Some(hotkey) = library.app_hotkey(shortcut)? {
            let action = match shortcut {
                AppShortcut::StopAll => Action::StopAll,
                AppShortcut::TogglePopover => Action::TogglePopover,
            };
            bindings.push((hotkey, action));
        }
    }

    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|error| format!("could not reset shortcuts: {error}"))?;

    let mut actions = HashMap::new();
    let mut failures = HashMap::new();
    for (hotkey, action) in bindings {
        let registered = Shortcut::from_str(&hotkey)
            .map_err(|error| error.to_string())
            .and_then(|shortcut| {
                shortcuts
                    .register(shortcut)
                    .map(|()| shortcut)
                    .map_err(|error| error.to_string())
            });
        match registered {
            Ok(shortcut) => {
                actions.insert(shortcut.id(), action);
            }
            Err(error) => {
                failures.insert(hotkey, error);
            }
        }
    }

    *state.actions.lock().map_err(|_| poisoned())? = actions;
    *state.failures.lock().map_err(|_| poisoned())? = failures;
    Ok(())
}

/// Saved hotkeys the system refused to register, with why.
pub fn failures(app: &AppHandle) -> Result<HashMap<String, String>, String> {
    Ok(app
        .state::<Hotkeys>()
        .failures
        .lock()
        .map_err(|_| poisoned())?
        .clone())
}

/// The plugin's handler for every registered shortcut.
pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let action = app
        .state::<Hotkeys>()
        .actions
        .lock()
        .ok()
        .and_then(|actions| actions.get(&shortcut.id()).copied());
    let Some(action) = action else { return };
    if action == Action::TogglePopover {
        popover::toggle(app);
        return;
    }

    // This runs on the main thread, and a first play reads the sound from disk.
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<AudioEngine>();
        let result = match action {
            Action::PlaySound(id) => commands::play_library_sound(
                &engine,
                &app.state::<Library>(),
                &app.state::<SoundCache>(),
                id,
            ),
            Action::StopAll => engine.stop_all(),
            Action::TogglePopover => Ok(()),
        };
        if let Err(error) = result {
            eprintln!("hotkey action failed: {error}");
        }
    });
}

fn poisoned() -> String {
    "the hotkey registry lock is poisoned".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_to_one_canonical_form_whatever_the_spelling() {
        assert_eq!(normalize("alt+Digit1").unwrap(), "alt+Digit1");
        assert_eq!(normalize("Option+1").unwrap(), "alt+Digit1");
        assert_eq!(normalize("super+shift+KeyA").unwrap(), "shift+super+KeyA");
        assert_eq!(normalize("cmd+shift+a").unwrap(), "shift+super+KeyA");
        assert_eq!(normalize("control+alt+F5").unwrap(), "control+alt+F5");
    }

    #[test]
    fn refuses_shortcuts_that_ordinary_typing_would_trigger() {
        assert!(normalize("KeyA").is_err());
        assert!(normalize("shift+KeyA").is_err());
        assert!(normalize("Digit1").is_err());
    }

    #[test]
    fn refuses_what_is_not_a_shortcut() {
        assert!(normalize("").is_err());
        assert!(normalize("alt+").is_err());
        assert!(normalize("alt+KeyA+KeyB").is_err());
        assert!(normalize("alt+NotAKey").is_err());
    }
}
