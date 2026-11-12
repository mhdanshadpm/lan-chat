//! Tray (Windows) / menu bar (macOS) icon with Open, Mute and Quit.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::bubble;
use crate::events;
use crate::state::AppState;

pub const TRAY_ID: &str = "lanchat-tray";
pub const TRAY_ACCELERATOR: &str = "CmdOrCtrl+Shift+Space";

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let handle = app.handle();

    let open = MenuItem::with_id(handle, "open", "Open LAN Chat", true, None::<&str>)?;
    let mute = MenuItem::with_id(handle, "mute", "Mute / unmute notifications", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quit LAN Chat", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&open, &mute, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("LAN Chat")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => bubble::open_main(app),
            "mute" => toggle_mute(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                bubble::open_main(tray.app_handle());
            }
        });

    if let Some(icon) = handle.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(handle)?;
    Ok(())
}

fn toggle_mute(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut settings = state.node.settings().await;
        settings.dnd = !settings.dnd;
        match state.node.update_settings(settings.clone()).await {
            Ok(saved) => events::emit_settings(&app, &saved),
            Err(error) => tracing::warn!(%error, "could not toggle DND"),
        }
    });
}

pub fn register_hotkey(app: &AppHandle) {
    if let Err(error) = lanchat_platform::current().register_hotkey(app, TRAY_ACCELERATOR) {
        tracing::warn!(%error, "global hotkey unavailable");
    }
}

pub fn cleanup_hotkeys(app: &AppHandle) {
    if let Err(error) = lanchat_platform::current().unregister_all_hotkeys(app) {
        tracing::debug!(%error, "could not unregister hotkeys");
    }
}
