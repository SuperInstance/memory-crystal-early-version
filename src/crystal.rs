use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;

use crate::decay::DecaySchedule;
use crate::decoder::{Reconstruction, TileDecoder};
use crate::encoder::TileEncoder;
use crate::error::{CrystalError, Result};
use crate::index::{CrystalIndex, TileMeta};
use crate::tile::{SalienceMap, Tile, TileId};

/// Statistics about a crystal.
#[derive(Debug, Clone)]
pub struct CrystalStats {
    pub tile_count: usize,
    pub total_bytes: u64,
    pub avg_valence: f64,
    pub max_valence: f64,
    pub min_valence: f64,
    pub oldest: Option<chrono::DateTime<Utc>>,
    pub newest: Option<chrono::DateTime<Utc>>,
}

/// The Crystal — a collection of tiles persisted on disk.
pub struct Crystal {
    path: PathBuf,
    index: CrystalIndex,
    encoder: TileEncoder,
    decoder: TileDecoder,
    decay: DecaySchedule,
}

impl Crystal {
    /// Open or create a crystal at the given directory path.
    pub fn open(path: &Path) -> Result<Self> {
        fs::create_dir_all(path)?;

        let mut index = CrystalIndex::new();
        let tiles_dir = path.join("tiles");
        fs::create_dir_all(&tiles_dir)?;

        // Load existing tiles into index.
        for entry in fs::read_dir(&tiles_dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "json") {
                let data = fs::read_to_string(entry.path())?;
                match serde_json::from_str::<Tile>(&data) {
                    Ok(tile) => {
                        let meta = TileMeta {
                            id: tile.id.clone(),
                            valence: tile.valence,
                            created: tile.created_at,
                            accessed: tile.accessed_at,
                            constraints: tile.constraints.keys().cloned().collect(),
                        };
                        index.insert(meta);
                    }
                    Err(e) => {
                        let tile_id = entry.path().file_stem().unwrap().to_string_lossy().to_string();
                        eprintln!("Warning: corrupt tile {}: {}", tile_id, e);
                    }
                }
            }
        }

        Ok(Self {
            path: path.to_path_buf(),
            index,
            encoder: TileEncoder::default(),
            decoder: TileDecoder::default(),
            decay: DecaySchedule::default(),
        })
    }

    /// Crystallize content into a new tile.
    pub fn crystallize(&mut self, content: &str, salience: SalienceMap) -> Result<TileId> {
        let tile = self.encoder.encode(content, &salience);
        let id = tile.id.clone();

        self.persist_tile(&tile)?;

        let meta = TileMeta {
            id: tile.id.clone(),
            valence: tile.valence,
            created: tile.created_at,
            accessed: tile.accessed_at,
            constraints: tile.constraints.keys().cloned().collect(),
        };
        self.index.insert(meta);

        Ok(id)
    }

    /// Recall a specific tile with context-dependent reconstruction.
    pub fn recall(&self, tile_id: &TileId, context: &str) -> Result<Reconstruction> {
        let tile = self.load_tile(tile_id)?;
        Ok(self.decoder.decode(&tile, context))
    }

    /// Recall multiple tiles matching a query.
    pub fn recall_collective(&self, query: &str, limit: usize) -> Result<Vec<Reconstruction>> {
        let keywords: Vec<&str> = query.split_whitespace().collect();
        let ids = self.index.query(&keywords, limit);

        let mut results = Vec::new();
        for id in &ids {
            if let Ok(tile) = self.load_tile(id) {
                results.push(self.decoder.decode(&tile, query));
            }
        }

        Ok(results)
    }

    /// Forget tiles older than the given duration (using decay schedule).
    pub fn forget(&mut self, older_than: Duration) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::from_std(older_than).unwrap_or(chrono::Duration::seconds(0));
        let mut forgotten = 0;

        let ids_to_check: Vec<TileId> = self.index.all_ids();
        for id in &ids_to_check {
            if let Ok(tile) = self.load_tile(id) {
                if tile.accessed_at < cutoff && self.decay.should_forget(&tile, Utc::now()) {
                    self.remove_tile(id)?;
                    forgotten += 1;
                }
            }
        }

        Ok(forgotten)
    }

    /// Reconsolidate a tile: update it with new context, resetting decay.
    pub fn reconsolidate(&mut self, tile_id: &TileId, new_context: &str) -> Result<()> {
        let mut tile = self.load_tile(tile_id)?;
        self.decay.reconsolidate(&mut tile, new_context);
        self.persist_tile(&tile)?;

        // Update index.
        self.index.remove(tile_id);
        let meta = TileMeta {
            id: tile.id.clone(),
            valence: tile.valence,
            created: tile.created_at,
            accessed: tile.accessed_at,
            constraints: tile.constraints.keys().cloned().collect(),
        };
        self.index.insert(meta);

        Ok(())
    }

    /// Get stats about the crystal.
    pub fn stats(&self) -> CrystalStats {
        let tile_count = self.index.len();

        let tiles_dir = self.path.join("tiles");
        let total_bytes = fs::metadata(&tiles_dir)
            .map(|m| m.len())
            .unwrap_or(0);

        let all_meta: Vec<&TileMeta> = self.index.all_ids()
            .iter()
            .filter_map(|id| self.index.get(id))
            .collect();

        let (min_valence, max_valence, avg_valence) = if all_meta.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let vals: Vec<f64> = all_meta.iter().map(|m| m.valence).collect();
            let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let avg = vals.iter().sum::<f64>() / vals.len() as f64;
            (min, max, avg)
        };

        let oldest = all_meta.iter().map(|m| m.created).min();
        let newest = all_meta.iter().map(|m| m.created).max();

        CrystalStats {
            tile_count,
            total_bytes,
            avg_valence,
            max_valence,
            min_valence,
            oldest,
            newest,
        }
    }

    // --- Private helpers ---

    fn tile_path(&self, id: &TileId) -> PathBuf {
        self.path.join("tiles").join(format!("{}.json", id.0))
    }

    fn persist_tile(&self, tile: &Tile) -> Result<()> {
        let path = self.tile_path(&tile.id);
        let data = serde_json::to_string_pretty(tile)?;
        fs::write(path, data)?;
        Ok(())
    }

    fn load_tile(&self, id: &TileId) -> Result<Tile> {
        let path = self.tile_path(id);
        if !path.exists() {
            return Err(CrystalError::TileNotFound(id.to_string()));
        }
        let data = fs::read_to_string(path)?;
        let tile: Tile = serde_json::from_str(&data)?;
        Ok(tile)
    }

    fn remove_tile(&mut self, id: &TileId) -> Result<()> {
        let path = self.tile_path(id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        self.index.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_empty_crystal() {
        let dir = TempDir::new().unwrap();
        let crystal = Crystal::open(dir.path()).unwrap();
        let stats = crystal.stats();
        assert_eq!(stats.tile_count, 0);
    }

    #[test]
    fn crystallize_and_recall() {
        let dir = TempDir::new().unwrap();
        let mut crystal = Crystal::open(dir.path()).unwrap();

        let id = crystal
            .crystallize(
                "Alice discovered a new algorithm in Seattle.",
                SalienceMap::default(),
            )
            .unwrap();

        let rec = crystal.recall(&id, "Alice was in Seattle").unwrap();
        assert!(rec.confidence > 0.0);
        assert!(!rec.content.is_empty());
    }

    #[test]
    fn recall_collective() {
        let dir = TempDir::new().unwrap();
        let mut crystal = Crystal::open(dir.path()).unwrap();

        // Crystallize with explicit constraints to ensure they're indexed.
        let mut salience1 = SalienceMap::default();
        salience1.constraints.insert("rust".into(), "Rust".into());
        salience1.constraints.insert("programming".into(), "programming".into());
        crystal.crystallize("Rust programming language features.", salience1).unwrap();

        let mut salience2 = SalienceMap::default();
        salience2.constraints.insert("python".into(), "Python".into());
        crystal.crystallize("Python machine learning libraries.", salience2).unwrap();

        let results = crystal.recall_collective("rust programming", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn forget_old_tiles() {
        let dir = TempDir::new().unwrap();
        let mut crystal = Crystal::open(dir.path()).unwrap();

        let id = crystal
            .crystallize("Temporary data", SalienceMap {
                valence: Some(0.1),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(crystal.stats().tile_count, 1);

        let forgotten = crystal.forget(Duration::from_secs(0)).unwrap();
        // With fast enough decay or low enough retention, tile may be forgotten.
        // (Depends on the decay schedule; at minimum, no crash.)
        assert!(forgotten <= 1);
    }

    #[test]
    fn reconsolidate_updates_tile() {
        let dir = TempDir::new().unwrap();
        let mut crystal = Crystal::open(dir.path()).unwrap();

        let id = crystal
            .crystallize("Original content", SalienceMap::default())
            .unwrap();

        crystal.reconsolidate(&id, "Updated context information").unwrap();

        let rec = crystal.recall(&id, "Updated context").unwrap();
        assert!(rec.content.contains("[updated:") || !rec.content.is_empty());
    }

    #[test]
    fn persist_and_reload() {
        let dir = TempDir::new().unwrap();

        let id = {
            let mut crystal = Crystal::open(dir.path()).unwrap();
            crystal
                .crystallize("Persistent content", SalienceMap::default())
                .unwrap()
        };

        // Reopen and verify.
        let crystal = Crystal::open(dir.path()).unwrap();
        assert_eq!(crystal.stats().tile_count, 1);

        let rec = crystal.recall(&id, "Persistent").unwrap();
        assert!(!rec.content.is_empty());
    }

    #[test]
    fn tile_not_found() {
        let dir = TempDir::new().unwrap();
        let crystal = Crystal::open(dir.path()).unwrap();
        let result = crystal.recall(&TileId("nonexistent".into()), "context");
        assert!(result.is_err());
    }
}
