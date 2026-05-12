use std::time::Duration;

use memory_crystal::{
    Crystal, CrystalStats, DecaySchedule, SalienceMap, TelephoneChain,
};

fn main() -> memory_crystal::Result<()> {
    println!("=== Memory Crystal: Basic Example ===\n");

    // Create a temporary crystal.
    let tmp = tempfile::tempdir().expect("create temp dir");
    let mut crystal = Crystal::open(tmp.path())?;
    println!("Opened crystal at: {:?}", tmp.path());

    // Crystallize some memories.
    let id1 = crystal.crystallize(
        "Alice discovered a critical bug in the production system on March 15th. \
         The database was losing records during peak traffic hours.",
        SalienceMap {
            valence: Some(0.9),
            ..Default::default()
        },
    )?;
    println!("\nCrystallized tile: {}", id1);

    let id2 = crystal.crystallize(
        "Team standup notes: Bob is working on the API refactor. \
         Carol will handle the deployment pipeline. Sprint ends Friday.",
        SalienceMap {
            valence: Some(0.4),
            ..Default::default()
        },
    )?;
    println!("Crystallized tile: {}", id2);

    // Recall with context.
    println!("\n--- Recall with context ---");
    let rec1 = crystal.recall(&id1, "production bug database March")?;
    println!("Recall tile 1 (confidence: {:.2}):", rec1.confidence);
    println!("  Content: {}", rec1.content);
    println!("  Preserved facts: {:?}", rec1.preserved_facts);
    println!("  Inferred facts: {:?}", rec1.inferred_facts);

    // Collective recall.
    println!("\n--- Collective Recall ---");
    let results = crystal.recall_collective("production bug", 5)?;
    for rec in &results {
        println!("  [{}] {:.2}: {}", rec.tile_id, rec.confidence, rec.content.chars().take(60).collect::<String>());
    }

    // Reconsolidate.
    println!("\n--- Reconsolidation ---");
    crystal.reconsolidate(&id1, "The bug was in the connection pool, not the database itself")?;
    let rec1_updated = crystal.recall(&id1, "bug connection pool")?;
    println!("After reconsolidation: {}", rec1_updated.content.chars().take(80).collect::<String>());

    // Stats.
    println!("\n--- Crystal Stats ---");
    let stats = crystal.stats();
    println!("  Tiles: {}", stats.tile_count);
    println!("  Avg valence: {:.2}", stats.avg_valence);

    // Telephone chain.
    println!("\n--- Telephone Chain ---");
    let mut chain = TelephoneChain::new(
        "Alice reported a critical emergency in Seattle on March 15th involving Project Alpha."
    );
    chain.add_round("Alice reported something important in Seattle on March 15th.");
    chain.add_round("Someone reported an issue in Seattle recently.");
    chain.add_round("There was an issue reported.");

    println!("Chain length: {} rounds", chain.len());
    let timeline = chain.fact_timeline();
    println!("Fact timeline ({} facts tracked):", timeline.len());
    for (fact, rounds) in &timeline {
        let survived: usize = rounds.iter().filter(|&&b| b).count();
        println!("  {} survived {}/{} rounds", fact, survived, rounds.len());
    }

    // Decay.
    println!("\n--- Decay Schedule ---");
    let decay = DecaySchedule::default();
    println!("Half-life: {:?}", decay.half_life);
    println!("Permanent threshold: {:.1}", decay.min_valence_for_permanent);

    // Forget (with very short duration — unlikely to forget fresh tiles).
    let forgotten = crystal.forget(Duration::from_secs(1))?;
    println!("\nForgotten tiles: {}", forgotten);

    println!("\n=== Done ===");
    Ok(())
}
