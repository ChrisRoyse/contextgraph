// FROZEN 2026-05-19 per #408 — forensic continuity only; do not re-run, do not modify.
// Q4 ambiguous surface frozen as display-only. See docs/futurebuild/01_overview.md §1.1.
use context_graph_mejepa::q4_trust_gate::{q4_trust_gate_report_from_catalog_path, Q4HeadKind};
use context_graph_mejepa::AccuracyMetric;
use context_graph_mejepa_instruments::{
    extract_q4_accuracy_labels, write_q4_accuracy_raw_outputs, Q4AccuracyLabelStore,
    Q4AccuracyMetricKind, Q4AccuracySignalRecord,
};
use context_graph_mejepa_train::labels::q4_accuracy_labels_to_prediction_surfaces;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "support/task_py_g_048_q4_accuracy_label_fsv_support.rs"]
mod task_py_g_048_q4_accuracy_label_fsv_support;

use task_py_g_048_q4_accuracy_label_fsv_support::*;

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let task = env::var("CG_Q4_ACCURACY_FSV_TASK").unwrap_or_else(|_| "TASK-PY-G-048".to_string());
    let issue = env::var("CG_Q4_ACCURACY_FSV_ISSUE")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(267);
    let row_prefix =
        env::var("CG_Q4_ACCURACY_ROW_PREFIX").unwrap_or_else(|_| "task-py-g-048".to_string());
    let evidence_name = env::var("CG_Q4_ACCURACY_FSV_EVIDENCE_NAME")
        .unwrap_or_else(|_| "q4_accuracy_label_producer_fsv.json".to_string());
    let producer_fixture_rows = env_u64("CG_Q4_ACCURACY_PRODUCER_FIXTURE_ROWS", 100)?;
    let producer_rows_min = env_u64("CG_Q4_ACCURACY_PRODUCER_ROWS_MIN", 100)?;
    require(
        "producer fixture row count must be positive",
        producer_fixture_rows > 0,
    )?;
    let fsv_root = env_path(
        "CG_Q4_ACCURACY_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv",
    );
    let raw_root = env_path(
        "CG_Q4_ACCURACY_RAW_ROOT",
        "/zfs/archive/contextgraph/corpus/python-q4-accuracy-labels-v1",
    );
    let catalog_producer_fsv_root = env_path(
        "CG_Q4_ACCURACY_CATALOG_PRODUCER_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/",
    );
    let catalog_path =
        PathBuf::from("/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let rocksdb_path = fsv_root.join(format!("q4-accuracy-labels-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join(evidence_name);
    let row_id = format!("{row_prefix}-{run_id}");
    let fixture_root = fsv_root.join(format!("fixture-{run_id}"));
    let fixture_script = write_accuracy_fixture(&fixture_root)?;

    let source = happy_source(&row_id);
    let outputs = run_fixture_outputs(&fixture_root, &fixture_script, &row_id, "pre", "post")?;
    let raw_paths = write_q4_accuracy_raw_outputs(&raw_root, &row_id, &outputs)?;
    require("metrics raw output exists", raw_paths.metrics_json.exists())?;

    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    require(
        "happy path produced one label",
        extraction.labels.len() == 1,
    )?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    let regression = extraction
        .labels
        .iter()
        .find(|label| label.metric_name == "accuracy")
        .ok_or("missing accuracy label")?;
    require("happy path regression", regression.regression)?;
    require(
        "happy path delta is -8.79 pct",
        (regression.delta_pct + 8.791_208_791).abs() < 0.01,
    )?;

    let store = Q4AccuracyLabelStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let mut producer_rows = 0_u64;
    producer_rows += persist_extraction(&store, &extraction)? as u64;
    let mut raw_artifact_rows = 1_u64;
    for idx in 0..producer_fixture_rows.saturating_sub(1) {
        let row_source = happy_source(&format!("{row_prefix}-{run_id}-producer-{idx:02}"));
        let row_outputs = run_fixture_outputs(
            &fixture_root,
            &fixture_script,
            &row_source.corpus_row_id,
            "pre",
            "post",
        )?;
        write_q4_accuracy_raw_outputs(&raw_root, &row_source.corpus_row_id, &row_outputs)?;
        raw_artifact_rows += 1;
        let row_extraction = extract_q4_accuracy_labels(&row_source, &row_outputs)?;
        producer_rows += persist_extraction(&store, &row_extraction)? as u64;
    }
    store.flush()?;
    let after_count = store.count_records()?;
    require(
        "producer row minimum met",
        producer_rows >= producer_rows_min,
    )?;
    require(
        "store count moved by producer rows",
        after_count == before_count + producer_rows as usize,
    )?;
    drop(store);

    let reopened = Q4AccuracyLabelStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    let canonical_readback = reopened
        .get_label_record(&source.corpus_row_id, "accuracy")?
        .ok_or("missing canonical accuracy label readback")?;
    require(
        "reopened RocksDB sees producer rows",
        reopened_rows.len() == after_count,
    )?;
    require(
        "reopened rows are labels",
        reopened_rows
            .iter()
            .all(|(_, row)| matches!(row.signal, Q4AccuracySignalRecord::Label(_))),
    )?;
    require(
        "canonical row metric readback is label",
        matches!(canonical_readback.signal, Q4AccuracySignalRecord::Label(_)),
    )?;
    drop(reopened);

    let surfaces = q4_accuracy_labels_to_prediction_surfaces(&extraction.labels)?;
    require(
        "accuracy adapter emits sibling-pattern degradation surface",
        surfaces
            .predicted_accuracy_degradations
            .iter()
            .any(|item| item.metric == AccuracyMetric::Accuracy),
    )?;

    let edge_cases = vec![
        edge_no_metric_tests_empty_success()?,
        edge_free_form_metric_parse_failure()?,
        edge_unstable_seed_quarantined()?,
        edge_accuracy_fix_direction()?,
        edge_loss_metric_direction()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 accuracy edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let persisted_quarantine_extraction = persisted_quarantine_fixture()?;
    let quarantine_store = Q4AccuracyLabelStore::open(&rocksdb_path)?;
    let persisted_quarantine_keys =
        quarantine_store.put_extraction(&persisted_quarantine_extraction)?;
    quarantine_store.flush()?;
    drop(quarantine_store);
    let quarantine_reopened = Q4AccuracyLabelStore::open(&rocksdb_path)?;
    let quarantine_reopened_rows = quarantine_reopened.scan_records()?;
    let quarantine_reopened_count = quarantine_reopened_rows
        .iter()
        .filter(|(_, row)| matches!(row.signal, Q4AccuracySignalRecord::Quarantine(_)))
        .count();
    require(
        "quarantine rows survive source-test/detail-aware persistence",
        quarantine_reopened_count >= persisted_quarantine_extraction.quarantines.len(),
    )?;
    drop(quarantine_reopened);

    let catalog = update_q4_catalog(&catalog_path, producer_rows, &catalog_producer_fsv_root)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let accuracy_readiness = trust_report
        .head(Q4HeadKind::Accuracy)
        .ok_or("missing accuracy Q4 readiness")?;
    require(
        "accuracy producer is supported",
        accuracy_readiness.producer_supported,
    )?;
    require(
        "accuracy remains untrusted without #127 calibration",
        !accuracy_readiness.q4_head_ready,
    )?;

    let evidence = json!({
        "task": task,
        "issue": issue,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "real_fixture_root": fixture_root,
            "real_fixture_script": fixture_script,
            "rocksdb_path": rocksdb_path,
            "cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_ACCURACY_LABELS,
            "evidence_path": evidence_path,
            "q4_catalog_path": catalog_path,
            "q4_catalog_producer_fsv_root": catalog_producer_fsv_root,
            "fsv_root": fsv_root,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Compute deterministic pre/post accuracy metrics from real Python arrays, normalize deltas into Q4 accuracy labels, persist to CF, reopen RocksDB, and link producer evidence into the Q4 trust catalog.",
            "expected": {
                "metric": "accuracy",
                "delta_pct": -8.79,
                "regression": true,
                "producer_rows_min": producer_rows_min,
                "trusted_without_calibration": false,
            },
            "after_state": {
                "labels": extraction.labels,
                "quarantines": extraction.quarantines,
                "producer_rows": producer_rows,
                "raw_artifact_rows": raw_artifact_rows,
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "canonical_readback": canonical_readback,
                "raw_metrics_sha256": sha256_file(&raw_paths.metrics_json)?,
                "predicted_accuracy_degradations": surfaces.predicted_accuracy_degradations,
                "persisted_quarantine_keys": persisted_quarantine_keys,
                "persisted_quarantine_rows": persisted_quarantine_extraction.quarantines,
                "quarantine_reopened_count": quarantine_reopened_count,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "producer_fixture_rows": producer_fixture_rows,
            "direction_rule": "higher-is-better metrics regress on negative deltas; lower-is-better loss/error metrics regress on positive deltas",
            "unstable_seed_rule": "stddev above fixed-seed threshold quarantines instead of inventing a stable label",
            "trust_boundary": "producer rows alone do not unlock trusted Q4 accuracy; #127 calibration and separately verified slot evidence remain required",
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    require(
        "loss edge exercised lower-is-better semantics",
        edge_cases.iter().any(|case| {
            case.name == "loss_metric_direction"
                && case.detail["metric_kind"]
                    == serde_json::to_value(Q4AccuracyMetricKind::MeanSquaredError).unwrap()
        }),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn env_path(var: &str, default: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn env_u64(var: &str, default: u64) -> FsvResult<u64> {
    match env::var(var) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|err| format!("{var} must be a u64: {err}").into()),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(format!("{var} unreadable: {err}").into()),
    }
}
