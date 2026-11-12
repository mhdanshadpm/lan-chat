//! Frame bodies. All types are additive-friendly: unknown fields are ignored
//! by serde and new optional fields default when absent.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::Envelope;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Hello(Hello),
    HelloAck(HelloAck),
    Presence(Presence),
    Message(ChatMessage),
    Ack(Ack),
    Typing(Typing),
    GroupCreate(GroupCreate),
    GroupUpdate(GroupUpdate),
    FileOffer(FileOffer),
    FileChunk(FileChunk),
    FileAck(FileAck),
    /// Wrapper used only after a successful passphrase handshake. The
    /// plaintext is another complete `Envelope` (also carrying `"v": 1`).
    Sealed(Sealed),
    Error(ErrorFrame),
}

impl Body {
    pub fn kind(&self) -> &'static str {
        match self {
            Body::Hello(_) => "hello",
            Body::HelloAck(_) => "hello_ack",
            Body::Presence(_) => "presence",
            Body::Message(_) => "message",
            Body::Ack(_) => "ack",
            Body::Typing(_) => "typing",
            Body::GroupCreate(_) => "group_create",
            Body::GroupUpdate(_) => "group_update",
            Body::FileOffer(_) => "file_offer",
            Body::FileChunk(_) => "file_chunk",
            Body::FileAck(_) => "file_ack",
            Body::Sealed(_) => "sealed",
            Body::Error(_) => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvKind {
    Direct,
    Group,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    Image,
    Video,
    Audio,
    File,
    System,
}

impl MessageKind {
    pub fn is_attachment(self) -> bool {
        matches!(
            self,
            MessageKind::Image | MessageKind::Video | MessageKind::Audio | MessageKind::File
        )
    }

    pub fn preview(self) -> &'static str {
        match self {
            MessageKind::Text | MessageKind::System => "",
            MessageKind::Image => "Photo",
            MessageKind::Video => "Video",
            MessageKind::Audio => "Voice note",
            MessageKind::File => "File",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Queued,
    Sending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl DeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryState::Queued => "queued",
            DeliveryState::Sending => "sending",
            DeliveryState::Sent => "sent",
            DeliveryState::Delivered => "delivered",
            DeliveryState::Read => "read",
            DeliveryState::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "queued" => DeliveryState::Queued,
            "sending" => DeliveryState::Sending,
            "sent" => DeliveryState::Sent,
            "delivered" => DeliveryState::Delivered,
            "read" => DeliveryState::Read,
            _ => DeliveryState::Failed,
        }
    }
}

/// First frame of every connection. Sent by the dialing peer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub peer_id: Uuid,
    pub display_name: String,
    pub avatar_color: u32,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// True when a LAN passphrase is configured on this device.
    #[serde(default)]
    pub secure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt_b64: Option<String>,
    /// AEAD proof of passphrase knowledge, bound to both peer ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<Sealed>,
    /// Random per-connection id, used for duplicate-connection tie-breaks.
    #[serde(default)]
    pub conn_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HelloAck {
    pub peer_id: Uuid,
    pub display_name: String,
    pub avatar_color: u32,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<Sealed>,
    #[serde(default)]
    pub group_version: u64,
    #[serde(default)]
    pub conn_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub online: bool,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub avatar_color: u32,
    #[serde(default)]
    pub group_version: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Direct chats use the peer id, groups use the group id.
    pub conv: Uuid,
    #[serde(default = "default_conv_kind")]
    pub conv_kind: ConvKind,
    /// Per-sender, per-conversation sequence hint (best effort).
    #[serde(default)]
    pub seq: i64,
    #[serde(default = "default_message_kind")]
    pub kind: MessageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Uuid>,
    #[serde(default)]
    pub mentions: Vec<Uuid>,
}

fn default_conv_kind() -> ConvKind {
    ConvKind::Direct
}

fn default_message_kind() -> MessageKind {
    MessageKind::Text
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ack {
    pub conv: Uuid,
    pub message_ids: Vec<Uuid>,
    pub state: DeliveryState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Typing {
    pub conv: Uuid,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupDesc {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub avatar_color: u32,
    #[serde(default)]
    pub members: Vec<Uuid>,
    #[serde(default)]
    pub created_by: Uuid,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupCreate {
    pub group: GroupDesc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupOp {
    AddMember,
    RemoveMember,
    Rename,
    Leave,
    /// Full-state sync, e.g. when a peer reconnects.
    Sync,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupUpdate {
    pub group_id: Uuid,
    pub op: GroupOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_color: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Uuid>,
    #[serde(default)]
    pub group_version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupDesc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thumbnail {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub data_b64: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileOffer {
    pub transfer_id: Uuid,
    pub message_id: Uuid,
    pub conv: Uuid,
    #[serde(default = "default_conv_kind")]
    pub conv_kind: ConvKind,
    pub name: String,
    #[serde(default)]
    pub mime: String,
    pub size: u64,
    pub chunk_size: u32,
    pub chunks: u32,
    #[serde(default)]
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb: Option<Thumbnail>,
    /// Sender hint: receiver may download without asking when small.
    #[serde(default)]
    pub auto_accept: bool,
    #[serde(default)]
    pub caption: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileChunk {
    pub transfer_id: Uuid,
    pub index: u32,
    pub data_b64: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Accept,
    Reject,
    Resume,
    Progress,
    Done,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileAck {
    pub transfer_id: Uuid,
    pub status: TransferStatus,
    #[serde(default)]
    pub next_index: u32,
    #[serde(default)]
    pub received: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sealed {
    pub nonce_b64: String,
    pub data_b64: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorFrame {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub fatal: bool,
}

impl ErrorFrame {
    pub fn fatal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            fatal: true,
        }
    }
}
