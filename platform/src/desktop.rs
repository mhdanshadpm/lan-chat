//! Helpers shared by the desktop implementations (Windows, macOS, Linux).
//! These wrap the Tauri plugins so each platform module stays small.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::GlobalShortcutExt as _;
use tauri_plugin_notification::NotificationExt as _;

use crate::{NotificationSpec, PlatformError, Result};

pub fn show_notification(app: &AppHandle, spec: &NotificationSpec) -> Result<()> {
    app.notification()
        .builder()
        .title(&spec.title)
        .body(&spec.body)
        .show()
        .map_err(|e| PlatformError::Message(format!("notification: {e}")))
}

pub fn set_autostart(app: &AppHandle, enabled: bool) -> Result<()> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| PlatformError::Message(format!("autostart: {e}")))
}

pub fn autostart_enabled(app: &AppHandle) -> Result<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| PlatformError::Message(format!("autostart: {e}")))
}

pub fn register_hotkey(app: &AppHandle, accelerator: &str) -> Result<()> {
    app.global_shortcut()
        .register(accelerator)
        .map_err(|e| PlatformError::Message(format!("hotkey {accelerator}: {e}")))
}

pub fn unregister_all_hotkeys(app: &AppHandle) -> Result<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| PlatformError::Message(format!("hotkey: {e}")))
}
