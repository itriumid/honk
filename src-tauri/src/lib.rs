mod audio_engine;
mod commands;
mod library;
mod output_devices;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(audio_engine::AudioEngine::start())
        .manage(commands::SoundCache::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_output_devices,
            commands::set_output_devices,
            commands::play_sound,
            commands::stop_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
