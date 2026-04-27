//! Full State Verification for the UTL learner-state storage namespace.
//!
//! Source of truth:
//! - RocksDB `learner_profile`
//! - RocksDB `fingerprints_learner`
//! - RocksDB `learner_state_history`
//! - RocksDB `learner_delta_log`
//! - RocksDB `learner_m_per_trace`
//! - RocksDB `learner_audit`
//!
//! Every test writes through the public API, then performs a separate read from
//! the persisted RocksDB column family to prove the final state exists.

use std::collections::{BTreeMap, BTreeSet};

use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_bytes,
    sha256_json, update_m_trace, ComputationEnvelope, LearnerAuditEntry, LearnerDeltaLog,
    LearnerFingerprint, LearnerModality, LearnerProfile, LearnerStateComponents,
    LearnerStateVector, ModalityEmbedding, ObservationEnvelope, MAX_LEARNER_STATE_VECTOR_VALUES,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

fn open_store(dir: &TempDir) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open_with_config(dir.path(), TeleologicalStoreConfig::default())
        .expect("open RocksDB teleological store")
}

async fn counts(store: &RocksDbTeleologicalStore) -> (usize, usize, usize, usize, usize, usize) {
    (
        store.count_learner_profiles().await.unwrap(),
        store.count_learner_fingerprints().await.unwrap(),
        store.count_learner_state_history().await.unwrap(),
        store.count_learner_delta_logs().await.unwrap(),
        store.count_learner_m_traces().await.unwrap(),
        store.count_learner_audit_entries().await.unwrap(),
    )
}

fn components() -> LearnerStateComponents {
    LearnerStateComponents {
        plasticity_window: 0.81,
        hrv_coherence: 0.64,
        valence: 0.2,
        arousal: 0.0,
        stress_floor: 0.9,
        k_sleep: 1.0,
    }
}

fn synthetic_records(
    learner_id: Uuid,
    trace_id: Uuid,
    session_ts: u64,
) -> (
    LearnerProfile,
    LearnerFingerprint,
    LearnerDeltaLog,
    context_graph_core::learner::LearnerMTrace,
    LearnerAuditEntry,
) {
    let profile = LearnerProfile::new(
        learner_id,
        "fsv-learner".into(),
        "consented-local-first".into(),
        BTreeSet::from([LearnerModality::AffectText, LearnerModality::SelfReport]),
        Some(session_ts),
    )
    .unwrap();
    let observation_id = Uuid::from_u128(0x11111111_1111_4111_8111_111111111111);
    let envelope = ObservationEnvelope::new(
        observation_id,
        learner_id,
        session_ts,
        LearnerModality::AffectText,
        "consented-local-first".into(),
        sha256_bytes(b"fsv synthetic raw observation"),
        "fsv-preprocess-v1".into(),
        "fsv-embedder-v1".into(),
        "thresholds-default-pending-calibration-v1".into(),
        Vec::new(),
    )
    .unwrap();
    let state_vector = LearnerStateVector {
        learner_id,
        session_ts,
        values: vec![0.81, 0.64, 0.2, 0.0, 0.9, 1.0],
        components: components(),
        context: BTreeMap::from([("source".into(), "fsv".into())]),
    };
    let fingerprint = LearnerFingerprint {
        learner_id,
        session_ts,
        observation_envelopes: vec![envelope],
        modality_embeddings: vec![ModalityEmbedding {
            modality: LearnerModality::AffectText,
            vector: vec![0.62, 0.71, 0.54],
            scalar: Some(0.62),
            source_observation_id: observation_id,
        }],
        state_vector,
    };

    let delta_s = compute_delta_s_from_text(
        "ephemeral means temporary",
        "ephemeral means lasting only a short time",
        Some("temporary short lived concept"),
        0.2,
        Some(0.7),
    )
    .unwrap();
    let delta_c = compute_delta_c(&[0.62, 0.74, 0.86], 0.64, 0.82, 0.05, None).unwrap();
    let delta_e = compute_delta_e(&components()).unwrap();
    let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None).unwrap();
    let output_hash = sha256_json(&computation).unwrap();
    let provenance = ComputationEnvelope::new(
        Uuid::from_u128(0x22222222_2222_4222_8222_222222222222),
        learner_id,
        session_ts,
        vec![observation_id],
        "thresholds-default-pending-calibration-v1".into(),
        output_hash.clone(),
    )
    .unwrap();
    let delta_log = LearnerDeltaLog {
        learner_id,
        session_ts,
        computation,
        provenance,
    };
    let m_trace = update_m_trace(
        None,
        learner_id,
        trace_id,
        session_ts,
        &delta_log.computation,
        Some(true),
    )
    .unwrap();
    let audit = LearnerAuditEntry {
        audit_id: Uuid::from_u128(0x33333333_3333_4333_8333_333333333333),
        learner_id,
        ts: session_ts,
        action: "fsv_record_synthetic".into(),
        target_cf: "learner_delta_log".into(),
        result_sha256: output_hash,
        parent_audit_id: None,
    };

    (profile, fingerprint, delta_log, m_trace, audit)
}

#[tokio::test]
async fn happy_path_persists_and_reopens_all_source_of_truth_rows() {
    println!("=== FSV: UTL learner-state happy path ===");
    println!(
        "SOURCE OF TRUTH: RocksDB CFs learner_profile, fingerprints_learner, learner_state_history, learner_delta_log, learner_m_per_trace, learner_audit"
    );
    let dir = TempDir::new().unwrap();
    let learner_id = Uuid::from_u128(0xaaaaaaaa_aaaa_4aaa_8aaa_aaaaaaaaaaaa);
    let trace_id = Uuid::from_u128(0xbbbbbbbb_bbbb_4bbb_8bbb_bbbbbbbbbbbb);
    let session_ts = 1_700_000_000;

    {
        let store = open_store(&dir);
        let before = counts(&store).await;
        println!("BEFORE counts={before:?}");
        assert_eq!(before, (0, 0, 0, 0, 0, 0));
        let (profile, fingerprint, delta_log, m_trace, audit) =
            synthetic_records(learner_id, trace_id, session_ts);
        println!(
            "TRIGGER write learner_id={} trace_id={} session_ts={} expected_state_len={} expected_l={}",
            learner_id,
            trace_id,
            session_ts,
            fingerprint.state_vector.values.len(),
            delta_log.computation.l
        );
        store.store_learner_profile(&profile).await.unwrap();
        store.store_learner_fingerprint(&fingerprint).await.unwrap();
        store.store_learner_delta_log(&delta_log).await.unwrap();
        store.store_learner_m_trace(&m_trace).await.unwrap();
        store.store_learner_audit_entry(&audit).await.unwrap();
        let after = counts(&store).await;
        println!("AFTER immediate counts={after:?}");
        assert_eq!(after, (1, 1, 1, 1, 1, 1));
    }

    {
        let store = open_store(&dir);
        let reopened_counts = counts(&store).await;
        println!("AFTER REOPEN counts={reopened_counts:?}");
        assert_eq!(reopened_counts, (1, 1, 1, 1, 1, 1));

        let profile = store
            .get_learner_profile(learner_id)
            .await
            .unwrap()
            .expect("profile exists");
        let fingerprint = store
            .get_learner_fingerprint(learner_id, session_ts)
            .await
            .unwrap()
            .expect("fingerprint exists");
        let state = store
            .get_learner_state_vector(learner_id, session_ts)
            .await
            .unwrap()
            .expect("state history exists");
        let delta_log = store
            .get_learner_delta_log(learner_id, session_ts)
            .await
            .unwrap()
            .expect("delta log exists");
        let m_trace = store
            .get_learner_m_trace(learner_id, trace_id)
            .await
            .unwrap()
            .expect("m trace exists");

        println!(
            "READBACK profile.handle={} modalities={} state_len={} delta_s={} delta_c={} delta_e={} l={} m_value={} next_review_ts={:?}",
            profile.handle,
            profile.modalities_enabled.len(),
            state.values.len(),
            delta_log.computation.delta_s.delta_s,
            delta_log.computation.delta_c.delta_c,
            delta_log.computation.delta_e.delta_e,
            delta_log.computation.l,
            m_trace.m_value,
            m_trace.next_review_ts
        );
        assert_eq!(fingerprint.modality_embeddings.len(), 1);
        assert_eq!(state.values.len(), 6);
        assert!(delta_log.computation.l > 0.0);
        assert!(m_trace.m_value > 0.0);
    }
}

#[tokio::test]
async fn edge_empty_text_inputs_persist_zero_surprise_delta_log() {
    println!("=== EDGE CASE: empty Delta S text inputs ===");
    let dir = TempDir::new().unwrap();
    let learner_id = Uuid::from_u128(0x44444444_4444_4444_8444_444444444444);
    let session_ts = 1_700_000_100;
    let store = open_store(&dir);
    println!("BEFORE counts={:?}", counts(&store).await);

    let delta_s = compute_delta_s_from_text("", "", None, 0.0, None).unwrap();
    let delta_c = compute_delta_c(&[], 0.0, 0.0, 0.0, None).unwrap();
    let delta_e = compute_delta_e(&components()).unwrap();
    let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None).unwrap();
    let provenance = ComputationEnvelope::new(
        Uuid::from_u128(0x55555555_5555_4555_8555_555555555555),
        learner_id,
        session_ts,
        Vec::new(),
        "thresholds-default-pending-calibration-v1".into(),
        sha256_json(&computation).unwrap(),
    )
    .unwrap();
    let log = LearnerDeltaLog {
        learner_id,
        session_ts,
        computation,
        provenance,
    };
    store.store_learner_delta_log(&log).await.unwrap();
    let readback = store
        .get_learner_delta_log(learner_id, session_ts)
        .await
        .unwrap()
        .unwrap();
    println!(
        "AFTER delta_log_count={} delta_s={} delta_c={} l={}",
        store.count_learner_delta_logs().await.unwrap(),
        readback.computation.delta_s.delta_s,
        readback.computation.delta_c.delta_c,
        readback.computation.l
    );
    assert_eq!(readback.computation.delta_s.delta_s, 0.0);
    assert_eq!(readback.computation.delta_c.delta_c, 0.0);
    assert_eq!(readback.computation.l, 0.0);
}

#[tokio::test]
async fn edge_invalid_component_is_rejected_and_not_persisted() {
    println!("=== EDGE CASE: invalid Delta E component ===");
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let before = counts(&store).await;
    println!("BEFORE counts={before:?}");
    let invalid = LearnerStateComponents {
        stress_floor: 1.2,
        ..components()
    };
    let result = compute_delta_e(&invalid);
    println!("ACTION invalid stress_floor=1.2 result={result:?}");
    assert!(result.is_err());
    let after = counts(&store).await;
    println!("AFTER counts={after:?}");
    assert_eq!(before, after);
}

#[tokio::test]
async fn edge_max_state_vector_boundary_persists_exact_length() {
    println!("=== EDGE CASE: max learner state vector length ===");
    let dir = TempDir::new().unwrap();
    let learner_id = Uuid::from_u128(0x66666666_6666_4666_8666_666666666666);
    let session_ts = 1_700_000_200;
    let store = open_store(&dir);
    println!(
        "BEFORE state_history_count={}",
        store.count_learner_state_history().await.unwrap()
    );

    let observation_id = Uuid::from_u128(0x77777777_7777_4777_8777_777777777777);
    let envelope = ObservationEnvelope::new(
        observation_id,
        learner_id,
        session_ts,
        LearnerModality::SelfReport,
        "consented-local-first".into(),
        sha256_bytes(b"max vector"),
        "fsv-preprocess-v1".into(),
        "fsv-embedder-v1".into(),
        "thresholds-default-pending-calibration-v1".into(),
        Vec::new(),
    )
    .unwrap();
    let state_vector = LearnerStateVector {
        learner_id,
        session_ts,
        values: vec![0.25; MAX_LEARNER_STATE_VECTOR_VALUES],
        components: components(),
        context: BTreeMap::new(),
    };
    let fingerprint = LearnerFingerprint {
        learner_id,
        session_ts,
        observation_envelopes: vec![envelope],
        modality_embeddings: vec![ModalityEmbedding {
            modality: LearnerModality::SelfReport,
            vector: vec![0.25],
            scalar: Some(0.25),
            source_observation_id: observation_id,
        }],
        state_vector,
    };
    store.store_learner_fingerprint(&fingerprint).await.unwrap();
    let readback = store
        .get_learner_state_vector(learner_id, session_ts)
        .await
        .unwrap()
        .unwrap();
    println!(
        "AFTER state_history_count={} readback_state_len={}",
        store.count_learner_state_history().await.unwrap(),
        readback.values.len()
    );
    assert_eq!(readback.values.len(), MAX_LEARNER_STATE_VECTOR_VALUES);
}
