use std::fmt;

/// Core error type for memory-crystal operations.
#[derive(Debug)]
pub enum CrystalError {
    /// I/O error reading or writing tile data.
    Io(std::io::Error),
    /// JSON serialization/deserialization error.
    Json(serde_json::Error),
    /// A tile was not found in the crystal.
    TileNotFound(String),
    /// Invalid argument provided.
    InvalidArgument(String),
    /// Corruption detected in tile data.
    Corruption { tile_id: String, detail: String },
}

impl fmt::Display for CrystalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::TileNotFound(id) => write!(f, "tile not found: {}", id),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {}", msg),
            Self::Corruption { tile_id, detail } => {
                write!(f, "corruption in tile {}: {}", tile_id, detail)
            }
        }
    }
}

impl std::error::Error for CrystalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CrystalError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for CrystalError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, CrystalError>;
