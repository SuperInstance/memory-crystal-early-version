use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a tile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TileId(pub String);

impl TileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A crystallized memory tile — lossy, compressed representation of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub id: TileId,
    pub source_hash: String,
    pub constraints: HashMap<String, String>,
    pub summary: String,
    pub context_required: Vec<String>,
    pub valence: f64,
    pub created_at: DateTime<Utc>,
    pub accessed_at: DateTime<Utc>,
    pub access_count: u64,
    pub generation: u32,
    pub parent_id: Option<TileId>,
}

impl Tile {
    /// Compute SHA-256 hash of content.
    pub fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Record an access, updating timestamp and count.
    pub fn touch(&mut self) {
        self.accessed_at = Utc::now();
        self.access_count += 1;
    }

    /// Age of the tile in seconds.
    pub fn age_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds()
    }
}

/// Map of content salience — which parts matter most.
#[derive(Debug, Clone, Default)]
pub struct SalienceMap {
    pub constraints: HashMap<String, String>,
    pub context_required: Vec<String>,
    pub valence: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_id_is_unique() {
        let a = TileId::new();
        let b = TileId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_content_deterministic() {
        let h1 = Tile::hash_content("hello world");
        let h2 = Tile::hash_content("hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn hash_content_different() {
        let h1 = Tile::hash_content("hello");
        let h2 = Tile::hash_content("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn tile_touch_updates_access() {
        let mut tile = Tile {
            id: TileId::new(),
            source_hash: String::new(),
            constraints: HashMap::new(),
            summary: "test".into(),
            context_required: vec![],
            valence: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
            generation: 0,
            parent_id: None,
        };
        let before = tile.accessed_at;
        tile.touch();
        assert_eq!(tile.access_count, 1);
        assert!(tile.accessed_at >= before);
    }

    #[test]
    fn tile_roundtrip_json() {
        let tile = Tile {
            id: TileId::new(),
            source_hash: "abc".into(),
            constraints: HashMap::from([("key".into(), "val".into())]),
            summary: "a summary".into(),
            context_required: vec!["ctx1".into()],
            valence: 0.8,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 3,
            generation: 1,
            parent_id: Some(TileId::new()),
        };
        let json = serde_json::to_string(&tile).unwrap();
        let back: Tile = serde_json::from_str(&json).unwrap();
        assert_eq!(tile.id, back.id);
        assert_eq!(tile.summary, back.summary);
        assert_eq!(tile.generation, back.generation);
        assert_eq!(tile.parent_id, back.parent_id);
    }
}
