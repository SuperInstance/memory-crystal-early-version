use crate::tile::{Tile, TileId};

/// A reconstructed memory — decoded from a tile with context.
#[derive(Debug, Clone)]
pub struct Reconstruction {
    pub content: String,
    pub tile_id: TileId,
    pub confidence: f64,
    pub inferred_facts: Vec<String>,
    pub preserved_facts: Vec<String>,
}

/// Context-dependent decoder: reconstructs content from tiles.
pub struct TileDecoder {
    pub context_window: usize,
}

impl Default for TileDecoder {
    fn default() -> Self {
        Self {
            context_window: 4096,
        }
    }
}

impl TileDecoder {
    /// Decode a single tile with the given context.
    ///
    /// The context string is used to enrich the reconstruction by matching
    /// against the tile's `context_required` fields and constraints.
    pub fn decode(&self, tile: &Tile, context: &str) -> Reconstruction {
        let context_lower = context.to_lowercase();

        // Collect preserved facts from tile constraints that appear in context.
        let preserved_facts: Vec<String> = tile
            .constraints
            .values()
            .filter(|v| context_lower.contains(&v.to_lowercase()))
            .cloned()
            .collect();

        // Infer facts from context that relate to the tile's required context cues.
        let mut inferred_facts = Vec::new();
        for cue in &tile.context_required {
            if !context_lower.contains(&cue.to_lowercase()) {
                inferred_facts.push(format!("missing context: {}", cue));
            }
        }

        // Build reconstruction: tile summary + enriched context.
        let mut content = tile.summary.clone();
        if !preserved_facts.is_empty() {
            content.push_str("\n\nVerified facts: ");
            content.push_str(&preserved_facts.join(", "));
        }

        // Confidence based on how much context matched.
        let total_cues = tile.context_required.len().max(1);
        let matched_cues = tile
            .context_required
            .iter()
            .filter(|c| context_lower.contains(&c.to_lowercase()))
            .count();
        let context_confidence = matched_cues as f64 / total_cues as f64;

        // Boost confidence by valence and access count.
        let access_boost = (tile.access_count as f64).log2().max(0.0) * 0.05;
        let confidence = (context_confidence * 0.7 + tile.valence * 0.2 + access_boost).clamp(0.0, 1.0);

        Reconstruction {
            content,
            tile_id: tile.id.clone(),
            confidence,
            inferred_facts,
            preserved_facts,
        }
    }

    /// Decode multiple tiles collectively into a single reconstruction.
    pub fn decode_collective(&self, tiles: &[&Tile], context: &str) -> Reconstruction {
        if tiles.is_empty() {
            return Reconstruction {
                content: String::new(),
                tile_id: TileId::new(),
                confidence: 0.0,
                inferred_facts: vec![],
                preserved_facts: vec![],
            };
        }

        // Use the highest-valence tile as the anchor.
        let anchor = tiles
            .iter()
            .max_by(|a, b| a.valence.partial_cmp(&b.valence).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();

        let content_parts: Vec<String> = tiles.iter().map(|t| t.summary.clone()).collect();
        let _context_lower = context.to_lowercase();

        let mut all_preserved = Vec::new();
        let mut all_inferred = Vec::new();
        let mut total_confidence = 0.0;

        for tile in tiles {
            let rec = self.decode(tile, context);
            all_preserved.extend(rec.preserved_facts);
            all_inferred.extend(rec.inferred_facts);
            total_confidence += rec.confidence;
        }

        all_preserved.sort();
        all_preserved.dedup();
        all_inferred.sort();
        all_inferred.dedup();

        // Truncate context window.
        let mut combined = content_parts.join("\n---\n");
        if combined.len() > self.context_window {
            combined.truncate(self.context_window);
        }

        if !all_preserved.is_empty() {
            combined.push_str("\n\nPreserved facts: ");
            combined.push_str(&all_preserved.join(", "));
        }

        let avg_confidence = total_confidence / tiles.len() as f64;

        Reconstruction {
            content: combined,
            tile_id: anchor.id.clone(),
            confidence: avg_confidence,
            inferred_facts: all_inferred,
            preserved_facts: all_preserved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_tile(summary: &str, constraints: HashMap<String, String>, valence: f64) -> Tile {
        Tile {
            id: TileId::new(),
            source_hash: String::new(),
            constraints,
            summary: summary.into(),
            context_required: vec![],
            valence,
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 1,
            generation: 0,
            parent_id: None,
        }
    }

    #[test]
    fn decode_basic() {
        let decoder = TileDecoder::default();
        let tile = test_tile(
            "Alice discovered a new algorithm.",
            HashMap::from([("person".into(), "Alice".into())]),
            0.8,
        );
        let rec = decoder.decode(&tile, "Alice was working on algorithms");
        assert!(rec.confidence > 0.0);
        assert!(rec.content.contains("Alice discovered"));
    }

    #[test]
    fn decode_with_preserved_facts() {
        let decoder = TileDecoder::default();
        let tile = test_tile(
            "Meeting notes",
            HashMap::from([("place".into(), "Seattle".into())]),
            0.5,
        );
        let rec = decoder.decode(&tile, "We went to Seattle for the meeting.");
        assert!(rec.preserved_facts.contains(&"Seattle".to_string()));
    }

    #[test]
    fn decode_collective() {
        let decoder = TileDecoder::default();
        let t1 = test_tile("First part", HashMap::new(), 0.6);
        let t2 = test_tile("Second part", HashMap::new(), 0.9);
        let rec = decoder.decode_collective(&[&t1, &t2], "context");
        assert!(rec.content.contains("First part"));
        assert!(rec.content.contains("Second part"));
        assert!(rec.confidence > 0.0);
    }

    #[test]
    fn decode_empty_tiles() {
        let decoder = TileDecoder::default();
        let rec = decoder.decode_collective(&[], "context");
        assert_eq!(rec.confidence, 0.0);
        assert!(rec.content.is_empty());
    }

    #[test]
    fn decode_missing_context() {
        let decoder = TileDecoder::default();
        let mut tile = test_tile("Test", HashMap::new(), 0.5);
        tile.context_required = vec!["Python".into(), "ML".into()];
        let rec = decoder.decode(&tile, "We discussed Rust and databases");
        assert!(rec
            .inferred_facts
            .iter()
            .any(|f| f.contains("missing context")));
    }
}
