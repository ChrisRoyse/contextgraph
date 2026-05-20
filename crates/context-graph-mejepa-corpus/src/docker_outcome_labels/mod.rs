//! Deterministic SWE-bench Docker outcome labels for TASK-PY-G-116.
//!
//! This module promotes the 300x8 Docker report corpus into a durable label
//! table keyed by `(task_instance_id, mutation_category)`. It does not execute
//! Docker. It reads already-materialized aiwonder corpus artifacts, derives
//! binary reality labels and test-phase vector counts, and persists the result
//! to a ME-JEPA RocksDB column family with byte readback.

mod discovery;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_DOCKER_OUTCOME_LABELS;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::Deserialize;

use crate::swebench::{parse_swebench_instance_report, swebench_report_to_verdict, TestsStatus};
use discovery::{
    discover_prediction_patches, discover_reports, path_is_mnt_d, reject_mnt_d_path,
    relative_or_absolute, sha256_file, sha256_text, PredictionPatchEvidence,
};
use types::{invalid, require};
pub use types::{
    DockerOutcomeLabelRow, DockerOutcomeMaterializationReport, DockerOutcomeResult,
    TestPhaseVectorLabels, DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
};

pub fn materialize_docker_outcome_labels(
    corpus_root: &Path,
    db: &DB,
    db_path: &Path,
) -> DockerOutcomeResult<DockerOutcomeMaterializationReport> {
    reject_mnt_d_path("corpus_root", corpus_root)?;
    reject_mnt_d_path("db_path", db_path)?;
    let index = read_index(corpus_root)?;
    let reports = discover_reports(corpus_root)?;
    let predictions = discover_prediction_patches(corpus_root)?;
    let mut seen_keys = BTreeSet::new();
    let mut category_counts = BTreeMap::new();
    let mut docker_label_counts = BTreeMap::new();
    let mut selected_report_count = 0_u64;
    let mut docker_report_missing_count = 0_u64;
    let mut resolved_index_mismatch_count = 0_u64;
    let mut oracle_per_test_count_mismatch_count = 0_u64;
    let mut prediction_patch_rows_attached = 0_u64;

    for entry in &index.entries {
        let key = DockerOutcomeLabelRow::storage_key(&entry.task_id, &entry.category);
        require(
            seen_keys.insert(key.clone()),
            format!("duplicate key {key}"),
        )?;
        *category_counts.entry(entry.category.clone()).or_insert(0) += 1;
        let report_path = reports.get(&key);
        let prediction = predictions.get(&key);
        if prediction.is_some() {
            prediction_patch_rows_attached += 1;
        }
        let row = build_row(corpus_root, entry, report_path, prediction)?;
        if row.selected_report_path.is_some() {
            selected_report_count += 1;
        }
        if row.docker_report_missing {
            docker_report_missing_count += 1;
        }
        if row.resolved_index_mismatch {
            resolved_index_mismatch_count += 1;
        }
        if row.test_phase_vector.total() != row.oracle_per_test_count {
            oracle_per_test_count_mismatch_count += 1;
        }
        increment_label_counts(&row, &mut docker_label_counts);
        write_row(db, &row)?;
    }

    Ok(DockerOutcomeMaterializationReport {
        schema_version: DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
        corpus_root: corpus_root.display().to_string(),
        db_path: db_path.display().to_string(),
        rows_persisted: seen_keys.len() as u64,
        unique_keys: seen_keys.len() as u64,
        expected_index_entries: index.entries.len() as u64,
        category_counts,
        expected_category_counts: index.expected_category_counts,
        docker_label_counts,
        selected_report_count,
        docker_report_missing_count,
        resolved_index_mismatch_count,
        oracle_per_test_count_mismatch_count,
        prediction_patch_rows_attached,
        active_mnt_d_reads_or_writes: path_is_mnt_d(corpus_root) || path_is_mnt_d(db_path),
    })
}

pub fn count_rows(db: &DB) -> DockerOutcomeResult<u64> {
    Ok(read_all_rows(db)?.len() as u64)
}

pub fn read_all_rows(db: &DB) -> DockerOutcomeResult<Vec<DockerOutcomeLabelRow>> {
    let cf = cf(db)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: DockerOutcomeLabelRow = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn build_row(
    corpus_root: &Path,
    entry: &IndexEntry,
    report_path: Option<&PathBuf>,
    prediction: Option<&PredictionPatchEvidence>,
) -> DockerOutcomeResult<DockerOutcomeLabelRow> {
    let key = DockerOutcomeLabelRow::storage_key(&entry.task_id, &entry.category);
    let mut vector = TestPhaseVectorLabels::default();
    let mut hashed_tests = BTreeMap::new();
    let mut docker_resolved_clean = false;
    let mut patch_apply_failure = false;
    let mut selected_report_path = None;
    let mut selected_report_sha256 = None;
    let mut report_missing = true;
    let mut mismatch_exception = None;
    let mut resolved_index_mismatch = false;

    if let Some(path) = report_path {
        report_missing = false;
        selected_report_path = Some(relative_or_absolute(corpus_root, path));
        selected_report_sha256 = Some(sha256_file(path)?);
        let text = fs::read_to_string(path)?;
        let report = parse_swebench_instance_report(&entry.task_id, &text)
            .map_err(|err| invalid(format!("parse report {}: {err}", path.display())))?;
        let verdict = swebench_report_to_verdict(&report, None)
            .map_err(|err| invalid(format!("verdict report {}: {err}", path.display())))?;
        docker_resolved_clean =
            report.resolved && report.patch_successfully_applied && verdict.exception.is_none();
        patch_apply_failure =
            report.patch_is_none || !report.patch_exists || !report.patch_successfully_applied;
        if report.resolved != entry.oracle_all_passed {
            resolved_index_mismatch = true;
            mismatch_exception = entry.oracle_exception.clone().or_else(|| {
                Some(format!(
                    "selected Docker resolved={} != index.oracle_all_passed={}",
                    report.resolved, entry.oracle_all_passed
                ))
            });
        }
        if let Some(tests) = report.tests_status.as_ref() {
            vector = vector_from_tests_status(tests);
            hashed_tests = hash_tests_by_bucket(tests);
        }
    }

    let target_failed = vector.fail_to_pass_failure_count > 0;
    let regression_failed = vector.pass_to_pass_failure_count > 0;
    let harness_exception_or_error = entry.oracle_exception.is_some() || patch_apply_failure;
    let row = DockerOutcomeLabelRow {
        schema_version: DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
        task_instance_id: entry.task_id.clone(),
        mutation_category: entry.category.clone(),
        key,
        oracle_pass: entry.oracle_all_passed,
        oracle_fail_or_error: !entry.oracle_all_passed || harness_exception_or_error,
        harness_exception_or_error,
        oracle_per_test_count: entry.oracle_per_test_count,
        oracle_verdict_sha256: entry.oracle_verdict_sha256.clone(),
        docker_resolved_clean,
        target_fail_to_pass_tests_failed: target_failed,
        regression_pass_to_pass_tests_failed: regression_failed,
        target_and_regression_tests_failed: target_failed && regression_failed,
        patch_apply_failure,
        docker_report_missing: report_missing,
        test_phase_vector: vector,
        hashed_test_identifiers_by_bucket: hashed_tests,
        index_patch_path: entry.patch_path.clone(),
        index_patch_sha256: entry.patch_sha256.clone(),
        selected_report_path,
        selected_report_sha256,
        prediction_jsonl_path: prediction.map(|p| relative_or_absolute(corpus_root, &p.jsonl_path)),
        prediction_patch_sha256: prediction.map(|p| p.model_patch_sha256.clone()),
        resolved_index_mismatch,
        mismatch_exception,
    };
    row.validate()?;
    Ok(row)
}

fn vector_from_tests_status(tests: &TestsStatus) -> TestPhaseVectorLabels {
    TestPhaseVectorLabels {
        fail_to_pass_success_count: tests.fail_to_pass.success.len() as u64,
        fail_to_pass_failure_count: tests.fail_to_pass.failure.len() as u64,
        pass_to_pass_success_count: tests.pass_to_pass.success.len() as u64,
        pass_to_pass_failure_count: tests.pass_to_pass.failure.len() as u64,
        fail_to_fail_success_count: tests.fail_to_fail.success.len() as u64,
        fail_to_fail_failure_count: tests.fail_to_fail.failure.len() as u64,
        pass_to_fail_success_count: tests.pass_to_fail.success.len() as u64,
        pass_to_fail_failure_count: tests.pass_to_fail.failure.len() as u64,
    }
}

fn hash_tests_by_bucket(tests: &TestsStatus) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    insert_hashes(
        &mut out,
        "FAIL_TO_PASS.success",
        &tests.fail_to_pass.success,
    );
    insert_hashes(
        &mut out,
        "FAIL_TO_PASS.failure",
        &tests.fail_to_pass.failure,
    );
    insert_hashes(
        &mut out,
        "PASS_TO_PASS.success",
        &tests.pass_to_pass.success,
    );
    insert_hashes(
        &mut out,
        "PASS_TO_PASS.failure",
        &tests.pass_to_pass.failure,
    );
    insert_hashes(
        &mut out,
        "FAIL_TO_FAIL.success",
        &tests.fail_to_fail.success,
    );
    insert_hashes(
        &mut out,
        "FAIL_TO_FAIL.failure",
        &tests.fail_to_fail.failure,
    );
    insert_hashes(
        &mut out,
        "PASS_TO_FAIL.success",
        &tests.pass_to_fail.success,
    );
    insert_hashes(
        &mut out,
        "PASS_TO_FAIL.failure",
        &tests.pass_to_fail.failure,
    );
    out
}

fn insert_hashes(out: &mut BTreeMap<String, Vec<String>>, key: &str, tests: &[String]) {
    out.insert(
        key.to_string(),
        tests.iter().map(|test| sha256_text(test)).collect(),
    );
}

fn write_row(db: &DB, row: &DockerOutcomeLabelRow) -> DockerOutcomeResult<()> {
    row.validate()?;
    let value = bincode::serialize(row)?;
    let cf = cf(db)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, row.key.as_bytes(), &value, &opts)?;
    let readback = db
        .get_cf(cf, row.key.as_bytes())?
        .ok_or_else(|| invalid(format!("missing readback for {}", row.key)))?;
    require(
        readback.as_slice() == value.as_slice(),
        "row readback mismatch",
    )
}

fn cf(db: &DB) -> DockerOutcomeResult<&rocksdb::ColumnFamily> {
    db.cf_handle(CF_MEJEPA_DOCKER_OUTCOME_LABELS)
        .ok_or_else(|| {
            invalid(format!(
                "missing column family {CF_MEJEPA_DOCKER_OUTCOME_LABELS}"
            ))
        })
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    entries: Vec<IndexEntry>,
    #[serde(default)]
    expected_category_counts: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    task_id: String,
    category: String,
    oracle_all_passed: bool,
    oracle_exception: Option<String>,
    oracle_per_test_count: u64,
    oracle_verdict_sha256: String,
    patch_path: Option<String>,
    patch_sha256: Option<String>,
}

fn read_index(corpus_root: &Path) -> DockerOutcomeResult<CorpusIndex> {
    let text = fs::read_to_string(corpus_root.join("index.json"))?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let entries: Vec<IndexEntry> = serde_json::from_value(
        value
            .get("entries")
            .cloned()
            .ok_or_else(|| invalid("index.json missing entries"))?,
    )?;
    let stats_text = fs::read_to_string(corpus_root.join("stats.json"))?;
    let stats_value: serde_json::Value = serde_json::from_str(&stats_text)?;
    let stats_by_category: BTreeMap<String, u64> = serde_json::from_value(
        stats_value
            .get("by_category")
            .cloned()
            .ok_or_else(|| invalid("stats.json missing by_category"))?,
    )?;
    Ok(CorpusIndex {
        expected_category_counts: stats_by_category.clone(),
        entries,
    })
}

fn increment_label_counts(row: &DockerOutcomeLabelRow, counts: &mut BTreeMap<String, u64>) {
    let labels = [
        ("oracle_pass", row.oracle_pass),
        ("oracle_fail_or_error", row.oracle_fail_or_error),
        ("harness_exception_or_error", row.harness_exception_or_error),
        ("docker_resolved_clean", row.docker_resolved_clean),
        (
            "target_fail_to_pass_tests_failed",
            row.target_fail_to_pass_tests_failed,
        ),
        (
            "regression_pass_to_pass_tests_failed",
            row.regression_pass_to_pass_tests_failed,
        ),
        (
            "target_and_regression_tests_failed",
            row.target_and_regression_tests_failed,
        ),
        ("patch_apply_failure", row.patch_apply_failure),
        ("docker_report_missing", row.docker_report_missing),
    ];
    for (label, active) in labels {
        if active {
            *counts.entry(label.to_string()).or_insert(0) += 1;
        }
    }
}
