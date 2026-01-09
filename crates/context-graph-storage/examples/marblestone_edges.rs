//! Marblestone Edge Example
//!
//! Demonstrates GraphEdge creation with neurotransmitter weights and domain modulation.
//! This example shows the Marblestone architecture features for neural-inspired edge weighting.
//!
//! Run with: `cargo run --package context-graph-storage --example marblestone_edges`

use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use context_graph_core::types::{GraphEdge, MemoryNode};
use context_graph_storage::{Memex, RocksDbMemex};
use tempfile::TempDir;

/// Creates a valid 1536-dimensional normalized embedding vector.
fn create_valid_embedding() -> Vec<f32> {
    const DIM: usize = 1536;
    let val = 1.0_f32 / (DIM as f32).sqrt();
    vec![val; DIM]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Marblestone Edge Example ===\n");

    // Create temp directory for database
    let temp_dir = TempDir::new()?;
    println!("Created temp database at: {:?}\n", temp_dir.path());

    // Open RocksDbMemex
    let memex = RocksDbMemex::open(temp_dir.path())?;

    // ========================================
    // Example 1: Create Connected Nodes
    // ========================================
    println!("--- Example 1: Create Connected Nodes ---");

    let node1 = MemoryNode::new(
        "async/await pattern in Rust".to_string(),
        create_valid_embedding(),
    );
    node1.validate()?;

    let node2 = MemoryNode::new(
        "Tokio runtime implementation".to_string(),
        create_valid_embedding(),
    );
    node2.validate()?;

    let node3 = MemoryNode::new(
        "Future trait and Pin semantics".to_string(),
        create_valid_embedding(),
    );
    node3.validate()?;

    // Store nodes
    memex.store_node(&node1)?;
    memex.store_node(&node2)?;
    memex.store_node(&node3)?;

    println!("Created 3 nodes:");
    println!("  Node 1: {} (ID: {})", node1.content, node1.id);
    println!("  Node 2: {} (ID: {})", node2.content, node2.id);
    println!("  Node 3: {} (ID: {})", node3.content, node3.id);
    println!();

    // ========================================
    // Example 2: Domain-Specific NT Weights
    // ========================================
    println!("--- Example 2: Domain-Specific NT Weights ---");

    // Show different domain profiles
    let domains = [
        Domain::Code,
        Domain::Legal,
        Domain::Medical,
        Domain::Creative,
        Domain::Research,
        Domain::General,
    ];

    println!("Neurotransmitter Weight Profiles by Domain:");
    for domain in domains {
        let weights = NeurotransmitterWeights::for_domain(domain);
        println!(
            "  {:?}: excitatory={:.2}, inhibitory={:.2}, modulatory={:.2}",
            domain, weights.excitatory, weights.inhibitory, weights.modulatory
        );
    }
    println!();

    // ========================================
    // Example 3: Create Edge with Code Domain
    // ========================================
    println!("--- Example 3: Create Edge with Code Domain ---");

    // Create edge with Code domain (NT weights auto-configured)
    let edge1 = GraphEdge::new(node1.id, node2.id, EdgeType::Causal, Domain::Code);

    println!("Edge 1 (async/await -> Tokio):");
    println!("  Type: {:?}", edge1.edge_type);
    println!("  Domain: {:?}", edge1.domain);
    println!("  Base Weight: {:.3}", edge1.weight);
    println!("  Confidence: {:.3}", edge1.confidence);
    println!("Code Domain NT Weights:");
    println!(
        "  Excitatory: {:.3}",
        edge1.neurotransmitter_weights.excitatory
    );
    println!(
        "  Inhibitory: {:.3}",
        edge1.neurotransmitter_weights.inhibitory
    );
    println!(
        "  Modulatory: {:.3}",
        edge1.neurotransmitter_weights.modulatory
    );
    println!();

    // ========================================
    // Example 4: Modulated Weight Calculation
    // ========================================
    println!("--- Example 4: Modulated Weight Calculation ---");

    // get_modulated_weight uses the edge's base weight and NT weights
    let modulated = edge1.get_modulated_weight();
    let nt = &edge1.neurotransmitter_weights;

    println!("Modulated Weight Calculation:");
    println!("  Base weight: {:.3}", edge1.weight);
    println!(
        "  NT Weights: E={:.3}, I={:.3}, M={:.3}",
        nt.excitatory, nt.inhibitory, nt.modulatory
    );
    println!("  Steering reward: {:.3}", edge1.steering_reward);
    println!("  Modulated weight: {:.3}", modulated);
    assert!(
        (0.0..=1.0).contains(&modulated),
        "Modulated weight should be in [0, 1]"
    );
    println!("  ✓ Modulation calculation verified\n");

    // ========================================
    // Example 5: Steering Reward System
    // ========================================
    println!("--- Example 5: Steering Reward System ---");

    let mut edge = edge1.clone();
    println!("Initial steering reward: {:.3}", edge.steering_reward);

    // Apply positive steering (good retrieval feedback)
    edge.apply_steering_reward(0.5);
    println!("After +0.5 reward: {:.3}", edge.steering_reward);

    // Apply more positive steering (accumulates)
    edge.apply_steering_reward(0.3);
    println!("After another +0.3 reward: {:.3}", edge.steering_reward);

    // Apply negative steering (bad feedback)
    edge.apply_steering_reward(-0.4);
    println!("After -0.4 reward: {:.3}", edge.steering_reward);

    // Values are clamped to [-1, 1]
    edge.apply_steering_reward(2.0);
    println!("After +2.0 (clamped): {:.3}", edge.steering_reward);
    assert!(
        edge.steering_reward <= 1.0,
        "Steering reward should clamp to 1.0"
    );
    println!("  ✓ Steering reward clamping verified\n");

    // ========================================
    // Example 6: Edge Types and Default Weights
    // ========================================
    println!("--- Example 6: Edge Types and Default Weights ---");

    let edge_types = [
        EdgeType::Semantic,
        EdgeType::Temporal,
        EdgeType::Causal,
        EdgeType::Hierarchical,
    ];

    for etype in edge_types {
        let default_weight = etype.default_weight();
        println!("  {:?}: default_weight={:.2}", etype, default_weight);
    }
    println!();

    // ========================================
    // Example 7: Store and Retrieve Edge
    // ========================================
    println!("--- Example 7: Store and Retrieve Edge ---");

    // Store the edge with steering reward
    memex.store_edge(&edge)?;
    println!("Stored edge: {} -> {}", edge.source_id, edge.target_id);

    // Retrieve from database (source of truth)
    let retrieved = memex.get_edge(&edge.source_id, &edge.target_id, edge.edge_type)?;
    println!("Retrieved edge:");
    println!("  Source: {}", retrieved.source_id);
    println!("  Target: {}", retrieved.target_id);
    println!("  Type: {:?}", retrieved.edge_type);
    println!("  Domain: {:?}", retrieved.domain);
    println!("  Steering Reward: {:.3}", retrieved.steering_reward);

    assert_eq!(retrieved.steering_reward, edge.steering_reward);
    println!("  ✓ Edge retrieval verified\n");

    // ========================================
    // Example 8: Outgoing Edge Query
    // ========================================
    println!("--- Example 8: Outgoing Edge Query ---");

    // Create another edge from node1
    let edge2 = GraphEdge::new(node1.id, node3.id, EdgeType::Semantic, Domain::Code);
    memex.store_edge(&edge2)?;

    // Get all outgoing edges from node1
    let outgoing = memex.get_edges_from(&node1.id)?;
    println!("Outgoing edges from node1:");
    for e in &outgoing {
        println!("  -> {} ({:?})", e.target_id, e.edge_type);
    }
    assert_eq!(outgoing.len(), 2, "Should have 2 outgoing edges");
    println!("  ✓ Found {} outgoing edges\n", outgoing.len());

    // ========================================
    // Example 9: Incoming Edge Query
    // ========================================
    println!("--- Example 9: Incoming Edge Query ---");

    // Get all incoming edges to node2
    let incoming = memex.get_edges_to(&node2.id)?;
    println!("Incoming edges to node2:");
    for e in &incoming {
        println!("  <- {} ({:?})", e.source_id, e.edge_type);
    }
    assert_eq!(incoming.len(), 1, "Should have 1 incoming edge");
    println!("  ✓ Found {} incoming edge(s)\n", incoming.len());

    // ========================================
    // Example 10: Amortized Shortcut Creation
    // ========================================
    println!("--- Example 10: Amortized Shortcuts ---");

    let mut shortcut = GraphEdge::new(node1.id, node3.id, EdgeType::Causal, Domain::Code);
    println!("Before shortcut marking:");
    println!(
        "  is_amortized_shortcut: {}",
        shortcut.is_amortized_shortcut
    );
    println!("  traversal_count: {}", shortcut.traversal_count);
    println!("  confidence: {:.2}", shortcut.confidence);
    println!("  steering_reward: {:.2}", shortcut.steering_reward);
    println!(
        "  is_reliable_shortcut: {}",
        shortcut.is_reliable_shortcut()
    );

    // Simulate multiple traversals (Marblestone amortized learning)
    for i in 0..6 {
        shortcut.record_traversal();
        println!("  Traversal {}: count={}", i + 1, shortcut.traversal_count);
    }

    // Apply positive feedback (required for reliable shortcut)
    shortcut.apply_steering_reward(0.5);
    // Increase confidence (required for reliable shortcut >= 0.7)
    shortcut.confidence = 0.8;

    // Mark as shortcut (happens when 3+ hop path traversed >=5x)
    shortcut.mark_as_shortcut();
    println!("\nAfter shortcut marking:");
    println!(
        "  is_amortized_shortcut: {}",
        shortcut.is_amortized_shortcut
    );
    println!("  traversal_count: {}", shortcut.traversal_count);
    println!("  confidence: {:.2}", shortcut.confidence);
    println!("  steering_reward: {:.2}", shortcut.steering_reward);
    println!(
        "  is_reliable_shortcut: {}",
        shortcut.is_reliable_shortcut()
    );

    // is_reliable_shortcut requires: is_amortized_shortcut && traversal >= 3 && reward > 0.3 && confidence >= 0.7
    assert!(shortcut.is_amortized_shortcut);
    assert!(shortcut.is_reliable_shortcut());
    println!("  ✓ Amortized shortcut verified\n");

    // ========================================
    // Example 11: Edge Age
    // ========================================
    println!("--- Example 11: Edge Age ---");

    let new_edge = GraphEdge::new(node2.id, node3.id, EdgeType::Temporal, Domain::Code);
    let age_seconds = new_edge.age_seconds();
    println!("New edge age: {} seconds", age_seconds);
    assert!(
        age_seconds < 5,
        "Fresh edge should be less than 5 seconds old"
    );
    println!("  ✓ Edge age tracking working\n");

    // ========================================
    // Summary: Database State
    // ========================================
    println!("--- Final Database State ---");

    // Use Memex trait for health_check to get StorageHealth
    let memex_trait: &dyn Memex = &memex;
    let health = memex_trait.health_check()?;
    println!("Database Health:");
    println!("  Nodes: {}", health.node_count);
    println!("  Edges: {}", health.edge_count);
    println!("  Is Healthy: {}", health.is_healthy);

    assert!(health.is_healthy);

    println!("\n=== All Examples Completed Successfully ===");
    Ok(())
}
