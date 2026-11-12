//! Linux stub. X11/Wayland always-on-top support varies by compositor; the
//! basic implementation uses what Tauri exposes so the crate compiles and the
//! app runs, but the bubble may not float over fullscreen Wayland clients.

use tauri::{AppHandle, WebviewWindow};

use crate::{desktop, BubbleWindowOptions, Capabilities, NotificationSpec, PlatformError, PlatformServices, Result, TrayStyle};

pub struct LinuxPlatform;

impl PlatformServices for LinuxPlatform {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            bubble_over_fullscreen: false,
            no_focus_bubble: false,
            tray: TrayStyle::SystemTray,
            native_sound: false,
            global_hotkey: true,
            autostart: true,
            dock_icon_toggle: false,
        }
    }

    fn configure_bubble(&self, window: &WebviewWindow, _options: BubbleWindowOptions) -> Result<()> {
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
        Ok(())
    }

    fn show_bubble_without_focus(&self, window: &WebviewWindow) -> Result<()> {
        window.show().map_err(PlatformError::from)
    }

    fn set_dock_icon_visible(&self, _app: &AppHandle, _visible: bool) -> Result<()> {
        Ok(())
    }

    fn show_notification(&self, app: &AppHandle, spec: &NotificationSpec) -> Result<()> {
        desktop::show_notification(app, spec)
    }

    fn play_notification_sound(&self) -> Result<()> {
        Err(PlatformError::Unsupported("notification sound"))
    }

    fn reveal_file(&self, path: &std::path::Path) -> Result<()> {
        let parent = path.parent().unwrap_or(path);
        std::process::Command::new("xdg-open").arg(parent).spawn()?;
        Ok(())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<()> {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
        Ok(())
    }

    fn set_autostart(&self, app: &AppHandle, enabled: bool) -> Result<()> {
        desktop::set_autostart(app, enabled)
    }

    fn autostart_enabled(&self, app: &AppHandle) -> Result<bool> {
        desktop::autostart_enabled(app)
    }

    fn register_hotkey(&self, app: &AppHandle, accelerator: &str) -> Result<()> {
        desktop::register_hotkey(app, accelerator)
    }

    fn unregister_all_hotkeys(&self, app: &AppHandle) -> Result<()> {
        desktop::unregister_all_hotkeys(app)
    }
}
