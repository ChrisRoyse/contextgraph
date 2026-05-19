//! Integration tests for `CF_TYPED_EDGE_VALIDATIONS` (F4 of the typed-edges
//! training-data factory).
//!
//! Every test opens a real RocksDB in a fresh [`TempDir`], builds
//! `LLMEdgeValidation` fixtures (covering all three verdict variants), runs
//! them through the production encode/decode path, and asserts field-by-field
//! that the roundtrip is lossless. No mocks; no LLM call — the LLM is a
//! different module's responsibility — this test only verifies storage.

use std::sync::Arc;

use context_graph_core::error::CoreError;
use context_graph_core::llm_edge_validation::{LLMEdgeValidation, LLM_EDGE_VALIDATION_VERSION};
use context_graph_core::typed_edge_export::LLMVerdict;
use context_graph_storage::teleological::rocksdb_store::{
    decode_llm_edge_validation, encode_llm_edge_validation,
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

fn valid_validation() -> LLMEdgeValidation {
    LLMEdgeValidation {
        validated_at: chrono::Utc::now(),
        verdict: LLMVerdict::Valid,
        confidence: 0.94,
        rationale: "Direct cause-effect stated in source.".into(),
        auto_derived_weight: 0.62,
        validator_version: "deterministic-validator-v1@2026-05".into(),
        prompt_hash: [0xAAu8; 32],
    }
}

fn invalid_validation() -> LLMEdgeValidation {
    LLMEdgeValidation {
        validated_at: chrono::Utc::now(),
        verdict: LLMVerdict::Invalid,
        confidence: 0.18,
        rationale: "LLM found no semantic link between source and target.".into(),
        auto_derived_weight: 0.52,
        validator_version: "deterministic-validator-v1@2026-05".into(),
        prompt_hash: [0xBBu8; 32],
    }
}

fn reclassify_validation() -> LLMEdgeValidation {
    LLMEdgeValidation {
        validated_at: chrono::Utc::now(),
        verdict: LLMVerdict::Reclassify { new_edge_type: 2 },
        confidence: 0.71,
        rationale: "Relationship is entity-shared, not causal.".into(),
        auto_derived_weight: 0.58,
        validator_version: "deterministic-validator-v1@2026-05".into(),
        prompt_hash: [0xCCu8; 32],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn stores_all_three_verdict_variants() {
    let (_td, store) = open_store();

    let s1 = Uuid::new_v4();
    let t1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let t2 = Uuid::new_v4();
    let s3 = Uuid::new_v4();
    let t3 = Uuid::new_v4();

    let v1 = valid_validation();
    let v2 = invalid_validation();
    let v3 = reclassify_validation();

    store
        .store_llm_edge_validation(s1, t1, 0, &v1)
        .await
        .unwrap();
    store
        .store_llm_edge_validation(s2, t2, 3, &v2)
        .await
        .unwrap();
    store
        .store_llm_edge_validation(s3, t3, 5, &v3)
        .await
        .unwrap();

    assert_eq!(store.count_llm_edge_validations().await.unwrap(), 3);

    let keys = store.list_llm_edge_validation_keys().await.unwrap();
    assert_eq!(keys.len(), 3);

    // Roundtrip each one through the store and verify byte-identical recovery.
    let back1 = store
        .get_llm_edge_validation(s1, t1, 0)
        .await
        .unwrap()
        .expect("v1 must exist");
    assert_eq!(back1, v1);
    assert!(matches!(back1.verdict, LLMVerdict::Valid));

    let back2 = store
        .get_llm_edge_validation(s2, t2, 3)
        .await
        .unwrap()
        .expect("v2 must exist");
    assert_eq!(back2, v2);
    assert!(matches!(back2.verdict, LLMVerdict::Invalid));

    let back3 = store
        .get_llm_edge_validation(s3, t3, 5)
        .await
        .unwrap()
        .expect("v3 must exist");
    assert_eq!(back3, v3);
    match back3.verdict {
        LLMVerdict::Reclassify { new_edge_type } => assert_eq!(new_edge_type, 2),
        other => panic!("expected Reclassify verdict, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_returns_none_for_missing_key() {
    let (_td, store) = open_store();
    let miss = store
        .get_llm_edge_validation(Uuid::new_v4(), Uuid::new_v4(), 1)
        .await
        .unwrap();
    assert!(miss.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn overwrite_is_idempotent() {
    let (_td, store) = open_store();
    let s = Uuid::new_v4();
    let t = Uuid::new_v4();
    let et = 4u8;

    store
        .store_llm_edge_validation(s, t, et, &valid_validation())
        .await
        .unwrap();
    assert_eq!(store.count_llm_edge_validations().await.unwrap(), 1);

    // Overwrite with a new verdict; count stays at 1.
    store
        .store_llm_edge_validation(s, t, et, &invalid_validation())
        .await
        .unwrap();
    assert_eq!(store.count_llm_edge_validations().await.unwrap(), 1);

    let back = store
        .get_llm_edge_validation(s, t, et)
        .await
        .unwrap()
        .expect("still present");
    assert!(matches!(back.verdict, LLMVerdict::Invalid));
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_all_empties_the_cf() {
    let (_td, store) = open_store();
    for (i, v) in [
        valid_validation(),
        invalid_validation(),
        reclassify_validation(),
    ]
    .iter()
    .enumerate()
    {
        store
            .store_llm_edge_validation(Uuid::new_v4(), Uuid::new_v4(), i as u8, v)
            .await
            .unwrap();
    }
    assert_eq!(store.count_llm_edge_validations().await.unwrap(), 3);

    let cleared = store.clear_all_llm_edge_validations().await.unwrap();
    assert_eq!(cleared, 3);
    assert_eq!(store.count_llm_edge_validations().await.unwrap(), 0);
}

#[test]
fn decode_rejects_wrong_version() {
    let v = valid_validation();
    let mut bytes = encode_llm_edge_validation(&v).unwrap();
    assert_eq!(bytes[0], LLM_EDGE_VALIDATION_VERSION);
    bytes[0] = LLM_EDGE_VALIDATION_VERSION.wrapping_add(1);

    let err = decode_llm_edge_validation(&bytes).unwrap_err();
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
    let err = decode_llm_edge_validation(&[]).unwrap_err();
    match &err {
        CoreError::SerializationError(msg) => {
            assert!(msg.contains("empty"), "expected 'empty' in: {}", msg);
        }
        other => panic!("expected SerializationError, got {:?}", other),
    }
}

#[test]
fn encode_prepends_version_byte() {
    let v = reclassify_validation();
    let bytes = encode_llm_edge_validation(&v).unwrap();
    assert!(bytes.len() > 1);
    assert_eq!(bytes[0], LLM_EDGE_VALIDATION_VERSION);
}
