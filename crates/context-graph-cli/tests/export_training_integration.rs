//! Phase 6 — integration tests for `context-graph-cli export training-corpus`.
//!
//! Every test uses a real `RocksDbTeleologicalStore` in a fresh `TempDir`
//! and a real Parquet reader to verify the produced file. NO MOCKS.
//!
//! Coverage:
//! - `roundtrip_training_corpus_parquet` — write N synthetic records, export,
//!   read back through the `parquet` crate, assert every field survives the
//!   bincode roundtrip.
//! - `empty_cf_produces_empty_parquet_file` — empty input yields a valid
//!   Parquet file with 0 rows.
//! - `batch_size_respected` — row-group count == ceil(total / batch_size).
//! - `limit_caps_records` — `--limit` bounds the written row count.
//! - `invalid_output_dir_fails_fast` — nonexistent parent dir returns `Err`.

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{Array, BinaryArray, StringArray};
use chrono::Utc;
use context_graph_cli::commands::export_training::{run, ExportTrainingArgs};
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, CausalLabel, TrainingEdge,
    TrainingRecord, NUM_CROSS_CORRELATIONS,
};
use context_graph_core::types::fingerprint::E14_DIM;
use context_graph_storage::teleological::{
    decode_training_record, RocksDbTeleologicalStore, TeleologicalStoreConfig,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::TempDir;
use uuid::Uuid;

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

/// Build a synthetic `TrainingRecord` with a known structure. Deterministic
/// per-call inputs so tests can assert byte-for-byte equality after roundtrip.
fn synthetic_record(content: &str, topic_peaks: &[(usize, f32)]) -> TrainingRecord {
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
        content: content.to_string(),
        importance: 0.75,
        created_at: Utc::now(),
        session_id: Some("phase6-fsv".into()),
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
        e14_bge_m3_dense: vec![0.14; E14_DIM],
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
            // One SemanticSimilar outgoing edge above (edge_type=0).
            let mut d = [0u32; 8];
            d[0] = 1;
            d
        },
    }
}

/// Open a real RocksDB store in a fresh temp dir. Returns `(TempDir, path)`;
/// the temp dir is returned so the caller can keep it alive (TempDir deletes
/// on drop).
fn fresh_store_dir() -> (TempDir, PathBuf) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    (td, path)
}

/// Write N synthetic records into the store at `path` and return their ids +
/// records (records kept for equality checks).
async fn seed_store(path: &std::path::Path, n: usize) -> Vec<(Uuid, TrainingRecord)> {
    let store =
        RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default())
            .expect("open store for seeding");
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rec = synthetic_record(
            &format!("fsv-{}", i),
            &[(i % NUM_EMBEDDERS, 0.5 + (i as f32) * 0.01)],
        );
        let id = rec.memory_id;
        store
            .store_training_record(id, &rec)
            .await
            .expect("store_training_record");
        out.push((id, rec));
    }
    // Drop the store so RocksDB releases the LOCK file for the next opener
    // (the CLI reopens the same path).
    drop(store);
    out
}

/// Open the store once and return an Arc so multiple operations share it.
async fn open_shared_store(path: &std::path::Path) -> Arc<RocksDbTeleologicalStore> {
    let store =
        RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default())
            .expect("open store");
    Arc::new(store)
}

/// Read all (memory_id, record_bytes) rows back from a Parquet file.
fn read_parquet_rows(path: &std::path::Path) -> (Vec<String>, Vec<Vec<u8>>, Vec<usize>) {
    let file = File::open(path).expect("open parquet file for read");
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(file).expect("ParquetRecordBatchReaderBuilder");
    // Keep the row-group count BEFORE consuming the reader (try_new parsed
    // the metadata upfront).
    let row_group_count = builder.metadata().num_row_groups();
    let row_group_rows: Vec<usize> = (0..row_group_count)
        .map(|i| builder.metadata().row_group(i).num_rows() as usize)
        .collect();
    let reader = builder.build().expect("build parquet reader");

    let mut ids = Vec::new();
    let mut payloads = Vec::new();
    for batch in reader {
        let batch = batch.expect("read batch");
        let id_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("memory_id column");
        let payload_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .expect("record_bytes column");
        for row in 0..batch.num_rows() {
            ids.push(id_col.value(row).to_string());
            payloads.push(payload_col.value(row).to_vec());
        }
    }
    (ids, payloads, row_group_rows)
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_training_corpus_parquet() {
    let (_td, storage) = fresh_store_dir();
    let seeded = seed_store(&storage, 3).await;

    let out_dir = TempDir::new().expect("out tempdir");
    let out_path = out_dir.path().join("corpus.parquet");

    let args = ExportTrainingArgs {
        kind: "training-corpus".to_string(),
        format: "parquet".to_string(),
        out: out_path.clone(),
        storage: storage.clone(),
        batch_size: 10,
        limit: None,
    };

    let summary = run(args).await.expect("export ok");
    assert_eq!(summary.total_records, 3, "all 3 records exported");
    assert!(summary.bytes_written > 0, "parquet file has bytes");
    // 3 records, batch_size 10 → one row group
    assert_eq!(summary.row_groups, 1);

    let (ids, payloads, _) = read_parquet_rows(&out_path);
    assert_eq!(ids.len(), 3);
    assert_eq!(payloads.len(), 3);

    // Decode every payload and verify the full TrainingRecord survives.
    use std::collections::HashMap;
    let seeded_by_id: HashMap<Uuid, &TrainingRecord> =
        seeded.iter().map(|(id, r)| (*id, r)).collect();

    for (id_str, payload) in ids.iter().zip(payloads.iter()) {
        let id = Uuid::parse_str(id_str).expect("valid uuid");
        let expected = seeded_by_id
            .get(&id)
            .unwrap_or_else(|| panic!("parquet row has unknown id {}", id));
        let decoded = decode_training_record(payload).expect("decode payload");
        // Spot-check every field that the synthetic record sets.
        assert_eq!(decoded.memory_id, id);
        assert_eq!(decoded.memory_id, expected.memory_id);
        assert_eq!(decoded.content, expected.content);
        assert_eq!(decoded.importance, expected.importance);
        assert_eq!(decoded.session_id, expected.session_id);
        assert_eq!(decoded.source_type, expected.source_type);
        assert_eq!(decoded.content_hash, expected.content_hash);
        assert_eq!(decoded.e1_semantic, expected.e1_semantic);
        assert_eq!(decoded.e5_causal_cause, expected.e5_causal_cause);
        assert_eq!(decoded.e5_causal_effect, expected.e5_causal_effect);
        assert_eq!(decoded.e6_sparse_indices, expected.e6_sparse_indices);
        assert_eq!(decoded.e6_sparse_values, expected.e6_sparse_values);
        assert_eq!(decoded.topic_profile, expected.topic_profile);
        assert_eq!(decoded.cross_correlations, expected.cross_correlations);
        assert_eq!(decoded.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
        assert_eq!(decoded.group_alignments, expected.group_alignments);
        assert_eq!(decoded.outgoing_edges.len(), 1);
        assert_eq!(decoded.outgoing_edges[0].weight, 0.85);
        assert_eq!(decoded.causal_effects.len(), 1);
        assert_eq!(decoded.causal_effects[0].confidence, 0.82);
        assert_eq!(
            decoded.causal_effects[0].mechanism_type.as_deref(),
            Some("mediated")
        );
        assert_eq!(decoded.knn_neighbors.len(), NUM_EMBEDDERS);
        assert!(decoded.temporal_labels.is_none());
        assert!(decoded.tucker_core.is_none());
    }

    // Sanity: keep the seeded store alive long enough that we can open it
    // again afterwards (regression guard against FD leaks in run()).
    let shared = open_shared_store(&storage).await;
    let count = shared.count_training_records().await.expect("count");
    assert_eq!(count, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_cf_produces_empty_parquet_file() {
    let (_td, storage) = fresh_store_dir();
    // Open + close the store so the directory structure exists with zero rows.
    {
        let _ = RocksDbTeleologicalStore::open_with_config(
            &storage,
            TeleologicalStoreConfig::default(),
        )
        .expect("open empty store");
    }

    let out_dir = TempDir::new().expect("out tempdir");
    let out_path = out_dir.path().join("empty.parquet");

    let args = ExportTrainingArgs {
        kind: "training-corpus".to_string(),
        format: "parquet".to_string(),
        out: out_path.clone(),
        storage: storage.clone(),
        batch_size: 100,
        limit: None,
    };

    let summary = run(args).await.expect("export ok");
    assert_eq!(summary.total_records, 0);
    assert_eq!(summary.row_groups, 0);
    assert!(out_path.exists(), "parquet file exists even when empty");
    assert!(summary.bytes_written > 0, "parquet footer is non-empty");

    let (ids, payloads, row_group_rows) = read_parquet_rows(&out_path);
    assert!(ids.is_empty());
    assert!(payloads.is_empty());
    assert!(row_group_rows.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_size_respected() {
    let (_td, storage) = fresh_store_dir();
    seed_store(&storage, 50).await;

    let out_dir = TempDir::new().expect("out tempdir");
    let out_path = out_dir.path().join("batched.parquet");

    let args = ExportTrainingArgs {
        kind: "training-corpus".to_string(),
        format: "parquet".to_string(),
        out: out_path.clone(),
        storage: storage.clone(),
        batch_size: 10,
        limit: None,
    };

    let summary = run(args).await.expect("export ok");
    assert_eq!(summary.total_records, 50);
    assert_eq!(summary.row_groups, 5, "50 / 10 = 5 row groups");

    let (_ids, _payloads, row_group_rows) = read_parquet_rows(&out_path);
    assert_eq!(row_group_rows.len(), 5);
    for rows in &row_group_rows {
        assert_eq!(*rows, 10, "each row group has batch_size rows");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn limit_caps_records() {
    let (_td, storage) = fresh_store_dir();
    seed_store(&storage, 20).await;

    let out_dir = TempDir::new().expect("out tempdir");
    let out_path = out_dir.path().join("limited.parquet");

    let args = ExportTrainingArgs {
        kind: "training-corpus".to_string(),
        format: "parquet".to_string(),
        out: out_path.clone(),
        storage: storage.clone(),
        batch_size: 100,
        limit: Some(7),
    };

    let summary = run(args).await.expect("export ok");
    assert_eq!(summary.total_records, 7);

    let (ids, payloads, _) = read_parquet_rows(&out_path);
    assert_eq!(ids.len(), 7);
    assert_eq!(payloads.len(), 7);
    for payload in &payloads {
        let _ = decode_training_record(payload).expect("decode payload");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_output_dir_fails_fast() {
    let (_td, storage) = fresh_store_dir();
    seed_store(&storage, 1).await;

    let bogus_out = PathBuf::from("/nonexistent/bogus/path/corpus.parquet");
    let args = ExportTrainingArgs {
        kind: "training-corpus".to_string(),
        format: "parquet".to_string(),
        out: bogus_out.clone(),
        storage: storage.clone(),
        batch_size: 100,
        limit: None,
    };

    let err = run(args)
        .await
        .expect_err("should reject missing parent dir");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("parent directory does not exist"),
        "unexpected error: {}",
        msg
    );
    assert!(!bogus_out.exists(), "no file should be created on failure");
}
