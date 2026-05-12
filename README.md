# memory-crystal

**Crystallized memory: lossy, reconstructive persistence for AI agents.**

Based on the [Tile Compression Theorem](https://en.wikipedia.org/wiki/Forgetting_curve) and Ebbinghaus forgetting curves. Memory Crystal implements the insight that human memory doesn't store verbatim records — it stores compressed tiles that are *reconstructed* on recall using context, and *decay* over time unless reinforced.

## Features

- **Lossy encoding** — compresses content into tiles with immortal constraints and compressed summaries
- **Context-dependent recall** — reconstructs memories differently depending on what context is available
- **Ebbinghaus decay** — memories fade following exponential forgetting curves, modified by emotional valence and access frequency
- **Reconsolidation** — recalling a memory resets its decay clock and can strengthen it (just like biological memory)
- **Telephone chains** — measure fact drift across successive lossy re-encodings
- **Disk persistence** — tiles are stored as JSON files, indexed in memory for fast queries
- **Constraint extraction** — automatically extracts proper nouns, numbers, and quoted strings as "immortal facts"

## Quick Start

```rust
use memory_crystal::{Crystal, SalienceMap};
use std::time::Duration;

// Open a crystal (directory on disk).
let mut crystal = Crystal::open("my_memories".as_ref())?;

// Crystallize a memory.
let id = crystal.crystallize(
    "Alice discovered a critical bug in production on March 15th.",
    SalienceMap { valence: Some(0.9), ..Default::default() },
)?;

// Recall with context (reconstruction depends on what you provide).
let rec = crystal.recall(&id, "production bug March")?;
println!("Confidence: {:.2}", rec.confidence);
println!("Content: {}", rec.content);
println!("Preserved facts: {:?}", rec.preserved_facts);

// Reconsolidate (strengthens memory, resets decay).
crystal.reconsolidate(&id, "The bug was in the connection pool")?;

// Forget old, low-valence memories.
let forgotten = crystal.forget(Duration::from_secs(30 * 24 * 3600))?;
println!("Forgot {} tiles", forgotten);
```

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  TileEncoder │────▶│     Tile      │────▶│  TileDecoder  │
│ (lossy)      │     │ (crystal)     │     │ (reconstruct) │
└─────────────┘     └──────┬───────┘     └──────────────┘
                    ┌──────┴───────┐
                    │ DecaySchedule│
                    │ (forgetting) │
                    └──────────────┘
```

- **TileEncoder**: Compresses content into a tile with constraints, summary, valence
- **TileDecoder**: Reconstructs content from a tile using context, reporting confidence
- **DecaySchedule**: Ebbinghaus forgetting curve — high-valence tiles decay slower, frequent access slows decay
- **CrystalIndex**: In-memory index for constraint-based, valence-based, and time-based queries
- **TelephoneChain**: Measures fact survival across successive lossy re-encodings

## Core Concepts

### Tiles
A tile is a lossy compression of content. It preserves:
- **Constraints** (immortal facts): proper nouns, numbers, key phrases
- **Summary**: compressed text that fits within a rate limit
- **Valence**: emotional salience (0.0–1.0) — high-valence memories decay slower

### Decay
Retention follows: `R(t) = e^(-λt)` where λ depends on half-life, valence, and access count. Tiles below 10% retention are candidates for forgetting.

### Reconsolidation
Like biological memory, recalling a tile makes it modifiable. Reconsolidation resets the decay clock, increases valence slightly, and can add new context to the summary.

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
memory-crystal = "0.1.0"
```

## License

MIT
