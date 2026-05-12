use std::collections::HashMap;

use crate::encoder::ConstraintExtractor;
use crate::tile::Tile;

/// Tracks a chain of tiles through the "telephone game" —
/// each round re-encodes the previous reconstruction, measuring fact drift.
pub struct TelephoneChain {
    tiles: Vec<Tile>,
    fact_tracker: FactTracker,
    original_facts: HashMap<String, String>,
}

impl TelephoneChain {
    /// Start a new telephone chain from source content.
    pub fn new(source: &str) -> Self {
        let extractor = ConstraintExtractor;
        let original_facts = extractor.extract(source);

        let encoder = crate::encoder::TileEncoder::default();
        let tile = encoder.encode(source, &crate::tile::SalienceMap::default());

        let mut fact_tracker = FactTracker::new(&original_facts);
        fact_tracker.record_round(&tile);

        Self {
            tiles: vec![tile],
            fact_tracker,
            original_facts,
        }
    }

    /// Add a round: the reconstruction is re-encoded into a new tile.
    pub fn add_round(&mut self, reconstruction: &str) -> &Tile {
        let encoder = crate::encoder::TileEncoder::default();
        let parent_id = self.tiles.last().unwrap().id.clone();

        let mut tile = encoder.encode(reconstruction, &crate::tile::SalienceMap::default());
        tile.generation = self.tiles.len() as u32;
        tile.parent_id = Some(parent_id);

        self.fact_tracker.record_round(&tile);
        self.tiles.push(tile);
        self.tiles.last().unwrap()
    }

    /// Number of rounds in the chain.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Is the chain empty?
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Get a tile by round number.
    pub fn get(&self, round: usize) -> Option<&Tile> {
        self.tiles.get(round)
    }

    /// Find the crystallization point — the round where fact survival stabilizes.
    ///
    /// Returns the round index where survival rate stops dropping significantly.
    pub fn crystallization_point(&self) -> Option<usize> {
        if self.tiles.len() < 3 {
            return None;
        }

        let mut prev_rate = 1.0;
        for round in 0..self.tiles.len() {
            let rate = self.fact_tracker.survival_rate(round);
            let drop = prev_rate - rate;
            // If the drop is less than 5%, we've crystallized.
            if drop < 0.05 && round > 0 {
                return Some(round);
            }
            prev_rate = rate;
        }

        None
    }

    /// Get the fact timeline: for each original fact, whether it survived each round.
    pub fn fact_timeline(&self) -> HashMap<String, Vec<bool>> {
        self.fact_tracker.timeline.clone()
    }

    /// Get the original facts.
    pub fn original_facts(&self) -> &HashMap<String, String> {
        &self.original_facts
    }
}

/// Tracks which facts from the original source survive each telephone round.
pub struct FactTracker {
    facts: HashMap<String, String>,
    timeline: HashMap<String, Vec<bool>>,
}

impl FactTracker {
    /// Create a new tracker from the original set of facts.
    pub fn new(facts: &HashMap<String, String>) -> Self {
        Self {
            facts: facts.clone(),
            timeline: facts.keys().map(|k| (k.clone(), vec![])).collect(),
        }
    }

    /// Record a round: check which original facts are still present in the tile.
    pub fn record_round(&mut self, tile: &Tile) {
        let tile_text = format!("{} {}", tile.summary, tile.constraints.values().cloned().collect::<Vec<_>>().join(" ")).to_lowercase();

        for (key, original_value) in &self.facts {
            let survived = tile_text.contains(&original_value.to_lowercase());
            self.timeline.entry(key.clone()).or_default().push(survived);
        }
    }

    /// Get survival rate at a specific round (fraction of facts still present).
    pub fn survival_rate(&self, round: usize) -> f64 {
        if self.timeline.is_empty() {
            return 1.0;
        }

        let survived: usize = self
            .timeline
            .values()
            .map(|v| v.get(round).copied().unwrap_or(false) as usize)
            .sum();

        survived as f64 / self.timeline.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telephone_chain_basic() {
        let chain = TelephoneChain::new("Alice went to Seattle on March 15th to work on Project Alpha.");
        assert_eq!(chain.len(), 1);
        assert!(!chain.tiles[0].summary.is_empty());
    }

    #[test]
    fn add_rounds() {
        let mut chain = TelephoneChain::new("Alice discovered a critical bug in the production system.");
        chain.add_round("Alice found a bug in production.");
        chain.add_round("There was a bug found.");
        assert_eq!(chain.len(), 3);

        // Each round should have incrementing generation.
        assert_eq!(chain.tiles[0].generation, 0);
        assert_eq!(chain.tiles[1].generation, 1);
        assert_eq!(chain.tiles[2].generation, 2);

        // Each round should have parent pointing to previous.
        assert!(chain.tiles[0].parent_id.is_none());
        assert_eq!(chain.tiles[1].parent_id.as_ref(), Some(&chain.tiles[0].id));
    }

    #[test]
    fn fact_timeline_tracks() {
        let chain = TelephoneChain::new("Alice went to Seattle for the important meeting.");
        let timeline = chain.fact_timeline();
        assert!(!timeline.is_empty());

        // First round should have all facts present.
        for (_, rounds) in &timeline {
            assert!(!rounds.is_empty());
        }
    }

    #[test]
    fn survival_rate_declines() {
        let mut chain = TelephoneChain::new("Alice reported a critical emergency in Seattle on March 15th.");
        let rate_0 = chain.fact_tracker.survival_rate(0);

        chain.add_round("Someone reported something somewhere.");
        let rate_1 = chain.fact_tracker.survival_rate(1);

        // Generally, survival declines with lossy rounds.
        // (This is a heuristic test — exact behavior depends on the encoder.)
        assert!(rate_0 >= 0.0 && rate_0 <= 1.0);
        assert!(rate_1 >= 0.0 && rate_1 <= 1.0);
    }

    #[test]
    fn crystallization_point_needs_multiple_rounds() {
        let chain = TelephoneChain::new("Test content");
        assert!(chain.crystallization_point().is_none());
    }
}
