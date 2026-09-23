mod audio_engine;
mod commands;
mod library;
mod output_devices;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            app.manage(library::Library::open(&data_directory)?);
            Ok(())
        })
        .manage(audio_engine::AudioEngine::start())
        .manage(commands::SoundCache::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_output_devices,
            commands::set_output_devices,
            commands::list_sounds,
            commands::import_sounds,
            commands::play_sound,
            commands::stop_all,
            commands::rename_sound,
            commands::set_sound_volume,
            commands::set_sound_favorite,
            commands::delete_sound,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
