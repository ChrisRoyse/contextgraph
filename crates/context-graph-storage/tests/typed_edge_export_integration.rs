//! Integration tests for `CF_TYPED_EDGE_RECORDS` (F1 of the typed-edges
//! training-data factory).
//!
//! Every test opens a real RocksDB in a fresh [`TempDir`], builds
//! `TypedEdgeTrainingRecord` fixtures (one per edge type), runs them through
//! the production encode/decode path, and asserts field-by-field that the
//! roundtrip is lossless. No mocks.

use std::sync::Arc;

use context_graph_core::error::CoreError;
use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::typed_edge_export::{
    LLMValidationSummary, LLMVerdict, TypedEdgeTrainingRecord, TYPED_EDGE_RECORD_VERSION,
};
use context_graph_storage::teleological::rocksdb_store::{
    decode_typed_edge_record, encode_typed_edge_record,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn open_store() -> (TempDir, Arc<RocksDbTeleologicalStore>) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    let store =
        RocksDbTeleologicalStore::open_with_config(&path, TeleologicalStoreConfig::default())
            .expect("open store");
    (td, Arc::new(store))
}

fn record_for_edge_type(et: GraphLinkEdgeType) -> TypedEdgeTrainingRecord {
    let edge_type_u8 = et.as_u8();
    let mut scores = [0f32; NUM_EMBEDDERS];
    if let Some(i) = et.primary_embedder_index() {
        scores[i] = 0.82;
    } else {
        scores[0] = 0.71;
        scores[5] = 0.65;
        scores[9] = 0.68;
    }

    let llm_validation = Some(LLMValidationSummary {
        validated_at: chrono::Utc::now(),
        verdict: LLMVerdict::Reclassify {
            new_edge_type: (edge_type_u8 + 1) % 8,
        },
        confidence: 0.78,
        rationale: format!("Reclassification rationale for et={}", edge_type_u8),
        validator_version: "deterministic-validator-v1@2026-05".into(),
    });

    TypedEdgeTrainingRecord {
        source_memory_id: Uuid::new_v4(),
        target_memory_id: Uuid::new_v4(),
        edge_type: edge_type_u8,
        edge_type_name: format!("{:?}", et).to_lowercase(),
        weight: 0.6 + 0.02 * edge_type_u8 as f32,
        direction: if et.is_asymmetric() { 1 } else { 0 },
        embedder_scores: scores,
        agreement_count: 1,
        agreeing_embedders: 1u16,
        source_content: format!("source content for et={}", edge_type_u8),
        target_content: format!("target content for et={}", edge_type_u8),
        source_session_id: Some(format!("session-{}", edge_type_u8)),
        target_session_id: Some(format!("session-{}", edge_type_u8)),
        source_type: Some("HookDescription".into()),
        target_type: Some("HookDescription".into()),
        mechanism_type: if matches!(et, GraphLinkEdgeType::CausalChain) {
            Some("direct".into())
        } else {
            None
        },
        llm_validation,
        exported_at: chrono::Utc::now(),
        exporter_version: "typed_edge_export_v1".into(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn stores_all_eight_edge_type_records() {
    let (_td, store) = open_store();

    let records: Vec<TypedEdgeTrainingRecord> = GraphLinkEdgeType::all()
        .iter()
        .map(|et| record_for_edge_type(*et))
        .collect();
    assert_eq!(records.len(), 8, "must cover every edge type");

    for r in &records {
        store
            .store_typed_edge_record(r)
            .await
            .expect("store typed-edge record");
    }

    assert_eq!(
        store.count_typed_edge_records().await.unwrap(),
        8,
        "all 8 records must be present"
    );

    let keys = store.list_typed_edge_record_keys().await.unwrap();
    assert_eq!(keys.len(), 8);

    // Every (source, target, edge_type) key in `keys` must match exactly one
    // of the records we stored.
    for r in &records {
        let hit = keys.iter().find(|(s, t, et)| {
            *s == r.source_memory_id && *t == r.target_memory_id && *et == r.edge_type
        });
        assert!(
            hit.is_some(),
            "stored record missing from list_typed_edge_record_keys: {:?}",
            r.edge_type_name
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_is_byte_identical() {
    let (_td, store) = open_store();
    let original = record_for_edge_type(GraphLinkEdgeType::CausalChain);

    store
        .store_typed_edge_record(&original)
        .await
        .expect("store");

    let back = store
        .get_typed_edge_record(
            original.source_memory_id,
            original.target_memory_id,
            original.edge_type,
        )
        .await
        .expect("get")
        .expect("must exist");

    assert_eq!(back, original, "roundtrip must be byte-identical");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_returns_none_for_missing_key() {
    let (_td, store) = open_store();

    let miss = store
        .get_typed_edge_record(Uuid::new_v4(), Uuid::new_v4(), 3)
        .await
        .expect("get");
    assert!(miss.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_all_empties_the_cf() {
    let (_td, store) = open_store();
    for et in GraphLinkEdgeType::all() {
        store
            .store_typed_edge_record(&record_for_edge_type(et))
            .await
            .unwrap();
    }
    assert_eq!(store.count_typed_edge_records().await.unwrap(), 8);

    let cleared = store.clear_all_typed_edge_records().await.unwrap();
    assert_eq!(cleared, 8);
    assert_eq!(store.count_typed_edge_records().await.unwrap(), 0);
}

#[test]
fn decode_rejects_wrong_version() {
    let record = record_for_edge_type(GraphLinkEdgeType::SemanticSimilar);
    let mut bytes = encode_typed_edge_record(&record).unwrap();
    assert_eq!(bytes[0], TYPED_EDGE_RECORD_VERSION);
    bytes[0] = TYPED_EDGE_RECORD_VERSION.wrapping_add(1);

    let err = decode_typed_edge_record(&bytes).unwrap_err();
    match &err {
        CoreError::SerializationError(msg) => {
            assert!(
                msg.contains("version mismatch"),
                "expected 'version mismatch' in: {}",
                msg
            );
        }
        other => panic!("expected SerializationError, got {:?}", other),
    }
}

#[test]
fn decode_rejects_empty_payload() {
    let err = decode_typed_edge_record(&[]).unwrap_err();
    match &err {
        CoreError::SerializationError(msg) => {
            assert!(msg.contains("empty"), "expected 'empty' in: {}", msg);
        }
        other => panic!("expected SerializationError, got {:?}", other),
    }
}

#[test]
fn encode_prepends_version_byte() {
    let record = record_for_edge_type(GraphLinkEdgeType::MultiAgreement);
    let bytes = encode_typed_edge_record(&record).unwrap();
    assert!(
        bytes.len() > 1,
        "encoded record must have at least version byte + body"
    );
    assert_eq!(bytes[0], TYPED_EDGE_RECORD_VERSION);
}
