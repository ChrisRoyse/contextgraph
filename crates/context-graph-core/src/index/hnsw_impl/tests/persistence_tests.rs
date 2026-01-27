//! Persistence and legacy format tests.

use crate::index::hnsw_impl::{HnswMultiSpaceIndex, RealHnswIndex};
use crate::index::manager::MultiSpaceIndexManager;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};
use uuid::Uuid;

use super::real_hnsw_tests::random_vector;

/// Helper to create a minimal valid SemanticFingerprint.
fn create_test_fingerprint() -> SemanticFingerprint {
    let e5_vec = random_vector(768);
    SemanticFingerprint {
        e1_semantic: random_vector(1024),
        e2_temporal_recent: random_vector(512),
        e3_temporal_periodic: random_vector(512),
        e4_temporal_positional: random_vector(512),
        e5_causal_as_cause: e5_vec.clone(),
        e5_causal_as_effect: e5_vec,
        e5_causal: Vec::new(), // Using new dual format
        e6_sparse: SparseVector::new(vec![100, 200], vec![0.5, 0.3]).unwrap(),
        e7_code: random_vector(1536),
        e8_graph_as_source: random_vector(1024),
        e8_graph_as_target: random_vector(1024),
        e8_graph: Vec::new(), // Legacy field, empty by default
        e9_hdc: random_vector(1024),
        e10_multimodal_as_intent: random_vector(768),
        e10_multimodal_as_context: random_vector(768),
        e10_multimodal: Vec::new(), // Legacy field, empty by default
        e11_entity: random_vector(768), // KEPLER
        e12_late_interaction: vec![random_vector(128); 3],
        e13_splade: SparseVector::new(vec![100, 200, 300], vec![0.5, 0.3, 0.2]).unwrap(),
    }
}

#[tokio::test]
async fn test_multi_space_persist_and_load() {
    let mut manager = HnswMultiSpaceIndex::new();
    manager.initialize().await.unwrap();

    let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for id in &ids {
        let fp = create_test_fingerprint();
        manager.add_fingerprint(*id, &fp).await.unwrap();
    }

    let before_count: usize = manager.status().iter().map(|s| s.element_count).sum();
    println!("[BEFORE PERSIST] Total elements = {}", before_count);

    let temp_dir = std::env::temp_dir().join(format!("hnsw_test_{}", Uuid::new_v4()));
    manager.persist(&temp_dir).await.unwrap();

    assert!(temp_dir.join("index_meta.json").exists());
    assert!(temp_dir.join("splade.bin").exists());
    println!("[PERSIST] Files created at {:?}", temp_dir);

    let mut loaded_manager = HnswMultiSpaceIndex::new();
    loaded_manager.load(&temp_dir).await.unwrap();

    let after_count: usize = loaded_manager
        .status()
        .iter()
        .map(|s| s.element_count)
        .sum();
    println!("[AFTER LOAD] Total elements = {}", after_count);

    assert_eq!(before_count, after_count);

    std::fs::remove_dir_all(&temp_dir).ok();

    println!("[VERIFIED] persist/load round-trip preserves data");
}

#[test]
fn test_load_rejects_legacy_simple_hnsw_format() {
    use std::io::Write;

    let temp_path =
        std::env::temp_dir().join(format!("legacy_simple_hnsw_test_{}.bin", Uuid::new_v4()));

    {
        let mut file = std::fs::File::create(&temp_path).unwrap();
        file.write_all(b"SIMPLE_HNSW_LEGACY_DATA_HERE_INVALID_FORMAT")
            .unwrap();
        file.flush().unwrap();
    }

    println!("[BEFORE] Attempting to load legacy SIMPLE_HNSW format");
    let result = RealHnswIndex::load(&temp_path);
    println!("[AFTER] result.is_err() = {}", result.is_err());

    std::fs::remove_file(&temp_path).ok();

    assert!(result.is_err(), "Should reject legacy format");

    let err = result.unwrap_err();
    let err_str = err.to_string();
    println!("[ERROR] {}", err_str);

    assert!(
        err_str.contains("LEGACY FORMAT REJECTED") || err_str.contains("legacy"),
        "Error should mention legacy format: {}",
        err_str
    );

    println!("[VERIFIED] Legacy SIMPLE_HNSW format rejected");
}

#[test]
fn test_load_rejects_legacy_simp_idx_format() {
    use std::io::Write;

    let temp_path =
        std::env::temp_dir().join(format!("legacy_simp_idx_test_{}.bin", Uuid::new_v4()));

    {
        let mut file = std::fs::File::create(&temp_path).unwrap();
        file.write_all(b"SIMP_IDX_LEGACY_DATA_HERE_INVALID_FORMAT")
            .unwrap();
        file.flush().unwrap();
    }

    println!("[BEFORE] Attempting to load legacy SIMP_IDX format");
    let result = RealHnswIndex::load(&temp_path);
    println!("[AFTER] result.is_err() = {}", result.is_err());

    std::fs::remove_file(&temp_path).ok();

    assert!(result.is_err(), "Should reject legacy format");

    let err = result.unwrap_err();
    let err_str = err.to_string();
    println!("[ERROR] {}", err_str);

    assert!(
        err_str.contains("LEGACY FORMAT REJECTED") || err_str.contains("legacy"),
        "Error should mention legacy format: {}",
        err_str
    );

    println!("[VERIFIED] Legacy SIMP_IDX format rejected");
}

#[test]
fn test_load_rejects_legacy_null_simple_format() {
    use std::io::Write;

    let temp_path =
        std::env::temp_dir().join(format!("legacy_null_simple_test_{}.bin", Uuid::new_v4()));

    {
        let mut file = std::fs::File::create(&temp_path).unwrap();
        file.write_all(b"\x00SIMPLE_LEGACY_DATA_HERE_INVALID_FORMAT")
            .unwrap();
        file.flush().unwrap();
    }

    println!("[BEFORE] Attempting to load legacy null-prefixed SIMPLE format");
    let result = RealHnswIndex::load(&temp_path);
    println!("[AFTER] result.is_err() = {}", result.is_err());

    std::fs::remove_file(&temp_path).ok();

    assert!(result.is_err(), "Should reject legacy format");

    let err = result.unwrap_err();
    let err_str = err.to_string();
    println!("[ERROR] {}", err_str);

    assert!(
        err_str.contains("LEGACY FORMAT REJECTED") || err_str.contains("legacy"),
        "Error should mention legacy format: {}",
        err_str
    );

    println!("[VERIFIED] Legacy null-prefixed SIMPLE format rejected");
}

#[tokio::test]
async fn test_multi_space_load_rejects_legacy_files() {
    use std::io::Write;

    let temp_dir = std::env::temp_dir().join(format!("legacy_multispace_test_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let meta_path = temp_dir.join("index_meta.json");
    let meta_content = serde_json::json!({
        "version": "3.0.0",
        "hnsw_count": 0,
        "splade_count": 0,
        "initialized": true,
        "index_type": "RealHnswIndex"
    });
    std::fs::write(&meta_path, meta_content.to_string()).unwrap();

    let legacy_path = temp_dir.join("E1Semantic.hnsw.bin");
    {
        let mut legacy_file = std::fs::File::create(&legacy_path).unwrap();
        legacy_file.write_all(b"LEGACY_SIMPLE_HNSW_DATA").unwrap();
    }

    println!("[BEFORE] Attempting to load directory with legacy .hnsw.bin file");
    let mut manager = HnswMultiSpaceIndex::new();
    let result = manager.load(&temp_dir).await;
    println!("[AFTER] result.is_err() = {}", result.is_err());

    std::fs::remove_dir_all(&temp_dir).ok();

    assert!(
        result.is_err(),
        "Should reject directory containing legacy files"
    );

    let err = result.unwrap_err();
    let err_str = err.to_string();
    println!("[ERROR] {}", err_str);

    assert!(
        err_str.contains("LEGACY FORMAT REJECTED") || err_str.contains("legacy"),
        "Error should mention legacy format: {}",
        err_str
    );

    println!("[VERIFIED] Multi-space load rejects legacy files");
}
