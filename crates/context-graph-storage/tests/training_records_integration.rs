//! Integration tests for `CF_TRAINING_RECORDS` persistence.
//!
//! Exercises `store_training_record` / `get_training_record` /
//! `list_training_record_ids` / `count_training_records` /
//! `delete_training_record` / `clear_all_training_records` /
//! `multi_get_training_records` on a real RocksDB instance in a temp
//! directory. These are the Source-of-Truth reads that back the MCP tool's
//! FSV story — the tests here prove that bytes written with the production
//! encoder come back identical via the production decoder.
//!
//! All tests build real `TeleologicalFingerprint` / `TrainingRecord`
//! instances; no mocks, no type coercion.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::{TuckerCore, NUM_EMBEDDERS};
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, extract_temporal_labels, CausalLabel,
    PeriodicBucket, TrainingEdge, TrainingRecord, NUM_CROSS_CORRELATIONS,
};
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

/// Build a synthetic training record with known structure. Deterministic.
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
        content_hash: Some([0xAB; 32]),
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
        e14_bge_m3_dense: vec![0.14; 8],
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
            related_memory_id: Uuid::nil(),
            rel_id: Uuid::new_v4(),
            description: "A -> B. mediated via C".into(),
            direction: "cause".into(),
            confidence: 0.82,
            mechanism_type: Some("mediated".into()),
        }],
        causal_causes: Vec::new(),
        topic_memberships: Vec::new(),
        temporal_labels: None,
        tucker_core: None,
        edge_type_distribution: {
            // One SemanticSimilar outgoing edge seeded above (edge_type=0).
            let mut d = [0u32; 8];
            d[0] = 1;
            d
        },
    }
}

fn open_store() -> (TempDir, Arc<RocksDbTeleologicalStore>) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    let store =
        RocksDbTeleologicalStore::open_with_config(&path, TeleologicalStoreConfig::default())
            .expect("open store");
    (td, Arc::new(store))
}

fn empty_semantic() -> SemanticFingerprint {
    SemanticFingerprint {
        e1_semantic: Vec::new(),
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_as_cause: Vec::new(),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector::empty(),
        e7_code: Vec::new(),
        e8_graph_as_source: Vec::new(),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: Vec::new(),
        e10_multimodal_paraphrase: Vec::new(),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: Vec::new(),
        e12_late_interaction: Vec::new(),
        e13_splade: SparseVector::empty(),
        e14_bge_m3_dense: Vec::new(),
    }
}

fn fingerprint_at(stored_at: chrono::DateTime<Utc>) -> TeleologicalFingerprint {
    TeleologicalFingerprint {
        id: Uuid::new_v4(),
        semantic: empty_semantic(),
        content_hash: [0u8; 32],
        created_at: stored_at,
        last_updated: stored_at,
        access_count: 0,
        importance: 0.5,
        last_accessed_at: stored_at,
        e6_sparse: None,
    }
}

// -----------------------------------------------------------------------------
// Roundtrip / lifecycle
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_single_record_preserves_all_fields() {
    let (_td, store) = open_store();
    let record = synthetic_record("hello fsv", &[(0, 0.9), (6, 0.8)]);
    let id = record.memory_id;

    println!(
        "BEFORE store_training_record: count={}, e14_len={}, cross_len={}, cross_last={:.6}",
        store.count_training_records().await.unwrap(),
        record.e14_bge_m3_dense.len(),
        record.cross_correlations.len(),
        record
            .cross_correlations
            .last()
            .copied()
            .unwrap_or_default()
    );
    store
        .store_training_record(id, &record)
        .await
        .expect("store");
    println!(
        "AFTER store_training_record: count={}",
        store.count_training_records().await.unwrap()
    );

    let back = store
        .get_training_record(id)
        .await
        .expect("get")
        .expect("exists");
    println!(
        "AFTER get_training_record: e14_len={}, e14_values={:?}, cross_len={}, cross_last={:.6}",
        back.e14_bge_m3_dense.len(),
        back.e14_bge_m3_dense,
        back.cross_correlations.len(),
        back.cross_correlations.last().copied().unwrap_or_default()
    );
    assert_eq!(back.memory_id, id);
    assert_eq!(back.content, "hello fsv");
    assert_eq!(back.topic_profile, record.topic_profile);
    assert_eq!(back.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
    assert_eq!(back.cross_correlations, record.cross_correlations);
    assert_eq!(back.e14_bge_m3_dense, record.e14_bge_m3_dense);
    assert_eq!(back.group_alignments, record.group_alignments);
    assert_eq!(back.e1_semantic, vec![0.1, 0.2, 0.3]);
    assert_eq!(back.e5_causal_cause, vec![0.5; 8]);
    assert_eq!(back.outgoing_edges.len(), 1);
    assert_eq!(back.outgoing_edges[0].weight, 0.85);
    assert_eq!(back.causal_effects.len(), 1);
    assert_eq!(back.causal_effects[0].confidence, 0.82);
    assert_eq!(
        back.causal_effects[0].mechanism_type.as_deref(),
        Some("mediated")
    );
    assert_eq!(back.causal_effects[0].related_memory_id, Uuid::nil());
    assert_eq!(back.knn_neighbors.len(), NUM_EMBEDDERS);
    assert_eq!(back.content_hash, Some([0xAB; 32]));
    assert!(back.temporal_labels.is_none());
    assert!(back.tucker_core.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_and_count_reflect_writes() {
    let (_td, store) = open_store();

    assert_eq!(store.count_training_records().await.unwrap(), 0);
    assert!(store.list_training_record_ids().await.unwrap().is_empty());

    let mut ids = Vec::new();
    for i in 0..5 {
        let rec = synthetic_record(&format!("doc {}", i), &[(0, 0.5 + i as f32 * 0.1)]);
        ids.push(rec.memory_id);
        store
            .store_training_record(rec.memory_id, &rec)
            .await
            .unwrap();
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
    let deleted = store.delete_training_record(Uuid::new_v4()).await.unwrap();
    assert!(!deleted);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_existing_removes_record() {
    let (_td, store) = open_store();
    let rec = synthetic_record("doomed", &[(0, 1.0)]);
    let id = rec.memory_id;
    store.store_training_record(id, &rec).await.unwrap();

    let deleted = store.delete_training_record(id).await.unwrap();
    assert!(deleted);

    assert_eq!(store.count_training_records().await.unwrap(), 0);
    assert!(store.get_training_record(id).await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_all_removes_every_record() {
    let (_td, store) = open_store();
    for i in 0..7 {
        let rec = synthetic_record(&format!("r{}", i), &[(i % NUM_EMBEDDERS, 0.5)]);
        store
            .store_training_record(rec.memory_id, &rec)
            .await
            .unwrap();
    }

    let cleared = store.clear_all_training_records().await.unwrap();
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

    rec.content = "v2 overwritten".into();
    store.store_training_record(id, &rec).await.unwrap();
    let after = store.get_training_record(id).await.unwrap().unwrap();

    assert_eq!(after.content, "v2 overwritten");
    assert_eq!(store.count_training_records().await.unwrap(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_nonexistent_returns_none() {
    let (_td, store) = open_store();
    assert!(store
        .get_training_record(Uuid::new_v4())
        .await
        .unwrap()
        .is_none());
}

// -----------------------------------------------------------------------------
// multi_get
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn multi_get_returns_parallel_slice_respecting_missing_keys() {
    let (_td, store) = open_store();

    // Store 3 records, then multi-get 5 ids (2 missing, interleaved).
    let mut stored_ids = Vec::new();
    for i in 0..3 {
        let rec = synthetic_record(&format!("m{}", i), &[(0, 0.5)]);
        stored_ids.push(rec.memory_id);
        store
            .store_training_record(rec.memory_id, &rec)
            .await
            .unwrap();
    }

    let missing_a = Uuid::new_v4();
    let missing_b = Uuid::new_v4();
    let query: Vec<Uuid> = vec![
        stored_ids[0],
        missing_a,
        stored_ids[1],
        missing_b,
        stored_ids[2],
    ];

    let got = store.multi_get_training_records(&query).await.unwrap();
    assert_eq!(got.len(), query.len());
    assert!(got[0].is_some());
    assert!(got[1].is_none());
    assert!(got[2].is_some());
    assert!(got[3].is_none());
    assert!(got[4].is_some());
    assert_eq!(got[0].as_ref().unwrap().memory_id, stored_ids[0]);
    assert_eq!(got[4].as_ref().unwrap().memory_id, stored_ids[2]);
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_get_empty_slice_returns_empty_vec() {
    let (_td, store) = open_store();
    let got = store.multi_get_training_records(&[]).await.unwrap();
    assert!(got.is_empty());
}

// -----------------------------------------------------------------------------
// Math identities (verified through RocksDB roundtrip)
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn math_identity_group_alignments_implementation_equals_topic_profile_slot_5() {
    let (_td, store) = open_store();
    let mut rec = synthetic_record("implementation-heavy", &[]);
    rec.topic_profile = [0.0f32; NUM_EMBEDDERS];
    rec.topic_profile[5] = 0.73;
    rec.group_alignments = compute_group_alignments(&rec.topic_profile);
    rec.cross_correlations =
        compute_cross_correlations(&rec.topic_profile, &SynergyMatrix::with_base_synergies());
    let id = rec.memory_id;

    println!(
        "BEFORE store: topic_profile[5]={}, group_alignments[5]={}",
        rec.topic_profile[5], rec.group_alignments[5]
    );
    store.store_training_record(id, &rec).await.unwrap();

    let back = store.get_training_record(id).await.unwrap().unwrap();
    println!(
        "AFTER get: topic_profile[5]={}, group_alignments[5]={}",
        back.topic_profile[5], back.group_alignments[5]
    );
    assert!(
        (back.group_alignments[5] - 0.73).abs() < 1e-5,
        "Implementation must equal topic_profile[5]=0.73, got {}",
        back.group_alignments[5],
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn math_identity_qualitative_halves_e10_when_e11_disabled() {
    let (_td, store) = open_store();
    let mut rec = synthetic_record("qualitative-only-e10", &[]);
    rec.topic_profile = [0.0f32; NUM_EMBEDDERS];
    rec.topic_profile[9] = 0.8; // E10
    rec.topic_profile[10] = 0.0; // E11 disabled
    rec.group_alignments = compute_group_alignments(&rec.topic_profile);
    rec.cross_correlations =
        compute_cross_correlations(&rec.topic_profile, &SynergyMatrix::with_base_synergies());
    let id = rec.memory_id;

    store.store_training_record(id, &rec).await.unwrap();
    let back = store.get_training_record(id).await.unwrap().unwrap();
    assert!(
        (back.group_alignments[4] - 0.4).abs() < 1e-5,
        "Qualitative=(E10+E11)/2 with E11=0 must be 0.4, got {}",
        back.group_alignments[4],
    );
}

// -----------------------------------------------------------------------------
// Temporal labels (Phase 5) — verified through RocksDB roundtrip
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn temporal_labels_known_saturday_morning_yields_weekend_morning_bucket() {
    let (_td, store) = open_store();
    let stored_at = Utc.with_ymd_and_hms(2026, 2, 28, 9, 0, 0).unwrap(); // Sat
    let fp = fingerprint_at(stored_at);
    let export_now = Utc.with_ymd_and_hms(2026, 2, 28, 9, 5, 0).unwrap();
    let labels = extract_temporal_labels(&fp, None, None, None, export_now);
    assert_eq!(labels.periodic_bucket, PeriodicBucket::WeekendMorning);

    let mut rec = synthetic_record("saturday-morning", &[]);
    rec.temporal_labels = Some(labels.clone());
    let id = rec.memory_id;
    store.store_training_record(id, &rec).await.unwrap();

    let back = store.get_training_record(id).await.unwrap().unwrap();
    let back_labels = back.temporal_labels.expect("temporal_labels Some");
    assert_eq!(back_labels.periodic_bucket, PeriodicBucket::WeekendMorning);
    assert_eq!(back_labels.stored_hour_utc, 9);
    assert_eq!(back_labels.stored_day_of_week, 6);
    assert_eq!(back_labels.age_seconds_at_export, 300);
}

#[tokio::test(flavor = "multi_thread")]
async fn temporal_labels_age_seconds_non_negative_for_past_stored_at() {
    let stored_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let export_now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 10).unwrap();
    let fp = fingerprint_at(stored_at);
    let labels = extract_temporal_labels(&fp, None, None, None, export_now);
    assert!(
        labels.age_seconds_at_export >= 0,
        "age must be non-negative for past stored_at, got {}",
        labels.age_seconds_at_export,
    );
}

// -----------------------------------------------------------------------------
// Phase 4: Tucker-core persistence (verified through RocksDB roundtrip)
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn record_with_tucker_core_roundtrips_through_rocksdb() {
    let (_td, store) = open_store();

    // Construct a deterministic TuckerCore. Default ranks (4, 4, 128) so the
    // data/factor sizes match TuckerCore::new's invariants exactly:
    //   data = 4 * 4 * 128 = 2048
    //   u1   = 13 * 4       = 52
    //   u2   = 13 * 4       = 52
    //   u3   = 1024 * 128   = 131072
    let ranks = (4usize, 4usize, 128usize);
    let data: Vec<f32> = (0..(ranks.0 * ranks.1 * ranks.2))
        .map(|i| (i as f32) * 0.001 + 1.0)
        .collect();
    let u1: Vec<f32> = (0..(NUM_EMBEDDERS * ranks.0))
        .map(|i| 0.1 + (i as f32) * 0.0001)
        .collect();
    let u2: Vec<f32> = (0..(NUM_EMBEDDERS * ranks.1))
        .map(|i| 0.2 + (i as f32) * 0.0001)
        .collect();
    let u3: Vec<f32> = (0..(1024 * ranks.2))
        .map(|i| 0.3 + (i as f32) * 0.00001)
        .collect();
    let tucker = TuckerCore {
        ranks,
        data,
        u1,
        u2,
        u3,
    };

    let mut rec = synthetic_record("tucker-persist", &[(0, 0.7)]);
    rec.tucker_core = Some(tucker.clone());
    let id = rec.memory_id;

    println!(
        "BEFORE store: tucker.data.len()={}, u1.len()={}, u3.len()={}",
        tucker.data.len(),
        tucker.u1.len(),
        tucker.u3.len(),
    );
    store.store_training_record(id, &rec).await.unwrap();

    let back = store.get_training_record(id).await.unwrap().unwrap();
    let back_tc = back.tucker_core.expect("tucker_core must roundtrip Some");
    println!(
        "AFTER get:  tucker.data.len()={}, u1.len()={}, u3.len()={}",
        back_tc.data.len(),
        back_tc.u1.len(),
        back_tc.u3.len(),
    );

    assert_eq!(back_tc.ranks, ranks);
    assert_eq!(back_tc.data.len(), tucker.data.len());
    assert_eq!(back_tc.u1.len(), tucker.u1.len());
    assert_eq!(back_tc.u2.len(), tucker.u2.len());
    assert_eq!(back_tc.u3.len(), tucker.u3.len());

    // Byte-for-byte equality on a few sampled entries (don't dump 131K floats to stdout).
    for idx in [0usize, 1, 2047] {
        assert_eq!(
            back_tc.data[idx].to_bits(),
            tucker.data[idx].to_bits(),
            "data[{}] bit-mismatch",
            idx
        );
    }
    for idx in [0usize, 51] {
        assert_eq!(back_tc.u1[idx].to_bits(), tucker.u1[idx].to_bits());
        assert_eq!(back_tc.u2[idx].to_bits(), tucker.u2[idx].to_bits());
    }
    for idx in [0usize, 1, 131_071] {
        assert_eq!(back_tc.u3[idx].to_bits(), tucker.u3[idx].to_bits());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn record_without_tucker_core_has_none() {
    let (_td, store) = open_store();
    let rec = synthetic_record("no-tucker", &[(0, 0.5)]);
    let id = rec.memory_id;
    assert!(rec.tucker_core.is_none());
    store.store_training_record(id, &rec).await.unwrap();

    let back = store.get_training_record(id).await.unwrap().unwrap();
    assert!(
        back.tucker_core.is_none(),
        "tucker_core must remain None when not populated"
    );
}
