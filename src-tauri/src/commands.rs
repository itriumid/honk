use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::audio_engine::{AudioEngine, SoundData};
use crate::hotkeys;
use crate::library::{AppShortcut, ImportResult, Library, Sound};
use crate::output_devices::{self, OutputDevice};
use crate::popover;

/// Stored file contents by sound id, so replaying a sound never touches the disk.
#[derive(Default)]
pub struct SoundCache(Mutex<HashMap<i64, SoundData>>);

impl SoundCache {
    fn load(&self, library: &Library, id: i64) -> Result<SoundData, String> {
        let mut sounds = self.lock()?;
        if let Some(sound) = sounds.get(&id) {
            return Ok(sound.clone());
        }
        let sound = library.load(id)?;
        sounds.insert(id, sound.clone());
        Ok(sound)
    }

    fn evict(&self, id: i64) -> Result<(), String> {
        self.lock()?.remove(&id);
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<i64, SoundData>>, String> {
        self.0
            .lock()
            .map_err(|_| "the sound cache lock is poisoned".to_string())
    }
}

// Commands are async so Tauri runs them off the main thread; the synchronous work they wait on
// would otherwise freeze the window.

#[tauri::command]
pub async fn list_output_devices() -> Result<Vec<OutputDevice>, String> {
    output_devices::list()
}

#[tauri::command]
pub async fn set_output_devices(
    engine: State<'_, AudioEngine>,
    primary: Option<String>,
    secondary: Option<String>,
) -> Result<(), String> {
    engine.set_output_devices(primary, secondary)
}

#[tauri::command]
pub async fn list_sounds(library: State<'_, Library>) -> Result<Vec<Sound>, String> {
    library.list()
}

#[tauri::command]
pub async fn import_sounds(
    library: State<'_, Library>,
    paths: Vec<PathBuf>,
) -> Result<Vec<ImportResult>, String> {
    Ok(library.import(paths))
}

/// Plays a library sound at its saved volume. Shared by the command and by hotkeys.
pub fn play_library_sound(
    engine: &AudioEngine,
    library: &Library,
    cache: &SoundCache,
    id: i64,
) -> Result<(), String> {
    let volume = library.get(id)?.volume;
    engine.play(id, cache.load(library, id)?, volume)?;
    Ok(())
}

#[tauri::command]
pub async fn play_sound(
    engine: State<'_, AudioEngine>,
    library: State<'_, Library>,
    cache: State<'_, SoundCache>,
    id: i64,
) -> Result<(), String> {
    play_library_sound(&engine, &library, &cache, id)
}

#[tauri::command]
pub async fn stop_all(engine: State<'_, AudioEngine>) -> Result<(), String> {
    engine.stop_all()
}

#[tauri::command]
pub async fn rename_sound(
    library: State<'_, Library>,
    id: i64,
    name: String,
) -> Result<Sound, String> {
    library.rename(id, &name)
}

#[tauri::command]
pub async fn set_sound_volume(
    library: State<'_, Library>,
    id: i64,
    volume: f32,
) -> Result<Sound, String> {
    library.set_volume(id, volume)
}

#[tauri::command]
pub async fn set_sound_favorite(
    library: State<'_, Library>,
    id: i64,
    favorite: bool,
) -> Result<Sound, String> {
    library.set_favorite(id, favorite)
}

#[tauri::command]
pub async fn delete_sound(
    app: AppHandle,
    library: State<'_, Library>,
    cache: State<'_, SoundCache>,
    id: i64,
) -> Result<(), String> {
    let had_hotkey = library.get(id)?.hotkey.is_some();
    library.delete(id)?;
    cache.evict(id)?;
    // Release the deleted pad's shortcut so it's free for other apps and pads.
    if had_hotkey {
        hotkeys::register_all(&app)?;
    }
    Ok(())
}

/// Sets or clears a pad's hotkey. If the system refuses the new shortcut — another app may own
/// it — the previous hotkey is restored and the refusal is returned.
#[tauri::command]
pub async fn set_sound_hotkey(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    hotkey: Option<String>,
) -> Result<Sound, String> {
    let hotkey = hotkey.as_deref().map(hotkeys::normalize).transpose()?;
    let previous = library.get(id)?.hotkey;
    let sound = library.set_hotkey(id, hotkey.as_deref())?;
    register_or_revert(&app, hotkey.as_deref(), || {
        library.set_hotkey(id, previous.as_deref()).map(|_| ())
    })?;
    Ok(sound)
}

#[derive(serde::Serialize)]
pub struct AppHotkeys {
    stop_all: Option<String>,
    toggle_popover: Option<String>,
}

#[tauri::command]
pub async fn app_hotkeys(library: State<'_, Library>) -> Result<AppHotkeys, String> {
    Ok(AppHotkeys {
        stop_all: library.app_hotkey(AppShortcut::StopAll)?,
        toggle_popover: library.app_hotkey(AppShortcut::TogglePopover)?,
    })
}

#[tauri::command]
pub async fn set_app_hotkey(
    app: AppHandle,
    library: State<'_, Library>,
    shortcut: AppShortcut,
    hotkey: Option<String>,
) -> Result<Option<String>, String> {
    let hotkey = hotkey.as_deref().map(hotkeys::normalize).transpose()?;
    let previous = library.app_hotkey(shortcut)?;
    library.set_app_hotkey(shortcut, hotkey.as_deref())?;
    register_or_revert(&app, hotkey.as_deref(), || {
        library.set_app_hotkey(shortcut, previous.as_deref())
    })?;
    Ok(hotkey)
}

#[tauri::command]
pub async fn show_in_dock(library: State<'_, Library>) -> Result<bool, String> {
    library.show_in_dock()
}

/// Saves the preference and applies it now. A no-op outside macOS, which has no Dock.
#[tauri::command]
pub async fn set_show_in_dock(
    app: AppHandle,
    library: State<'_, Library>,
    show: bool,
) -> Result<(), String> {
    library.set_show_in_dock(show)?;
    apply_dock_visibility(&app, show)
}

pub fn apply_dock_visibility(app: &AppHandle, show: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    app.set_dock_visibility(show)
        .map_err(|error| format!("could not change the Dock icon: {error}"))?;
    #[cfg(not(target_os = "macos"))]
    let _ = (app, show);
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) {
    popover::show_main_window(&app);
}

#[tauri::command]
pub async fn hide_popover(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(popover::LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Saved hotkeys that aren't working because the system refused them, with why.
#[tauri::command]
pub async fn hotkey_failures(app: AppHandle) -> Result<HashMap<String, String>, String> {
    hotkeys::failures(&app)
}

fn register_or_revert(
    app: &AppHandle,
    hotkey: Option<&str>,
    revert: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    hotkeys::register_all(app)?;
    let Some(hotkey) = hotkey else { return Ok(()) };
    let Some(error) = hotkeys::failures(app)?.remove(hotkey) else {
        return Ok(());
    };
    revert()?;
    hotkeys::register_all(app)?;
    Err(format!(
        "the system refused that shortcut — another app may already use it ({error})"
    ))
}
