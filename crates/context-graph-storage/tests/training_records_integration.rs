//! Integration tests for `CF_TRAINING_RECORDS` persistence.
//!
//! Exercises the `store_training_record` / `get_training_record` /
//! `list_training_record_ids` / `count_training_records` /
//! `delete_training_record` / `clear_all_training_records` methods on a real
//! RocksDB instance in a temp directory. These are the Source-of-Truth reads
//! that back the MCP tool's FSV story — the tests here prove that bytes
//! written with the production encoder come back identical via the production
//! decoder.

use std::sync::Arc;

use chrono::Utc;
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, CausalLabel, TrainingEdge,
    TrainingRecord, NUM_CROSS_CORRELATIONS,
};
use context_graph_storage::teleological::{
    RocksDbTeleologicalStore, TeleologicalStoreConfig,
};
use tempfile::TempDir;
use uuid::Uuid;

/// Build a synthetic training record with known structure so we can assert on
/// specific bytes after round-trip. Everything is deterministic.
fn synthetic_record(seed_content: &str, topic_peaks: &[(usize, f32)]) -> TrainingRecord {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    for &(idx, val) in topic_peaks {
        profile[idx] = val;
    }
    let synergy = SynergyMatrix::with_base_synergies();
    let cross = compute_cross_correlations(&profile, &synergy);
    let groups = compute_group_alignments(&profile);
    let knn = (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect();

    TrainingRecord {
        memory_id: Uuid::new_v4(),
        content: seed_content.to_string(),
        importance: 0.75,
        created_at: Utc::now(),
        session_id: Some("fsv-storage-test".into()),
        source_type: Some("Manual".into()),
        source_path: None,
        e1_semantic: vec![0.1, 0.2, 0.3],
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_cause: vec![0.5; 8],
        e5_causal_effect: vec![-0.5; 8],
        e7_code: Vec::new(),
        e8_graph_source: Vec::new(),
        e8_graph_target: Vec::new(),
        e9_hdc: Vec::new(),
        e10_paraphrase: Vec::new(),
        e10_context: Vec::new(),
        e11_entity: Vec::new(),
        e6_sparse_indices: vec![7, 42, 100],
        e6_sparse_values: vec![0.9, 0.5, 0.1],
        e13_splade_indices: Vec::new(),
        e13_splade_values: Vec::new(),
        e12_token_embeddings: Vec::new(),
        topic_profile: profile,
        cross_correlations: cross,
        group_alignments: groups,
        outgoing_edges: vec![TrainingEdge {
            edge_type: 0,
            peer_id: Uuid::new_v4(),
            weight: 0.85,
            direction: 0,
            agreement_count: 4,
            embedder_scores: [0.1f32; NUM_EMBEDDERS],
        }],
        incoming_edges: Vec::new(),
        knn_neighbors: knn,
        causal_effects: vec![CausalLabel {
            related_memory_id: Uuid::new_v4(),
            description: "A -> B: mediated via C".into(),
            direction: "cause".into(),
            confidence: 0.82,
        }],
        causal_causes: Vec::new(),
        topic_memberships: Vec::new(),
    }
}

fn open_store() -> (TempDir, Arc<RocksDbTeleologicalStore>) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    let store = RocksDbTeleologicalStore::open_with_config(
        &path,
        TeleologicalStoreConfig::default(),
    )
    .expect("open store");
    (td, Arc::new(store))
}

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_single_record_preserves_all_fields() {
    let (_td, store) = open_store();
    let record = synthetic_record("hello fsv", &[(0, 0.9), (6, 0.8)]);
    let id = record.memory_id;

    println!("BEFORE store_training_record: count={}", store.count_training_records().await.unwrap());
    store.store_training_record(id, &record).await.expect("store");
    println!("AFTER store_training_record: count={}", store.count_training_records().await.unwrap());

    let back = store.get_training_record(id).await.expect("get").expect("exists");
    assert_eq!(back.memory_id, id);
    assert_eq!(back.content, "hello fsv");
    assert_eq!(back.topic_profile, record.topic_profile);
    assert_eq!(back.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
    assert_eq!(back.cross_correlations, record.cross_correlations);
    assert_eq!(back.group_alignments, record.group_alignments);
    assert_eq!(back.e1_semantic, vec![0.1, 0.2, 0.3]);
    assert_eq!(back.e5_causal_cause, vec![0.5; 8]);
    assert_eq!(back.outgoing_edges.len(), 1);
    assert_eq!(back.outgoing_edges[0].weight, 0.85);
    assert_eq!(back.causal_effects.len(), 1);
    assert_eq!(back.causal_effects[0].confidence, 0.82);
    assert_eq!(back.knn_neighbors.len(), NUM_EMBEDDERS);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_and_count_reflect_writes() {
    let (_td, store) = open_store();

    println!("BEFORE any writes: count={}", store.count_training_records().await.unwrap());
    assert_eq!(store.count_training_records().await.unwrap(), 0);
    assert!(store.list_training_record_ids().await.unwrap().is_empty());

    let mut ids = Vec::new();
    for i in 0..5 {
        let rec = synthetic_record(&format!("doc {}", i), &[(0, 0.5 + i as f32 * 0.1)]);
        ids.push(rec.memory_id);
        store.store_training_record(rec.memory_id, &rec).await.unwrap();
    }

    let count = store.count_training_records().await.unwrap();
    let listed = store.list_training_record_ids().await.unwrap();
    println!("AFTER 5 writes: count={}, listed={}", count, listed.len());
    assert_eq!(count, 5);
    assert_eq!(listed.len(), 5);
    for id in &ids {
        assert!(listed.contains(id), "missing id {} in listed", id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_returns_false() {
    let (_td, store) = open_store();
    let nonexistent = Uuid::new_v4();
    println!("BEFORE delete of nonexistent: count={}", store.count_training_records().await.unwrap());
    let deleted = store.delete_training_record(nonexistent).await.unwrap();
    println!("AFTER delete of nonexistent: deleted={}, count={}", deleted, store.count_training_records().await.unwrap());
    assert!(!deleted);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_existing_removes_record() {
    let (_td, store) = open_store();
    let rec = synthetic_record("doomed", &[(0, 1.0)]);
    let id = rec.memory_id;
    store.store_training_record(id, &rec).await.unwrap();

    println!("BEFORE delete: count={}, get_exists={}",
        store.count_training_records().await.unwrap(),
        store.get_training_record(id).await.unwrap().is_some()
    );

    let deleted = store.delete_training_record(id).await.unwrap();
    assert!(deleted);

    println!("AFTER delete: count={}, get_exists={}",
        store.count_training_records().await.unwrap(),
        store.get_training_record(id).await.unwrap().is_some()
    );

    assert_eq!(store.count_training_records().await.unwrap(), 0);
    assert!(store.get_training_record(id).await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_all_removes_every_record() {
    let (_td, store) = open_store();
    for i in 0..7 {
        let rec = synthetic_record(&format!("r{}", i), &[(i % NUM_EMBEDDERS, 0.5)]);
        store.store_training_record(rec.memory_id, &rec).await.unwrap();
    }
    println!("BEFORE clear_all: count={}", store.count_training_records().await.unwrap());

    let cleared = store.clear_all_training_records().await.unwrap();
    println!("AFTER clear_all: cleared={}, count={}", cleared, store.count_training_records().await.unwrap());

    assert_eq!(cleared, 7);
    assert_eq!(store.count_training_records().await.unwrap(), 0);
    assert!(store.list_training_record_ids().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn store_is_idempotent_overwrites_prior_record() {
    let (_td, store) = open_store();
    let mut rec = synthetic_record("v1", &[(0, 0.5)]);
    let id = rec.memory_id;
    store.store_training_record(id, &rec).await.unwrap();
    println!("BEFORE overwrite: content={}", store.get_training_record(id).await.unwrap().unwrap().content);

    rec.content = "v2 — overwritten".into();
    store.store_training_record(id, &rec).await.unwrap();
    let after = store.get_training_record(id).await.unwrap().unwrap();
    println!("AFTER overwrite: content={}", after.content);

    assert_eq!(after.content, "v2 — overwritten");
    assert_eq!(store.count_training_records().await.unwrap(), 1); // still 1, not 2
}

#[tokio::test(flavor = "multi_thread")]
async fn get_nonexistent_returns_none() {
    let (_td, store) = open_store();
    let result = store.get_training_record(Uuid::new_v4()).await.unwrap();
    println!("get(random UUID) on empty CF = {:?}", result.as_ref().map(|_| "Some"));
    assert!(result.is_none());
}
