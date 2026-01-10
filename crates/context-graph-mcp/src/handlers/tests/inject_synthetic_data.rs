//! Production Database Injection Test
//!
//! This test injects synthetic data DIRECTLY into the production RocksDB database
//! to populate it with test data for verification.
//!
//! IMPORTANT: This writes to the PRODUCTION database at:
//! /home/cabdru/contextgraph/contextgraph_data
//!
//! Run with: cargo test -p context-graph-mcp inject_synthetic_production -- --nocapture

use std::path::PathBuf;
use std::sync::Arc;

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};

use context_graph_storage::teleological::RocksDbTeleologicalStore;

/// Production database path - this is the REAL database
const PRODUCTION_DB_PATH: &str = "/home/cabdru/contextgraph/contextgraph_data";

/// Number of embedders
const NUM_EMBEDDERS: usize = 13;

/// Embedding dimensions from semantic/constants.rs
const E1_DIM: usize = 1024;  // Semantic
const E2_DIM: usize = 512;   // Temporal Recent
const E3_DIM: usize = 512;   // Temporal Periodic
const E4_DIM: usize = 512;   // Temporal Positional
const E5_DIM: usize = 768;   // Causal
const E7_DIM: usize = 1536;  // Code
const E8_DIM: usize = 384;   // Graph
const E9_DIM: usize = 1024;  // HDC
const E10_DIM: usize = 768;  // Multimodal
const E11_DIM: usize = 384;  // Entity
const E12_TOKEN_DIM: usize = 128; // Per-token dimension

/// Synthetic test data - 10 memories covering diverse AI/ML topics
const SYNTHETIC_MEMORIES: [(&str, &str); 10] = [
    (
        "machine_learning_fundamentals",
        "Machine learning optimization techniques for neural networks including gradient descent, \
         Adam optimizer, learning rate scheduling, and batch normalization for improved convergence.",
    ),
    (
        "distributed_systems",
        "Distributed systems architecture patterns for high availability including load balancing, \
         replication strategies, consensus protocols like Raft and Paxos, and eventual consistency.",
    ),
    (
        "transformer_nlp",
        "Natural language processing with transformer models covering self-attention mechanisms, \
         positional encodings, BERT bidirectional training, and GPT autoregressive generation.",
    ),
    (
        "database_indexing",
        "Database indexing strategies for fast retrieval using B-trees, hash indexes, \
         covering indexes, and index-only scans for query optimization.",
    ),
    (
        "api_design",
        "API design best practices for scalability including REST principles, GraphQL schemas, \
         rate limiting strategies, caching layers, and idempotency guarantees.",
    ),
    (
        "rust_memory_safety",
        "Rust programming language memory safety guarantees through ownership, borrowing, \
         lifetimes, and the borrow checker preventing data races at compile time.",
    ),
    (
        "vector_embeddings",
        "Vector embedding spaces for semantic similarity including word2vec, sentence transformers, \
         cosine similarity metrics, and approximate nearest neighbor search with HNSW indexes.",
    ),
    (
        "consciousness_models",
        "Computational models of consciousness including Global Workspace Theory, \
         Integrated Information Theory (IIT), attention mechanisms, and meta-cognitive loops.",
    ),
    (
        "knowledge_graphs",
        "Knowledge graph construction and reasoning including entity extraction, \
         relation classification, graph neural networks, and ontology-based inference.",
    ),
    (
        "teleological_systems",
        "Teleological system design with purpose vectors, goal alignment metrics, \
         North Star guidance, and autonomous self-improvement through learning feedback loops.",
    ),
];

/// Create a synthetic TeleologicalFingerprint with realistic-looking data.
fn create_synthetic_fingerprint(content: &str, topic: &str) -> TeleologicalFingerprint {
    // Generate deterministic embeddings based on content hash
    let content_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash_bytes: [u8; 32] = hasher.finalize().into();
        hash_bytes
    };

    // Create synthetic semantic fingerprint with varied embedding values
    let semantic = create_synthetic_semantic(content, &content_hash);

    // Create purpose vector with varied alignment scores
    let purpose_vector = create_synthetic_purpose_vector(topic);

    // Create Johari fingerprint (all start in "unknown" quadrant)
    let johari = JohariFingerprint::zeroed();

    TeleologicalFingerprint::new(semantic, purpose_vector, johari, content_hash)
}

/// Create synthetic SemanticFingerprint with realistic embedding patterns.
fn create_synthetic_semantic(content: &str, hash: &[u8; 32]) -> SemanticFingerprint {
    // Use content hash bytes to seed pseudo-random values
    let seed = |offset: usize| -> f32 {
        let idx = offset % 32;
        let val = hash[idx] as f32 / 255.0;
        // Scale to [-1, 1] range typical for embeddings
        (val * 2.0) - 1.0
    };

    // E1: Semantic 1024D - main semantic meaning
    let e1_semantic: Vec<f32> = (0..E1_DIM).map(|i| seed(i) * 0.5).collect();

    // E2: Temporal Recent 512D
    let e2_temporal_recent: Vec<f32> = (0..E2_DIM).map(|i| seed(i + 1000)).collect();

    // E3: Temporal Periodic 512D
    let e3_temporal_periodic: Vec<f32> = (0..E3_DIM).map(|i| seed(i + 2000) * 0.8).collect();

    // E4: Temporal Positional 512D
    let e4_temporal_positional: Vec<f32> = (0..E4_DIM).map(|i| seed(i + 3000) * 0.6).collect();

    // E5: Causal 768D
    let e5_causal: Vec<f32> = (0..E5_DIM).map(|i| seed(i + 4000)).collect();

    // E6: Sparse (SPLADE-style) - use sparse indices (u16 for vocab)
    let e6_sparse_indices: Vec<u16> = (0..20)
        .map(|i| ((hash[i % 32] as u32 * 100) % 30000) as u16)
        .collect();
    let e6_sparse_values: Vec<f32> = (0..20).map(|i| seed(i + 5000).abs() * 2.0).collect();

    // E7: Code 1536D
    let e7_code: Vec<f32> = (0..E7_DIM).map(|i| seed(i + 6000)).collect();

    // E8: Graph 384D
    let e8_graph: Vec<f32> = (0..E8_DIM).map(|i| seed(i + 7000)).collect();

    // E9: HDC 1024D (projected from high-dimensional)
    let e9_hdc: Vec<f32> = (0..E9_DIM)
        .map(|i| if seed(i + 8000) > 0.0 { 1.0 } else { -1.0 })
        .collect();

    // E10: Multimodal 768D
    let e10_multimodal: Vec<f32> = (0..E10_DIM).map(|i| seed(i + 9000)).collect();

    // E11: Entity 384D
    let e11_entity: Vec<f32> = (0..E11_DIM).map(|i| seed(i + 10000)).collect();

    // E12: Late interaction (ColBERT-style) - multiple token vectors 128D each
    let num_tokens = content.split_whitespace().count().min(64);
    let e12_late_interaction: Vec<Vec<f32>> = (0..num_tokens)
        .map(|t| (0..E12_TOKEN_DIM).map(|i| seed(i + t * 128 + 11000)).collect())
        .collect();

    // E13: SPLADE sparse (u16 indices)
    let mut e13_indices: Vec<u16> = (0..30)
        .map(|i| ((hash[(i + 10) % 32] as u32 * 200) % 30000) as u16)
        .collect();
    // Sort indices for sparse vector validity
    e13_indices.sort();
    e13_indices.dedup();
    let e13_values: Vec<f32> = (0..e13_indices.len())
        .map(|i| seed(i + 12000).abs() * 3.0)
        .collect();

    // Sort e6 indices too
    let mut e6_sorted: Vec<(u16, f32)> = e6_sparse_indices
        .into_iter()
        .zip(e6_sparse_values)
        .collect();
    e6_sorted.sort_by_key(|(idx, _)| *idx);
    e6_sorted.dedup_by_key(|(idx, _)| *idx);
    let (e6_indices, e6_values): (Vec<u16>, Vec<f32>) = e6_sorted.into_iter().unzip();

    SemanticFingerprint {
        e1_semantic,
        e2_temporal_recent,
        e3_temporal_periodic,
        e4_temporal_positional,
        e5_causal,
        e6_sparse: SparseVector {
            indices: e6_indices,
            values: e6_values,
        },
        e7_code,
        e8_graph,
        e9_hdc,
        e10_multimodal,
        e11_entity,
        e12_late_interaction,
        e13_splade: SparseVector {
            indices: e13_indices,
            values: e13_values,
        },
    }
}

/// Create synthetic purpose vector with topic-based alignment.
fn create_synthetic_purpose_vector(topic: &str) -> PurposeVector {
    // Different topics have different alignment patterns
    let alignment_seed = match topic {
        "machine_learning_fundamentals" => 0.85,
        "distributed_systems" => 0.75,
        "transformer_nlp" => 0.90,
        "database_indexing" => 0.70,
        "api_design" => 0.65,
        "rust_memory_safety" => 0.80,
        "vector_embeddings" => 0.95,
        "consciousness_models" => 0.88,
        "knowledge_graphs" => 0.82,
        "teleological_systems" => 0.92,
        _ => 0.5,
    };

    // Create varied alignments per embedder (13 embedders)
    let alignments: [f32; NUM_EMBEDDERS] = [
        alignment_seed * 0.9,         // E1 semantic
        alignment_seed * 0.7,         // E2 temporal recent
        alignment_seed * 0.65,        // E3 temporal periodic
        alignment_seed * 0.6,         // E4 temporal positional
        alignment_seed * 0.75,        // E5 causal
        alignment_seed * 0.5,         // E6 sparse
        alignment_seed * 0.8,         // E7 code
        alignment_seed * 0.7,         // E8 graph
        alignment_seed * 0.4,         // E9 hdc
        alignment_seed * 0.55,        // E10 multimodal
        alignment_seed * 0.6,         // E11 entity
        alignment_seed * 0.85,        // E12 late interaction
        alignment_seed * 0.5,         // E13 splade
    ];

    // Use the constructor which computes dominant_embedder and coherence
    PurposeVector::new(alignments)
}

/// Inject synthetic data into the PRODUCTION database.
///
/// This test is ignored by default because:
/// 1. It writes to production data
/// 2. Requires exclusive database access (can't run concurrently)
/// 3. Is meant to be run manually when needed
///
/// Run with: cargo test -p context-graph-mcp inject_synthetic_production -- --ignored --nocapture
#[tokio::test]
#[ignore = "Production database injection - run manually with --ignored"]
async fn inject_synthetic_production() {
    println!("\n================================================================================");
    println!("PRODUCTION DATABASE INJECTION: Injecting Synthetic Data");
    println!("================================================================================");
    println!("Database path: {}", PRODUCTION_DB_PATH);

    let db_path = PathBuf::from(PRODUCTION_DB_PATH);

    // Open the production database
    println!("\n[OPENING] Production RocksDB database...");
    let store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open production RocksDB database");
    let store: Arc<dyn TeleologicalMemoryStore> = Arc::new(store);

    // Get count BEFORE injection
    let count_before = store.count().await.expect("Failed to count before injection");
    println!("[BEFORE] Fingerprint count: {}", count_before);

    // Inject all synthetic memories
    println!("\n[INJECTING] {} synthetic memories:", SYNTHETIC_MEMORIES.len());
    let mut stored_ids: Vec<uuid::Uuid> = Vec::new();

    for (i, (topic, content)) in SYNTHETIC_MEMORIES.iter().enumerate() {
        let fingerprint = create_synthetic_fingerprint(content, topic);
        let id = fingerprint.id;

        match store.store(fingerprint).await {
            Ok(stored_id) => {
                stored_ids.push(stored_id);
                println!(
                    "  [{:2}] {} - {} ({} chars)",
                    i + 1,
                    stored_id,
                    topic,
                    content.len()
                );
            }
            Err(e) => {
                eprintln!("  [{:2}] FAILED: {} - {}", i + 1, topic, e);
            }
        }
    }

    // Get count AFTER injection
    let count_after = store.count().await.expect("Failed to count after injection");
    println!("\n[AFTER] Fingerprint count: {}", count_after);
    println!("  Added: {} fingerprints", count_after - count_before);

    // Verify each stored fingerprint is retrievable
    println!("\n[VERIFICATION] Checking stored fingerprints:");
    let mut verified_count = 0;
    for (i, id) in stored_ids.iter().enumerate() {
        match store.retrieve(*id).await {
            Ok(Some(fp)) => {
                verified_count += 1;
                println!(
                    "  [{:2}] {} - theta={:.4}, coherence={:.4}, dominant_embedder={}",
                    i + 1,
                    id,
                    fp.theta_to_north_star,
                    fp.purpose_vector.coherence,
                    fp.purpose_vector.dominant_embedder
                );
            }
            Ok(None) => {
                eprintln!("  [{:2}] {} - NOT FOUND!", i + 1, id);
            }
            Err(e) => {
                eprintln!("  [{:2}] {} - ERROR: {}", i + 1, id, e);
            }
        }
    }

    // Check quadrant distribution
    let quadrant_counts = store
        .count_by_quadrant()
        .await
        .expect("Failed to count by quadrant");
    println!("\n[QUADRANT DISTRIBUTION]");
    println!("  Open:    {}", quadrant_counts[0]);
    println!("  Blind:   {}", quadrant_counts[1]);
    println!("  Hidden:  {}", quadrant_counts[2]);
    println!("  Unknown: {}", quadrant_counts[3]);

    // Summary
    println!("\n================================================================================");
    println!("INJECTION COMPLETE");
    println!("================================================================================");
    println!("  Initial count: {}", count_before);
    println!("  Final count:   {}", count_after);
    println!("  Injected:      {}", stored_ids.len());
    println!("  Verified:      {}", verified_count);
    println!("================================================================================\n");

    assert_eq!(
        count_after - count_before,
        SYNTHETIC_MEMORIES.len(),
        "Should have added exactly {} fingerprints",
        SYNTHETIC_MEMORIES.len()
    );
    assert_eq!(
        verified_count,
        stored_ids.len(),
        "All stored fingerprints should be verifiable"
    );
}
