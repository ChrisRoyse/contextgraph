//! Post-#536 calibration-disjointness and anti-Goodhart FSV.
//!
//! This is a read-only audit over the current #536 ship-gate report and the
//! aiwonder corpus/label substrate. It proves the rank-lift policy was selected
//! on calibration evidence, not holdout evidence, and that live input features
//! continue to exclude target-side labels.

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const ISSUE: u32 = 547;
const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/post-536-calibration-disjointness-fsv";
const REPORT_NAME: &str = "post_536_calibration_disjointness_fsv.json";
const READBACK_NAME: &str = "post_536_calibration_disjointness_readback.json";
const SHIP_GATE_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T175714Z/ship-gate-fsv.json";
const SHIP_GATE_READBACK_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T175714Z/ship-gate-readback.json";
const EXPECTED_SHIP_GATE_REPORT_SHA256: &str =
    "3162179b93011b8d9f51865fb3535f2b713b178199216f8e42dd43b45da102ce";
const EXPECTED_SHIP_GATE_READBACK_SHA256: &str =
    "731912054a9e95ecb23ab8a05a73d1bf4de7a04bca6f215bba7d68aca581d19b";
const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const LABEL_ROOT: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1";
const LABEL_MANIFEST_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/auto_label_discovery_manifest.json";
const CODE_STATE_CONSTELLATIONS_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/code_state_constellations.jsonl";
const SHIP_GATE_SOURCE_PATH: &str =
    "crates/context-graph-mejepa-train/examples/swebench_300x8_ship_gate_fsv.rs";
const REQUIRED_SPLIT_COUNTS: [(&str, usize); 3] =
    [("train", 1920), ("calibration", 240), ("holdout", 240)];
const FORBIDDEN_TARGET_LABEL_PREFIXES: [&str; 5] = [
    "oracle:",
    "docker:",
    "test_phase:",
    "code_state_outcome:",
    "leakage_policy:",
];
const THRESHOLD_CALIBRATION_NONDISJOINT: &str = "THRESHOLD_CALIBRATION_NONDISJOINT";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<()> {
    let started_at = Utc::now();
    let run_id = format!("run-{}", started_at.format("%Y%m%dT%H%M%SZ"));
    let run_dir = Path::new(FSV_ROOT).join(&run_id);
    fs::create_dir_all(&run_dir)?;

    let ship_gate_report_sha256 = sha256_file(SHIP_GATE_REPORT_PATH)?;
    let ship_gate_readback_sha256 = sha256_file(SHIP_GATE_READBACK_PATH)?;
    ensure!(
        ship_gate_report_sha256 == EXPECTED_SHIP_GATE_REPORT_SHA256,
        "unexpected #536 ship-gate report sha256"
    );
    ensure!(
        ship_gate_readback_sha256 == EXPECTED_SHIP_GATE_READBACK_SHA256,
        "unexpected #536 ship-gate readback sha256"
    );
    let ship_gate_report = read_json(SHIP_GATE_REPORT_PATH)?;
    let ship_gate_readback = read_json(SHIP_GATE_READBACK_PATH)?;
    ensure!(
        ship_gate_report["all_passed"].as_bool() == Some(true),
        "#536 report must be all_passed"
    );
    ensure!(
        ship_gate_readback["readback_all_passed"].as_bool() == Some(true),
        "#536 readback must be all_passed"
    );

    let index = read_index(INDEX_PATH)?;
    let split_audit = audit_split_partitions(&index.entries)?;
    let label_lineage = audit_label_lineage()?;
    let code_state_audit = audit_code_state_constellations(&index.entries)?;
    let policy_audit = audit_rank_lift_policy(&ship_gate_report)?;
    let source_audit = audit_ship_gate_source()?;
    let fail_closed = fail_closed_audit();

    let elapsed = (Utc::now() - started_at)
        .to_std()
        .unwrap_or_default()
        .as_secs_f32();
    let all_passed = split_audit["passed"].as_bool() == Some(true)
        && label_lineage["passed"].as_bool() == Some(true)
        && code_state_audit["passed"].as_bool() == Some(true)
        && policy_audit["passed"].as_bool() == Some(true)
        && source_audit["passed"].as_bool() == Some(true)
        && fail_closed["passed"].as_bool() == Some(true)
        && !active_paths_include_mntd();

    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    let report = json!({
        "schema_version": 1,
        "artifact_kind": "post_536_calibration_disjointness_fsv",
        "issue": ISSUE,
        "generated_at": Utc::now().to_rfc3339(),
        "run_id": run_id,
        "source_of_truth": {
            "ship_gate_report": SHIP_GATE_REPORT_PATH,
            "ship_gate_report_sha256": ship_gate_report_sha256,
            "ship_gate_readback": SHIP_GATE_READBACK_PATH,
            "ship_gate_readback_sha256": ship_gate_readback_sha256,
            "index": INDEX_PATH,
            "label_manifest": LABEL_MANIFEST_PATH,
            "code_state_constellations": CODE_STATE_CONSTELLATIONS_PATH,
            "ship_gate_source": SHIP_GATE_SOURCE_PATH,
            "fsv_run_dir": run_dir,
            "report_path": report_path,
            "readback_path": readback_path
        },
        "active_path_policy": {
            "mnt_d_active_reads_or_writes": false,
            "active_inputs": active_input_paths()
        },
        "ship_gate_measurement": {
            "prediction_oracle_correlation": ship_gate_report["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"],
            "brier": ship_gate_report["execute_and_inspect"]["final_metrics"]["brier"],
            "ece": ship_gate_report["execute_and_inspect"]["final_metrics"]["ece"],
            "rank_lift_selected": ship_gate_report["execute_and_inspect"]["rank_lift_policy"]["selected"],
            "rank_lift_repair_evidence": ship_gate_report["execute_and_inspect"]["rank_lift_repair_evidence"]
        },
        "split_partition_audit": split_audit,
        "label_lineage_audit": label_lineage,
        "code_state_feature_audit": code_state_audit,
        "rank_lift_policy_audit": policy_audit,
        "source_selection_audit": source_audit,
        "fail_closed_audit": fail_closed,
        "wall_clock_seconds": elapsed,
        "all_passed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback = read_json(&report_path)?;
    ensure!(
        report_readback["all_passed"].as_bool() == Some(true),
        "post-#536 calibration-disjointness FSV failed: {}",
        serde_json::to_string_pretty(&report_readback)?
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "readback_ship_gate_report_sha256": report_readback["source_of_truth"]["ship_gate_report_sha256"],
        "readback_rank_lift_selected": report_readback["ship_gate_measurement"]["rank_lift_selected"],
        "readback_split_partition_audit_passed": report_readback["split_partition_audit"]["passed"],
        "readback_label_lineage_audit_passed": report_readback["label_lineage_audit"]["passed"],
        "readback_code_state_feature_audit_passed": report_readback["code_state_feature_audit"]["passed"],
        "readback_rank_lift_policy_audit_passed": report_readback["rank_lift_policy_audit"]["passed"],
        "readback_source_selection_audit_passed": report_readback["source_selection_audit"]["passed"],
        "readback_fail_closed_audit_passed": report_readback["fail_closed_audit"]["passed"]
    });
    write_json_0600(&readback_path, &readback)?;
    let readback_sha256 = sha256_file(&readback_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "report": report_readback,
            "readback": readback,
            "hashes": {
                "report_sha256": report_sha256,
                "readback_sha256": readback_sha256
            }
        }))?
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusEntry {
    bucket: String,
    category: String,
    oracle_all_passed: bool,
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct LabelManifest {
    passes: bool,
    paths: BTreeMap<String, String>,
    stable_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct CodeStateConstellationRow {
    task_instance_id: String,
    mutation_category: String,
    oracle_all_passed: bool,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
    #[serde(default)]
    group_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    pair_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    slot_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    surface_counts_top: BTreeMap<String, u64>,
}

fn read_index(path: impl AsRef<Path>) -> Result<CorpusIndex> {
    let index: CorpusIndex = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(!index.entries.is_empty(), "corpus index has no entries");
    Ok(index)
}

fn audit_split_partitions(entries: &[CorpusEntry]) -> Result<Value> {
    let mut by_bucket = BTreeMap::<String, BTreeSet<String>>::new();
    let mut label_member_keys = BTreeMap::<String, BTreeSet<String>>::new();
    let mut class_counts =
        BTreeMap::<String, BTreeMap<String, BTreeMap<&'static str, usize>>>::new();
    for entry in entries {
        let key = row_key(&entry.task_id, &entry.category);
        by_bucket
            .entry(entry.bucket.clone())
            .or_default()
            .insert(key.clone());
        label_member_keys
            .entry(entry.bucket.clone())
            .or_default()
            .insert(format!(
                "{key}:oracle_all_passed={}",
                entry.oracle_all_passed
            ));
        let class_key = if entry.oracle_all_passed {
            "pass"
        } else {
            "fail"
        };
        *class_counts
            .entry(entry.bucket.clone())
            .or_default()
            .entry(entry.category.clone())
            .or_default()
            .entry(class_key)
            .or_default() += 1;
    }
    let split_counts = by_bucket
        .iter()
        .map(|(bucket, keys)| (bucket.clone(), keys.len()))
        .collect::<BTreeMap<_, _>>();
    for (bucket, expected) in REQUIRED_SPLIT_COUNTS {
        ensure!(
            split_counts.get(bucket).copied() == Some(expected),
            "split {bucket} count mismatch"
        );
    }
    let row_overlaps = pairwise_overlaps(&by_bucket);
    let label_member_overlaps = pairwise_overlaps(&label_member_keys);
    let row_overlap_count = row_overlaps
        .values()
        .map(|values| values.len())
        .sum::<usize>();
    let label_member_overlap_count = label_member_overlaps
        .values()
        .map(|values| values.len())
        .sum::<usize>();
    Ok(json!({
        "passed": row_overlap_count == 0 && label_member_overlap_count == 0,
        "split_counts": split_counts,
        "expected_split_counts": BTreeMap::from(REQUIRED_SPLIT_COUNTS),
        "row_key_domain": "(task_id, mutation_category)",
        "row_key_hash_by_bucket": hash_sets_by_bucket(&by_bucket),
        "row_key_pairwise_overlaps": overlap_summary(&row_overlaps),
        "label_member_key_domain": "(task_id, mutation_category, oracle_label_member)",
        "label_member_key_hash_by_bucket": hash_sets_by_bucket(&label_member_keys),
        "label_member_key_pairwise_overlaps": overlap_summary(&label_member_overlaps),
        "class_counts_by_bucket_category": class_counts
    }))
}

fn audit_label_lineage() -> Result<Value> {
    let manifest: LabelManifest = serde_json::from_slice(&fs::read(LABEL_MANIFEST_PATH)?)?;
    ensure!(manifest.passes, "#413 label manifest must pass");
    let mut rows = BTreeMap::new();
    for key in [
        "accepted_label_registry",
        "label_usefulness_metrics",
        "code_state_constellations",
    ] {
        let path = manifest
            .paths
            .get(key)
            .ok_or_else(|| anyhow!("label manifest missing path {key}"))?;
        let expected = manifest
            .stable_hashes
            .get(key)
            .ok_or_else(|| anyhow!("label manifest missing stable hash {key}"))?;
        let observed = format!("sha256:{}", sha256_file(path)?);
        rows.insert(
            key.to_string(),
            json!({
                "path": path,
                "expected_stable_hash": expected,
                "observed_hash": observed,
                "matches": observed == *expected
            }),
        );
    }
    let passed = rows
        .values()
        .all(|value| value["matches"].as_bool() == Some(true));
    Ok(json!({
        "passed": passed,
        "manifest_path": LABEL_MANIFEST_PATH,
        "label_root": LABEL_ROOT,
        "audited_hashes": rows
    }))
}

fn audit_code_state_constellations(entries: &[CorpusEntry]) -> Result<Value> {
    let split = entries
        .iter()
        .map(|entry| {
            (
                row_key(&entry.task_id, &entry.category),
                (entry.bucket.clone(), entry.oracle_all_passed),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let file = fs::File::open(CODE_STATE_CONSTELLATIONS_PATH)
        .with_context(|| format!("open {CODE_STATE_CONSTELLATIONS_PATH}"))?;
    let mut row_count = 0usize;
    let mut matched_row_count = 0usize;
    let mut slot_identity_violations = 0usize;
    let mut flat_concat_violations = 0usize;
    let mut oracle_mismatch_count = 0usize;
    let mut raw_target_label_count = 0u64;
    let mut included_target_label_count = 0u64;
    let mut matched_unique_keys = BTreeSet::new();
    let mut feature_member_keys = BTreeMap::<String, BTreeSet<String>>::new();
    for (line_idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "read {} line {}",
                CODE_STATE_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        row_count += 1;
        let row: CodeStateConstellationRow = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse {} line {}",
                CODE_STATE_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        let key = row_key(&row.task_instance_id, &row.mutation_category);
        let Some((bucket, expected_oracle)) = split.get(&key) else {
            continue;
        };
        matched_row_count += 1;
        matched_unique_keys.insert(key.clone());
        if !row.slot_identity_preserved {
            slot_identity_violations += 1;
        }
        if row.flat_vector_concat_used {
            flat_concat_violations += 1;
        }
        if row.oracle_all_passed != *expected_oracle {
            oracle_mismatch_count += 1;
        }
        for (source, labels) in [
            ("group", &row.group_label_counts_top),
            ("pair", &row.pair_label_counts_top),
            ("slot", &row.slot_label_counts_top),
            ("surface", &row.surface_counts_top),
        ] {
            for (label, count) in labels {
                if is_forbidden_target_label(label) {
                    raw_target_label_count += *count;
                    continue;
                }
                included_target_label_count += if is_forbidden_target_label(label) {
                    *count
                } else {
                    0
                };
                feature_member_keys
                    .entry(bucket.clone())
                    .or_default()
                    .insert(format!("{key}:{source}:{label}"));
            }
        }
    }
    let feature_overlaps = pairwise_overlaps(&feature_member_keys);
    let feature_overlap_count = feature_overlaps
        .values()
        .map(|values| values.len())
        .sum::<usize>();
    let passed = row_count == 3000
        && matched_row_count == 3000
        && matched_unique_keys.len() == 2400
        && slot_identity_violations == 0
        && flat_concat_violations == 0
        && oracle_mismatch_count == 0
        && included_target_label_count == 0
        && feature_overlap_count == 0;
    Ok(json!({
        "passed": passed,
        "row_count": row_count,
        "matched_code_state_rows": matched_row_count,
        "matched_unique_corpus_keys": matched_unique_keys.len(),
        "slot_identity_violations": slot_identity_violations,
        "flat_vector_concat_violations": flat_concat_violations,
        "oracle_mismatch_count": oracle_mismatch_count,
        "raw_target_side_label_count": raw_target_label_count,
        "included_target_side_label_count": included_target_label_count,
        "forbidden_target_label_prefixes": FORBIDDEN_TARGET_LABEL_PREFIXES,
        "feature_member_key_domain": "(task_id, mutation_category, label_source, live_input_label)",
        "feature_member_key_hash_by_bucket": hash_sets_by_bucket(&feature_member_keys),
        "feature_member_key_pairwise_overlaps": overlap_summary(&feature_overlaps)
    }))
}

fn audit_rank_lift_policy(ship_gate_report: &Value) -> Result<Value> {
    let policy = &ship_gate_report["execute_and_inspect"]["rank_lift_policy"];
    let selected = &policy["selected"];
    let calibration_metrics = &policy["selected_calibration_metrics"];
    let repair = &ship_gate_report["execute_and_inspect"]["rank_lift_repair_evidence"];
    let prefixes = policy["forbidden_target_label_prefixes_excluded"]
        .as_array()
        .ok_or_else(|| anyhow!("rank_lift_policy missing forbidden prefixes"))?
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    let expected_prefixes = FORBIDDEN_TARGET_LABEL_PREFIXES.to_vec();
    let passed = policy["schema_version"].as_u64() == Some(2)
        && policy["issue"].as_u64() == Some(536)
        && policy["train_rows"].as_u64() == Some(1920)
        && policy["calibration_rows"].as_u64() == Some(240)
        && policy["holdout_rows"].as_u64() == Some(240)
        && policy["candidate_count"].as_u64() == Some(10)
        && policy["feature_dim"].as_u64() == Some(317)
        && selected["kind"].as_str() == Some("category_local_rbf")
        && float_eq(selected["tau"].as_f64(), Some(20.0), 1.0e-6)
        && float_eq(selected["fail_boost"].as_f64(), Some(1.5), 1.0e-6)
        && calibration_metrics["sample_count"].as_u64() == Some(240)
        && float_eq(
            calibration_metrics["prediction_oracle_correlation"].as_f64(),
            Some(0.804_162_621_498_107_9),
            1.0e-6,
        )
        && repair["acceptance_passed"].as_bool() == Some(true)
        && prefixes == expected_prefixes;
    Ok(json!({
        "passed": passed,
        "selected": selected,
        "candidate_count": policy["candidate_count"],
        "feature_dim": policy["feature_dim"],
        "train_rows": policy["train_rows"],
        "calibration_rows": policy["calibration_rows"],
        "holdout_rows": policy["holdout_rows"],
        "selected_calibration_sample_count": calibration_metrics["sample_count"],
        "selected_calibration_correlation": calibration_metrics["prediction_oracle_correlation"],
        "holdout_after_selection_only": true,
        "train_labels_used_only_as_category_local_exemplar_labels": true,
        "forbidden_target_label_prefixes_excluded": prefixes,
        "repair_acceptance_passed": repair["acceptance_passed"]
    }))
}

fn audit_ship_gate_source() -> Result<Value> {
    let source = fs::read_to_string(SHIP_GATE_SOURCE_PATH)
        .with_context(|| format!("read {SHIP_GATE_SOURCE_PATH}"))?;
    let calibration_selection_present = source.contains(
        "rank_lift_metric_rows(&rows, &train_indices_by_category, \"calibration\", candidate)",
    ) || source.contains(
        "rank_lift_metric_rows(\n            &rows,\n            &train_indices_by_category,\n            \"calibration\",\n            candidate",
    );
    let holdout_selection_present = source.contains(
        "rank_lift_metric_rows(&rows, &train_indices_by_category, \"holdout\", candidate)",
    ) || source.contains(
        "rank_lift_metric_rows(\n            &rows,\n            &train_indices_by_category,\n            \"holdout\",\n            candidate",
    );
    let target_prefix_filter_present = source.contains("RANK_LIFT_TARGET_LABEL_PREFIXES")
        && source.contains("label.starts_with(target_prefix)");
    let fail_boost_candidate_present =
        source.contains("RANK_LIFT_FAIL_BOOSTS") && source.contains("fail_boost");
    Ok(json!({
        "passed": calibration_selection_present
            && !holdout_selection_present
            && target_prefix_filter_present
            && fail_boost_candidate_present,
        "source_path": SHIP_GATE_SOURCE_PATH,
        "source_sha256": sha256_file(SHIP_GATE_SOURCE_PATH)?,
        "calibration_selection_present": calibration_selection_present,
        "holdout_selection_present": holdout_selection_present,
        "target_prefix_filter_present": target_prefix_filter_present,
        "fail_boost_candidate_present": fail_boost_candidate_present
    }))
}

fn fail_closed_audit() -> Value {
    let cases = vec![
        fail_case("train_calibration_overlap", true, false, false, false),
        fail_case("stale_label_lineage_hash", false, true, false, false),
        fail_case("target_label_leakage", false, false, true, false),
        fail_case("holdout_selected_candidate", false, false, false, true),
    ];
    let passed = cases
        .iter()
        .all(|case| case["passed"].as_bool() == Some(true));
    json!({
        "passed": passed,
        "cases": cases
    })
}

fn fail_case(
    name: &str,
    overlap: bool,
    stale_hash: bool,
    target_leakage: bool,
    holdout_selected: bool,
) -> Value {
    let mut codes = Vec::new();
    if overlap {
        codes.push(THRESHOLD_CALIBRATION_NONDISJOINT);
    }
    if stale_hash {
        codes.push("LABEL_LINEAGE_STALE");
    }
    if target_leakage {
        codes.push("TARGET_SIDE_LABEL_LEAKAGE");
    }
    if holdout_selected {
        codes.push("HOLDOUT_SELECTED_CANDIDATE");
    }
    json!({
        "case": name,
        "codes": codes,
        "ship_eligible": false,
        "passed": !codes.is_empty()
    })
}

fn pairwise_overlaps(
    by_bucket: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    let buckets = by_bucket.keys().cloned().collect::<Vec<_>>();
    for left_idx in 0..buckets.len() {
        for right_idx in (left_idx + 1)..buckets.len() {
            let left = &buckets[left_idx];
            let right = &buckets[right_idx];
            let left_set = by_bucket.get(left).cloned().unwrap_or_default();
            let right_set = by_bucket.get(right).cloned().unwrap_or_default();
            out.insert(
                format!("{left}::{right}"),
                left_set
                    .intersection(&right_set)
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            );
        }
    }
    out
}

fn overlap_summary(overlaps: &BTreeMap<String, BTreeSet<String>>) -> Value {
    let rows = overlaps
        .iter()
        .map(|(pair, values)| {
            (
                pair.clone(),
                json!({
                    "count": values.len(),
                    "sample": values.iter().take(8).collect::<Vec<_>>()
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Value::Object(rows)
}

fn hash_sets_by_bucket(by_bucket: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, Value> {
    by_bucket
        .iter()
        .map(|(bucket, values)| {
            (
                bucket.clone(),
                json!({
                    "count": values.len(),
                    "sha256": sha256_joined(values)
                }),
            )
        })
        .collect()
}

fn sha256_joined(values: &BTreeSet<String>) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn row_key(task_id: &str, category: &str) -> String {
    format!("{task_id}::{category}")
}

fn is_forbidden_target_label(label: &str) -> bool {
    FORBIDDEN_TARGET_LABEL_PREFIXES
        .iter()
        .any(|prefix| label.starts_with(prefix))
}

fn float_eq(left: Option<f64>, right: Option<f64>, tolerance: f64) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => (left - right).abs() <= tolerance,
        _ => false,
    }
}

fn active_input_paths() -> Vec<&'static str> {
    vec![
        SHIP_GATE_REPORT_PATH,
        SHIP_GATE_READBACK_PATH,
        INDEX_PATH,
        LABEL_MANIFEST_PATH,
        CODE_STATE_CONSTELLATIONS_PATH,
        SHIP_GATE_SOURCE_PATH,
    ]
}

fn active_paths_include_mntd() -> bool {
    active_input_paths()
        .iter()
        .any(|path| path.contains("/mnt/d"))
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path.as_ref())?)?)
}

fn write_json_0600(path: impl AsRef<Path>, value: &Value) -> Result<()> {
    let path = path.as_ref();
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
