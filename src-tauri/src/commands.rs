use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::audio_engine::{AudioEngine, SoundData};
use crate::output_devices::{self, OutputDevice};

/// Encoded file contents by path, so replaying a sound never touches the disk. Temporary: the
/// library will own this once sounds are imported into app storage.
#[derive(Default)]
pub struct SoundCache(Mutex<HashMap<PathBuf, SoundData>>);

impl SoundCache {
    fn load(&self, path: PathBuf) -> Result<SoundData, String> {
        let mut sounds = self
            .0
            .lock()
            .map_err(|_| "the sound cache is poisoned".to_string())?;
        if let Some(sound) = sounds.get(&path) {
            return Ok(sound.clone());
        }
        let sound: SoundData = std::fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?
            .into();
        sounds.insert(path, sound.clone());
        Ok(sound)
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
pub async fn play_sound(
    engine: State<'_, AudioEngine>,
    cache: State<'_, SoundCache>,
    path: PathBuf,
    volume: f32,
) -> Result<(), String> {
    engine.play(cache.load(path)?, volume)
}

#[tauri::command]
pub async fn stop_all(engine: State<'_, AudioEngine>) -> Result<(), String> {
    engine.stop_all()
}
