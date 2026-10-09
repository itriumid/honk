mod audio_engine;
mod commands;
mod data_directory;
mod hotkeys;
mod library;
mod output_devices;
mod popover;
mod sharing;

use tauri::{Emitter, Manager};

/// The panel plugin on macOS; an empty plugin elsewhere, so the builder chain stays the same.
fn panel_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    #[cfg(target_os = "macos")]
    return tauri_nspanel::init();
    #[cfg(not(target_os = "macos"))]
    return tauri::plugin::Builder::new("nspanel").build();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(panel_plugin())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkeys::handle)
                .build(),
        )
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            // Failing to move an old library shouldn't stop the app; it starts with an empty
            // one and says why, and the old directory stays where it was.
            if let Err(error) = data_directory::adopt_previous(&data_directory) {
                eprintln!("could not move the library from an older version: {error}");
            }
            app.manage(library::Library::open(&data_directory)?);

            let handle = app.handle().clone();
            app.manage(audio_engine::AudioEngine::start(move |event| {
                let _ = handle.emit("playback", event);
            }));

            let show_in_dock = app.state::<library::Library>().show_in_dock()?;
            commands::apply_dock_visibility(app.handle(), show_in_dock)?;

            app.manage(popover::PopoverState::default());
            popover::create(app.handle())?;
            popover::create_tray(app.handle())?;
            let show_in_menu_bar = app.state::<library::Library>().show_in_menu_bar()?;
            popover::set_tray_visible(app.handle(), show_in_menu_bar);

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
            commands::reorder_sounds,
            commands::set_sound_category,
            commands::list_categories,
            commands::create_category,
            commands::rename_category,
            commands::delete_category,
            commands::export_library,
            commands::preview_library_file,
            commands::import_library_file,
            commands::delete_sound,
            commands::set_sound_hotkey,
            commands::app_hotkeys,
            commands::set_app_hotkey,
            commands::presence,
            commands::set_show_in_dock,
            commands::set_show_in_menu_bar,
            commands::quit,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Opening Honk again from Finder or Spotlight brings its window back, so hiding both
            // icons never locks anyone out.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                popover::show_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
