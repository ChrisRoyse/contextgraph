use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelBuilder};
use tempfile::TempDir;

use crate::*;

fn versions() -> BTreeMap<EmbedderId, [u8; 32]> {
    EmbedderId::all()
        .into_iter()
        .enumerate()
        .map(|(idx, embedder)| (embedder, [idx as u8 + 1; 32]))
        .collect()
}

fn base_panel() -> Panel {
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let vector = (0..slot.dim())
            .map(|idx| {
                let phase = (slot.offset() + idx + 1) as f32 * 0.013;
                phase.sin() * 0.25 + phase.cos() * 0.05 + 0.01
            })
            .collect::<Vec<_>>();
        builder.set_slot(slot, &vector).unwrap();
    }
    builder.require_slots(&InstrumentSlot::all()).unwrap();
    builder.build().unwrap()
}

fn panel_with_negated_slot(slot: InstrumentSlot) -> Panel {
    let source = base_panel();
    let mut builder = PanelBuilder::new();
    for current in InstrumentSlot::all() {
        let mut vector = source.slot(current).to_vec();
        if current == slot {
            for value in &mut vector {
                *value = -*value;
            }
        }
        builder.set_slot(current, &vector).unwrap();
    }
    builder.require_slots(&InstrumentSlot::all()).unwrap();
    builder.build().unwrap()
}

fn chunk_id(entity_type: EntityType) -> ChunkId {
    ChunkId::try_new([7u8; 32], Language::Python, entity_type, 10, 20, [9u8; 16]).unwrap()
}

fn build_with(
    thresholds: Thresholds,
    sample_count: usize,
) -> (TctConstellation, Vec<ShrinkageDecision>) {
    let mut builder = ConstellationBuilder::new([3u8; 32], versions(), "a".repeat(40)).unwrap();
    let panel = base_panel();
    let chunk = chunk_id(EntityType::Function);
    for _ in 0..sample_count {
        builder
            .ingest_corpus_entry(
                &panel,
                MutationCategory::KnownGood,
                OracleOutcome::Pass,
                Language::Python,
                EntityType::Function,
                &[(chunk.clone(), panel.clone())],
            )
            .unwrap();
    }
    builder.finalize(thresholds, SystemTime::now()).unwrap()
}

fn threshold(value: f32) -> Thresholds {
    Thresholds::try_new(
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, value))
            .collect(),
        BTreeMap::new(),
    )
    .unwrap()
}

fn calibrated_constellation() -> (
    TctConstellation,
    Vec<ShrinkageDecision>,
    Vec<CalibrationDecision>,
) {
    let (draft, _decisions) = build_with(threshold(0.0), 50);
    let validation = HeldOutValidation {
        knowngood_samples: (0..50)
            .map(|_| HeldOutSample {
                language: Language::Python,
                entity_type: EntityType::Function,
                mutation: MutationCategory::KnownGood,
                panel: base_panel(),
            })
            .collect(),
    };
    let (thresholds, calibration) = calibrate(&draft, &validation).unwrap();
    let (constellation, decisions) = build_with(thresholds, 50);
    (constellation, decisions, calibration)
}

#[test]
fn public_type_validation_rejects_bad_inputs() {
    assert_eq!(MutationCategory::all().len(), 8);
    assert_eq!(Language::all().len(), 11);
    assert_eq!(EntityType::all().len(), 13);
    assert_eq!(EmbedderId::all().len(), 21);
    assert_eq!(
        ChunkId::try_new(
            [0; 32],
            Language::Python,
            EntityType::Function,
            9,
            8,
            [0; 16]
        )
        .unwrap_err()
        .code(),
        "MEJEPA_TCT_INVALID_INPUT"
    );
    assert_eq!(
        GtauViolation::try_new(
            EmbedderId::E1,
            f32::NAN,
            0.5,
            None,
            ShrinkageOrigin::OwnCell
        )
        .unwrap_err()
        .code(),
        "MEJEPA_INFER_NAN_DETECTED"
    );
}

#[test]
fn calibration_fails_closed_on_under_sampled_validation() {
    let (draft, _decisions) = build_with(threshold(0.0), 50);
    let validation = HeldOutValidation {
        knowngood_samples: (0..29)
            .map(|_| HeldOutSample {
                language: Language::Python,
                entity_type: EntityType::Function,
                mutation: MutationCategory::KnownGood,
                panel: base_panel(),
            })
            .collect(),
    };
    assert_eq!(
        calibrate(&draft, &validation).unwrap_err().code(),
        "MEJEPA_TCT_INSUFFICIENT_SAMPLES"
    );
}

#[test]
fn gtau_accepts_base_panel_and_rejects_negated_slot() {
    let (constellation, _decisions, _calibration) = calibrated_constellation();
    let accepted = gtau_check(
        &base_panel(),
        MutationCategory::KnownGood,
        Language::Python,
        EntityType::Function,
        &constellation,
    )
    .unwrap();
    assert!(accepted.gtau_satisfied);
    assert_eq!(accepted.evaluated_embedder_count, 21);

    let rejected = gtau_check(
        &panel_with_negated_slot(panel_slot_for_embedder(EmbedderId::E7)),
        MutationCategory::KnownGood,
        Language::Python,
        EntityType::Function,
        &constellation,
    )
    .unwrap();
    assert!(!rejected.gtau_satisfied);
    assert!(rejected
        .violations
        .iter()
        .any(|violation| violation.embedder == EmbedderId::E7));
}

#[test]
fn chunk_gtau_localizes_the_single_failing_chunk() {
    let (constellation, _decisions, _calibration) = calibrated_constellation();
    let good = chunk_id(EntityType::Function);
    let failing = ChunkId::try_new(
        [8u8; 32],
        Language::Python,
        EntityType::Function,
        30,
        40,
        [10u8; 16],
    )
    .unwrap();
    let output = gtau_check_chunks(
        &[
            (good.clone(), base_panel()),
            (
                failing.clone(),
                panel_with_negated_slot(panel_slot_for_embedder(EmbedderId::E7)),
            ),
        ],
        MutationCategory::KnownGood,
        &constellation,
    )
    .unwrap();
    assert!(!output.aggregate_satisfied);
    assert_eq!(output.violating_chunks, vec![failing]);
}

#[test]
fn store_persists_and_reopens_source_of_truth() {
    let (constellation, _decisions, _calibration) = calibrated_constellation();
    let temp = TempDir::new().unwrap();
    let db = open_tct_rocksdb(temp.path()).unwrap();
    let store = ConstellationStore::new(db.clone()).unwrap();
    let version = store.persist(&constellation).unwrap();
    drop(store);
    drop(db);

    let db = open_tct_rocksdb(temp.path()).unwrap();
    let store = ConstellationStore::new(db).unwrap();
    assert_eq!(store.count_constellations().unwrap(), 1);
    let loaded = store
        .load(version, &constellation.corpus_provenance.embedder_versions)
        .unwrap();
    assert_eq!(loaded.version_id(), constellation.version_id());
    assert_eq!(store.read_raw_by_version(version).unwrap().len(), 1);
}

#[test]
fn freshness_stale_state_fails_closed() {
    let (mut constellation, _decisions, _calibration) = calibrated_constellation();
    constellation.frozen_at = SystemTime::now() - Duration::from_secs(91 * 86_400);
    assert_eq!(
        constellation.check_freshness(90, false).unwrap_err().code(),
        "MEJEPA_INFER_GTAU_STALE_CONSTELLATION"
    );
}

#[test]
fn violation_rate_uses_persisted_guard_decision_rows() {
    let temp = TempDir::new().unwrap();
    let db = open_tct_rocksdb(temp.path()).unwrap();
    let aggregator = ViolationRateAggregator::new(db).unwrap();
    let now = SystemTime::now();
    let predictor =
        PredictorOutput::try_new(MutationCategory::KnownGood, vec![0.9], (0.8, 0.95), 1.0).unwrap();
    let violation =
        GtauViolation::try_new(EmbedderId::E1, 0.1, 0.8, None, ShrinkageOrigin::OwnCell).unwrap();
    let rejected_payload =
        VerdictGuardRejected::try_new(vec![violation], predictor, Vec::new(), [1u8; 32]).unwrap();
    for idx in 0..100 {
        let timestamp = now - Duration::from_secs((100 - idx) as u64);
        let record = if idx < 25 {
            VerdictRecord::GuardRejected(VerdictGuardRejectedSummary {
                timestamp,
                payload: rejected_payload.clone(),
            })
        } else {
            VerdictRecord::Approve(VerdictApproveSummary {
                timestamp,
                constellation_version_id: [1u8; 32],
            })
        };
        aggregator.record_verdict(&record).unwrap();
    }
    assert_eq!(aggregator.count_rows().unwrap(), 100);
    let rate = aggregator
        .constellation_violation_rate(RollingWindow::try_new(1000, 24).unwrap(), now)
        .unwrap()
        .unwrap();
    assert!((rate - 0.25).abs() < 1.0e-6, "rate={rate}");
}

#[test]
fn cosine_similarity_rejects_zero_norm_vectors() {
    let err = crate::gtau::cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INFER_CONSTELLATION_VIOLATION");
}
