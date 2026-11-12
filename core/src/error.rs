use std::fmt;

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
    Websocket(tokio_tungstenite::tungstenite::Error),
    Protocol(crate::protocol::ProtocolError),
    Crypto(String),
    Discovery(String),
    NotFound(String),
    Invalid(String),
    Internal(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "io error: {e}"),
            CoreError::Json(e) => write!(f, "json error: {e}"),
            CoreError::Sqlite(e) => write!(f, "database error: {e}"),
            CoreError::Websocket(e) => write!(f, "websocket error: {e}"),
            CoreError::Protocol(e) => write!(f, "protocol error: {e}"),
            CoreError::Crypto(e) => write!(f, "crypto error: {e}"),
            CoreError::Discovery(e) => write!(f, "discovery error: {e}"),
            CoreError::NotFound(e) => write!(f, "not found: {e}"),
            CoreError::Invalid(e) => write!(f, "invalid input: {e}"),
            CoreError::Internal(e) => write!(f, "internal error: {e}"),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CoreError::Io(e) => Some(e),
            CoreError::Json(e) => Some(e),
            CoreError::Sqlite(e) => Some(e),
            CoreError::Websocket(e) => Some(e),
            CoreError::Protocol(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        CoreError::Io(value)
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        CoreError::Json(value)
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(value: rusqlite::Error) -> Self {
        CoreError::Sqlite(value)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for CoreError {
    fn from(value: tokio_tungstenite::tungstenite::Error) -> Self {
        CoreError::Websocket(value)
    }
}

impl From<crate::protocol::ProtocolError> for CoreError {
    fn from(value: crate::protocol::ProtocolError) -> Self {
        CoreError::Protocol(value)
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
