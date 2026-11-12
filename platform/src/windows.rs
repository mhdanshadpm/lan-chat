use std::ffi::c_void;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{AppHandle, WebviewWindow};

use crate::{desktop, BubbleWindowOptions, Capabilities, NotificationSpec, PlatformError, PlatformServices, Result, TrayStyle};

extern "C" {
    fn lanchat_win_make_topmost(hwnd: *mut c_void);
    fn lanchat_win_show_without_focus(hwnd: *mut c_void);
    fn lanchat_win_beep();
}

pub struct WindowsPlatform;

fn hwnd(window: &WebviewWindow) -> Result<*mut c_void> {
    let handle = window
        .window_handle()
        .map_err(|e| PlatformError::Message(format!("window handle: {e}")))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(h.hwnd.get() as *mut c_void),
        _ => Err(PlatformError::Message("not a Win32 window".into())),
    }
}

impl PlatformServices for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            bubble_over_fullscreen: true,
            no_focus_bubble: true,
            tray: TrayStyle::SystemTray,
            native_sound: true,
            global_hotkey: true,
            autostart: true,
            dock_icon_toggle: false,
        }
    }

    fn configure_bubble(&self, window: &WebviewWindow, options: BubbleWindowOptions) -> Result<()> {
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
        let _ = options.opacity;
        let handle = hwnd(window)?;
        unsafe { lanchat_win_make_topmost(handle) };
        Ok(())
    }

    fn show_bubble_without_focus(&self, window: &WebviewWindow) -> Result<()> {
        let handle = hwnd(window)?;
        unsafe { lanchat_win_show_without_focus(handle) };
        Ok(())
    }

    fn set_dock_icon_visible(&self, _app: &AppHandle, _visible: bool) -> Result<()> {
        Ok(())
    }

    fn show_notification(&self, app: &AppHandle, spec: &NotificationSpec) -> Result<()> {
        desktop::show_notification(app, spec)
    }

    fn play_notification_sound(&self) -> Result<()> {
        unsafe { lanchat_win_beep() };
        Ok(())
    }

    fn reveal_file(&self, path: &std::path::Path) -> Result<()> {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()?;
        Ok(())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<()> {
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?;
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
