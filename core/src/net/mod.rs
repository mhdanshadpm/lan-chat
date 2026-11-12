//! WebSocket mesh: each instance listens on a random TCP port and every peer
//! connects to every other peer that it discovers. Duplicate connections are
//! resolved deterministically. All traffic is JSON `Envelope`s, optionally
//! sealed with the LAN passphrase.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{
    accept_async, connect_async,
    tungstenite::{self, Message},
    WebSocketStream,
};
use uuid::Uuid;

use crate::config::Settings;
use crate::crypto::{new_salt, SessionCipher, SALT_LEN};
use crate::discovery::{network_fingerprint, Discovery, DiscoveryEvent};
use crate::error::{CoreError, Result};
use crate::group::{everyone_group, extract_mentions};
use crate::identity::Identity;
use crate::protocol::*;
use crate::store::{ConversationRecord, CoreEvent, GroupRecord, MessageRecord, PeerSnapshot, Store};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const PEER_TIMEOUT: Duration = Duration::from_secs(20);
const DIAL_COOLDOWN: Duration = Duration::from_secs(4);
const OUTBOX_FLUSH_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub data_dir: PathBuf,
    pub download_dir: PathBuf,
    pub identity: Identity,
    pub settings: Settings,
}

/// Everything a peer connection needs to push frames to its writer task.
pub(crate) enum Outbound {
    Envelope(Envelope),
    Close,
}

pub(crate) struct PeerRuntime {
    pub(crate) snapshot: PeerSnapshot,
    pub(crate) tx: mpsc::Sender<Outbound>,
    pub(crate) conn_id: Uuid,
    pub(crate) initiator: bool,
    pub(crate) last_seen: Instant,
}

pub(crate) struct Inner {
    pub(crate) store: Store,
    pub(crate) identity: RwLock<Identity>,
    pub(crate) settings: RwLock<Settings>,
    pub(crate) peers: RwLock<HashMap<Uuid, PeerRuntime>>,
    pub(crate) events: broadcast::Sender<CoreEvent>,
    pub(crate) port: u16,
    pub(crate) disc_port: u16,
    pub(crate) data_dir: PathBuf,
    pub(crate) download_dir: PathBuf,
    pub(crate) shutdown: AtomicBool,
    pub(crate) discovery: tokio::sync::Mutex<Option<Discovery>>,
    pub(crate) ack_waiters: parking_lot::Mutex<HashMap<(Uuid, Uuid), mpsc::Sender<FileAck>>>,
    pub(crate) last_dial: parking_lot::Mutex<HashMap<Uuid, Instant>>,
    pub(crate) typing_sent: parking_lot::Mutex<HashMap<Uuid, Instant>>,
    pub(crate) active_transfers: parking_lot::Mutex<std::collections::HashSet<(Uuid, Uuid)>>,
}

/// Public handle to the running node. Cheap to clone through `Arc`.
#[derive(Clone)]
pub struct Node {
    pub(crate) inner: Arc<Inner>,
}

impl Node {
    pub async fn start(config: NodeConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.download_dir)?;
        std::fs::create_dir_all(config.data_dir.join("thumbs"))?;

        let store = Store::open(&crate::store::default_data_path(&config.data_dir))?;
        let settings = if store.get_setting("settings").is_some() {
            store.load_settings()
        } else {
            store.save_settings(&config.settings)?;
            config.settings.clone()
        };
        let identity = config.identity;
        if !identity.is_configured() {
            identity.save(&config.data_dir)?;
        }
        store.seed_everyone(identity.peer_id)?;
        store.set_all_peers_offline()?;

        let listener = TcpListener::bind(("0.0.0.0", 0)).await?;
        let port = listener.local_addr()?.port();
        let disc_port = std::env::var("LANCHAT_DISCOVERY_PORT")
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok())
            .filter(|port| *port != 0)
            .unwrap_or(if settings.discovery_port == 0 {
                DEFAULT_DISCOVERY_PORT
            } else {
                settings.discovery_port
            });
        let (events, _) = broadcast::channel(2048);

        let inner = Arc::new(Inner {
            store,
            identity: RwLock::new(identity),
            settings: RwLock::new(settings),
            peers: RwLock::new(HashMap::new()),
            events,
            port,
            disc_port,
            data_dir: config.data_dir,
            download_dir: config.download_dir,
            shutdown: AtomicBool::new(false),
            discovery: tokio::sync::Mutex::new(None),
            ack_waiters: parking_lot::Mutex::new(HashMap::new()),
            last_dial: parking_lot::Mutex::new(HashMap::new()),
            typing_sent: parking_lot::Mutex::new(HashMap::new()),
            active_transfers: parking_lot::Mutex::new(std::collections::HashSet::new()),
        });

        inner.restart_discovery().await;
        tokio::spawn(accept_loop(inner.clone(), listener));
        tokio::spawn(heartbeat_loop(inner.clone()));
        tokio::spawn(netmon_loop(inner.clone()));
        inner.emit(CoreEvent::Listening(port));
        tracing::info!(port, disc_port, "LAN Chat node started");
        Ok(Self { inner })
    }

    pub fn local_port(&self) -> u16 {
        self.inner.port
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.inner.data_dir
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.inner.events.subscribe()
    }

    pub async fn identity(&self) -> Identity {
        self.inner.identity.read().await.clone()
    }

    pub async fn set_identity(&self, name: Option<String>, avatar_color: Option<u32>) -> Result<Identity> {
        let updated = {
            let mut identity = self.inner.identity.write().await;
            if let Some(name) = name {
                identity.set_display_name(&name)?;
            }
            if let Some(color) = avatar_color {
                identity.avatar_color = color;
            }
            identity.save(&self.inner.data_dir)?;
            identity.clone()
        };
        self.inner.restart_discovery().await;
        self.inner.broadcast_presence().await;
        Ok(updated)
    }

    pub async fn settings(&self) -> Settings {
        self.inner.settings.read().await.clone()
    }

    pub async fn update_settings(&self, settings: Settings) -> Result<Settings> {
        let old_passphrase = self.inner.settings.read().await.passphrase.clone();
        self.inner.store.save_settings(&settings)?;
        *self.inner.settings.write().await = settings.clone();
        if old_passphrase != settings.passphrase {
            self.inner.disconnect_all().await;
            self.inner.restart_discovery().await;
        }
        Ok(settings)
    }

    pub fn peers(&self) -> Vec<PeerSnapshot> {
        self.inner.store.list_peers()
    }

    pub fn conversations(&self) -> Vec<ConversationRecord> {
        self.inner.store.list_conversations()
    }

    pub fn conversation(&self, id: Uuid) -> Option<ConversationRecord> {
        self.inner.store.conversation(id)
    }

    pub fn messages(&self, conv: Uuid, before_ts: Option<i64>, limit: u32) -> Vec<MessageRecord> {
        self.inner.store.list_messages(conv, before_ts, limit)
    }

    pub fn search(&self, query: &str, limit: u32) -> Vec<MessageRecord> {
        self.inner.store.search_messages(query, limit)
    }

    pub async fn groups(&self) -> Vec<GroupRecord> {
        let me = self.inner.my_id().await;
        let peers = self.inner.store.list_peers();
        let mut groups = vec![everyone_group(me, &peers)];
        for group in self.inner.store.list_groups() {
            if group.is_default || group.id == EVERYONE_GROUP_ID {
                continue;
            }
            if group.members.contains(&me) {
                groups.push(group);
            }
        }
        groups
    }

    pub fn unread_total(&self) -> u32 {
        self.inner.store.unread_total()
    }

    pub async fn send_text(
        &self,
        conv: Uuid,
        text: String,
        reply_to: Option<Uuid>,
    ) -> Result<MessageRecord> {
        let trimmed = text.trim_end().to_string();
        if trimmed.trim().is_empty() {
            return Err(CoreError::Invalid("message text is empty".into()));
        }
        let me = self.inner.my_id().await;
        let candidates: Vec<(Uuid, String)> = self
            .inner
            .store
            .list_peers()
            .into_iter()
            .filter(|p| p.id != me)
            .map(|p| (p.id, p.display_name))
            .collect();
        let mentions = extract_mentions(&trimmed, &candidates);
        self.inner
            .send_chat(conv, MessageKind::Text, Some(trimmed), None, reply_to, mentions, true)
            .await
    }

    pub async fn mark_read(&self, conv: Uuid) -> Result<()> {
        let changed = self.inner.store.mark_read(conv);
        let read_receipts = self.inner.settings.read().await.read_receipts;
        if !changed.is_empty() && read_receipts {
            let mut by_sender: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
            for (id, sender) in &changed {
                by_sender.entry(*sender).or_default().push(*id);
            }
            for (sender, ids) in by_sender {
                self.inner
                    .send_body(sender, Body::Ack(Ack { conv, message_ids: ids, state: DeliveryState::Read }))
                    .await;
            }
        }
        if !changed.is_empty() {
            self.inner.emit(CoreEvent::MessageState {
                conv,
                ids: changed.into_iter().map(|(id, _)| id).collect(),
                state: DeliveryState::Read,
            });
        }
        if let Some(c) = self.inner.store.conversation(conv) {
            self.inner.emit(CoreEvent::ConversationUpdated(c));
        }
        Ok(())
    }

    pub async fn set_typing(&self, conv: Uuid, active: bool) -> Result<()> {
        if !self.inner.settings.read().await.typing_indicators {
            return Ok(());
        }
        if active {
            let now = Instant::now();
            let mut sent = self.inner.typing_sent.lock();
            if sent.get(&conv).map(|t| now.duration_since(*t) < Duration::from_secs(2)).unwrap_or(false) {
                return Ok(());
            }
            sent.insert(conv, now);
        } else {
            self.inner.typing_sent.lock().remove(&conv);
        }
        let (_, members) = self.inner.conv_meta(conv).await?;
        for member in members {
            self.inner
                .send_body(member, Body::Typing(Typing { conv, active }))
                .await;
        }
        Ok(())
    }

    pub async fn create_group(&self, name: String, members: Vec<Uuid>) -> Result<GroupRecord> {
        let me = self.inner.my_id().await;
        let name = crate::group::sanitize_group_name(&name)
            .ok_or_else(|| CoreError::Invalid("group name is required".into()))?;
        let mut members: Vec<Uuid> = members.into_iter().filter(|m| *m != me).collect();
        members.sort();
        members.dedup();
        members.truncate(crate::group::MAX_GROUP_MEMBERS);
        members.push(me);
        let group = GroupRecord {
            id: Uuid::new_v4(),
            name,
            avatar_color: crate::identity::AVATAR_PALETTE
                [(Uuid::new_v4().as_u128() % crate::identity::AVATAR_PALETTE.len() as u128) as usize],
            members,
            created_by: me,
            created_at: crate::now_ms(),
            is_default: false,
            version: 1,
        };
        self.inner.store.upsert_group(&group)?;
        let body = Body::GroupCreate(GroupCreate { group: group_desc(&group) });
        self.inner.send_to_members(&group.members, body).await;
        self.inner.emit(CoreEvent::GroupUpdated(group.clone()));
        Ok(group)
    }

    pub async fn add_group_member(&self, group_id: Uuid, peer: Uuid) -> Result<()> {
        self.inner.store.add_group_member(group_id, peer)?;
        self.inner.push_group_update(group_id, GroupOp::AddMember, Some(peer)).await
    }

    pub async fn remove_group_member(&self, group_id: Uuid, peer: Uuid) -> Result<()> {
        self.inner.store.remove_group_member(group_id, peer)?;
        self.inner.push_group_update(group_id, GroupOp::RemoveMember, Some(peer)).await
    }

    pub async fn rename_group(&self, group_id: Uuid, name: String) -> Result<()> {
        let name = crate::group::sanitize_group_name(&name)
            .ok_or_else(|| CoreError::Invalid("group name is required".into()))?;
        self.inner.store.rename_group(group_id, &name, None)?;
        self.inner.push_group_update(group_id, GroupOp::Rename, None).await
    }

    pub async fn leave_group(&self, group_id: Uuid) -> Result<()> {
        if group_id == EVERYONE_GROUP_ID {
            return Err(CoreError::Invalid("the Everyone group cannot be left".into()));
        }
        let me = self.inner.my_id().await;
        let group = self
            .inner
            .store
            .get_group(group_id)
            .ok_or_else(|| CoreError::NotFound("group".into()))?;
        if !group.members.contains(&me) {
            return Err(CoreError::Invalid("not a member of this group".into()));
        }
        self.inner.store.remove_group_member(group_id, me)?;
        self.inner
            .push_group_update(group_id, GroupOp::Leave, Some(me))
            .await?;
        Ok(())
    }

    pub fn set_muted(&self, conv: Uuid, muted: bool) -> Result<()> {
        self.inner.store.set_conversation_muted(conv, muted)?;
        if let Some(c) = self.inner.store.conversation(conv) {
            self.inner.emit(CoreEvent::ConversationUpdated(c));
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        let mut discovery = self.inner.discovery.lock().await;
        if let Some(mut discovery) = discovery.take() {
            discovery.stop();
        }
        drop(discovery);
        self.inner.disconnect_all().await;
    }
}

impl Inner {
    pub(crate) async fn my_id(&self) -> Uuid {
        self.identity.read().await.peer_id
    }

    pub(crate) fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    pub(crate) async fn restart_discovery(self: &Arc<Self>) {
        let mut guard = self.discovery.lock().await;
        if let Some(mut old) = guard.take() {
            old.stop();
        }
        let identity = self.identity.read().await.clone();
        let secure = self
            .settings
            .read()
            .await
            .passphrase
            .as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        let (tx, rx) = broadcast::channel(512);
        match Discovery::start(&identity, self.port, secure, self.disc_port, tx) {
            Ok(discovery) => {
                *guard = Some(discovery);
                drop(guard);
                tokio::spawn(discovery_loop(self.clone(), rx));
            }
            Err(err) => {
                tracing::warn!(?err, "discovery failed to start");
                self.emit(CoreEvent::Error {
                    context: "discovery".into(),
                    message: err.to_string(),
                });
            }
        }
    }

    pub(crate) async fn broadcast_presence(&self) {
        let identity = self.identity.read().await.clone();
        let body = Body::Presence(Presence {
            online: true,
            display_name: identity.display_name,
            avatar_color: identity.avatar_color,
            group_version: 0,
        });
        let peers: Vec<Uuid> = self.peers.read().await.keys().copied().collect();
        for peer in peers {
            self.send_body(peer, body.clone()).await;
        }
    }

    pub(crate) async fn conv_meta(&self, conv: Uuid) -> Result<(ConvKind, Vec<Uuid>)> {
        let me = self.my_id().await;
        if let Some(group) = self.store.get_group(conv) {
            let members = group.members.into_iter().filter(|p| *p != me).collect();
            return Ok((ConvKind::Group, members));
        }
        let exists = self.store.get_peer(conv).is_some();
        if !exists {
            return Err(CoreError::NotFound(format!("conversation {conv}")));
        }
        Ok((ConvKind::Direct, vec![conv]))
    }

    pub(crate) async fn targets_for(&self, conv: Uuid, kind: ConvKind) -> Vec<Uuid> {
        let me = self.my_id().await;
        match kind {
            ConvKind::Direct => vec![conv],
            ConvKind::Group => {
                if conv == EVERYONE_GROUP_ID {
                    let mut targets: Vec<Uuid> = self
                        .store
                        .list_peers()
                        .into_iter()
                        .map(|p| p.id)
                        .filter(|id| *id != me)
                        .collect();
                    for id in self.peers.read().await.keys() {
                        if *id != me && !targets.contains(id) {
                            targets.push(*id);
                        }
                    }
                    targets
                } else {
                    self.store
                        .get_group(conv)
                        .map(|g| g.members.into_iter().filter(|m| *m != me).collect())
                        .unwrap_or_default()
                }
            }
        }
    }

    pub(crate) async fn send_body(&self, peer: Uuid, body: Body) -> bool {
        let me = self.my_id().await;
        let env = Envelope::new(me, Some(peer), body);
        let encoded = match env.encode() {
            Ok(encoded) => encoded,
            Err(err) => {
                tracing::warn!(?err, "could not encode frame");
                return false;
            }
        };
        self.deliver(peer, env, &encoded, false).await
    }

    pub(crate) async fn send_envelope(&self, peer: Uuid, env: Envelope) -> bool {
        let tx = self
            .peers
            .read()
            .await
            .get(&peer)
            .map(|runtime| runtime.tx.clone());
        match tx {
            Some(tx) => tx.send(Outbound::Envelope(env)).await.is_ok(),
            None => false,
        }
    }

    pub(crate) async fn deliver(
        &self,
        peer: Uuid,
        env: Envelope,
        encoded: &str,
        queue_offline: bool,
    ) -> bool {
        if peer == self.my_id().await {
            return false;
        }
        let sent = self.send_envelope(peer, env).await;
        if !sent && queue_offline {
            if let Err(err) = self.store.enqueue_outbox(peer, encoded) {
                tracing::warn!(?err, "outbox enqueue failed");
            }
        }
        sent
    }

    pub(crate) async fn send_chat(
        &self,
        conv: Uuid,
        kind: MessageKind,
        text: Option<String>,
        file_id: Option<Uuid>,
        reply_to: Option<Uuid>,
        mentions: Vec<Uuid>,
        queue_offline: bool,
    ) -> Result<MessageRecord> {
        let me = self.identity.read().await.clone();
        let (conv_kind, _members) = self.conv_meta(conv).await?;
        let mut targets = self.targets_for(conv, conv_kind).await;
        targets.retain(|t| *t != me.peer_id);
        targets.dedup();

        let (title, avatar_color) = if let Some(group) = self.store.get_group(conv) {
            (group.name, group.avatar_color)
        } else {
            self.store
                .get_peer(conv)
                .map(|p| (p.display_name, p.avatar_color))
                .unwrap_or_default()
        };
        self.store
            .ensure_conversation(conv, conv_kind, &title, avatar_color)?;

        let ts = crate::now_ms();
        let mut message = MessageRecord {
            id: Uuid::new_v4(),
            conv,
            conv_kind,
            sender_id: me.peer_id,
            sender_name: me.display_name.clone(),
            kind,
            text: text.clone(),
            file_id,
            reply_to,
            mentions: mentions.clone(),
            ts,
            state: DeliveryState::Queued,
            outgoing: true,
        };
        self.store.insert_message(&message, false)?;

        let body = ChatMessage {
            conv,
            conv_kind,
            seq: self.store.next_seq(conv),
            kind,
            text,
            file_id,
            reply_to,
            mentions,
        };
        let mut any_sent = false;
        for target in targets {
            let env = Envelope {
                v: PROTOCOL_VERSION,
                id: message.id,
                from: me.peer_id,
                to: Some(target),
                ts,
                body: Body::Message(body.clone()),
            };
            let encoded = env.encode()?;
            if self
                .deliver(target, env, &encoded, queue_offline && conv != EVERYONE_GROUP_ID)
                .await
            {
                any_sent = true;
            }
        }

        if any_sent {
            self.store
                .update_message_state(&[message.id], DeliveryState::Sent);
            message.state = DeliveryState::Sent;
            self.emit(CoreEvent::MessageState {
                conv,
                ids: vec![message.id],
                state: DeliveryState::Sent,
            });
        }
        self.emit(CoreEvent::Message(message.clone()));
        if let Some(conversation) = self.store.conversation(conv) {
            self.emit(CoreEvent::ConversationUpdated(conversation));
        }
        Ok(message)
    }

    pub(crate) async fn send_to_members(&self, members: &[Uuid], body: Body) {
        let me = self.my_id().await;
        for member in members {
            if *member == me {
                continue;
            }
            let env = Envelope::new(me, Some(*member), body.clone());
            if let Ok(encoded) = env.encode() {
                self.deliver(*member, env, &encoded, true).await;
            }
        }
    }

    pub(crate) async fn push_group_update(
        &self,
        group_id: Uuid,
        op: GroupOp,
        member: Option<Uuid>,
    ) -> Result<()> {
        let group = self
            .store
            .get_group(group_id)
            .ok_or_else(|| CoreError::NotFound("group".into()))?;
        let update = GroupUpdate {
            group_id,
            op,
            name: Some(group.name.clone()),
            avatar_color: Some(group.avatar_color),
            member,
            group_version: group.version,
            group: Some(group_desc(&group)),
        };
        self.send_to_members(&group.members, Body::GroupUpdate(update))
            .await;
        self.emit(CoreEvent::GroupUpdated(group));
        Ok(())
    }

    async fn register(
        self: &Arc<Self>,
        mut peer: PeerSnapshot,
        tx: mpsc::Sender<Outbound>,
        conn_id: Uuid,
        initiator: bool,
        secure: bool,
    ) -> bool {
        let me = self.my_id().await;
        if peer.id == me {
            return false;
        }
        if let Some(existing) = self.store.get_peer(peer.id) {
            if peer.display_name.is_empty() {
                peer.display_name = existing.display_name;
            }
            if peer.avatar_color == 0 {
                peer.avatar_color = existing.avatar_color;
            }
            if peer.device_name.is_empty() {
                peer.device_name = existing.device_name;
            }
            if peer.addr.is_none() {
                peer.addr = existing.addr;
            }
            if peer.port.is_none() {
                peer.port = existing.port;
            }
        }
        peer.online = true;
        peer.last_seen = crate::now_ms();
        peer.secure = secure;

        let preferred_initiator = me < peer.id;
        let mut close_old = None;
        {
            let mut peers = self.peers.write().await;
            if let Some(existing) = peers.get(&peer.id) {
                // Same-direction duplicates keep the older connection; opposite
                // directions keep the one where the lower UUID dialed.
                let keep_new = initiator == preferred_initiator && existing.initiator != preferred_initiator;
                if !keep_new {
                    return false;
                }
                close_old = Some(existing.tx.clone());
            }
            peers.insert(
                peer.id,
                PeerRuntime {
                    snapshot: peer.clone(),
                    tx,
                    conn_id,
                    initiator,
                    last_seen: Instant::now(),
                },
            );
        }
        if let Some(tx) = close_old {
            let _ = tx.send(Outbound::Close).await;
        }
        if let Err(err) = self.store.upsert_peer(&peer) {
            tracing::warn!(?err, "peer upsert failed");
        }
        self.emit(CoreEvent::PeerUpdated(peer));
        true
    }

    async fn touch(&self, peer: Uuid) {
        if let Some(runtime) = self.peers.write().await.get_mut(&peer) {
            runtime.last_seen = Instant::now();
        }
    }

    async fn remove_connection(self: &Arc<Self>, peer: Uuid, conn_id: Uuid) {
        let removed = {
            let mut peers = self.peers.write().await;
            match peers.get(&peer) {
                Some(runtime) if runtime.conn_id == conn_id => {
                    peers.remove(&peer);
                    true
                }
                _ => false,
            }
        };
        if !removed {
            return;
        }
        let _ = self.store.set_peer_online(peer, false);
        self.emit(CoreEvent::PeerOffline(peer));
        if let Some(snapshot) = self.store.get_peer(peer) {
            self.emit(CoreEvent::PeerUpdated(snapshot.clone()));
            if snapshot.addr.is_some() && snapshot.port.is_some() {
                spawn_reconnect(self.clone(), snapshot);
            }
        }
    }

    async fn disconnect_all(&self) {
        let peers: Vec<(Uuid, mpsc::Sender<Outbound>)> = {
            let mut guard = self.peers.write().await;
            guard
                .drain()
                .map(|(id, runtime)| (id, runtime.tx))
                .collect()
        };
        for (id, tx) in peers {
            let _ = tx.send(Outbound::Close).await;
            let _ = self.store.set_peer_online(id, false);
        }
    }

    async fn after_connect(self: &Arc<Self>, peer: Uuid) {
        let identity = self.identity.read().await.clone();
        self.send_body(
            peer,
            Body::Presence(Presence {
                online: true,
                display_name: identity.display_name,
                avatar_color: identity.avatar_color,
                group_version: 0,
            }),
        )
        .await;

        let me = identity.peer_id;
        for group in self.store.list_groups() {
            if group.is_default || group.id == EVERYONE_GROUP_ID {
                continue;
            }
            if group.members.contains(&peer) && group.members.contains(&me) {
                let _ = self
                    .send_body(
                        peer,
                        Body::GroupCreate(GroupCreate {
                            group: group_desc(&group),
                        }),
                    )
                    .await;
            }
        }

        for (rowid, payload) in self.store.outbox_for(peer) {
            if rowid as usize > OUTBOX_FLUSH_LIMIT {
                break;
            }
            match Envelope::decode(&payload) {
                Ok(env) => {
                    if self.send_envelope(peer, env).await {
                        self.store.dequeue_outbox(rowid);
                    } else {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!(?err, "dropping corrupt outbox frame");
                    self.store.dequeue_outbox(rowid);
                }
            }
        }
        self.resume_transfers_for(peer).await;
    }

    async fn handle_envelope(self: &Arc<Self>, peer: Uuid, env: Envelope) -> Result<()> {
        let me = self.my_id().await;
        if env.v != PROTOCOL_VERSION {
            return Err(CoreError::Protocol(ProtocolError::UnsupportedVersion(env.v)));
        }
        if env.from != peer {
            return Err(CoreError::Invalid("envelope.from mismatch".into()));
        }
        if let Some(to) = env.to {
            if to != me {
                return Err(CoreError::Invalid("frame not addressed to this peer".into()));
            }
        }
        match env.body {
            Body::Presence(p) => {
                let snapshot = {
                    let mut peers = self.peers.write().await;
                    match peers.get_mut(&peer) {
                        Some(runtime) => {
                            if !p.display_name.is_empty() {
                                runtime.snapshot.display_name = p.display_name.clone();
                            }
                            if p.avatar_color != 0 {
                                runtime.snapshot.avatar_color = p.avatar_color;
                            }
                            runtime.snapshot.online = p.online;
                            runtime.last_seen = Instant::now();
                            Some(runtime.snapshot.clone())
                        }
                        None => None,
                    }
                };
                if let Some(snapshot) = snapshot {
                    let _ = self.store.upsert_peer(&snapshot);
                    self.emit(CoreEvent::PeerUpdated(snapshot));
                }
            }
            Body::Message(msg) => {
                self.handle_chat(peer, env.id, msg).await?;
            }
            Body::Ack(ack) => {
                self.store.update_message_state(&ack.message_ids, ack.state);
                self.emit(CoreEvent::MessageState {
                    conv: ack.conv,
                    ids: ack.message_ids,
                    state: ack.state,
                });
            }
            Body::Typing(t) => {
                let name = self
                    .store
                    .get_peer(peer)
                    .map(|p| p.display_name)
                    .unwrap_or_default();
                self.emit(CoreEvent::Typing {
                    peer,
                    peer_name: name,
                    conv: t.conv,
                    active: t.active,
                });
            }
            Body::GroupCreate(create) => {
                if create.group.id == EVERYONE_GROUP_ID {
                    return Ok(());
                }
                let record = GroupRecord {
                    id: create.group.id,
                    name: create.group.name,
                    avatar_color: create.group.avatar_color,
                    members: create.group.members,
                    created_by: create.group.created_by,
                    created_at: create.group.created_at,
                    is_default: false,
                    version: create.group.version.max(1),
                };
                self.store.upsert_group(&record)?;
                self.emit(CoreEvent::GroupUpdated(record));
            }
            Body::GroupUpdate(update) => {
                self.apply_group_update(update, peer).await?;
            }
            Body::FileOffer(offer) => {
                self.handle_file_offer(peer, offer).await?;
            }
            Body::FileChunk(chunk) => {
                self.handle_file_chunk(peer, chunk).await?;
            }
            Body::FileAck(ack) => {
                if let Some(sender) = self.ack_waiters.lock().get(&(ack.transfer_id, peer)).cloned() {
                    let _ = sender.try_send(ack.clone());
                }
                self.handle_file_ack(peer, ack).await?;
            }
            Body::Error(err) => {
                tracing::warn!(code = %err.code, message = %err.message, "peer reported an error");
                if err.fatal {
                    if let Some(runtime) = self.peers.read().await.get(&peer) {
                        let _ = runtime.tx.send(Outbound::Close).await;
                    }
                }
            }
            Body::Hello(_) | Body::HelloAck(_) => {
                tracing::debug!("ignoring handshake frame on established connection");
            }
            Body::Sealed(_) => {
                tracing::warn!("unexpected sealed wrapper after decryption");
            }
        }
        Ok(())
    }

    async fn handle_chat(self: &Arc<Self>, from: Uuid, id: Uuid, msg: ChatMessage) -> Result<()> {
        let me = self.my_id().await;
        if from == me {
            return Ok(());
        }
        if msg.conv_kind == ConvKind::Direct && msg.conv != from {
            tracing::warn!(%from, conv = %msg.conv, "direct message with mismatched conversation");
            return Ok(());
        }
        let peer = self
            .store
            .get_peer(from)
            .unwrap_or_else(|| PeerSnapshot::placeholder(from));
        let sender_name = if peer.display_name.is_empty() {
            "Unknown".to_string()
        } else {
            peer.display_name.clone()
        };

        let (title, avatar_color) = if msg.conv_kind == ConvKind::Group {
            match self.store.get_group(msg.conv) {
                Some(group) => (group.name, group.avatar_color),
                None => ("Group".to_string(), 0),
            }
        } else {
            (sender_name.clone(), peer.avatar_color)
        };
        self.store
            .ensure_conversation(msg.conv, msg.conv_kind, &title, avatar_color)?;

        let mentioned = msg.mentions.contains(&me);
        let record = MessageRecord {
            id,
            conv: msg.conv,
            conv_kind: msg.conv_kind,
            sender_id: from,
            sender_name,
            kind: msg.kind,
            text: msg.text.clone(),
            file_id: msg.file_id,
            reply_to: msg.reply_to,
            mentions: msg.mentions.clone(),
            ts: crate::now_ms(),
            state: DeliveryState::Delivered,
            outgoing: false,
        };
        let inserted = self.store.insert_message(&record, mentioned)?;
        if inserted {
            self.emit(CoreEvent::Message(record));
            if let Some(conversation) = self.store.conversation(msg.conv) {
                self.emit(CoreEvent::ConversationUpdated(conversation));
            }
        }
        self.send_body(
            from,
            Body::Ack(Ack {
                conv: msg.conv,
                message_ids: vec![id],
                state: DeliveryState::Delivered,
            }),
        )
        .await;
        Ok(())
    }

    async fn apply_group_update(self: &Arc<Self>, update: GroupUpdate, from: Uuid) -> Result<()> {
        let me = self.my_id().await;
        if update.group_id == EVERYONE_GROUP_ID {
            return Ok(());
        }
        if let Some(full) = &update.group {
            let group = GroupRecord {
                id: full.id,
                name: full.name.clone(),
                avatar_color: full.avatar_color,
                members: full.members.clone(),
                created_by: full.created_by,
                created_at: full.created_at,
                is_default: false,
                version: full.version.max(update.group_version),
            };
            self.store.upsert_group(&group)?;
            self.emit(CoreEvent::GroupUpdated(group));
            return Ok(());
        }
        let Some(mut group) = self.store.get_group(update.group_id) else {
            return Ok(());
        };
        if update.group_version < group.version && update.op != GroupOp::Sync {
            return Ok(());
        }
        match update.op {
            GroupOp::Rename => {
                if let Some(name) = update.name {
                    group.name = name;
                }
                if let Some(color) = update.avatar_color {
                    group.avatar_color = color;
                }
            }
            GroupOp::AddMember => {
                if let Some(member) = update.member {
                    if !group.members.contains(&member) {
                        group.members.push(member);
                    }
                }
            }
            GroupOp::RemoveMember | GroupOp::Leave => {
                if let Some(member) = update.member {
                    group.members.retain(|m| *m != member);
                }
            }
            GroupOp::Sync => {}
        }
        group.version = group.version.max(update.group_version);
        if group.members.is_empty() {
            self.store.delete_group(update.group_id)?;
            return Ok(());
        }
        self.store.upsert_group(&group)?;
        self.emit(CoreEvent::GroupUpdated(group));
        let _ = (from, me);
        Ok(())
    }

    pub(crate) async fn is_connected(&self, peer: Uuid) -> bool {
        self.peers.read().await.contains_key(&peer)
    }

    async fn on_discovered(self: &Arc<Self>, announcement: crate::Announcement, addr: IpAddr) {
        let me = self.my_id().await;
        if announcement.peer_id == me {
            return;
        }
        let connected = self.is_connected(announcement.peer_id).await;
        let mut snapshot = self
            .store
            .get_peer(announcement.peer_id)
            .unwrap_or_else(|| PeerSnapshot::placeholder(announcement.peer_id));
        snapshot.display_name = announcement.display_name;
        snapshot.avatar_color = announcement.avatar_color;
        snapshot.device_name = announcement.device_name;
        snapshot.last_seen = crate::now_ms();
        snapshot.online = connected;
        snapshot.addr = Some(addr.to_string());
        snapshot.port = Some(announcement.port);
        snapshot.secure = announcement.secure;
        let _ = self.store.upsert_peer(&snapshot);
        self.emit(CoreEvent::PeerUpdated(snapshot.clone()));
        if !connected {
            spawn_dial(self.clone(), snapshot);
        }
    }
}

async fn accept_loop(inner: Arc<Inner>, listener: TcpListener) {
    loop {
        if inner.shutdown.load(Ordering::Relaxed) {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(800), listener.accept()).await {
            Ok(Ok((stream, addr))) => {
                let inner = inner.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(HANDSHAKE_TIMEOUT, accept_async(stream)).await {
                        Ok(Ok(ws)) => {
                            if let Err(err) = run_connection(
                                inner,
                                ws,
                                false,
                                None,
                                addr.ip(),
                            )
                            .await
                            {
                                tracing::debug!(?err, %addr, "incoming connection ended");
                            }
                        }
                        Ok(Err(err)) => tracing::debug!(?err, %addr, "websocket accept failed"),
                        Err(_) => tracing::debug!(%addr, "websocket handshake timed out"),
                    }
                });
            }
            Ok(Err(err)) => {
                tracing::warn!(?err, "tcp accept failed");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(_) => continue,
        }
    }
}

async fn discovery_loop(inner: Arc<Inner>, mut rx: broadcast::Receiver<DiscoveryEvent>) {
    loop {
        match rx.recv().await {
            Ok(DiscoveryEvent::Found { announcement, addr }) => {
                inner.clone().on_discovered(announcement, addr).await;
            }
            Ok(DiscoveryEvent::Lost(peer)) => {
                if !inner.is_connected(peer).await {
                    let _ = inner.store.set_peer_online(peer, false);
                    inner.emit(CoreEvent::PeerOffline(peer));
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn spawn_dial(inner: Arc<Inner>, peer: PeerSnapshot) {
    tokio::spawn(async move { attempt_connect(inner, peer).await });
}

fn spawn_reconnect(inner: Arc<Inner>, peer: PeerSnapshot) {
    tokio::spawn(async move {
        for delay in [2u64, 4, 8, 16, 30] {
            if inner.shutdown.load(Ordering::Relaxed) {
                return;
            }
            tokio::time::sleep(Duration::from_secs(delay)).await;
            if inner.is_connected(peer.id).await {
                return;
            }
            attempt_connect(inner.clone(), peer.clone()).await;
            if inner.is_connected(peer.id).await {
                return;
            }
        }
    });
}

async fn attempt_connect(inner: Arc<Inner>, peer: PeerSnapshot) {
    if inner.is_connected(peer.id).await {
        return;
    }
    {
        let mut last = inner.last_dial.lock();
        if let Some(at) = last.get(&peer.id) {
            if at.elapsed() < DIAL_COOLDOWN {
                return;
            }
        }
        last.insert(peer.id, Instant::now());
    }
    let (Some(addr), Some(port)) = (peer.addr.as_deref(), peer.port) else {
        return;
    };
    let Ok(ip) = addr.parse::<IpAddr>() else {
        return;
    };
    let url = if ip.is_ipv6() {
        format!("ws://[{ip}]:{port}/lanchat")
    } else {
        format!("ws://{ip}:{port}/lanchat")
    };
    tracing::debug!(%url, "dialing peer");
    match tokio::time::timeout(Duration::from_secs(8), connect_async(url)).await {
        Ok(Ok((ws, _))) => {
            if let Err(err) = run_connection(inner.clone(), ws, true, Some(peer), ip).await {
                tracing::debug!(?err, "outgoing connection ended");
            }
        }
        Ok(Err(err)) => tracing::debug!(?err, "dial failed"),
        Err(_) => tracing::debug!("dial timed out"),
    }
}

async fn heartbeat_loop(inner: Arc<Inner>) {
    let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if inner.shutdown.load(Ordering::Relaxed) {
            break;
        }
        let identity = inner.identity.read().await.clone();
        let stale: Vec<(Uuid, Uuid)> = {
            let peers = inner.peers.read().await;
            peers
                .iter()
                .filter(|(_, runtime)| runtime.last_seen.elapsed() > PEER_TIMEOUT)
                .map(|(id, runtime)| (*id, runtime.conn_id))
                .collect()
        };
        for (peer, conn) in stale {
            tracing::info!(%peer, "peer timed out");
            if let Some(runtime) = inner.peers.read().await.get(&peer) {
                if runtime.conn_id == conn {
                    let _ = runtime.tx.send(Outbound::Close).await;
                }
            }
            inner.remove_connection(peer, conn).await;
        }
        let presence = Body::Presence(Presence {
            online: true,
            display_name: identity.display_name,
            avatar_color: identity.avatar_color,
            group_version: 0,
        });
        let targets: Vec<Uuid> = inner.peers.read().await.keys().copied().collect();
        for peer in targets {
            inner.send_body(peer, presence.clone()).await;
        }
    }
}

async fn netmon_loop(inner: Arc<Inner>) {
    let mut last = network_fingerprint();
    loop {
        tokio::time::sleep(Duration::from_secs(8)).await;
        if inner.shutdown.load(Ordering::Relaxed) {
            break;
        }
        let fingerprint = network_fingerprint();
        if fingerprint != last {
            last = fingerprint;
            tracing::info!("network interfaces changed, re-announcing");
            inner.emit(CoreEvent::NetworkChanged);
            let peers = inner.store.list_peers();
            for peer in peers {
                if !inner.is_connected(peer.id).await {
                    spawn_dial(inner.clone(), peer);
                }
            }
        }
    }
}

struct Handshake {
    peer: PeerSnapshot,
    cipher: Option<SessionCipher>,
    conn_id: Uuid,
}

async fn run_connection<S>(
    inner: Arc<Inner>,
    mut ws: WebSocketStream<S>,
    initiator: bool,
    expected: Option<PeerSnapshot>,
    addr: IpAddr,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let handshake = handshake(&inner, &mut ws, initiator, expected.as_ref(), addr).await?;
    let peer = handshake.peer;
    let conn_id = handshake.conn_id;
    let cipher = handshake.cipher;

    let (tx, rx) = mpsc::channel::<Outbound>(512);
    let accepted = inner
        .clone()
        .register(peer.clone(), tx.clone(), conn_id, initiator, cipher.is_some())
        .await;
    if !accepted {
        tracing::debug!(peer = %peer.id, "duplicate connection dropped");
        drop(tx);
        let _ = ws.close(None).await;
        return Ok(());
    }

    let (sink, stream) = ws.split();
    let writer = tokio::spawn(writer_loop(sink, rx, cipher.clone()));
    let connected = inner.clone();
    tokio::spawn(async move {
        connected.after_connect(peer.id).await;
    });

    let result = read_loop(inner.clone(), peer.id, stream, cipher).await;
    writer.abort();
    inner.remove_connection(peer.id, conn_id).await;
    result
}

async fn handshake<S>(
    inner: &Arc<Inner>,
    ws: &mut WebSocketStream<S>,
    initiator: bool,
    expected: Option<&PeerSnapshot>,
    addr: IpAddr,
) -> Result<Handshake>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let me = inner.identity.read().await.clone();
    let passphrase = inner
        .settings
        .read()
        .await
        .passphrase
        .clone()
        .filter(|p| !p.is_empty());

    if initiator {
        let expected_id = expected
            .map(|p| p.id)
            .ok_or_else(|| CoreError::Invalid("dialing without a peer id".into()))?;
        let conn_id = Uuid::new_v4();
        let salt = new_salt();
        let mut cipher = None;
        let mut proof = None;
        let mut salt_b64 = None;
        if let Some(passphrase) = &passphrase {
            let session = SessionCipher::from_passphrase(passphrase, &salt)?;
            proof = Some(session.hello_proof(me.peer_id, expected_id)?);
            salt_b64 = Some(B64.encode(salt));
            cipher = Some(session);
        }
        let hello = Hello {
            peer_id: me.peer_id,
            display_name: me.display_name.clone(),
            avatar_color: me.avatar_color,
            device_name: me.device_name.clone(),
            app_version: crate::APP_VERSION.to_string(),
            capabilities: capabilities(),
            secure: cipher.is_some(),
            salt_b64,
            proof,
            conn_id,
        };
        let envelope = Envelope::new(me.peer_id, Some(expected_id), Body::Hello(hello));
        ws.send(Message::Text(envelope.encode()?.into())).await?;

        let raw = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_text(ws))
            .await
            .map_err(|_| CoreError::Invalid("handshake timed out".into()))??;
        let envelope = Envelope::decode(&raw)?;
        envelope.validate()?;
        let Body::HelloAck(ack) = envelope.body else {
            return Err(reject(ws, me.peer_id, "expected_hello_ack", "handshake order violation").await);
        };
        if ack.peer_id != expected_id {
            return Err(reject(ws, me.peer_id, "identity_mismatch", "peer id differs from discovery").await);
        }
        if ack.peer_id == me.peer_id {
            return Err(reject(ws, me.peer_id, "self_connection", "cannot connect to self").await);
        }
        if ack.secure != cipher.is_some() {
            return Err(reject(
                ws,
                me.peer_id,
                "passphrase_mismatch",
                "one side uses a LAN passphrase and the other does not",
            )
            .await);
        }
        if let Some(session) = &cipher {
            let Some(proof) = ack.proof.as_ref() else {
                return Err(reject(ws, me.peer_id, "missing_proof", "secure hello_ack has no proof").await);
            };
            session
                .verify_ack_proof(proof, ack.peer_id, me.peer_id)
                .map_err(|_| CoreError::Crypto("invalid passphrase proof".into()))?;
        }
        let mut peer = PeerSnapshot {
            id: ack.peer_id,
            display_name: ack.display_name,
            avatar_color: ack.avatar_color,
            device_name: ack.device_name,
            online: true,
            last_seen: crate::now_ms(),
            addr: Some(addr.to_string()),
            port: expected.and_then(|p| p.port),
            secure: ack.secure,
        };
        if peer.display_name.is_empty() {
            if let Some(e) = expected {
                peer.display_name = e.display_name.clone();
            }
        }
        Ok(Handshake { peer, cipher, conn_id })
    } else {
        let raw = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_text(ws))
            .await
            .map_err(|_| CoreError::Invalid("handshake timed out".into()))??;
        let envelope = Envelope::decode(&raw)?;
        envelope.validate()?;
        let Body::Hello(hello) = envelope.body else {
            return Err(reject(ws, me.peer_id, "expected_hello", "first frame must be hello").await);
        };
        if hello.peer_id != envelope.from {
            return Err(reject(ws, me.peer_id, "identity_mismatch", "hello.peer_id must equal from").await);
        }
        if hello.peer_id == me.peer_id {
            return Err(reject(ws, me.peer_id, "self_connection", "cannot connect to self").await);
        }
        let cipher = match (&passphrase, hello.secure) {
            (Some(passphrase), true) => {
                let salt_b64 = hello
                    .salt_b64
                    .as_deref()
                    .ok_or_else(|| CoreError::Invalid("secure hello missing salt".into()))?;
                let salt = B64
                    .decode(salt_b64)
                    .map_err(|_| CoreError::Invalid("hello salt is not base64".into()))?;
                if salt.len() != SALT_LEN {
                    return Err(reject(ws, me.peer_id, "bad_salt", "hello salt has the wrong length").await);
                }
                let salt_array: [u8; SALT_LEN] = salt
                    .try_into()
                    .map_err(|_| CoreError::Invalid("hello salt has the wrong length".into()))?;
                let session = SessionCipher::from_passphrase(passphrase, &salt_array)?;
                let Some(proof) = hello.proof.as_ref() else {
                    return Err(reject(ws, me.peer_id, "missing_proof", "secure hello has no proof").await);
                };
                session
                    .verify_hello_proof(proof, hello.peer_id, me.peer_id)
                    .map_err(|_| {
                        CoreError::Crypto("passphrase proof did not verify".into())
                    })?;
                Some(session)
            }
            (Some(_), false) => {
                return Err(reject(
                    ws,
                    me.peer_id,
                    "passphrase_required",
                    "this device requires a LAN passphrase but the peer has none",
                )
                .await)
            }
            (None, true) => {
                return Err(reject(
                    ws,
                    me.peer_id,
                    "passphrase_not_configured",
                    "peer uses a passphrase but this device has none",
                )
                .await)
            }
            (None, false) => None,
        };
        let ack = HelloAck {
            peer_id: me.peer_id,
            display_name: me.display_name.clone(),
            avatar_color: me.avatar_color,
            device_name: me.device_name.clone(),
            app_version: crate::APP_VERSION.to_string(),
            capabilities: capabilities(),
            secure: cipher.is_some(),
            proof: match &cipher {
                Some(session) => Some(session.ack_proof(me.peer_id, hello.peer_id)?),
                None => None,
            },
            group_version: 0,
            conn_id: hello.conn_id,
        };
        let ack_env = Envelope::new(me.peer_id, Some(hello.peer_id), Body::HelloAck(ack));
        ws.send(Message::Text(ack_env.encode()?.into())).await?;

        let existing = inner.store.get_peer(hello.peer_id);
        Ok(Handshake {
            peer: PeerSnapshot {
                id: hello.peer_id,
                display_name: hello.display_name,
                avatar_color: hello.avatar_color,
                device_name: hello.device_name,
                online: true,
                last_seen: crate::now_ms(),
                addr: Some(addr.to_string()),
                port: existing.and_then(|p| p.port),
                secure: cipher.is_some(),
            },
            cipher,
            conn_id: hello.conn_id,
        })
    }
}

async fn reject<S>(
    ws: &mut WebSocketStream<S>,
    from: Uuid,
    code: &str,
    message: &str,
) -> CoreError
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let envelope = Envelope::new(from, None, Body::Error(ErrorFrame::fatal(code, message)));
    if let Ok(raw) = envelope.encode() {
        let _ = ws.send(Message::Text(raw.into())).await;
    }
    let _ = ws.close(None).await;
    CoreError::Invalid(format!("{code}: {message}"))
}

async fn read_text<S>(ws: &mut WebSocketStream<S>) -> Result<String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => return Ok(text.to_string()),
            Some(Ok(Message::Binary(_))) => {
                return Err(CoreError::Invalid("binary frames are not supported".into()))
            }
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => continue,
            Some(Ok(Message::Close(_))) | None => {
                return Err(CoreError::Invalid("connection closed during handshake".into()))
            }
            Some(Ok(Message::Frame(_))) => continue,
            Some(Err(err)) => return Err(err.into()),
        }
    }
}

async fn writer_loop<S>(
    mut sink: S,
    mut rx: mpsc::Receiver<Outbound>,
    cipher: Option<SessionCipher>,
) where
    S: futures_util::Sink<Message, Error = tungstenite::Error> + Unpin,
{
    while let Some(outbound) = rx.recv().await {
        match outbound {
            Outbound::Close => {
                let _ = sink.close().await;
                break;
            }
            Outbound::Envelope(envelope) => {
                let from = envelope.from;
                let to = envelope.to;
                let raw = match envelope.encode() {
                    Ok(raw) => raw,
                    Err(err) => {
                        tracing::warn!(?err, "could not encode outgoing frame");
                        continue;
                    }
                };
                let payload = match &cipher {
                    Some(session) => match session.seal_frame(raw.as_bytes()) {
                        Ok(sealed) => {
                            let wrapped = Envelope {
                                v: PROTOCOL_VERSION,
                                id: Uuid::new_v4(),
                                from,
                                to,
                                ts: crate::now_ms(),
                                body: Body::Sealed(sealed),
                            };
                            match wrapped.encode() {
                                Ok(raw) => raw,
                                Err(err) => {
                                    tracing::warn!(?err, "could not encode sealed frame");
                                    continue;
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!(?err, "frame encryption failed");
                            continue;
                        }
                    },
                    None => raw,
                };
                if sink.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn read_loop<S>(
    inner: Arc<Inner>,
    peer: Uuid,
    mut stream: S,
    cipher: Option<SessionCipher>,
) -> Result<()>
where
    S: futures_util::Stream<Item = std::result::Result<Message, tungstenite::Error>> + Unpin,
{
    while let Some(frame) = stream.next().await {
        let frame = match frame {
            Ok(frame) => frame,
            Err(err) => {
                tracing::debug!(?err, %peer, "connection read error");
                break;
            }
        };
        match frame {
            Message::Text(text) => {
                let envelope = match decode_frame(&text, &cipher) {
                    Ok(envelope) => envelope,
                    Err(err) => {
                        tracing::warn!(?err, %peer, "dropping invalid frame");
                        continue;
                    }
                };
                if let Err(err) = envelope.validate() {
                    tracing::warn!(?err, %peer, "dropping invalid envelope");
                    continue;
                }
                inner.touch(peer).await;
                if let Err(err) = inner.handle_envelope(peer, envelope).await {
                    tracing::warn!(?err, %peer, "failed to handle frame");
                }
            }
            Message::Close(_) => break,
            Message::Binary(_) => tracing::warn!(%peer, "ignoring binary frame"),
            _ => {}
        }
    }
    Ok(())
}

fn decode_frame(raw: &str, cipher: &Option<SessionCipher>) -> Result<Envelope> {
    let envelope = Envelope::decode(raw)?;
    match cipher {
        Some(session) => match &envelope.body {
            Body::Sealed(sealed) => {
                let plain = session.open_frame(sealed)?;
                let text = String::from_utf8(plain)
                    .map_err(|_| CoreError::Invalid("sealed frame is not utf-8".into()))?;
                Ok(Envelope::decode(&text)?)
            }
            _ => Err(CoreError::Invalid(
                "plaintext frame on an encrypted connection".into(),
            )),
        },
        None => match &envelope.body {
            Body::Sealed(_) => Err(CoreError::Invalid(
                "received a sealed frame without a session key".into(),
            )),
            _ => Ok(envelope),
        },
    }
}

fn capabilities() -> Vec<String> {
    vec![
        "files".into(),
        "groups".into(),
        "typing".into(),
        "read_receipts".into(),
        "encryption".into(),
    ]
}

pub(crate) fn group_desc(group: &GroupRecord) -> GroupDesc {
    GroupDesc {
        id: group.id,
        name: group.name.clone(),
        avatar_color: group.avatar_color,
        members: group.members.clone(),
        created_by: group.created_by,
        created_at: group.created_at,
        version: group.version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_frame_roundtrip() {
        let salt = new_salt();
        let cipher = SessionCipher::from_passphrase("shared secret", &salt).unwrap();
        let inner = Envelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Body::Typing(Typing {
                conv: Uuid::new_v4(),
                active: true,
            }),
        );
        let raw = inner.encode().unwrap();
        let sealed = cipher.seal_frame(raw.as_bytes()).unwrap();
        let outer = Envelope::new(inner.from, inner.to, Body::Sealed(sealed));
        let outer_raw = outer.encode().unwrap();
        let decoded = decode_frame(&outer_raw, &Some(cipher)).unwrap();
        assert_eq!(decoded.id, inner.id);
        assert_eq!(decoded.body, inner.body);
    }

    #[test]
    fn plaintext_frame_is_rejected_on_secure_session() {
        let salt = new_salt();
        let cipher = SessionCipher::from_passphrase("shared secret", &salt).unwrap();
        let env = Envelope::new(
            Uuid::new_v4(),
            None,
            Body::Typing(Typing {
                conv: Uuid::new_v4(),
                active: false,
            }),
        );
        let raw = env.encode().unwrap();
        assert!(decode_frame(&raw, &Some(cipher)).is_err());
    }

    #[test]
    fn sealed_frame_without_key_is_rejected() {
        let salt = new_salt();
        let cipher = SessionCipher::from_passphrase("shared secret", &salt).unwrap();
        let sealed = cipher.seal_frame(b"{}").unwrap();
        let env = Envelope::new(Uuid::new_v4(), None, Body::Sealed(sealed));
        let raw = env.encode().unwrap();
        assert!(decode_frame(&raw, &None).is_err());
    }

    #[test]
    fn group_desc_preserves_version_and_members() {
        let member = Uuid::new_v4();
        let group = GroupRecord {
            id: Uuid::new_v4(),
            name: "Test".into(),
            avatar_color: 1,
            members: vec![member],
            created_by: Uuid::nil(),
            created_at: 0,
            is_default: false,
            version: 3,
        };
        let desc = group_desc(&group);
        assert_eq!(desc.name, "Test");
        assert_eq!(desc.version, 3);
        assert_eq!(desc.members, vec![member]);
    }

    #[test]
    fn outbox_roundtrip_keeps_envelope_intact() {
        let env = Envelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Body::Presence(Presence {
                online: true,
                display_name: "A".into(),
                avatar_color: 0,
                group_version: 0,
            }),
        );
        let raw = env.encode().unwrap();
        let decoded = Envelope::decode(&raw).unwrap();
        assert_eq!(decoded.body, env.body);
    }
}
