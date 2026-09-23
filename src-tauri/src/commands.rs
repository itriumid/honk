use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::audio_engine::{AudioEngine, SoundData};
use crate::library::{ImportResult, Library, Sound};
use crate::output_devices::{self, OutputDevice};

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

/// Plays a library sound at its saved volume.
#[tauri::command]
pub async fn play_sound(
    engine: State<'_, AudioEngine>,
    library: State<'_, Library>,
    cache: State<'_, SoundCache>,
    id: i64,
) -> Result<(), String> {
    let volume = library.get(id)?.volume;
    engine.play(cache.load(&library, id)?, volume)
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
    library: State<'_, Library>,
    cache: State<'_, SoundCache>,
    id: i64,
) -> Result<(), String> {
    library.delete(id)?;
    cache.evict(id)
}
