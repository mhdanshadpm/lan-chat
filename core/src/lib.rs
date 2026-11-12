//! Platform-independent LAN Chat core.
//!
//! The crate contains no OS specific code and no Tauri dependency. It owns the
//! wire protocol, peer discovery, websocket connections, encryption, SQLite
//! history, group membership and file transfer. The UI layer (Tauri commands)
//! only calls into `Node` and listens to [`CoreEvent`]s.

pub mod config;
pub mod crypto;
pub mod discovery;
pub mod error;
pub mod group;
pub mod identity;
pub mod net;
pub mod protocol;
pub mod store;
pub mod transfer;

pub use config::{BubbleSettings, Corner, Settings, Theme};
pub use discovery::Announcement;
pub use error::{CoreError, Result};
pub use identity::Identity;
pub use net::{Node, NodeConfig};
pub use store::models::{CoreEvent, ConversationRecord, FileProgress, GroupRecord, MessageRecord, PeerSnapshot};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
