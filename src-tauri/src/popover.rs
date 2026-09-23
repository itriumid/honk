//! The menu bar popover: a small borderless window that drops down from the tray icon, the way
//! system menu bar apps do, and hides again as soon as it loses focus.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{
    AppHandle, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

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

    let hide_on_blur = window.clone();
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if hide_on_blur.is_visible().unwrap_or(false) {
                let _ = hide_on_blur.hide();
                if let Ok(mut hidden) = app_handle.state::<PopoverState>().hidden_on_blur_at.lock()
                {
                    *hidden = Some(Instant::now());
                }
            }
        }
    });
    Ok(())
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
        let _ = window.hide();
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
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(popover) = app.get_webview_window(LABEL) {
        let _ = popover.hide();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

/// Where the popover goes on the primary monitor, in physical pixels.
fn position_for(window: &WebviewWindow, anchor: Option<Rect>) -> Option<PhysicalPosition<i32>> {
    let monitor = window.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let screen = Screen {
        left: monitor.position().x as f64,
        top: monitor.position().y as f64,
        width: monitor.size().width as f64,
    };
    let icon = anchor.map(|rect| {
        let position = rect.position.to_physical::<f64>(scale);
        let size = rect.size.to_physical::<f64>(scale);
        (position.x, position.y, size.width, size.height)
    });
    let (x, y) = place(icon, screen, scale);
    Some(PhysicalPosition::new(x.round() as i32, y.round() as i32))
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
