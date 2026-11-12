//! Platform services for the bubble window, tray/menu bar, hotkeys,
//! notifications, sound and autostart.
//!
//! This is the **only** crate where `#[cfg(target_os = "...")]` is allowed.
//! The `core` crate is deliberately OS-agnostic; the Tauri app only calls
//! through the [`PlatformServices`] trait.

#[allow(dead_code)]
mod android;
mod desktop;
#[allow(dead_code)]
mod ios;
mod linux;
mod macos;
#[allow(dead_code)]
mod windows;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("not supported on this platform: {0}")]
    Unsupported(&'static str),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, PlatformError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayStyle {
    /// Windows notification area / Linux StatusNotifier.
    SystemTray,
    /// macOS menu bar extra.
    MenuBar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Capabilities {
    /// Bubble can join every Space and float over fullscreen apps.
    pub bubble_over_fullscreen: bool,
    /// Window can be made topmost without stealing focus.
    pub no_focus_bubble: bool,
    pub tray: TrayStyle,
    pub native_sound: bool,
    pub global_hotkey: bool,
    pub autostart: bool,
    /// App can switch between Dock icon and accessory (menu bar only) mode.
    pub dock_icon_toggle: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationSpec {
    pub title: String,
    pub body: String,
    /// Conversation id, so the app can route a click (future).
    pub conv: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct BubbleWindowOptions {
    pub opacity: f32,
    pub collapsed: bool,
    pub no_focus: bool,
}

/// Everything OS-specific the app needs. Implementations live in the sibling
/// modules for each platform.
pub trait PlatformServices: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    /// Applies always-on-top, fullscreen/Spaces behaviour and focus rules to
    /// the bubble window. Safe to call every time the bubble is shown.
    fn configure_bubble(
        &self,
        window: &tauri::WebviewWindow,
        options: BubbleWindowOptions,
    ) -> Result<()>;

    /// Shows the bubble without activating the app or stealing keyboard focus.
    fn show_bubble_without_focus(&self, window: &tauri::WebviewWindow) -> Result<()>;

    /// macOS: hide the Dock icon while only the bubble is visible.
    fn set_dock_icon_visible(&self, app: &tauri::AppHandle, visible: bool) -> Result<()>;

    fn show_notification(&self, app: &tauri::AppHandle, spec: &NotificationSpec) -> Result<()>;

    fn play_notification_sound(&self) -> Result<()>;

    /// Opens the containing folder and selects the file.
    fn reveal_file(&self, path: &std::path::Path) -> Result<()>;

    /// Opens the file with the user's default application.
    fn open_path(&self, path: &std::path::Path) -> Result<()>;

    fn set_autostart(&self, app: &tauri::AppHandle, enabled: bool) -> Result<()>;

    fn autostart_enabled(&self, app: &tauri::AppHandle) -> Result<bool>;

    fn register_hotkey(&self, app: &tauri::AppHandle, accelerator: &str) -> Result<()>;

    fn unregister_all_hotkeys(&self, app: &tauri::AppHandle) -> Result<()>;
}

pub fn current() -> &'static dyn PlatformServices {
    #[cfg(target_os = "windows")]
    {
        return &windows::WindowsPlatform;
    }
    #[cfg(target_os = "macos")]
    {
        return &macos::MacOsPlatform;
    }
    #[cfg(target_os = "linux")]
    {
        return &linux::LinuxPlatform;
    }
    #[cfg(target_os = "android")]
    {
        return &android::AndroidPlatform;
    }
    #[cfg(target_os = "ios")]
    {
        return &ios::IosPlatform;
    }
    #[allow(unreachable_code)]
    {
        &linux::LinuxPlatform
    }
}
