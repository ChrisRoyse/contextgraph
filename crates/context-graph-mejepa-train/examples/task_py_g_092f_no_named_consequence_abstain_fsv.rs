//! TASK-PY-G-092F FSV: Fail assertions without named consequences abstain.
//!
//! This harness does not rerun predictor inference. It reads the retained
//! aiwonder source-of-truth artifacts for the post-fix ship-gate report, the
//! #392 active/abstain score surface, and the #404 hierarchy DB, then verifies
//! the live-safe policy layer:
//!
//! active predicted Fail + no FailLikely named consequence => Abstain.

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL1,
    CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
    CF_MEJEPA_NAMED_PREDICTIONS, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, read_all_chunk_skill_membership_rows, read_all_skill_reverse_index_rows,
};
use context_graph_mejepa_train::failure_mode_hierarchy::{
    CodeStateOutcomeRow, NamedPredictionRow, NamedPredictionVerdict,
};
use context_graph_mejepa_train::label_bridge::load_label_learning_bridge;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const SHIP_GATE_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/ship-gate-fsv.json";
const SHIP_GATE_REPORT_SHA256: &str =
    "8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499";
const TASK_PY_G_392_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-392-abstain-accounting-fsv/run-20260521T124733Z/task_py_g_392_temporal_trajectory_features_fsv.json";
const TASK_PY_G_392_REPORT_SHA256: &str =
    "496a6482781a622f3a0ab9f7a69a2cdc2570cfd73ee84f2cfd8cb6660b00c92e";
const TASK_PY_G_110_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779376526126/task_py_g_110_failure_mode_hierarchy_fsv.json";
const TASK_PY_G_110_REPORT_SHA256: &str =
    "77deb00b1d975bd1b83bf71a4bf48e2069eb1808202a6bf819da5e414f47d879";
const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-092f-no-named-consequence-abstain-fsv";
const REPORT_NAME: &str = "task_py_g_092f_no_named_consequence_abstain_fsv.json";
const READBACK_NAME: &str = "task_py_g_092f_no_named_consequence_abstain_readback.json";
const REQUIRED_ROW_COUNT: usize = 240;
const EXPECTED_RANK_LIFT_NO_NAMED_FALSE_POSITIVES: usize = 13;

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

    let ship_gate_sha256 = sha256_file(SHIP_GATE_REPORT_PATH)?;
    let task_392_sha256 = sha256_file(TASK_PY_G_392_REPORT_PATH)?;
    let task_110_sha256 = sha256_file(TASK_PY_G_110_REPORT_PATH)?;
    ensure!(
        ship_gate_sha256 == SHIP_GATE_REPORT_SHA256,
        "ship-gate report hash drifted: expected {SHIP_GATE_REPORT_SHA256}, got {ship_gate_sha256}"
    );
    ensure!(
        task_392_sha256 == TASK_PY_G_392_REPORT_SHA256,
        "TASK-PY-G-392 report hash drifted: expected {TASK_PY_G_392_REPORT_SHA256}, got {task_392_sha256}"
    );
    ensure!(
        task_110_sha256 == TASK_PY_G_110_REPORT_SHA256,
        "TASK-PY-G-110 report hash drifted: expected {TASK_PY_G_110_REPORT_SHA256}, got {task_110_sha256}"
    );

    let ship_gate_report = read_json(SHIP_GATE_REPORT_PATH)?;
    let task_392_report = read_json(TASK_PY_G_392_REPORT_PATH)?;
    let task_110_report = read_json(TASK_PY_G_110_REPORT_PATH)?;
    ensure!(
        ship_gate_report["all_passed"].as_bool() == Some(true),
        "ship-gate report must have all_passed=true"
    );
    ensure!(
        task_392_report["all_passed"].as_bool() == Some(true),
        "TASK-PY-G-392 report must have all_passed=true"
    );
    ensure!(
        task_110_report["passes"].as_bool() == Some(true),
        "TASK-PY-G-110 report must have passes=true"
    );
    ensure!(
        task_110_report["post_fix_predicted_fail_named_mode_evaluation"]
            ["targetLabelsUsedAsLiveInputs"]
            .as_bool()
            == Some(false),
        "named-mode evidence must not use target labels as live inputs"
    );

    let label_root = value_path(&task_110_report, "label_root")?;
    let db_path = value_path(&task_110_report, "db_path")?;
    ensure_aiwonder_archive_path(&label_root, "label_root")?;
    ensure_aiwonder_archive_path(&db_path, "db_path")?;

    let db = open_db(&db_path)?;
    let cf_counts = cf_counts(&db)?;
    let reverse_rows = read_all_skill_reverse_index_rows(&db)?;
    let membership_rows = read_all_chunk_skill_membership_rows(&db)?;
    let named_rows =
        context_graph_mejepa_train::failure_mode_hierarchy::read_all_named_prediction_rows(&db)?;
    drop(db);

    let code_state_rows = read_code_state_outcomes(&label_root)?;
    let named_context = build_named_context(
        &code_state_rows,
        &reverse_rows,
        &membership_rows,
        &named_rows,
    )?;

    let rank_lift_input = rank_lift_rows_from_ship_gate(&ship_gate_report)?;
    let model = active_abstain_model_from_392(&task_392_report)?;
    let active_abstain_input = active_abstain_rows_from_392(&task_392_report, &ship_gate_report)?;
    let rank_lift_application = apply_no_named_consequence_policy(
        "rank_lift_holdout_before_policy",
        &rank_lift_input,
        &named_context,
        None,
    )?;
    let active_abstain_application = apply_no_named_consequence_policy(
        "task_py_g_392_active_abstain_before_policy",
        &active_abstain_input,
        &named_context,
        Some(model),
    )?;
    let edge_case_audit = edge_case_audit()?;

    let all_passed = rank_lift_application.total_row_count == REQUIRED_ROW_COUNT
        && active_abstain_application.total_row_count == REQUIRED_ROW_COUNT
        && rank_lift_application.no_named_false_positive_fail_count
            == EXPECTED_RANK_LIFT_NO_NAMED_FALSE_POSITIVES
        && rank_lift_application.no_named_false_positive_fail_rows_after_active == 0
        && active_abstain_application.no_named_fail_rows_after_active == 0
        && rank_lift_application.join_rejected_missing_named_context
        && active_abstain_application.join_rejected_missing_named_context
        && edge_case_audit.all_passed
        && !named_context.target_labels_used_as_live_inputs;

    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    let report = json!({
        "artifact_kind": "task_py_g_092f_no_named_consequence_abstain_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-092F",
        "issue": 563,
        "generated_at": Utc::now().to_rfc3339(),
        "run_id": run_id,
        "source_of_truth": {
            "fsv_run_dir": run_dir,
            "report_path": report_path,
            "readback_path": readback_path,
            "ship_gate_report": SHIP_GATE_REPORT_PATH,
            "task_py_g_392_report": TASK_PY_G_392_REPORT_PATH,
            "task_py_g_110_report": TASK_PY_G_110_REPORT_PATH,
            "failure_mode_db": db_path,
            "label_root": label_root
        },
        "trigger_process_outcome": {
            "trigger": "cargo run --release -p context-graph-mejepa-train --example task_py_g_092f_no_named_consequence_abstain_fsv",
            "process": "read retained aiwonder reports -> reopen #404 hierarchy RocksDB -> join rows by task/category/patch -> apply no-named-consequence abstain policy -> write FSV report/readback",
            "outcome": "active predicted Fail rows without FailLikely named consequence evidence are abstained and no longer remain active Fail assertions"
        },
        "pre_state": {
            "source_hashes": {
                "ship_gate_report_sha256": ship_gate_sha256,
                "task_py_g_392_report_sha256": task_392_sha256,
                "task_py_g_110_report_sha256": task_110_sha256
            },
            "hierarchy_cf_counts": cf_counts,
            "named_context": {
                "code_state_count": named_context.by_key.len(),
                "target_labels_used_as_live_inputs": named_context.target_labels_used_as_live_inputs
            },
            "task_py_g_110_post_fix_claim": task_110_report["post_fix_predicted_fail_named_mode_evaluation"]
        },
        "execute_and_inspect": {
            "policy": "if a row is active, predicts Fail, and has no FailLikely named consequence evidence, move it to Abstain/Unknown",
            "policy_uses_target_labels_as_live_inputs": false,
            "rank_lift_policy_application": rank_lift_application,
            "task_py_g_392_active_abstain_policy_application": active_abstain_application
        },
        "edge_case_audit": edge_case_audit,
        "all_passed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback = read_json(&report_path)?;
    ensure!(
        report_readback["all_passed"].as_bool() == Some(true),
        "TASK-PY-G-092F FSV failed: {}",
        serde_json::to_string_pretty(&report_readback)?
    );
    ensure!(
        report_readback["execute_and_inspect"]["rank_lift_policy_application"]
            ["no_named_false_positive_fail_count"]
            .as_u64()
            == Some(EXPECTED_RANK_LIFT_NO_NAMED_FALSE_POSITIVES as u64),
        "readback must list the 13 no-named false-positive Fail rows"
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "rank_lift_no_named_false_positive_fail_count": report_readback["execute_and_inspect"]["rank_lift_policy_application"]["no_named_false_positive_fail_count"],
        "rank_lift_no_named_false_positive_fail_rows_after_active": report_readback["execute_and_inspect"]["rank_lift_policy_application"]["no_named_false_positive_fail_rows_after_active"],
        "task_py_g_392_no_named_fail_rows_after_active": report_readback["execute_and_inspect"]["task_py_g_392_active_abstain_policy_application"]["no_named_fail_rows_after_active"],
        "task_py_g_392_after_metrics": report_readback["execute_and_inspect"]["task_py_g_392_active_abstain_policy_application"]["after_active_metrics"],
        "edge_case_all_passed": report_readback["edge_case_audit"]["all_passed"]
    });
    write_json_0600(&readback_path, &readback)?;
    let readback_sha256 = sha256_file(&readback_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passes": true,
            "report_path": report_path,
            "report_sha256": report_sha256,
            "readback_path": readback_path,
            "readback_sha256": readback_sha256,
            "rank_lift_no_named_false_positive_fail_count": readback["rank_lift_no_named_false_positive_fail_count"],
            "task_py_g_392_no_named_fail_rows_after_active": readback["task_py_g_392_no_named_fail_rows_after_active"],
            "edge_case_all_passed": readback["edge_case_all_passed"]
        }))?
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize)]
struct ActiveAbstainModel {
    abstain_low: f32,
    abstain_high: f32,
    min_active_coverage: f32,
}

#[derive(Debug, Clone)]
struct InputScoreRow {
    task_id: String,
    category: String,
    patch_path: String,
    patch_sha256: String,
    actual_oracle_pass: bool,
    score: f32,
}

#[derive(Debug, Clone, Serialize)]
struct PolicyApplication {
    surface: String,
    total_row_count: usize,
    before_active_count: usize,
    before_abstained_count: usize,
    after_active_count: usize,
    after_abstained_count: usize,
    before_active_fail_count: usize,
    after_active_fail_count: usize,
    no_named_fail_count: usize,
    no_named_fail_rows_after_active: usize,
    no_named_false_positive_fail_count: usize,
    no_named_false_positive_fail_rows_after_active: usize,
    active_coverage_before: f32,
    active_coverage_after: f32,
    active_coverage_floor: Option<f32>,
    coverage_blockers_after: Vec<String>,
    before_active_metrics: MetricBlock,
    after_active_metrics: MetricBlock,
    before_per_category_metrics: BTreeMap<String, MetricBlock>,
    after_per_category_metrics: BTreeMap<String, MetricBlock>,
    no_named_false_positive_fail_rows: Vec<PolicyRow>,
    no_named_fail_rows: Vec<PolicyRow>,
    sample_after_abstained_rows: Vec<PolicyRow>,
    join_rejected_missing_named_context: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PolicyRow {
    key: String,
    task_id: String,
    category: String,
    patch_path: String,
    code_state_key: String,
    actual_oracle_pass: bool,
    score: f32,
    predicted_pass: bool,
    before_active: bool,
    after_active: bool,
    policy_abstained: bool,
    policy_reason: String,
    has_any_named_mode: bool,
    has_fail_likely_named_consequence: bool,
    active_skill_ids: Vec<String>,
    active_higher_ability_ids: Vec<String>,
    named_prediction_ids: Vec<String>,
    fail_likely_named_prediction_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MetricBlock {
    sample_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    predicted_pass_count: usize,
    predicted_fail_count: usize,
    accuracy: f32,
    prediction_oracle_correlation: Option<f32>,
    brier: f32,
    ece: f32,
}

#[derive(Debug, Clone, Serialize)]
struct EdgeCaseAudit {
    named_fail_remains_active: EdgeCase,
    no_named_fail_abstains: EdgeCase,
    score_band_stays_abstained: EdgeCase,
    missing_named_context_rejected: bool,
    all_passed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct EdgeCase {
    name: String,
    before: Value,
    after: Value,
    expected: Value,
    passed: bool,
}

#[derive(Debug, Default, Clone)]
struct NamedContextMap {
    by_key: BTreeMap<String, NamedContext>,
    by_task_category: BTreeMap<(String, String), Vec<String>>,
    target_labels_used_as_live_inputs: bool,
}

#[derive(Debug, Clone, Default)]
struct NamedContext {
    code_state_key: String,
    task_id: String,
    category: String,
    oracle_all_passed: bool,
    active_skill_ids: BTreeSet<String>,
    active_higher_ability_ids: BTreeSet<String>,
    named_prediction_ids: BTreeSet<String>,
    fail_likely_named_prediction_ids: BTreeSet<String>,
}

fn apply_no_named_consequence_policy(
    surface: &str,
    input_rows: &[InputScoreRow],
    context: &NamedContextMap,
    active_model: Option<ActiveAbstainModel>,
) -> Result<PolicyApplication> {
    let policy_rows = build_policy_rows(input_rows, context, active_model)?;
    let before_active_rows = policy_rows
        .iter()
        .filter(|row| row.before_active)
        .cloned()
        .collect::<Vec<_>>();
    let after_active_rows = policy_rows
        .iter()
        .filter(|row| row.after_active)
        .cloned()
        .collect::<Vec<_>>();
    let no_named_fail_rows = policy_rows
        .iter()
        .filter(|row| {
            row.before_active && !row.predicted_pass && !row.has_fail_likely_named_consequence
        })
        .cloned()
        .collect::<Vec<_>>();
    let no_named_false_positive_fail_rows = no_named_fail_rows
        .iter()
        .filter(|row| row.actual_oracle_pass)
        .cloned()
        .collect::<Vec<_>>();
    let no_named_fail_rows_after_active = no_named_fail_rows
        .iter()
        .filter(|row| row.after_active)
        .count();
    let no_named_false_positive_fail_rows_after_active = no_named_false_positive_fail_rows
        .iter()
        .filter(|row| row.after_active)
        .count();
    let coverage_floor = active_model.map(|model| model.min_active_coverage);
    let active_coverage_before = coverage_ratio(before_active_rows.len(), policy_rows.len());
    let active_coverage_after = coverage_ratio(after_active_rows.len(), policy_rows.len());
    let mut coverage_blockers_after = Vec::new();
    if let Some(floor) = coverage_floor {
        if active_coverage_after + 1.0e-6 < floor {
            coverage_blockers_after.push(format!(
                "active_coverage_after_policy_below_floor: {active_coverage_after:.6} < {floor:.6}"
            ));
        }
    }
    let first_state_key = input_rows
        .first()
        .map(|row| context.state_key_for(&row.task_id, &row.category))
        .transpose()?;
    let join_rejected_missing_named_context = first_state_key
        .map(|state_key| {
            let mut missing_context = context.clone();
            missing_context.by_key.remove(&state_key);
            build_policy_rows(input_rows, &missing_context, active_model).is_err()
        })
        .unwrap_or(false);
    Ok(PolicyApplication {
        surface: surface.to_string(),
        total_row_count: policy_rows.len(),
        before_active_count: before_active_rows.len(),
        before_abstained_count: policy_rows.len().saturating_sub(before_active_rows.len()),
        after_active_count: after_active_rows.len(),
        after_abstained_count: policy_rows.len().saturating_sub(after_active_rows.len()),
        before_active_fail_count: before_active_rows
            .iter()
            .filter(|row| !row.predicted_pass)
            .count(),
        after_active_fail_count: after_active_rows
            .iter()
            .filter(|row| !row.predicted_pass)
            .count(),
        no_named_fail_count: no_named_fail_rows.len(),
        no_named_fail_rows_after_active,
        no_named_false_positive_fail_count: no_named_false_positive_fail_rows.len(),
        no_named_false_positive_fail_rows_after_active,
        active_coverage_before,
        active_coverage_after,
        active_coverage_floor: coverage_floor,
        coverage_blockers_after,
        before_active_metrics: metrics_for_policy_rows(&before_active_rows),
        after_active_metrics: metrics_for_policy_rows(&after_active_rows),
        before_per_category_metrics: per_category_metrics_for_policy_rows(&before_active_rows),
        after_per_category_metrics: per_category_metrics_for_policy_rows(&after_active_rows),
        no_named_false_positive_fail_rows,
        no_named_fail_rows,
        sample_after_abstained_rows: policy_rows
            .iter()
            .filter(|row| !row.after_active)
            .take(16)
            .cloned()
            .collect(),
        join_rejected_missing_named_context,
    })
}

fn build_policy_rows(
    input_rows: &[InputScoreRow],
    context: &NamedContextMap,
    active_model: Option<ActiveAbstainModel>,
) -> Result<Vec<PolicyRow>> {
    ensure!(!input_rows.is_empty(), "policy rows must not be empty");
    let mut out = Vec::with_capacity(input_rows.len());
    let mut seen = BTreeSet::new();
    for input in input_rows {
        let key = policy_key(&input.task_id, &input.category, &input.patch_path);
        ensure!(seen.insert(key.clone()), "duplicate policy row {key}");
        let state_key = context.state_key_for(&input.task_id, &input.category)?;
        let state = context
            .by_key
            .get(&state_key)
            .ok_or_else(|| anyhow!("missing named context for state {state_key}"))?;
        ensure!(
            state.oracle_all_passed == input.actual_oracle_pass,
            "oracle label mismatch for {}:{}",
            input.task_id,
            input.category
        );
        let predicted_pass = input.score >= 0.5;
        let before_abstained = active_model
            .map(|model| score_in_abstain_band(input.score, model))
            .unwrap_or(false);
        let before_active = !before_abstained;
        let has_fail_likely = !state.fail_likely_named_prediction_ids.is_empty();
        let policy_abstained = before_active && !predicted_pass && !has_fail_likely;
        let after_active = before_active && !policy_abstained;
        let policy_reason = if before_abstained {
            "score_abstain_band"
        } else if policy_abstained {
            "fail_without_named_consequence"
        } else {
            "active_policy_passed"
        };
        out.push(PolicyRow {
            key,
            task_id: input.task_id.clone(),
            category: input.category.clone(),
            patch_path: normalized_patch_path(&input.patch_path),
            code_state_key: state_key,
            actual_oracle_pass: input.actual_oracle_pass,
            score: input.score,
            predicted_pass,
            before_active,
            after_active,
            policy_abstained,
            policy_reason: policy_reason.to_string(),
            has_any_named_mode: !state.named_prediction_ids.is_empty(),
            has_fail_likely_named_consequence: has_fail_likely,
            active_skill_ids: take_sorted(&state.active_skill_ids, 16),
            active_higher_ability_ids: take_sorted(&state.active_higher_ability_ids, 16),
            named_prediction_ids: take_sorted(&state.named_prediction_ids, 16),
            fail_likely_named_prediction_ids: take_sorted(
                &state.fail_likely_named_prediction_ids,
                16,
            ),
        });
    }
    Ok(out)
}

impl NamedContextMap {
    fn state_key_for(&self, task_id: &str, category: &str) -> Result<String> {
        let key = (task_id.to_string(), category.to_string());
        let candidates = self
            .by_task_category
            .get(&key)
            .ok_or_else(|| anyhow!("no code-state row for {task_id}:{category}"))?;
        if candidates.len() == 1 {
            return Ok(candidates[0].clone());
        }
        if category == "known_good" {
            for suffix in ["|gold", "|base", "|test_patch"] {
                if let Some(value) = candidates
                    .iter()
                    .find(|candidate| candidate.ends_with(suffix))
                {
                    return Ok(value.clone());
                }
            }
        }
        Err(anyhow!(
            "ambiguous code-state rows for {task_id}:{category} ({})",
            candidates.len()
        ))
    }
}

fn build_named_context(
    code_states: &[CodeStateOutcomeRow],
    reverse_rows: &[context_graph_mejepa_train::chunk_skill_membership::SkillReverseIndexRow],
    membership_rows: &[context_graph_mejepa_train::chunk_skill_membership::ChunkSkillMembershipRow],
    named_rows: &[NamedPredictionRow],
) -> Result<NamedContextMap> {
    let mut out = NamedContextMap::default();
    for row in code_states {
        row.validate()?;
        ensure!(
            out.by_key
                .insert(
                    row.code_state_key.clone(),
                    NamedContext {
                        code_state_key: row.code_state_key.clone(),
                        task_id: row.task_instance_id.clone(),
                        category: row.mutation_category.clone(),
                        oracle_all_passed: row.oracle_all_passed,
                        ..NamedContext::default()
                    },
                )
                .is_none(),
            "duplicate code-state key {}",
            row.code_state_key
        );
        out.by_task_category
            .entry((row.task_instance_id.clone(), row.mutation_category.clone()))
            .or_default()
            .push(row.code_state_key.clone());
    }
    for values in out.by_task_category.values_mut() {
        values.sort();
    }
    let mut predictions_by_source = BTreeMap::<String, Vec<&NamedPredictionRow>>::new();
    for row in named_rows {
        row.validate()?;
        out.target_labels_used_as_live_inputs |= row.target_labels_used_as_live_inputs;
        predictions_by_source
            .entry(row.source_hierarchy_id.clone())
            .or_default()
            .push(row);
    }
    ensure!(
        !predictions_by_source.is_empty(),
        "named prediction rows must not be empty"
    );
    for row in reverse_rows {
        row.validate()?;
        let state = out
            .by_key
            .get_mut(&row.code_state_key)
            .ok_or_else(|| anyhow!("reverse index references unknown {}", row.code_state_key))?;
        state.active_skill_ids.insert(row.skill_id.clone());
    }
    for row in membership_rows {
        row.validate()?;
        if row.hierarchy_level < 3 {
            continue;
        }
        let state = out
            .by_key
            .get_mut(&row.code_state_key)
            .ok_or_else(|| anyhow!("membership references unknown {}", row.code_state_key))?;
        state.active_higher_ability_ids.insert(row.skill_id.clone());
    }
    for state in out.by_key.values_mut() {
        debug_assert!(!state.code_state_key.is_empty());
        debug_assert!(!state.task_id.is_empty());
        debug_assert!(!state.category.is_empty());
        for source_id in state
            .active_skill_ids
            .iter()
            .chain(state.active_higher_ability_ids.iter())
        {
            let Some(predictions) = predictions_by_source.get(source_id) else {
                continue;
            };
            for prediction in predictions {
                state
                    .named_prediction_ids
                    .insert(prediction.named_prediction_id.clone());
                if prediction.predicted_verdict == NamedPredictionVerdict::FailLikely {
                    state
                        .fail_likely_named_prediction_ids
                        .insert(prediction.named_prediction_id.clone());
                }
            }
        }
    }
    ensure!(
        !out.target_labels_used_as_live_inputs,
        "named context rows used target labels as live inputs"
    );
    Ok(out)
}

fn rank_lift_rows_from_ship_gate(report: &Value) -> Result<Vec<InputScoreRow>> {
    let rows = report
        .pointer("/execute_and_inspect/holdout_predictions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("ship-gate report missing holdout_predictions"))?;
    ensure!(
        rows.len() == REQUIRED_ROW_COUNT,
        "ship-gate holdout_predictions must contain {REQUIRED_ROW_COUNT} rows"
    );
    let mut out = Vec::with_capacity(rows.len());
    for value in rows {
        out.push(InputScoreRow {
            task_id: value_string(value, "task_id")?,
            category: value_string(value, "category")?,
            patch_path: value_string(value, "patch_path")?,
            patch_sha256: value_string(value, "patch_sha256")?,
            actual_oracle_pass: value_bool(value, "actual_oracle_pass")?,
            score: value_f32(&value["predicted_oracle_pass"], "predicted_oracle_pass")?,
        });
    }
    Ok(out)
}

fn active_abstain_rows_from_392(
    report: &Value,
    ship_gate_report: &Value,
) -> Result<Vec<InputScoreRow>> {
    let patch_sha_by_key = rank_lift_rows_from_ship_gate(ship_gate_report)?
        .into_iter()
        .map(|row| {
            (
                policy_key(&row.task_id, &row.category, &row.patch_path),
                row.patch_sha256,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let rows = report["legal_best_holdout_row_diagnostics"]
        .as_array()
        .ok_or_else(|| {
            anyhow!("TASK-PY-G-392 report missing legal_best_holdout_row_diagnostics")
        })?;
    ensure!(
        rows.len() == REQUIRED_ROW_COUNT,
        "TASK-PY-G-392 row diagnostics must contain {REQUIRED_ROW_COUNT} rows"
    );
    let mut out = Vec::with_capacity(rows.len());
    for value in rows {
        let task_id = value_string(value, "task_id")?;
        let category = value_string(value, "category")?;
        let patch_path = value_string(value, "patch_path")?;
        let key = policy_key(&task_id, &category, &patch_path);
        let patch_sha256 = patch_sha_by_key
            .get(&key)
            .ok_or_else(|| anyhow!("missing patch sha for TASK-PY-G-392 row {key}"))?
            .clone();
        out.push(InputScoreRow {
            task_id,
            category,
            patch_path,
            patch_sha256,
            actual_oracle_pass: value_bool(value, "actual_oracle_pass")?,
            score: value_f32(&value["score_sharpness_score"], "score_sharpness_score")?,
        });
    }
    Ok(out)
}

fn active_abstain_model_from_392(report: &Value) -> Result<ActiveAbstainModel> {
    let model = &report["legal_score_sharpness_abstain_band"]["model"];
    let out = ActiveAbstainModel {
        abstain_low: value_f32(&model["abstain_low"], "abstain_low")?,
        abstain_high: value_f32(&model["abstain_high"], "abstain_high")?,
        min_active_coverage: value_f32(&model["min_active_coverage"], "min_active_coverage")?,
    };
    ensure!(
        out.abstain_low.is_finite()
            && out.abstain_high.is_finite()
            && out.abstain_low < out.abstain_high,
        "invalid #392 active/abstain band"
    );
    Ok(out)
}

fn metrics_for_policy_rows(rows: &[PolicyRow]) -> MetricBlock {
    if rows.is_empty() {
        return MetricBlock {
            sample_count: 0,
            actual_pass_count: 0,
            actual_fail_count: 0,
            predicted_pass_count: 0,
            predicted_fail_count: 0,
            accuracy: 0.0,
            prediction_oracle_correlation: None,
            brier: 0.0,
            ece: 0.0,
        };
    }
    let sample_count = rows.len();
    let actual_pass_count = rows.iter().filter(|row| row.actual_oracle_pass).count();
    let actual_fail_count = sample_count.saturating_sub(actual_pass_count);
    let predicted_pass_count = rows.iter().filter(|row| row.predicted_pass).count();
    let predicted_fail_count = sample_count.saturating_sub(predicted_pass_count);
    let correct = rows
        .iter()
        .filter(|row| row.predicted_pass == row.actual_oracle_pass)
        .count();
    let brier = rows
        .iter()
        .map(|row| {
            let actual = if row.actual_oracle_pass { 1.0 } else { 0.0 };
            let diff = row.score - actual;
            diff * diff
        })
        .sum::<f32>()
        / sample_count as f32;
    MetricBlock {
        sample_count,
        actual_pass_count,
        actual_fail_count,
        predicted_pass_count,
        predicted_fail_count,
        accuracy: correct as f32 / sample_count as f32,
        prediction_oracle_correlation: pearson(rows),
        brier,
        ece: ece(rows),
    }
}

fn per_category_metrics_for_policy_rows(rows: &[PolicyRow]) -> BTreeMap<String, MetricBlock> {
    let mut by_category = BTreeMap::<String, Vec<PolicyRow>>::new();
    for row in rows {
        by_category
            .entry(row.category.clone())
            .or_default()
            .push(row.clone());
    }
    by_category
        .into_iter()
        .map(|(category, rows)| (category, metrics_for_policy_rows(&rows)))
        .collect()
}

fn pearson(rows: &[PolicyRow]) -> Option<f32> {
    if rows.len() < 2 {
        return None;
    }
    let n = rows.len() as f32;
    let mean_x = rows.iter().map(|row| row.score).sum::<f32>() / n;
    let mean_y = rows
        .iter()
        .map(|row| if row.actual_oracle_pass { 1.0 } else { 0.0 })
        .sum::<f32>()
        / n;
    let mut numerator = 0.0;
    let mut denom_x = 0.0;
    let mut denom_y = 0.0;
    for row in rows {
        let dx = row.score - mean_x;
        let y = if row.actual_oracle_pass { 1.0 } else { 0.0 };
        let dy = y - mean_y;
        numerator += dx * dy;
        denom_x += dx * dx;
        denom_y += dy * dy;
    }
    if denom_x <= f32::EPSILON || denom_y <= f32::EPSILON {
        None
    } else {
        Some(numerator / (denom_x.sqrt() * denom_y.sqrt()))
    }
}

fn ece(rows: &[PolicyRow]) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for bin in 0..10 {
        let low = bin as f32 / 10.0;
        let high = (bin + 1) as f32 / 10.0;
        let bin_rows = rows
            .iter()
            .filter(|row| {
                if bin == 9 {
                    row.score >= low && row.score <= high
                } else {
                    row.score >= low && row.score < high
                }
            })
            .collect::<Vec<_>>();
        if bin_rows.is_empty() {
            continue;
        }
        let predicted_pass_probability =
            bin_rows.iter().map(|row| row.score).sum::<f32>() / bin_rows.len() as f32;
        let actual_pass_rate = bin_rows.iter().filter(|row| row.actual_oracle_pass).count() as f32
            / bin_rows.len() as f32;
        total += (bin_rows.len() as f32 / rows.len() as f32)
            * (predicted_pass_probability - actual_pass_rate).abs();
    }
    total
}

fn edge_case_audit() -> Result<EdgeCaseAudit> {
    let named_fail = edge_case(
        "named_fail_remains_active",
        0.10,
        false,
        true,
        false,
        true,
        "active_policy_passed",
    );
    let no_named_fail = edge_case(
        "no_named_fail_abstains",
        0.10,
        false,
        false,
        true,
        false,
        "fail_without_named_consequence",
    );
    let score_band = edge_case(
        "score_band_stays_abstained",
        0.50,
        true,
        true,
        false,
        false,
        "score_abstain_band",
    );
    let missing_named_context_rejected = {
        let row = InputScoreRow {
            task_id: "synthetic_task".to_string(),
            category: "off_by_one".to_string(),
            patch_path: "mutations/synthetic_task/off_by_one.patch".to_string(),
            patch_sha256: "sha256synthetic".to_string(),
            actual_oracle_pass: true,
            score: 0.10,
        };
        build_policy_rows(&[row], &NamedContextMap::default(), None).is_err()
    };
    let all_passed = named_fail.passed
        && no_named_fail.passed
        && score_band.passed
        && missing_named_context_rejected;
    Ok(EdgeCaseAudit {
        named_fail_remains_active: named_fail,
        no_named_fail_abstains: no_named_fail,
        score_band_stays_abstained: score_band,
        missing_named_context_rejected,
        all_passed,
    })
}

fn edge_case(
    name: &str,
    score: f32,
    before_abstained: bool,
    has_fail_likely: bool,
    expected_policy_abstained: bool,
    expected_after_active: bool,
    expected_reason: &str,
) -> EdgeCase {
    let predicted_pass = score >= 0.5;
    let before_active = !before_abstained;
    let policy_abstained = before_active && !predicted_pass && !has_fail_likely;
    let after_active = before_active && !policy_abstained;
    let reason = if before_abstained {
        "score_abstain_band"
    } else if policy_abstained {
        "fail_without_named_consequence"
    } else {
        "active_policy_passed"
    };
    let before = json!({
        "score": score,
        "predicted_pass": predicted_pass,
        "before_active": before_active,
        "has_fail_likely_named_consequence": has_fail_likely
    });
    let after = json!({
        "policy_abstained": policy_abstained,
        "after_active": after_active,
        "policy_reason": reason
    });
    let expected = json!({
        "policy_abstained": expected_policy_abstained,
        "after_active": expected_after_active,
        "policy_reason": expected_reason
    });
    EdgeCase {
        name: name.to_string(),
        before,
        after,
        expected,
        passed: policy_abstained == expected_policy_abstained
            && after_active == expected_after_active
            && reason == expected_reason,
    }
}

#[derive(Debug, Deserialize)]
struct CodeStateOutcomeJsonRow {
    group_key: String,
    task_instance_id: String,
    mutation_category: String,
    docker_label: String,
    oracle_all_passed: bool,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
}

fn read_code_state_outcomes(label_root: &Path) -> Result<Vec<CodeStateOutcomeRow>> {
    let bridge = load_label_learning_bridge(label_root)?;
    let code_state_path =
        resolve_label_path(label_root, &bridge.manifest.paths.code_state_constellations);
    let mut rows = Vec::new();
    for row in read_jsonl::<CodeStateOutcomeJsonRow>(&code_state_path)? {
        rows.push(CodeStateOutcomeRow {
            code_state_key: row.group_key,
            task_instance_id: row.task_instance_id,
            mutation_category: row.mutation_category,
            docker_label: row.docker_label,
            oracle_all_passed: row.oracle_all_passed,
            slot_identity_preserved: row.slot_identity_preserved,
            flat_vector_concat_used: row.flat_vector_concat_used,
        });
    }
    ensure!(
        !rows.is_empty(),
        "code-state outcome file produced no rows: {}",
        code_state_path.display()
    );
    Ok(rows)
}

fn open_db(path: &Path) -> Result<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(false);
    opts.create_missing_column_families(false);
    opts.set_paranoid_checks(true);
    let descriptors = [
        CF_MEJEPA_FAILURE_MODE_LEVEL1,
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
        CF_MEJEPA_NAMED_PREDICTIONS,
        CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
        CF_MEJEPA_SKILL_REVERSE_INDEX,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
    ]
    .into_iter()
    .map(|name| ColumnFamilyDescriptor::new(name, Options::default()))
    .collect::<Vec<_>>();
    Ok(DB::open_cf_descriptors(&opts, path, descriptors)?)
}

fn cf_counts(db: &DB) -> Result<BTreeMap<String, u64>> {
    Ok(BTreeMap::from([
        (
            "level1Groups".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL1)?,
        ),
        (
            "level2Skills".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?,
        ),
        (
            "level3Constellations".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS)?,
        ),
        (
            "namedPredictions".to_string(),
            count_cf_rows(db, CF_MEJEPA_NAMED_PREDICTIONS)?,
        ),
        (
            "chunkSkillMembership".to_string(),
            count_cf_rows(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?,
        ),
        (
            "skillReverseIndex".to_string(),
            count_cf_rows(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?,
        ),
        (
            "skillLifecycleAudit".to_string(),
            count_cf_rows(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?,
        ),
    ]))
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    Ok(serde_json::from_slice(&fs::read(path).with_context(
        || format!("reading JSON {}", path.display()),
    )?)?)
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).map_err(|err| {
            anyhow!(
                "{}:{} failed JSON parse for code-state outcome row: {err}",
                path.display(),
                idx + 1
            )
        })?);
    }
    Ok(rows)
}

fn write_json_0600(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    let bytes = fs::read(path.as_ref())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn value_path(value: &Value, key: &str) -> Result<PathBuf> {
    value[key]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("expected path string field {key}"))
}

fn value_string(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("expected string field {key}"))
}

fn value_bool(value: &Value, key: &str) -> Result<bool> {
    value[key]
        .as_bool()
        .ok_or_else(|| anyhow!("expected bool field {key}"))
}

fn value_f32(value: &Value, name: &str) -> Result<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .ok_or_else(|| anyhow!("expected numeric field {name}, got {value}"))
}

fn ensure_aiwonder_archive_path(path: &Path, name: &str) -> Result<()> {
    ensure!(
        path.to_string_lossy()
            .starts_with("/zfs/archive/contextgraph/"),
        "{name} must be on aiwonder archive root: {}",
        path.display()
    );
    Ok(())
}

fn resolve_label_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn policy_key(task_id: &str, category: &str, patch_path: &str) -> String {
    format!(
        "{task_id}::{category}::{}",
        normalized_patch_path(patch_path)
    )
}

fn normalized_patch_path(path: &str) -> String {
    let prefix = format!("{CORPUS_DIR}/");
    path.strip_prefix(&prefix).unwrap_or(path).to_string()
}

fn score_in_abstain_band(score: f32, model: ActiveAbstainModel) -> bool {
    score >= model.abstain_low && score <= model.abstain_high
}

fn coverage_ratio(active: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        active as f32 / total as f32
    }
}

fn take_sorted(values: &BTreeSet<String>, limit: usize) -> Vec<String> {
    values.iter().take(limit).cloned().collect()
}
