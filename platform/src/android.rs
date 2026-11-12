//! Android stub. A future implementation would use a foreground service with
//! an overlay window (SYSTEM_ALERT_WINDOW) for the bubble and the notification
//! channel API for alerts.

use tauri::{AppHandle, WebviewWindow};

use crate::{BubbleWindowOptions, Capabilities, NotificationSpec, PlatformError, PlatformServices, Result, TrayStyle};

pub struct AndroidPlatform;

impl PlatformServices for AndroidPlatform {
    fn name(&self) -> &'static str {
        "android"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            bubble_over_fullscreen: false,
            no_focus_bubble: false,
            tray: TrayStyle::SystemTray,
            native_sound: false,
            global_hotkey: false,
            autostart: false,
            dock_icon_toggle: false,
        }
    }

    fn configure_bubble(&self, _window: &WebviewWindow, _options: BubbleWindowOptions) -> Result<()> {
        Err(PlatformError::Unsupported("floating bubble window"))
    }

    fn show_bubble_without_focus(&self, _window: &WebviewWindow) -> Result<()> {
        Err(PlatformError::Unsupported("floating bubble window"))
    }

    fn set_dock_icon_visible(&self, _app: &AppHandle, _visible: bool) -> Result<()> {
        Ok(())
    }

    fn show_notification(&self, _app: &AppHandle, _spec: &NotificationSpec) -> Result<()> {
        Err(PlatformError::Unsupported("notifications"))
    }

    fn play_notification_sound(&self) -> Result<()> {
        Err(PlatformError::Unsupported("notification sound"))
    }

    fn reveal_file(&self, _path: &std::path::Path) -> Result<()> {
        Err(PlatformError::Unsupported("opening files"))
    }

    fn open_path(&self, _path: &std::path::Path) -> Result<()> {
        Err(PlatformError::Unsupported("opening files"))
    }

    fn set_autostart(&self, _app: &AppHandle, _enabled: bool) -> Result<()> {
        Err(PlatformError::Unsupported("autostart"))
    }

    fn autostart_enabled(&self, _app: &AppHandle) -> Result<bool> {
        Ok(false)
    }

    fn register_hotkey(&self, _app: &AppHandle, _accelerator: &str) -> Result<()> {
        Err(PlatformError::Unsupported("global hotkey"))
    }

    fn unregister_all_hotkeys(&self, _app: &AppHandle) -> Result<()> {
        Ok(())
    }
}
