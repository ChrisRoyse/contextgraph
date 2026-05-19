use context_graph_mejepa::q4_trust_gate::{q4_trust_gate_report_from_catalog_path, Q4HeadKind};
use context_graph_mejepa::PerfAxis;
use context_graph_mejepa_instruments::{
    extract_q4_perf_labels, write_q4_perf_raw_outputs, Q4PerfLabelStore, Q4PerfSignalRecord,
};
use context_graph_mejepa_train::labels::q4_perf_labels_to_prediction_surfaces;
use serde_json::{json, Value};
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
    let fsv_root =
        PathBuf::from("/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv");
    let raw_root = PathBuf::from("/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1");
    let catalog_path =
        PathBuf::from("/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let rocksdb_path = fsv_root.join(format!("q4-perf-labels-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join("q4_perf_label_producer_fsv.json");
    let row_id = format!("task-py-g-047-{run_id}");
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
    producer_rows += persist_extraction(&store, &extraction)? as u64;
    let mut raw_artifact_rows = 1_u64;
    for idx in 0..29 {
        let row_source = happy_source(&format!("task-py-g-047-{run_id}-producer-{idx:02}"));
        let row_outputs =
            run_fixture_outputs(&fixture_root, &fixture_script, &row_source.corpus_row_id)?;
        write_q4_perf_raw_outputs(&raw_root, &row_source.corpus_row_id, &row_outputs)?;
        raw_artifact_rows += 1;
        let row_extraction = extract_q4_perf_labels(&row_source, &row_outputs)?;
        producer_rows += persist_extraction(&store, &row_extraction)? as u64;
    }
    store.flush()?;
    let after_count = store.count_records()?;
    require("producer row minimum met", producer_rows >= 30)?;
    require(
        "store count moved by producer rows",
        after_count == before_count + producer_rows as usize,
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
    require(
        "reopened rows are labels",
        reopened_rows
            .iter()
            .all(|(_, row)| matches!(row.signal, Q4PerfSignalRecord::Label(_))),
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

    let catalog = update_q4_catalog(&catalog_path, producer_rows)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let perf_readiness = trust_report
        .head(Q4HeadKind::Perf)
        .ok_or("missing perf Q4 readiness")?;
    require(
        "perf producer is supported",
        perf_readiness.producer_supported,
    )?;
    require(
        "perf remains untrusted without #127 calibration",
        !perf_readiness.q4_head_ready,
    )?;

    let evidence = json!({
        "task": "TASK-PY-G-047",
        "issue": 266,
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
            "fsv_root": fsv_root,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Normalize pytest-benchmark/cProfile pre-post measurements into Q4 perf labels, persist to CF, reopen RocksDB, and link producer evidence into the Q4 trust catalog.",
            "expected": {
                "metric": "test_linear",
                "delta_pct_gt": 25.0,
                "regression": true,
                "producer_rows_min": 30,
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
                "raw_benchmark_sha256": sha256_file(&raw_paths.benchmark_json)?,
                "raw_cprofile_sha256": sha256_file(&raw_paths.cprofile_json)?,
                "predicted_perf_regressions": surfaces.predicted_perf_regressions,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
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
