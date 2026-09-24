//! The menu bar popover: a small borderless window that drops down from the tray icon, the way
//! system menu bar apps do, and hides again as soon as it loses focus.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Rect, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

#[cfg(not(target_os = "macos"))]
use tauri::WindowEvent;

pub const LABEL: &str = "popover";
const WIDTH: f64 = 320.0;
const HEIGHT: f64 = 440.0;
/// Logical pixels between the menu bar and the popover.
const GAP: f64 = 6.0;

/// A tray click that lands this soon after the popover hid itself on blur is the same click
/// that caused the blur — the user clicking the icon to close it.
const CLICK_AFTER_BLUR: Duration = Duration::from_millis(300);

#[derive(Default)]
pub struct PopoverState {
    /// Where the tray icon was last clicked, so opening from a hotkey drops down from there.
    anchor: Mutex<Option<Rect>>,
    hidden_on_blur_at: Mutex<Option<Instant>>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover".into()))
        .title("Honk")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .state(EffectState::Active)
                .radius(12.0)
                .build(),
        )
        .shadow(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;

    let app_handle = app.clone();
    let hide_on_blur = move || {
        if app_handle
            .get_webview_window(LABEL)
            .is_some_and(|window| window.is_visible().unwrap_or(false))
        {
            hide(&app_handle);
            if let Ok(mut hidden) = app_handle.state::<PopoverState>().hidden_on_blur_at.lock() {
                *hidden = Some(Instant::now());
            }
        }
    };

    #[cfg(target_os = "macos")]
    panel::convert(&window, hide_on_blur)?;
    #[cfg(not(target_os = "macos"))]
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide_on_blur();
        }
    });
    Ok(())
}

/// On macOS the popover is an `NSPanel` that shows and takes keyboard focus *without*
/// activating Honk. An ordinary window can't: activating Honk pulls the user off a full-screen
/// app's Space, and macOS won't put a background app's ordinary window on that Space at all.
#[cfg(target_os = "macos")]
#[allow(
    clippy::unused_unit,
    reason = "tauri_panel!'s panel_event! syntax requires an explicit `-> ()` on every event"
)]
mod panel {
    use tauri::WebviewWindow;
    use tauri_nspanel::{
        tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
    };

    tauri_panel! {
        panel!(PopoverPanel {
            config: {
                can_become_key_window: true,
                is_floating_panel: true
            }
        })

        panel_event!(PopoverPanelEvents {
            window_did_resign_key(notification: &NSNotification) -> ()
        })
    }

    /// Replaces the window's delegate, so Tauri's own focus events stop for this window —
    /// losing focus is reported through `on_blur` instead.
    pub fn convert(window: &WebviewWindow, on_blur: impl Fn() + 'static) -> tauri::Result<()> {
        let panel = window.to_panel::<PopoverPanel>()?;
        panel
            .set_style_mask(StyleMask::empty().nonactivating_panel().into())
            .map_err(|error| {
                tauri::Error::Io(std::io::Error::other(format!(
                    "could not make the popover a non-activating panel: {error:?}"
                )))
            })?;
        panel.set_collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .into(),
        );
        panel.set_level(PanelLevel::PopUpMenu.value());

        let events = PopoverPanelEvents::new();
        events.window_did_resign_key(move |_| on_blur());
        panel.set_event_handler(Some(events.as_ref()));
        Ok(())
    }

    pub fn show(app: &tauri::AppHandle) {
        on_main_thread(app, |app| {
            if let Ok(panel) = app.get_webview_panel(super::LABEL) {
                panel.show_and_make_key();
            }
        });
    }

    pub fn hide(app: &tauri::AppHandle) {
        on_main_thread(app, |app| {
            if let Ok(panel) = app.get_webview_panel(super::LABEL) {
                panel.hide();
            }
        });
    }

    /// AppKit aborts the app if a panel is touched off the main thread. Tauri's own window calls
    /// hop there by themselves, but tauri-nspanel's don't, and async commands run on a worker
    /// thread. On the main thread already, `work` runs straight away rather than being queued.
    fn on_main_thread(
        app: &tauri::AppHandle,
        work: impl FnOnce(&tauri::AppHandle) + Send + 'static,
    ) {
        let handle = app.clone();
        if let Err(error) = app.run_on_main_thread(move || work(&handle)) {
            eprintln!("could not reach the main thread for the popover: {error}");
        }
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Honk", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Honk", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

    TrayIconBuilder::with_id("honk")
        .icon(Image::from_bytes(include_bytes!(
            "../icons/tray-template@2x.png"
        ))?)
        .icon_as_template(true)
        .tooltip("Honk")
        .menu(&menu)
        // Left click opens the popover; the menu is for right click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                let state = app.state::<PopoverState>();
                if let Ok(mut anchor) = state.anchor.lock() {
                    *anchor = Some(rect);
                }
                let just_hidden = state
                    .hidden_on_blur_at
                    .lock()
                    .ok()
                    .and_then(|mut hidden| hidden.take())
                    .is_some_and(|at| at.elapsed() < CLICK_AFTER_BLUR);
                if !just_hidden {
                    toggle(app);
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Shows the popover under the tray icon, or hides it if it's already showing.
pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }
    let anchor = app
        .state::<PopoverState>()
        .anchor
        .lock()
        .ok()
        .and_then(|anchor| *anchor);
    if let Some(position) = position_for(&window, anchor) {
        let _ = window.set_position(position);
    }
    #[cfg(target_os = "macos")]
    panel::show(app);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
    // Focus events don't reach the popover page on macOS (see `panel::convert`), so it's told
    // directly — to refresh and focus the search field.
    let _ = app.emit_to(LABEL, "popover-shown", ());
}

pub fn hide(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    panel::hide(app);
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

pub fn show_main_window(app: &AppHandle) {
    hide(app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

/// Where the popover goes, in macOS's global points. Everything is worked out in points
/// because displays can differ in scale, and a physical position would be converted using the
/// scale of whichever display the popover happens to be on now — not the one it's going to.
fn position_for(window: &WebviewWindow, anchor: Option<Rect>) -> Option<LogicalPosition<f64>> {
    let monitors = window.available_monitors().ok()?;
    let displays: Vec<Display> = monitors
        .iter()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            Display {
                screen: Screen {
                    left: monitor.position().x as f64 / scale,
                    top: monitor.position().y as f64 / scale,
                    width: monitor.size().width as f64 / scale,
                },
                height: monitor.size().height as f64 / scale,
                scale,
            }
        })
        .collect();
    let primary = window.primary_monitor().ok().flatten()?;
    let primary_index = monitors
        .iter()
        .position(|monitor| monitor.position() == primary.position())
        .unwrap_or(0);

    let icon = anchor.map(|rect| {
        // The tray reports points multiplied by its own display's scale.
        let position = rect.position.to_physical::<f64>(1.0);
        let size = rect.size.to_physical::<f64>(1.0);
        (position.x, position.y, size.width, size.height)
    });
    let index = icon
        .and_then(|icon| display_holding(icon, &displays))
        .unwrap_or(primary_index);
    let display = displays.get(index)?;
    let icon_in_points = icon.map(|(x, y, width, height)| {
        let scale = display.scale;
        (x / scale, y / scale, width / scale, height / scale)
    });
    let (x, y) = place(icon_in_points, display.screen, 1.0);
    Some(LogicalPosition::new(x, y))
}

/// A display in global points, with the scale it reports physical pixels at.
#[derive(Clone, Copy)]
struct Display {
    screen: Screen,
    height: f64,
    scale: f64,
}

/// Which display the tray icon is on. Its rect is in points times *its own* display's scale,
/// so each display is tested at its own scale: the right one is where the result lands inside.
fn display_holding(icon: (f64, f64, f64, f64), displays: &[Display]) -> Option<usize> {
    let (x, y, width, height) = icon;
    displays.iter().position(|display| {
        let centre_x = (x + width / 2.0) / display.scale;
        let centre_y = (y + height / 2.0) / display.scale;
        centre_x >= display.screen.left
            && centre_x < display.screen.left + display.screen.width
            && centre_y >= display.screen.top
            && centre_y < display.screen.top + display.height
    })
}

#[derive(Clone, Copy)]
struct Screen {
    left: f64,
    top: f64,
    width: f64,
}

/// Centred under the tray icon (`x, y, width, height`, physical) when there is one, otherwise at
/// the top right of the screen — where the menu bar icons live — and always kept on screen.
fn place(icon: Option<(f64, f64, f64, f64)>, screen: Screen, scale: f64) -> (f64, f64) {
    let width = WIDTH * scale;
    let gap = GAP * scale;
    let (x, y) = match icon {
        Some((x, y, icon_width, icon_height)) => {
            (x + icon_width / 2.0 - width / 2.0, y + icon_height + gap)
        }
        None => (
            screen.left + screen.width - width - 16.0 * scale,
            screen.top + 24.0 * scale + gap,
        ),
    };
    let x = x.clamp(screen.left + gap, screen.left + screen.width - width - gap);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETINA: Screen = Screen {
        left: 0.0,
        top: 0.0,
        width: 2880.0,
    };

    #[test]
    fn centres_under_the_tray_icon() {
        // A 44x48 physical icon at x=2000 on a 2x screen: its centre is x=2022.
        let (x, y) = place(Some((2000.0, 0.0, 44.0, 48.0)), RETINA, 2.0);
        assert_eq!(x, 2022.0 - 320.0);
        assert_eq!(y, 48.0 + 12.0);
    }

    #[test]
    fn stays_on_screen_when_the_icon_is_near_the_edge() {
        let (x, _) = place(Some((2860.0, 0.0, 44.0, 48.0)), RETINA, 2.0);
        assert_eq!(x, 2880.0 - 640.0 - 12.0);
        let (x, _) = place(Some((0.0, 0.0, 44.0, 48.0)), RETINA, 2.0);
        assert_eq!(x, 12.0);
    }

    #[test]
    fn without_a_click_opens_at_the_top_right() {
        let (x, y) = place(None, RETINA, 2.0);
        assert_eq!(x, 2880.0 - 640.0 - 32.0);
        assert_eq!(y, 48.0 + 12.0);
    }

    /// A 2x built-in display at the origin, and a 1x external one to its left, in points.
    fn two_displays() -> [Display; 2] {
        [
            Display {
                screen: Screen {
                    left: 0.0,
                    top: 0.0,
                    width: 1440.0,
                },
                height: 900.0,
                scale: 2.0,
            },
            Display {
                screen: Screen {
                    left: -1920.0,
                    top: 0.0,
                    width: 1920.0,
                },
                height: 1080.0,
                scale: 1.0,
            },
        ]
    }

    #[test]
    fn finds_the_external_display_when_its_menu_bar_was_clicked() {
        // An icon at x = -300 points on the 1x display arrives as -300 physical.
        assert_eq!(
            display_holding((-300.0, 0.0, 22.0, 24.0), &two_displays()),
            Some(1)
        );
    }

    #[test]
    fn finds_the_built_in_display_despite_its_doubled_coordinates() {
        // An icon at x = 1200 points on the 2x display arrives as 2400 physical — outside the
        // display if taken as points, which is the bug this guards against.
        assert_eq!(
            display_holding((2400.0, 0.0, 44.0, 48.0), &two_displays()),
            Some(0)
        );
    }

    #[test]
    fn works_on_a_secondary_arrangement_offset_from_the_origin() {
        let screen = Screen {
            left: -1920.0,
            top: 0.0,
            width: 1920.0,
        };
        let (x, _) = place(Some((-100.0, 0.0, 22.0, 24.0)), screen, 1.0);
        assert_eq!(x, -1920.0 + 1920.0 - 320.0 - 6.0);
    }
}
