use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    calibrate_and_persist_q5_reality_impact, count_cf, open_infer_rocksdb,
    read_all_q5_calibration_records, read_all_reality_impact_records, write_reality_impact_record,
    Language, Q5CalibrationFlag, Q5CalibrationInput, Q5ReplayExclusion,
    RealityImpactClassification, RealityImpactItemKind, RealityImpactMatch, RealityImpactMiss,
    RealityImpactObservedItem, RealityImpactPredictedItem, RealityImpactRecord, TestOutcome,
    Q5_HIGH_SURPRISE_RATE_THRESHOLD, REALITY_IMPACT_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{CF_MEJEPA_Q5_CALIBRATIONS, CF_MEJEPA_REALITY_IMPACT};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const FSV_SUBDIR: &str = "task-py-g-013-q5-calibration-fsv";
const REPORT_NAME: &str = "q5_calibration_fsv.json";
const READBACK_NAME: &str = "independent_rocksdb_readback.json";
const SAMPLE_NAME: &str = "q5_calibration_samples.json";

fn main() -> Result<()> {
    let root = fsv_root();
    if root.exists() {
        fs::remove_dir_all(&root).context("clear previous FSV root")?;
    }
    fs::create_dir_all(&root).context("create FSV root")?;
    let db_path = root.join("q5-calibration.rocksdb");
    let db = open_infer_rocksdb(&db_path).context("open FSV RocksDB")?;
    let before = source_counts(db.as_ref())?;

    let rows = vec![
        impact_record(
            "happy-a",
            vec![matched(RealityImpactItemKind::FileChange, "src/a.py")],
            vec![matched(
                RealityImpactItemKind::FailedTest,
                "tests/test_a.py::test_a",
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        impact_record(
            "happy-b",
            vec![matched(RealityImpactItemKind::FileChange, "src/b.py")],
            vec![matched(
                RealityImpactItemKind::FailedTest,
                "tests/test_b.py::test_b",
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        impact_record(
            "one-missed-test",
            Vec::new(),
            Vec::new(),
            vec![missed(
                RealityImpactItemKind::FailedTest,
                "tests/test_c.py::test_c",
                "TEST_NEVER_RAN",
            )],
            Vec::new(),
            Vec::new(),
        ),
        impact_record(
            "awaiting-oracle",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![missed(
                RealityImpactItemKind::FailedTest,
                "tests/test_later.py::test_later",
                "REPLAY_WINDOW_EMPTY",
            )],
            Vec::new(),
        ),
        impact_record(
            "zero-prediction-surprise",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![surprise(
                RealityImpactItemKind::FileChange,
                "src/unpredicted.py",
            )],
        ),
        impact_record(
            "dead-code-retrain",
            Vec::new(),
            Vec::new(),
            vec![missed(
                RealityImpactItemKind::DeadCode,
                "src/dead.py",
                "DEAD_CODE_NEVER_OBSERVED",
            )],
            Vec::new(),
            Vec::new(),
        ),
    ];

    for row in &rows {
        write_reality_impact_record(db.as_ref(), row)?;
    }
    let persisted = read_all_reality_impact_records(db.as_ref())?;
    let inputs = persisted
        .into_iter()
        .map(|record| {
            let mutation_category = if record.prediction_id == id16("zero-prediction-surprise") {
                "surprise_cell"
            } else if record.prediction_id == id16("dead-code-retrain") {
                "dead_code_cell"
            } else {
                "bool_flip"
            };
            Q5CalibrationInput {
                mutation_category: mutation_category.to_string(),
                language: Language::Python,
                record,
            }
        })
        .collect::<Vec<_>>();
    let exclusions = vec![Q5ReplayExclusion {
        mutation_category: "bool_flip".to_string(),
        language: Language::Python,
        prediction_id: id16("chain-broken"),
        side_effect_kind: RealityImpactItemKind::FailedTest,
        reason: "SHIFT_LOG_CHAIN_BROKEN".to_string(),
    }];
    let records = calibrate_and_persist_q5_reality_impact(
        db.as_ref(),
        "task-py-g-013-fsv",
        inputs,
        exclusions,
        1_779_160_000_000,
    )?;
    let after = source_counts(db.as_ref())?;
    let failed_test = find_record(&records, "bool_flip", RealityImpactItemKind::FailedTest);
    let file_surprise = find_record(&records, "surprise_cell", RealityImpactItemKind::FileChange);
    let awaiting = find_record(&records, "bool_flip", RealityImpactItemKind::FailedTest);
    let dead_code = find_record(&records, "dead_code_cell", RealityImpactItemKind::DeadCode);

    let happy_pass = failed_test.true_positives == 2
        && failed_test.false_negatives == 1
        && failed_test.f1 >= 0.70
        && failed_test
            .flags
            .contains(&Q5CalibrationFlag::ReplayExcluded);
    let awaiting_pass = awaiting.awaiting_oracle == 1
        && awaiting.flags.contains(&Q5CalibrationFlag::AwaitingOracle);
    let high_surprise_pass = file_surprise.surprise_rate > Q5_HIGH_SURPRISE_RATE_THRESHOLD
        && file_surprise
            .flags
            .contains(&Q5CalibrationFlag::Q5HighSurpriseRate);
    let zero_prediction_pass =
        file_surprise.true_positives == 0 && file_surprise.false_positives == 1;
    let retrain_pass = dead_code
        .flags
        .contains(&Q5CalibrationFlag::Q5CellNeedsRetraining);

    ensure!(
        happy_pass && awaiting_pass && high_surprise_pass && zero_prediction_pass && retrain_pass,
        "one or more TASK-PY-G-013 checks failed"
    );
    let samples = json!({
        "truePositiveSample": hex::encode(id16("happy-a")),
        "falseNegativeSample": hex::encode(id16("one-missed-test")),
        "surpriseSample": hex::encode(id16("zero-prediction-surprise")),
        "excludedReplaySample": hex::encode(id16("chain-broken"))
    });
    write_json_0600(&root.join(SAMPLE_NAME), &samples)?;
    let report = json!({
        "task": "TASK-PY-G-013",
        "status": "pass",
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "realityImpactCf": CF_MEJEPA_REALITY_IMPACT,
            "q5CalibrationCf": CF_MEJEPA_Q5_CALIBRATIONS
        },
        "before": before,
        "after": after,
        "checks": {
            "happyDominantClassesF1AtLeast70": happy_pass,
            "awaitingOracleExcludedFromDenominators": awaiting_pass,
            "highSurpriseFlag": high_surprise_pass,
            "zeroPredictionStillScored": zero_prediction_pass,
            "cellNeedsRetrainingFlag": retrain_pass
        },
        "records": records
    });
    write_json_0600(&root.join(REPORT_NAME), &report)?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen FSV RocksDB")?;
    let readback_rows = read_all_q5_calibration_records(reopened.as_ref())?;
    let independent = json!({
        "status": "pass",
        "dbPath": db_path.display().to_string(),
        "realityImpactRows": count_cf(reopened.as_ref(), CF_MEJEPA_REALITY_IMPACT)?,
        "q5CalibrationRows": count_cf(reopened.as_ref(), CF_MEJEPA_Q5_CALIBRATIONS)?,
        "records": readback_rows
    });
    write_json_0600(&root.join(READBACK_NAME), &independent)?;
    println!("{}", serde_json::to_string_pretty(&independent)?);
    Ok(())
}

fn find_record<'a>(
    records: &'a [context_graph_mejepa::Q5CalibrationRecord],
    mutation_category: &str,
    kind: RealityImpactItemKind,
) -> &'a context_graph_mejepa::Q5CalibrationRecord {
    records
        .iter()
        .find(|record| {
            record.cell_key.mutation_category == mutation_category
                && record.cell_key.side_effect_kind == kind
        })
        .expect("calibration record exists")
}

fn source_counts(db: &rocksdb::DB) -> Result<Value> {
    Ok(json!({
        CF_MEJEPA_REALITY_IMPACT: count_cf(db, CF_MEJEPA_REALITY_IMPACT)?,
        CF_MEJEPA_Q5_CALIBRATIONS: count_cf(db, CF_MEJEPA_Q5_CALIBRATIONS)?
    }))
}

fn impact_record(
    seed: &str,
    file_matches: Vec<RealityImpactMatch>,
    test_matches: Vec<RealityImpactMatch>,
    missed: Vec<RealityImpactMiss>,
    not_yet_observed: Vec<RealityImpactMiss>,
    surprises: Vec<context_graph_mejepa::RealityImpactSurprise>,
) -> RealityImpactRecord {
    let mut matched = file_matches;
    matched.extend(test_matches);
    RealityImpactRecord {
        schema_version: REALITY_IMPACT_SCHEMA_VERSION,
        prediction_id: id16(seed),
        session_id: id16(&format!("{seed}-session")),
        window_start_unix_ms: 1_000,
        window_end_unix_ms: 3_000,
        source_shift_log_path: PathBuf::from(format!("cgreality-shift-log/{seed}.jsonl")),
        source_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_impact_cf: CF_MEJEPA_REALITY_IMPACT.to_string(),
        matched,
        missed,
        not_yet_observed,
        surprises,
        shift_count: 1,
        observed_test_count: 1,
        created_at_unix_ms: 3_100,
    }
}

fn matched(kind: RealityImpactItemKind, target: &str) -> RealityImpactMatch {
    RealityImpactMatch {
        predicted: predicted(kind, target),
        observed: observed(kind, target),
        classification: RealityImpactClassification::Confirmed,
    }
}

fn missed(kind: RealityImpactItemKind, target: &str, reason: &str) -> RealityImpactMiss {
    RealityImpactMiss {
        predicted: predicted(kind, target),
        classification: if reason == "REPLAY_WINDOW_EMPTY" {
            RealityImpactClassification::NotYetObserved
        } else {
            RealityImpactClassification::Missed
        },
        reason: reason.to_string(),
    }
}

fn surprise(
    kind: RealityImpactItemKind,
    target: &str,
) -> context_graph_mejepa::RealityImpactSurprise {
    context_graph_mejepa::RealityImpactSurprise {
        observed: observed(kind, target),
        classification: RealityImpactClassification::Surprise,
        reason: "UNPREDICTED_REALITY_SHIFT".to_string(),
    }
}

fn predicted(kind: RealityImpactItemKind, target: &str) -> RealityImpactPredictedItem {
    RealityImpactPredictedItem {
        item_id: format!("{kind:?}:{target}"),
        kind,
        target: target.to_string(),
        detail: "synthetic q5 calibration fixture".to_string(),
    }
}

fn observed(kind: RealityImpactItemKind, target: &str) -> RealityImpactObservedItem {
    RealityImpactObservedItem {
        shift_id: format!("shift-{}", hex::encode(id16(target))),
        kind,
        target: target.to_string(),
        outcome: matches!(kind, RealityImpactItemKind::FailedTest).then_some(TestOutcome::Fail),
        file: matches!(kind, RealityImpactItemKind::FileChange).then(|| PathBuf::from(target)),
        timestamp_unix_ms: 2_000,
    }
}

fn fsv_root() -> PathBuf {
    std::env::var("CG_MEJEPA_TASK_PY_G_013_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/contextgraph").join(FSV_SUBDIR))
}

fn id16(seed: &str) -> [u8; 16] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}
