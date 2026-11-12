//! Chunked, resumable file transfer and local thumbnail generation.
//!
//! Sender flow: offer → wait for `accept`/`resume` → stream chunks → wait for
//! `done`. If the connection drops mid-stream the task pauses, waits for the
//! peer to reconnect, re-offers and continues from the receiver's `next_index`.
//!
//! Receiver flow: offer → auto-accept below the configured threshold (or wait
//! for the UI) → append chunks in order → verify sha256 → move `.part` into
//! place → acknowledge `done`.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sha2::{Digest, Sha256};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::net::{Inner, Node};
use crate::protocol::*;
use crate::store::{
    CoreEvent, FileDirection, FileProgress, FileRecord, FileState, MessageRecord,
};

const TRANSFER_DEADLINE: Duration = Duration::from_secs(600);
const OFFER_RETRY: Duration = Duration::from_secs(45);
const STREAM_STATE: Duration = Duration::from_millis(180);

impl Node {
    /// Persists an outgoing attachment message and starts a transfer per
    /// recipient. Returns the stored message immediately; progress is emitted
    /// as [`CoreEvent::Transfer`].
    pub async fn send_file(
        &self,
        conv: Uuid,
        path: PathBuf,
        caption: Option<String>,
    ) -> Result<MessageRecord> {
        let settings = self.inner.settings.read().await.clone();
        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.len() > settings.max_file_bytes() {
            return Err(CoreError::Invalid(format!(
                "file is larger than the configured {} MB limit",
                settings.max_file_mb
            )));
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(sanitize_filename)
            .unwrap_or_else(|| "file".into());
        let mime = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();
        let kind = kind_for(&mime, &name);
        let sha256 = sha256_file(path.clone()).await?;
        let thumb = make_thumbnail(&path, &mime).await;
        let transfer_id = Uuid::new_v4();
        let size = metadata.len();
        let chunk_size = FILE_CHUNK_BYTES as u32;
        let chunks = (((size + FILE_CHUNK_BYTES as u64 - 1) / FILE_CHUNK_BYTES as u64) as u32).max(1);

        let record = FileRecord {
            id: transfer_id,
            message_id: None,
            conv,
            peer_id: None,
            direction: FileDirection::Outgoing,
            name,
            path: Some(path),
            thumb_path: None,
            mime,
            size,
            received: 0,
            sha256,
            state: FileState::AwaitingAccept,
            next_index: 0,
            chunk_size,
            chunks,
            created_at: crate::now_ms(),
        };
        self.inner.store.insert_file(&record)?;

        let caption = caption.filter(|c| !c.trim().is_empty());
        let message = self
            .inner
            .send_chat(conv, kind, caption, Some(transfer_id), None, Vec::new(), true)
            .await?;
        let direct_peer = if message.conv_kind == ConvKind::Direct {
            Some(conv)
        } else {
            None
        };
        self.inner
            .store
            .link_file(transfer_id, message.id, direct_peer)?;

        let file = self
            .inner
            .store
            .get_file(transfer_id)
            .ok_or_else(|| CoreError::Internal("file row disappeared".into()))?;
        let targets = self
            .inner
            .targets_for(conv, message.conv_kind)
            .await;
        if targets.is_empty() {
            emit_file(&self.inner, transfer_id, None);
        }
        for target in targets {
            let inner = self.inner.clone();
            let file = file.clone();
            let thumb = thumb.clone();
            tokio::spawn(async move {
                send_to_peer(inner, file, target, thumb).await;
            });
        }
        emit_file(&self.inner, transfer_id, None);
        Ok(message)
    }

    /// Accepts paste/drag payloads from the webview where a native path is not
    /// available. The bytes are written under the app data directory first.
    pub async fn send_file_bytes(
        &self,
        conv: Uuid,
        name: String,
        _mime: String,
        data_b64: String,
        caption: Option<String>,
    ) -> Result<MessageRecord> {
        let settings = self.inner.settings.read().await.clone();
        let bytes = B64
            .decode(data_b64.as_bytes())
            .map_err(|e| CoreError::Invalid(format!("attachment is not base64: {e}")))?;
        if bytes.len() as u64 > settings.max_file_bytes() {
            return Err(CoreError::Invalid("attachment is too large".into()));
        }
        let dir = self.inner.data_dir.join("outbox");
        tokio::fs::create_dir_all(&dir).await?;
        let safe = sanitize_filename(&name);
        let path = dir.join(format!("{}-{}", Uuid::new_v4(), safe));
        tokio::fs::write(&path, &bytes).await?;
        self.send_file(conv, path, caption).await
    }

    pub fn file(&self, transfer_id: Uuid) -> Option<FileRecord> {
        self.inner.store.get_file(transfer_id)
    }

    pub fn pending_files(&self) -> Vec<FileRecord> {
        self.inner.store.pending_incoming_files()
    }

    pub async fn respond_file(&self, transfer_id: Uuid, accept: bool) -> Result<()> {
        let file = self
            .inner
            .store
            .get_file(transfer_id)
            .ok_or_else(|| CoreError::NotFound("transfer".into()))?;
        if file.direction != FileDirection::Incoming {
            return Err(CoreError::Invalid("not an incoming transfer".into()));
        }
        let peer = file
            .peer_id
            .ok_or_else(|| CoreError::Invalid("transfer has no sender".into()))?;
        if accept {
            self.inner.accept_incoming(transfer_id, peer).await
        } else {
            self.inner
                .store
                .set_file_state(transfer_id, FileState::Rejected)?;
            self.inner
                .send_body(
                    peer,
                    Body::FileAck(FileAck {
                        transfer_id,
                        status: TransferStatus::Cancel,
                        next_index: 0,
                        received: file.received,
                    }),
                )
                .await;
            emit_file(&self.inner, transfer_id, Some(peer));
            Ok(())
        }
    }
}

impl Inner {
    pub(crate) async fn accept_incoming(self: &std::sync::Arc<Self>, transfer_id: Uuid, peer: Uuid) -> Result<()> {
        let file = self
            .store
            .get_file(transfer_id)
            .ok_or_else(|| CoreError::NotFound("transfer".into()))?;
        let next_index = file.next_index;
        self.store
            .update_file_progress(transfer_id, file.received, next_index, FileState::Downloading)?;
        let status = if next_index > 0 {
            TransferStatus::Resume
        } else {
            TransferStatus::Accept
        };
        self.send_body(
            peer,
            Body::FileAck(FileAck {
                transfer_id,
                status,
                next_index,
                received: file.received,
            }),
        )
        .await;
        emit_file(self, transfer_id, Some(peer));
        Ok(())
    }

    pub(crate) async fn resume_transfers_for(
        self: &std::sync::Arc<Self>,
        peer: Uuid,
    ) {
        for file in self.store.outgoing_pending_files(Some(peer)) {
            if self.active_transfers.lock().contains(&(file.id, peer)) {
                continue;
            }
            let inner = self.clone();
            tokio::spawn(async move {
                send_to_peer(inner, file, peer, None).await;
            });
        }
    }

    pub(crate) async fn handle_file_offer(
        self: &std::sync::Arc<Self>,
        peer: Uuid,
        offer: FileOffer,
    ) -> Result<()> {
        let settings = self.settings.read().await.clone();
        if offer.size > settings.max_file_bytes() {
            self.send_body(
                peer,
                Body::FileAck(FileAck {
                    transfer_id: offer.transfer_id,
                    status: TransferStatus::Reject,
                    next_index: 0,
                    received: 0,
                }),
            )
            .await;
            return Ok(());
        }
        if offer.conv_kind == ConvKind::Direct && offer.conv != peer {
            tracing::warn!(%peer, "rejecting direct file offer with mismatched conversation");
            return Ok(());
        }
        if offer.conv_kind == ConvKind::Group
            && offer.conv != EVERYONE_GROUP_ID
            && self.store.get_group(offer.conv).is_none()
        {
            tracing::warn!(%peer, conv = %offer.conv, "file offer for unknown group");
        }

        if let Some(existing) = self.store.get_file(offer.transfer_id) {
            match existing.state {
                FileState::Done => {
                    self.send_body(
                        peer,
                        Body::FileAck(FileAck {
                            transfer_id: offer.transfer_id,
                            status: TransferStatus::Done,
                            next_index: existing.chunks,
                            received: existing.size,
                        }),
                    )
                    .await;
                    return Ok(());
                }
                FileState::Downloading | FileState::Paused => {
                    return self.accept_incoming(offer.transfer_id, peer).await;
                }
                FileState::PendingOffer => {
                    if offer.size <= settings.auto_accept_bytes() {
                        return self.accept_incoming(offer.transfer_id, peer).await;
                    }
                    emit_file(self, offer.transfer_id, Some(peer));
                    return Ok(());
                }
                _ => {}
            }
        }

        let (title, avatar_color) = if offer.conv_kind == ConvKind::Group {
            match self.store.get_group(offer.conv) {
                Some(group) => (group.name, group.avatar_color),
                None => ("Group".to_string(), 0),
            }
        } else {
            self.store
                .get_peer(peer)
                .map(|p| (p.display_name, p.avatar_color))
                .unwrap_or_default()
        };
        self.store
            .ensure_conversation(offer.conv, offer.conv_kind, &title, avatar_color)?;

        let safe_name = sanitize_filename(&offer.name);
        let thumb_path = save_thumbnail(&self.data_dir, offer.transfer_id, offer.thumb.as_ref());
        let final_path = unique_download_path(&self.download_dir, &safe_name);
        let record = FileRecord {
            id: offer.transfer_id,
            message_id: Some(offer.message_id),
            conv: offer.conv,
            peer_id: Some(peer),
            direction: FileDirection::Incoming,
            name: safe_name,
            path: Some(final_path),
            thumb_path,
            mime: offer.mime.clone(),
            size: offer.size,
            received: 0,
            sha256: offer.sha256.clone(),
            state: FileState::PendingOffer,
            next_index: 0,
            chunk_size: offer.chunk_size,
            chunks: offer.chunks,
            created_at: crate::now_ms(),
        };
        self.store.insert_file(&record)?;
        emit_file(self, offer.transfer_id, Some(peer));

        if offer.size <= settings.auto_accept_bytes() {
            self.accept_incoming(offer.transfer_id, peer).await?;
        }
        Ok(())
    }

    pub(crate) async fn handle_file_chunk(
        self: &std::sync::Arc<Self>,
        peer: Uuid,
        chunk: FileChunk,
    ) -> Result<()> {
        let Some(file) = self.store.get_file(chunk.transfer_id) else {
            return Ok(());
        };
        if file.direction != FileDirection::Incoming || file.peer_id != Some(peer) {
            return Ok(());
        }
        if file.state != FileState::Downloading {
            return Ok(());
        }
        if chunk.index != file.next_index {
            self.send_body(
                peer,
                Body::FileAck(FileAck {
                    transfer_id: chunk.transfer_id,
                    status: TransferStatus::Resume,
                    next_index: file.next_index,
                    received: file.received,
                }),
            )
            .await;
            return Ok(());
        }
        let data = match B64.decode(chunk.data_b64.as_bytes()) {
            Ok(data) => data,
            Err(_) => {
                self.store
                    .set_file_state(chunk.transfer_id, FileState::Failed)?;
                emit_file(self, chunk.transfer_id, Some(peer));
                return Ok(());
            }
        };
        let Some(final_path) = file.path.clone() else {
            return Ok(());
        };
        let part = part_path(&final_path);
        if let Some(parent) = part.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let chunk_size = file.chunk_size.max(1) as u64;
        let offset = chunk.index as u64 * chunk_size;
        {
            let mut handle = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&part)
                .await?;
            handle.seek(std::io::SeekFrom::Start(offset)).await?;
            handle.write_all(&data).await?;
            handle.flush().await?;
        }
        let received = offset + data.len() as u64;
        let next_index = chunk.index + 1;
        let final_chunk = next_index >= file.chunks;

        if final_chunk {
            if !file.sha256.is_empty() {
                let actual = sha256_file(part.clone()).await?;
                if !actual.eq_ignore_ascii_case(&file.sha256) {
                    tracing::warn!(transfer = %file.id, "file hash mismatch");
                    self.store
                        .set_file_state(file.id, FileState::Failed)?;
                    let _ = tokio::fs::remove_file(&part).await;
                    emit_file(self, file.id, Some(peer));
                    self.send_body(
                        peer,
                        Body::FileAck(FileAck {
                            transfer_id: file.id,
                            status: TransferStatus::Cancel,
                            next_index: 0,
                            received: 0,
                        }),
                    )
                    .await;
                    return Ok(());
                }
            }
            tokio::fs::rename(&part, &final_path).await?;
            self.store
                .update_file_progress(file.id, received, next_index, FileState::Done)?;
            self.store
                .set_file_paths(file.id, Some(&final_path), None, FileState::Done)?;
            self.send_body(
                peer,
                Body::FileAck(FileAck {
                    transfer_id: file.id,
                    status: TransferStatus::Done,
                    next_index,
                    received,
                }),
            )
            .await;
            emit_file(self, file.id, Some(peer));
        } else {
            self.store
                .update_file_progress(file.id, received, next_index, FileState::Downloading)?;
            if next_index % 16 == 0 {
                self.send_body(
                    peer,
                    Body::FileAck(FileAck {
                        transfer_id: file.id,
                        status: TransferStatus::Progress,
                        next_index,
                        received,
                    }),
                )
                .await;
                emit_file(self, file.id, Some(peer));
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_file_ack(
        self: &std::sync::Arc<Self>,
        peer: Uuid,
        ack: FileAck,
    ) -> Result<()> {
        let Some(file) = self.store.get_file(ack.transfer_id) else {
            return Ok(());
        };
        if file.direction != FileDirection::Outgoing {
            return Ok(());
        }
        match ack.status {
            TransferStatus::Done => {
                self.store.set_file_state(file.id, FileState::Done)?;
                self.store
                    .update_file_progress(file.id, file.size, file.chunks, FileState::Done)?;
                emit_file(self, file.id, Some(peer));
            }
            TransferStatus::Reject | TransferStatus::Cancel => {
                self.store.set_file_state(file.id, FileState::Rejected)?;
                emit_file(self, file.id, Some(peer));
            }
            TransferStatus::Progress => {
                let received = ack.received.min(file.size);
                self.store.update_file_progress(
                    file.id,
                    received,
                    ack.next_index,
                    FileState::Uploading,
                )?;
                emit_file(self, file.id, Some(peer));
            }
            TransferStatus::Accept | TransferStatus::Resume => {}
        }
        Ok(())
    }
}

async fn send_to_peer(
    inner: std::sync::Arc<Inner>,
    file: FileRecord,
    peer: Uuid,
    thumb: Option<Thumbnail>,
) {
    if !inner.active_transfers.lock().insert((file.id, peer)) {
        return;
    }
    let result = transfer_to_peer(inner.clone(), &file, peer, thumb).await;
    inner.active_transfers.lock().remove(&(file.id, peer));
    inner.ack_waiters.lock().remove(&(file.id, peer));
    if let Err(err) = result {
        tracing::warn!(?err, transfer = %file.id, %peer, "file transfer failed");
        let _ = inner.store.set_file_state(file.id, FileState::Failed);
        emit_file(&inner, file.id, Some(peer));
    }
}

async fn transfer_to_peer(
    inner: std::sync::Arc<Inner>,
    file: &FileRecord,
    peer: Uuid,
    thumb: Option<Thumbnail>,
) -> Result<()> {
    let Some(path) = file.path.clone() else {
        return Err(CoreError::Invalid("outgoing file has no path".into()));
    };
    if !path.exists() {
        return Err(CoreError::Invalid("outgoing file no longer exists".into()));
    }
    let settings = inner.settings.read().await.clone();
    let auto_accept = file.size <= settings.auto_accept_bytes();
    let thumb = match thumb {
        Some(thumb) => Some(thumb),
        None => make_thumbnail(&path, &file.mime).await,
    };
    let conv_kind = if inner.store.get_group(file.conv).is_some() {
        ConvKind::Group
    } else {
        ConvKind::Direct
    };
    let (ack_tx, mut ack_rx) = mpsc::channel::<FileAck>(64);
    inner
        .ack_waiters
        .lock()
        .insert((file.id, peer), ack_tx);
    let deadline = Instant::now() + TRANSFER_DEADLINE;
    let mut index: u32 = 0;

    'upload: loop {
        if inner.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        while !inner.is_connected(peer).await {
            inner.store.set_file_state(file.id, FileState::Paused)?;
            emit_file(&inner, file.id, Some(peer));
            if Instant::now() > deadline {
                return Err(CoreError::Invalid("peer did not come back online".into()));
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        let offer = FileOffer {
            transfer_id: file.id,
            message_id: file.message_id.unwrap_or(file.id),
            conv: file.conv,
            conv_kind,
            name: file.name.clone(),
            mime: file.mime.clone(),
            size: file.size,
            chunk_size: file.chunk_size,
            chunks: file.chunks,
            sha256: file.sha256.clone(),
            thumb: thumb.clone(),
            auto_accept,
            caption: None,
        };
        if !inner.send_body(peer, Body::FileOffer(offer)).await {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        let mut accepted = false;
        loop {
            match tokio::time::timeout(OFFER_RETRY, ack_rx.recv()).await {
                Ok(Some(ack)) => match ack.status {
                    TransferStatus::Accept | TransferStatus::Resume => {
                        index = ack.next_index.min(file.chunks);
                        accepted = true;
                        break;
                    }
                    TransferStatus::Reject | TransferStatus::Cancel => {
                        inner.store.set_file_state(file.id, FileState::Rejected)?;
                        emit_file(&inner, file.id, Some(peer));
                        return Ok(());
                    }
                    _ => continue,
                },
                Ok(None) => return Ok(()),
                Err(_) => break,
            }
        }
        if !accepted {
            if Instant::now() > deadline {
                return Err(CoreError::Invalid("peer never accepted the offer".into()));
            }
            continue;
        }

        let mut handle = tokio::fs::File::open(&path).await?;
        let mut buffer = vec![0u8; file.chunk_size.max(1) as usize];
        let mut last_emit = Instant::now() - STREAM_STATE;
        while index < file.chunks {
            if inner.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                inner
                    .store
                    .update_file_progress(file.id, file.received, index, FileState::Paused)?;
                return Ok(());
            }
            let offset = index as u64 * file.chunk_size as u64;
            handle.seek(std::io::SeekFrom::Start(offset)).await?;
            let read = handle.read(&mut buffer).await?;
            if read == 0 && !(file.size == 0 && index == 0) {
                return Err(CoreError::Invalid("file ended before all chunks were sent".into()));
            }
            let envelope = Envelope::new(
                inner.my_id().await,
                Some(peer),
                Body::FileChunk(FileChunk {
                    transfer_id: file.id,
                    index,
                    data_b64: B64.encode(&buffer[..read]),
                }),
            );
            if !inner.send_envelope(peer, envelope).await {
                let done = index as u64 * file.chunk_size as u64;
                inner
                    .store
                    .update_file_progress(file.id, done, index, FileState::Paused)?;
                emit_file(&inner, file.id, Some(peer));
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue 'upload;
            }
            index += 1;
            let sent = (index as u64 * file.chunk_size as u64).min(file.size.max(read as u64));
            inner.store.update_file_progress(
                file.id,
                sent,
                index,
                if index >= file.chunks {
                    FileState::AwaitingAccept
                } else {
                    FileState::Uploading
                },
            )?;
            if last_emit.elapsed() >= STREAM_STATE || index >= file.chunks {
                emit_file(&inner, file.id, Some(peer));
                last_emit = Instant::now();
            }
        }
        break;
    }

    // The receiver verifies the hash and sends `done`; don't block completion
    // forever if that ack is lost.
    let _ = tokio::time::timeout(Duration::from_secs(20), ack_rx.recv()).await;
    if let Some(current) = inner.store.get_file(file.id) {
        if current.state != FileState::Rejected && current.state != FileState::Failed {
            inner
                .store
                .update_file_progress(file.id, current.size, current.chunks, FileState::Done)?;
            emit_file(&inner, file.id, Some(peer));
        }
    }
    Ok(())
}

// ---- helpers ------------------------------------------------------------

pub fn kind_for(mime: &str, name: &str) -> MessageKind {
    if mime.starts_with("image/") {
        return MessageKind::Image;
    }
    if mime.starts_with("video/") {
        return MessageKind::Video;
    }
    if mime.starts_with("audio/") {
        return MessageKind::Audio;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "heic" | "bmp" => MessageKind::Image,
        "mp4" | "mov" | "m4v" | "mkv" | "webm" => MessageKind::Video,
        "mp3" | "m4a" | "wav" | "ogg" | "opus" | "aac" | "flac" => MessageKind::Audio,
        _ => MessageKind::File,
    }
}

pub fn sanitize_filename(raw: &str) -> String {
    // Strip directory components for both separator styles so a Windows peer
    // cannot smuggle a path that Unix would treat as a plain file name.
    let base = raw
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "file".into()
    } else {
        trimmed.chars().take(180).collect()
    }
}

fn unique_download_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let extension = path.extension().and_then(|e| e.to_str());
    for counter in 1..10_000 {
        let file_name = match extension {
            Some(ext) => format!("{stem} ({counter}).{ext}"),
            None => format!("{stem} ({counter})"),
        };
        let candidate = dir.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{}-{}", Uuid::new_v4(), name))
}

fn part_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", path.to_string_lossy()))
}

fn emit_file(inner: &Inner, id: Uuid, peer: Option<Uuid>) {
    if let Some(file) = inner.store.get_file(id) {
        let mut progress = FileProgress::from(&file);
        if peer.is_some() {
            progress.peer_id = peer;
        }
        inner.emit(CoreEvent::Transfer(progress));
    }
}

async fn sha256_file(path: PathBuf) -> Result<String> {
    tokio::task::spawn_blocking(move || -> Result<String> {
        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = std::io::Read::read(&mut file, &mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await
    .map_err(|e| CoreError::Internal(format!("hash task failed: {e}")))?
}

async fn make_thumbnail(path: &Path, mime: &str) -> Option<Thumbnail> {
    if !(mime.starts_with("image/") || mime.starts_with("video/")) {
        return None;
    }
    let path = path.to_path_buf();
    let is_video = mime.starts_with("video/");
    tokio::task::spawn_blocking(move || {
        if is_video {
            video_thumbnail(&path)
        } else {
            image_thumbnail(&path)
        }
    })
    .await
    .ok()
    .flatten()
}

fn image_thumbnail(path: &Path) -> Option<Thumbnail> {
    let image = image::open(path).ok()?;
    let thumb = image.thumbnail(480, 480);
    let mut buffer = Vec::new();
    {
        let cursor = Cursor::new(&mut buffer);
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(cursor, 72);
        thumb.write_with_encoder(encoder).ok()?;
    }
    if buffer.len() > 300_000 {
        return None;
    }
    Some(Thumbnail {
        mime: "image/jpeg".into(),
        width: thumb.width(),
        height: thumb.height(),
        data_b64: B64.encode(buffer),
    })
}

fn video_thumbnail(path: &Path) -> Option<Thumbnail> {
    let out = std::env::temp_dir().join(format!("lanchat-vthumb-{}.jpg", Uuid::new_v4()));
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-ss", "0.5", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", "scale=480:-2"])
        .arg(&out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !status.success() || !out.exists() {
        let _ = std::fs::remove_file(&out);
        return None;
    }
    let thumb = image_thumbnail(&out);
    let _ = std::fs::remove_file(&out);
    thumb
}

fn save_thumbnail(data_dir: &Path, transfer_id: Uuid, thumb: Option<&Thumbnail>) -> Option<PathBuf> {
    let thumb = thumb?;
    if thumb.data_b64.len() > 512 * 1024 {
        return None;
    }
    let bytes = B64.decode(thumb.data_b64.as_bytes()).ok()?;
    let dir = data_dir.join("thumbs");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{transfer_id}.jpg"));
    let mut file = std::fs::File::create(&path).ok()?;
    file.write_all(&bytes).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("a/b\\c.txt"), "c.txt");
        assert_eq!(sanitize_filename(".."), "file");
    }

    #[test]
    fn classifies_media_by_mime_and_extension() {
        assert_eq!(kind_for("image/png", "x.bin"), MessageKind::Image);
        assert_eq!(kind_for("video/mp4", "x"), MessageKind::Video);
        assert_eq!(kind_for("audio/ogg", "x"), MessageKind::Audio);
        assert_eq!(kind_for("application/octet-stream", "holiday.JPG"), MessageKind::Image);
        assert_eq!(kind_for("application/pdf", "doc.pdf"), MessageKind::File);
    }

    #[test]
    fn unique_paths_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"one").unwrap();
        let chosen = unique_download_path(dir.path(), "a.txt");
        assert_eq!(
            chosen.file_name().and_then(|n| n.to_str()),
            Some("a (1).txt")
        );
    }

    #[test]
    fn chunks_cover_size_exactly() {
        for size in [0u64, 1, 64 * 1024, 64 * 1024 + 1] {
            let chunks =
                (((size + FILE_CHUNK_BYTES as u64 - 1) / FILE_CHUNK_BYTES as u64) as u32).max(1);
            assert!(chunks >= 1);
            assert!(chunks as u64 * FILE_CHUNK_BYTES as u64 >= size);
        }
    }
}
