use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;
use crate::protocol::ProtocolError;

/// Colours are packed 0xRRGGBB and rendered directly by the UI.
pub const AVATAR_PALETTE: [u32; 10] = [
    0xE0_6C_75, 0xF2_9E_6E, 0xF5_D0_6B, 0x8B_D4_9A, 0x6B_D3_C9, 0x74_AA_F7, 0x9B_8C_F5,
    0xE0_8C_F5, 0xF5_8C_C0, 0x9A_A6_B4,
];

/// Stable device identity. Created on first run and persisted as JSON next to
/// the SQLite database so it survives reinstalls that keep app data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub peer_id: uuid::Uuid,
    pub display_name: String,
    pub avatar_color: u32,
    pub device_name: String,
    pub created_at: i64,
}

impl Identity {
    pub fn new(display_name: impl Into<String>, avatar_color: Option<u32>) -> Self {
        let name: String = display_name.into();
        let avatar_color = avatar_color.unwrap_or_else(|| {
            let idx = (uuid::Uuid::new_v4().as_u128() % AVATAR_PALETTE.len() as u128) as usize;
            AVATAR_PALETTE[idx]
        });
        Self {
            peer_id: uuid::Uuid::new_v4(),
            display_name: name,
            avatar_color,
            device_name: default_device_name(),
            created_at: crate::now_ms(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.display_name.trim().is_empty()
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let identity: Identity = serde_json::from_str(&raw)?;
        Ok(identity)
    }

    /// Loads the identity file, creating a fresh one when absent or corrupt.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let path = dir.join("identity.json");
        match Identity::load(&path) {
            Ok(identity) => Ok(identity),
            Err(crate::error::CoreError::Io(err))
                if err.kind() == std::io::ErrorKind::NotFound =>
            {
                let identity = Identity::new("", None);
                identity.save(dir)?;
                Ok(identity)
            }
            Err(err) => {
                // Don't lose the old file silently.
                tracing::warn!(?err, "identity file unreadable, regenerating");
                let backup = dir.join(format!("identity.corrupt-{}.json", crate::now_ms()));
                let _ = std::fs::rename(&path, backup);
                let identity = Identity::new("", None);
                identity.save(dir)?;
                Ok(identity)
            }
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("identity.json");
        let tmp = dir.join("identity.json.tmp");
        let raw = serde_json::to_vec_pretty(self)?;
        std::fs::write(&tmp, raw)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn set_display_name(&mut self, name: &str) -> std::result::Result<(), ProtocolError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ProtocolError::InvalidField(
                "display_name".into(),
                "must not be empty".into(),
            ));
        }
        if trimmed.chars().count() > 64 {
            return Err(ProtocolError::InvalidField(
                "display_name".into(),
                "must be 64 characters or fewer".into(),
            ));
        }
        self.display_name = trimmed.to_string();
        Ok(())
    }
}

fn default_device_name() -> String {
    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown device".into());
    host.chars().take(64).collect()
}
