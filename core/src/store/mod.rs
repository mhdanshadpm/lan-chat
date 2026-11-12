pub mod models;

pub use models::*;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use uuid::Uuid;

use crate::config::Settings;
use crate::error::Result;
use crate::protocol::{ConvKind, DeliveryState, MessageKind, EVERYONE_GROUP_ID};

const MIGRATION_V1: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS peers (
    id            TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL DEFAULT '',
    avatar_color  INTEGER NOT NULL DEFAULT 0,
    device_name   TEXT NOT NULL DEFAULT '',
    last_seen     INTEGER NOT NULL DEFAULT 0,
    online        INTEGER NOT NULL DEFAULT 0,
    addr          TEXT,
    port          INTEGER,
    secure        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS conversations (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    title         TEXT NOT NULL DEFAULT '',
    avatar_color  INTEGER NOT NULL DEFAULT 0,
    unread        INTEGER NOT NULL DEFAULT 0,
    muted         INTEGER NOT NULL DEFAULT 0,
    last_ts       INTEGER NOT NULL DEFAULT 0,
    last_preview  TEXT NOT NULL DEFAULT '',
    last_sender   TEXT NOT NULL DEFAULT '',
    mentioned     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS groups (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    avatar_color  INTEGER NOT NULL DEFAULT 0,
    created_by    TEXT NOT NULL DEFAULT '',
    created_at    INTEGER NOT NULL DEFAULT 0,
    is_default    INTEGER NOT NULL DEFAULT 0,
    version       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS group_members (
    group_id      TEXT NOT NULL,
    peer_id       TEXT NOT NULL,
    joined_at     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, peer_id)
);

CREATE TABLE IF NOT EXISTS messages (
    id            TEXT PRIMARY KEY,
    conv_id       TEXT NOT NULL,
    conv_kind     TEXT NOT NULL,
    sender_id     TEXT NOT NULL,
    sender_name   TEXT NOT NULL DEFAULT '',
    kind          TEXT NOT NULL,
    text          TEXT,
    file_id       TEXT,
    reply_to      TEXT,
    mentions      TEXT NOT NULL DEFAULT '[]',
    ts            INTEGER NOT NULL,
    seq           INTEGER NOT NULL DEFAULT 0,
    state         TEXT NOT NULL,
    direction     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS messages_conv_ts ON messages (conv_id, ts DESC);
CREATE INDEX IF NOT EXISTS messages_ts ON messages (ts DESC);

CREATE TABLE IF NOT EXISTS files (
    id            TEXT PRIMARY KEY,
    message_id    TEXT,
    conv_id       TEXT NOT NULL,
    peer_id       TEXT,
    direction     TEXT NOT NULL,
    name          TEXT NOT NULL,
    path          TEXT,
    thumb_path    TEXT,
    mime          TEXT NOT NULL DEFAULT 'application/octet-stream',
    size          INTEGER NOT NULL DEFAULT 0,
    received      INTEGER NOT NULL DEFAULT 0,
    sha256        TEXT NOT NULL DEFAULT '',
    state         TEXT NOT NULL,
    next_index    INTEGER NOT NULL DEFAULT 0,
    chunk_size    INTEGER NOT NULL DEFAULT 0,
    chunks        INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS outbox (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id       TEXT NOT NULL,
    payload       TEXT NOT NULL,
    created_at    INTEGER NOT NULL DEFAULT 0,
    attempts      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS outbox_peer ON outbox (peer_id);

CREATE TABLE IF NOT EXISTS settings (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL
);
"#;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(MIGRATION_V1)?;
        Ok(())
    }

    // ---- settings -------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .lock()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn load_settings(&self) -> Settings {
        self.get_setting("settings")
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let raw = serde_json::to_string(settings)?;
        self.set_setting("settings", &raw)
    }

    // ---- peers ----------------------------------------------------------

    pub fn upsert_peer(&self, peer: &PeerSnapshot) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO peers (id, display_name, avatar_color, device_name, last_seen, online, addr, port, secure)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET
                display_name = CASE WHEN excluded.display_name <> '' THEN excluded.display_name ELSE peers.display_name END,
                avatar_color = CASE WHEN excluded.avatar_color <> 0 THEN excluded.avatar_color ELSE peers.avatar_color END,
                device_name  = CASE WHEN excluded.device_name <> '' THEN excluded.device_name ELSE peers.device_name END,
                last_seen    = MAX(peers.last_seen, excluded.last_seen),
                online       = excluded.online,
                addr         = COALESCE(excluded.addr, peers.addr),
                port         = COALESCE(excluded.port, peers.port),
                secure       = excluded.secure",
            params![
                peer.id.to_string(),
                peer.display_name,
                peer.avatar_color as i64,
                peer.device_name,
                peer.last_seen,
                peer.online as i64,
                peer.addr,
                peer.port.map(|p| p as i64),
                peer.secure as i64,
            ],
        )?;
        // Keep direct conversation titles in sync with the display name.
        conn.execute(
            "UPDATE conversations SET title = ?2, avatar_color = ?3 WHERE id = ?1 AND kind = 'direct' AND ?2 <> ''",
            params![peer.id.to_string(), peer.display_name, peer.avatar_color as i64],
        )?;
        Ok(())
    }

    pub fn set_peer_online(&self, id: Uuid, online: bool) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE peers SET online = ?2 WHERE id = ?1",
            params![id.to_string(), online as i64],
        )?;
        Ok(())
    }

    pub fn set_all_peers_offline(&self) -> Result<()> {
        self.conn
            .lock()
            .execute("UPDATE peers SET online = 0", [])?;
        Ok(())
    }

    pub fn get_peer(&self, id: Uuid) -> Option<PeerSnapshot> {
        self.conn
            .lock()
            .query_row(
                "SELECT id, display_name, avatar_color, device_name, online, last_seen, addr, port, secure
                 FROM peers WHERE id = ?1",
                [id.to_string()],
                peer_from_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn list_peers(&self) -> Vec<PeerSnapshot> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, display_name, avatar_color, device_name, online, last_seen, addr, port, secure
             FROM peers ORDER BY online DESC, display_name COLLATE NOCASE ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], peer_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    // ---- conversations --------------------------------------------------

    pub fn ensure_conversation(
        &self,
        id: Uuid,
        kind: ConvKind,
        title: &str,
        avatar_color: u32,
    ) -> Result<()> {
        let kind_str = conv_kind_str(kind);
        self.conn.lock().execute(
            "INSERT INTO conversations (id, kind, title, avatar_color) VALUES (?1,?2,?3,?4)
             ON CONFLICT(id) DO UPDATE SET
                title = CASE WHEN conversations.title = '' THEN excluded.title ELSE conversations.title END,
                avatar_color = CASE WHEN conversations.avatar_color = 0 THEN excluded.avatar_color ELSE conversations.avatar_color END",
            params![id.to_string(), kind_str, title, avatar_color as i64],
        )?;
        Ok(())
    }

    pub fn conversation(&self, id: Uuid) -> Option<ConversationRecord> {
        self.list_conversations()
            .into_iter()
            .find(|c| c.id == id)
    }

    pub fn list_conversations(&self) -> Vec<ConversationRecord> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT c.id, c.kind,
                    CASE WHEN c.kind = 'direct' THEN COALESCE(NULLIF(p.display_name, ''), c.title) ELSE c.title END AS title,
                    CASE WHEN c.kind = 'direct' AND p.avatar_color IS NOT NULL AND p.avatar_color <> 0
                         THEN p.avatar_color ELSE c.avatar_color END AS avatar_color,
                    c.unread, c.muted, c.last_ts, c.last_preview, c.last_sender, c.mentioned,
                    p.online,
                    COALESCE((SELECT COUNT(*) FROM group_members gm WHERE gm.group_id = c.id), 0) AS member_count
             FROM conversations c
             LEFT JOIN peers p ON p.id = c.id
             ORDER BY c.last_ts DESC, c.title COLLATE NOCASE ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        let everyone = EVERYONE_GROUP_ID.to_string();
        let rows = stmt.query_map([], |row| conversation_from_row(row, &everyone));
        rows.map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn conversation_muted(&self, conv: Uuid) -> bool {
        self.conn
            .lock()
            .query_row(
                "SELECT muted FROM conversations WHERE id = ?1",
                [conv.to_string()],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
            != 0
    }

    pub fn set_conversation_muted(&self, conv: Uuid, muted: bool) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE conversations SET muted = ?2 WHERE id = ?1",
            params![conv.to_string(), muted as i64],
        )?;
        Ok(())
    }

    pub fn last_active_conversation(&self) -> Option<Uuid> {
        self.conn
            .lock()
            .query_row(
                "SELECT id FROM conversations ORDER BY last_ts DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok())
    }

    pub fn unread_total(&self) -> u32 {
        self.conn
            .lock()
            .query_row(
                "SELECT COALESCE(SUM(unread), 0) FROM conversations",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) as u32
    }

    // ---- messages -------------------------------------------------------

    pub fn next_seq(&self, conv: Uuid) -> i64 {
        self.conn
            .lock()
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM messages WHERE conv_id = ?1",
                [conv.to_string()],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(1)
    }

    /// Returns `false` when the message id already existed (dedupe).
    pub fn insert_message(&self, msg: &MessageRecord, mentioned_me: bool) -> Result<bool> {
        let conn = self.conn.lock();
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO messages
                (id, conv_id, conv_kind, sender_id, sender_name, kind, text, file_id, reply_to, mentions, ts, state, direction)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                msg.id.to_string(),
                msg.conv.to_string(),
                conv_kind_str(msg.conv_kind),
                msg.sender_id.to_string(),
                msg.sender_name,
                message_kind_str(msg.kind),
                msg.text,
                msg.file_id.map(|id| id.to_string()),
                msg.reply_to.map(|id| id.to_string()),
                serde_json::to_string(&msg.mentions)?,
                msg.ts,
                msg.state.as_str(),
                if msg.outgoing { "outgoing" } else { "incoming" },
            ],
        )?;
        if inserted == 0 {
            return Ok(false);
        }
        let muted: bool = conn
            .query_row(
                "SELECT muted FROM conversations WHERE id = ?1",
                [msg.conv.to_string()],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
            != 0;
        let unread_delta = if msg.outgoing || muted { 0 } else { 1 };
        let title = if msg.conv_kind == ConvKind::Direct {
            msg.sender_name.clone()
        } else {
            String::new()
        };
        let preview = msg.preview();
        conn.execute(
            "INSERT INTO conversations (id, kind, title, last_ts, last_preview, last_sender, unread, mentioned)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
                last_ts      = MAX(conversations.last_ts, excluded.last_ts),
                last_preview = CASE WHEN excluded.last_ts >= conversations.last_ts THEN excluded.last_preview ELSE conversations.last_preview END,
                last_sender  = CASE WHEN excluded.last_ts >= conversations.last_ts THEN excluded.last_sender ELSE conversations.last_sender END,
                unread       = conversations.unread + ?7,
                mentioned    = MAX(conversations.mentioned, ?8),
                title        = CASE WHEN conversations.title = '' THEN excluded.title ELSE conversations.title END",
            params![
                msg.conv.to_string(),
                conv_kind_str(msg.conv_kind),
                title,
                msg.ts,
                preview,
                if msg.outgoing { "" } else { msg.sender_name.as_str() },
                unread_delta as i64,
                (mentioned_me && !msg.outgoing) as i64,
            ],
        )?;
        Ok(true)
    }

    pub fn list_messages(&self, conv: Uuid, before_ts: Option<i64>, limit: u32) -> Vec<MessageRecord> {
        let conn = self.conn.lock();
        let limit = limit.clamp(1, 500) as i64;
        let sql = if before_ts.is_some() {
            "SELECT id, conv_id, conv_kind, sender_id, sender_name, kind, text, file_id, reply_to, mentions, ts, state, direction
             FROM messages WHERE conv_id = ?1 AND ts < ?2 ORDER BY ts DESC LIMIT ?3"
        } else {
            "SELECT id, conv_id, conv_kind, sender_id, sender_name, kind, text, file_id, reply_to, mentions, ts, state, direction
             FROM messages WHERE conv_id = ?1 ORDER BY ts DESC LIMIT ?2"
        };
        let mut stmt = match conn.prepare(sql) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        let mut out: Vec<MessageRecord> = if let Some(before) = before_ts {
            stmt.query_map(params![conv.to_string(), before, limit], message_from_row)
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        } else {
            stmt.query_map(params![conv.to_string(), limit], message_from_row)
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        };
        out.reverse();
        out
    }

    pub fn search_messages(&self, query: &str, limit: u32) -> Vec<MessageRecord> {
        let pattern = format!("%{}%", escape_like(query.trim()));
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, conv_id, conv_kind, sender_id, sender_name, kind, text, file_id, reply_to, mentions, ts, state, direction
             FROM messages
             WHERE text LIKE ?1 ESCAPE '\\'
             ORDER BY ts DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        let limit = limit.clamp(1, 500) as i64;
        let mut out: Vec<MessageRecord> = stmt
            .query_map(params![pattern, limit], message_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        out.sort_by_key(|m| m.ts);
        out
    }

    /// Marks a conversation read and returns `(message_id, sender_id)` for
    /// incoming messages that changed state.
    pub fn mark_read(&self, conv: Uuid) -> Vec<(Uuid, Uuid)> {
        let conn = self.conn.lock();
        let mut ids: Vec<(Uuid, Uuid)> = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, sender_id FROM messages WHERE conv_id = ?1 AND direction = 'incoming' AND state <> 'read'",
        ) {
            if let Ok(rows) = stmt.query_map([conv.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                ids = rows
                    .filter_map(|r| r.ok())
                    .map(|(id, sender)| (parse_uuid(&id), parse_uuid(&sender)))
                    .collect();
            }
        }
        let _ = conn.execute(
            "UPDATE messages SET state = 'read' WHERE conv_id = ?1 AND direction = 'incoming' AND state <> 'read'",
            [conv.to_string()],
        );
        let _ = conn.execute(
            "UPDATE conversations SET unread = 0, mentioned = 0 WHERE id = ?1",
            [conv.to_string()],
        );
        ids
    }

    pub fn update_message_state(&self, ids: &[Uuid], state: DeliveryState) {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare("SELECT state FROM messages WHERE id = ?1") {
            Ok(stmt) => stmt,
            Err(_) => return,
        };
        let mut update = match conn.prepare("UPDATE messages SET state = ?2 WHERE id = ?1") {
            Ok(stmt) => stmt,
            Err(_) => return,
        };
        for id in ids {
            let current: Option<String> = stmt
                .query_row([id.to_string()], |r| r.get::<_, String>(0))
                .optional()
                .ok()
                .flatten();
            let Some(current) = current else { continue };
            if state_rank(state) > state_rank(DeliveryState::parse(&current)) {
                let _ = update.execute(params![id.to_string(), state.as_str()]);
            }
        }
    }

    pub fn message_conv(&self, id: Uuid) -> Option<Uuid> {
        self.conn
            .lock()
            .query_row(
                "SELECT conv_id FROM messages WHERE id = ?1",
                [id.to_string()],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok())
    }

    // ---- groups ---------------------------------------------------------

    pub fn seed_everyone(&self, me: Uuid) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO groups (id, name, avatar_color, created_by, created_at, is_default, version)
             VALUES (?1, 'Everyone', ?2, ?3, ?4, 1, 1)
             ON CONFLICT(id) DO NOTHING",
            params![
                EVERYONE_GROUP_ID.to_string(),
                0x74AA_F7i64,
                me.to_string(),
                crate::now_ms()
            ],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO group_members (group_id, peer_id, joined_at) VALUES (?1, ?2, ?3)",
            params![EVERYONE_GROUP_ID.to_string(), me.to_string(), crate::now_ms()],
        )?;
        Ok(())
    }

    pub fn upsert_group(&self, group: &GroupRecord) -> Result<()> {
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let current_version: i64 = tx
            .query_row(
                "SELECT version FROM groups WHERE id = ?1",
                [group.id.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(-1);
        tx.execute(
            "INSERT INTO groups (id, name, avatar_color, created_by, created_at, is_default, version)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                avatar_color = excluded.avatar_color,
                version = MAX(groups.version, excluded.version)",
            params![
                group.id.to_string(),
                group.name,
                group.avatar_color as i64,
                group.created_by.to_string(),
                group.created_at,
                group.is_default as i64,
                group.version as i64,
            ],
        )?;
        let existing: Vec<Uuid> = {
            let mut stmt = tx.prepare("SELECT peer_id FROM group_members WHERE group_id = ?1")?;
            let rows = stmt.query_map([group.id.to_string()], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok())
                .filter_map(|s| Uuid::parse_str(&s).ok())
                .collect()
        };
        // Only synchronise membership when the received group is not older.
        if (group.version as i64) >= current_version || existing.is_empty() {
            for member in &group.members {
                tx.execute(
                    "INSERT OR IGNORE INTO group_members (group_id, peer_id, joined_at) VALUES (?1,?2,?3)",
                    params![group.id.to_string(), member.to_string(), crate::now_ms()],
                )?;
            }
            for existing_member in existing {
                if !group.members.contains(&existing_member) {
                    tx.execute(
                        "DELETE FROM group_members WHERE group_id = ?1 AND peer_id = ?2",
                        params![group.id.to_string(), existing_member.to_string()],
                    )?;
                }
            }
        }
        tx.execute(
            "INSERT INTO conversations (id, kind, title, avatar_color) VALUES (?1, 'group', ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, avatar_color = excluded.avatar_color",
            params![group.id.to_string(), group.name, group.avatar_color as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_groups(&self) -> Vec<GroupRecord> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, name, avatar_color, created_by, created_at, is_default, version FROM groups ORDER BY is_default DESC, created_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        let base: Vec<(Uuid, String, u32, Uuid, i64, bool, u64)> = stmt
            .query_map([], |row| {
                Ok((
                    parse_uuid(&row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as u32,
                    parse_uuid(&row.get::<_, String>(3)?),
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, i64>(6)? as u64,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        let mut member_stmt = match conn.prepare(
            "SELECT peer_id FROM group_members WHERE group_id = ?1 ORDER BY joined_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        base.into_iter()
            .map(|(id, name, avatar_color, created_by, created_at, is_default, version)| {
                let members: Vec<Uuid> = member_stmt
                    .query_map([id.to_string()], |r| r.get::<_, String>(0))
                    .map(|rows| {
                        rows.filter_map(|r| r.ok())
                            .filter_map(|s| Uuid::parse_str(&s).ok())
                            .collect()
                    })
                    .unwrap_or_default();
                GroupRecord {
                    id,
                    name,
                    avatar_color,
                    members,
                    created_by,
                    created_at,
                    is_default,
                    version,
                }
            })
            .collect()
    }

    pub fn get_group(&self, id: Uuid) -> Option<GroupRecord> {
        self.list_groups().into_iter().find(|g| g.id == id)
    }

    pub fn add_group_member(&self, group: Uuid, peer: Uuid) -> Result<()> {
        self.conn.lock().execute(
            "INSERT OR IGNORE INTO group_members (group_id, peer_id, joined_at) VALUES (?1,?2,?3)",
            params![group.to_string(), peer.to_string(), crate::now_ms()],
        )?;
        self.bump_group_version(group)
    }

    pub fn remove_group_member(&self, group: Uuid, peer: Uuid) -> Result<()> {
        self.conn.lock().execute(
            "DELETE FROM group_members WHERE group_id = ?1 AND peer_id = ?2",
            params![group.to_string(), peer.to_string()],
        )?;
        self.bump_group_version(group)
    }

    pub fn rename_group(&self, group: Uuid, name: &str, avatar_color: Option<u32>) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE groups SET name = ?2, avatar_color = COALESCE(?3, avatar_color) WHERE id = ?1",
            params![group.to_string(), name, avatar_color.map(|c| c as i64)],
        )?;
        self.conn.lock().execute(
            "UPDATE conversations SET title = ?2, avatar_color = COALESCE(?3, avatar_color) WHERE id = ?1",
            params![group.to_string(), name, avatar_color.map(|c| c as i64)],
        )?;
        self.bump_group_version(group)
    }

    pub fn bump_group_version(&self, group: Uuid) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE groups SET version = version + 1 WHERE id = ?1",
            [group.to_string()],
        )?;
        Ok(())
    }

    pub fn delete_group(&self, group: Uuid) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM groups WHERE id = ?1", [group.to_string()])?;
        conn.execute("DELETE FROM group_members WHERE group_id = ?1", [group.to_string()])?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", [group.to_string()])?;
        Ok(())
    }

    // ---- outbox ---------------------------------------------------------

    pub fn enqueue_outbox(&self, peer: Uuid, payload: &str) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO outbox (peer_id, payload, created_at) VALUES (?1,?2,?3)",
            params![peer.to_string(), payload, crate::now_ms()],
        )?;
        Ok(())
    }

    pub fn outbox_for(&self, peer: Uuid) -> Vec<(i64, String)> {
        let conn = self.conn.lock();
        let mut stmt = match conn
            .prepare("SELECT id, payload FROM outbox WHERE peer_id = ?1 ORDER BY id ASC LIMIT 200")
        {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([peer.to_string()], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn dequeue_outbox(&self, id: i64) {
        let _ = self
            .conn
            .lock()
            .execute("DELETE FROM outbox WHERE id = ?1", [id]);
    }

    pub fn outbox_len(&self) -> u32 {
        self.conn
            .lock()
            .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as u32
    }

    // ---- files ----------------------------------------------------------

    pub fn insert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO files
                (id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                mime = excluded.mime,
                size = excluded.size,
                sha256 = excluded.sha256,
                chunk_size = excluded.chunk_size,
                chunks = excluded.chunks,
                message_id = COALESCE(excluded.message_id, files.message_id),
                thumb_path = COALESCE(excluded.thumb_path, files.thumb_path),
                peer_id = COALESCE(excluded.peer_id, files.peer_id),
                conv_id = excluded.conv_id",
            params![
                file.id.to_string(),
                file.message_id.map(|id| id.to_string()),
                file.conv.to_string(),
                file.peer_id.map(|id| id.to_string()),
                file.direction.as_str(),
                file.name,
                file.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                file.thumb_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                file.mime,
                file.size as i64,
                file.received as i64,
                file.sha256,
                file.state.as_str(),
                file.next_index as i64,
                file.chunk_size as i64,
                file.chunks as i64,
                file.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_file(&self, id: Uuid) -> Option<FileRecord> {
        self.conn
            .lock()
            .query_row(
                "SELECT id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at
                 FROM files WHERE id = ?1",
                [id.to_string()],
                file_from_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn update_file_progress(
        &self,
        id: Uuid,
        received: u64,
        next_index: u32,
        state: FileState,
    ) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE files SET received = ?2, next_index = ?3, state = ?4 WHERE id = ?1",
            params![
                id.to_string(),
                received as i64,
                next_index as i64,
                state.as_str()
            ],
        )?;
        Ok(())
    }

    pub fn set_file_state(&self, id: Uuid, state: FileState) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE files SET state = ?2 WHERE id = ?1",
            params![id.to_string(), state.as_str()],
        )?;
        Ok(())
    }

    pub fn link_file(&self, id: Uuid, message_id: Uuid, peer_id: Option<Uuid>) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE files SET message_id = ?2, peer_id = COALESCE(?3, peer_id) WHERE id = ?1",
            params![
                id.to_string(),
                message_id.to_string(),
                peer_id.map(|p| p.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn set_file_paths(
        &self,
        id: Uuid,
        path: Option<&Path>,
        thumb_path: Option<&Path>,
        state: FileState,
    ) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE files SET path = COALESCE(?2, path), thumb_path = COALESCE(?3, thumb_path), state = ?4 WHERE id = ?1",
            params![
                id.to_string(),
                path.map(|p| p.to_string_lossy().to_string()),
                thumb_path.map(|p| p.to_string_lossy().to_string()),
                state.as_str()
            ],
        )?;
        Ok(())
    }

    pub fn pending_incoming_files(&self) -> Vec<FileRecord> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at
             FROM files WHERE direction = 'incoming' AND state IN ('pending_offer','downloading','paused') ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], file_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Outgoing transfers that never finished (app restart, peer offline).
    pub fn outgoing_pending_files(&self, peer: Option<Uuid>) -> Vec<FileRecord> {
        let conn = self.conn.lock();
        if let Some(peer) = peer {
            let mut stmt = match conn.prepare(
                "SELECT id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at
                 FROM files WHERE direction = 'outgoing' AND state IN ('awaiting_accept','paused','uploading')
                 AND (peer_id = ?1 OR peer_id IS NULL) ORDER BY created_at ASC",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return Vec::new(),
            };
            return stmt
                .query_map([peer.to_string()], file_from_row)
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
        }
        let mut stmt = match conn.prepare(
            "SELECT id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at
             FROM files WHERE direction = 'outgoing' AND state IN ('awaiting_accept','paused','uploading') ORDER BY created_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], file_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn files_for_conv(&self, conv: Uuid) -> Vec<FileRecord> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, message_id, conv_id, peer_id, direction, name, path, thumb_path, mime, size, received, sha256, state, next_index, chunk_size, chunks, created_at
             FROM files WHERE conv_id = ?1 ORDER BY created_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([conv.to_string()], file_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}

fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn conv_kind_str(kind: ConvKind) -> &'static str {
    match kind {
        ConvKind::Direct => "direct",
        ConvKind::Group => "group",
    }
}

fn conv_kind_parse(s: &str) -> ConvKind {
    if s == "group" {
        ConvKind::Group
    } else {
        ConvKind::Direct
    }
}

fn message_kind_str(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Text => "text",
        MessageKind::Image => "image",
        MessageKind::Video => "video",
        MessageKind::Audio => "audio",
        MessageKind::File => "file",
        MessageKind::System => "system",
    }
}

fn message_kind_parse(s: &str) -> MessageKind {
    match s {
        "image" => MessageKind::Image,
        "video" => MessageKind::Video,
        "audio" => MessageKind::Audio,
        "file" => MessageKind::File,
        "system" => MessageKind::System,
        _ => MessageKind::Text,
    }
}

fn state_rank(state: DeliveryState) -> u8 {
    match state {
        DeliveryState::Queued => 0,
        DeliveryState::Sending => 1,
        DeliveryState::Sent => 2,
        DeliveryState::Delivered => 3,
        DeliveryState::Read => 4,
        DeliveryState::Failed => 5,
    }
}

fn parse_uuid(raw: &str) -> Uuid {
    Uuid::parse_str(raw).unwrap_or_default()
}

fn peer_from_row(row: &Row<'_>) -> rusqlite::Result<PeerSnapshot> {
    Ok(PeerSnapshot {
        id: parse_uuid(&row.get::<_, String>(0)?),
        display_name: row.get(1)?,
        avatar_color: row.get::<_, i64>(2)? as u32,
        device_name: row.get(3)?,
        online: row.get::<_, i64>(4)? != 0,
        last_seen: row.get(5)?,
        addr: row.get(6)?,
        port: row.get::<_, Option<i64>>(7)?.map(|p| p as u16),
        secure: row.get::<_, i64>(8)? != 0,
    })
}

fn conversation_from_row(row: &Row<'_>, everyone: &str) -> rusqlite::Result<ConversationRecord> {
    let id = parse_uuid(&row.get::<_, String>(0)?);
    Ok(ConversationRecord {
        id,
        kind: conv_kind_parse(&row.get::<_, String>(1)?),
        title: row.get(2)?,
        avatar_color: row.get::<_, i64>(3)? as u32,
        unread: row.get::<_, i64>(4)? as u32,
        muted: row.get::<_, i64>(5)? != 0,
        last_ts: row.get(6)?,
        last_preview: row.get(7)?,
        last_sender: row.get(8)?,
        mentioned: row.get::<_, i64>(9)? != 0,
        online: row.get::<_, Option<i64>>(10)?.map(|v| v != 0),
        member_count: row.get::<_, i64>(11)? as u32,
        is_everyone: id.to_string() == everyone,
    })
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<MessageRecord> {
    let mentions: Vec<Uuid> = row
        .get::<_, String>(9)
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .map(|ids| ids.iter().filter_map(|s| Uuid::parse_str(s).ok()).collect())
        .unwrap_or_default();
    Ok(MessageRecord {
        id: parse_uuid(&row.get::<_, String>(0)?),
        conv: parse_uuid(&row.get::<_, String>(1)?),
        conv_kind: conv_kind_parse(&row.get::<_, String>(2)?),
        sender_id: parse_uuid(&row.get::<_, String>(3)?),
        sender_name: row.get(4)?,
        kind: message_kind_parse(&row.get::<_, String>(5)?),
        text: row.get(6)?,
        file_id: row
            .get::<_, Option<String>>(7)?
            .and_then(|s| Uuid::parse_str(&s).ok()),
        reply_to: row
            .get::<_, Option<String>>(8)?
            .and_then(|s| Uuid::parse_str(&s).ok()),
        mentions,
        ts: row.get(10)?,
        state: DeliveryState::parse(&row.get::<_, String>(11)?),
        outgoing: row.get::<_, String>(12)? == "outgoing",
    })
}

fn file_from_row(row: &Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: parse_uuid(&row.get::<_, String>(0)?),
        message_id: row
            .get::<_, Option<String>>(1)?
            .and_then(|s| Uuid::parse_str(&s).ok()),
        conv: parse_uuid(&row.get::<_, String>(2)?),
        peer_id: row
            .get::<_, Option<String>>(3)?
            .and_then(|s| Uuid::parse_str(&s).ok()),
        direction: FileDirection::parse(&row.get::<_, String>(4)?),
        name: row.get(5)?,
        path: row.get::<_, Option<String>>(6)?.map(Into::into),
        thumb_path: row.get::<_, Option<String>>(7)?.map(Into::into),
        mime: row.get(8)?,
        size: row.get::<_, i64>(9)? as u64,
        received: row.get::<_, i64>(10)? as u64,
        sha256: row.get(11)?,
        state: FileState::parse(&row.get::<_, String>(12)?),
        next_index: row.get::<_, i64>(13)? as u32,
        chunk_size: row.get::<_, i64>(14)? as u32,
        chunks: row.get::<_, i64>(15)? as u32,
        created_at: row.get(16)?,
    })
}

pub fn default_data_path(app_data_dir: &Path) -> std::path::PathBuf {
    app_data_dir.join("lanchat.sqlite3")
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<Store>();
}
