mod audio_engine;
mod commands;
mod hotkeys;
mod library;
mod output_devices;
mod popover;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkeys::handle)
                .build(),
        )
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            app.manage(library::Library::open(&data_directory)?);

            let handle = app.handle().clone();
            app.manage(audio_engine::AudioEngine::start(move |event| {
                let _ = handle.emit("playback", event);
            }));

            app.manage(popover::PopoverState::default());
            popover::create(app.handle())?;
            popover::create_tray(app.handle())?;

            app.manage(hotkeys::Hotkeys::default());
            // A shortcut another app owns shouldn't stop the app from starting; the failures
            // are reported to the frontend instead.
            if let Err(error) = hotkeys::register_all(app.handle()) {
                eprintln!("could not register hotkeys: {error}");
            }
            Ok(())
        })
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
            commands::set_sound_hotkey,
            commands::app_hotkeys,
            commands::set_app_hotkey,
            commands::show_main_window,
            commands::hide_popover,
            commands::hotkey_failures,
        ])
        .on_window_event(|window, event| {
            // A menu bar app keeps running when its window closes; the tray brings it back.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
