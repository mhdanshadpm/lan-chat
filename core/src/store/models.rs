use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::protocol::{ConvKind, DeliveryState, MessageKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerSnapshot {
    pub id: Uuid,
    pub display_name: String,
    pub avatar_color: u32,
    pub device_name: String,
    pub online: bool,
    pub last_seen: i64,
    pub addr: Option<String>,
    pub port: Option<u16>,
    pub secure: bool,
}

impl PeerSnapshot {
    pub fn placeholder(id: Uuid) -> Self {
        Self {
            id,
            display_name: String::new(),
            avatar_color: 0x9A_A6_B4,
            device_name: String::new(),
            online: false,
            last_seen: 0,
            addr: None,
            port: None,
            secure: false,
        }
    }

    pub fn label(&self) -> String {
        if self.display_name.trim().is_empty() {
            self.id.to_string()[..8].to_string()
        } else {
            self.display_name.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: Uuid,
    pub conv: Uuid,
    pub conv_kind: ConvKind,
    pub sender_id: Uuid,
    pub sender_name: String,
    pub kind: MessageKind,
    pub text: Option<String>,
    pub file_id: Option<Uuid>,
    pub reply_to: Option<Uuid>,
    pub mentions: Vec<Uuid>,
    pub ts: i64,
    pub state: DeliveryState,
    pub outgoing: bool,
}

impl MessageRecord {
    pub fn preview(&self) -> String {
        if let Some(text) = &self.text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.chars().take(140).collect();
            }
        }
        if self.kind.is_attachment() {
            return self.kind.preview().to_string();
        }
        String::new()
    }

    pub fn mentions_me(&self, me: Uuid) -> bool {
        self.mentions.contains(&me)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub id: Uuid,
    pub kind: ConvKind,
    pub title: String,
    pub avatar_color: u32,
    pub unread: u32,
    pub muted: bool,
    pub last_ts: i64,
    pub last_preview: String,
    pub last_sender: String,
    pub mentioned: bool,
    pub online: Option<bool>,
    pub member_count: u32,
    pub is_everyone: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: Uuid,
    pub name: String,
    pub avatar_color: u32,
    pub members: Vec<Uuid>,
    pub created_by: Uuid,
    pub created_at: i64,
    pub is_default: bool,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileDirection {
    Incoming,
    Outgoing,
}

impl FileDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            FileDirection::Incoming => "incoming",
            FileDirection::Outgoing => "outgoing",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "outgoing" {
            FileDirection::Outgoing
        } else {
            FileDirection::Incoming
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// Incoming offer waiting for the user to accept.
    PendingOffer,
    /// Outgoing offer waiting for the peer to accept.
    AwaitingAccept,
    Downloading,
    Uploading,
    Paused,
    Done,
    Rejected,
    Failed,
}

impl FileState {
    pub fn as_str(self) -> &'static str {
        match self {
            FileState::PendingOffer => "pending_offer",
            FileState::AwaitingAccept => "awaiting_accept",
            FileState::Downloading => "downloading",
            FileState::Uploading => "uploading",
            FileState::Paused => "paused",
            FileState::Done => "done",
            FileState::Rejected => "rejected",
            FileState::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "pending_offer" => FileState::PendingOffer,
            "awaiting_accept" => FileState::AwaitingAccept,
            "downloading" => FileState::Downloading,
            "uploading" => FileState::Uploading,
            "paused" => FileState::Paused,
            "done" => FileState::Done,
            "rejected" => FileState::Rejected,
            _ => FileState::Failed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: Uuid,
    pub message_id: Option<Uuid>,
    pub conv: Uuid,
    pub peer_id: Option<Uuid>,
    pub direction: FileDirection,
    pub name: String,
    pub path: Option<PathBuf>,
    pub thumb_path: Option<PathBuf>,
    pub mime: String,
    pub size: u64,
    pub received: u64,
    pub sha256: String,
    pub state: FileState,
    pub next_index: u32,
    pub chunk_size: u32,
    pub chunks: u32,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProgress {
    pub transfer_id: Uuid,
    pub conv: Uuid,
    pub direction: FileDirection,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub received: u64,
    pub state: FileState,
    pub path: Option<PathBuf>,
    pub thumb_path: Option<PathBuf>,
    pub peer_id: Option<Uuid>,
}

impl From<&FileRecord> for FileProgress {
    fn from(f: &FileRecord) -> Self {
        Self {
            transfer_id: f.id,
            conv: f.conv,
            direction: f.direction,
            name: f.name.clone(),
            mime: f.mime.clone(),
            size: f.size,
            received: f.received,
            state: f.state,
            path: f.path.clone(),
            thumb_path: f.thumb_path.clone(),
            peer_id: f.peer_id,
        }
    }
}

/// Events emitted by the core for the UI layer. The core never knows about
/// Tauri; the app crate forwards these to webviews.
#[derive(Clone, Debug)]
pub enum CoreEvent {
    PeerUpdated(PeerSnapshot),
    PeerOffline(Uuid),
    Message(MessageRecord),
    MessageState {
        conv: Uuid,
        ids: Vec<Uuid>,
        state: DeliveryState,
    },
    Typing {
        peer: Uuid,
        peer_name: String,
        conv: Uuid,
        active: bool,
    },
    GroupUpdated(GroupRecord),
    ConversationUpdated(ConversationRecord),
    Transfer(FileProgress),
    Error {
        context: String,
        message: String,
    },
    /// Network interfaces changed (new IP, Wi-Fi switch, cable plugged in).
    NetworkChanged,
    /// Our WebSocket server is up on this port.
    Listening(u16),
}
