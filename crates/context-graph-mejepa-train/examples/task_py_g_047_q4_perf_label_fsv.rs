use context_graph_mejepa::q4_trust_gate::{q4_trust_gate_report_from_catalog_path, Q4HeadKind};
use context_graph_mejepa::PerfAxis;
use context_graph_mejepa_instruments::{
    extract_q4_perf_labels, write_q4_perf_raw_outputs, Q4PerfLabelStore, Q4PerfSignalRecord,
};
use context_graph_mejepa_train::labels::q4_perf_labels_to_prediction_surfaces;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "task_py_g_047_q4_perf_label_fsv_support.rs"]
mod task_py_g_047_q4_perf_label_fsv_support;

use task_py_g_047_q4_perf_label_fsv_support::*;

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let task = env::var("CG_Q4_PERF_FSV_TASK").unwrap_or_else(|_| "TASK-PY-G-047".to_string());
    let issue = env::var("CG_Q4_PERF_FSV_ISSUE")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(266);
    let row_prefix =
        env::var("CG_Q4_PERF_ROW_PREFIX").unwrap_or_else(|_| "task-py-g-047".to_string());
    let evidence_name = env::var("CG_Q4_PERF_FSV_EVIDENCE_NAME")
        .unwrap_or_else(|_| "q4_perf_label_producer_fsv.json".to_string());
    let producer_fixture_rows = env_u64("CG_Q4_PERF_PRODUCER_FIXTURE_ROWS", 60)?;
    let producer_rows_min = env_u64("CG_Q4_PERF_PRODUCER_ROWS_MIN", 100)?;
    require(
        "producer fixture row count must be positive",
        producer_fixture_rows > 0,
    )?;
    let fsv_root = env_path(
        "CG_Q4_PERF_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv",
    );
    let raw_root = env_path(
        "CG_Q4_PERF_RAW_ROOT",
        "/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1",
    );
    let catalog_producer_fsv_root = env_path(
        "CG_Q4_PERF_CATALOG_PRODUCER_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/",
    );
    let catalog_path =
        PathBuf::from("/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let rocksdb_path = fsv_root.join(format!("q4-perf-labels-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join(evidence_name);
    let row_id = format!("{row_prefix}-{run_id}");
    let fixture_root = fsv_root.join(format!("fixture-{run_id}"));
    let fixture_script = write_perf_fixture(&fixture_root)?;
    let profile_pre = profile_path(&fixture_root, &row_id, "pre");
    let profile_post = profile_path(&fixture_root, &row_id, "post");

    let source = happy_source(&row_id);
    let outputs = run_fixture_outputs(&fixture_root, &fixture_script, &row_id)?;
    let raw_paths = write_q4_perf_raw_outputs(&raw_root, &row_id, &outputs)?;
    require(
        "benchmark raw output exists",
        raw_paths.benchmark_json.exists(),
    )?;
    require(
        "cprofile raw output exists",
        raw_paths.cprofile_json.exists(),
    )?;
    require("real pre cProfile artifact exists", profile_pre.exists())?;
    require("real post cProfile artifact exists", profile_post.exists())?;

    let extraction = extract_q4_perf_labels(&source, &outputs)?;
    require("happy path produced labels", !extraction.labels.is_empty())?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    let regression = extraction
        .labels
        .iter()
        .find(|label| label.metric == "test_linear")
        .ok_or("missing test_linear perf label")?;
    require("happy path regression", regression.regression)?;
    require(
        "happy path delta exceeds 25 pct",
        regression.delta_pct.unwrap_or_default() > 25.0,
    )?;

    let store = Q4PerfLabelStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let mut producer_rows = 0_u64;
    let mut stored_signal_rows = 0_u64;
    producer_rows += extraction.labels.len() as u64;
    stored_signal_rows += persist_extraction(&store, &extraction)? as u64;
    let mut raw_artifact_rows = 1_u64;
    for idx in 0..producer_fixture_rows.saturating_sub(1) {
        let row_source = happy_source(&format!("{row_prefix}-{run_id}-producer-{idx:02}"));
        let row_outputs =
            run_fixture_outputs(&fixture_root, &fixture_script, &row_source.corpus_row_id)?;
        write_q4_perf_raw_outputs(&raw_root, &row_source.corpus_row_id, &row_outputs)?;
        raw_artifact_rows += 1;
        let row_extraction = extract_q4_perf_labels(&row_source, &row_outputs)?;
        producer_rows += row_extraction.labels.len() as u64;
        stored_signal_rows += persist_extraction(&store, &row_extraction)? as u64;
    }
    store.flush()?;
    let after_count = store.count_records()?;
    require(
        "producer row minimum met",
        producer_rows >= producer_rows_min,
    )?;
    require(
        "store count moved by stored signal rows",
        after_count == before_count + stored_signal_rows as usize,
    )?;
    drop(store);
    let reopened = Q4PerfLabelStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    let canonical_readback = reopened
        .get_label_record(&source.corpus_row_id, "test_linear")?
        .ok_or("missing canonical test_linear label readback")?;
    require(
        "reopened RocksDB sees producer rows",
        reopened_rows.len() == after_count,
    )?;
    let reopened_label_count = reopened_rows
        .iter()
        .filter(|(_, row)| matches!(row.signal, Q4PerfSignalRecord::Label(_)))
        .count();
    let reopened_quarantine_count = reopened_rows
        .iter()
        .filter(|(_, row)| matches!(row.signal, Q4PerfSignalRecord::Quarantine(_)))
        .count();
    require(
        "reopened labels meet usable producer rows",
        reopened_label_count >= producer_rows as usize,
    )?;
    require(
        "canonical row metric readback is label",
        matches!(canonical_readback.signal, Q4PerfSignalRecord::Label(_)),
    )?;
    drop(reopened);

    let surfaces = q4_perf_labels_to_prediction_surfaces(&extraction.labels)?;
    require(
        "perf adapter emits sibling-pattern regression surface",
        surfaces
            .predicted_perf_regressions
            .iter()
            .any(|item| item.axis == PerfAxis::WallclockMs),
    )?;

    let edge_cases = vec![
        edge_unstable_benchmark_quarantined()?,
        edge_no_timing_tests_empty_success()?,
        edge_profiler_budget_exceeded()?,
        edge_negative_delta_improvement()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 perf edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let catalog = update_q4_catalog(&catalog_path, producer_rows, &catalog_producer_fsv_root)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let perf_readiness = trust_report
        .head(Q4HeadKind::Perf)
        .ok_or("missing perf Q4 readiness")?;
    require(
        "perf producer is supported",
        perf_readiness.producer_supported,
    )?;
    require(
        "perf catalog producer root stays canonical",
        perf_readiness.producer_fsv_root == canonical_fsv_root(&catalog_producer_fsv_root),
    )?;
    require(
        "perf per-slot evidence is not synthesized",
        !perf_readiness.per_slot_supported,
    )?;
    require(
        "perf remains untrusted without #127 calibration",
        !perf_readiness.q4_head_ready,
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
            "real_cprofile_artifacts": {
                "pre": profile_pre,
                "post": profile_post,
                "pre_sha256": sha256_file(&profile_pre)?,
                "post_sha256": sha256_file(&profile_post)?,
            },
            "rocksdb_path": rocksdb_path,
            "cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_PERF_LABELS,
            "evidence_path": evidence_path,
            "q4_catalog_path": catalog_path,
            "q4_catalog_producer_fsv_root": catalog_producer_fsv_root,
            "fsv_root": fsv_root,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Normalize pytest-benchmark/cProfile pre-post measurements into Q4 perf labels, persist to CF, reopen RocksDB, and link producer evidence into the Q4 trust catalog.",
            "expected": {
                "metric": "test_linear",
                "delta_pct_gt": 25.0,
                "regression": true,
                "producer_rows_min": producer_rows_min,
                "trusted_without_calibration": false,
            },
            "after_state": {
                "labels": extraction.labels,
                "quarantines": extraction.quarantines,
                "producer_rows": producer_rows,
                "stored_signal_rows": stored_signal_rows,
                "raw_artifact_rows": raw_artifact_rows,
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "reopened_label_count": reopened_label_count,
                "reopened_quarantine_count": reopened_quarantine_count,
                "canonical_readback": canonical_readback,
                "raw_benchmark_sha256": sha256_file(&raw_paths.benchmark_json)?,
                "raw_cprofile_sha256": sha256_file(&raw_paths.cprofile_json)?,
                "predicted_perf_regressions": surfaces.predicted_perf_regressions,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "producer_fixture_rows": producer_fixture_rows,
            "benchmark_instability_rule": "stddev_ns / mean_ns > 0.15 quarantines the metric and emits no false regression label",
            "budget_rule": "profiler timeout is a WallclockBudgetExceeded signal with no fabricated baseline/after/delta",
            "trust_boundary": "producer rows alone do not unlock trusted Q4 perf; #127 calibration and separately verified slot evidence remain required",
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
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
