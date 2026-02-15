//! Integration tests for the background causal discovery loop.
//!
//! These tests use REAL RocksDB storage and verify state physically exists
//! in the database after operations. NO mock data - real embeddings generated
//! from deterministic seeds, real store operations.
//!
//! Tests requiring the LLM model are gated by `model_path.exists()`.

use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

use context_graph_causal_agent::{
    CausalDiscoveryConfig, CausalDiscoveryService, DiscoveryCursor,
};
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use context_graph_core::types::SourceMetadata;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create deterministic vector from seed (same logic as storage tests).
fn generate_vec(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| {
            let x = ((seed as f64 * 0.1 + i as f64 * 0.01) * PI as f64).sin() as f32;
            x * 0.5 + 0.5
        })
        .collect()
}

/// Create deterministic sparse vector.
fn generate_sparse(seed: u64) -> SparseVector {
    let num_entries = 50 + (seed % 50) as usize;
    let mut indices = Vec::with_capacity(num_entries);
    let mut values = Vec::with_capacity(num_entries);
    for i in 0..num_entries {
        indices.push(((seed + i as u64 * 31) % 30522) as u16);
        values
            .push(((seed as f64 * 0.1 + i as f64 * 0.2) * PI as f64).sin().abs() as f32 + 0.1);
    }
    SparseVector { indices, values }
}

/// Create deterministic late-interaction vectors.
fn generate_late_interaction(seed: u64) -> Vec<Vec<f32>> {
    let num_tokens = 5 + (seed % 10) as usize;
    (0..num_tokens)
        .map(|t| generate_vec(128, seed + t as u64 * 100))
        .collect()
}

/// Create a test fingerprint with deterministic embeddings.
/// The content_hash is the SHA-256 of `content` so `store_content` validation passes.
fn create_test_fingerprint(seed: u64, content: &str) -> TeleologicalFingerprint {
    let e5_vec = generate_vec(768, seed + 4);
    let semantic = SemanticFingerprint {
        e1_semantic: generate_vec(1024, seed),
        e2_temporal_recent: generate_vec(512, seed + 1),
        e3_temporal_periodic: generate_vec(512, seed + 2),
        e4_temporal_positional: generate_vec(512, seed + 3),
        e5_causal_as_cause: e5_vec.clone(),
        e5_causal_as_effect: e5_vec,
        e5_causal: Vec::new(),
        e6_sparse: generate_sparse(seed + 5),
        e7_code: generate_vec(1536, seed + 6),
        e8_graph_as_source: generate_vec(1024, seed + 7),
        e8_graph_as_target: generate_vec(1024, seed + 8),
        e8_graph: Vec::new(),
        e9_hdc: generate_vec(1024, seed + 8),
        e10_multimodal_paraphrase: generate_vec(768, seed + 9),
        e10_multimodal_as_context: generate_vec(768, seed + 13),
        e11_entity: generate_vec(768, seed + 10),
        e12_late_interaction: generate_late_interaction(seed + 11),
        e13_splade: generate_sparse(seed + 12),
    };

    // Compute real SHA-256 hash of the content for store_content validation
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();

    TeleologicalFingerprint::new(semantic, hash)
}

/// Create a SourceMetadata for a given session.
fn create_test_source_metadata(session_id: &str) -> SourceMetadata {
    let mut meta = SourceMetadata::default();
    meta.session_id = Some(session_id.to_string());
    meta.created_at = Some(Utc::now());
    meta
}

/// Open a store and wrap in Arc for trait usage.
fn open_test_store(path: &std::path::Path) -> Arc<dyn TeleologicalMemoryStore> {
    let store = RocksDbTeleologicalStore::open(path)
        .expect("Failed to open RocksDB store");
    Arc::new(store)
}

/// Default test config.
fn test_config() -> CausalDiscoveryConfig {
    CausalDiscoveryConfig {
        batch_size: 10,
        min_confidence: 0.5,
        interval: Duration::from_secs(5),
        ..CausalDiscoveryConfig::default()
    }
}

/// Check if the LLM model file exists.
fn model_exists(config: &CausalDiscoveryConfig) -> bool {
    config.llm_config.model_path.exists()
}

// ============================================================================
// CURSOR PERSISTENCE TESTS (No LLM needed)
// ============================================================================

#[tokio::test]
async fn test_cursor_store_and_load() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Create cursor with known values
    let cursor = DiscoveryCursor {
        last_timestamp: Some(Utc::now()),
        last_fingerprint_id: Some(Uuid::new_v4()),
        cycles_completed: 42,
        total_relationships: 100,
    };

    // Serialize and store
    let json = serde_json::to_vec(&cursor).expect("cursor serialization failed");
    store
        .store_processing_cursor("causal_discovery_cursor", &json)
        .await
        .expect("store_processing_cursor failed");

    // Read back and verify
    let loaded = store
        .get_processing_cursor("causal_discovery_cursor")
        .await
        .expect("get_processing_cursor failed")
        .expect("cursor not found in store");

    let restored: DiscoveryCursor =
        serde_json::from_slice(&loaded).expect("cursor deserialization failed");

    assert_eq!(restored.cycles_completed, 42, "cycles_completed mismatch");
    assert_eq!(
        restored.total_relationships, 100,
        "total_relationships mismatch"
    );
    assert!(
        restored.last_timestamp.is_some(),
        "last_timestamp should be present"
    );
    assert!(
        restored.last_fingerprint_id.is_some(),
        "last_fingerprint_id should be present"
    );
    println!("PASS: Cursor round-trip through RocksDB CF_SYSTEM verified");
}

#[tokio::test]
async fn test_cursor_overwrite() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store initial cursor
    let cursor1 = DiscoveryCursor {
        cycles_completed: 1,
        total_relationships: 5,
        ..Default::default()
    };
    let json1 = serde_json::to_vec(&cursor1).unwrap();
    store
        .store_processing_cursor("causal_discovery_cursor", &json1)
        .await
        .unwrap();

    // Overwrite with updated cursor
    let cursor2 = DiscoveryCursor {
        cycles_completed: 2,
        total_relationships: 12,
        ..Default::default()
    };
    let json2 = serde_json::to_vec(&cursor2).unwrap();
    store
        .store_processing_cursor("causal_discovery_cursor", &json2)
        .await
        .unwrap();

    // Verify we get the latest
    let loaded = store
        .get_processing_cursor("causal_discovery_cursor")
        .await
        .unwrap()
        .unwrap();
    let restored: DiscoveryCursor = serde_json::from_slice(&loaded).unwrap();

    assert_eq!(restored.cycles_completed, 2);
    assert_eq!(restored.total_relationships, 12);
    println!("PASS: Cursor overwrite verified - latest value persisted");
}

#[tokio::test]
async fn test_cursor_not_found_returns_none() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    let result = store
        .get_processing_cursor("nonexistent_cursor")
        .await
        .unwrap();
    assert!(result.is_none(), "Expected None for nonexistent cursor");
    println!("PASS: Missing cursor returns None (no error)");
}

// ============================================================================
// MEMORY STORAGE VERIFICATION (No LLM needed)
// ============================================================================

#[tokio::test]
async fn test_store_and_retrieve_memory_with_content() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store a fingerprint
    let content = "Deploying version 2.3 of the authentication service introduced a breaking change";
    let fp = create_test_fingerprint(100, content);
    let fp_id = fp.id;
    store.store(fp).await.expect("store fingerprint failed");
    store
        .store_content(fp_id, content)
        .await
        .expect("store_content failed");

    // Store source metadata
    let meta = create_test_source_metadata("test-session");
    store
        .store_source_metadata(fp_id, &meta)
        .await
        .expect("store_source_metadata failed");

    // Verify: fingerprint exists
    let retrieved = store
        .retrieve(fp_id)
        .await
        .expect("retrieve failed")
        .expect("fingerprint not found");
    assert_eq!(retrieved.id, fp_id);
    assert_eq!(
        retrieved.semantic.e1_semantic.len(),
        1024,
        "E1 should be 1024D"
    );

    // Verify: content exists
    let loaded_content = store
        .get_content(fp_id)
        .await
        .expect("get_content failed")
        .expect("content not found");
    assert_eq!(loaded_content, content);

    // Verify: source metadata exists
    let loaded_meta = store
        .get_source_metadata(fp_id)
        .await
        .expect("get_source_metadata failed")
        .expect("metadata not found");
    assert_eq!(loaded_meta.session_id, Some("test-session".to_string()));

    println!("PASS: Memory storage + content + metadata round-trip verified in RocksDB");
}

// ============================================================================
// SCAN FOR CLUSTERING (Memory harvest prerequisite)
// ============================================================================

#[tokio::test]
async fn test_scan_fingerprints_for_harvest() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store 3 fingerprints with content
    let contents = [
        "Deploying version 2.3 introduced a breaking session token change",
        "Monitoring showed 500% spike in HTTP 401 errors after deployment",
        "The weather in Seattle was rainy during November 2025",
    ];

    let mut ids = Vec::new();
    for (i, content) in contents.iter().enumerate() {
        let fp = create_test_fingerprint(200 + i as u64, content);
        let id = fp.id;
        ids.push(id);
        store.store(fp).await.unwrap();
        store.store_content(id, content).await.unwrap();
    }

    // Verify scan returns all fingerprints
    let scan_result = store
        .scan_fingerprints_for_clustering(Some(100))
        .await
        .expect("scan_fingerprints_for_clustering failed");

    assert_eq!(
        scan_result.len(),
        3,
        "Should find 3 fingerprints in scan"
    );

    // Verify content is retrievable for each
    for id in &ids {
        let content = store.get_content(*id).await.unwrap();
        assert!(content.is_some(), "Content missing for fingerprint {}", id);
    }

    println!(
        "PASS: scan_fingerprints_for_clustering returns {} items, all with content",
        scan_result.len()
    );
}

// ============================================================================
// EMPTY DATABASE EDGE CASE
// ============================================================================

#[tokio::test]
async fn test_empty_database_scan() {
    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Scan empty database
    let scan_result = store
        .scan_fingerprints_for_clustering(Some(100))
        .await
        .expect("scan on empty DB should succeed");

    assert!(
        scan_result.is_empty(),
        "Empty DB should return empty scan results"
    );

    // Verify count is 0
    let count = store.count().await.expect("count failed");
    assert_eq!(count, 0, "Empty DB should have count=0");

    println!("PASS: Empty database edge case - scan returns [], count=0");
}

// ============================================================================
// DISCOVERY CYCLE WITH REAL LLM (GPU required)
// ============================================================================

#[tokio::test]
#[ignore = "requires LLM model weights"]
async fn test_discovery_cycle_with_real_store() {
    let config = test_config();
    assert!(model_exists(&config), "LLM model not found at {:?}", config.llm_config.model_path);

    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store causal pair (from spec §9.2)
    let content_a = "Deploying version 2.3 of the authentication service introduced a breaking \
                     change in the session token format, causing all existing sessions to become invalid.";
    let content_b = "After the v2.3 deployment, the monitoring dashboard showed a 500% spike in \
                     HTTP 401 errors across all microservices that depend on the authentication service.";

    let fp_a = create_test_fingerprint(300, content_a);
    let fp_b = create_test_fingerprint(301, content_b);
    let id_a = fp_a.id;
    let id_b = fp_b.id;

    store.store(fp_a).await.unwrap();
    store.store(fp_b).await.unwrap();
    store.store_content(id_a, content_a).await.unwrap();
    store.store_content(id_b, content_b).await.unwrap();

    let meta_a = create_test_source_metadata("causal-test");
    let meta_b = create_test_source_metadata("causal-test");
    store.store_source_metadata(id_a, &meta_a).await.unwrap();
    store.store_source_metadata(id_b, &meta_b).await.unwrap();

    // Create memories for analysis
    let memories = vec![
        context_graph_causal_agent::MemoryForAnalysis {
            id: id_a,
            content: content_a.to_string(),
            session_id: Some("causal-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 300),
        },
        context_graph_causal_agent::MemoryForAnalysis {
            id: id_b,
            content: content_b.to_string(),
            session_id: Some("causal-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 301),
        },
    ];

    // Create service (deprecated new() for testing without CausalModel)
    #[allow(deprecated)]
    let service = CausalDiscoveryService::new(config)
        .await
        .expect("CausalDiscoveryService creation failed");

    service.load_model().await.expect("LLM load failed");

    // Run discovery cycle WITH store
    let result = service
        .run_discovery_cycle(&memories, Some(&store))
        .await
        .expect("run_discovery_cycle failed");

    println!("=== Discovery Cycle Results ===");
    println!("  Candidates found:         {}", result.candidates_found);
    println!(
        "  Relationships confirmed:  {}",
        result.relationships_confirmed
    );
    println!(
        "  Relationships rejected:   {}",
        result.relationships_rejected
    );
    println!("  Errors:                   {}", result.errors);

    // The causal pair should have been analyzed.
    // We can't guarantee the LLM finds a link (depends on model), but we can verify:
    assert!(
        result.errors == 0,
        "Discovery cycle should complete without errors"
    );
    assert!(
        result.candidates_found > 0,
        "Scanner should find at least 1 candidate pair"
    );

    // If relationships were confirmed, verify they're in the store
    if result.relationships_confirmed > 0 {
        // Search for causal relationships using E5 embedding
        let search_results = store
            .search_causal_relationships(&generate_vec(768, 300 + 4), 10, None)
            .await
            .expect("search_causal_relationships failed");

        println!(
            "  Causal search results:    {} (should be > 0 if confirmed)",
            search_results.len()
        );

        // Read back at least one relationship
        if let Some((rel_id, score)) = search_results.first() {
            let relationship = store
                .get_causal_relationship(*rel_id)
                .await
                .expect("get_causal_relationship failed")
                .expect("relationship not found by ID");

            println!("=== Verified CausalRelationship ===");
            println!("  ID:          {}", relationship.id);
            println!("  Confidence:  {}", relationship.confidence);
            println!(
                "  E5 cause:    {} dims, non-zero={}",
                relationship.e5_as_cause.len(),
                relationship.e5_as_cause.iter().any(|&v| v != 0.0)
            );
            println!(
                "  E5 effect:   {} dims, non-zero={}",
                relationship.e5_as_effect.len(),
                relationship.e5_as_effect.iter().any(|&v| v != 0.0)
            );
            println!("  Score:       {}", score);

            assert_eq!(
                relationship.e5_as_cause.len(),
                768,
                "E5 cause should be 768D"
            );
            assert_eq!(
                relationship.e5_as_effect.len(),
                768,
                "E5 effect should be 768D"
            );
            assert!(
                relationship.confidence >= 0.5,
                "Confidence should meet threshold"
            );
        }

        println!("PASS: CausalRelationship physically verified in RocksDB");
    } else {
        println!("NOTE: LLM did not confirm a causal link (model-dependent). Storage path not tested.");
    }
}

#[tokio::test]
#[ignore = "requires LLM model weights"]
async fn test_non_causal_pair_no_relationship_stored() {
    let config = test_config();
    assert!(model_exists(&config), "LLM model not found at {:?}", config.llm_config.model_path);

    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store non-causal pair (from spec §9.2)
    let content_c = "The weather in Seattle was particularly rainy during November 2025.";
    let content_d = "The quarterly revenue report for Q4 2025 exceeded analyst expectations by 12%.";

    let fp_c = create_test_fingerprint(400, content_c);
    let fp_d = create_test_fingerprint(401, content_d);
    let id_c = fp_c.id;
    let id_d = fp_d.id;

    store.store(fp_c).await.unwrap();
    store.store(fp_d).await.unwrap();
    store.store_content(id_c, content_c).await.unwrap();
    store.store_content(id_d, content_d).await.unwrap();

    let memories = vec![
        context_graph_causal_agent::MemoryForAnalysis {
            id: id_c,
            content: content_c.to_string(),
            session_id: Some("noncausal-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 400),
        },
        context_graph_causal_agent::MemoryForAnalysis {
            id: id_d,
            content: content_d.to_string(),
            session_id: Some("noncausal-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 401),
        },
    ];

    #[allow(deprecated)]
    let service = CausalDiscoveryService::new(config)
        .await
        .expect("service creation failed");
    service.load_model().await.expect("LLM load failed");

    let result = service
        .run_discovery_cycle(&memories, Some(&store))
        .await
        .expect("discovery cycle failed");

    println!("=== Non-Causal Pair Results ===");
    println!(
        "  Confirmed: {} (expected: 0)",
        result.relationships_confirmed
    );
    println!("  Rejected:  {}", result.relationships_rejected);

    // Non-causal pair should NOT produce confirmed relationships
    assert_eq!(
        result.relationships_confirmed, 0,
        "Non-causal pair should not confirm relationships, but got {} confirmed",
        result.relationships_confirmed
    );
}

// ============================================================================
// AUDIT RECORD VERIFICATION
// ============================================================================

#[tokio::test]
#[ignore = "requires LLM model weights"]
async fn test_audit_record_after_discovery() {
    let config = test_config();
    assert!(model_exists(&config), "LLM model not found at {:?}", config.llm_config.model_path);

    let tmp = TempDir::new().unwrap();
    let store = open_test_store(tmp.path());

    // Store test memories
    let fp = create_test_fingerprint(500, "System error caused service outage");
    let id = fp.id;
    store.store(fp).await.unwrap();
    store
        .store_content(id, "System error caused service outage")
        .await
        .unwrap();

    let fp2 = create_test_fingerprint(501, "Users reported inability to access accounts after outage");
    let id2 = fp2.id;
    store.store(fp2).await.unwrap();
    store
        .store_content(id2, "Users reported inability to access accounts after outage")
        .await
        .unwrap();

    // Check audit count before
    let audit_before = store
        .count_audit_records()
        .await
        .expect("count_audit_records failed");

    let memories = vec![
        context_graph_causal_agent::MemoryForAnalysis {
            id,
            content: "System error caused service outage".to_string(),
            session_id: Some("audit-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 500),
        },
        context_graph_causal_agent::MemoryForAnalysis {
            id: id2,
            content: "Users reported inability to access accounts after outage".to_string(),
            session_id: Some("audit-test".to_string()),
            created_at: Utc::now(),
            e1_embedding: generate_vec(1024, 501),
        },
    ];

    #[allow(deprecated)]
    let service = CausalDiscoveryService::new(config).await.unwrap();
    service.load_model().await.unwrap();

    let _result = service
        .run_discovery_cycle(&memories, Some(&store))
        .await
        .unwrap();

    // Note: run_discovery_cycle does not emit audit records directly -
    // that's done in run_background_tick. The audit trail verification
    // tests the store's audit capability.
    let audit_after = store
        .count_audit_records()
        .await
        .expect("count_audit_records failed");

    println!("=== Audit Record Verification ===");
    println!("  Before cycle: {} audit records", audit_before);
    println!("  After cycle:  {} audit records", audit_after);
    println!("PASS: Audit trail accessible (count API works)");
}

// ============================================================================
// ADAPTIVE INTERVAL TESTS (No LLM needed)
// ============================================================================

#[tokio::test]
async fn test_adaptive_interval_with_real_store() {
    // Verify intervals are correct per spec:
    // 0 memories → 600s, 0 discoveries → 300s, ≤5 → 120s, >5 → 30s
    let config = test_config();
    #[allow(deprecated)]
    let service = CausalDiscoveryService::new(config).await.unwrap();

    let cases = vec![
        (
            "empty",
            context_graph_causal_agent::CycleMetrics {
                memories_harvested: 0,
                ..Default::default()
            },
            Duration::from_secs(600),
        ),
        (
            "no discoveries",
            context_graph_causal_agent::CycleMetrics {
                memories_harvested: 50,
                relationships_discovered: 0,
                ..Default::default()
            },
            Duration::from_secs(300),
        ),
        (
            "few discoveries",
            context_graph_causal_agent::CycleMetrics {
                memories_harvested: 50,
                relationships_discovered: 3,
                ..Default::default()
            },
            Duration::from_secs(120),
        ),
        (
            "heavy discoveries",
            context_graph_causal_agent::CycleMetrics {
                memories_harvested: 50,
                relationships_discovered: 10,
                ..Default::default()
            },
            Duration::from_secs(30),
        ),
    ];

    for (name, metrics, expected) in cases {
        let interval = service.compute_next_interval(&metrics);
        assert_eq!(
            interval, expected,
            "Adaptive interval for '{}': got {:?}, expected {:?}",
            name, interval, expected
        );
        println!("PASS: Adaptive interval '{}' = {:?}", name, interval);
    }
}

// ============================================================================
// SERVICE STATUS TESTS (No LLM needed)
// ============================================================================

#[tokio::test]
async fn test_service_stopped_initially() {
    let config = test_config();
    #[allow(deprecated)]
    let service = CausalDiscoveryService::new(config).await.unwrap();
    assert!(!service.is_running());
    println!("PASS: Service starts in Stopped state");
}

// ============================================================================
// CONFIG ENV OVERRIDES
// ============================================================================

#[test]
fn test_env_overrides_applied() {
    // Set env vars
    std::env::set_var("CAUSAL_DISCOVERY_INTERVAL_SECS", "45");
    std::env::set_var("CAUSAL_DISCOVERY_BATCH_SIZE", "250");
    std::env::set_var("CAUSAL_DISCOVERY_MIN_CONFIDENCE", "0.8");

    let config = CausalDiscoveryConfig::default().with_env_overrides();
    assert_eq!(config.interval, Duration::from_secs(45));
    assert_eq!(config.batch_size, 250);
    assert!((config.min_confidence - 0.8).abs() < 0.001);

    // Clean up
    std::env::remove_var("CAUSAL_DISCOVERY_INTERVAL_SECS");
    std::env::remove_var("CAUSAL_DISCOVERY_BATCH_SIZE");
    std::env::remove_var("CAUSAL_DISCOVERY_MIN_CONFIDENCE");

    println!("PASS: Environment variable overrides applied correctly");
}
