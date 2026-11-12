use std::ffi::c_void;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{AppHandle, WebviewWindow};

use crate::{desktop, BubbleWindowOptions, Capabilities, NotificationSpec, PlatformError, PlatformServices, Result, TrayStyle};

extern "C" {
    fn lanchat_macos_configure_bubble(ns_view: *mut c_void, no_focus: i32);
    fn lanchat_macos_show_without_focus(ns_view: *mut c_void);
    fn lanchat_macos_set_dock_visible(visible: i32);
}

pub struct MacOsPlatform;

fn ns_view(window: &WebviewWindow) -> Result<*mut c_void> {
    let handle = window
        .window_handle()
        .map_err(|e| PlatformError::Message(format!("window handle: {e}")))?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Ok(h.ns_view.as_ptr() as *mut c_void),
        _ => Err(PlatformError::Message("not an AppKit window".into())),
    }
}

impl PlatformServices for MacOsPlatform {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            bubble_over_fullscreen: true,
            no_focus_bubble: true,
            tray: TrayStyle::MenuBar,
            native_sound: true,
            global_hotkey: true,
            autostart: true,
            dock_icon_toggle: true,
        }
    }

    fn configure_bubble(&self, window: &WebviewWindow, options: BubbleWindowOptions) -> Result<()> {
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
        let _ = window.set_visible_on_all_workspaces(true);
        let view = ns_view(window)?;
        unsafe { lanchat_macos_configure_bubble(view, i32::from(options.no_focus)) };
        Ok(())
    }

    fn show_bubble_without_focus(&self, window: &WebviewWindow) -> Result<()> {
        let view = ns_view(window)?;
        unsafe { lanchat_macos_show_without_focus(view) };
        Ok(())
    }

    fn set_dock_icon_visible(&self, _app: &AppHandle, visible: bool) -> Result<()> {
        unsafe { lanchat_macos_set_dock_visible(i32::from(visible)) };
        Ok(())
    }

    fn show_notification(&self, app: &AppHandle, spec: &NotificationSpec) -> Result<()> {
        desktop::show_notification(app, spec)
    }

    fn play_notification_sound(&self) -> Result<()> {
        std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn()
            .map_err(PlatformError::from)?;
        Ok(())
    }

    fn reveal_file(&self, path: &std::path::Path) -> Result<()> {
        std::process::Command::new("open").arg("-R").arg(path).spawn()?;
        Ok(())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<()> {
        std::process::Command::new("open").arg(path).spawn()?;
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
