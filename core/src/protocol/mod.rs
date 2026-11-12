//! LAN Chat wire protocol, version 1.
//!
//! Every frame is a JSON object with a top-level `"v": 1` plus envelope
//! metadata. The body is flattened into the same object and tagged with
//! `"type"`. See `docs/PROTOCOL.md` for the normative schema and examples.

pub mod messages;

pub use messages::*;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The only wire version this build speaks. Bump only for breaking changes;
/// additive fields are backwards compatible within a major version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Hard cap for a single WebSocket text frame after JSON decoding.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Payload bytes per file chunk (base64 makes the frame ~33% larger).
pub const FILE_CHUNK_BYTES: usize = 64 * 1024;
/// Discovery announcer/receiver UDP port. Override with `LANCHAT_DISCOVERY_PORT`.
pub const DEFAULT_DISCOVERY_PORT: u16 = 47_823;
/// DNS-SD service type used for mDNS discovery.
pub const SERVICE_TYPE: &str = "_lanchat._tcp.local.";
/// Stable conversation id of the automatic "Everyone" group.
pub const EVERYONE_GROUP_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

const MAX_TEXT_CHARS: usize = 16_000;
const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("missing protocol version")]
    MissingVersion,
    #[error("unsupported protocol version {0} (this build speaks v{PROTOCOL_VERSION})")]
    UnsupportedVersion(u16),
    #[error("invalid field `{0}`: {1}")]
    InvalidField(String, String),
    #[error("field `{field}` exceeds {max} bytes")]
    TooLarge { field: &'static str, max: usize },
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    FrameTooLarge,
    #[error("malformed JSON: {0}")]
    Malformed(String),
}

/// A versioned wire frame.
///
/// ```json
/// {"v":1,"id":"…","from":"…","ts":1730000000000,"type":"message","conv":"…"}
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u16,
    pub id: Uuid,
    pub from: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Uuid>,
    /// Sender wall clock in milliseconds since the Unix epoch. Used for
    /// ordering hints only; local insertion time is authoritative.
    pub ts: i64,
    #[serde(flatten)]
    pub body: Body,
}

impl Envelope {
    pub fn new(from: Uuid, to: Option<Uuid>, body: Body) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: Uuid::new_v4(),
            from,
            to,
            ts: crate::now_ms(),
            body,
        }
    }

    pub fn encode(&self) -> Result<String, ProtocolError> {
        let s = serde_json::to_string(self).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
        if s.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(s)
    }

    pub fn decode(raw: &str) -> Result<Self, ProtocolError> {
        if raw.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
        let obj = value
            .as_object()
            .ok_or_else(|| ProtocolError::Malformed("frame must be a JSON object".into()))?;
        let v = obj
            .get("v")
            .ok_or(ProtocolError::MissingVersion)?
            .as_u64()
            .ok_or(ProtocolError::MissingVersion)?;
        if v != u64::from(PROTOCOL_VERSION) {
            return Err(ProtocolError::UnsupportedVersion(v as u16));
        }
        serde_json::from_value(value).map_err(|e| ProtocolError::Malformed(e.to_string()))
    }

    pub fn kind(&self) -> &'static str {
        self.body.kind()
    }

    /// Structural validation that does not require any network context.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.v != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(self.v));
        }
        if self.from.is_nil() {
            return Err(ProtocolError::InvalidField("from".into(), "nil uuid".into()));
        }
        match &self.body {
            Body::Hello(h) => {
                if h.peer_id != self.from {
                    return Err(ProtocolError::InvalidField(
                        "hello.peer_id".into(),
                        "must equal envelope.from".into(),
                    ));
                }
                check_len("hello.display_name", &h.display_name, MAX_NAME_CHARS)?;
                check_len("hello.device_name", &h.device_name, MAX_NAME_CHARS)?;
                if h.secure && (h.salt_b64.is_none() || h.proof.is_none()) {
                    return Err(ProtocolError::InvalidField(
                        "hello.proof".into(),
                        "secure hello requires salt and proof".into(),
                    ));
                }
            }
            Body::HelloAck(h) => {
                check_len("hello_ack.display_name", &h.display_name, MAX_NAME_CHARS)?;
                if h.secure && h.proof.is_none() {
                    return Err(ProtocolError::InvalidField(
                        "hello_ack.proof".into(),
                        "secure hello_ack requires a proof".into(),
                    ));
                }
            }
            Body::Presence(p) => {
                check_len("presence.display_name", &p.display_name, MAX_NAME_CHARS)?;
            }
            Body::Message(m) => {
                if let Some(text) = &m.text {
                    check_len("message.text", text, MAX_TEXT_CHARS)?;
                }
                if m.kind == MessageKind::Text && m.text.is_none() {
                    return Err(ProtocolError::InvalidField(
                        "message.text".into(),
                        "text messages require text".into(),
                    ));
                }
            }
            Body::Typing(_) | Body::Ack(_) => {}
            Body::GroupCreate(g) => check_len("group_create.name", &g.group.name, MAX_NAME_CHARS)?,
            Body::GroupUpdate(u) => {
                if let Some(name) = &u.name {
                    check_len("group_update.name", name, MAX_NAME_CHARS)?;
                }
            }
            Body::FileOffer(o) => {
                check_len("file_offer.name", &o.name, 255)?;
                if o.chunks == 0 || o.chunk_size == 0 {
                    return Err(ProtocolError::InvalidField(
                        "file_offer.chunks".into(),
                        "must be positive".into(),
                    ));
                }
                if o.chunk_size as usize > FILE_CHUNK_BYTES {
                    return Err(ProtocolError::InvalidField(
                        "file_offer.chunk_size".into(),
                        "larger than protocol maximum".into(),
                    ));
                }
            }
            Body::FileChunk(c) => {
                if c.data_b64.len() > (FILE_CHUNK_BYTES * 4 / 3) + 64 {
                    return Err(ProtocolError::TooLarge {
                        field: "file_chunk.data_b64",
                        max: FILE_CHUNK_BYTES * 4 / 3 + 64,
                    });
                }
            }
            Body::FileAck(_) => {}
            Body::Sealed(s) => {
                if s.nonce_b64.len() > 64 || s.data_b64.len() > MAX_FRAME_BYTES {
                    return Err(ProtocolError::TooLarge {
                        field: "sealed.data_b64",
                        max: MAX_FRAME_BYTES,
                    });
                }
            }
            Body::Error(_) => {}
        }
        Ok(())
    }
}

fn check_len(field: &'static str, value: &str, max_chars: usize) -> Result<(), ProtocolError> {
    if value.chars().count() > max_chars {
        return Err(ProtocolError::TooLarge {
            field,
            max: max_chars,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_envelope() -> Envelope {
        let from = Uuid::new_v4();
        Envelope::new(
            from,
            Some(Uuid::new_v4()),
            Body::Message(ChatMessage {
                conv: from,
                conv_kind: ConvKind::Direct,
                seq: 7,
                kind: MessageKind::Text,
                text: Some("hello from the LAN".into()),
                file_id: None,
                reply_to: None,
                mentions: vec![],
            }),
        )
    }

    #[test]
    fn envelope_roundtrip_preserves_the_body() {
        let env = text_envelope();
        assert_eq!(env.kind(), "message");
        let raw = env.encode().unwrap();
        let decoded = Envelope::decode(&raw).unwrap();
        assert_eq!(decoded, env);
        decoded.validate().unwrap();
    }

    #[test]
    fn decode_requires_a_version() {
        let raw = r#"{"id":"5c4b3a2f-1e0d-4c2b-8a9f-6e5d4c3b2a10","from":"4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10","ts":0,"type":"typing","conv":"0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44","active":true}"#;
        assert_eq!(Envelope::decode(raw), Err(ProtocolError::MissingVersion));
    }

    #[test]
    fn decode_rejects_other_versions() {
        let raw = r#"{"v":2,"id":"5c4b3a2f-1e0d-4c2b-8a9f-6e5d4c3b2a10","from":"4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10","ts":0,"type":"typing","conv":"0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44","active":true}"#;
        assert_eq!(
            Envelope::decode(raw),
            Err(ProtocolError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn decode_ignores_unknown_fields() {
        let env = text_envelope();
        let mut value = serde_json::to_value(&env).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("future_field".into(), serde_json::json!({"any": true}));
        let decoded = Envelope::decode(&value.to_string()).unwrap();
        assert_eq!(decoded, env);
    }

    #[test]
    fn text_messages_require_text() {
        let env = Envelope::new(
            Uuid::new_v4(),
            None,
            Body::Message(ChatMessage {
                conv: Uuid::new_v4(),
                conv_kind: ConvKind::Direct,
                seq: 0,
                kind: MessageKind::Text,
                text: None,
                file_id: None,
                reply_to: None,
                mentions: vec![],
            }),
        );
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::InvalidField(field, _)) if field == "message.text"
        ));
    }

    #[test]
    fn oversized_text_is_rejected() {
        let mut env = text_envelope();
        if let Body::Message(message) = &mut env.body {
            message.text = Some("x".repeat(MAX_TEXT_CHARS + 1));
        }
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::TooLarge { field: "message.text", .. })
        ));
    }

    #[test]
    fn attachments_may_omit_text() {
        let env = Envelope::new(
            Uuid::new_v4(),
            None,
            Body::Message(ChatMessage {
                conv: Uuid::new_v4(),
                conv_kind: ConvKind::Direct,
                seq: 0,
                kind: MessageKind::Image,
                text: None,
                file_id: Some(Uuid::new_v4()),
                reply_to: None,
                mentions: vec![],
            }),
        );
        env.validate().unwrap();
    }

    #[test]
    fn hello_must_match_the_envelope_sender() {
        let from = Uuid::new_v4();
        let env = Envelope::new(
            from,
            None,
            Body::Hello(Hello {
                peer_id: Uuid::new_v4(),
                display_name: "Alice".into(),
                avatar_color: 0x74aaf7,
                device_name: String::new(),
                app_version: "0.1.0".into(),
                capabilities: vec!["files".into()],
                secure: false,
                salt_b64: None,
                proof: None,
                conn_id: Uuid::new_v4(),
            }),
        );
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::InvalidField(field, _)) if field == "hello.peer_id"
        ));
    }

    #[test]
    fn secure_hello_requires_a_salt_and_proof() {
        let from = Uuid::new_v4();
        let env = Envelope::new(
            from,
            None,
            Body::Hello(Hello {
                peer_id: from,
                display_name: "Alice".into(),
                avatar_color: 0,
                device_name: String::new(),
                app_version: String::new(),
                capabilities: vec![],
                secure: true,
                salt_b64: None,
                proof: None,
                conn_id: Uuid::new_v4(),
            }),
        );
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::InvalidField(field, _)) if field == "hello.proof"
        ));
    }

    #[test]
    fn oversized_file_chunks_are_rejected() {
        let env = Envelope::new(
            Uuid::new_v4(),
            None,
            Body::FileChunk(FileChunk {
                transfer_id: Uuid::new_v4(),
                index: 0,
                data_b64: "A".repeat(FILE_CHUNK_BYTES * 4 / 3 + 65),
            }),
        );
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::TooLarge {
                field: "file_chunk.data_b64",
                ..
            })
        ));
    }

    #[test]
    fn file_offer_rejects_zero_chunks() {
        let env = Envelope::new(
            Uuid::new_v4(),
            None,
            Body::FileOffer(FileOffer {
                transfer_id: Uuid::new_v4(),
                message_id: Uuid::new_v4(),
                conv: Uuid::new_v4(),
                conv_kind: ConvKind::Direct,
                name: "photo.jpg".into(),
                mime: "image/jpeg".into(),
                size: 10,
                chunk_size: FILE_CHUNK_BYTES as u32,
                chunks: 0,
                sha256: String::new(),
                thumb: None,
                auto_accept: false,
                caption: None,
            }),
        );
        assert!(matches!(
            env.validate(),
            Err(ProtocolError::InvalidField(field, _)) if field == "file_offer.chunks"
        ));
    }
}
