use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tile::TileId;

/// Lightweight metadata for indexing tiles in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMeta {
    pub id: TileId,
    pub valence: f64,
    pub created: DateTime<Utc>,
    pub accessed: DateTime<Utc>,
    pub constraints: HashSet<String>,
}

/// In-memory index for fast recall queries.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CrystalIndex {
    tiles: HashMap<TileId, TileMeta>,
    by_valence: BTreeMap<OrderedValence, Vec<TileId>>,
    by_time: BTreeMap<i64, TileId>,
    constraint_index: HashMap<String, HashSet<TileId>>,
}

/// Wrapper for f64 that implements Ord (BTreeMap requires it).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderedValence(pub f64);

impl PartialEq for OrderedValence {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for OrderedValence {}

impl PartialOrd for OrderedValence {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedValence {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl CrystalIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a tile into the index.
    pub fn insert(&mut self, meta: TileMeta) {
        let id = meta.id.clone();
        let valence = meta.valence;
        let created_ts = meta.created.timestamp();
        let constraints = meta.constraints.clone();

        // Main index.
        self.tiles.insert(id.clone(), meta);

        // Valence index.
        self.by_valence
            .entry(OrderedValence(valence))
            .or_default()
            .push(id.clone());

        // Time index.
        self.by_time.insert(created_ts, id.clone());

        // Constraint index.
        for key in &constraints {
            self.constraint_index
                .entry(key.clone())
                .or_default()
                .insert(id.clone());
        }
    }

    /// Remove a tile from the index.
    pub fn remove(&mut self, id: &TileId) -> Option<TileMeta> {
        let meta = self.tiles.remove(id)?;

        // Remove from valence index.
        if let Some(ids) = self.by_valence.get_mut(&OrderedValence(meta.valence)) {
            ids.retain(|i| i != id);
            if ids.is_empty() {
                self.by_valence.remove(&OrderedValence(meta.valence));
            }
        }

        // Remove from time index.
        let ts = meta.created.timestamp();
        self.by_time.remove(&ts);

        // Remove from constraint index.
        for key in &meta.constraints {
            if let Some(set) = self.constraint_index.get_mut(key) {
                set.remove(id);
                if set.is_empty() {
                    self.constraint_index.remove(key);
                }
            }
        }

        Some(meta)
    }

    /// Query tiles by constraint keywords, returning up to `limit` matches.
    pub fn query(&self, constraints: &[&str], limit: usize) -> Vec<TileId> {
        let mut candidates: HashMap<TileId, usize> = HashMap::new();

        for key in constraints {
            if let Some(ids) = self.constraint_index.get(*key) {
                for id in ids {
                    *candidates.entry(id.clone()).or_default() += 1;
                }
            }
        }

        // Sort by number of matching constraints (descending).
        let mut ranked: Vec<_> = candidates.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));

        ranked.into_iter().take(limit).map(|(id, _)| id).collect()
    }

    /// Get top tiles by valence.
    pub fn top_by_valence(&self, limit: usize) -> Vec<TileId> {
        self.by_valence
            .iter()
            .rev() // Highest valence first.
            .flat_map(|(_, ids)| ids.iter().cloned())
            .take(limit)
            .collect()
    }

    /// Get most recent tiles.
    pub fn recent(&self, limit: usize) -> Vec<TileId> {
        self.by_time
            .iter()
            .rev() // Most recent first.
            .map(|(_, id)| id.clone())
            .take(limit)
            .collect()
    }

    /// Get a tile's metadata.
    pub fn get(&self, id: &TileId) -> Option<&TileMeta> {
        self.tiles.get(id)
    }

    /// Total number of indexed tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Is the index empty?
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Get all tile IDs.
    pub fn all_ids(&self) -> Vec<TileId> {
        self.tiles.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(valence: f64, constraints: &[&str]) -> TileMeta {
        TileMeta {
            id: TileId::new(),
            valence,
            created: Utc::now(),
            accessed: Utc::now(),
            constraints: constraints.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn insert_and_query() {
        let mut index = CrystalIndex::new();
        let m = meta(0.5, &["rust", "programming"]);
        let id = m.id.clone();
        index.insert(m);

        let results = index.query(&["rust"], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[test]
    fn top_by_valence() {
        let mut index = CrystalIndex::new();
        let m1 = meta(0.3, &[]);
        let m2 = meta(0.9, &[]);
        let m3 = meta(0.6, &[]);
        let high_id = m2.id.clone();
        index.insert(m1);
        index.insert(m2);
        index.insert(m3);

        let top = index.top_by_valence(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0], high_id);
    }

    #[test]
    fn recent() {
        let mut index = CrystalIndex::new();
        let mut m1 = meta(0.5, &[]);
        m1.created = Utc::now() - chrono::Duration::days(1);
        let m2 = meta(0.5, &[]);
        let recent_id = m2.id.clone();
        index.insert(m1);
        index.insert(m2);

        let recent = index.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0], recent_id);
    }

    #[test]
    fn remove_tile() {
        let mut index = CrystalIndex::new();
        let m = meta(0.5, &["test"]);
        let id = m.id.clone();
        index.insert(m);
        assert_eq!(index.len(), 1);

        index.remove(&id);
        assert!(index.is_empty());
        assert!(index.query(&["test"], 10).is_empty());
    }

    #[test]
    fn multi_constraint_query() {
        let mut index = CrystalIndex::new();
        let m1 = meta(0.5, &["rust", "systems"]);
        let m2 = meta(0.5, &["rust", "web"]);
        index.insert(m1);
        index.insert(m2);

        // Both match "rust", only one matches "systems".
        let results = index.query(&["rust"], 10);
        assert_eq!(results.len(), 2);

        let results = index.query(&["systems"], 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn empty_query() {
        let index = CrystalIndex::new();
        let results = index.query(&["nonexistent"], 10);
        assert!(results.is_empty());
    }
}
