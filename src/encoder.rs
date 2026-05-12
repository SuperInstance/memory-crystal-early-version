use std::collections::HashMap;

use crate::tile::{SalienceMap, Tile, TileId};

/// Lossy encoder: compresses raw content into a tile.
pub struct TileEncoder {
    pub max_summary_bytes: usize,
    pub constraint_extractor: ConstraintExtractor,
}

impl Default for TileEncoder {
    fn default() -> Self {
        Self {
            max_summary_bytes: 512,
            constraint_extractor: ConstraintExtractor::default(),
        }
    }
}

impl TileEncoder {
    /// Encode content into a tile using the provided salience map.
    pub fn encode(&self, content: &str, salience: &SalienceMap) -> Tile {
        let summary = self.compress(content);
        let constraints = if salience.constraints.is_empty() {
            self.constraint_extractor.extract(content)
        } else {
            salience.constraints.clone()
        };
        let valence = salience.valence.unwrap_or_else(|| self.constraint_extractor.valence(content));

        Tile {
            id: TileId::new(),
            source_hash: Tile::hash_content(content),
            constraints,
            summary,
            context_required: if salience.context_required.is_empty() {
                self.infer_context(content)
            } else {
                salience.context_required.clone()
            },
            valence,
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 0,
            generation: 0,
            parent_id: None,
        }
    }

    /// Encode with explicit valence override.
    pub fn encode_with_valence(&self, content: &str, valence: f64) -> Tile {
        let salience = SalienceMap {
            valence: Some(valence),
            ..Default::default()
        };
        self.encode(content, &salience)
    }

    /// Compress content to fit within max_summary_bytes.
    fn compress(&self, content: &str) -> String {
        if content.len() <= self.max_summary_bytes {
            return content.to_string();
        }

        // Take sentences until we hit the byte limit.
        let mut summary = String::new();
        for sentence in content.split_inclusive(|c: char| c == '.' || c == '!' || c == '?') {
            if summary.len() + sentence.len() > self.max_summary_bytes {
                break;
            }
            summary.push_str(sentence);
        }

        if summary.is_empty() {
            // Fallback: truncate at byte boundary (char-aligned).
            let end = content
                .char_indices()
                .take_while(|(i, _)| *i < self.max_summary_bytes)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(self.max_summary_bytes.min(content.len()));
            summary = content[..end].to_string();
        }

        summary
    }

    /// Infer what context would be needed to reconstruct this content.
    fn infer_context(&self, content: &str) -> Vec<String> {
        let mut ctx = Vec::new();
        // Extract capitalized phrases as potential context cues.
        for word in content.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if trimmed.starts_with(char::is_uppercase) && trimmed.len() > 2 {
                ctx.push(trimmed.to_string());
            }
        }
        ctx.dedup();
        ctx.truncate(10);
        ctx
    }
}

/// Extracts "immortal facts" — proper nouns, numbers, dramatic phrases.
#[derive(Default)]
pub struct ConstraintExtractor;

impl ConstraintExtractor {
    /// Extract immutable facts from content.
    pub fn extract(&self, content: &str) -> HashMap<String, String> {
        let mut constraints = HashMap::new();

        // Extract numbers with units or standalone.
        for word in content.split_whitespace() {
            if word.chars().any(|c| c.is_ascii_digit()) {
                let key = format!("num_{}", constraints.len());
                constraints.insert(key, word.to_string());
            }
        }

        // Extract capitalized phrases (proper nouns).
        let mut proper_nouns = Vec::new();
        for word in content.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if trimmed.starts_with(char::is_uppercase) && trimmed.len() > 2 && !proper_nouns.contains(&trimmed) {
                proper_nouns.push(trimmed);
            }
        }
        if !proper_nouns.is_empty() {
            constraints.insert("proper_nouns".into(), proper_nouns.join(", "));
        }

        // Extract quoted strings as important.
        let mut in_quote = false;
        let mut quoted = String::new();
        let mut quotes = Vec::new();
        for ch in content.chars() {
            match ch {
                '"' | '\'' if in_quote => {
                    in_quote = false;
                    if !quoted.is_empty() {
                        quotes.push(quoted.clone());
                    }
                    quoted.clear();
                }
                '"' | '\'' => {
                    in_quote = true;
                }
                _ if in_quote => {
                    quoted.push(ch);
                }
                _ => {}
            }
        }
        if !quotes.is_empty() {
            constraints.insert("quoted".into(), quotes.join("; "));
        }

        constraints
    }

    /// Score emotional valence from text features (0.0–1.0).
    pub fn valence(&self, content: &str) -> f64 {
        let high_valence = [
            "amazing", "incredible", "excited", "love", "fantastic", "breakthrough",
            "critical", "urgent", "important", "dramatic", "shocking", "unbelievable",
            "urgent", "crisis", "emergency", "disaster", "triumph", "victory",
        ];
        let low_valence = [
            "maybe", "perhaps", "minor", "routine", "normal", "fine", "okay",
            "standard", "regular", "typical", "usual",
        ];

        let lower = content.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();

        let high_count = words.iter().filter(|w| high_valence.contains(&w.as_ref())).count();
        let low_count = words.iter().filter(|w| low_valence.contains(&w.as_ref())).count();

        if high_count + low_count == 0 {
            return 0.3; // Default low-medium valence.
        }

        let base = high_count as f64 / (high_count + low_count) as f64;
        // Boost for exclamation marks and caps.
        let excl_boost = content.matches('!').count().min(5) as f64 * 0.05;
        let caps_boost = content
            .chars()
            .filter(|c| c.is_uppercase())
            .count()
            .min(20) as f64
            * 0.005;

        (base + excl_boost + caps_boost).clamp(0.0, 1.0)
    }

    /// Compute compression ratio.
    pub fn compression_ratio(&self, original: &str, tile: &Tile) -> f64 {
        if original.is_empty() {
            return 1.0;
        }
        tile.summary.len() as f64 / original.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_basic() {
        let encoder = TileEncoder::default();
        let tile = encoder.encode("Hello world. This is a test.", &SalienceMap::default());
        assert!(!tile.summary.is_empty());
        assert_eq!(tile.generation, 0);
        assert!(tile.parent_id.is_none());
    }

    #[test]
    fn encode_with_valence() {
        let encoder = TileEncoder::default();
        let tile = encoder.encode_with_valence("Boring content.", 0.95);
        assert!((tile.valence - 0.95).abs() < 0.001);
    }

    #[test]
    fn compress_long_content() {
        let encoder = TileEncoder {
            max_summary_bytes: 50,
            ..Default::default()
        };
        let long = "This is sentence one. This is sentence two. This is sentence three.";
        let tile = encoder.encode(long, &SalienceMap::default());
        assert!(tile.summary.len() <= 60); // Some slack for sentence boundaries.
    }

    #[test]
    fn extract_constraints() {
        let extractor = ConstraintExtractor;
        let constraints = extractor.extract("Alice met Bob at 42nd Street on March 15th.");
        assert!(constraints.contains_key("proper_nouns"));
        assert!(constraints.values().any(|v| v.contains("42nd")));
    }

    #[test]
    fn valence_high() {
        let extractor = ConstraintExtractor;
        let v = extractor.valence("This is an amazing breakthrough!! Incredible results!");
        assert!(v > 0.5, "expected high valence, got {}", v);
    }

    #[test]
    fn valence_low() {
        let extractor = ConstraintExtractor;
        let v = extractor.valence("This is a routine update. Nothing special.");
        assert!(v < 0.5, "expected low valence, got {}", v);
    }

    #[test]
    fn compression_ratio() {
        let extractor = ConstraintExtractor;
        let tile = Tile {
            id: TileId::new(),
            source_hash: String::new(),
            constraints: HashMap::new(),
            summary: "short".into(),
            context_required: vec![],
            valence: 0.5,
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 0,
            generation: 0,
            parent_id: None,
        };
        let ratio = extractor.compression_ratio("a much longer original content string", &tile);
        assert!(ratio < 1.0);
        assert!(ratio > 0.0);
    }
}
