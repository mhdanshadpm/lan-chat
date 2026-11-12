use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    #[default]
    BottomRight,
    BottomLeft,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BubbleSettings {
    pub enabled: bool,
    /// Seconds of inactivity before the bubble hides itself.
    pub auto_hide_secs: u64,
    pub corner: Corner,
    /// 0.55 ..= 1.0
    pub opacity: f32,
    pub collapsed: bool,
    /// Remembered top-left position in physical pixels. Overrides `corner`.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub show_on_every_message: bool,
}

impl Default for BubbleSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_hide_secs: 8,
            corner: Corner::BottomRight,
            opacity: 0.96,
            collapsed: false,
            x: None,
            y: None,
            show_on_every_message: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub bubble: BubbleSettings,
    /// Shared LAN passphrase. `None` means frames travel in plaintext on the
    /// local network. Stored in the local SQLite settings table.
    pub passphrase: Option<String>,
    pub dnd: bool,
    pub notification_sound: bool,
    pub launch_at_login: bool,
    /// Incoming files at or below this size are accepted without prompting.
    pub auto_accept_files_mb: u64,
    /// Incoming files at or below this size are downloaded automatically even
    /// when larger than the auto-accept threshold? (kept for UI parity)
    pub auto_download_files_mb: u64,
    pub max_file_mb: u64,
    pub discovery_port: u16,
    pub read_receipts: bool,
    pub typing_indicators: bool,
    /// Hide the bubble while the main window has focus.
    pub pause_bubble_when_app_focused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            bubble: BubbleSettings::default(),
            passphrase: None,
            dnd: false,
            notification_sound: true,
            launch_at_login: false,
            auto_accept_files_mb: 10,
            auto_download_files_mb: 25,
            max_file_mb: 2048,
            discovery_port: crate::protocol::DEFAULT_DISCOVERY_PORT,
            read_receipts: true,
            typing_indicators: true,
            pause_bubble_when_app_focused: true,
        }
    }
}

impl Settings {
    pub fn auto_accept_bytes(&self) -> u64 {
        self.auto_accept_files_mb.saturating_mul(1024 * 1024)
    }

    pub fn max_file_bytes(&self) -> u64 {
        self.max_file_mb.saturating_mul(1024 * 1024)
    }
}
