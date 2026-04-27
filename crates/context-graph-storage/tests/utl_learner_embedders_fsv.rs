//! Full State Verification for UTL learner embedders E14-E20.
//!
//! Source of truth:
//! - embedder registry: `context_graph_embeddings::learner`
//! - calibration dataset: deterministic `synthetic_calibration_fixture()`
//! - persisted result: RocksDB `fingerprints_learner` and `learner_state_history`

use std::collections::{BTreeMap, BTreeSet};

use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_bytes,
    sha256_json, ComputationEnvelope, LearnerAuditEntry, LearnerDeltaLog, LearnerFingerprint,
    LearnerModality, LearnerProfile, ObservationEnvelope,
};
use context_graph_embeddings::{
    embed_calibration_sample, embed_learner_signal, learner_embedder_specs,
    state_vector_from_outputs, synthetic_calibration_fixture, LearnerEmbedderInput,
    LearnerEmbedderSlot, LEARNER_EMBEDDER_COUNT, UTL_PLANNED_TOTAL_EMBEDDERS,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

fn open_store(dir: &TempDir) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open_with_config(dir.path(), TeleologicalStoreConfig::default())
        .expect("open RocksDB teleological store")
}

async fn counts(store: &RocksDbTeleologicalStore) -> (usize, usize, usize, usize) {
    (
        store.count_learner_profiles().await.unwrap(),
        store.count_learner_fingerprints().await.unwrap(),
        store.count_learner_state_history().await.unwrap(),
        store.count_learner_delta_logs().await.unwrap(),
    )
}

#[tokio::test]
async fn happy_path_all_e14_e20_embedders_persist_and_read_back() {
    println!("=== FSV: UTL E14-E20 learner embedders happy path ===");
    println!(
        "SOURCE OF TRUTH: learner embedder registry + synthetic calibration fixture + RocksDB learner CFs"
    );
    let specs = learner_embedder_specs();
    println!(
        "REGISTRY planned_total={} learner_embedder_count={} slots={:?}",
        UTL_PLANNED_TOTAL_EMBEDDERS,
        specs.len(),
        specs
            .iter()
            .map(|spec| (spec.slot.as_str(), spec.output_dimension))
            .collect::<Vec<_>>()
    );
    assert_eq!(UTL_PLANNED_TOTAL_EMBEDDERS, 20);
    assert_eq!(specs.len(), LEARNER_EMBEDDER_COUNT);

    let dir = TempDir::new().unwrap();
    let learner_id = Uuid::from_u128(0x10101010_1010_4010_8010_101010101010);
    let session_ts = 1_700_010_000;
    let store = open_store(&dir);
    let before = counts(&store).await;
    println!("BEFORE counts={before:?}");
    assert_eq!(before, (0, 0, 0, 0));

    let fixture = synthetic_calibration_fixture();
    let sample = fixture
        .samples
        .iter()
        .find(|sample| sample.sample_id == "regulated-baseline")
        .expect("regulated sample exists");
    let outputs = embed_calibration_sample(sample).expect("embed all E14-E20 signals");
    println!(
        "TRIGGER fixture_id={} sample_id={} output_summary={:?}",
        fixture.fixture_id,
        sample.sample_id,
        outputs
            .iter()
            .map(|output| {
                (
                    output.slot.as_str(),
                    output.modality.as_str(),
                    output.vector.len(),
                    output.scalars.clone(),
                )
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(outputs.len(), LEARNER_EMBEDDER_COUNT);

    let state_vector = state_vector_from_outputs(
        learner_id,
        session_ts,
        &outputs,
        BTreeMap::from([
            ("fixture_id".into(), fixture.fixture_id.into()),
            ("sample_id".into(), sample.sample_id.into()),
        ]),
    )
    .expect("derive state vector from all E14-E20 outputs");

    let profile = LearnerProfile::new(
        learner_id,
        "all-e14-e20-fsv-learner".into(),
        "consented-local-first".into(),
        LearnerEmbedderSlot::all()
            .iter()
            .map(|slot| slot.modality())
            .collect::<BTreeSet<_>>(),
        Some(session_ts),
    )
    .unwrap();

    let mut envelopes = Vec::new();
    let mut modality_embeddings = Vec::new();
    for output in &outputs {
        let observation_id = Uuid::from_u128(
            0x20202020_2020_4020_8020_202020202000 + u128::from(output.slot.slot_number()),
        );
        envelopes.push(
            ObservationEnvelope::new(
                observation_id,
                learner_id,
                session_ts,
                output.modality,
                "consented-local-first".into(),
                sha256_bytes(format!("{}:{}", sample.sample_id, output.slot.as_str()).as_bytes()),
                "fsv-synthetic-preprocess-v1".into(),
                output.embedder_version.clone(),
                "thresholds-default-pending-calibration-v1".into(),
                Vec::new(),
            )
            .unwrap(),
        );
        modality_embeddings.push(output.to_modality_embedding(observation_id));
    }

    let fingerprint = LearnerFingerprint {
        learner_id,
        session_ts,
        observation_envelopes: envelopes,
        modality_embeddings,
        state_vector,
    };
    fingerprint.validate().unwrap();

    let delta_s = compute_delta_s_from_text(
        "spaced retrieval improves memory",
        "spaced retrieval improves long-term memory",
        Some("retrieval practice helps remember"),
        0.15,
        Some(0.7),
    )
    .unwrap();
    let delta_c = compute_delta_c(
        &[0.7, 0.76, 0.83],
        fingerprint.state_vector.components.hrv_coherence,
        0.81,
        0.02,
        None,
    )
    .unwrap();
    let delta_e = compute_delta_e(&fingerprint.state_vector.components).unwrap();
    let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None).unwrap();
    let delta_log = LearnerDeltaLog {
        learner_id,
        session_ts,
        provenance: ComputationEnvelope::new(
            Uuid::from_u128(0x30303030_3030_4030_8030_303030303030),
            learner_id,
            session_ts,
            fingerprint
                .observation_envelopes
                .iter()
                .map(|envelope| envelope.observation_id)
                .collect(),
            "thresholds-default-pending-calibration-v1".into(),
            sha256_json(&computation).unwrap(),
        )
        .unwrap(),
        computation,
    };
    let audit = LearnerAuditEntry {
        audit_id: Uuid::from_u128(0x40404040_4040_4040_8040_404040404040),
        learner_id,
        ts: session_ts,
        action: "fsv_all_e14_e20_learner_embedders".into(),
        target_cf: "fingerprints_learner".into(),
        result_sha256: sha256_json(&fingerprint).unwrap(),
        parent_audit_id: None,
    };

    store.store_learner_profile(&profile).await.unwrap();
    store.store_learner_fingerprint(&fingerprint).await.unwrap();
    store.store_learner_delta_log(&delta_log).await.unwrap();
    store.store_learner_audit_entry(&audit).await.unwrap();
    let after = counts(&store).await;
    println!("AFTER immediate counts={after:?}");
    assert_eq!(after, (1, 1, 1, 1));
    drop(store);

    let reopened = open_store(&dir);
    let readback_fingerprint = reopened
        .get_learner_fingerprint(learner_id, session_ts)
        .await
        .unwrap()
        .expect("fingerprint exists after reopen");
    let readback_state = reopened
        .get_learner_state_vector(learner_id, session_ts)
        .await
        .unwrap()
        .expect("state vector exists after reopen");
    let readback_delta = reopened
        .get_learner_delta_log(learner_id, session_ts)
        .await
        .unwrap()
        .expect("delta log exists after reopen");
    println!(
        "READBACK modality_embeddings={} observation_envelopes={} state_len={} components={:?} l={} diagnostic={}",
        readback_fingerprint.modality_embeddings.len(),
        readback_fingerprint.observation_envelopes.len(),
        readback_state.values.len(),
        readback_state.components,
        readback_delta.computation.l,
        readback_delta.computation.diagnostic_state.as_str()
    );
    assert_eq!(
        readback_fingerprint.modality_embeddings.len(),
        LEARNER_EMBEDDER_COUNT
    );
    assert_eq!(
        readback_fingerprint.observation_envelopes.len(),
        LEARNER_EMBEDDER_COUNT
    );
    assert_eq!(readback_state.values.len(), 4032);
    assert!(readback_delta.computation.l > 0.0);
}

#[tokio::test]
async fn edge_empty_affect_text_rejected_before_persistence() {
    println!("=== EDGE CASE: empty E16 affect-text input ===");
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let before = counts(&store).await;
    println!("BEFORE counts={before:?}");
    let input = LearnerEmbedderInput::Text { content: "".into() };
    let result = embed_learner_signal(LearnerEmbedderSlot::E16AffectText, &input);
    println!("ACTION empty text result={result:?}");
    assert!(result.is_err());
    let after = counts(&store).await;
    println!("AFTER counts={after:?}");
    assert_eq!(before, after);
}

#[tokio::test]
async fn edge_missing_one_slot_rejected_before_persistence() {
    println!("=== EDGE CASE: missing one E14-E20 output ===");
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let before = counts(&store).await;
    println!("BEFORE counts={before:?}");
    let fixture = synthetic_calibration_fixture();
    let sample = &fixture.samples[0];
    let mut outputs = embed_calibration_sample(sample).unwrap();
    let removed = outputs.pop().unwrap();
    let result = state_vector_from_outputs(
        Uuid::from_u128(0x50505050_5050_4050_8050_505050505050),
        1_700_010_100,
        &outputs,
        BTreeMap::new(),
    );
    println!(
        "ACTION removed_slot={} remaining_outputs={} result={result:?}",
        removed.slot.as_str(),
        outputs.len()
    );
    assert!(result.is_err());
    let after = counts(&store).await;
    println!("AFTER counts={after:?}");
    assert_eq!(before, after);
}

#[tokio::test]
async fn edge_nonfinite_eda_sample_rejected_before_persistence() {
    println!("=== EDGE CASE: nonfinite E18 EDA sample ===");
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let before = counts(&store).await;
    println!("BEFORE counts={before:?}");
    let input = LearnerEmbedderInput::Samples {
        modality: LearnerModality::Eda,
        samples: vec![0.1, f32::NAN, 0.2],
        sample_rate_hz: 4,
        channels: 1,
    };
    let result = embed_learner_signal(LearnerEmbedderSlot::E18Eda, &input);
    println!("ACTION nonfinite EDA result={result:?}");
    assert!(result.is_err());
    let after = counts(&store).await;
    println!("AFTER counts={after:?}");
    assert_eq!(before, after);
}
