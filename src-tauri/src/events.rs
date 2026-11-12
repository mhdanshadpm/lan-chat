//! Bridges `lanchat-core` events to Tauri events consumed by both webviews.
//! Also triggers desktop notifications and the bubble popup.

use lanchat_core::protocol::{ConvKind, DeliveryState, MessageKind};
use lanchat_core::store::{
    ConversationRecord, CoreEvent, FileProgress, GroupRecord, MessageRecord, PeerSnapshot,
};
use lanchat_core::{Node, Settings};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::bubble;

pub const EVENT: &str = "core://event";

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    Peer { peer: PeerSnapshot },
    PeerOffline { id: Uuid },
    Message { message: MessageRecord },
    MessageState { conv: Uuid, ids: Vec<Uuid>, state: DeliveryState },
    Typing { peer: Uuid, peer_name: String, conv: Uuid, active: bool },
    GroupUpdated { group: GroupRecord },
    ConversationUpdated { conversation: ConversationRecord },
    Transfer { file: FileProgress },
    Error { context: String, message: String },
    NetworkChanged,
    Listening { port: u16 },
    Settings { settings: Settings },
}

pub fn spawn(app: AppHandle, node: Node) {
    tauri::async_runtime::spawn(async move {
        let mut rx = node.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let CoreEvent::Message(message) = &event {
                        maybe_notify(&app, &node, message).await;
                    }
                    if let Some(ui_event) = to_ui(event) {
                        if let Err(err) = app.emit(EVENT, ui_event) {
                            tracing::debug!(%err, "no webview listening for core event");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "UI event stream lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn to_ui(event: CoreEvent) -> Option<UiEvent> {
    Some(match event {
        CoreEvent::PeerUpdated(peer) => UiEvent::Peer { peer },
        CoreEvent::PeerOffline(id) => UiEvent::PeerOffline { id },
        CoreEvent::Message(message) => UiEvent::Message { message },
        CoreEvent::MessageState { conv, ids, state } => {
            UiEvent::MessageState { conv, ids, state }
        }
        CoreEvent::Typing { peer, peer_name, conv, active } => {
            UiEvent::Typing { peer, peer_name, conv, active }
        }
        CoreEvent::GroupUpdated(group) => UiEvent::GroupUpdated { group },
        CoreEvent::ConversationUpdated(conversation) => {
            UiEvent::ConversationUpdated { conversation }
        }
        CoreEvent::Transfer(file) => UiEvent::Transfer { file },
        CoreEvent::Error { context, message } => UiEvent::Error { context, message },
        CoreEvent::NetworkChanged => UiEvent::NetworkChanged,
        CoreEvent::Listening(port) => UiEvent::Listening { port },
    })
}

async fn maybe_notify(app: &AppHandle, node: &Node, message: &MessageRecord) {
    if message.outgoing || matches!(message.kind, MessageKind::System) {
        return;
    }

    bubble::handle_incoming(app, message).await;

    let settings = node.settings().await;
    let muted = node.store().conversation_muted(message.conv);
    if settings.dnd || muted || bubble::main_focused(app) {
        return;
    }

    let title = match node.conversation(message.conv) {
        Some(conv) if conv.kind == ConvKind::Group => {
            format!("{} · {}", conv.title, message.sender_name)
        }
        _ => message.sender_name.clone(),
    };
    let spec = lanchat_platform::NotificationSpec {
        title,
        body: message.preview(),
        conv: Some(message.conv.to_string()),
    };
    let platform = lanchat_platform::current();
    if let Err(err) = platform.show_notification(app, &spec) {
        tracing::debug!(%err, "notification failed");
    }
    if settings.notification_sound {
        if let Err(err) = platform.play_notification_sound() {
            tracing::debug!(%err, "notification sound failed");
        }
    }
}

/// Emits a settings update to both windows (e.g. after toggling DND from the
/// tray menu).
pub fn emit_settings(app: &AppHandle, settings: &Settings) {
    let _ = app.emit(EVENT, UiEvent::Settings { settings: settings.clone() });
}
