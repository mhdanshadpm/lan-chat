use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use lanchat_core::store::FileRecord;
use lanchat_core::{
    ConversationRecord, GroupRecord, Identity, MessageRecord, PeerSnapshot, Settings,
};
use lanchat_platform::Capabilities;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::bubble;
use crate::events;
use crate::state::AppState;

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[derive(Serialize)]
pub struct Bootstrap {
    pub identity: Identity,
    pub settings: Settings,
    pub peers: Vec<PeerSnapshot>,
    pub conversations: Vec<ConversationRecord>,
    pub groups: Vec<GroupRecord>,
    pub unread_total: u32,
    pub port: u16,
    pub platform: String,
    pub capabilities: Capabilities,
    pub configured: bool,
}

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, String> {
    let node = &state.node;
    let identity = node.identity().await;
    Ok(Bootstrap {
        settings: node.settings().await,
        peers: node.peers(),
        conversations: node.conversations(),
        groups: node.groups().await,
        unread_total: node.unread_total(),
        port: node.local_port(),
        platform: lanchat_platform::current().name().to_string(),
        capabilities: lanchat_platform::current().capabilities(),
        configured: identity.is_configured(),
        identity,
    })
}

#[tauri::command]
pub async fn save_identity(
    state: State<'_, AppState>,
    display_name: String,
    avatar_color: u32,
) -> Result<Identity, String> {
    state
        .node
        .set_identity(Some(display_name), Some(avatar_color))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn list_peers(state: State<'_, AppState>) -> Result<Vec<PeerSnapshot>, String> {
    Ok(state.node.peers())
}

#[tauri::command]
pub async fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationRecord>, String> {
    Ok(state.node.conversations())
}

#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> Result<Vec<GroupRecord>, String> {
    Ok(state.node.groups().await)
}

#[tauri::command]
pub async fn messages(
    state: State<'_, AppState>,
    conv: Uuid,
    before_ts: Option<i64>,
    limit: Option<u32>,
) -> Result<Vec<MessageRecord>, String> {
    Ok(state.node.messages(conv, before_ts, limit.unwrap_or(80).clamp(1, 500)))
}

#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<MessageRecord>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(state.node.search(&query, limit.unwrap_or(100).clamp(1, 500)))
}

#[tauri::command]
pub async fn send_text(
    state: State<'_, AppState>,
    conv: Uuid,
    text: String,
    reply_to: Option<Uuid>,
) -> Result<MessageRecord, String> {
    state
        .node
        .send_text(conv, text, reply_to)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn send_file(
    state: State<'_, AppState>,
    conv: Uuid,
    path: String,
    caption: Option<String>,
) -> Result<MessageRecord, String> {
    state
        .node
        .send_file(conv, PathBuf::from(path), caption)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn send_file_bytes(
    state: State<'_, AppState>,
    conv: Uuid,
    name: String,
    mime: String,
    data_b64: String,
    caption: Option<String>,
) -> Result<MessageRecord, String> {
    state
        .node
        .send_file_bytes(conv, name, mime, data_b64, caption)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn mark_read(state: State<'_, AppState>, conv: Uuid) -> Result<(), String> {
    state.node.mark_read(conv).await.map_err(err)
}

#[tauri::command]
pub async fn set_typing(
    state: State<'_, AppState>,
    conv: Uuid,
    active: bool,
) -> Result<(), String> {
    state.node.set_typing(conv, active).await.map_err(err)
}

#[tauri::command]
pub async fn set_muted(
    app: AppHandle,
    state: State<'_, AppState>,
    conv: Uuid,
    muted: bool,
) -> Result<(), String> {
    state.node.set_muted(conv, muted).map_err(err)?;
    let settings = state.node.settings().await;
    events::emit_settings(&app, &settings);
    Ok(())
}

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    name: String,
    members: Vec<Uuid>,
) -> Result<GroupRecord, String> {
    state.node.create_group(name, members).await.map_err(err)
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: Uuid,
    peer: Uuid,
) -> Result<(), String> {
    state.node.add_group_member(group_id, peer).await.map_err(err)
}

#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, AppState>,
    group_id: Uuid,
    peer: Uuid,
) -> Result<(), String> {
    state
        .node
        .remove_group_member(group_id, peer)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn rename_group(
    state: State<'_, AppState>,
    group_id: Uuid,
    name: String,
) -> Result<(), String> {
    state.node.rename_group(group_id, name).await.map_err(err)
}

#[tauri::command]
pub async fn leave_group(state: State<'_, AppState>, group_id: Uuid) -> Result<(), String> {
    state.node.leave_group(group_id).await.map_err(err)
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let old = state.node.settings().await;
    let saved = state.node.update_settings(settings).await.map_err(err)?;
    if old.launch_at_login != saved.launch_at_login {
        if let Err(error) = lanchat_platform::current().set_autostart(&app, saved.launch_at_login) {
            tracing::warn!(%error, "could not update launch at login");
        }
    }
    bubble::apply_size(&app, &saved.bubble);
    bubble::apply_position(&app, &saved.bubble);
    events::emit_settings(&app, &saved);
    Ok(saved)
}

#[tauri::command]
pub async fn save_bubble_position(
    app: AppHandle,
    state: State<'_, AppState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut settings = state.node.settings().await;
    settings.bubble.x = Some(x);
    settings.bubble.y = Some(y);
    let saved = state.node.update_settings(settings).await.map_err(err)?;
    events::emit_settings(&app, &saved);
    Ok(())
}

#[tauri::command]
pub async fn reset_bubble_position(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut settings = state.node.settings().await;
    settings.bubble.x = None;
    settings.bubble.y = None;
    let saved = state.node.update_settings(settings).await.map_err(err)?;
    bubble::apply_position(&app, &saved.bubble);
    events::emit_settings(&app, &saved);
    Ok(())
}

#[tauri::command]
pub fn show_bubble(app: AppHandle, conv: Option<Uuid>) {
    bubble::open_for(&app, conv);
}

#[tauri::command]
pub fn hide_bubble(app: AppHandle) {
    bubble::hide(&app);
}

#[tauri::command]
pub fn open_main(app: AppHandle, conv: Option<Uuid>) {
    if let Some(conv) = conv {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.emit("app://open-conv", conv);
        }
    }
    bubble::open_main(&app);
}

#[tauri::command]
pub async fn last_active_conversation(state: State<'_, AppState>) -> Result<Option<Uuid>, String> {
    Ok(state.node.store().last_active_conversation())
}

#[tauri::command]
pub async fn pending_files(state: State<'_, AppState>) -> Result<Vec<FileRecord>, String> {
    Ok(state.node.pending_files())
}

#[tauri::command]
pub async fn file_record(
    state: State<'_, AppState>,
    transfer_id: Uuid,
) -> Result<Option<FileRecord>, String> {
    Ok(state.node.file(transfer_id))
}

#[tauri::command]
pub async fn respond_file(
    state: State<'_, AppState>,
    transfer_id: Uuid,
    accept: bool,
) -> Result<(), String> {
    state.node.respond_file(transfer_id, accept).await.map_err(err)
}

#[tauri::command]
pub fn file_thumbnail(state: State<'_, AppState>, transfer_id: Uuid) -> Option<String> {
    let file = state.node.file(transfer_id)?;
    let (path, mime) = if let Some(thumb) = file.thumb_path {
        (thumb, "image/jpeg".to_string())
    } else if file.mime.starts_with("image/") {
        (file.path?, file.mime)
    } else {
        return None;
    };
    let metadata = std::fs::metadata(&path).ok()?;
    if metadata.len() > 12 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    Some(format!("data:{mime};base64,{}", B64.encode(bytes)))
}

fn existing_path(state: &State<'_, AppState>, transfer_id: Uuid) -> Result<PathBuf, String> {
    let file = state
        .node
        .file(transfer_id)
        .ok_or_else(|| "unknown transfer".to_string())?;
    let path = file.path.ok_or_else(|| "file is not available yet".to_string())?;
    if !path.exists() {
        return Err("file is not downloaded yet".into());
    }
    Ok(path)
}

#[tauri::command]
pub fn reveal_file(state: State<'_, AppState>, transfer_id: Uuid) -> Result<(), String> {
    let path = existing_path(&state, transfer_id)?;
    lanchat_platform::current().reveal_file(&path).map_err(err)
}

#[tauri::command]
pub fn open_file(state: State<'_, AppState>, transfer_id: Uuid) -> Result<(), String> {
    let path = existing_path(&state, transfer_id)?;
    lanchat_platform::current().open_path(&path).map_err(err)
}

#[tauri::command]
pub async fn unread_total(state: State<'_, AppState>) -> Result<u32, String> {
    Ok(state.node.unread_total())
}
