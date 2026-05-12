use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::tile::Tile;

/// Ebbinghaus-inspired memory decay schedule.
#[derive(Debug, Clone)]
pub struct DecaySchedule {
    pub half_life: Duration,
    pub valence_weight: f64,
    pub min_valence_for_permanent: f64,
}

impl Default for DecaySchedule {
    fn default() -> Self {
        Self {
            half_life: Duration::from_secs(7 * 24 * 3600), // 1 week
            valence_weight: 0.5,
            min_valence_for_permanent: 0.9,
        }
    }
}

impl DecaySchedule {
    /// Ebbinghaus forgetting curve: retention = e^(-t/S)
    ///
    /// Returns a value between 0.0 and 1.0 representing how well this tile
    /// should be retained. High-valence tiles decay slower.
    pub fn retention(&self, tile: &Tile, now: DateTime<Utc>) -> f64 {
        // Permanent memories never decay.
        if tile.valence >= self.min_valence_for_permanent {
            return 1.0;
        }

        let elapsed = now.signed_duration_since(tile.accessed_at);
        let elapsed_secs = elapsed.num_seconds().max(0) as f64;
        let half_life_secs = self.half_life.as_secs() as f64;

        // Effective half-life: high-valence memories decay slower.
        let valence_factor = 1.0 + self.valence_weight * tile.valence;
        let effective_half_life = half_life_secs * valence_factor;

        // Access count slows decay (spaced repetition effect).
        let access_factor = 1.0 + (tile.access_count as f64).ln().max(0.0) * 0.3;
        let adjusted_half_life = effective_half_life * access_factor;

        let decay_constant = 0.693 / adjusted_half_life; // ln(2) / half_life
        let retention = (-decay_constant * elapsed_secs).exp();

        retention.clamp(0.0, 1.0)
    }

    /// Should this tile be forgotten based on decay?
    pub fn should_forget(&self, tile: &Tile, now: DateTime<Utc>) -> bool {
        self.retention(tile, now) < 0.1 // Below 10% retention = forget.
    }

    /// Reconsolidate: update tile with new context, resetting decay.
    ///
    /// This models the neurobiological process where recalling a memory
    /// makes it labile again, and re-storing it can strengthen or modify it.
    pub fn reconsolidate(&self, tile: &mut Tile, new_context: &str) {
        // Reset the access clock.
        tile.accessed_at = Utc::now();
        tile.access_count += 1;

        // If new context adds constraints, merge them.
        if !new_context.is_empty() {
            // Extract key phrases from new context and add to summary.
            let additions: Vec<&str> = new_context
                .split('.')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .take(3)
                .collect();

            if !additions.is_empty() {
                tile.summary = format!("{} [updated: {}]", tile.summary, additions.join("; "));
            }
        }

        // Slightly increase valence (memory is being reinforced).
        tile.valence = (tile.valence + 0.05).min(1.0);
    }

    /// Create a fast-decay schedule (for testing).
    pub fn fast_decay() -> Self {
        Self {
            half_life: Duration::from_secs(60), // 1 minute
            valence_weight: 0.3,
            min_valence_for_permanent: 0.95,
        }
    }

    /// Create a slow-decay schedule (permanent-ish memories).
    pub fn slow_decay() -> Self {
        Self {
            half_life: Duration::from_secs(365 * 24 * 3600), // 1 year
            valence_weight: 1.0,
            min_valence_for_permanent: 0.7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_tile(valence: f64) -> Tile {
        Tile {
            id: crate::tile::TileId::new(),
            source_hash: String::new(),
            constraints: HashMap::new(),
            summary: "test tile".into(),
            context_required: vec![],
            valence,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
            generation: 0,
            parent_id: None,
        }
    }

    #[test]
    fn fresh_tile_full_retention() {
        let schedule = DecaySchedule::default();
        let tile = test_tile(0.5);
        let retention = schedule.retention(&tile, Utc::now());
        assert!(retention > 0.99, "fresh tile should have ~1.0 retention, got {}", retention);
    }

    #[test]
    fn permanent_never_decays() {
        let schedule = DecaySchedule::default();
        let mut tile = test_tile(0.95);
        tile.accessed_at = Utc::now() - chrono::Duration::days(365);
        let retention = schedule.retention(&tile, Utc::now());
        assert_eq!(retention, 1.0);
    }

    #[test]
    fn fast_decay_forgets_quickly() {
        let schedule = DecaySchedule::fast_decay();
        let mut tile = test_tile(0.1);
        tile.accessed_at = Utc::now() - chrono::Duration::minutes(10);
        assert!(schedule.should_forget(&tile, Utc::now()));
    }

    #[test]
    fn reconsolidate_resets_decay() {
        let schedule = DecaySchedule::default();
        let mut tile = test_tile(0.5);
        // Age the tile.
        tile.accessed_at = Utc::now() - chrono::Duration::days(30);
        let before_retention = schedule.retention(&tile, Utc::now());

        // Reconsolidate.
        schedule.reconsolidate(&mut tile, "New information about the project");

        let after_retention = schedule.retention(&tile, Utc::now());
        assert!(after_retention > before_retention);
        assert_eq!(tile.access_count, 1);
        assert!(tile.valence > 0.5);
    }

    #[test]
    fn access_count_slows_decay() {
        let schedule = DecaySchedule::default();
        let mut tile1 = test_tile(0.5);
        tile1.accessed_at = Utc::now() - chrono::Duration::days(7);
        tile1.access_count = 1;

        let mut tile2 = test_tile(0.5);
        tile2.accessed_at = Utc::now() - chrono::Duration::days(7);
        tile2.access_count = 100;

        let r1 = schedule.retention(&tile1, Utc::now());
        let r2 = schedule.retention(&tile2, Utc::now());
        assert!(r2 > r1, "more accesses should retain better: {} vs {}", r2, r1);
    }

    #[test]
    fn high_valence_decays_slower() {
        let schedule = DecaySchedule::default();
        let mut low = test_tile(0.1);
        low.accessed_at = Utc::now() - chrono::Duration::days(7);
        let mut high = test_tile(0.8);
        high.accessed_at = Utc::now() - chrono::Duration::days(7);

        let r_low = schedule.retention(&low, Utc::now());
        let r_high = schedule.retention(&high, Utc::now());
        assert!(r_high > r_low);
    }
}
