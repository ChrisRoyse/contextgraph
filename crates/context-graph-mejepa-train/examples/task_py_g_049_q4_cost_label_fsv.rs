use context_graph_mejepa::q4_trust_gate::{q4_trust_gate_report_from_catalog_path, Q4HeadKind};
use context_graph_mejepa::CostAxis;
use context_graph_mejepa_instruments::{
    extract_q4_cost_labels, write_q4_cost_raw_outputs, Q4CostKind, Q4CostLabelStore,
    Q4CostSignalRecord,
};
use context_graph_mejepa_train::labels::q4_cost_labels_to_prediction_surfaces;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "task_py_g_049_q4_cost_label_fsv_support.rs"]
mod task_py_g_049_q4_cost_label_fsv_support;

use task_py_g_049_q4_cost_label_fsv_support::*;

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let task = env::var("CG_Q4_COST_FSV_TASK").unwrap_or_else(|_| "TASK-PY-G-049".to_string());
    let issue = env::var("CG_Q4_COST_FSV_ISSUE")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(268);
    let row_prefix =
        env::var("CG_Q4_COST_ROW_PREFIX").unwrap_or_else(|_| "task-py-g-049".to_string());
    let evidence_name = env::var("CG_Q4_COST_FSV_EVIDENCE_NAME")
        .unwrap_or_else(|_| "q4_cost_label_producer_fsv.json".to_string());
    let fsv_root = env_path(
        "CG_Q4_COST_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv",
    );
    let raw_root = env_path(
        "CG_Q4_COST_RAW_ROOT",
        "/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1",
    );
    let catalog_producer_fsv_root =
        PathBuf::from("/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/");
    let catalog_path =
        PathBuf::from("/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let rocksdb_path = fsv_root.join(format!("q4-cost-labels-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join(evidence_name);
    let row_id = format!("{row_prefix}-{run_id}");
    let fixture_root = fsv_root.join(format!("fixture-{run_id}"));
    let fixture_script = write_cost_fixture(&fixture_root)?;
    let fixture_patch = fixture_root.join("requirements.patch");
    let fixture_patch_text = fs::read_to_string(&fixture_patch)?;
    require(
        "happy fixture patch adds numpy and scipy requirements",
        fixture_patch_text.contains("+numpy==2.0") && fixture_patch_text.contains("+scipy==1.14"),
    )?;

    let source = happy_source(&row_id);
    let outputs = run_fixture_outputs(&fixture_root, &fixture_script, "pre", "post")?;
    let raw_paths = write_q4_cost_raw_outputs(&raw_root, &row_id, &outputs)?;
    require("cost raw output exists", raw_paths.cost_json.exists())?;

    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    require(
        "happy path produced three labels",
        extraction.labels.len() == 3,
    )?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    let dep_label = extraction
        .labels
        .iter()
        .find(|label| label.kind == Q4CostKind::DependencyCount)
        .ok_or("missing dependency-count label")?;
    require("happy dep-count regression", dep_label.regression)?;
    require("happy dep-count delta is 2", dep_label.delta == 2.0)?;

    let store = Q4CostLabelStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let mut producer_rows = 0_u64;
    producer_rows += persist_extraction(&store, &extraction)? as u64;
    let mut raw_artifact_rows = 1_u64;
    for idx in 0..29 {
        let row_source = happy_source(&format!("{row_prefix}-{run_id}-producer-{idx:02}"));
        let row_outputs = run_fixture_outputs(&fixture_root, &fixture_script, "pre", "post")?;
        write_q4_cost_raw_outputs(&raw_root, &row_source.corpus_row_id, &row_outputs)?;
        raw_artifact_rows += 1;
        let row_extraction = extract_q4_cost_labels(&row_source, &row_outputs)?;
        producer_rows += persist_extraction(&store, &row_extraction)? as u64;
    }
    let alternate_source = alternate_chunk_source(&source.corpus_row_id);
    let alternate_outputs = run_fixture_outputs(&fixture_root, &fixture_script, "pre", "post")?;
    let alternate_raw_paths = write_q4_cost_raw_outputs(
        &raw_root,
        &format!("{}-alternate-chunk", alternate_source.corpus_row_id),
        &alternate_outputs,
    )?;
    raw_artifact_rows += 1;
    let alternate_extraction = extract_q4_cost_labels(&alternate_source, &alternate_outputs)?;
    producer_rows += persist_extraction(&store, &alternate_extraction)? as u64;
    store.flush()?;
    let after_count = store.count_records()?;
    require("producer row minimum met", producer_rows >= 30)?;
    require(
        "store count moved by producer rows",
        after_count == before_count + producer_rows as usize,
    )?;
    drop(store);

    let reopened = Q4CostLabelStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    let canonical_readback = reopened
        .get_label_record(
            &source.corpus_row_id,
            &source.chunk_id,
            Q4CostKind::DependencyCount,
        )?
        .ok_or("missing canonical cost label readback")?;
    let alternate_readback = reopened
        .get_label_record(
            &alternate_source.corpus_row_id,
            &alternate_source.chunk_id,
            Q4CostKind::DependencyCount,
        )?
        .ok_or("missing alternate same-row cost label readback")?;
    require(
        "reopened RocksDB sees producer rows",
        reopened_rows.len() == after_count,
    )?;
    require(
        "reopened rows are labels",
        reopened_rows
            .iter()
            .all(|(_, row)| matches!(row.signal, Q4CostSignalRecord::Label(_))),
    )?;
    require(
        "canonical row/kind readback is label",
        matches!(canonical_readback.signal, Q4CostSignalRecord::Label(_)),
    )?;
    require(
        "same row/different chunk readback is label",
        matches!(alternate_readback.signal, Q4CostSignalRecord::Label(_)),
    )?;
    let canonical_key = format!(
        "{}:label:{}:{}",
        source.corpus_row_id,
        Q4CostKind::DependencyCount.as_str(),
        sha256_text(&source.chunk_id),
    );
    let alternate_key = format!(
        "{}:label:{}:{}",
        alternate_source.corpus_row_id,
        Q4CostKind::DependencyCount.as_str(),
        sha256_text(&alternate_source.chunk_id),
    );
    require(
        "same row/different chunk keys do not collide",
        canonical_key != alternate_key,
    )?;
    drop(reopened);

    let surfaces = q4_cost_labels_to_prediction_surfaces(&extraction.labels)?;
    require(
        "cost adapter emits dependency regression surface",
        surfaces
            .predicted_cost_regressions
            .iter()
            .any(|item| item.axis == CostAxis::DependencyCount),
    )?;

    let edge_cases = vec![
        edge_no_requirements_absent_dep_label(&fixture_root, &fixture_script)?,
        edge_wheel_build_failure_quarantined(&fixture_root, &fixture_script)?,
        edge_dependency_removal_improvement(&fixture_root, &fixture_script)?,
        edge_noisy_walltime_only(&fixture_root, &fixture_script)?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 cost edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let persisted_quarantine_extraction = persisted_quarantine_fixture()?;
    let quarantine_store = Q4CostLabelStore::open(&rocksdb_path)?;
    let persisted_quarantine_keys =
        quarantine_store.put_extraction(&persisted_quarantine_extraction)?;
    quarantine_store.flush()?;
    drop(quarantine_store);
    let quarantine_reopened = Q4CostLabelStore::open(&rocksdb_path)?;
    let quarantine_reopened_rows = quarantine_reopened.scan_records()?;
    let quarantine_reopened_count = quarantine_reopened_rows
        .iter()
        .filter(|(_, row)| matches!(row.signal, Q4CostSignalRecord::Quarantine(_)))
        .count();
    require(
        "quarantine rows survive detail-aware persistence",
        quarantine_reopened_count >= persisted_quarantine_extraction.quarantines.len(),
    )?;
    drop(quarantine_reopened);

    let catalog = update_q4_catalog(&catalog_path, producer_rows, &catalog_producer_fsv_root)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let cost_readiness = trust_report
        .head(Q4HeadKind::Cost)
        .ok_or("missing cost Q4 readiness")?;
    require(
        "cost producer is supported",
        cost_readiness.producer_supported,
    )?;
    require(
        "cost catalog producer root stays canonical",
        cost_readiness.producer_fsv_root == canonical_fsv_root(&catalog_producer_fsv_root),
    )?;
    require(
        "cost per-slot evidence is not synthesized",
        !cost_readiness.per_slot_supported,
    )?;
    require(
        "cost remains untrusted without #127 calibration",
        !cost_readiness.q4_head_ready,
    )?;

    let evidence = json!({
        "task": task,
        "issue": issue,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "alternate_same_row_raw_output_paths": alternate_raw_paths,
            "real_fixture_root": fixture_root,
            "real_fixture_script": fixture_script,
            "real_fixture_patch": fixture_patch,
            "rocksdb_path": rocksdb_path,
            "cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_COST_LABELS,
            "evidence_path": evidence_path,
            "q4_catalog_path": catalog_path,
            "q4_catalog_producer_fsv_root": catalog_producer_fsv_root,
            "fsv_root": fsv_root,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Materialize a Python row with pre/post requirements.txt and wheel artifacts, apply the accepted patch shape adding numpy==2.0 and scipy==1.14, parse pytest-compatible wall-clock output plus requirements/wheel metadata from real bounded Python commands, normalize deltas into Q4 cost labels, persist to CF, reopen RocksDB, and link producer evidence into the Q4 trust catalog.",
            "expected": {
                "kind": "dependency_count",
                "baseline": 12.0,
                "after": 14.0,
                "delta": 2.0,
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
                "alternate_same_row_readback": alternate_readback,
                "canonical_readback_key": canonical_key,
                "alternate_same_row_readback_key": alternate_key,
                "raw_cost_sha256": sha256_file(&raw_paths.cost_json)?,
                "alternate_same_row_raw_cost_sha256": sha256_file(&alternate_raw_paths.cost_json)?,
                "predicted_cost_regressions": surfaces.predicted_cost_regressions,
                "persisted_quarantine_keys": persisted_quarantine_keys,
                "persisted_quarantine_rows": persisted_quarantine_extraction.quarantines,
                "quarantine_reopened_count": quarantine_reopened_count,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "direction_rule": "positive cost deltas are regressions; negative cost deltas are improvements and do not emit regression surfaces",
            "missing_precondition_rule": "missing dependency manifests do not synthesize zero dependency labels",
            "noisy_walltime_rule": "CI-minute CV above 0.20 quarantines only the CI-minute label and leaves independent dependency/wheel labels usable",
            "trust_boundary": "producer rows alone do not unlock trusted Q4 cost; #127 calibration and separately verified slot evidence remain required",
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    require(
        "dependency improvement edge exercised",
        edge_cases
            .iter()
            .any(|case| case.name == "dependency_removal_improvement"),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn env_path(var: &str, default: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}
