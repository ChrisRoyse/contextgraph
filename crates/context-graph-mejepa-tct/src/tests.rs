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

fn refresh_report_for_test(
    constellation: &TctConstellation,
    decisions: &[ShrinkageDecision],
) -> ConstellationRefreshReport {
    let source_row_count = 50;
    let source_chunk_count = 50;
    let cell_support = constellation
        .per_chunk_type_centroids
        .iter()
        .map(|((mutation, entity_type, language, embedder), centroid)| {
            CellSupportRecord::try_new(
                *mutation,
                *entity_type,
                *language,
                *embedder,
                centroid.sample_count,
                centroid.sample_count,
                centroid.origin,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut shrinkage = RefreshShrinkageSummary {
        total_cells: decisions.len(),
        own_cell: 0,
        language_aggregate: 0,
        entity_aggregate: 0,
        category_aggregate: 0,
    };
    for decision in decisions {
        match decision.origin {
            ShrinkageOrigin::OwnCell => shrinkage.own_cell += 1,
            ShrinkageOrigin::LanguageAggregate => shrinkage.language_aggregate += 1,
            ShrinkageOrigin::EntityAggregate => shrinkage.entity_aggregate += 1,
            ShrinkageOrigin::CategoryAggregate => shrinkage.category_aggregate += 1,
        }
    }
    let panel_level_cell_count = constellation
        .per_category_centroids
        .values()
        .map(BTreeMap::len)
        .sum::<usize>()
        + constellation
            .per_language_centroids
            .values()
            .map(BTreeMap::len)
            .sum::<usize>()
        + constellation
            .outcome_centroids
            .values()
            .map(BTreeMap::len)
            .sum::<usize>();
    let operator_diagnostics = OperatorDiagnosticSummary {
        inspected_chunk_count: 2,
        rejected_chunk_count: 1,
        violating_embedder_count: 1,
        worst_margin: -0.25,
    };
    let reward_signal_summary = RefreshRewardSignalSummary {
        source_row_count,
        source_chunk_count,
        mutation_category_count: 1,
        language_count: 1,
        oracle_outcome_count: 1,
        per_chunk_type_cell_count: cell_support.len(),
        panel_level_cell_count,
        strict_guard_rejection_count: operator_diagnostics.rejected_chunk_count,
        violating_chunk_count: operator_diagnostics.rejected_chunk_count,
        estimated_reward_scalar_count: source_chunk_count * (EmbedderId::all().len() + 2)
            + cell_support.len()
            + panel_level_cell_count,
    };
    ConstellationRefreshReport::try_new(ConstellationRefreshReportInput {
        started_at: constellation.frozen_at,
        finished_at: constellation.frozen_at + Duration::from_secs(1),
        constellation_version_id: constellation.version_id(),
        corpus_sha: constellation.corpus_provenance.corpus_sha,
        code_version: constellation.corpus_provenance.code_version.clone(),
        source_corpus_path: "/zfs/archive/contextgraph/corpus/test-index.json".to_string(),
        source_corpus_sha256: [2u8; 32],
        source_row_count,
        source_chunk_count,
        ingested_panel_count: source_row_count + source_chunk_count,
        per_entity_support: vec![EntitySupportRecord::try_new(
            EntityType::Function,
            source_chunk_count,
        )
        .unwrap()],
        per_category_support: vec![CategorySupportRecord::try_new(
            MutationCategory::KnownGood,
            source_row_count,
            source_chunk_count,
        )
        .unwrap()],
        per_language_support: vec![LanguageSupportRecord::try_new(
            Language::Python,
            source_row_count,
            source_chunk_count,
        )
        .unwrap()],
        per_oracle_outcome_support: vec![OracleOutcomeSupportRecord::try_new(
            OracleOutcome::Pass,
            source_row_count,
        )
        .unwrap()],
        cell_support,
        shrinkage,
        operator_diagnostics,
        reward_signal_summary,
    })
    .unwrap()
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
fn refresh_report_persists_reopens_and_captures_reward_support() {
    let (constellation, decisions, _calibration) = calibrated_constellation();
    let report = refresh_report_for_test(&constellation, &decisions);
    let temp = TempDir::new().unwrap();
    let db = open_tct_rocksdb(temp.path()).unwrap();
    let store = ConstellationStore::new(db.clone()).unwrap();
    assert_eq!(store.count_refresh_reports().unwrap(), 0);
    let version = store.persist(&constellation).unwrap();
    let report_id = store.persist_refresh_report(&report).unwrap();
    assert_eq!(version, constellation.version_id());
    assert_eq!(report_id, report.report_id);
    assert_eq!(store.count_constellations().unwrap(), 1);
    assert_eq!(store.count_refresh_reports().unwrap(), 1);
    drop(store);
    drop(db);

    let reopened = ConstellationStore::new(open_tct_rocksdb(temp.path()).unwrap()).unwrap();
    let loaded = reopened.load_refresh_report(report_id).unwrap();
    assert_eq!(loaded, report);
    assert_eq!(reopened.latest_refresh_report().unwrap(), report);
    assert_eq!(
        reopened
            .refresh_reports_for_constellation(version, 1)
            .unwrap(),
        vec![report.clone()]
    );
    assert_eq!(loaded.per_category_support.len(), 1);
    assert_eq!(loaded.per_language_support.len(), 1);
    assert_eq!(loaded.per_oracle_outcome_support.len(), 1);
    assert_eq!(
        loaded.reward_signal_summary.per_chunk_type_cell_count,
        loaded.cell_support.len()
    );
}

#[test]
fn refresh_report_validation_fails_closed_on_reward_signal_mismatches() {
    let (constellation, decisions, _calibration) = calibrated_constellation();
    let report = refresh_report_for_test(&constellation, &decisions);

    let mut mismatched_category = report.clone();
    mismatched_category.per_category_support[0].row_count += 1;
    assert_eq!(
        mismatched_category.validate_integrity().unwrap_err().code(),
        "MEJEPA_TCT_INVALID_INPUT"
    );

    let mut duplicate_cell = report.clone();
    duplicate_cell
        .cell_support
        .push(duplicate_cell.cell_support[0].clone());
    assert_eq!(
        duplicate_cell.validate_integrity().unwrap_err().code(),
        "MEJEPA_TCT_INVALID_INPUT"
    );

    let mut tampered_summary = report;
    tampered_summary
        .reward_signal_summary
        .estimated_reward_scalar_count = 1;
    assert_eq!(
        tampered_summary.validate_integrity().unwrap_err().code(),
        "MEJEPA_TCT_INVALID_INPUT"
    );
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
