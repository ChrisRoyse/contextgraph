#![recursion_limit = "256"]

//! TASK-PY-G-392 temporal constellation trajectory diagnostic FSV.
//!
//! This deliberately does not rerun the CUDA predictor. It replays the
//! current aiwonder row traces and derives ordered base -> candidate -> test
//! trajectory features from slot-preserving code-state constellations to test
//! whether temporal/multi-chunk evidence can legally repair true mixed cells.

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    read_all_chunk_skill_membership_rows, read_all_level2_skill_rows, ChunkSkillMembershipRow,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    skill_candidate_kind_for_row, skill_genericity_score_for_steps, Level2SkillRow,
    SkillCandidateKind,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const LABEL_ROOT: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split";
const CODE_STATE_CONSTELLATIONS_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split/code_state_constellations.jsonl";
const FILE_PATH_CONSTELLATIONS_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split/file_path_constellations.jsonl";
const HIERARCHY_DB_PATH: &str =
    "/zfs/archive/contextgraph/fsv/task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/skill-corpus.rocksdb";
const HIERARCHY_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/task_py_g_118_aiwonder_skill_materialization_fsv.json";
const HIERARCHY_DB_PATH_ENV: &str = "CG_MEJEPA_HIERARCHY_DB_PATH";
const HIERARCHY_REPORT_PATH_ENV: &str = "CG_MEJEPA_HIERARCHY_REPORT_PATH";
const BASELINE_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/ship-gate-fsv.json";
const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv";
const TASK_SKILL_002_FSV_ROOT_ENV: &str = "CG_MEJEPA_TASK_SKILL_002_FSV_ROOT";
const REPORT_NAME: &str = "task_py_g_392_temporal_trajectory_features_fsv.json";
const READBACK_NAME: &str = "task_py_g_392_temporal_trajectory_features_readback.json";
const REQUIRED_SPLIT_COUNTS: (usize, usize, usize) = (1920, 240, 240);
const REQUIRED_EVAL_SPLIT_COUNT: usize = 240;
const REQUIRED_CATEGORIES: [&str; 8] = [
    "known_good",
    "subtle_flip",
    "off_by_one",
    "swap_variable",
    "delete_test_call",
    "wrong_file",
    "over_engineer",
    "compile_error",
];
const TRUE_MIXED_CATEGORIES: [&str; 3] = ["off_by_one", "subtle_flip", "swap_variable"];
const SCORE_TRACE_FIELDS: [&str; 6] = [
    "calibration_predictions",
    "calibration_rank_safe_predictions",
    "calibration_rank_lift_predictions",
    "holdout_raw_predictions",
    "holdout_rank_safe_predictions",
    "holdout_predictions",
];
const TARGET_LABEL_PREFIXES: [&str; 5] = [
    "oracle:",
    "docker:",
    "test_phase:",
    "code_state_outcome:",
    "leakage_policy:",
];
const HASH_FEATURES: usize = 256;
const REPO_HASH_FEATURES: usize = 64;
const PATH_HASH_FEATURES: usize = 128;
const ECE_BIN_COUNT: usize = 10;
const BASELINE_536_CORRELATION: f32 = 0.794_219;
const BASELINE_536_BRIER: f32 = 0.092_373;
const BASELINE_536_ECE: f32 = 0.024_032;
const DEGENERATE_RECALL_TARGET: f32 = 0.95;
const DEGENERATE_CLASS_CONFIDENCE_TARGET: f32 = 0.95;
const DEGENERATE_BRIER_MAX: f32 = 0.05;
const DEGENERATE_ECE_MAX: f32 = 0.05;
const SELECTION_ECE_WEIGHT: f32 = 0.25;
const SKILL_POLICY_OBJECTIVE_IMPROVEMENT_MARGIN: f32 = 1.0e-3;
const SKILL_POLICY_ECE_REGRESSION_CAP: f32 = 6.0e-3;
const FINAL_BIN_GRID_OBJECTIVE_EPSILON: f32 = 1.0e-6;
const FINAL_BIN_LEGAL_BEST_OBJECTIVE_MARGIN: f32 = 1.0e-3;
const FINAL_BIN_NONREGRESSION_EPSILON: f32 = 1.0e-6;
const SLOT_IDS: [&str; 12] = [
    "e1", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e6", "e7", "e8", "e9",
];
const SLOT_SCALAR_NAMES: [&str; 5] = [
    "abs_mean",
    "entropy_proxy",
    "norm_l2",
    "std",
    "zero_fraction",
];
const CURRENT_RBF: Candidate = Candidate::CategoryLocalRbf {
    tau: 20.0,
    fail_boost: 1.5,
};

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
    let fsv_root = fsv_root();
    let run_dir = Path::new(&fsv_root).join(&run_id);
    fs::create_dir_all(&run_dir)?;

    ensure!(
        !active_paths_include_mntd(),
        "active diagnostic inputs must not use /mnt/d"
    );

    let baseline_report = read_json(BASELINE_REPORT_PATH)?;
    ensure!(
        baseline_report["all_passed"].as_bool() == Some(true),
        "baseline #536 FSV must have all_passed=true"
    );
    let baseline_report_sha256 = sha256_file(BASELINE_REPORT_PATH)?;
    let baseline_final_metrics =
        metrics_from_report(&baseline_report, &["execute_and_inspect", "final_metrics"])?;
    let baseline_per_category = per_category_metrics_from_report(&baseline_report)?;
    let baseline_holdout_rows =
        prediction_rows_from_report(&baseline_report, "holdout_predictions")
            .context("parse baseline holdout row trace")?;
    let baseline_row_trace_comparison = reproduction_report(
        &baseline_final_metrics,
        &metrics_for(&baseline_holdout_rows),
    );

    let index = read_index(INDEX_PATH)?;
    let split_counts = split_counts(&index.entries);
    ensure!(
        (
            split_counts.get("train").copied().unwrap_or_default(),
            split_counts.get("calibration").copied().unwrap_or_default(),
            split_counts.get("holdout").copied().unwrap_or_default(),
        ) == REQUIRED_SPLIT_COUNTS,
        "loaded corpus split counts must match 1920/240/240"
    );
    ensure_category_grid(&index.entries, "calibration")?;
    ensure_category_grid(&index.entries, "holdout")?;

    let base_feature_table = load_feature_table(&index.entries, FeatureMode::CodeState, false)?;
    let no_skill_feature_table =
        load_feature_table(&index.entries, FeatureMode::FileTrajectory, false)?;
    let raw_skill_feature_table = load_feature_table(
        &index.entries,
        FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::RawCurrent),
        false,
    )?;
    let failure_skill_feature_table = load_feature_table(
        &index.entries,
        FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::FailureOnly),
        false,
    )?;
    let pass_stability_skill_feature_table = load_feature_table(
        &index.entries,
        FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::PassStabilityOnly),
        false,
    )?;
    let context_skill_feature_table = load_feature_table(
        &index.entries,
        FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::ContextOnly),
        false,
    )?;
    let usefulness_feature_table = load_feature_table(
        &index.entries,
        FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::UsefulnessCombined),
        false,
    )?;
    let skill_policy_ablation = skill_policy_ablation_report(
        &base_feature_table,
        &no_skill_feature_table,
        &raw_skill_feature_table,
        &failure_skill_feature_table,
        &pass_stability_skill_feature_table,
        &context_skill_feature_table,
        &usefulness_feature_table,
        &baseline_final_metrics,
    )?;
    let feature_table = selected_skill_feature_table(
        &skill_policy_ablation,
        &raw_skill_feature_table,
        &failure_skill_feature_table,
        &pass_stability_skill_feature_table,
        &context_skill_feature_table,
        &usefulness_feature_table,
    );
    let train_counts = train_counts_by_category(&feature_table.rows);
    let class_support = class_support_by_category(&feature_table.rows);

    let current_trajectory_policy = candidate_split_report(&base_feature_table, CURRENT_RBF)?;
    let baseline_comparison = reproduction_report(
        &baseline_final_metrics,
        &current_trajectory_policy.holdout.metrics,
    );

    let candidate_results = rank_candidate_sweep(feature_table)?;
    let best_legal_global = candidate_results
        .first()
        .ok_or_else(|| anyhow!("candidate sweep returned no candidates"))?
        .clone();
    let legal_threshold = select_threshold_repair(
        feature_table,
        ThresholdSelectionSplit::Calibration,
        ThresholdRepairSpec::default(),
    )?;
    let score_traces = score_trace_rows(&baseline_report)?;
    let legal_affine_calibration = select_affine_calibration_repair(feature_table)?;
    let legal_prediction_blend = select_prediction_blend_repair(feature_table, &score_traces)?;
    let legal_logit_calibration = select_logit_calibration_repair(feature_table)?;
    let legal_mean_bias_calibration = select_mean_bias_calibration_repair(feature_table)?;
    let legal_ece_focused_mean_bias_calibration =
        select_ece_focused_mean_bias_calibration_repair(feature_table)?;
    let legal_cross_window_stability_calibration =
        select_cross_window_stability_calibration_repair(feature_table)?;
    let legal_cause_residual_calibration =
        select_cause_residual_calibration_repair(feature_table, &legal_mean_bias_calibration)?;
    let legal_cause_bin_calibration =
        select_cause_bin_calibration_repair(feature_table, &legal_cause_residual_calibration)?;
    let legal_score_sharpness_calibration =
        select_score_sharpness_calibration_repair(feature_table, &legal_cause_bin_calibration)?;
    let legal_score_sharpness_logit_calibration = select_score_sharpness_logit_calibration_repair(
        feature_table,
        &legal_score_sharpness_calibration,
    )?;
    let legal_score_sharpness_abstain_band = select_score_sharpness_abstain_band_repair(
        feature_table,
        &legal_score_sharpness_calibration,
    )?;
    let abstain_candidate_found = !legal_score_sharpness_abstain_band.selection_forbidden
        && legal_score_sharpness_abstain_band.blockers.is_empty();
    let legal_cause_band_calibration =
        select_cause_band_calibration_repair(feature_table, &legal_cause_bin_calibration)?;
    let legal_final_bin_calibration =
        select_final_bin_calibration_repair(feature_table, &legal_cause_bin_calibration)?;
    let legal_support_shrink_calibration =
        select_support_shrink_calibration_repair(feature_table, &legal_mean_bias_calibration)?;
    let legal_evidence_consequence_calibration = select_evidence_consequence_calibration_repair(
        feature_table,
        &legal_mean_bias_calibration,
    )?;
    let legal_bin_consequence_calibration = select_bin_consequence_calibration_repair(
        feature_table,
        &legal_evidence_consequence_calibration,
    )?;
    let legal_probability_calibration = select_probability_calibration_repair(feature_table)?;
    let forbidden_threshold = select_threshold_repair(
        feature_table,
        ThresholdSelectionSplit::Holdout,
        ThresholdRepairSpec::default(),
    )?;
    ensure!(
        forbidden_threshold.selection_forbidden,
        "holdout-selected threshold repair must be marked forbidden"
    );

    let legal_candidate_reports = LegalCandidateReports {
        threshold: &legal_threshold,
        affine_calibration: &legal_affine_calibration,
        prediction_blend: &legal_prediction_blend,
        logit_calibration: &legal_logit_calibration,
        mean_bias_calibration: &legal_mean_bias_calibration,
        cross_window_stability_calibration: &legal_cross_window_stability_calibration,
        cause_residual_calibration: &legal_cause_residual_calibration,
        cause_bin_calibration: &legal_cause_bin_calibration,
        score_sharpness_calibration: &legal_score_sharpness_calibration,
        score_sharpness_logit_calibration: &legal_score_sharpness_logit_calibration,
        cause_band_calibration: &legal_cause_band_calibration,
        final_bin_calibration: &legal_final_bin_calibration,
        support_shrink_calibration: &legal_support_shrink_calibration,
        evidence_consequence_calibration: &legal_evidence_consequence_calibration,
        bin_consequence_calibration: &legal_bin_consequence_calibration,
        probability_calibration: &legal_probability_calibration,
    };
    let legal_best_candidate = legal_best_candidate(&best_legal_global, &legal_candidate_reports);
    let legal_best_holdout_rows = legal_best_holdout_rows(
        feature_table,
        &legal_best_candidate,
        &best_legal_global,
        &legal_threshold,
        &legal_affine_calibration,
        &legal_prediction_blend,
        &score_traces,
        &legal_logit_calibration,
        &legal_mean_bias_calibration,
        &legal_cross_window_stability_calibration,
        &legal_cause_residual_calibration,
        &legal_cause_bin_calibration,
        &legal_score_sharpness_calibration,
        &legal_score_sharpness_logit_calibration,
        &legal_cause_band_calibration,
        &legal_final_bin_calibration,
        &legal_support_shrink_calibration,
        &legal_evidence_consequence_calibration,
        &legal_bin_consequence_calibration,
        &legal_probability_calibration,
    )?;
    let legal_best_row_reproduction = reproduction_report(
        &legal_best_candidate.holdout.metrics,
        &metrics_for(&legal_best_holdout_rows),
    );
    let legal_best_row_audit = legal_best_row_audit(
        feature_table,
        &legal_best_candidate,
        &legal_best_holdout_rows,
        &legal_best_row_reproduction,
    );
    let legal_best_ece_breakdown = ece_breakdown_report(&legal_best_holdout_rows);
    let legal_best_holdout_row_diagnostics = legal_best_holdout_row_diagnostics(
        feature_table,
        &legal_best_holdout_rows,
        &legal_score_sharpness_calibration.model,
    )?;
    let legal_best_ece_breakdown_audit = ece_breakdown_audit(
        &legal_best_candidate.holdout.metrics,
        &legal_best_ece_breakdown,
    );
    let bin_consequence_candidate_audit =
        bin_consequence_candidate_audit(feature_table, &legal_bin_consequence_calibration)?;
    let directional_signal =
        directional_signal_report(&baseline_final_metrics, &legal_best_candidate);
    let score_fusion = score_fusion_report(&baseline_report)?;
    let typed_blocker = typed_blocker(&legal_best_candidate, &forbidden_threshold);
    let edge_cases = edge_case_audit(
        feature_table,
        &forbidden_threshold,
        &legal_affine_calibration,
        &legal_support_shrink_calibration,
        &legal_evidence_consequence_calibration,
        &legal_bin_consequence_calibration,
        &legal_cause_residual_calibration,
        &legal_cause_bin_calibration,
        &legal_score_sharpness_calibration,
        &legal_cause_band_calibration,
    )?;

    let promotable_candidate_found = legal_best_candidate.blockers.is_empty();
    let all_passed = edge_cases.ordered_replay_passed
        && baseline_row_trace_comparison.passed
        && edge_cases.non_monotonic_order_fails_closed
        && edge_cases.missing_code_state_row_fails_closed
        && edge_cases.missing_file_path_row_fails_closed
        && edge_cases.hierarchy_db_readback_passed
        && directional_signal.passed
        && legal_best_row_reproduction.passed
        && legal_best_row_audit.all_passed
        && legal_best_ece_breakdown_audit.all_passed
        && legal_cross_window_stability_calibration
            .model
            .bias_stats
            .values()
            .any(|stats| stats.valid_window_count >= 2)
        && legal_cause_residual_calibration.model.feature_count > 0
        && !legal_cause_bin_calibration.model.key_stats.is_empty()
        && legal_cause_bin_calibration
            .calibration_application
            .applied_row_count
            > 0
        && legal_cause_bin_calibration
            .holdout_application
            .applied_row_count
            > 0
        && legal_score_sharpness_calibration
            .calibration_application
            .low_tail_shaped_row_count
            + legal_score_sharpness_calibration
                .calibration_application
                .high_tail_shaped_row_count
            > 0
        && legal_score_sharpness_calibration
            .holdout_application
            .low_tail_shaped_row_count
            + legal_score_sharpness_calibration
                .holdout_application
                .high_tail_shaped_row_count
            > 0
        && !legal_score_sharpness_abstain_band
            .calibration_application
            .per_category
            .is_empty()
        && !legal_score_sharpness_abstain_band
            .holdout_application
            .per_category
            .is_empty()
        && legal_cause_band_calibration
            .calibration_application
            .applied_row_count
            > 0
        && legal_cause_band_calibration
            .holdout_application
            .applied_row_count
            > 0
        && bin_consequence_candidate_audit.all_passed
        && (promotable_candidate_found || typed_blocker.accepted)
        && score_fusion.edge_cases.all_passed
        && skill_policy_ablation.all_passed
        && edge_cases.all_passed
        && !active_paths_include_mntd();
    let report = json!({
        "schema_version": 1,
        "issue": 392,
        "run_id": run_id,
        "started_at": started_at.to_rfc3339(),
        "all_passed": all_passed,
        "promotable_candidate_found": promotable_candidate_found,
        "abstain_candidate_found": abstain_candidate_found,
        "selection_policy": {
            "objective": "brier + SELECTION_ECE_WEIGHT * ece - 0.10 * pearson_correlation",
            "selection_ece_weight": SELECTION_ECE_WEIGHT,
            "selected_on": "calibration",
        },
        "active_input_paths": active_input_paths(),
        "baseline_report_path": BASELINE_REPORT_PATH,
        "baseline_report_sha256": baseline_report_sha256,
        "feature_source": {
            "index_path": INDEX_PATH,
            "label_root": LABEL_ROOT,
            "code_state_constellations_path": CODE_STATE_CONSTELLATIONS_PATH,
            "file_path_constellations_path": FILE_PATH_CONSTELLATIONS_PATH,
            "hierarchy_db_path": hierarchy_db_path(),
            "hierarchy_report_path": hierarchy_report_path(),
            "feature_policy": "ordered base -> candidate -> test_patch trajectory features over slot-preserving code-state, file-path, and selected skill/ability policy; per-slot deltas, named scalar cross-slot relationships, file movement, live label transitions, and usefulness-weighted membership transitions; target-side oracle/docker/test/leakage labels excluded; train split standardization only",
            "active_skill_policy": skill_policy_ablation.selected_policy.clone(),
            "feature_dim": feature_table.feature_dim,
            "baseline_feature_dim_code_state_only": base_feature_table.feature_dim,
            "trajectory_order": ["base", "candidate_workspace", "test_patch"],
            "known_good_candidate_workspace": "gold",
            "loaded_rows": feature_table.rows.len(),
            "split_counts": split_counts,
            "train_counts_by_category": train_counts,
            "class_support_by_category": class_support,
            "skill_coverage": feature_table.skill_coverage.clone(),
            "target_label_prefixes_excluded": TARGET_LABEL_PREFIXES,
        },
        "task_skill_002": {
            "issue": 554,
            "policy": "usefulness-weighted ordered skills from train-only skill outcome profiles; target-side labels never enter live row features",
            "skill_policy_ablation": skill_policy_ablation,
            "blanket_expansion_negative_control": feature_table.skill_coverage.as_ref().map(|coverage| &coverage.usefulness_policy.blanket_expansion_negative_control),
        },
        "baseline_536": {
            "final_metrics": baseline_final_metrics,
            "per_category_metrics": baseline_per_category,
            "holdout_prediction_count": baseline_holdout_rows.len(),
            "row_trace_comparison": baseline_row_trace_comparison,
        },
        "current_trajectory_policy": {
            "candidate": CURRENT_RBF,
            "calibration": current_trajectory_policy.calibration,
            "holdout": current_trajectory_policy.holdout,
            "baseline_comparison": baseline_comparison,
        },
        "candidate_results": candidate_results,
        "legal_best_candidate": legal_best_candidate,
        "legal_best_row_reproduction": legal_best_row_reproduction,
        "legal_best_row_audit": legal_best_row_audit,
        "legal_best_holdout_row_diagnostics": legal_best_holdout_row_diagnostics,
        "legal_best_ece_breakdown": legal_best_ece_breakdown,
        "legal_best_ece_breakdown_audit": legal_best_ece_breakdown_audit,
        "bin_consequence_candidate_audit": bin_consequence_candidate_audit,
        "legal_affine_calibration": legal_affine_calibration,
        "legal_prediction_blend": legal_prediction_blend,
        "legal_logit_calibration": legal_logit_calibration,
        "legal_mean_bias_calibration": legal_mean_bias_calibration,
        "legal_ece_focused_mean_bias_calibration": legal_ece_focused_mean_bias_calibration,
        "legal_cross_window_stability_calibration": legal_cross_window_stability_calibration,
        "legal_cause_residual_calibration": legal_cause_residual_calibration,
        "legal_cause_bin_calibration": legal_cause_bin_calibration,
        "legal_score_sharpness_calibration": legal_score_sharpness_calibration,
        "legal_score_sharpness_logit_calibration": legal_score_sharpness_logit_calibration,
        "legal_score_sharpness_abstain_band": legal_score_sharpness_abstain_band,
        "legal_cause_band_calibration": legal_cause_band_calibration,
        "legal_final_bin_calibration": legal_final_bin_calibration,
        "legal_support_shrink_calibration": legal_support_shrink_calibration,
        "legal_evidence_consequence_calibration": legal_evidence_consequence_calibration,
        "legal_bin_consequence_calibration": legal_bin_consequence_calibration,
        "legal_probability_calibration": legal_probability_calibration,
        "directional_signal": directional_signal,
        "static_score_fusion_reference": score_fusion,
        "forbidden_holdout_oracle": forbidden_threshold,
        "typed_blocker": typed_blocker,
        "edge_cases": edge_cases,
        "elapsed_seconds": (Utc::now() - started_at).to_std().unwrap_or_default().as_secs_f32(),
    });

    let report_path = run_dir.join(REPORT_NAME);
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback = read_json(&report_path)?;
    ensure!(
        report_readback["all_passed"].as_bool() == Some(true),
        "diagnostic FSV failed: {}",
        serde_json::to_string_pretty(&report_readback["typed_blocker"])?
    );
    ensure!(
        report_readback["legal_best_row_audit"]["all_passed"].as_bool() == Some(true),
        "legal best row audit failed closed"
    );
    ensure!(
        report_readback["legal_best_ece_breakdown_audit"]["all_passed"].as_bool() == Some(true),
        "legal best ECE breakdown audit failed closed"
    );
    ensure!(
        report_readback["bin_consequence_candidate_audit"]["all_passed"].as_bool() == Some(true),
        "bin-consequence candidate audit failed closed"
    );
    let worst_ece_bins = report_readback["legal_best_ece_breakdown"]["worst_bins"]
        .as_array()
        .ok_or_else(|| anyhow!("legal best ECE worst_bins missing or not an array"))?;
    ensure!(
        !worst_ece_bins.is_empty(),
        "legal best ECE worst_bins must be non-empty for non-empty holdout rows"
    );
    let worst_ece_bin = worst_ece_bins[0].clone();
    let row_diagnostics = report_readback["legal_best_holdout_row_diagnostics"]
        .as_array()
        .ok_or_else(|| anyhow!("legal best holdout row diagnostics missing or not an array"))?;
    ensure!(
        row_diagnostics.len()
            == report_readback["legal_best_candidate"]["holdout"]["metrics"]["sample_count"]
                .as_u64()
                .unwrap_or_default() as usize,
        "legal best holdout row diagnostics must cover every holdout row"
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "readback_blocker_code": report_readback["typed_blocker"]["code"],
        "readback_current_trajectory_correlation": report_readback["current_trajectory_policy"]["holdout"]["metrics"]["prediction_oracle_correlation"],
        "readback_legal_best_correlation": report_readback["legal_best_candidate"]["holdout"]["metrics"]["prediction_oracle_correlation"],
        "readback_legal_best_brier": report_readback["legal_best_candidate"]["holdout"]["metrics"]["brier"],
        "readback_legal_best_ece": report_readback["legal_best_candidate"]["holdout"]["metrics"]["ece"],
        "readback_legal_best_row_audit": report_readback["legal_best_row_audit"],
        "readback_legal_best_ece_aggregate": report_readback["legal_best_ece_breakdown"]["aggregate_ece"],
        "readback_legal_best_ece_from_bins": report_readback["legal_best_ece_breakdown"]["ece_from_bins"],
        "readback_legal_best_ece_non_empty_bin_count": report_readback["legal_best_ece_breakdown"]["non_empty_bin_count"],
        "readback_legal_best_ece_breakdown_audit": report_readback["legal_best_ece_breakdown_audit"],
        "readback_legal_best_worst_ece_bin": worst_ece_bin,
        "readback_legal_best_holdout_row_diagnostic_count": row_diagnostics.len(),
        "readback_bin_consequence_candidate_audit": report_readback["bin_consequence_candidate_audit"],
        "readback_directional_signal_passed": report_readback["directional_signal"]["passed"],
        "readback_legal_best_blockers": report_readback["legal_best_candidate"]["blockers"],
        "readback_forbidden_reason": report_readback["forbidden_holdout_oracle"]["forbidden_reason"],
    });
    let readback_path = run_dir.join(READBACK_NAME);
    write_json_0600(&readback_path, &readback)?;
    let readback_sha256 = sha256_file(&readback_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "summary": {
                "all_passed": report_readback["all_passed"],
                "typed_blocker": report_readback["typed_blocker"],
                "legal_best_kind": report_readback["legal_best_candidate"]["kind"],
                "legal_best_metrics": report_readback["legal_best_candidate"]["holdout"]["metrics"],
                "worst_ece_bin": worst_ece_bin,
            },
            "readback": readback,
            "hashes": {
                "report_sha256": report_sha256,
                "readback_sha256": readback_sha256,
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
    patch_path: String,
    repo: String,
    task_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CodeStateConstellationRow {
    task_instance_id: String,
    mutation_category: String,
    workspace_state: String,
    oracle_all_passed: bool,
    #[serde(default)]
    relative_path: Option<String>,
    #[serde(default)]
    chunk_count: usize,
    #[serde(default)]
    file_count: usize,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    slot_feature_means: BTreeMap<String, BTreeMap<String, f64>>,
    #[serde(default)]
    group_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    pair_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    slot_label_counts_top: BTreeMap<String, u64>,
    #[serde(default)]
    surface_counts_top: BTreeMap<String, u64>,
}

type FilePathConstellationRow = CodeStateConstellationRow;

#[derive(Debug, Clone, Copy)]
enum FeatureMode {
    CodeState,
    FileTrajectory,
    FileSkillTrajectory(SkillFeaturePolicy),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SkillFeaturePolicy {
    RawCurrent,
    UsefulnessCombined,
    FailureOnly,
    PassStabilityOnly,
    ContextOnly,
}

impl SkillFeaturePolicy {
    fn label(self) -> &'static str {
        match self {
            SkillFeaturePolicy::RawCurrent => "current_raw_skill_surface",
            SkillFeaturePolicy::UsefulnessCombined => "usefulness_weighted_combined",
            SkillFeaturePolicy::FailureOnly => "failure_only",
            SkillFeaturePolicy::PassStabilityOnly => "pass_stability_only",
            SkillFeaturePolicy::ContextOnly => "context_negative_evidence_only",
        }
    }

    fn included_kinds(self) -> &'static [&'static str] {
        match self {
            SkillFeaturePolicy::RawCurrent => &[],
            SkillFeaturePolicy::UsefulnessCombined => &[
                "failure_skill",
                "pass_stability_skill",
                "context_negative_evidence",
            ],
            SkillFeaturePolicy::FailureOnly => &["failure_skill"],
            SkillFeaturePolicy::PassStabilityOnly => &["pass_stability_skill"],
            SkillFeaturePolicy::ContextOnly => &["context_negative_evidence"],
        }
    }
}

#[derive(Debug, Clone)]
struct FeatureRow {
    task_id: String,
    category: String,
    bucket: String,
    actual_oracle_pass: bool,
    patch_path: String,
    repo: String,
    evidence_keys: Vec<String>,
    cause_features: Vec<f32>,
    standardized_features: Vec<f32>,
}

#[derive(Debug)]
struct FeatureTable {
    rows: Vec<FeatureRow>,
    feature_dim: usize,
    train_indices_by_category: BTreeMap<String, Vec<usize>>,
    skill_coverage: Option<SkillCoverageReport>,
}

#[derive(Debug, Clone, Default)]
struct SkillStateSummary {
    level2_skill_ids: BTreeSet<String>,
    higher_ability_ids: BTreeSet<String>,
    chunk_ids: BTreeSet<String>,
    file_paths: BTreeSet<String>,
    accepted_label_ids: BTreeSet<String>,
    group_ids: BTreeSet<String>,
    membership_count: usize,
    ordered_step_count: usize,
    multi_step_membership_count: usize,
    membership_score_sum: f64,
    membership_score_max: f64,
    usefulness_weight_sum: f64,
    usefulness_weight_max: f64,
    kind_summaries: BTreeMap<String, SkillKindStateSummary>,
}

#[derive(Debug, Clone, Default)]
struct SkillKindStateSummary {
    level2_skill_ids: BTreeSet<String>,
    higher_ability_ids: BTreeSet<String>,
    chunk_ids: BTreeSet<String>,
    file_paths: BTreeSet<String>,
    accepted_label_ids: BTreeSet<String>,
    group_ids: BTreeSet<String>,
    membership_count: usize,
    ordered_step_count: usize,
    multi_step_membership_count: usize,
    raw_membership_score_sum: f64,
    raw_membership_score_max: f64,
    usefulness_weight_sum: f64,
    usefulness_weight_max: f64,
}

#[derive(Debug, Clone)]
struct SkillFeatureIndex {
    by_code_state: BTreeMap<String, SkillStateSummary>,
    level2_skill_count: usize,
    membership_count: usize,
    hierarchy_report_sha256: String,
    coverage: SkillCoverageReport,
}

#[derive(Debug, Clone, Serialize)]
struct SkillCoverageReport {
    expected_code_state_count: usize,
    covered_code_state_count: usize,
    uncovered_code_state_count: usize,
    uncovered_code_state_sample: Vec<String>,
    usefulness_policy: SkillUsefulnessPolicyReport,
}

#[derive(Debug, Clone, Serialize)]
struct SkillUsefulnessPolicyReport {
    scoring_split: String,
    train_skill_profile_count: usize,
    skill_kind_counts: BTreeMap<String, usize>,
    zero_train_support_skill_count: usize,
    blanket_expansion_negative_control: BlanketExpansionNegativeControl,
}

#[derive(Debug, Clone, Serialize)]
struct BlanketExpansionNegativeControl {
    coverage_before: String,
    coverage_after: String,
    correlation_before: f32,
    correlation_after: f32,
    ece_before: f32,
    ece_after: f32,
    selected: bool,
    reason: String,
}

#[derive(Debug, Clone)]
struct SkillUsefulnessDescriptor {
    kind_label: String,
    split_selection_weight: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PredictionRow {
    task_id: String,
    category: String,
    actual_oracle_pass: bool,
    predicted_oracle_pass: f32,
    predicted_pass: bool,
    correct: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Candidate {
    CategoryLocalRbf { tau: f32, fail_boost: f32 },
    CategoryLocalKnn { k: usize, fail_boost: f32 },
}

#[derive(Debug, Clone, Serialize)]
struct SplitMetrics {
    candidate: Candidate,
    calibration: SplitReport,
    holdout: SplitReport,
}

#[derive(Debug, Clone, Serialize)]
struct SplitReport {
    metrics: MetricBlock,
    per_category_metrics: BTreeMap<String, MetricBlock>,
}

#[derive(Debug, Clone, Serialize)]
struct RankedCandidate {
    rank: usize,
    candidate: Candidate,
    selection_objective: f32,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct ScoreWeights {
    raw: f32,
    rank_safe: f32,
    rank_lift: f32,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreFusionCandidate {
    rank: usize,
    policy: String,
    default_weights: ScoreWeights,
    category_overrides: BTreeMap<String, ScoreWeights>,
    selection_objective: f32,
    holdout_objective: f32,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ForbiddenScoreFusionOracle {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: String,
    candidate: ScoreFusionCandidate,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreFusionReport {
    row_trace_source_path: String,
    required_row_trace_fields: Vec<&'static str>,
    row_trace_counts: BTreeMap<String, usize>,
    candidate_count: usize,
    best_legal_candidate: ScoreFusionCandidate,
    top_calibration_selected_candidates: Vec<ScoreFusionCandidate>,
    forbidden_holdout_oracle: ForbiddenScoreFusionOracle,
    edge_cases: ScoreFusionEdgeCases,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreFusionEdgeCases {
    all_passed: bool,
    missing_row_trace_fails_closed: bool,
    mismatched_row_identity_fails_closed: bool,
    holdout_weight_selection_flagged_forbidden: bool,
}

#[derive(Debug, Clone, Copy)]
enum ThresholdSelectionSplit {
    Calibration,
    Holdout,
}

#[derive(Debug, Clone, Copy)]
struct ThresholdRepairSpec {
    category: &'static str,
    base_candidate: Candidate,
    probe_candidate: Candidate,
    low_values: &'static [f32],
    high_values: &'static [f32],
}

impl Default for ThresholdRepairSpec {
    fn default() -> Self {
        Self {
            category: "off_by_one",
            base_candidate: CURRENT_RBF,
            probe_candidate: Candidate::CategoryLocalKnn {
                k: 31,
                fail_boost: 2.0,
            },
            low_values: &[0.30, 0.31, 0.35, 0.40],
            high_values: &[0.58, 0.60, 0.70, 0.75],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ThresholdRepairReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    category: String,
    base_candidate: Candidate,
    probe_candidate: Candidate,
    threshold: f32,
    low: f32,
    high: f32,
    selection_objective: f32,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CalibrationScope {
    Global,
    PerCategory,
    TrueMixedPerCategory,
}

#[derive(Debug, Clone, Serialize)]
struct ProbabilityCalibrationBin {
    count: usize,
    positives: usize,
    calibrated_probability: f32,
}

#[derive(Debug, Clone, Serialize)]
struct ProbabilityCalibrationModel {
    selected_on: String,
    scope: CalibrationScope,
    base_candidate: Candidate,
    bin_count: usize,
    smoothing: f32,
    raw_weight: f32,
    global_prior: f32,
    category_priors: BTreeMap<String, f32>,
    bins_by_key: BTreeMap<String, Vec<ProbabilityCalibrationBin>>,
}

#[derive(Debug, Clone, Serialize)]
struct ProbabilityCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    unpenalized_selection_objective: f32,
    complexity_penalty: f32,
    selection_objective: f32,
    model: ProbabilityCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AffineCalibrationModel {
    selected_on: String,
    scope: CalibrationScope,
    base_candidate: Candidate,
    scale: f32,
    offset: f32,
}

#[derive(Debug, Clone, Serialize)]
struct AffineCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: AffineCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PredictionBlendModel {
    selected_on: String,
    affine_model: AffineCalibrationModel,
    affine_weight: f32,
    rank_lift_weight: f32,
}

#[derive(Debug, Clone, Serialize)]
struct PredictionBlendReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: PredictionBlendModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LogitCalibrationModel {
    selected_on: String,
    scope: CalibrationScope,
    base_candidate: Candidate,
    slope: f32,
    bias: f32,
}

#[derive(Debug, Clone, Serialize)]
struct LogitCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: LogitCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MeanBiasCalibrationModel {
    selected_on: String,
    scope: CalibrationScope,
    affine_model: AffineCalibrationModel,
    shrinkage: f32,
    bias_by_key: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Serialize)]
struct MeanBiasCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: MeanBiasCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CrossWindowBiasStats {
    support: usize,
    positives: usize,
    full_bias: f32,
    stable_bias: f32,
    window_bias_mean: f32,
    window_bias_std: f32,
    valid_window_count: usize,
    min_window_support_seen: usize,
    stability_weight: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CrossWindowStabilityCalibrationModel {
    selected_on: String,
    scope: CalibrationScope,
    affine_model: AffineCalibrationModel,
    shrinkage: f32,
    window_count: usize,
    min_window_support: usize,
    instability_scale: f32,
    bias_cap: f32,
    bias_stats: BTreeMap<String, CrossWindowBiasStats>,
}

#[derive(Debug, Clone, Serialize)]
struct CrossWindowStabilityCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: CrossWindowStabilityCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseResidualCalibrationModel {
    selected_on: String,
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    window_count: usize,
    regularization: f32,
    correction_scale: f32,
    correction_cap: f32,
    feature_count: usize,
    feature_mean: Vec<f32>,
    feature_std: Vec<f32>,
    weights_by_key: BTreeMap<String, Vec<f32>>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseResidualCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: CauseResidualCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBinKeyStats {
    support: usize,
    positives: usize,
    prior: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBinCalibrationModel {
    selected_on: String,
    base_model: CauseResidualCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
    key_stats: BTreeMap<String, CauseBinKeyStats>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBinApplicationStats {
    scoped_row_count: usize,
    applied_row_count: usize,
    missing_key_row_count: usize,
    under_min_support_row_count: usize,
    non_scoped_row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBinCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: CauseBinCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    calibration_application: CauseBinApplicationStats,
    holdout_application: CauseBinApplicationStats,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessCalibrationModel {
    selected_on: String,
    base_model: CauseBinCalibrationModel,
    scope: CalibrationScope,
    low_cutoff: f32,
    low_tail_scale: f32,
    high_cutoff: f32,
    high_tail_scale: f32,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessApplicationStats {
    scoped_row_count: usize,
    low_tail_shaped_row_count: usize,
    high_tail_shaped_row_count: usize,
    unchanged_scoped_row_count: usize,
    non_scoped_row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: ScoreSharpnessCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    calibration_application: ScoreSharpnessApplicationStats,
    holdout_application: ScoreSharpnessApplicationStats,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessLogitCalibrationModel {
    selected_on: String,
    base_model: ScoreSharpnessCalibrationModel,
    logit_model: LogitCalibrationModel,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessLogitCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: ScoreSharpnessLogitCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessAbstainBandModel {
    selected_on: String,
    base_model: ScoreSharpnessCalibrationModel,
    abstain_low: f32,
    abstain_high: f32,
    min_active_coverage: f32,
}

#[derive(Debug, Clone, Serialize)]
struct AbstainBandApplicationStats {
    total_row_count: usize,
    active_row_count: usize,
    abstained_row_count: usize,
    active_coverage: f32,
    abstained_actual_pass_count: usize,
    abstained_actual_fail_count: usize,
    per_category: BTreeMap<String, AbstainBandCategoryStats>,
}

#[derive(Debug, Clone, Serialize)]
struct AbstainBandCategoryStats {
    total_row_count: usize,
    active_row_count: usize,
    abstained_row_count: usize,
    active_coverage: f32,
    active_actual_pass_count: usize,
    active_actual_fail_count: usize,
    active_predicted_pass_count: usize,
    active_predicted_fail_count: usize,
    abstained_actual_pass_count: usize,
    abstained_actual_fail_count: usize,
    active_oracle_variance_class: OracleVarianceClass,
    active_gate_contract: String,
    active_gate_passed: bool,
    active_gate_blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ScoreSharpnessAbstainBandReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: ScoreSharpnessAbstainBandModel,
    calibration: SplitReport,
    holdout: SplitReport,
    calibration_application: AbstainBandApplicationStats,
    holdout_application: AbstainBandApplicationStats,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBandKeyStats {
    support: usize,
    positives: usize,
    prior: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBandCalibrationModel {
    selected_on: String,
    base_model: CauseBinCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
    key_stats: BTreeMap<String, CauseBandKeyStats>,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBandApplicationStats {
    scoped_row_count: usize,
    applied_row_count: usize,
    missing_key_row_count: usize,
    under_min_support_row_count: usize,
    non_scoped_row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CauseBandCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: CauseBandCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    calibration_application: CauseBandApplicationStats,
    holdout_application: CauseBandApplicationStats,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FinalBinCalibrationModel {
    selected_on: String,
    base_model: CauseBinCalibrationModel,
    probability_model: ProbabilityCalibrationModel,
}

#[derive(Debug, Clone, Serialize)]
struct FinalBinCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: FinalBinCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SupportShrinkCalibrationModel {
    selected_on: String,
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    smoothing: f32,
    prior_weight: f32,
    global_prior: f32,
    prior_by_key: BTreeMap<String, f32>,
    support_by_key: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
struct SupportShrinkCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: SupportShrinkCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceConsequenceKeyStats {
    support: usize,
    positives: usize,
    prior: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceConsequenceCalibrationModel {
    selected_on: String,
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
    global_prior: f32,
    key_stats: BTreeMap<String, EvidenceConsequenceKeyStats>,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceConsequenceCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: EvidenceConsequenceCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BinConsequenceKeyStats {
    support: usize,
    positives: usize,
    prior: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct BinConsequenceCalibrationModel {
    selected_on: String,
    base_model: EvidenceConsequenceCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
    key_stats: BTreeMap<String, BinConsequenceKeyStats>,
}

#[derive(Debug, Clone, Serialize)]
struct BinConsequenceCalibrationReport {
    selected_on: String,
    selection_forbidden: bool,
    forbidden_reason: Option<String>,
    selection_objective: f32,
    model: BinConsequenceCalibrationModel,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BinConsequenceApplicationStats {
    all_passed: bool,
    sample_count: usize,
    scoped_row_count: usize,
    applied_row_count: usize,
    noop_scoped_row_count: usize,
    applied_fraction: f32,
    scoped_applied_fraction: f32,
}

#[derive(Debug, Clone, Serialize)]
struct BinConsequenceCandidateAudit {
    all_passed: bool,
    calibration_row_reproduction: ReproductionReport,
    holdout_row_reproduction: ReproductionReport,
    calibration_row_audit: LegalBestRowAudit,
    holdout_row_audit: LegalBestRowAudit,
    calibration_ece_breakdown_audit: EceBreakdownAudit,
    holdout_ece_breakdown_audit: EceBreakdownAudit,
    calibration_application: BinConsequenceApplicationStats,
    holdout_application: BinConsequenceApplicationStats,
}

#[derive(Debug, Clone, Serialize)]
struct LegalBestCandidate {
    kind: String,
    candidate: Value,
    selection_reason: String,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
}

struct LegalCandidateReports<'a> {
    threshold: &'a ThresholdRepairReport,
    affine_calibration: &'a AffineCalibrationReport,
    prediction_blend: &'a PredictionBlendReport,
    logit_calibration: &'a LogitCalibrationReport,
    mean_bias_calibration: &'a MeanBiasCalibrationReport,
    cross_window_stability_calibration: &'a CrossWindowStabilityCalibrationReport,
    cause_residual_calibration: &'a CauseResidualCalibrationReport,
    cause_bin_calibration: &'a CauseBinCalibrationReport,
    score_sharpness_calibration: &'a ScoreSharpnessCalibrationReport,
    score_sharpness_logit_calibration: &'a ScoreSharpnessLogitCalibrationReport,
    cause_band_calibration: &'a CauseBandCalibrationReport,
    final_bin_calibration: &'a FinalBinCalibrationReport,
    support_shrink_calibration: &'a SupportShrinkCalibrationReport,
    evidence_consequence_calibration: &'a EvidenceConsequenceCalibrationReport,
    bin_consequence_calibration: &'a BinConsequenceCalibrationReport,
    probability_calibration: &'a ProbabilityCalibrationReport,
}

#[derive(Debug, Clone, Serialize)]
struct TypedBlocker {
    code: String,
    accepted: bool,
    why: String,
    evidence: Vec<String>,
    missing_signal: Vec<String>,
    next_work: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EdgeCaseAudit {
    all_passed: bool,
    ordered_replay_passed: bool,
    non_monotonic_order_fails_closed: bool,
    missing_code_state_row_fails_closed: bool,
    missing_file_path_row_fails_closed: bool,
    target_label_prefixes_excluded: bool,
    holdout_threshold_flagged_forbidden: bool,
    hierarchy_db_readback_passed: bool,
    affine_selected_on_calibration: bool,
    affine_true_mixed_scope_leaves_non_mixed_rows_unchanged: bool,
    affine_clamp_passed: bool,
    missing_slot_scalar_fails_closed: bool,
    missing_skill_summary_fails_closed: bool,
    support_shrink_missing_key_fails_closed: bool,
    support_shrink_non_scoped_row_unchanged: bool,
    support_shrink_clamp_passed: bool,
    evidence_consequence_non_scoped_row_unchanged: bool,
    bin_consequence_missing_key_noops: bool,
    bin_consequence_non_scoped_row_unchanged: bool,
    bin_consequence_clamp_passed: bool,
    cause_residual_non_scoped_row_unchanged: bool,
    cause_residual_missing_weight_noops: bool,
    cause_residual_feature_dimension_mismatch_fails_closed: bool,
    cause_residual_clamp_passed: bool,
    cause_bin_missing_key_noops: bool,
    cause_bin_under_min_support_noops: bool,
    cause_bin_non_scoped_row_unchanged: bool,
    cause_bin_clamp_passed: bool,
    score_sharpness_non_scoped_row_unchanged: bool,
    score_sharpness_low_tail_continuous_sharpens: bool,
    score_sharpness_high_tail_continuous_compresses: bool,
    score_sharpness_clamp_passed: bool,
    cause_band_missing_key_noops: bool,
    cause_band_under_min_support_noops: bool,
    cause_band_non_scoped_row_unchanged: bool,
    cause_band_dimension_mismatch_fails_closed: bool,
    cause_band_clamp_passed: bool,
    ece_empty_rows_zero: bool,
    ece_boundary_bins_passed: bool,
    ece_per_category_empty_subset_passed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ReproductionReport {
    passed: bool,
    correlation_delta_abs: Option<f32>,
    brier_delta_abs: f32,
    ece_delta_abs: f32,
    tolerance: f32,
}

#[derive(Debug, Clone, Serialize)]
struct LegalBestRowAudit {
    all_passed: bool,
    expected_sample_count: usize,
    actual_sample_count: usize,
    sample_count_matches: bool,
    expected_row_identity_sha256: String,
    actual_row_identity_sha256: String,
    row_identity_matches: bool,
    expected_per_category_counts: BTreeMap<String, usize>,
    actual_per_category_counts: BTreeMap<String, usize>,
    per_category_counts_match: bool,
    exact_metric_reproduction_passed: bool,
    tight_tolerance: f32,
}

#[derive(Debug, Clone, Serialize)]
struct LegalBestRowDiagnostic {
    row_index: usize,
    task_id: String,
    category: String,
    repo: String,
    patch_path: String,
    actual_oracle_pass: bool,
    predicted_oracle_pass: f32,
    predicted_pass: bool,
    correct: bool,
    absolute_error: f32,
    ece_bin_index: usize,
    current_rbf_score: f32,
    selected_knn_score: f32,
    cause_bin_score: f32,
    score_sharpness_score: f32,
    evidence_key_count: usize,
    evidence_key_sample: Vec<String>,
    cause_feature_nonzero_count: usize,
    cause_feature_large_magnitude_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct EceBreakdownAudit {
    all_passed: bool,
    metric_ece: f32,
    aggregate_ece: f32,
    ece_from_bins: f32,
    sample_count_matches: bool,
    aggregate_matches_metric: bool,
    bins_sum_matches_aggregate: bool,
    non_empty_bins_when_sampled: bool,
    tolerance: f32,
}

#[derive(Debug, Clone, Serialize)]
struct DirectionalSignalReport {
    passed: bool,
    baseline_correlation: Option<f32>,
    candidate_correlation: Option<f32>,
    correlation_delta: Option<f32>,
    baseline_brier: f32,
    candidate_brier: f32,
    brier_delta: f32,
    baseline_ece: f32,
    candidate_ece: f32,
    ece_delta: f32,
}

#[derive(Debug, Clone, Serialize)]
struct SkillPolicyAblationReport {
    all_passed: bool,
    selected_policy: String,
    selected_reason: String,
    typed_blocker: String,
    evaluations: Vec<SkillPolicyEvaluation>,
}

#[derive(Debug, Clone, Serialize)]
struct SkillPolicyEvaluation {
    policy: String,
    feature_dim: usize,
    current_rbf: SplitMetrics,
    mean_bias: MeanBiasCalibrationReport,
    delta_vs_raw_current: Option<MetricDeltaReport>,
    delta_vs_536: MetricDeltaReport,
    selection_eligible: bool,
    rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MetricDeltaReport {
    reference: String,
    candidate: String,
    reference_correlation: Option<f32>,
    candidate_correlation: Option<f32>,
    correlation_delta: Option<f32>,
    reference_brier: f32,
    candidate_brier: f32,
    brier_delta: f32,
    reference_ece: f32,
    candidate_ece: f32,
    ece_delta: f32,
    non_regressing: bool,
    ece_improved: bool,
}

#[derive(Debug, Clone, Serialize)]
struct MetricBlock {
    sample_count: usize,
    accuracy: f32,
    prediction_oracle_correlation: Option<f32>,
    oracle_variance_class: OracleVarianceClass,
    brier: f32,
    ece: f32,
    predicted_pass_count: usize,
    predicted_fail_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    pass_recall: Option<f32>,
    fail_recall: Option<f32>,
    actual_class_confidence_mean: f32,
    score_min: f32,
    score_max: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct EceBreakdownReport {
    aggregate_ece: f32,
    ece_from_bins: f32,
    sample_count: usize,
    non_empty_bin_count: usize,
    worst_bins: Vec<EceBinBreakdown>,
    per_category: BTreeMap<String, CategoryEceBreakdown>,
}

#[derive(Debug, Clone, Serialize)]
struct CategoryEceBreakdown {
    metrics: MetricBlock,
    non_empty_bin_count: usize,
    worst_gap: f32,
    bins: Vec<EceBinBreakdown>,
}

#[derive(Debug, Clone, Serialize)]
struct EceBinBreakdown {
    bin_index: usize,
    range_start: f32,
    range_end: f32,
    sample_count: usize,
    actual_pass_count: usize,
    actual_fail_count: usize,
    predicted_pass_count: usize,
    predicted_fail_count: usize,
    confidence_mean: f32,
    empirical_pass_rate: f32,
    abs_gap: f32,
    weighted_ece: f32,
    sample_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OracleVarianceClass {
    Empty,
    AllFail,
    AllPass,
    Mixed,
}

fn read_index(path: impl AsRef<Path>) -> Result<CorpusIndex> {
    let index: CorpusIndex = serde_json::from_slice(&fs::read(path.as_ref())?)?;
    ensure!(!index.entries.is_empty(), "corpus index has no entries");
    Ok(index)
}

fn load_feature_table(
    entries: &[CorpusEntry],
    mode: FeatureMode,
    include_metadata_features: bool,
) -> Result<FeatureTable> {
    let expected_labels = entries
        .iter()
        .map(|entry| {
            (
                (entry.task_id.clone(), entry.category.clone()),
                entry.oracle_all_passed,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected_tasks = entries
        .iter()
        .map(|entry| entry.task_id.clone())
        .collect::<BTreeSet<_>>();
    let mut states = BTreeMap::<(String, String), CodeStateConstellationRow>::new();
    let file = fs::File::open(CODE_STATE_CONSTELLATIONS_PATH)
        .with_context(|| format!("open {CODE_STATE_CONSTELLATIONS_PATH}"))?;
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
        let row: CodeStateConstellationRow = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse {} line {}",
                CODE_STATE_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        ensure!(
            row.slot_identity_preserved && !row.flat_vector_concat_used,
            "code-state row {}:{} must preserve slot identity and reject flat vectors",
            row.task_instance_id,
            row.mutation_category
        );
        if !expected_tasks.contains(&row.task_instance_id) {
            continue;
        }
        let state_key = (row.task_instance_id.clone(), row.workspace_state.clone());
        ensure!(
            states.insert(state_key.clone(), row).is_none(),
            "duplicate code-state row for {:?}",
            state_key
        );
    }
    let file_states = match mode {
        FeatureMode::CodeState => BTreeMap::new(),
        FeatureMode::FileTrajectory | FeatureMode::FileSkillTrajectory(_) => {
            load_file_path_states(&expected_tasks)?
        }
    };
    let skill_index = match mode {
        FeatureMode::FileSkillTrajectory(_) => Some(load_skill_feature_index(&states, entries)?),
        FeatureMode::CodeState | FeatureMode::FileTrajectory => None,
    };
    let skill_coverage = skill_index.as_ref().map(|index| index.coverage.clone());

    let mut rows = Vec::with_capacity(entries.len());
    let mut raw_features = Vec::with_capacity(entries.len());
    for entry in entries {
        let base = required_state(&states, &entry.task_id, "base")?;
        let target_workspace = target_workspace_state(&entry.category);
        let target = required_state(&states, &entry.task_id, &target_workspace)?;
        let test_patch = required_state(&states, &entry.task_id, "test_patch")?;
        ensure_ordered_states(base, target, test_patch)?;
        ensure!(
            target.mutation_category == entry.category,
            "target state category mismatch for {}:{} via {}",
            entry.task_id,
            entry.category,
            target_workspace
        );
        ensure!(
            target.oracle_all_passed
                == *expected_labels
                    .get(&(entry.task_id.clone(), entry.category.clone()))
                    .ok_or_else(|| anyhow!(
                        "missing expected label for {}:{}",
                        entry.task_id,
                        entry.category
                    ))?,
            "target state oracle label mismatch for {}:{}",
            entry.task_id,
            entry.category
        );
        let mut features = trajectory_feature_vector(base, target, test_patch)?;
        if matches!(
            mode,
            FeatureMode::FileTrajectory | FeatureMode::FileSkillTrajectory(_)
        ) {
            features.extend(file_path_trajectory_feature_vector(
                &file_states,
                &entry.task_id,
                &target_workspace,
            )?);
        }
        if let FeatureMode::FileSkillTrajectory(policy) = mode {
            features.extend(skill_trajectory_feature_vector(
                skill_index
                    .as_ref()
                    .ok_or_else(|| anyhow!("skill index missing in skill trajectory mode"))?,
                base,
                target,
                test_patch,
                policy,
            )?);
        }
        if include_metadata_features {
            features.extend(metadata_features(entry)?);
        }
        let evidence_keys = consequence_evidence_keys(
            &file_states,
            skill_index.as_ref(),
            mode,
            entry,
            base,
            target,
            test_patch,
            &target_workspace,
        )?;
        let cause_features = cause_residual_feature_vector(
            &file_states,
            skill_index.as_ref(),
            mode,
            &entry.task_id,
            &target_workspace,
            base,
            target,
            test_patch,
        )?;
        raw_features.push(features);
        rows.push(FeatureRow {
            task_id: entry.task_id.clone(),
            category: entry.category.clone(),
            bucket: entry.bucket.clone(),
            actual_oracle_pass: entry.oracle_all_passed,
            patch_path: entry.patch_path.clone(),
            repo: entry.repo.clone(),
            evidence_keys,
            cause_features,
            standardized_features: Vec::new(),
        });
    }
    let feature_dim = raw_features
        .first()
        .map(|features| features.len())
        .unwrap_or_default();
    ensure!(feature_dim > 0, "feature dimension must be non-zero");
    standardize_features(&mut rows, &mut raw_features)?;

    let mut train_indices_by_category = BTreeMap::<String, Vec<usize>>::new();
    for (idx, row) in rows.iter().enumerate() {
        if row.bucket == "train" {
            train_indices_by_category
                .entry(row.category.clone())
                .or_default()
                .push(idx);
        }
    }
    Ok(FeatureTable {
        rows,
        feature_dim,
        train_indices_by_category,
        skill_coverage,
    })
}

fn consequence_evidence_keys(
    file_states: &BTreeMap<(String, String, String), FilePathConstellationRow>,
    skill_index: Option<&SkillFeatureIndex>,
    mode: FeatureMode,
    entry: &CorpusEntry,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
    target_workspace: &str,
) -> Result<Vec<String>> {
    let category = key_component(&entry.category);
    let slot_sig = slot_delta_evidence_signature(base, target)?;
    let label_sig = label_delta_evidence_signature(base, target);
    let file_sig = if matches!(
        mode,
        FeatureMode::FileTrajectory | FeatureMode::FileSkillTrajectory(_)
    ) {
        file_path_evidence_signature(file_states, &entry.task_id, target_workspace)?
    } else {
        "none".to_string()
    };
    let (skill_sig, skill_kind_sig) = if let FeatureMode::FileSkillTrajectory(policy) = mode {
        let index = skill_index.ok_or_else(|| anyhow!("skill index missing for evidence keys"))?;
        (
            skill_evidence_signature(index, base, target, test_patch, policy)?,
            skill_kind_evidence_signature(index, base, target, test_patch)?,
        )
    } else {
        ("none".to_string(), "none".to_string())
    };

    let mut keys = Vec::new();
    push_evidence_key(
        &mut keys,
        format!(
            "cat:{category}|skill:{skill_sig}|file:{file_sig}|slot:{slot_sig}|label:{label_sig}"
        ),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill:{skill_sig}|slot:{slot_sig}|label:{label_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|file:{file_sig}|slot:{slot_sig}|label:{label_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill:{skill_sig}|file:{file_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!(
            "cat:{category}|skill_kind:{skill_kind_sig}|file:{file_sig}|slot:{slot_sig}|label:{label_sig}"
        ),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill_kind:{skill_kind_sig}|slot:{slot_sig}|label:{label_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill_kind:{skill_kind_sig}|file:{file_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill_kind:{skill_kind_sig}|label:{label_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|skill_kind:{skill_kind_sig}"),
    );
    push_evidence_key(
        &mut keys,
        format!("cat:{category}|slot:{slot_sig}|label:{label_sig}"),
    );
    push_evidence_key(&mut keys, format!("cat:{category}|skill:{skill_sig}"));
    push_evidence_key(&mut keys, format!("cat:{category}|file:{file_sig}"));
    push_evidence_key(&mut keys, format!("cat:{category}|slot:{slot_sig}"));
    push_evidence_key(&mut keys, format!("cat:{category}|label:{label_sig}"));
    push_evidence_key(&mut keys, format!("cat:{category}"));
    ensure!(!keys.is_empty(), "evidence key ladder is empty");
    Ok(keys)
}

fn push_evidence_key(keys: &mut Vec<String>, key: String) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn slot_delta_evidence_signature(
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
) -> Result<String> {
    let base_matrix = slot_matrix(base)?;
    let target_matrix = slot_matrix(target)?;
    let mut deltas = Vec::new();
    for (slot_idx, (base_slot, target_slot)) in base_matrix.iter().zip(&target_matrix).enumerate() {
        for (scalar_idx, (base_value, target_value)) in
            base_slot.iter().zip(target_slot).enumerate()
        {
            let delta = target_value - base_value;
            deltas.push((
                delta.abs(),
                format!(
                    "{}.{}.{}",
                    SLOT_IDS[slot_idx],
                    SLOT_SCALAR_NAMES[scalar_idx],
                    signed_magnitude_bucket(delta)
                ),
            ));
        }
    }
    deltas.sort_by(|left, right| right.0.partial_cmp(&left.0).unwrap_or(Ordering::Equal));
    let parts = deltas
        .into_iter()
        .take(4)
        .filter(|(magnitude, _)| *magnitude > 1.0e-6)
        .map(|(_, label)| label)
        .collect::<Vec<_>>();
    Ok(if parts.is_empty() {
        "stable".to_string()
    } else {
        parts.join("+")
    })
}

fn signed_magnitude_bucket(delta: f32) -> &'static str {
    let prefix = if delta >= 0.0 { "up" } else { "down" };
    let magnitude = delta.abs();
    match (prefix, magnitude) {
        ("up", value) if value >= 1.0 => "up_xl",
        ("up", value) if value >= 0.25 => "up_l",
        ("up", value) if value >= 0.05 => "up_m",
        ("up", _) => "up_s",
        (_, value) if value >= 1.0 => "down_xl",
        (_, value) if value >= 0.25 => "down_l",
        (_, value) if value >= 0.05 => "down_m",
        _ => "down_s",
    }
}

fn label_delta_evidence_signature(
    from: &CodeStateConstellationRow,
    to: &CodeStateConstellationRow,
) -> String {
    let from_labels = live_labels(from);
    let to_labels = live_labels(to);
    let added = compact_set_signature(to_labels.difference(&from_labels), 3);
    let removed = compact_set_signature(from_labels.difference(&to_labels), 3);
    format!("add:{added};rm:{removed}")
}

fn file_path_evidence_signature(
    file_states: &BTreeMap<(String, String, String), FilePathConstellationRow>,
    task_id: &str,
    target_workspace: &str,
) -> Result<String> {
    let base_rows = required_file_rows(file_states, task_id, "base")?;
    let target_rows = required_file_rows(file_states, task_id, target_workspace)?;
    let test_rows = required_file_rows(file_states, task_id, "test_patch")?;
    let base_paths = row_path_set(&base_rows)?;
    let target_paths = row_path_set(&target_rows)?;
    let test_paths = row_path_set(&test_rows)?;
    let target_added = compact_path_shape_signature(target_paths.difference(&base_paths), 3);
    let target_removed = compact_path_shape_signature(base_paths.difference(&target_paths), 3);
    let test_added = compact_path_shape_signature(test_paths.difference(&target_paths), 3);
    Ok(format!(
        "bt_add:{target_added};bt_rm:{target_removed};tt_add:{test_added};bt_j:{:.2};tt_j:{:.2}",
        set_jaccard(&base_paths, &target_paths),
        set_jaccard(&target_paths, &test_paths)
    ))
}

fn skill_evidence_signature(
    skill_index: &SkillFeatureIndex,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
    policy: SkillFeaturePolicy,
) -> Result<String> {
    let base_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, base)?, policy);
    let target_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, target)?, policy);
    let test_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, test_patch)?, policy);
    let bt_added = compact_set_signature(
        target_summary
            .level2_skill_ids
            .difference(&base_summary.level2_skill_ids),
        4,
    );
    let bt_removed = compact_set_signature(
        base_summary
            .level2_skill_ids
            .difference(&target_summary.level2_skill_ids),
        4,
    );
    let tt_added = compact_set_signature(
        test_summary
            .level2_skill_ids
            .difference(&target_summary.level2_skill_ids),
        4,
    );
    let label_added = compact_set_signature(
        target_summary
            .accepted_label_ids
            .difference(&base_summary.accepted_label_ids),
        3,
    );
    Ok(format!(
        "bt_add:{bt_added};bt_rm:{bt_removed};tt_add:{tt_added};label_add:{label_added};steps:{};multi:{}",
        target_summary.ordered_step_count.saturating_sub(base_summary.ordered_step_count),
        target_summary
            .multi_step_membership_count
            .saturating_sub(base_summary.multi_step_membership_count)
    ))
}

fn skill_kind_evidence_signature(
    skill_index: &SkillFeatureIndex,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
) -> Result<String> {
    let base_summary = skill_summary_for_state(skill_index, base)?;
    let target_summary = skill_summary_for_state(skill_index, target)?;
    let test_summary = skill_summary_for_state(skill_index, test_patch)?;
    let mut parts = Vec::new();
    for kind in [
        "failure_skill",
        "pass_stability_skill",
        "context_negative_evidence",
    ] {
        let empty = SkillKindStateSummary::default();
        let base_kind = base_summary.kind_summaries.get(kind).unwrap_or(&empty);
        let target_kind = target_summary.kind_summaries.get(kind).unwrap_or(&empty);
        let test_kind = test_summary.kind_summaries.get(kind).unwrap_or(&empty);
        let signature = skill_kind_transition_signature(kind, base_kind, target_kind, test_kind);
        if signature != "stable" {
            parts.push(signature);
        }
    }
    Ok(if parts.is_empty() {
        "stable".to_string()
    } else {
        parts.join("+")
    })
}

fn skill_kind_transition_signature(
    kind: &str,
    base: &SkillKindStateSummary,
    target: &SkillKindStateSummary,
    test_patch: &SkillKindStateSummary,
) -> String {
    let bt_added = target
        .level2_skill_ids
        .difference(&base.level2_skill_ids)
        .count();
    let bt_removed = base
        .level2_skill_ids
        .difference(&target.level2_skill_ids)
        .count();
    let tt_added = test_patch
        .level2_skill_ids
        .difference(&target.level2_skill_ids)
        .count();
    let label_added = target
        .accepted_label_ids
        .difference(&base.accepted_label_ids)
        .count();
    let step_delta = target.ordered_step_count as isize - base.ordered_step_count as isize;
    let multi_delta =
        target.multi_step_membership_count as isize - base.multi_step_membership_count as isize;
    let usefulness_delta =
        normalized_usefulness_weight(target) - normalized_usefulness_weight(base);
    if bt_added == 0
        && bt_removed == 0
        && tt_added == 0
        && label_added == 0
        && step_delta == 0
        && multi_delta == 0
        && usefulness_delta.abs() < 1.0e-6
    {
        return "stable".to_string();
    }
    format!(
        "{}:bt_add:{};bt_rm:{};tt_add:{};label_add:{};steps:{};multi:{};weight:{}",
        key_component(kind),
        count_bucket(bt_added),
        count_bucket(bt_removed),
        count_bucket(tt_added),
        count_bucket(label_added),
        signed_count_bucket(step_delta),
        signed_count_bucket(multi_delta),
        signed_weight_bucket(usefulness_delta)
    )
}

fn cause_residual_feature_vector(
    file_states: &BTreeMap<(String, String, String), FilePathConstellationRow>,
    skill_index: Option<&SkillFeatureIndex>,
    mode: FeatureMode,
    task_id: &str,
    target_workspace: &str,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
) -> Result<Vec<f32>> {
    ensure_ordered_states(base, target, test_patch)?;
    let v0 = slot_matrix(base)?;
    let v1 = slot_matrix(target)?;
    let v2 = slot_matrix(test_patch)?;
    let d10 = matrix_delta(&v1, &v0);
    let d21 = matrix_delta(&v2, &v1);
    let d20 = matrix_delta(&v2, &v0);
    let mut features = Vec::with_capacity(64);
    add_temporal_cause_features(&mut features, &d10, &d21, &d20);
    add_live_label_cause_features(&mut features, base, target, test_patch);
    if matches!(
        mode,
        FeatureMode::FileTrajectory | FeatureMode::FileSkillTrajectory(_)
    ) {
        let base_rows = required_file_rows(file_states, task_id, "base")?;
        let target_rows = required_file_rows(file_states, task_id, target_workspace)?;
        let test_rows = required_file_rows(file_states, task_id, "test_patch")?;
        add_file_cause_features(&mut features, &base_rows, &target_rows, &test_rows)?;
    } else {
        features.extend([0.0_f32; 8]);
    }
    if let FeatureMode::FileSkillTrajectory(_) = mode {
        let index = skill_index
            .ok_or_else(|| anyhow!("skill index missing for cause residual features"))?;
        add_skill_cause_features(
            &mut features,
            skill_summary_for_state(index, base)?,
            skill_summary_for_state(index, target)?,
            skill_summary_for_state(index, test_patch)?,
        );
    } else {
        features.extend([0.0_f32; 18]);
    }
    ensure!(
        features.iter().all(|value| value.is_finite()),
        "cause residual feature vector contains non-finite value"
    );
    Ok(features)
}

fn add_temporal_cause_features(
    features: &mut Vec<f32>,
    d10: &[Vec<f32>],
    d21: &[Vec<f32>],
    d20: &[Vec<f32>],
) {
    for matrix in [d10, d21, d20] {
        let temporal = mean_slot_norm(matrix, true);
        let non_temporal = mean_slot_norm(matrix, false);
        features.push(temporal);
        features.push(non_temporal);
        features.push(temporal - non_temporal);
        features.push(temporal / (non_temporal + 1.0e-6));
    }
    features.push(flattened_cosine(d10, d21));
    features.push(flattened_cosine(d10, d20));
    features.push(flattened_cosine(d21, d20));
    let d10_norm = matrix_l2_norm(d10);
    let d21_norm = matrix_l2_norm(d21);
    let d20_norm = matrix_l2_norm(d20);
    features.push(d21_norm - d10_norm);
    features.push(d20_norm / (d10_norm + 1.0e-6));
    features.push((d10_norm + d21_norm - d20_norm).clamp(-10.0, 10.0));
}

fn mean_slot_norm(matrix: &[Vec<f32>], temporal: bool) -> f32 {
    let mut sum = 0.0_f32;
    let mut count = 0_usize;
    for (idx, slot) in matrix.iter().enumerate() {
        let is_temporal = matches!(SLOT_IDS[idx], "e2" | "e3" | "e4");
        if is_temporal == temporal {
            sum += l2_norm(slot);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

fn matrix_l2_norm(matrix: &[Vec<f32>]) -> f32 {
    matrix
        .iter()
        .flat_map(|slot| slot.iter())
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt()
}

fn flattened_cosine(left: &[Vec<f32>], right: &[Vec<f32>]) -> f32 {
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_slot, right_slot) in left.iter().zip(right) {
        for (left_value, right_value) in left_slot.iter().zip(right_slot) {
            dot += left_value * right_value;
            left_norm += left_value * left_value;
            right_norm += right_value * right_value;
        }
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        0.0
    } else {
        (dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0)
    }
}

fn add_live_label_cause_features(
    features: &mut Vec<f32>,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
) {
    let base_labels = live_labels(base);
    let target_labels = live_labels(target);
    let test_labels = live_labels(test_patch);
    features.push(target_labels.difference(&base_labels).count() as f32);
    features.push(base_labels.difference(&target_labels).count() as f32);
    features.push(test_labels.difference(&target_labels).count() as f32);
    features.push(set_jaccard(&base_labels, &target_labels));
    features.push(set_jaccard(&target_labels, &test_labels));
    features.push(set_jaccard(&base_labels, &test_labels));
}

fn add_file_cause_features(
    features: &mut Vec<f32>,
    base_rows: &[&FilePathConstellationRow],
    target_rows: &[&FilePathConstellationRow],
    test_rows: &[&FilePathConstellationRow],
) -> Result<()> {
    let base_paths = row_path_set(base_rows)?;
    let target_paths = row_path_set(target_rows)?;
    let test_paths = row_path_set(test_rows)?;
    features.push(set_jaccard(&base_paths, &target_paths));
    features.push(set_jaccard(&target_paths, &test_paths));
    features.push(set_jaccard(&base_paths, &test_paths));
    features.push((target_paths.difference(&base_paths).count() as f32).ln_1p());
    features.push((base_paths.difference(&target_paths).count() as f32).ln_1p());
    features.push((test_paths.difference(&target_paths).count() as f32).ln_1p());
    features.push((target_paths.intersection(&test_paths).count() as f32).ln_1p());
    features.push((target_paths.len() as f32 - base_paths.len() as f32).clamp(-50.0, 50.0));
    Ok(())
}

fn add_skill_cause_features(
    features: &mut Vec<f32>,
    base: &SkillStateSummary,
    target: &SkillStateSummary,
    test_patch: &SkillStateSummary,
) {
    for kind in [
        "failure_skill",
        "pass_stability_skill",
        "context_negative_evidence",
    ] {
        let empty = SkillKindStateSummary::default();
        let base_kind = base.kind_summaries.get(kind).unwrap_or(&empty);
        let target_kind = target.kind_summaries.get(kind).unwrap_or(&empty);
        let test_kind = test_patch.kind_summaries.get(kind).unwrap_or(&empty);
        features.push(
            target_kind
                .level2_skill_ids
                .difference(&base_kind.level2_skill_ids)
                .count() as f32,
        );
        features.push(
            base_kind
                .level2_skill_ids
                .difference(&target_kind.level2_skill_ids)
                .count() as f32,
        );
        features.push(
            test_kind
                .level2_skill_ids
                .difference(&target_kind.level2_skill_ids)
                .count() as f32,
        );
        features.push(target_kind.ordered_step_count as f32 - base_kind.ordered_step_count as f32);
        features.push(
            target_kind.multi_step_membership_count as f32
                - base_kind.multi_step_membership_count as f32,
        );
        features.push(
            (normalized_usefulness_weight(target_kind) - normalized_usefulness_weight(base_kind))
                as f32,
        );
    }
}

fn normalized_usefulness_weight(summary: &SkillKindStateSummary) -> f64 {
    let membership_count = summary.membership_count.max(1) as f64;
    (summary.usefulness_weight_sum / membership_count).clamp(0.0, 1.0)
}

fn count_bucket(value: usize) -> &'static str {
    match value {
        0 => "0",
        1 => "1",
        2 => "2",
        3..=5 => "3_5",
        6..=10 => "6_10",
        _ => "gt10",
    }
}

fn signed_count_bucket(value: isize) -> String {
    let prefix = if value >= 0 { "up" } else { "down" };
    format!("{prefix}_{}", count_bucket(value.unsigned_abs()))
}

fn signed_weight_bucket(delta: f64) -> &'static str {
    let magnitude = delta.abs();
    match (delta >= 0.0, magnitude) {
        (_, value) if value < 1.0e-6 => "stable",
        (true, value) if value >= 0.25 => "up_l",
        (true, value) if value >= 0.05 => "up_m",
        (true, _) => "up_s",
        (false, value) if value >= 0.25 => "down_l",
        (false, value) if value >= 0.05 => "down_m",
        (false, _) => "down_s",
    }
}

fn compact_set_signature<'a, I>(values: I, limit: usize) -> String
where
    I: Iterator<Item = &'a String>,
{
    let parts = values
        .take(limit)
        .map(|value| key_component(value))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join("+")
    }
}

fn compact_path_shape_signature<'a, I>(values: I, limit: usize) -> String
where
    I: Iterator<Item = &'a String>,
{
    let parts = values
        .take(limit)
        .map(|path| path_shape_component(path))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join("+")
    }
}

fn path_shape_component(path: &str) -> String {
    let top = path.split('/').next().unwrap_or("root");
    let ext = path
        .rsplit_once('.')
        .map(|(_stem, ext)| ext)
        .unwrap_or("noext");
    format!("top:{}:ext:{}", key_component(top), key_component(ext))
}

fn key_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.len() <= 80 {
        sanitized
    } else {
        let digest = Sha256::digest(value.as_bytes());
        format!(
            "{}_{:02x}{:02x}{:02x}{:02x}",
            &sanitized[..72],
            digest[0],
            digest[1],
            digest[2],
            digest[3]
        )
    }
}

fn load_file_path_states(
    expected_tasks: &BTreeSet<String>,
) -> Result<BTreeMap<(String, String, String), FilePathConstellationRow>> {
    let file = fs::File::open(FILE_PATH_CONSTELLATIONS_PATH)
        .with_context(|| format!("open {FILE_PATH_CONSTELLATIONS_PATH}"))?;
    let mut states = BTreeMap::<(String, String, String), FilePathConstellationRow>::new();
    for (line_idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "read {} line {}",
                FILE_PATH_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let row: FilePathConstellationRow = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse {} line {}",
                FILE_PATH_CONSTELLATIONS_PATH,
                line_idx + 1
            )
        })?;
        ensure!(
            row.slot_identity_preserved && !row.flat_vector_concat_used,
            "file-path row {}:{} must preserve slot identity and reject flat vectors",
            row.task_instance_id,
            row.workspace_state
        );
        if !expected_tasks.contains(&row.task_instance_id) {
            continue;
        }
        let relative_path = row.relative_path.clone().ok_or_else(|| {
            anyhow!(
                "file-path row {}:{} missing relative_path",
                row.task_instance_id,
                row.workspace_state
            )
        })?;
        let key = (
            row.task_instance_id.clone(),
            row.workspace_state.clone(),
            relative_path,
        );
        ensure!(
            states.insert(key.clone(), row).is_none(),
            "duplicate file-path row for {:?}",
            key
        );
    }
    ensure!(!states.is_empty(), "file-path constellation table is empty");
    Ok(states)
}

fn load_skill_feature_index(
    expected_states: &BTreeMap<(String, String), CodeStateConstellationRow>,
    entries: &[CorpusEntry],
) -> Result<SkillFeatureIndex> {
    let hierarchy_report_path = hierarchy_report_path();
    let hierarchy_db_path = hierarchy_db_path();
    let hierarchy_report_sha256 = sha256_file(&hierarchy_report_path)
        .with_context(|| format!("sha256 {}", hierarchy_report_path))?;
    let hierarchy_report = read_json(&hierarchy_report_path)?;
    ensure!(
        hierarchy_report["passes"].as_bool() == Some(true),
        "hierarchy FSV report must have passes=true"
    );
    ensure!(
        hierarchy_report["active_path_policy"]["mnt_d_active_reads_or_writes"].as_bool()
            == Some(false),
        "hierarchy FSV must not use /mnt/d active paths"
    );
    let db = open_hierarchy_db_read_only(Path::new(&hierarchy_db_path))?;
    let level2_skills = read_all_level2_skill_rows(&db)?;
    let memberships = read_all_chunk_skill_membership_rows(&db)?;
    ensure!(
        !level2_skills.is_empty(),
        "hierarchy DB has no Level-2 skills"
    );
    ensure!(
        !memberships.is_empty(),
        "hierarchy DB has no chunk-skill memberships"
    );
    let usefulness = skill_usefulness_descriptors(&level2_skills, &memberships, entries)?;
    let mut by_code_state = BTreeMap::<String, SkillStateSummary>::new();
    for row in &memberships {
        ensure!(
            row.live_input_allowed,
            "membership {} is not live-input safe",
            row.membership_key
        );
        ensure!(
            row.source_accepted_label_ids
                .iter()
                .chain(
                    row.ordered_step_evidence
                        .iter()
                        .flat_map(|step| step.accepted_label_ids.iter())
                )
                .all(|label| is_live_label(label)),
            "membership {} contains target-side accepted label",
            row.membership_key
        );
        push_skill_summary(&mut by_code_state, row, &usefulness.descriptors);
    }
    let mut uncovered_code_state_keys = BTreeSet::new();
    for row in expected_states.values() {
        let key = code_state_key(row);
        if let Entry::Vacant(entry) = by_code_state.entry(key.clone()) {
            uncovered_code_state_keys.insert(key);
            entry.insert(SkillStateSummary::default());
        }
    }
    let expected_code_state_count = expected_states.len();
    let covered_code_state_count =
        expected_code_state_count.saturating_sub(uncovered_code_state_keys.len());
    let coverage = SkillCoverageReport {
        expected_code_state_count,
        covered_code_state_count,
        uncovered_code_state_count: uncovered_code_state_keys.len(),
        uncovered_code_state_sample: uncovered_code_state_keys.into_iter().take(12).collect(),
        usefulness_policy: usefulness.report,
    };
    Ok(SkillFeatureIndex {
        by_code_state,
        level2_skill_count: level2_skills.len(),
        membership_count: memberships.len(),
        hierarchy_report_sha256,
        coverage,
    })
}

fn push_skill_summary(
    by_code_state: &mut BTreeMap<String, SkillStateSummary>,
    row: &ChunkSkillMembershipRow,
    usefulness: &BTreeMap<String, SkillUsefulnessDescriptor>,
) {
    let summary = by_code_state.entry(row.code_state_key.clone()).or_default();
    let descriptor = usefulness
        .get(&row.skill_id)
        .cloned()
        .unwrap_or_else(|| neutral_skill_descriptor(&row.skill_id));
    if row.hierarchy_level == 2 {
        summary.level2_skill_ids.insert(row.skill_id.clone());
    } else {
        summary.higher_ability_ids.insert(row.skill_id.clone());
    }
    summary.chunk_ids.insert(row.chunk_id.clone());
    summary.file_paths.insert(row.file_path.clone());
    summary.membership_count += 1;
    summary.ordered_step_count += row.ordered_step_evidence.len();
    if row.ordered_step_evidence.len() > 1 {
        summary.multi_step_membership_count += 1;
    }
    summary.membership_score_sum += row.membership_score;
    summary.membership_score_max = summary.membership_score_max.max(row.membership_score);
    let usefulness_score =
        (row.membership_score * descriptor.split_selection_weight).clamp(0.0, 1.0);
    summary.usefulness_weight_sum += usefulness_score;
    summary.usefulness_weight_max = summary.usefulness_weight_max.max(usefulness_score);
    let kind_summary = summary
        .kind_summaries
        .entry(descriptor.kind_label.clone())
        .or_default();
    if row.hierarchy_level == 2 {
        kind_summary.level2_skill_ids.insert(row.skill_id.clone());
    } else {
        kind_summary.higher_ability_ids.insert(row.skill_id.clone());
    }
    kind_summary.chunk_ids.insert(row.chunk_id.clone());
    kind_summary.file_paths.insert(row.file_path.clone());
    kind_summary.membership_count += 1;
    kind_summary.ordered_step_count += row.ordered_step_evidence.len();
    if row.ordered_step_evidence.len() > 1 {
        kind_summary.multi_step_membership_count += 1;
    }
    kind_summary.raw_membership_score_sum += row.membership_score;
    kind_summary.raw_membership_score_max = kind_summary
        .raw_membership_score_max
        .max(row.membership_score);
    kind_summary.usefulness_weight_sum += usefulness_score;
    kind_summary.usefulness_weight_max = kind_summary.usefulness_weight_max.max(usefulness_score);
    for label in &row.source_accepted_label_ids {
        if is_live_label(label) {
            summary.accepted_label_ids.insert(label.clone());
            kind_summary.accepted_label_ids.insert(label.clone());
        }
    }
    for step in &row.ordered_step_evidence {
        for label in &step.accepted_label_ids {
            if is_live_label(label) {
                summary.accepted_label_ids.insert(label.clone());
                kind_summary.accepted_label_ids.insert(label.clone());
            }
        }
        for group in &step.group_ids {
            if is_live_label(group) {
                summary.group_ids.insert(group.clone());
                kind_summary.group_ids.insert(group.clone());
            }
        }
    }
}

struct SkillUsefulnessBuild {
    descriptors: BTreeMap<String, SkillUsefulnessDescriptor>,
    report: SkillUsefulnessPolicyReport,
}

fn skill_usefulness_descriptors(
    level2_skills: &[Level2SkillRow],
    memberships: &[ChunkSkillMembershipRow],
    entries: &[CorpusEntry],
) -> Result<SkillUsefulnessBuild> {
    let mut train_target_outcomes = BTreeMap::<String, bool>::new();
    let mut train_pass = 0_u64;
    let mut train_fail = 0_u64;
    for entry in entries.iter().filter(|entry| entry.bucket == "train") {
        let key = format!(
            "{}|{}|{}",
            entry.task_id,
            entry.category,
            target_workspace_state(&entry.category)
        );
        ensure!(
            train_target_outcomes
                .insert(key, entry.oracle_all_passed)
                .is_none(),
            "duplicate train target outcome for {}:{}",
            entry.task_id,
            entry.category
        );
        if entry.oracle_all_passed {
            train_pass += 1;
        } else {
            train_fail += 1;
        }
    }
    ensure!(
        train_pass + train_fail > 0,
        "skill usefulness requires train outcomes"
    );
    let train_fail_rate = train_fail as f64 / (train_pass + train_fail) as f64;
    let train_pass_rate = train_pass as f64 / (train_pass + train_fail) as f64;

    let mut skill_state_outcomes = BTreeMap::<String, BTreeMap<String, bool>>::new();
    for membership in memberships {
        if let Some(outcome) = train_target_outcomes.get(&membership.code_state_key) {
            skill_state_outcomes
                .entry(membership.skill_id.clone())
                .or_default()
                .insert(membership.code_state_key.clone(), *outcome);
        }
    }

    let mut descriptors = BTreeMap::new();
    let mut skill_kind_counts = BTreeMap::<String, usize>::new();
    let mut zero_train_support_skill_count = 0_usize;
    for skill in level2_skills {
        let state_outcomes = skill_state_outcomes
            .get(&skill.skill_id)
            .cloned()
            .unwrap_or_default();
        let support = state_outcomes.len() as u64;
        if support == 0 {
            zero_train_support_skill_count += 1;
        }
        let pass_count = state_outcomes.values().filter(|passed| **passed).count() as u64;
        let fail_count = support.saturating_sub(pass_count);
        let genericity = skill_genericity_score_for_steps(&skill.ordered_steps);
        let kind = if support > 0 {
            train_candidate_kind(pass_count, fail_count, genericity, train_fail_rate)
        } else {
            skill_candidate_kind_for_row(skill)
        };
        let kind_label = skill_kind_label(kind).to_string();
        *skill_kind_counts.entry(kind_label.clone()).or_default() += 1;
        let confidence = if support > 0 {
            pass_count.max(fail_count) as f64 / support as f64
        } else {
            skill.confidence
        };
        let lift = train_lift_for_kind(
            kind,
            pass_count,
            fail_count,
            support,
            train_pass_rate,
            train_fail_rate,
            skill.lift_over_cell_baseline,
        );
        let split_selection_weight =
            skill_split_selection_weight(kind, lift, confidence, skill.stability, genericity);
        descriptors.insert(
            skill.skill_id.clone(),
            SkillUsefulnessDescriptor {
                kind_label,
                split_selection_weight,
            },
        );
    }
    Ok(SkillUsefulnessBuild {
        descriptors,
        report: SkillUsefulnessPolicyReport {
            scoring_split: "train".to_string(),
            train_skill_profile_count: level2_skills.len(),
            skill_kind_counts,
            zero_train_support_skill_count,
            blanket_expansion_negative_control: BlanketExpansionNegativeControl {
                coverage_before: "1531/3000".to_string(),
                coverage_after: "2477/3000".to_string(),
                correlation_before: 0.849_787,
                correlation_after: 0.805_525,
                ece_before: 0.029_909,
                ece_after: 0.034_136,
                selected: false,
                reason: "coverage-only blanket expansion regressed #392 correlation and ECE; targeted usefulness scoring is required".to_string(),
            },
        },
    })
}

fn train_candidate_kind(
    pass_count: u64,
    fail_count: u64,
    genericity_score: f64,
    train_fail_rate: f64,
) -> SkillCandidateKind {
    let support = pass_count + fail_count;
    if support == 0 {
        return SkillCandidateKind::NeutralDiagnostic;
    }
    let pass_rate = pass_count as f64 / support as f64;
    let fail_rate = fail_count as f64 / support as f64;
    if genericity_score >= 0.70 && pass_rate >= 0.80 && pass_rate > 1.0 - train_fail_rate {
        SkillCandidateKind::ContextNegativeEvidence
    } else if fail_rate >= pass_rate && fail_rate > train_fail_rate {
        SkillCandidateKind::FailureSkill
    } else if pass_rate > fail_rate && pass_rate > 1.0 - train_fail_rate {
        SkillCandidateKind::PassStabilitySkill
    } else if genericity_score >= 0.85 {
        SkillCandidateKind::RejectOverbroadOrLeaky
    } else {
        SkillCandidateKind::NeutralDiagnostic
    }
}

fn train_lift_for_kind(
    kind: SkillCandidateKind,
    pass_count: u64,
    fail_count: u64,
    support: u64,
    train_pass_rate: f64,
    train_fail_rate: f64,
    fallback_lift: f64,
) -> f64 {
    if support == 0 {
        return fallback_lift.max(0.0);
    }
    let pass_rate = pass_count as f64 / support as f64;
    let fail_rate = fail_count as f64 / support as f64;
    match kind {
        SkillCandidateKind::FailureSkill => (fail_rate - train_fail_rate).max(0.0),
        SkillCandidateKind::PassStabilitySkill | SkillCandidateKind::ContextNegativeEvidence => {
            (pass_rate - train_pass_rate).max(0.0)
        }
        SkillCandidateKind::NeutralDiagnostic | SkillCandidateKind::RejectOverbroadOrLeaky => {
            fallback_lift.max(0.0) * 0.25
        }
    }
}

fn skill_split_selection_weight(
    kind: SkillCandidateKind,
    lift: f64,
    confidence: f64,
    stability: f64,
    genericity_score: f64,
) -> f64 {
    let kind_weight = match kind {
        SkillCandidateKind::FailureSkill => 1.0,
        SkillCandidateKind::PassStabilitySkill => 0.80,
        SkillCandidateKind::ContextNegativeEvidence => 0.55,
        SkillCandidateKind::NeutralDiagnostic => 0.20,
        SkillCandidateKind::RejectOverbroadOrLeaky => 0.0,
    };
    let genericity_penalty = if matches!(kind, SkillCandidateKind::ContextNegativeEvidence) {
        0.85
    } else {
        1.0 - (genericity_score * 0.60)
    };
    (lift.max(0.0) * confidence * stability * kind_weight * genericity_penalty).clamp(0.0, 1.0)
}

fn neutral_skill_descriptor(skill_id: &str) -> SkillUsefulnessDescriptor {
    SkillUsefulnessDescriptor {
        kind_label: format!("neutral_diagnostic:missing_descriptor:{skill_id}"),
        split_selection_weight: 0.0,
    }
}

fn skill_kind_label(kind: SkillCandidateKind) -> &'static str {
    match kind {
        SkillCandidateKind::FailureSkill => "failure_skill",
        SkillCandidateKind::PassStabilitySkill => "pass_stability_skill",
        SkillCandidateKind::ContextNegativeEvidence => "context_negative_evidence",
        SkillCandidateKind::NeutralDiagnostic => "neutral_diagnostic",
        SkillCandidateKind::RejectOverbroadOrLeaky => "reject_overbroad_or_leaky",
    }
}

fn empty_skill_usefulness_policy_report() -> SkillUsefulnessPolicyReport {
    SkillUsefulnessPolicyReport {
        scoring_split: "synthetic-empty".to_string(),
        train_skill_profile_count: 0,
        skill_kind_counts: BTreeMap::new(),
        zero_train_support_skill_count: 0,
        blanket_expansion_negative_control: BlanketExpansionNegativeControl {
            coverage_before: "0/0".to_string(),
            coverage_after: "0/0".to_string(),
            correlation_before: 0.0,
            correlation_after: 0.0,
            ece_before: 0.0,
            ece_after: 0.0,
            selected: false,
            reason: "synthetic edge-case placeholder".to_string(),
        },
    }
}

fn open_hierarchy_db_read_only(path: &Path) -> Result<DB> {
    ensure!(
        !path.to_string_lossy().starts_with("/mnt/d/"),
        "hierarchy DB active path must not use /mnt/d"
    );
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    let existing = DB::list_cf(&opts, path)
        .with_context(|| format!("list column families for {}", path.display()))?;
    for required in [
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
    ] {
        ensure!(
            existing.iter().any(|name| name == required),
            "hierarchy DB {} missing required column family {}",
            path.display(),
            required
        );
    }
    let descriptors = existing
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()))
        .collect::<Vec<_>>();
    Ok(DB::open_cf_descriptors_read_only(
        &opts,
        path,
        descriptors,
        false,
    )?)
}

fn metadata_features(entry: &CorpusEntry) -> Result<Vec<f32>> {
    let mut features = vec![0.0_f32; REPO_HASH_FEATURES + PATH_HASH_FEATURES];
    features[hash_index(&format!("repo:{}", entry.repo), REPO_HASH_FEATURES)] += 1.0;
    let patch_path = Path::new(CORPUS_DIR).join(&entry.patch_path);
    let patch_text = fs::read_to_string(&patch_path)
        .with_context(|| format!("read patch metadata from {}", patch_path.display()))?;
    for label in touched_path_labels(&patch_text) {
        let idx = REPO_HASH_FEATURES + hash_index(&label, PATH_HASH_FEATURES);
        features[idx] += 1.0;
    }
    Ok(features)
}

fn touched_path_labels(patch_text: &str) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for line in patch_text.lines() {
        let Some(path) = line
            .strip_prefix("+++ b/")
            .or_else(|| line.strip_prefix("--- a/"))
        else {
            continue;
        };
        labels.insert(format!("path:{path}"));
        let parts = path.split('/').collect::<Vec<_>>();
        if let Some(top) = parts.first() {
            labels.insert(format!("topdir:{top}"));
        }
        if parts.len() > 1 {
            labels.insert(format!("dir2:{}/{}", parts[0], parts[1]));
        }
        if let Some(file) = parts.last() {
            if let Some((_stem, ext)) = file.rsplit_once('.') {
                labels.insert(format!("ext:{ext}"));
            }
        }
    }
    labels.into_iter().collect()
}

fn target_workspace_state(category: &str) -> String {
    if category == "known_good" {
        "gold".to_string()
    } else {
        format!("mutation_{category}")
    }
}

fn required_state<'a>(
    states: &'a BTreeMap<(String, String), CodeStateConstellationRow>,
    task_id: &str,
    workspace_state: &str,
) -> Result<&'a CodeStateConstellationRow> {
    states
        .get(&(task_id.to_string(), workspace_state.to_string()))
        .ok_or_else(|| anyhow!("missing code-state row for {task_id}:{workspace_state}"))
}

fn ensure_ordered_states(
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
) -> Result<()> {
    ensure!(
        base.workspace_state == "base",
        "trajectory t0 must be base, got {}",
        base.workspace_state
    );
    ensure!(
        target.workspace_state != "base" && target.workspace_state != "test_patch",
        "trajectory t1 must be candidate workspace, got {}",
        target.workspace_state
    );
    ensure!(
        test_patch.workspace_state == "test_patch",
        "trajectory t2 must be test_patch, got {}",
        test_patch.workspace_state
    );
    ensure!(
        base.task_instance_id == target.task_instance_id
            && base.task_instance_id == test_patch.task_instance_id,
        "trajectory states must share a task id"
    );
    Ok(())
}

fn trajectory_feature_vector(
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
) -> Result<Vec<f32>> {
    ensure_ordered_states(base, target, test_patch)?;
    let v0 = slot_matrix(base)?;
    let v1 = slot_matrix(target)?;
    let v2 = slot_matrix(test_patch)?;
    let d10 = matrix_delta(&v1, &v0);
    let d21 = matrix_delta(&v2, &v1);
    let d20 = matrix_delta(&v2, &v0);
    let mut features = Vec::with_capacity(900);
    for matrix in [&v0, &v1, &v2, &d10, &d21, &d20] {
        flatten_matrix(matrix, &mut features, false);
    }
    for matrix in [&d10, &d21, &d20] {
        flatten_matrix(matrix, &mut features, true);
    }
    for matrix in [&d10, &d21, &d20] {
        for slot in matrix {
            features.push(l2_norm(slot));
        }
    }
    for matrix in [&d10, &d21, &d20] {
        add_named_cross_slot_scalar_relationships(matrix, &mut features)?;
    }
    features.push((target.chunk_count as f32).ln_1p() - (base.chunk_count as f32).ln_1p());
    features.push((test_patch.chunk_count as f32).ln_1p() - (target.chunk_count as f32).ln_1p());
    features.push(target.file_count as f32 - base.file_count as f32);
    features.push(test_patch.file_count as f32 - target.file_count as f32);

    let mut hashed = vec![0.0_f32; HASH_FEATURES];
    add_transition_labels(&mut hashed, "base_to_target", base, target);
    add_transition_labels(&mut hashed, "target_to_test", target, test_patch);
    add_transition_labels(&mut hashed, "base_to_test", base, test_patch);
    let denom = (base.chunk_count + target.chunk_count + test_patch.chunk_count) as f32;
    add_count_features(
        &mut hashed,
        "target_group",
        &target.group_label_counts_top,
        denom.max(1.0),
    );
    add_count_features(
        &mut hashed,
        "target_pair",
        &target.pair_label_counts_top,
        denom.max(1.0),
    );
    add_count_features(
        &mut hashed,
        "target_slot",
        &target.slot_label_counts_top,
        denom.max(1.0),
    );
    add_count_features(
        &mut hashed,
        "target_surface",
        &target.surface_counts_top,
        denom.max(1.0),
    );
    features.extend(hashed);
    Ok(features)
}

fn file_path_trajectory_feature_vector(
    file_states: &BTreeMap<(String, String, String), FilePathConstellationRow>,
    task_id: &str,
    target_workspace: &str,
) -> Result<Vec<f32>> {
    let base_rows = required_file_rows(file_states, task_id, "base")?;
    let target_rows = required_file_rows(file_states, task_id, target_workspace)?;
    let test_patch_rows = required_file_rows(file_states, task_id, "test_patch")?;
    let v0 = mean_slot_matrix(&base_rows)?;
    let v1 = mean_slot_matrix(&target_rows)?;
    let v2 = mean_slot_matrix(&test_patch_rows)?;
    let d10 = matrix_delta(&v1, &v0);
    let d21 = matrix_delta(&v2, &v1);
    let d20 = matrix_delta(&v2, &v0);
    let mut features = Vec::with_capacity(1800);
    for matrix in [&v0, &v1, &v2, &d10, &d21, &d20] {
        flatten_matrix(matrix, &mut features, false);
    }
    for matrix in [&d10, &d21, &d20] {
        flatten_matrix(matrix, &mut features, true);
    }
    for matrix in [&d10, &d21, &d20] {
        for slot in matrix {
            features.push(l2_norm(slot));
        }
        add_named_cross_slot_scalar_relationships(matrix, &mut features)?;
    }

    let base_paths = row_path_set(&base_rows)?;
    let target_paths = row_path_set(&target_rows)?;
    let test_paths = row_path_set(&test_patch_rows)?;
    add_path_set_features(&mut features, &base_paths, &target_paths, &test_paths);

    let mut hashed = vec![0.0_f32; HASH_FEATURES];
    add_file_transition_labels(&mut hashed, "file_base_to_target", &base_rows, &target_rows);
    add_file_transition_labels(
        &mut hashed,
        "file_target_to_test",
        &target_rows,
        &test_patch_rows,
    );
    add_file_transition_labels(
        &mut hashed,
        "file_base_to_test",
        &base_rows,
        &test_patch_rows,
    );
    add_file_count_features(&mut hashed, "file_target_group", &target_rows, |row| {
        &row.group_label_counts_top
    });
    add_file_count_features(&mut hashed, "file_target_pair", &target_rows, |row| {
        &row.pair_label_counts_top
    });
    add_file_count_features(&mut hashed, "file_target_slot", &target_rows, |row| {
        &row.slot_label_counts_top
    });
    add_file_count_features(&mut hashed, "file_target_surface", &target_rows, |row| {
        &row.surface_counts_top
    });
    features.extend(hashed);
    Ok(features)
}

fn required_file_rows<'a>(
    file_states: &'a BTreeMap<(String, String, String), FilePathConstellationRow>,
    task_id: &str,
    workspace_state: &str,
) -> Result<Vec<&'a FilePathConstellationRow>> {
    let rows = file_states
        .iter()
        .filter_map(|((row_task, row_state, _path), row)| {
            (row_task == task_id && row_state == workspace_state).then_some(row)
        })
        .collect::<Vec<_>>();
    ensure!(
        !rows.is_empty(),
        "missing file-path constellation rows for {task_id}:{workspace_state}"
    );
    Ok(rows)
}

fn row_path_set(rows: &[&FilePathConstellationRow]) -> Result<BTreeSet<String>> {
    rows.iter()
        .map(|row| {
            row.relative_path.clone().ok_or_else(|| {
                anyhow!(
                    "file-path row {}:{} missing relative_path",
                    row.task_instance_id,
                    row.workspace_state
                )
            })
        })
        .collect()
}

fn add_path_set_features(
    features: &mut Vec<f32>,
    base_paths: &BTreeSet<String>,
    target_paths: &BTreeSet<String>,
    test_paths: &BTreeSet<String>,
) {
    features.push((base_paths.len() as f32).ln_1p());
    features.push((target_paths.len() as f32).ln_1p());
    features.push((test_paths.len() as f32).ln_1p());
    features.push(set_jaccard(base_paths, target_paths));
    features.push(set_jaccard(target_paths, test_paths));
    features.push(set_jaccard(base_paths, test_paths));
    features.push(target_paths.difference(base_paths).count() as f32);
    features.push(base_paths.difference(target_paths).count() as f32);
    features.push(test_paths.difference(target_paths).count() as f32);
    features.push(target_paths.intersection(test_paths).count() as f32);
}

fn set_jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f32 {
    let union = left.union(right).count();
    if union == 0 {
        1.0
    } else {
        left.intersection(right).count() as f32 / union as f32
    }
}

fn add_file_transition_labels(
    hashed: &mut [f32],
    prefix: &str,
    from: &[&FilePathConstellationRow],
    to: &[&FilePathConstellationRow],
) {
    let from_labels = live_labels_from_rows(from);
    let to_labels = live_labels_from_rows(to);
    for label in to_labels.difference(&from_labels) {
        let idx = hash_index(&format!("{prefix}:added:{label}"), hashed.len());
        hashed[idx] += 1.0;
    }
    for label in from_labels.difference(&to_labels) {
        let idx = hash_index(&format!("{prefix}:removed:{label}"), hashed.len());
        hashed[idx] += 1.0;
    }
}

fn live_labels_from_rows(rows: &[&FilePathConstellationRow]) -> BTreeSet<String> {
    rows.iter()
        .flat_map(|row| live_labels(row).into_iter())
        .collect()
}

fn add_file_count_features<F>(
    hashed: &mut [f32],
    prefix: &str,
    rows: &[&FilePathConstellationRow],
    counts_fn: F,
) where
    F: Fn(&FilePathConstellationRow) -> &BTreeMap<String, u64>,
{
    let denom = rows
        .iter()
        .map(|row| row.chunk_count.max(1))
        .sum::<usize>()
        .max(1) as f32;
    for row in rows {
        add_count_features(hashed, prefix, counts_fn(row), denom);
    }
}

fn skill_trajectory_feature_vector(
    skill_index: &SkillFeatureIndex,
    base: &CodeStateConstellationRow,
    target: &CodeStateConstellationRow,
    test_patch: &CodeStateConstellationRow,
    policy: SkillFeaturePolicy,
) -> Result<Vec<f32>> {
    ensure_ordered_states(base, target, test_patch)?;
    ensure!(
        skill_index.level2_skill_count > 0
            && skill_index.membership_count > 0
            && !skill_index.hierarchy_report_sha256.is_empty(),
        "skill feature index readback is empty"
    );
    let base_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, base)?, policy);
    let target_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, target)?, policy);
    let test_summary =
        skill_projection_for_policy(skill_summary_for_state(skill_index, test_patch)?, policy);
    let mut features = Vec::with_capacity(220);
    let s0 = skill_summary_scalars(&base_summary, policy);
    let s1 = skill_summary_scalars(&target_summary, policy);
    let s2 = skill_summary_scalars(&test_summary, policy);
    for scalars in [&s0, &s1, &s2] {
        features.extend(scalars);
    }
    for (left, right) in [(&s1, &s0), (&s2, &s1), (&s2, &s0)] {
        for (l, r) in left.iter().zip(right) {
            features.push(l - r);
            features.push((l - r).abs());
        }
    }
    add_skill_set_transition_features(&mut features, &base_summary, &target_summary);
    add_skill_set_transition_features(&mut features, &target_summary, &test_summary);
    add_skill_set_transition_features(&mut features, &base_summary, &test_summary);

    let mut hashed = vec![0.0_f32; HASH_FEATURES];
    add_skill_transition_hashes(
        &mut hashed,
        "skill_base_to_target",
        &base_summary,
        &target_summary,
    );
    add_skill_transition_hashes(
        &mut hashed,
        "skill_target_to_test",
        &target_summary,
        &test_summary,
    );
    add_skill_transition_hashes(
        &mut hashed,
        "skill_base_to_test",
        &base_summary,
        &test_summary,
    );
    features.extend(hashed);
    Ok(features)
}

fn skill_summary_for_state<'a>(
    skill_index: &'a SkillFeatureIndex,
    row: &CodeStateConstellationRow,
) -> Result<&'a SkillStateSummary> {
    let key = code_state_key(row);
    skill_index
        .by_code_state
        .get(&key)
        .ok_or_else(|| anyhow!("missing skill summary for code state {key}"))
}

fn skill_projection_for_policy(
    summary: &SkillStateSummary,
    policy: SkillFeaturePolicy,
) -> SkillKindStateSummary {
    if matches!(policy, SkillFeaturePolicy::RawCurrent) {
        return raw_skill_projection(summary);
    }
    let mut projection = SkillKindStateSummary::default();
    for kind in policy.included_kinds() {
        if let Some(kind_summary) = summary.kind_summaries.get(*kind) {
            merge_skill_projection(&mut projection, kind_summary);
        }
    }
    projection
}

fn raw_skill_projection(summary: &SkillStateSummary) -> SkillKindStateSummary {
    SkillKindStateSummary {
        level2_skill_ids: summary.level2_skill_ids.clone(),
        higher_ability_ids: summary.higher_ability_ids.clone(),
        chunk_ids: summary.chunk_ids.clone(),
        file_paths: summary.file_paths.clone(),
        accepted_label_ids: summary.accepted_label_ids.clone(),
        group_ids: summary.group_ids.clone(),
        membership_count: summary.membership_count,
        ordered_step_count: summary.ordered_step_count,
        multi_step_membership_count: summary.multi_step_membership_count,
        raw_membership_score_sum: summary.membership_score_sum,
        raw_membership_score_max: summary.membership_score_max,
        usefulness_weight_sum: summary.usefulness_weight_sum,
        usefulness_weight_max: summary.usefulness_weight_max,
    }
}

fn merge_skill_projection(target: &mut SkillKindStateSummary, source: &SkillKindStateSummary) {
    target
        .level2_skill_ids
        .extend(source.level2_skill_ids.iter().cloned());
    target
        .higher_ability_ids
        .extend(source.higher_ability_ids.iter().cloned());
    target.chunk_ids.extend(source.chunk_ids.iter().cloned());
    target.file_paths.extend(source.file_paths.iter().cloned());
    target
        .accepted_label_ids
        .extend(source.accepted_label_ids.iter().cloned());
    target.group_ids.extend(source.group_ids.iter().cloned());
    target.membership_count += source.membership_count;
    target.ordered_step_count += source.ordered_step_count;
    target.multi_step_membership_count += source.multi_step_membership_count;
    target.raw_membership_score_sum += source.raw_membership_score_sum;
    target.raw_membership_score_max = target
        .raw_membership_score_max
        .max(source.raw_membership_score_max);
    target.usefulness_weight_sum += source.usefulness_weight_sum;
    target.usefulness_weight_max = target
        .usefulness_weight_max
        .max(source.usefulness_weight_max);
}

fn skill_summary_scalars(summary: &SkillKindStateSummary, policy: SkillFeaturePolicy) -> Vec<f32> {
    let membership_count = summary.membership_count.max(1) as f32;
    let mut scalars = vec![
        (summary.level2_skill_ids.len() as f32).ln_1p(),
        (summary.higher_ability_ids.len() as f32).ln_1p(),
        (summary.chunk_ids.len() as f32).ln_1p(),
        (summary.file_paths.len() as f32).ln_1p(),
        (summary.accepted_label_ids.len() as f32).ln_1p(),
        (summary.group_ids.len() as f32).ln_1p(),
        (summary.membership_count as f32).ln_1p(),
        summary.ordered_step_count as f32 / membership_count,
        summary.multi_step_membership_count as f32 / membership_count,
        (summary.raw_membership_score_sum as f32 / membership_count).clamp(0.0, 1.0),
        (summary.raw_membership_score_max as f32).clamp(0.0, 1.0),
    ];
    if !matches!(policy, SkillFeaturePolicy::RawCurrent) {
        scalars.push((summary.usefulness_weight_sum as f32 / membership_count).clamp(0.0, 1.0));
        scalars.push((summary.usefulness_weight_max as f32).clamp(0.0, 1.0));
    }
    scalars
}

fn add_skill_set_transition_features(
    features: &mut Vec<f32>,
    from: &SkillKindStateSummary,
    to: &SkillKindStateSummary,
) {
    features.push(set_jaccard(&from.level2_skill_ids, &to.level2_skill_ids));
    features.push(set_jaccard(
        &from.higher_ability_ids,
        &to.higher_ability_ids,
    ));
    features.push(
        to.level2_skill_ids
            .difference(&from.level2_skill_ids)
            .count() as f32,
    );
    features.push(
        from.level2_skill_ids
            .difference(&to.level2_skill_ids)
            .count() as f32,
    );
    features.push(
        to.higher_ability_ids
            .difference(&from.higher_ability_ids)
            .count() as f32,
    );
    features.push(
        from.higher_ability_ids
            .difference(&to.higher_ability_ids)
            .count() as f32,
    );
    features.push(set_jaccard(
        &from.accepted_label_ids,
        &to.accepted_label_ids,
    ));
    features.push(set_jaccard(&from.group_ids, &to.group_ids));
}

fn add_skill_transition_hashes(
    hashed: &mut [f32],
    prefix: &str,
    from: &SkillKindStateSummary,
    to: &SkillKindStateSummary,
) {
    add_set_delta_hashes(
        hashed,
        prefix,
        "level2",
        &from.level2_skill_ids,
        &to.level2_skill_ids,
    );
    add_set_delta_hashes(
        hashed,
        prefix,
        "ability",
        &from.higher_ability_ids,
        &to.higher_ability_ids,
    );
    add_set_delta_hashes(
        hashed,
        prefix,
        "label",
        &from.accepted_label_ids,
        &to.accepted_label_ids,
    );
    add_set_delta_hashes(hashed, prefix, "group", &from.group_ids, &to.group_ids);
}

fn add_set_delta_hashes(
    hashed: &mut [f32],
    prefix: &str,
    kind: &str,
    from: &BTreeSet<String>,
    to: &BTreeSet<String>,
) {
    for value in to.difference(from) {
        let idx = hash_index(&format!("{prefix}:{kind}:added:{value}"), hashed.len());
        hashed[idx] += 1.0;
    }
    for value in from.difference(to) {
        let idx = hash_index(&format!("{prefix}:{kind}:removed:{value}"), hashed.len());
        hashed[idx] += 1.0;
    }
}

fn code_state_key(row: &CodeStateConstellationRow) -> String {
    format!(
        "{}|{}|{}",
        row.task_instance_id, row.mutation_category, row.workspace_state
    )
}

fn slot_matrix(row: &CodeStateConstellationRow) -> Result<Vec<Vec<f32>>> {
    SLOT_IDS
        .iter()
        .map(|embedder_id| {
            let slot = row.slot_feature_means.get(*embedder_id).ok_or_else(|| {
                anyhow!(
                    "missing slot feature means for {}:{}:{}:{}",
                    row.task_instance_id,
                    row.mutation_category,
                    row.workspace_state,
                    embedder_id
                )
            })?;
            SLOT_SCALAR_NAMES
                .iter()
                .map(|feature_name| {
                    let value = *slot.get(*feature_name).ok_or_else(|| {
                        anyhow!(
                            "missing slot scalar {} for {}:{}:{}:{}",
                            feature_name,
                            row.task_instance_id,
                            row.mutation_category,
                            row.workspace_state,
                            embedder_id
                        )
                    })? as f32;
                    ensure!(
                        value.is_finite(),
                        "non-finite slot scalar {} for {}:{}:{}:{}",
                        feature_name,
                        row.task_instance_id,
                        row.mutation_category,
                        row.workspace_state,
                        embedder_id
                    );
                    Ok(value)
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect()
}

fn mean_slot_matrix(rows: &[&CodeStateConstellationRow]) -> Result<Vec<Vec<f32>>> {
    ensure!(
        !rows.is_empty(),
        "cannot aggregate an empty file trajectory state"
    );
    let mut matrix = vec![vec![0.0_f32; SLOT_SCALAR_NAMES.len()]; SLOT_IDS.len()];
    for row in rows {
        let row_matrix = slot_matrix(row)?;
        for (target_slot, row_slot) in matrix.iter_mut().zip(row_matrix) {
            for (target_value, row_value) in target_slot.iter_mut().zip(row_slot) {
                *target_value += row_value;
            }
        }
    }
    let denom = rows.len() as f32;
    for slot in &mut matrix {
        for value in slot {
            *value /= denom;
        }
    }
    Ok(matrix)
}

fn matrix_delta(left: &[Vec<f32>], right: &[Vec<f32>]) -> Vec<Vec<f32>> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.iter().zip(right).map(|(l, r)| l - r).collect())
        .collect()
}

fn flatten_matrix(matrix: &[Vec<f32>], out: &mut Vec<f32>, absolute: bool) {
    for slot in matrix {
        for value in slot {
            out.push(if absolute { value.abs() } else { *value });
        }
    }
}

fn add_named_cross_slot_scalar_relationships(
    matrix: &[Vec<f32>],
    out: &mut Vec<f32>,
) -> Result<()> {
    ensure!(
        matrix.len() == SLOT_IDS.len()
            && matrix
                .iter()
                .all(|slot| slot.len() == SLOT_SCALAR_NAMES.len()),
        "cross-slot scalar relationship matrix shape mismatch"
    );
    let pair_count = (SLOT_IDS.len() * (SLOT_IDS.len() - 1) / 2) as f32;
    for scalar_idx in 0..SLOT_SCALAR_NAMES.len() {
        let mut abs_gap_sum = 0.0_f32;
        let mut abs_gap_max = 0.0_f32;
        let mut same_sign = 0.0_f32;
        let mut opposite_sign = 0.0_f32;
        for left in 0..matrix.len() {
            for right in (left + 1)..matrix.len() {
                let left_value = matrix[left][scalar_idx];
                let right_value = matrix[right][scalar_idx];
                let gap = (left_value - right_value).abs();
                abs_gap_sum += gap;
                abs_gap_max = abs_gap_max.max(gap);
                match sign_agreement(left_value, right_value).partial_cmp(&0.0) {
                    Some(Ordering::Greater) => same_sign += 1.0,
                    Some(Ordering::Less) => opposite_sign += 1.0,
                    _ => {}
                }
            }
        }
        out.push(abs_gap_sum / pair_count);
        out.push(abs_gap_max);
        out.push(same_sign / pair_count);
        out.push(opposite_sign / pair_count);
    }
    Ok(())
}

fn sign_agreement(left: f32, right: f32) -> f32 {
    if left.abs() <= f32::EPSILON || right.abs() <= f32::EPSILON {
        0.0
    } else if left.signum() == right.signum() {
        1.0
    } else {
        -1.0
    }
}

fn add_transition_labels(
    hashed: &mut [f32],
    prefix: &str,
    from: &CodeStateConstellationRow,
    to: &CodeStateConstellationRow,
) {
    let from_labels = live_labels(from);
    let to_labels = live_labels(to);
    for label in to_labels.difference(&from_labels) {
        let idx = hash_index(&format!("{prefix}:added:{label}"), hashed.len());
        hashed[idx] += 1.0;
    }
    for label in from_labels.difference(&to_labels) {
        let idx = hash_index(&format!("{prefix}:removed:{label}"), hashed.len());
        hashed[idx] += 1.0;
    }
}

fn live_labels(row: &CodeStateConstellationRow) -> BTreeSet<String> {
    row.labels
        .iter()
        .filter(|label| is_live_label(label))
        .cloned()
        .collect()
}

fn is_live_label(label: &str) -> bool {
    !TARGET_LABEL_PREFIXES
        .iter()
        .any(|target_prefix| label.starts_with(target_prefix))
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn add_count_features(
    hashed: &mut [f32],
    prefix: &str,
    counts: &BTreeMap<String, u64>,
    denom: f32,
) {
    for (label, count) in counts {
        if TARGET_LABEL_PREFIXES
            .iter()
            .any(|target_prefix| label.starts_with(target_prefix))
        {
            continue;
        }
        let feature = format!("{prefix}:{label}");
        let idx = hash_index(&feature, hashed.len());
        hashed[idx] += *count as f32 / denom;
    }
}

fn hash_index(value: &str, modulus: usize) -> usize {
    let digest = Sha256::digest(value.as_bytes());
    let prefix = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as usize;
    prefix % modulus
}

fn standardize_features(rows: &mut [FeatureRow], raw_features: &mut [Vec<f32>]) -> Result<()> {
    let feature_dim = raw_features
        .first()
        .map(|features| features.len())
        .unwrap_or_default();
    ensure!(feature_dim > 0, "feature dimension must be non-zero");
    let train_indices = rows
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| (row.bucket == "train").then_some(idx))
        .collect::<Vec<_>>();
    ensure!(!train_indices.is_empty(), "train split is empty");
    for (idx, features) in raw_features.iter().enumerate() {
        ensure!(
            features.len() == feature_dim,
            "feature dimension mismatch for row {idx}"
        );
        for (feature_idx, value) in features.iter().enumerate() {
            ensure!(
                value.is_finite(),
                "non-finite raw feature row={idx} feature={feature_idx}"
            );
        }
    }
    let mut means = vec![0.0_f64; feature_dim];
    for idx in &train_indices {
        for (mean, value) in means.iter_mut().zip(&raw_features[*idx]) {
            *mean += *value as f64;
        }
    }
    for mean in &mut means {
        *mean /= train_indices.len() as f64;
    }
    let mut stds = vec![0.0_f64; feature_dim];
    for idx in &train_indices {
        for ((std, value), mean) in stds.iter_mut().zip(&raw_features[*idx]).zip(&means) {
            *std += (*value as f64 - *mean).powi(2);
        }
    }
    for std in &mut stds {
        *std = (*std / train_indices.len() as f64).sqrt();
        if *std < 1.0e-9 {
            *std = 1.0;
        }
    }
    for (row, features) in rows.iter_mut().zip(raw_features) {
        let mut standardized_features = Vec::with_capacity(feature_dim);
        for (feature_idx, ((value, mean), std)) in
            features.iter().zip(&means).zip(&stds).enumerate()
        {
            let standardized = ((*value as f64 - *mean) / *std) as f32;
            ensure!(
                standardized.is_finite(),
                "non-finite standardized feature task={} category={} feature={}",
                row.task_id,
                row.category,
                feature_idx
            );
            standardized_features.push(standardized);
        }
        row.standardized_features = standardized_features;
    }
    Ok(())
}

fn skill_policy_ablation_report(
    base_feature_table: &FeatureTable,
    no_skill_feature_table: &FeatureTable,
    raw_skill_feature_table: &FeatureTable,
    failure_skill_feature_table: &FeatureTable,
    pass_stability_skill_feature_table: &FeatureTable,
    context_skill_feature_table: &FeatureTable,
    usefulness_feature_table: &FeatureTable,
    baseline_metrics: &MetricBlock,
) -> Result<SkillPolicyAblationReport> {
    let raw_eval = evaluate_skill_policy(
        SkillFeaturePolicy::RawCurrent.label(),
        raw_skill_feature_table,
        baseline_metrics,
        None,
        None,
        true,
    )?;
    let raw_calibration_metrics = raw_eval.mean_bias.calibration.metrics.clone();
    let raw_selection_objective = raw_eval.mean_bias.selection_objective;
    let mut evaluations = vec![
        evaluate_skill_policy(
            "code_state_only",
            base_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            false,
        )?,
        evaluate_skill_policy(
            "no_skill_file_trajectory",
            no_skill_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            false,
        )?,
        raw_eval,
        evaluate_skill_policy(
            SkillFeaturePolicy::FailureOnly.label(),
            failure_skill_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::PassStabilityOnly.label(),
            pass_stability_skill_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::ContextOnly.label(),
            context_skill_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::UsefulnessCombined.label(),
            usefulness_feature_table,
            baseline_metrics,
            Some(&raw_calibration_metrics),
            Some(raw_selection_objective),
            true,
        )?,
    ];
    for eval in &mut evaluations {
        if eval.policy == SkillFeaturePolicy::RawCurrent.label() {
            eval.delta_vs_raw_current = Some(metric_delta(
                "current_raw_skill_surface",
                "current_raw_skill_surface",
                &raw_calibration_metrics,
                &eval.mean_bias.calibration.metrics,
            ));
            eval.selection_eligible = true;
            eval.rejection_reason = None;
        }
    }

    let mut selected = evaluations
        .iter()
        .filter(|eval| eval.selection_eligible)
        .min_by(|left, right| {
            left.mean_bias
                .selection_objective
                .partial_cmp(&right.mean_bias.selection_objective)
                .unwrap_or(Ordering::Equal)
        })
        .ok_or_else(|| anyhow!("skill policy ablation produced no selectable policy"))?
        .clone();
    if selected.policy != SkillFeaturePolicy::UsefulnessCombined.label() {
        let useful = evaluations
            .iter()
            .find(|eval| eval.policy == SkillFeaturePolicy::UsefulnessCombined.label())
            .ok_or_else(|| anyhow!("missing usefulness-weighted skill policy evaluation"))?;
        if useful.selection_eligible
            && skill_policy_objective_eligible(
                useful.delta_vs_raw_current.as_ref(),
                useful.mean_bias.selection_objective,
                raw_selection_objective,
            )
            && useful.mean_bias.selection_objective
                <= selected.mean_bias.selection_objective + 0.002
        {
            selected = useful.clone();
        }
    }
    let selected_reason = if selected.policy == SkillFeaturePolicy::UsefulnessCombined.label() {
        "usefulness-weighted ordered skills selected by calibration-only objective gate".to_string()
    } else {
        "usefulness-weighted ordered skills evaluated but not selected; current raw skill surface remains the active policy".to_string()
    };
    let typed_blocker = if selected.policy == SkillFeaturePolicy::UsefulnessCombined.label()
        && selected.mean_bias.holdout.metrics.ece < BASELINE_536_ECE
    {
        "TASK_SKILL_002_USEFULNESS_WEIGHTED_CLEAR".to_string()
    } else if selected.selection_eligible {
        "TASK_SKILL_002_USEFULNESS_WEIGHTED_OBJECTIVE_CLEAR_BUT_ECE_BLOCKED".to_string()
    } else {
        "TASK_SKILL_002_USEFULNESS_WEIGHTED_REGRESSED_REJECTED".to_string()
    };
    let all_passed = evaluations
        .iter()
        .any(|eval| eval.policy == "no_skill_file_trajectory")
        && evaluations
            .iter()
            .any(|eval| eval.policy == SkillFeaturePolicy::RawCurrent.label())
        && evaluations
            .iter()
            .any(|eval| eval.policy == SkillFeaturePolicy::UsefulnessCombined.label())
        && selected.selection_eligible;
    Ok(SkillPolicyAblationReport {
        all_passed,
        selected_policy: selected.policy,
        selected_reason,
        typed_blocker,
        evaluations,
    })
}

fn evaluate_skill_policy(
    policy: &str,
    table: &FeatureTable,
    baseline_metrics: &MetricBlock,
    raw_reference: Option<&MetricBlock>,
    raw_selection_objective: Option<f32>,
    skill_policy_selectable: bool,
) -> Result<SkillPolicyEvaluation> {
    let current_rbf = candidate_split_report(table, CURRENT_RBF)?;
    let mean_bias = select_mean_bias_calibration_repair(table)?;
    let delta_vs_536 = metric_delta(
        "baseline_536_rank_lift",
        policy,
        baseline_metrics,
        &mean_bias.holdout.metrics,
    );
    let delta_vs_raw_current = raw_reference.map(|raw| {
        metric_delta(
            "current_raw_skill_surface",
            policy,
            raw,
            &mean_bias.calibration.metrics,
        )
    });
    let selection_eligible = if skill_policy_selectable {
        match (&delta_vs_raw_current, raw_selection_objective) {
            (Some(delta), Some(raw_objective)) => skill_policy_objective_eligible(
                Some(delta),
                mean_bias.selection_objective,
                raw_objective,
            ),
            (Some(delta), None) => delta.non_regressing,
            (None, _) => true,
        }
    } else {
        false
    };
    let rejection_reason = if selection_eligible {
        None
    } else if skill_policy_selectable {
        Some(format!(
            "policy failed calibration-only objective gate versus current raw skill surface; require non-regression or objective improvement >= {SKILL_POLICY_OBJECTIVE_IMPROVEMENT_MARGIN:.3} with ece regression <= {SKILL_POLICY_ECE_REGRESSION_CAP:.3}"
        ))
    } else {
        Some("baseline-only comparison; not a selectable skill policy".to_string())
    };
    Ok(SkillPolicyEvaluation {
        policy: policy.to_string(),
        feature_dim: table.feature_dim,
        current_rbf,
        mean_bias,
        delta_vs_raw_current,
        delta_vs_536,
        selection_eligible,
        rejection_reason,
    })
}

fn skill_policy_objective_eligible(
    delta_vs_raw: Option<&MetricDeltaReport>,
    candidate_objective: f32,
    raw_objective: f32,
) -> bool {
    let objective_improved =
        candidate_objective + SKILL_POLICY_OBJECTIVE_IMPROVEMENT_MARGIN <= raw_objective;
    let ece_regression_within_cap = delta_vs_raw
        .map(|delta| delta.ece_delta <= SKILL_POLICY_ECE_REGRESSION_CAP)
        .unwrap_or(true);
    let non_regressing = delta_vs_raw
        .map(|delta| delta.non_regressing)
        .unwrap_or(true);
    non_regressing || (objective_improved && ece_regression_within_cap)
}

fn selected_skill_feature_table<'a>(
    report: &SkillPolicyAblationReport,
    raw: &'a FeatureTable,
    failure: &'a FeatureTable,
    pass_stability: &'a FeatureTable,
    context: &'a FeatureTable,
    usefulness: &'a FeatureTable,
) -> &'a FeatureTable {
    match report.selected_policy.as_str() {
        "usefulness_weighted_combined" => usefulness,
        "failure_only" => failure,
        "pass_stability_only" => pass_stability,
        "context_negative_evidence_only" => context,
        _ => raw,
    }
}

fn metric_delta(
    reference: &str,
    candidate: &str,
    reference_metrics: &MetricBlock,
    candidate_metrics: &MetricBlock,
) -> MetricDeltaReport {
    let correlation_delta = reference_metrics
        .prediction_oracle_correlation
        .zip(candidate_metrics.prediction_oracle_correlation)
        .map(|(reference, candidate)| candidate - reference);
    let brier_delta = candidate_metrics.brier - reference_metrics.brier;
    let ece_delta = candidate_metrics.ece - reference_metrics.ece;
    MetricDeltaReport {
        reference: reference.to_string(),
        candidate: candidate.to_string(),
        reference_correlation: reference_metrics.prediction_oracle_correlation,
        candidate_correlation: candidate_metrics.prediction_oracle_correlation,
        correlation_delta,
        reference_brier: reference_metrics.brier,
        candidate_brier: candidate_metrics.brier,
        brier_delta,
        reference_ece: reference_metrics.ece,
        candidate_ece: candidate_metrics.ece,
        ece_delta,
        non_regressing: correlation_delta
            .map(|delta| delta >= -0.002)
            .unwrap_or(false)
            && brier_delta <= 0.002
            && ece_delta <= 0.002,
        ece_improved: ece_delta < 0.0,
    }
}

fn rank_candidate_sweep(table: &FeatureTable) -> Result<Vec<RankedCandidate>> {
    let mut ranked = Vec::new();
    for candidate in candidates() {
        let split = candidate_split_report(table, candidate)?;
        let objective = selection_objective(&split.calibration.metrics);
        let blockers = repair_blockers(&split.holdout.metrics, &split.holdout.per_category_metrics);
        ranked.push(RankedCandidate {
            rank: 0,
            candidate,
            selection_objective: objective,
            calibration: split.calibration,
            holdout: split.holdout,
            blockers,
        });
    }
    ranked.sort_by(|left, right| {
        left.selection_objective
            .partial_cmp(&right.selection_objective)
            .unwrap_or(Ordering::Equal)
    });
    for (idx, candidate) in ranked.iter_mut().enumerate() {
        candidate.rank = idx + 1;
    }
    ranked.truncate(12);
    Ok(ranked)
}

fn candidates() -> Vec<Candidate> {
    let mut out = Vec::new();
    for tau in [2.0, 5.0, 10.0, 20.0, 50.0] {
        for fail_boost in [1.0, 1.25, 1.5, 2.0] {
            out.push(Candidate::CategoryLocalRbf { tau, fail_boost });
        }
    }
    for k in [31, 61, 121, 181, 239] {
        for fail_boost in [1.0, 1.25, 1.5, 2.0] {
            out.push(Candidate::CategoryLocalKnn { k, fail_boost });
        }
    }
    out
}

fn candidate_split_report(table: &FeatureTable, candidate: Candidate) -> Result<SplitMetrics> {
    Ok(SplitMetrics {
        candidate,
        calibration: split_report(table, "calibration", |idx| {
            score_candidate(table, idx, candidate)
        })?,
        holdout: split_report(table, "holdout", |idx| {
            score_candidate(table, idx, candidate)
        })?,
    })
}

fn split_report<F>(table: &FeatureTable, bucket: &str, mut score_fn: F) -> Result<SplitReport>
where
    F: FnMut(usize) -> Result<f32>,
{
    let rows = table
        .rows
        .iter()
        .enumerate()
        .filter(|(_idx, row)| row.bucket == bucket)
        .map(|(idx, row)| {
            let score = score_fn(idx)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn prediction_rows_for_candidate(
    table: &FeatureTable,
    bucket: &str,
    candidate: Candidate,
) -> Result<Vec<PredictionRow>> {
    table
        .rows
        .iter()
        .enumerate()
        .filter(|(_idx, row)| row.bucket == bucket)
        .map(|(idx, row)| {
            let score = score_candidate(table, idx, candidate)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn score_candidate(table: &FeatureTable, row_idx: usize, candidate: Candidate) -> Result<f32> {
    match candidate {
        Candidate::CategoryLocalRbf { tau, fail_boost } => {
            category_local_rbf_score(table, row_idx, tau, fail_boost)
        }
        Candidate::CategoryLocalKnn { k, fail_boost } => {
            category_local_knn_score(table, row_idx, k, fail_boost)
        }
    }
}

fn category_local_rbf_score(
    table: &FeatureTable,
    row_idx: usize,
    tau: f32,
    fail_boost: f32,
) -> Result<f32> {
    ensure!(tau > 0.0, "RBF tau must be positive");
    ensure!(fail_boost > 0.0, "RBF fail_boost must be positive");
    let row = table
        .rows
        .get(row_idx)
        .ok_or_else(|| anyhow!("row index out of range"))?;
    let train_indices = category_train_indices(table, &row.category)?;
    let distances = train_indices
        .iter()
        .map(|train_idx| {
            let train = table
                .rows
                .get(*train_idx)
                .ok_or_else(|| anyhow!("train index out of range"))?;
            Ok((
                *train_idx,
                squared_distance(&row.standardized_features, &train.standardized_features),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let min_distance = distances
        .iter()
        .map(|(_idx, distance)| *distance)
        .fold(f32::INFINITY, f32::min);
    let mut weight_sum = 0.0_f64;
    let mut weighted_label_sum = 0.0_f64;
    for (train_idx, distance) in distances {
        let train = table
            .rows
            .get(train_idx)
            .ok_or_else(|| anyhow!("train index out of range"))?;
        let class_weight = if train.actual_oracle_pass {
            1.0
        } else {
            fail_boost as f64
        };
        let shifted_distance = (distance - min_distance).max(0.0);
        let weight =
            (-(shifted_distance as f64) / (2.0 * (tau as f64).powi(2))).exp() * class_weight;
        weight_sum += weight;
        if train.actual_oracle_pass {
            weighted_label_sum += weight;
        }
    }
    ensure!(
        weight_sum.is_finite() && weight_sum > 0.0,
        "RBF weight sum must be positive"
    );
    Ok((weighted_label_sum / weight_sum).clamp(0.001, 0.999) as f32)
}

fn category_local_knn_score(
    table: &FeatureTable,
    row_idx: usize,
    k: usize,
    fail_boost: f32,
) -> Result<f32> {
    ensure!(k > 0, "KNN k must be positive");
    ensure!(fail_boost > 0.0, "KNN fail_boost must be positive");
    let row = table
        .rows
        .get(row_idx)
        .ok_or_else(|| anyhow!("row index out of range"))?;
    let mut distances = category_train_indices(table, &row.category)?
        .iter()
        .map(|train_idx| {
            let train = &table.rows[*train_idx];
            (
                squared_distance(&row.standardized_features, &train.standardized_features),
                train.actual_oracle_pass,
            )
        })
        .collect::<Vec<_>>();
    distances.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(Ordering::Equal));
    let mut weight_sum = 0.0_f64;
    let mut weighted_label_sum = 0.0_f64;
    for (_distance, actual_pass) in distances.into_iter().take(k) {
        let class_weight = if actual_pass { 1.0 } else { fail_boost as f64 };
        weight_sum += class_weight;
        if actual_pass {
            weighted_label_sum += class_weight;
        }
    }
    ensure!(
        weight_sum.is_finite() && weight_sum > 0.0,
        "KNN weight sum must be positive"
    );
    Ok((weighted_label_sum / weight_sum).clamp(0.001, 0.999) as f32)
}

fn category_train_indices<'a>(table: &'a FeatureTable, category: &str) -> Result<&'a [usize]> {
    table
        .train_indices_by_category
        .get(category)
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow!("category {category} has no train rows"))
}

fn select_threshold_repair(
    table: &FeatureTable,
    selected_on: ThresholdSelectionSplit,
    spec: ThresholdRepairSpec,
) -> Result<ThresholdRepairReport> {
    let selection_bucket = match selected_on {
        ThresholdSelectionSplit::Calibration => "calibration",
        ThresholdSelectionSplit::Holdout => "holdout",
    };
    let thresholds =
        threshold_candidates(table, selection_bucket, spec.category, spec.probe_candidate)?;
    let mut best: Option<ThresholdRepairReport> = None;
    for threshold in thresholds {
        for low in spec.low_values {
            for high in spec.high_values {
                let calibration =
                    threshold_split_report(table, "calibration", spec, threshold, *low, *high)?;
                let holdout =
                    threshold_split_report(table, "holdout", spec, threshold, *low, *high)?;
                let selection_report = if selection_bucket == "calibration" {
                    &calibration
                } else {
                    &holdout
                };
                let objective = selection_objective(&selection_report.metrics);
                let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                let candidate = ThresholdRepairReport {
                    selected_on: selection_bucket.to_string(),
                    selection_forbidden: selection_bucket == "holdout",
                    forbidden_reason: (selection_bucket == "holdout")
                        .then_some("threshold_selected_from_holdout_scores".to_string()),
                    category: spec.category.to_string(),
                    base_candidate: spec.base_candidate,
                    probe_candidate: spec.probe_candidate,
                    threshold,
                    low: *low,
                    high: *high,
                    selection_objective: objective,
                    calibration,
                    holdout,
                    blockers,
                };
                if best
                    .as_ref()
                    .map(|previous| candidate.selection_objective < previous.selection_objective)
                    .unwrap_or(true)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("threshold sweep produced no candidates"))
}

fn threshold_split_report(
    table: &FeatureTable,
    bucket: &str,
    spec: ThresholdRepairSpec,
    threshold: f32,
    low: f32,
    high: f32,
) -> Result<SplitReport> {
    split_report(table, bucket, |idx| {
        let row = &table.rows[idx];
        if row.category == spec.category {
            let probe = score_candidate(table, idx, spec.probe_candidate)?;
            if probe >= threshold {
                Ok(high)
            } else {
                Ok(low)
            }
        } else {
            score_candidate(table, idx, spec.base_candidate)
        }
    })
}

fn threshold_candidates(
    table: &FeatureTable,
    bucket: &str,
    category: &str,
    probe_candidate: Candidate,
) -> Result<Vec<f32>> {
    let mut scores = table
        .rows
        .iter()
        .enumerate()
        .filter(|(_idx, row)| row.bucket == bucket && row.category == category)
        .map(|(idx, _row)| score_candidate(table, idx, probe_candidate))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !scores.is_empty(),
        "no threshold scores for {bucket}:{category}"
    );
    scores.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    scores.dedup_by(|left, right| (*left - *right).abs() <= 1.0e-6);
    let mut thresholds = BTreeSet::<OrderedF32>::new();
    thresholds.insert(OrderedF32(scores[0] - 1.0e-6));
    for window in scores.windows(2) {
        thresholds.insert(OrderedF32((window[0] + window[1]) / 2.0));
    }
    thresholds.insert(OrderedF32(scores[scores.len() - 1] + 1.0e-6));
    Ok(thresholds.into_iter().map(|value| value.0).collect())
}

fn select_affine_calibration_repair(table: &FeatureTable) -> Result<AffineCalibrationReport> {
    let mut best: Option<AffineCalibrationReport> = None;
    for base_candidate in probability_calibration_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for model in affine_calibration_models_for_candidate(base_candidate) {
            let calibration = affine_calibration_split_report(&calibration_base_rows, &model)?;
            let holdout = affine_calibration_split_report(&holdout_base_rows, &model)?;
            let selection_objective = selection_objective(&calibration.metrics);
            let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
            let report = AffineCalibrationReport {
                selected_on: "calibration".to_string(),
                selection_forbidden: false,
                forbidden_reason: None,
                selection_objective,
                model,
                calibration,
                holdout,
                blockers,
            };
            if best
                .as_ref()
                .map(|previous| report.selection_objective < previous.selection_objective)
                .unwrap_or(true)
            {
                best = Some(report);
            }
        }
    }
    best.ok_or_else(|| anyhow!("affine calibration sweep produced no candidates"))
}

fn affine_calibration_models_for_candidate(
    base_candidate: Candidate,
) -> Vec<AffineCalibrationModel> {
    let mut models = Vec::new();
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for scale in [0.70_f32, 0.85, 1.0, 1.15, 1.30] {
            for offset in [-0.08_f32, -0.06, -0.04, -0.02, 0.0, 0.02, 0.04, 0.06, 0.08] {
                models.push(AffineCalibrationModel {
                    selected_on: "calibration".to_string(),
                    scope,
                    base_candidate,
                    scale,
                    offset,
                });
            }
        }
    }
    models
}

fn affine_calibration_split_report(
    rows: &[PredictionRow],
    model: &AffineCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = rows
        .iter()
        .map(|row| {
            let score = apply_affine_calibration(row, model);
            PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            }
        })
        .collect::<Vec<_>>();
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn apply_affine_calibration(row: &PredictionRow, model: &AffineCalibrationModel) -> f32 {
    if !calibration_scope_applies(model.scope, &row.category) {
        return row.predicted_oracle_pass;
    }
    (0.5 + model.scale * (row.predicted_oracle_pass - 0.5) + model.offset).clamp(0.001, 0.999)
}

fn select_prediction_blend_repair(
    table: &FeatureTable,
    traces: &ScoreTraceRows,
) -> Result<PredictionBlendReport> {
    let mut best: Option<PredictionBlendReport> = None;
    for base_candidate in probability_calibration_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for affine_model in affine_calibration_models_for_candidate(base_candidate) {
            for affine_weight in [0.70_f32, 0.75, 0.80, 0.85, 0.90, 0.95] {
                let model = PredictionBlendModel {
                    selected_on: "calibration".to_string(),
                    affine_model: affine_model.clone(),
                    affine_weight,
                    rank_lift_weight: 1.0 - affine_weight,
                };
                let calibration = prediction_blend_split_report(
                    &calibration_base_rows,
                    &traces.calibration_rank_lift,
                    &model,
                )?;
                let holdout = prediction_blend_split_report(
                    &holdout_base_rows,
                    &traces.holdout_rank_lift,
                    &model,
                )?;
                let selection_objective = selection_objective(&calibration.metrics);
                let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                let report = PredictionBlendReport {
                    selected_on: "calibration".to_string(),
                    selection_forbidden: false,
                    forbidden_reason: None,
                    selection_objective,
                    model,
                    calibration,
                    holdout,
                    blockers,
                };
                if best
                    .as_ref()
                    .map(|previous| report.selection_objective < previous.selection_objective)
                    .unwrap_or(true)
                {
                    best = Some(report);
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("prediction blend sweep produced no candidates"))
}

fn prediction_blend_split_report(
    base_rows: &[PredictionRow],
    rank_lift_rows: &[PredictionRow],
    model: &PredictionBlendModel,
) -> Result<SplitReport> {
    ensure_prediction_rows_aligned(base_rows, rank_lift_rows, "prediction blend rank-lift")?;
    let rows = base_rows
        .iter()
        .zip(rank_lift_rows)
        .map(|(base, rank_lift)| {
            let affine_score = apply_affine_calibration(base, &model.affine_model);
            let score = (model.affine_weight * affine_score
                + model.rank_lift_weight * rank_lift.predicted_oracle_pass)
                .clamp(0.001, 0.999);
            PredictionRow {
                task_id: base.task_id.clone(),
                category: base.category.clone(),
                actual_oracle_pass: base.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base.actual_oracle_pass,
            }
        })
        .collect::<Vec<_>>();
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn select_logit_calibration_repair(table: &FeatureTable) -> Result<LogitCalibrationReport> {
    let mut best: Option<LogitCalibrationReport> = None;
    for base_candidate in probability_calibration_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for model in logit_calibration_models_for_candidate(base_candidate) {
            let calibration = logit_calibration_split_report(&calibration_base_rows, &model)?;
            let holdout = logit_calibration_split_report(&holdout_base_rows, &model)?;
            let selection_objective = selection_objective(&calibration.metrics);
            let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
            let report = LogitCalibrationReport {
                selected_on: "calibration".to_string(),
                selection_forbidden: false,
                forbidden_reason: None,
                selection_objective,
                model,
                calibration,
                holdout,
                blockers,
            };
            if best
                .as_ref()
                .map(|previous| report.selection_objective < previous.selection_objective)
                .unwrap_or(true)
            {
                best = Some(report);
            }
        }
    }
    best.ok_or_else(|| anyhow!("logit calibration sweep produced no candidates"))
}

fn logit_calibration_models_for_candidate(base_candidate: Candidate) -> Vec<LogitCalibrationModel> {
    let mut models = Vec::new();
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for slope in [0.40_f32, 0.55, 0.70, 0.85, 1.0, 1.15, 1.30] {
            for bias in [
                -1.20_f32, -0.80, -0.50, -0.30, -0.10, 0.0, 0.10, 0.30, 0.50, 0.80, 1.20,
            ] {
                models.push(LogitCalibrationModel {
                    selected_on: "calibration".to_string(),
                    scope,
                    base_candidate,
                    slope,
                    bias,
                });
            }
        }
    }
    models
}

fn logit_calibration_split_report(
    rows: &[PredictionRow],
    model: &LogitCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = rows
        .iter()
        .map(|row| {
            let score = apply_logit_calibration(row, model);
            PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            }
        })
        .collect::<Vec<_>>();
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn apply_logit_calibration(row: &PredictionRow, model: &LogitCalibrationModel) -> f32 {
    if !calibration_scope_applies(model.scope, &row.category) {
        return row.predicted_oracle_pass;
    }
    let p = row.predicted_oracle_pass.clamp(0.001, 0.999);
    let logit = (p / (1.0 - p)).ln();
    let shifted = (model.slope * logit + model.bias).clamp(-20.0, 20.0);
    (1.0 / (1.0 + (-shifted).exp())).clamp(0.001, 0.999)
}

fn select_mean_bias_calibration_repair(table: &FeatureTable) -> Result<MeanBiasCalibrationReport> {
    let mut best: Option<MeanBiasCalibrationReport> = None;
    for base_candidate in probability_calibration_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for affine_model in affine_calibration_models_for_candidate(base_candidate) {
            for scope in [
                CalibrationScope::Global,
                CalibrationScope::PerCategory,
                CalibrationScope::TrueMixedPerCategory,
            ] {
                for shrinkage in [0.25_f32, 0.50, 0.75, 1.0] {
                    let model = fit_mean_bias_calibration_model(
                        &calibration_base_rows,
                        affine_model.clone(),
                        scope,
                        shrinkage,
                    )?;
                    let calibration = mean_bias_calibration_cross_split_report(
                        &calibration_base_rows,
                        &affine_model,
                        scope,
                        shrinkage,
                    )?;
                    let holdout = mean_bias_calibration_split_report(&holdout_base_rows, &model)?;
                    let selection_objective = selection_objective(&calibration.metrics);
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = MeanBiasCalibrationReport {
                        selected_on: "calibration".to_string(),
                        selection_forbidden: false,
                        forbidden_reason: None,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        blockers,
                    };
                    if best
                        .as_ref()
                        .map(|previous| report.selection_objective < previous.selection_objective)
                        .unwrap_or(true)
                    {
                        best = Some(report);
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("mean-bias calibration sweep produced no candidates"))
}

fn select_ece_focused_mean_bias_calibration_repair(
    table: &FeatureTable,
) -> Result<MeanBiasCalibrationReport> {
    let mut best: Option<MeanBiasCalibrationReport> = None;
    for base_candidate in ece_focused_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for affine_model in ece_focused_affine_models_for_candidate(base_candidate) {
            for scope in [
                CalibrationScope::Global,
                CalibrationScope::PerCategory,
                CalibrationScope::TrueMixedPerCategory,
            ] {
                for shrinkage in [0.0_f32, 0.05, 0.10, 0.15, 0.25, 0.50, 0.75, 1.0] {
                    let model = fit_mean_bias_calibration_model(
                        &calibration_base_rows,
                        affine_model.clone(),
                        scope,
                        shrinkage,
                    )?;
                    let calibration = mean_bias_calibration_cross_split_report(
                        &calibration_base_rows,
                        &affine_model,
                        scope,
                        shrinkage,
                    )?;
                    let holdout = mean_bias_calibration_split_report(&holdout_base_rows, &model)?;
                    let selection_objective = ece_repair_objective(&calibration.metrics);
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = MeanBiasCalibrationReport {
                        selected_on: "calibration_ece_objective".to_string(),
                        selection_forbidden: false,
                        forbidden_reason: None,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        blockers,
                    };
                    if best
                        .as_ref()
                        .map(|previous| report.selection_objective < previous.selection_objective)
                        .unwrap_or(true)
                    {
                        best = Some(report);
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("ECE-focused mean-bias calibration sweep produced no candidates"))
}

fn ece_focused_base_candidates() -> [Candidate; 2] {
    [
        Candidate::CategoryLocalKnn {
            k: 31,
            fail_boost: 1.5,
        },
        CURRENT_RBF,
    ]
}

fn ece_focused_affine_models_for_candidate(
    base_candidate: Candidate,
) -> Vec<AffineCalibrationModel> {
    let mut models = Vec::new();
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for scale in [1.15_f32, 1.30] {
            for offset in [0.02_f32, 0.04, 0.06] {
                models.push(AffineCalibrationModel {
                    selected_on: "calibration".to_string(),
                    scope,
                    base_candidate,
                    scale,
                    offset,
                });
            }
        }
    }
    models
}

fn fit_mean_bias_calibration_model(
    rows: &[PredictionRow],
    affine_model: AffineCalibrationModel,
    scope: CalibrationScope,
    shrinkage: f32,
) -> Result<MeanBiasCalibrationModel> {
    ensure!(
        (0.0..=1.0).contains(&shrinkage),
        "mean-bias shrinkage must be in [0, 1]"
    );
    let mut stats = BTreeMap::<String, (usize, usize, f32)>::new();
    for row in rows {
        if !calibration_scope_applies(scope, &row.category) {
            continue;
        }
        let key = calibration_scope_key(scope, &row.category);
        let entry = stats.entry(key).or_insert((0, 0, 0.0));
        entry.0 += 1;
        entry.1 += usize::from(row.actual_oracle_pass);
        entry.2 += apply_affine_calibration(row, &affine_model);
    }
    ensure!(
        !stats.is_empty(),
        "mean-bias calibration produced no scoped rows"
    );
    let bias_by_key = stats
        .into_iter()
        .map(|(key, (count, positives, score_sum))| {
            let actual_rate = positives as f32 / count as f32;
            let score_mean = score_sum / count as f32;
            let bias = (shrinkage * (actual_rate - score_mean)).clamp(-0.25, 0.25);
            (key, bias)
        })
        .collect::<BTreeMap<_, _>>();
    Ok(MeanBiasCalibrationModel {
        selected_on: "calibration".to_string(),
        scope,
        affine_model,
        shrinkage,
        bias_by_key,
    })
}

fn mean_bias_calibration_cross_split_report(
    rows: &[PredictionRow],
    affine_model: &AffineCalibrationModel,
    scope: CalibrationScope,
    shrinkage: f32,
) -> Result<SplitReport> {
    ensure!(
        rows.len() >= 2,
        "mean-bias calibration cross split needs at least two rows"
    );
    let mut calibrated_rows = Vec::with_capacity(rows.len());
    for held_out_fold in 0..2_u8 {
        let fit_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) != held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        let eval_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) == held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !fit_rows.is_empty() && !eval_rows.is_empty(),
            "mean-bias calibration cross split produced an empty fold"
        );
        let model =
            fit_mean_bias_calibration_model(&fit_rows, affine_model.clone(), scope, shrinkage)?;
        calibrated_rows.extend(mean_bias_calibrated_rows(&eval_rows, &model)?);
    }
    ensure!(
        calibrated_rows.len() == rows.len(),
        "mean-bias calibration cross split lost rows"
    );
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn mean_bias_calibration_split_report(
    rows: &[PredictionRow],
    model: &MeanBiasCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = mean_bias_calibrated_rows(rows, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn mean_bias_calibrated_rows(
    rows: &[PredictionRow],
    model: &MeanBiasCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = apply_mean_bias_calibration(row, model)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_mean_bias_calibration(
    row: &PredictionRow,
    model: &MeanBiasCalibrationModel,
) -> Result<f32> {
    let affine_score = apply_affine_calibration(row, &model.affine_model);
    if !calibration_scope_applies(model.scope, &row.category) {
        return Ok(affine_score);
    }
    let key = calibration_scope_key(model.scope, &row.category);
    let bias = model
        .bias_by_key
        .get(&key)
        .ok_or_else(|| anyhow!("mean-bias calibration missing key {key}"))?;
    Ok((affine_score + bias).clamp(0.001, 0.999))
}

fn select_cross_window_stability_calibration_repair(
    table: &FeatureTable,
) -> Result<CrossWindowStabilityCalibrationReport> {
    let mut best: Option<CrossWindowStabilityCalibrationReport> = None;
    for base_candidate in ece_focused_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for affine_model in ece_focused_affine_models_for_candidate(base_candidate) {
            for scope in [
                CalibrationScope::PerCategory,
                CalibrationScope::TrueMixedPerCategory,
            ] {
                for shrinkage in [0.25_f32, 0.50, 0.75, 1.0] {
                    for min_window_support in [2_usize, 3, 4] {
                        for instability_scale in [8.0_f32, 16.0, 32.0] {
                            for bias_cap in [0.10_f32, 0.15, 0.25] {
                                let model = fit_cross_window_stability_model(
                                    &calibration_base_rows,
                                    affine_model.clone(),
                                    scope,
                                    shrinkage,
                                    4,
                                    min_window_support,
                                    instability_scale,
                                    bias_cap,
                                )?;
                                let calibration = cross_window_stability_cross_split_report(
                                    &calibration_base_rows,
                                    &affine_model,
                                    scope,
                                    shrinkage,
                                    4,
                                    min_window_support,
                                    instability_scale,
                                    bias_cap,
                                )?;
                                let holdout = cross_window_stability_split_report(
                                    &holdout_base_rows,
                                    &model,
                                )?;
                                let selection_objective = selection_objective(&calibration.metrics)
                                    + cross_window_stability_complexity_penalty(&model);
                                let blockers = repair_blockers(
                                    &holdout.metrics,
                                    &holdout.per_category_metrics,
                                );
                                let report = CrossWindowStabilityCalibrationReport {
                                    selected_on: "calibration_cross_window".to_string(),
                                    selection_forbidden: false,
                                    forbidden_reason: None,
                                    selection_objective,
                                    model,
                                    calibration,
                                    holdout,
                                    blockers,
                                };
                                if best
                                    .as_ref()
                                    .map(|previous| {
                                        report.selection_objective < previous.selection_objective
                                    })
                                    .unwrap_or(true)
                                {
                                    best = Some(report);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("cross-window stability calibration sweep produced no candidates"))
}

#[allow(clippy::too_many_arguments)]
fn fit_cross_window_stability_model(
    rows: &[PredictionRow],
    affine_model: AffineCalibrationModel,
    scope: CalibrationScope,
    shrinkage: f32,
    window_count: usize,
    min_window_support: usize,
    instability_scale: f32,
    bias_cap: f32,
) -> Result<CrossWindowStabilityCalibrationModel> {
    ensure!(
        (0.0..=1.0).contains(&shrinkage),
        "cross-window shrinkage must be in [0, 1]"
    );
    ensure!(window_count >= 2, "cross-window count must be >= 2");
    ensure!(
        min_window_support > 0,
        "cross-window min support must be positive"
    );
    ensure!(
        instability_scale >= 0.0,
        "cross-window instability scale must be non-negative"
    );
    ensure!(bias_cap > 0.0, "cross-window bias cap must be positive");
    let mut full_stats = BTreeMap::<String, (usize, usize, f32)>::new();
    let mut window_stats = BTreeMap::<String, Vec<(usize, usize, f32)>>::new();
    for row in rows {
        if !calibration_scope_applies(scope, &row.category) {
            continue;
        }
        let key = calibration_scope_key(scope, &row.category);
        let affine_score = apply_affine_calibration(row, &affine_model);
        let entry = full_stats.entry(key.clone()).or_insert((0, 0, 0.0));
        entry.0 += 1;
        entry.1 += usize::from(row.actual_oracle_pass);
        entry.2 += affine_score;
        let windows = window_stats
            .entry(key)
            .or_insert_with(|| vec![(0, 0, 0.0); window_count]);
        let window_idx = cross_window_index(row, window_count);
        let window_entry = &mut windows[window_idx];
        window_entry.0 += 1;
        window_entry.1 += usize::from(row.actual_oracle_pass);
        window_entry.2 += affine_score;
    }
    ensure!(
        !full_stats.is_empty(),
        "cross-window stability calibration produced no scoped rows"
    );
    let mut bias_stats = BTreeMap::new();
    for (key, (support, positives, score_sum)) in full_stats {
        let score_mean = score_sum / support as f32;
        let actual_rate = positives as f32 / support as f32;
        let full_bias = (shrinkage * (actual_rate - score_mean)).clamp(-bias_cap, bias_cap);
        let windows = window_stats
            .get(&key)
            .ok_or_else(|| anyhow!("cross-window stats missing key {key}"))?;
        let mut window_biases = Vec::new();
        let mut min_window_support_seen = usize::MAX;
        for (window_support, window_positives, window_score_sum) in windows {
            if *window_support == 0 {
                min_window_support_seen = 0;
                continue;
            }
            min_window_support_seen = min_window_support_seen.min(*window_support);
            if *window_support < min_window_support {
                continue;
            }
            let window_score_mean = *window_score_sum / *window_support as f32;
            let window_actual_rate = *window_positives as f32 / *window_support as f32;
            window_biases.push(
                (shrinkage * (window_actual_rate - window_score_mean)).clamp(-bias_cap, bias_cap),
            );
        }
        let valid_window_count = window_biases.len();
        let window_bias_mean = if window_biases.is_empty() {
            0.0
        } else {
            window_biases.iter().sum::<f32>() / window_biases.len() as f32
        };
        let window_bias_std = if window_biases.len() < 2 {
            bias_cap
        } else {
            let variance = window_biases
                .iter()
                .map(|bias| (bias - window_bias_mean).powi(2))
                .sum::<f32>()
                / window_biases.len() as f32;
            variance.sqrt()
        };
        let coverage_weight = valid_window_count as f32 / window_count as f32;
        let instability_weight = 1.0 / (1.0 + instability_scale * window_bias_std);
        let agreement_weight = if full_bias.abs() <= f32::EPSILON
            || window_bias_mean.abs() <= f32::EPSILON
            || full_bias.signum() == window_bias_mean.signum()
        {
            1.0
        } else {
            0.25
        };
        let stability_weight = if valid_window_count >= 2 {
            (coverage_weight * instability_weight * agreement_weight).clamp(0.0, 1.0)
        } else {
            0.0
        };
        bias_stats.insert(
            key,
            CrossWindowBiasStats {
                support,
                positives,
                full_bias,
                stable_bias: (full_bias * stability_weight).clamp(-bias_cap, bias_cap),
                window_bias_mean,
                window_bias_std,
                valid_window_count,
                min_window_support_seen: if min_window_support_seen == usize::MAX {
                    0
                } else {
                    min_window_support_seen
                },
                stability_weight,
            },
        );
    }
    Ok(CrossWindowStabilityCalibrationModel {
        selected_on: "calibration_cross_window".to_string(),
        scope,
        affine_model,
        shrinkage,
        window_count,
        min_window_support,
        instability_scale,
        bias_cap,
        bias_stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn cross_window_stability_cross_split_report(
    rows: &[PredictionRow],
    affine_model: &AffineCalibrationModel,
    scope: CalibrationScope,
    shrinkage: f32,
    window_count: usize,
    min_window_support: usize,
    instability_scale: f32,
    bias_cap: f32,
) -> Result<SplitReport> {
    ensure!(
        rows.len() >= 2,
        "cross-window stability cross split needs at least two rows"
    );
    let mut calibrated_rows = Vec::with_capacity(rows.len());
    for held_out_fold in 0..2_u8 {
        let fit_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) != held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        let eval_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) == held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !fit_rows.is_empty() && !eval_rows.is_empty(),
            "cross-window stability cross split produced an empty fold"
        );
        let model = fit_cross_window_stability_model(
            &fit_rows,
            affine_model.clone(),
            scope,
            shrinkage,
            window_count,
            min_window_support,
            instability_scale,
            bias_cap,
        )?;
        calibrated_rows.extend(cross_window_stability_calibrated_rows(&eval_rows, &model)?);
    }
    ensure!(
        calibrated_rows.len() == rows.len(),
        "cross-window stability cross split lost rows"
    );
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn cross_window_stability_split_report(
    rows: &[PredictionRow],
    model: &CrossWindowStabilityCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = cross_window_stability_calibrated_rows(rows, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn cross_window_stability_calibrated_rows(
    rows: &[PredictionRow],
    model: &CrossWindowStabilityCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = apply_cross_window_stability_calibration(row, model)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_cross_window_stability_calibration(
    row: &PredictionRow,
    model: &CrossWindowStabilityCalibrationModel,
) -> Result<f32> {
    let affine_score = apply_affine_calibration(row, &model.affine_model);
    if !calibration_scope_applies(model.scope, &row.category) {
        return Ok(affine_score);
    }
    let key = calibration_scope_key(model.scope, &row.category);
    let stats = model
        .bias_stats
        .get(&key)
        .ok_or_else(|| anyhow!("cross-window stability calibration missing key {key}"))?;
    Ok((affine_score + stats.stable_bias).clamp(0.001, 0.999))
}

fn cross_window_index(row: &PredictionRow, window_count: usize) -> usize {
    let digest =
        Sha256::digest(format!("{}|{}|cross_window", row.task_id, row.category).as_bytes());
    (digest[0] as usize) % window_count
}

fn cross_window_stability_complexity_penalty(model: &CrossWindowStabilityCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.004,
        CalibrationScope::Global => 0.012,
    };
    let unstable_key_fraction = if model.bias_stats.is_empty() {
        1.0
    } else {
        model
            .bias_stats
            .values()
            .filter(|stats| stats.valid_window_count < 2 || stats.stability_weight < 0.25)
            .count() as f32
            / model.bias_stats.len() as f32
    };
    scope_penalty + 0.010 * unstable_key_fraction
}

fn select_cause_residual_calibration_repair(
    table: &FeatureTable,
    mean_bias: &MeanBiasCalibrationReport,
) -> Result<CauseResidualCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best: Option<CauseResidualCalibrationReport> = None;
    for scope in [
        CalibrationScope::TrueMixedPerCategory,
        CalibrationScope::PerCategory,
    ] {
        for regularization in [1.0_f32, 10.0, 50.0, 100.0] {
            for correction_scale in [0.25_f32, 0.50, 0.75] {
                for correction_cap in [0.05_f32, 0.10, 0.15] {
                    let model = fit_cause_residual_model(
                        table,
                        &calibration_indices,
                        mean_bias.model.clone(),
                        scope,
                        4,
                        regularization,
                        correction_scale,
                        correction_cap,
                    )?;
                    let calibration = cause_residual_cross_window_report(
                        table,
                        &calibration_indices,
                        &mean_bias.model,
                        scope,
                        4,
                        regularization,
                        correction_scale,
                        correction_cap,
                    )?;
                    let holdout = cause_residual_split_report(table, &holdout_indices, &model)?;
                    let selection_objective = selection_objective(&calibration.metrics)
                        + cause_residual_complexity_penalty(&model);
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = CauseResidualCalibrationReport {
                        selected_on: "calibration_leave_one_window_out".to_string(),
                        selection_forbidden: false,
                        forbidden_reason: None,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        blockers,
                    };
                    if best
                        .as_ref()
                        .map(|previous| report.selection_objective < previous.selection_objective)
                        .unwrap_or(true)
                    {
                        best = Some(report);
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("cause-residual calibration sweep produced no candidates"))
}

#[allow(clippy::too_many_arguments)]
fn fit_cause_residual_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    window_count: usize,
    regularization: f32,
    correction_scale: f32,
    correction_cap: f32,
) -> Result<CauseResidualCalibrationModel> {
    ensure!(
        window_count >= 2,
        "cause residual window count must be >= 2"
    );
    ensure!(
        regularization > 0.0,
        "cause residual regularization must be positive"
    );
    ensure!(
        (0.0..=1.0).contains(&correction_scale),
        "cause residual correction scale must be in [0, 1]"
    );
    ensure!(
        correction_cap > 0.0,
        "cause residual correction cap must be positive"
    );
    let base_rows = mean_bias_rows_for_indices(table, indices, &base_model)?;
    let feature_count = indices
        .iter()
        .map(|idx| table.rows[*idx].cause_features.len())
        .max()
        .unwrap_or_default();
    ensure!(
        feature_count > 0,
        "cause residual feature vectors must be non-empty"
    );
    let mut raw_samples = Vec::<(String, Vec<f32>, f32)>::new();
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        if !calibration_scope_applies(scope, &base_row.category) {
            continue;
        }
        let feature_row = &table.rows[*idx];
        ensure!(
            feature_row.cause_features.len() == feature_count,
            "cause residual feature dimension drift for {}:{}",
            feature_row.task_id,
            feature_row.category
        );
        let key = calibration_scope_key(scope, &base_row.category);
        let residual = if base_row.actual_oracle_pass {
            1.0
        } else {
            0.0
        } - base_row.predicted_oracle_pass;
        raw_samples.push((key, feature_row.cause_features.clone(), residual));
    }
    ensure!(
        !raw_samples.is_empty(),
        "cause residual fit produced no scoped samples"
    );
    let (feature_mean, feature_std) = cause_feature_standardization(&raw_samples, feature_count)?;
    let mut samples_by_key = BTreeMap::<String, Vec<(Vec<f32>, f32)>>::new();
    for (key, features, residual) in raw_samples {
        samples_by_key.entry(key).or_default().push((
            standardize_cause_features(&features, &feature_mean, &feature_std)?,
            residual,
        ));
    }
    let mut weights_by_key = BTreeMap::new();
    for (key, samples) in samples_by_key {
        if samples.len() < 4 {
            continue;
        }
        weights_by_key.insert(key, ridge_residual_weights(&samples, regularization)?);
    }
    ensure!(
        !weights_by_key.is_empty(),
        "cause residual fit produced no trained residual weights"
    );
    Ok(CauseResidualCalibrationModel {
        selected_on: "calibration_leave_one_window_out".to_string(),
        base_model,
        scope,
        window_count,
        regularization,
        correction_scale,
        correction_cap,
        feature_count,
        feature_mean,
        feature_std,
        weights_by_key,
    })
}

fn cause_feature_standardization(
    samples: &[(String, Vec<f32>, f32)],
    feature_count: usize,
) -> Result<(Vec<f32>, Vec<f32>)> {
    ensure!(
        !samples.is_empty(),
        "cause residual standardization needs samples"
    );
    let mut mean = vec![0.0_f64; feature_count];
    for (_key, features, _residual) in samples {
        ensure!(
            features.len() == feature_count,
            "cause residual standardization feature dimension mismatch"
        );
        for (idx, value) in features.iter().enumerate() {
            mean[idx] += *value as f64;
        }
    }
    let sample_count = samples.len() as f64;
    for value in &mut mean {
        *value /= sample_count;
    }
    let mut variance = vec![0.0_f64; feature_count];
    for (_key, features, _residual) in samples {
        for (idx, value) in features.iter().enumerate() {
            let delta = *value as f64 - mean[idx];
            variance[idx] += delta * delta;
        }
    }
    let std = variance
        .into_iter()
        .map(|value| {
            let std = (value / sample_count).sqrt();
            if std.is_finite() && std > 1.0e-6 {
                std as f32
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    Ok((mean.into_iter().map(|value| value as f32).collect(), std))
}

fn standardize_cause_features(features: &[f32], mean: &[f32], std: &[f32]) -> Result<Vec<f32>> {
    ensure!(
        features.len() == mean.len() && features.len() == std.len(),
        "cause residual standardization dimension mismatch"
    );
    Ok(features
        .iter()
        .zip(mean)
        .zip(std)
        .map(|((feature, mean), std)| (*feature - *mean) / (*std).max(1.0e-6))
        .collect())
}

fn ridge_residual_weights(samples: &[(Vec<f32>, f32)], regularization: f32) -> Result<Vec<f32>> {
    let feature_count = samples
        .first()
        .map(|(features, _residual)| features.len())
        .unwrap_or_default();
    ensure!(feature_count > 0, "ridge residual features are empty");
    let dim = feature_count + 1;
    let mut lhs = vec![vec![0.0_f64; dim]; dim];
    let mut rhs = vec![0.0_f64; dim];
    for (features, residual) in samples {
        ensure!(
            features.len() == feature_count,
            "ridge residual feature dimension mismatch"
        );
        let mut row = Vec::with_capacity(dim);
        row.push(1.0_f64);
        row.extend(features.iter().map(|value| *value as f64));
        for i in 0..dim {
            rhs[i] += row[i] * *residual as f64;
            for j in 0..dim {
                lhs[i][j] += row[i] * row[j];
            }
        }
    }
    for (idx, row) in lhs.iter_mut().enumerate().skip(1) {
        row[idx] += regularization as f64;
    }
    lhs[0][0] += 1.0e-6;
    solve_linear_system(lhs, rhs).map(|weights| weights.into_iter().map(|v| v as f32).collect())
}

fn solve_linear_system(mut lhs: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Result<Vec<f64>> {
    let n = rhs.len();
    ensure!(lhs.len() == n, "linear system row count mismatch");
    for pivot_idx in 0..n {
        let mut best_row = pivot_idx;
        let mut best_abs = lhs[pivot_idx][pivot_idx].abs();
        for (row_idx, row) in lhs.iter().enumerate().skip(pivot_idx + 1) {
            let value = row[pivot_idx].abs();
            if value > best_abs {
                best_row = row_idx;
                best_abs = value;
            }
        }
        ensure!(
            best_abs > 1.0e-10,
            "linear system is singular at pivot {pivot_idx}"
        );
        if best_row != pivot_idx {
            lhs.swap(best_row, pivot_idx);
            rhs.swap(best_row, pivot_idx);
        }
        let pivot = lhs[pivot_idx][pivot_idx];
        for value in lhs[pivot_idx].iter_mut().skip(pivot_idx) {
            *value /= pivot;
        }
        rhs[pivot_idx] /= pivot;
        let pivot_tail = lhs[pivot_idx][pivot_idx..].to_vec();
        for row_idx in 0..n {
            if row_idx == pivot_idx {
                continue;
            }
            let factor = lhs[row_idx][pivot_idx];
            if factor.abs() <= 1.0e-12 {
                continue;
            }
            for (value, pivot_value) in lhs[row_idx].iter_mut().skip(pivot_idx).zip(&pivot_tail) {
                *value -= factor * pivot_value;
            }
            rhs[row_idx] -= factor * rhs[pivot_idx];
        }
    }
    ensure!(
        rhs.iter().all(|value| value.is_finite()),
        "linear system produced non-finite weights"
    );
    Ok(rhs)
}

#[allow(clippy::too_many_arguments)]
fn cause_residual_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    base_model: &MeanBiasCalibrationModel,
    scope: CalibrationScope,
    window_count: usize,
    regularization: f32,
    correction_scale: f32,
    correction_cap: f32,
) -> Result<SplitReport> {
    ensure!(
        indices.len() >= window_count,
        "cause residual cross-window split needs at least one row per window"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_window in 0..window_count {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| cause_window_index(&table.rows[*idx], window_count) != held_out_window)
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| cause_window_index(&table.rows[*idx], window_count) == held_out_window)
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "cause residual cross-window split produced an empty window"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            base_model.affine_model.base_candidate,
        )?;
        let fold_base_model = fit_mean_bias_calibration_model(
            &fit_base_rows,
            base_model.affine_model.clone(),
            base_model.scope,
            base_model.shrinkage,
        )?;
        let model = fit_cause_residual_model(
            table,
            &fit_indices,
            fold_base_model,
            scope,
            window_count,
            regularization,
            correction_scale,
            correction_cap,
        )?;
        for (idx, row) in eval_indices
            .iter()
            .copied()
            .zip(cause_residual_calibrated_rows(
                table,
                &eval_indices,
                &model,
            )?)
        {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "cause residual cross-window produced duplicate row index {idx}"
            );
        }
    }
    let calibrated_rows = indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("cause residual cross-window missing row index {idx}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn cause_residual_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseResidualCalibrationModel,
) -> Result<SplitReport> {
    let rows = cause_residual_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn cause_residual_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseResidualCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = mean_bias_rows_for_indices(table, indices, &model.base_model)?;
    indices
        .iter()
        .zip(base_rows.iter())
        .map(|(idx, base_row)| {
            let score = apply_cause_residual_calibration(base_row, &table.rows[*idx], model)?;
            Ok(PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_cause_residual_calibration(
    base_row: &PredictionRow,
    feature_row: &FeatureRow,
    model: &CauseResidualCalibrationModel,
) -> Result<f32> {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return Ok(base_score);
    }
    ensure!(
        feature_row.cause_features.len() == model.feature_count,
        "cause residual feature count mismatch for {}:{}",
        feature_row.task_id,
        feature_row.category
    );
    let key = calibration_scope_key(model.scope, &base_row.category);
    let Some(weights) = model.weights_by_key.get(&key) else {
        return Ok(base_score);
    };
    ensure!(
        weights.len() == model.feature_count + 1,
        "cause residual weight count mismatch for key {key}"
    );
    let standardized = standardize_cause_features(
        &feature_row.cause_features,
        &model.feature_mean,
        &model.feature_std,
    )?;
    let correction = weights
        .iter()
        .skip(1)
        .zip(&standardized)
        .fold(weights[0], |sum, (weight, feature)| sum + weight * feature)
        .clamp(-model.correction_cap, model.correction_cap);
    Ok((base_score + model.correction_scale * correction).clamp(0.001, 0.999))
}

fn cause_window_index(row: &FeatureRow, window_count: usize) -> usize {
    let digest =
        Sha256::digest(format!("{}|{}|cause_window", row.task_id, row.category).as_bytes());
    (digest[0] as usize) % window_count
}

fn cause_residual_complexity_penalty(model: &CauseResidualCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.006,
        CalibrationScope::Global => 0.014,
    };
    let feature_penalty = (model.feature_count as f32 / 10_000.0).min(0.010);
    let key_penalty = if model.weights_by_key.len() > TRUE_MIXED_CATEGORIES.len() {
        0.006
    } else {
        0.0
    };
    scope_penalty + feature_penalty + key_penalty
}

fn select_cause_bin_calibration_repair(
    table: &FeatureTable,
    cause_residual: &CauseResidualCalibrationReport,
) -> Result<CauseBinCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best: Option<CauseBinCalibrationReport> = None;
    for scope in [CalibrationScope::TrueMixedPerCategory] {
        for bin_count in [10_usize] {
            for min_support in [2_usize, 4] {
                for smoothing in [1.0_f32, 3.0, 8.0] {
                    for prior_weight in [0.25_f32, 0.50, 0.75] {
                        let model = fit_cause_bin_calibration_model(
                            table,
                            &calibration_indices,
                            cause_residual.model.clone(),
                            scope,
                            bin_count,
                            min_support,
                            smoothing,
                            prior_weight,
                        )?;
                        let calibration = cause_bin_cross_window_report(
                            table,
                            &calibration_indices,
                            &cause_residual.model,
                            scope,
                            bin_count,
                            min_support,
                            smoothing,
                            prior_weight,
                        )?;
                        let holdout = cause_bin_split_report(table, &holdout_indices, &model)?;
                        let calibration_application =
                            cause_bin_application_stats(table, &calibration_indices, &model)?;
                        let holdout_application =
                            cause_bin_application_stats(table, &holdout_indices, &model)?;
                        let selection_objective = selection_objective(&calibration.metrics)
                            + cause_bin_complexity_penalty(&model);
                        let blockers =
                            repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                        let report = CauseBinCalibrationReport {
                            selected_on: "calibration_leave_one_window_out".to_string(),
                            selection_forbidden: false,
                            forbidden_reason: None,
                            selection_objective,
                            model,
                            calibration,
                            holdout,
                            calibration_application,
                            holdout_application,
                            blockers,
                        };
                        if best
                            .as_ref()
                            .map(|previous| {
                                report.selection_objective < previous.selection_objective
                            })
                            .unwrap_or(true)
                        {
                            best = Some(report);
                        }
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("cause-bin calibration sweep produced no candidates"))
}

#[allow(clippy::too_many_arguments)]
fn fit_cause_bin_calibration_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: CauseResidualCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<CauseBinCalibrationModel> {
    ensure!(bin_count > 1, "cause-bin bin_count must be > 1");
    ensure!(min_support > 0, "cause-bin min_support must be positive");
    ensure!(smoothing >= 0.0, "cause-bin smoothing must be non-negative");
    ensure!(
        (0.0..=1.0).contains(&prior_weight),
        "cause-bin prior_weight must be in [0, 1]"
    );
    let base_rows = cause_residual_calibrated_rows(table, indices, &base_model)?;
    let global_prior = pass_rate(&base_rows);
    let mut stats = BTreeMap::<String, (usize, usize, f32)>::new();
    for base_row in &base_rows {
        if !calibration_scope_applies(scope, &base_row.category) {
            continue;
        }
        let scope_key = calibration_scope_key(scope, &base_row.category);
        let bin_idx = probability_bin_index(base_row.predicted_oracle_pass, bin_count);
        let entry = stats
            .entry(cause_bin_key(&scope_key, bin_idx))
            .or_insert((0, 0, 0.0));
        entry.0 += 1;
        entry.1 += usize::from(base_row.actual_oracle_pass);
        entry.2 += base_row.predicted_oracle_pass;
    }
    ensure!(
        !stats.is_empty(),
        "cause-bin calibration produced no scoped score-bin stats"
    );
    let key_stats = stats
        .into_iter()
        .map(|(key, (support, positives, score_sum))| {
            let prior =
                (positives as f32 + smoothing * global_prior) / (support as f32 + smoothing);
            (
                key,
                CauseBinKeyStats {
                    support,
                    positives,
                    prior: prior.clamp(0.001, 0.999),
                    score_mean: score_sum / support as f32,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(CauseBinCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        scope,
        bin_count,
        min_support,
        smoothing,
        prior_weight,
        key_stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn cause_bin_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    cause_model: &CauseResidualCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SplitReport> {
    let rows = cause_bin_cross_window_rows(
        table,
        indices,
        cause_model,
        scope,
        bin_count,
        min_support,
        smoothing,
        prior_weight,
    )?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

#[allow(clippy::too_many_arguments)]
fn cause_bin_cross_window_rows(
    table: &FeatureTable,
    indices: &[usize],
    cause_model: &CauseResidualCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<Vec<PredictionRow>> {
    ensure!(
        indices.len() >= cause_model.window_count,
        "cause-bin cross-window split needs at least one row per window"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_window in 0..cause_model.window_count {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_model.window_count) != held_out_window
            })
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_model.window_count) == held_out_window
            })
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "cause-bin cross-window split produced an empty window"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            cause_model.base_model.affine_model.base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            cause_model.base_model.affine_model.clone(),
            cause_model.base_model.scope,
            cause_model.base_model.shrinkage,
        )?;
        let fold_cause = fit_cause_residual_model(
            table,
            &fit_indices,
            fold_mean_bias,
            cause_model.scope,
            cause_model.window_count,
            cause_model.regularization,
            cause_model.correction_scale,
            cause_model.correction_cap,
        )?;
        let model = fit_cause_bin_calibration_model(
            table,
            &fit_indices,
            fold_cause,
            scope,
            bin_count,
            min_support,
            smoothing,
            prior_weight,
        )?;
        for (idx, row) in eval_indices.iter().copied().zip(cause_bin_calibrated_rows(
            table,
            &eval_indices,
            &model,
        )?) {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "cause-bin cross-window produced duplicate row index {idx}"
            );
        }
    }
    indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("cause-bin cross-window missing row index {idx}"))
        })
        .collect()
}

fn cause_bin_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBinCalibrationModel,
) -> Result<SplitReport> {
    let rows = cause_bin_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn cause_bin_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBinCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = cause_residual_calibrated_rows(table, indices, &model.base_model)?;
    base_rows
        .iter()
        .map(|base_row| {
            let score = apply_cause_bin_calibration(base_row, model);
            Ok(PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            })
        })
        .collect()
}

fn cause_bin_application_stats(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBinCalibrationModel,
) -> Result<CauseBinApplicationStats> {
    let base_rows = cause_residual_calibrated_rows(table, indices, &model.base_model)?;
    let mut stats = CauseBinApplicationStats {
        scoped_row_count: 0,
        applied_row_count: 0,
        missing_key_row_count: 0,
        under_min_support_row_count: 0,
        non_scoped_row_count: 0,
    };
    for base_row in &base_rows {
        if !calibration_scope_applies(model.scope, &base_row.category) {
            stats.non_scoped_row_count += 1;
            continue;
        }
        stats.scoped_row_count += 1;
        let scope_key = calibration_scope_key(model.scope, &base_row.category);
        let bin_idx = probability_bin_index(base_row.predicted_oracle_pass, model.bin_count);
        let Some(key_stats) = model.key_stats.get(&cause_bin_key(&scope_key, bin_idx)) else {
            stats.missing_key_row_count += 1;
            continue;
        };
        if key_stats.support < model.min_support {
            stats.under_min_support_row_count += 1;
            continue;
        }
        stats.applied_row_count += 1;
    }
    Ok(stats)
}

fn apply_cause_bin_calibration(base_row: &PredictionRow, model: &CauseBinCalibrationModel) -> f32 {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return base_score;
    }
    let scope_key = calibration_scope_key(model.scope, &base_row.category);
    let bin_idx = probability_bin_index(base_score, model.bin_count);
    let Some(stats) = model.key_stats.get(&cause_bin_key(&scope_key, bin_idx)) else {
        return base_score;
    };
    if stats.support < model.min_support {
        return base_score;
    }
    let support_weight = if model.smoothing <= f32::EPSILON {
        1.0
    } else {
        stats.support as f32 / (stats.support as f32 + model.smoothing)
    };
    let effective_weight = (model.prior_weight * support_weight).clamp(0.0, 1.0);
    ((1.0 - effective_weight) * base_score + effective_weight * stats.prior).clamp(0.001, 0.999)
}

fn cause_bin_key(scope_key: &str, bin_idx: usize) -> String {
    format!("scope:{}|bin:{}", key_component(scope_key), bin_idx)
}

fn cause_bin_complexity_penalty(model: &CauseBinCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.006,
        CalibrationScope::Global => 0.014,
    };
    let bin_penalty = if model.bin_count > 10 { 0.004 } else { 0.0 };
    let support_penalty = if model.min_support < 4 { 0.006 } else { 0.0 };
    let weight_penalty = if model.prior_weight > 0.75 {
        0.006
    } else {
        0.0
    };
    let sparse_penalty = if model.key_stats.is_empty() {
        0.020
    } else {
        0.0
    };
    scope_penalty + bin_penalty + support_penalty + weight_penalty + sparse_penalty
}

fn select_score_sharpness_calibration_repair(
    table: &FeatureTable,
    cause_bin: &CauseBinCalibrationReport,
) -> Result<ScoreSharpnessCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best_allowed: Option<ScoreSharpnessCalibrationReport> = None;
    let mut best_forbidden: Option<ScoreSharpnessCalibrationReport> = None;
    for scope in [CalibrationScope::TrueMixedPerCategory] {
        for low_cutoff in [0.20_f32, 0.30, 0.40, 0.50] {
            for low_tail_scale in [1.25_f32, 1.50, 2.0, 3.0] {
                for high_cutoff in [0.80_f32, 0.90] {
                    for high_tail_scale in [0.50_f32, 0.75, 1.0] {
                        let model = ScoreSharpnessCalibrationModel {
                            selected_on: "calibration".to_string(),
                            base_model: cause_bin.model.clone(),
                            scope,
                            low_cutoff,
                            low_tail_scale,
                            high_cutoff,
                            high_tail_scale,
                        };
                        let calibration = score_sharpness_cross_window_report(
                            table,
                            &calibration_indices,
                            &cause_bin.model,
                            scope,
                            low_cutoff,
                            low_tail_scale,
                            high_cutoff,
                            high_tail_scale,
                        )?;
                        let holdout =
                            score_sharpness_split_report(table, &holdout_indices, &model)?;
                        let calibration_application =
                            score_sharpness_application_stats(table, &calibration_indices, &model)?;
                        let holdout_application =
                            score_sharpness_application_stats(table, &holdout_indices, &model)?;
                        let selection_objective = selection_objective(&calibration.metrics)
                            + score_sharpness_complexity_penalty(&model);
                        let forbidden_reason = score_sharpness_forbidden_reason(
                            &calibration,
                            &calibration_application,
                            cause_bin,
                        );
                        let blockers =
                            repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                        let report = ScoreSharpnessCalibrationReport {
                            selected_on: "calibration_leave_one_window_out".to_string(),
                            selection_forbidden: forbidden_reason.is_some(),
                            forbidden_reason,
                            selection_objective,
                            model,
                            calibration,
                            holdout,
                            calibration_application,
                            holdout_application,
                            blockers,
                        };
                        let target = if report.selection_forbidden {
                            &mut best_forbidden
                        } else {
                            &mut best_allowed
                        };
                        if target
                            .as_ref()
                            .map(|previous| {
                                report.selection_objective < previous.selection_objective
                            })
                            .unwrap_or(true)
                        {
                            *target = Some(report);
                        }
                    }
                }
            }
        }
    }
    best_allowed
        .or(best_forbidden)
        .ok_or_else(|| anyhow!("score-sharpness calibration sweep produced no candidates"))
}

#[allow(clippy::too_many_arguments)]
fn score_sharpness_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    cause_bin_model: &CauseBinCalibrationModel,
    scope: CalibrationScope,
    low_cutoff: f32,
    low_tail_scale: f32,
    high_cutoff: f32,
    high_tail_scale: f32,
) -> Result<SplitReport> {
    let rows = score_sharpness_cross_window_rows(
        table,
        indices,
        cause_bin_model,
        scope,
        low_cutoff,
        low_tail_scale,
        high_cutoff,
        high_tail_scale,
    )?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

#[allow(clippy::too_many_arguments)]
fn score_sharpness_cross_window_rows(
    table: &FeatureTable,
    indices: &[usize],
    cause_bin_model: &CauseBinCalibrationModel,
    scope: CalibrationScope,
    low_cutoff: f32,
    low_tail_scale: f32,
    high_cutoff: f32,
    high_tail_scale: f32,
) -> Result<Vec<PredictionRow>> {
    ensure!(
        indices.len() >= cause_bin_model.base_model.window_count,
        "score-sharpness cross-window split needs at least one row per window"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_window in 0..cause_bin_model.base_model.window_count {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_bin_model.base_model.window_count)
                    != held_out_window
            })
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_bin_model.base_model.window_count)
                    == held_out_window
            })
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "score-sharpness cross-window split produced an empty window"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            cause_bin_model
                .base_model
                .base_model
                .affine_model
                .base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            cause_bin_model.base_model.base_model.affine_model.clone(),
            cause_bin_model.base_model.base_model.scope,
            cause_bin_model.base_model.base_model.shrinkage,
        )?;
        let fold_cause = fit_cause_residual_model(
            table,
            &fit_indices,
            fold_mean_bias,
            cause_bin_model.base_model.scope,
            cause_bin_model.base_model.window_count,
            cause_bin_model.base_model.regularization,
            cause_bin_model.base_model.correction_scale,
            cause_bin_model.base_model.correction_cap,
        )?;
        let fold_bin = fit_cause_bin_calibration_model(
            table,
            &fit_indices,
            fold_cause,
            cause_bin_model.scope,
            cause_bin_model.bin_count,
            cause_bin_model.min_support,
            cause_bin_model.smoothing,
            cause_bin_model.prior_weight,
        )?;
        let model = ScoreSharpnessCalibrationModel {
            selected_on: "calibration".to_string(),
            base_model: fold_bin,
            scope,
            low_cutoff,
            low_tail_scale,
            high_cutoff,
            high_tail_scale,
        };
        for (idx, row) in eval_indices
            .iter()
            .copied()
            .zip(score_sharpness_calibrated_rows(
                table,
                &eval_indices,
                &model,
            )?)
        {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "score-sharpness cross-window produced duplicate row index {idx}"
            );
        }
    }
    indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("score-sharpness cross-window missing row index {idx}"))
        })
        .collect()
}

fn score_sharpness_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessCalibrationModel,
) -> Result<SplitReport> {
    let rows = score_sharpness_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn score_sharpness_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &model.base_model)?;
    Ok(base_rows
        .iter()
        .map(|base_row| {
            let score = apply_score_sharpness_calibration(base_row, model);
            PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            }
        })
        .collect())
}

fn score_sharpness_application_stats(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessCalibrationModel,
) -> Result<ScoreSharpnessApplicationStats> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &model.base_model)?;
    let mut stats = ScoreSharpnessApplicationStats {
        scoped_row_count: 0,
        low_tail_shaped_row_count: 0,
        high_tail_shaped_row_count: 0,
        unchanged_scoped_row_count: 0,
        non_scoped_row_count: 0,
    };
    for row in &base_rows {
        if !calibration_scope_applies(model.scope, &row.category) {
            stats.non_scoped_row_count += 1;
            continue;
        }
        stats.scoped_row_count += 1;
        let shaped = apply_score_sharpness_calibration(row, model);
        if (shaped - row.predicted_oracle_pass).abs() < 1.0e-6 {
            stats.unchanged_scoped_row_count += 1;
        } else if row.predicted_oracle_pass < model.low_cutoff {
            stats.low_tail_shaped_row_count += 1;
        } else if row.predicted_oracle_pass > model.high_cutoff {
            stats.high_tail_shaped_row_count += 1;
        } else {
            stats.unchanged_scoped_row_count += 1;
        }
    }
    Ok(stats)
}

fn apply_score_sharpness_calibration(
    base_row: &PredictionRow,
    model: &ScoreSharpnessCalibrationModel,
) -> f32 {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return base_score;
    }
    let shaped = if base_score < model.low_cutoff {
        model.low_cutoff - (model.low_cutoff - base_score) * model.low_tail_scale
    } else if base_score > model.high_cutoff {
        model.high_cutoff + (base_score - model.high_cutoff) * model.high_tail_scale
    } else {
        base_score
    };
    shaped.clamp(0.001, 0.999)
}

fn score_sharpness_forbidden_reason(
    calibration: &SplitReport,
    application: &ScoreSharpnessApplicationStats,
    cause_bin: &CauseBinCalibrationReport,
) -> Option<String> {
    let shaped_rows =
        application.low_tail_shaped_row_count + application.high_tail_shaped_row_count;
    if application.scoped_row_count == 0 || shaped_rows == 0 {
        return Some("score_sharpness_no_scoped_rows_shaped".to_string());
    }
    if calibration.metrics.ece > cause_bin.calibration.metrics.ece + 1.0e-6 {
        return Some(format!(
            "calibration_ece_regressed: score_sharpness={:.9} cause_bin={:.9}",
            calibration.metrics.ece, cause_bin.calibration.metrics.ece
        ));
    }
    if calibration.metrics.brier > cause_bin.calibration.metrics.brier + 0.001 {
        return Some(format!(
            "calibration_brier_regressed: score_sharpness={:.9} cause_bin={:.9}",
            calibration.metrics.brier, cause_bin.calibration.metrics.brier
        ));
    }
    if let (Some(sharp_corr), Some(cause_corr)) = (
        calibration.metrics.prediction_oracle_correlation,
        cause_bin.calibration.metrics.prediction_oracle_correlation,
    ) {
        if sharp_corr + 0.005 < cause_corr {
            return Some(format!(
                "calibration_correlation_regressed: score_sharpness={sharp_corr:.9} cause_bin={cause_corr:.9}"
            ));
        }
    }
    None
}

fn score_sharpness_complexity_penalty(model: &ScoreSharpnessCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.006,
        CalibrationScope::Global => 0.014,
    };
    let low_penalty = if model.low_tail_scale > 2.0 {
        0.003
    } else {
        0.0
    };
    let high_penalty = if (model.high_tail_scale - 1.0).abs() > f32::EPSILON {
        0.002
    } else {
        0.0
    };
    scope_penalty + low_penalty + high_penalty
}

fn select_score_sharpness_logit_calibration_repair(
    table: &FeatureTable,
    score_sharpness: &ScoreSharpnessCalibrationReport,
) -> Result<ScoreSharpnessLogitCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best_allowed: Option<ScoreSharpnessLogitCalibrationReport> = None;
    let mut best_forbidden: Option<ScoreSharpnessLogitCalibrationReport> = None;
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for slope in [0.70_f32, 0.80, 0.90, 1.0, 1.10, 1.20] {
            for bias in [-0.20_f32, -0.10, -0.05, 0.0, 0.05, 0.10, 0.20] {
                let logit_model = LogitCalibrationModel {
                    selected_on: "calibration".to_string(),
                    scope,
                    base_candidate: CURRENT_RBF,
                    slope,
                    bias,
                };
                let model = ScoreSharpnessLogitCalibrationModel {
                    selected_on: "calibration".to_string(),
                    base_model: score_sharpness.model.clone(),
                    logit_model: logit_model.clone(),
                };
                let calibration = score_sharpness_logit_cross_window_report(
                    table,
                    &calibration_indices,
                    &score_sharpness.model,
                    logit_model.clone(),
                )?;
                let holdout = score_sharpness_logit_split_report(table, &holdout_indices, &model)?;
                let unpenalized_selection_objective = selection_objective(&calibration.metrics);
                let selection_objective = unpenalized_selection_objective
                    + score_sharpness_logit_complexity_penalty(&model);
                let forbidden_reason = score_sharpness_logit_forbidden_reason(
                    unpenalized_selection_objective,
                    &calibration.metrics,
                    score_sharpness,
                    &logit_model,
                );
                let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                let report = ScoreSharpnessLogitCalibrationReport {
                    selected_on: "calibration_leave_one_window_out".to_string(),
                    selection_forbidden: forbidden_reason.is_some(),
                    forbidden_reason,
                    selection_objective,
                    model,
                    calibration,
                    holdout,
                    blockers,
                };
                let target = if report.selection_forbidden {
                    &mut best_forbidden
                } else {
                    &mut best_allowed
                };
                if target
                    .as_ref()
                    .map(|previous| report.selection_objective < previous.selection_objective)
                    .unwrap_or(true)
                {
                    *target = Some(report);
                }
            }
        }
    }
    best_allowed
        .or(best_forbidden)
        .ok_or_else(|| anyhow!("score-sharpness logit calibration sweep produced no candidates"))
}

fn score_sharpness_logit_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    score_sharpness_model: &ScoreSharpnessCalibrationModel,
    logit_model: LogitCalibrationModel,
) -> Result<SplitReport> {
    let rows = score_sharpness_cross_window_rows(
        table,
        indices,
        &score_sharpness_model.base_model,
        score_sharpness_model.scope,
        score_sharpness_model.low_cutoff,
        score_sharpness_model.low_tail_scale,
        score_sharpness_model.high_cutoff,
        score_sharpness_model.high_tail_scale,
    )?;
    logit_calibration_split_report(&rows, &logit_model)
}

fn score_sharpness_logit_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessLogitCalibrationModel,
) -> Result<SplitReport> {
    let rows = score_sharpness_logit_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn score_sharpness_logit_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessLogitCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let score_sharpness_rows = score_sharpness_calibrated_rows(table, indices, &model.base_model)?;
    Ok(score_sharpness_rows
        .iter()
        .map(|row| {
            let score = apply_logit_calibration(row, &model.logit_model);
            PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            }
        })
        .collect())
}

fn score_sharpness_logit_forbidden_reason(
    unpenalized_selection_objective: f32,
    calibration: &MetricBlock,
    score_sharpness: &ScoreSharpnessCalibrationReport,
    logit_model: &LogitCalibrationModel,
) -> Option<String> {
    if logit_model_is_identity(logit_model) {
        return Some("identity_logit_noop".to_string());
    }
    let base_unpenalized_objective = selection_objective(&score_sharpness.calibration.metrics);
    if unpenalized_selection_objective + 1.0e-6 >= base_unpenalized_objective {
        return Some(format!(
            "unpenalized_objective_not_improved: score_sharpness_logit={unpenalized_selection_objective:.9} score_sharpness={base_unpenalized_objective:.9}"
        ));
    }
    if calibration.ece > score_sharpness.calibration.metrics.ece + 1.0e-6 {
        return Some(format!(
            "calibration_ece_regressed: score_sharpness_logit={:.9} score_sharpness={:.9}",
            calibration.ece, score_sharpness.calibration.metrics.ece
        ));
    }
    if calibration.brier > score_sharpness.calibration.metrics.brier + 0.002 {
        return Some(format!(
            "calibration_brier_regressed: score_sharpness_logit={:.9} score_sharpness={:.9}",
            calibration.brier, score_sharpness.calibration.metrics.brier
        ));
    }
    None
}

fn logit_model_is_identity(model: &LogitCalibrationModel) -> bool {
    (model.slope - 1.0).abs() <= f32::EPSILON && model.bias.abs() <= f32::EPSILON
}

fn score_sharpness_logit_complexity_penalty(model: &ScoreSharpnessLogitCalibrationModel) -> f32 {
    let scope_penalty = match model.logit_model.scope {
        CalibrationScope::Global => 0.002,
        CalibrationScope::TrueMixedPerCategory => 0.004,
        CalibrationScope::PerCategory => 0.008,
    };
    let non_identity_penalty = if (model.logit_model.slope - 1.0).abs() > f32::EPSILON
        || model.logit_model.bias.abs() > f32::EPSILON
    {
        0.001
    } else {
        0.0
    };
    scope_penalty + non_identity_penalty
}

fn select_score_sharpness_abstain_band_repair(
    table: &FeatureTable,
    score_sharpness: &ScoreSharpnessCalibrationReport,
) -> Result<ScoreSharpnessAbstainBandReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best_allowed: Option<ScoreSharpnessAbstainBandReport> = None;
    let mut best_forbidden: Option<ScoreSharpnessAbstainBandReport> = None;
    for (abstain_low, abstain_high) in [
        (0.10_f32, 0.90_f32),
        (0.20, 0.80),
        (0.25, 0.75),
        (0.30, 0.70),
        (0.35, 0.65),
        (0.40, 0.60),
        (0.45, 0.55),
    ] {
        let model = ScoreSharpnessAbstainBandModel {
            selected_on: "calibration".to_string(),
            base_model: score_sharpness.model.clone(),
            abstain_low,
            abstain_high,
            min_active_coverage: 0.85,
        };
        let (calibration, calibration_application) =
            score_sharpness_abstain_band_cross_window_report(
                table,
                &calibration_indices,
                &score_sharpness.model,
                abstain_low,
                abstain_high,
            )?;
        let (holdout, holdout_application) =
            score_sharpness_abstain_band_split_report(table, &holdout_indices, &model)?;
        let selection_objective = selection_objective(&calibration.metrics)
            + abstain_band_complexity_penalty(&calibration_application, &model);
        let forbidden_reason =
            abstain_band_forbidden_reason(selection_objective, &calibration_application, &model);
        let blockers = abstain_band_repair_blockers(&holdout, &holdout_application, &model);
        let report = ScoreSharpnessAbstainBandReport {
            selected_on: "calibration_leave_one_window_out".to_string(),
            selection_forbidden: forbidden_reason.is_some(),
            forbidden_reason,
            selection_objective,
            model,
            calibration,
            holdout,
            calibration_application,
            holdout_application,
            blockers,
        };
        let target = if report.selection_forbidden {
            &mut best_forbidden
        } else {
            &mut best_allowed
        };
        if target
            .as_ref()
            .map(|previous| report.selection_objective < previous.selection_objective)
            .unwrap_or(true)
        {
            *target = Some(report);
        }
    }
    best_allowed
        .or(best_forbidden)
        .ok_or_else(|| anyhow!("score-sharpness abstain-band sweep produced no candidates"))
}

fn score_sharpness_abstain_band_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    score_sharpness_model: &ScoreSharpnessCalibrationModel,
    abstain_low: f32,
    abstain_high: f32,
) -> Result<(SplitReport, AbstainBandApplicationStats)> {
    let rows = score_sharpness_cross_window_rows(
        table,
        indices,
        &score_sharpness_model.base_model,
        score_sharpness_model.scope,
        score_sharpness_model.low_cutoff,
        score_sharpness_model.low_tail_scale,
        score_sharpness_model.high_cutoff,
        score_sharpness_model.high_tail_scale,
    )?;
    Ok(abstain_band_report_from_rows(
        &rows,
        abstain_low,
        abstain_high,
    ))
}

fn score_sharpness_abstain_band_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &ScoreSharpnessAbstainBandModel,
) -> Result<(SplitReport, AbstainBandApplicationStats)> {
    let rows = score_sharpness_calibrated_rows(table, indices, &model.base_model)?;
    Ok(abstain_band_report_from_rows(
        &rows,
        model.abstain_low,
        model.abstain_high,
    ))
}

fn abstain_band_report_from_rows(
    rows: &[PredictionRow],
    abstain_low: f32,
    abstain_high: f32,
) -> (SplitReport, AbstainBandApplicationStats) {
    let active_rows = rows
        .iter()
        .filter(|row| !score_in_abstain_band(row.predicted_oracle_pass, abstain_low, abstain_high))
        .cloned()
        .collect::<Vec<_>>();
    let abstained_actual_pass_count = rows
        .iter()
        .filter(|row| {
            row.actual_oracle_pass
                && score_in_abstain_band(row.predicted_oracle_pass, abstain_low, abstain_high)
        })
        .count();
    let abstained_actual_fail_count = rows
        .iter()
        .filter(|row| {
            !row.actual_oracle_pass
                && score_in_abstain_band(row.predicted_oracle_pass, abstain_low, abstain_high)
        })
        .count();
    let active_row_count = active_rows.len();
    let per_category = abstain_band_category_stats(rows, &active_rows);
    let stats = AbstainBandApplicationStats {
        total_row_count: rows.len(),
        active_row_count,
        abstained_row_count: rows.len().saturating_sub(active_row_count),
        active_coverage: if rows.is_empty() {
            0.0
        } else {
            active_row_count as f32 / rows.len() as f32
        },
        abstained_actual_pass_count,
        abstained_actual_fail_count,
        per_category,
    };
    (
        SplitReport {
            metrics: metrics_for(&active_rows),
            per_category_metrics: per_category_metrics(&active_rows),
        },
        stats,
    )
}

fn abstain_band_category_stats(
    rows: &[PredictionRow],
    active_rows: &[PredictionRow],
) -> BTreeMap<String, AbstainBandCategoryStats> {
    REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            let category_rows = rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            let active_category_rows = active_rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            let active_metrics = metrics_for(&active_category_rows);
            let (active_gate_contract, active_gate_blockers) =
                abstain_active_gate_contract(category, &active_metrics);
            let total_row_count = category_rows.len();
            let active_row_count = active_category_rows.len();
            let abstained_row_count = total_row_count.saturating_sub(active_row_count);
            let active_actual_pass_count = active_metrics.actual_pass_count;
            let active_actual_fail_count = active_metrics.actual_fail_count;
            let active_predicted_pass_count = active_metrics.predicted_pass_count;
            let active_predicted_fail_count = active_metrics.predicted_fail_count;
            let abstained_actual_pass_count = category_rows
                .iter()
                .filter(|row| {
                    row.actual_oracle_pass
                        && !active_category_rows.iter().any(|active| {
                            active.task_id == row.task_id && active.category == row.category
                        })
                })
                .count();
            let abstained_actual_fail_count = category_rows
                .iter()
                .filter(|row| {
                    !row.actual_oracle_pass
                        && !active_category_rows.iter().any(|active| {
                            active.task_id == row.task_id && active.category == row.category
                        })
                })
                .count();
            (
                (*category).to_string(),
                AbstainBandCategoryStats {
                    total_row_count,
                    active_row_count,
                    abstained_row_count,
                    active_coverage: if total_row_count == 0 {
                        0.0
                    } else {
                        active_row_count as f32 / total_row_count as f32
                    },
                    active_actual_pass_count,
                    active_actual_fail_count,
                    active_predicted_pass_count,
                    active_predicted_fail_count,
                    abstained_actual_pass_count,
                    abstained_actual_fail_count,
                    active_oracle_variance_class: active_metrics.oracle_variance_class,
                    active_gate_contract,
                    active_gate_passed: active_gate_blockers.is_empty(),
                    active_gate_blockers,
                },
            )
        })
        .collect()
}

fn abstain_band_repair_blockers(
    active_split: &SplitReport,
    application: &AbstainBandApplicationStats,
    model: &ScoreSharpnessAbstainBandModel,
) -> Vec<String> {
    let mut blockers = aggregate_repair_blockers(&active_split.metrics);
    if application.active_coverage + 1.0e-6 < model.min_active_coverage {
        blockers.push(format!(
            "active_coverage_below_floor: {:.6} < {:.6}",
            application.active_coverage, model.min_active_coverage
        ));
    }
    for category in TRUE_MIXED_CATEGORIES {
        let Some(stats) = application.per_category.get(category) else {
            blockers.push(format!("{category}:missing_abstain_category_stats"));
            continue;
        };
        for blocker in &stats.active_gate_blockers {
            blockers.push(format!("{category}:{blocker}"));
        }
    }
    blockers
}

fn aggregate_repair_blockers(metrics: &MetricBlock) -> Vec<String> {
    let mut blockers = Vec::new();
    if metrics
        .prediction_oracle_correlation
        .map(|value| value < BASELINE_536_CORRELATION)
        .unwrap_or(true)
    {
        blockers.push(format!(
            "aggregate_correlation_not_above_536: {:?}",
            metrics.prediction_oracle_correlation
        ));
    }
    if metrics.brier > BASELINE_536_BRIER {
        blockers.push(format!("aggregate_brier_regressed: {:.9}", metrics.brier));
    }
    if metrics.ece > BASELINE_536_ECE {
        blockers.push(format!("aggregate_ece_regressed: {:.9}", metrics.ece));
    }
    blockers
}

fn abstain_active_gate_contract(_category: &str, metrics: &MetricBlock) -> (String, Vec<String>) {
    match metrics.oracle_variance_class {
        OracleVarianceClass::Mixed => {
            let mut blockers = Vec::new();
            if metrics.predicted_pass_count == 0 || metrics.predicted_fail_count == 0 {
                blockers.push(format!(
                    "threshold_degenerate_predicted_pass={} predicted_fail={}",
                    metrics.predicted_pass_count, metrics.predicted_fail_count
                ));
            }
            if metrics
                .prediction_oracle_correlation
                .map(|value| value <= 0.0)
                .unwrap_or(true)
            {
                blockers.push(format!(
                    "non_positive_correlation={:?}",
                    metrics.prediction_oracle_correlation
                ));
            }
            (
                "mixed_positive_correlation_and_threshold_support".to_string(),
                blockers,
            )
        }
        OracleVarianceClass::AllFail => degenerate_active_gate_contract(
            "all_fail_recall_confidence_brier_ece",
            metrics.fail_recall,
            metrics.actual_class_confidence_mean,
            metrics.brier,
            metrics.ece,
        ),
        OracleVarianceClass::AllPass => degenerate_active_gate_contract(
            "all_pass_recall_confidence_brier_ece",
            metrics.pass_recall,
            metrics.actual_class_confidence_mean,
            metrics.brier,
            metrics.ece,
        ),
        OracleVarianceClass::Empty => (
            "empty_active_cell".to_string(),
            vec!["active_row_count_zero".to_string()],
        ),
    }
}

fn degenerate_active_gate_contract(
    metric_name: &str,
    recall: Option<f32>,
    actual_class_confidence_mean: f32,
    brier: f32,
    ece: f32,
) -> (String, Vec<String>) {
    let mut blockers = Vec::new();
    match recall {
        Some(value) if value >= DEGENERATE_RECALL_TARGET => {}
        Some(value) => blockers.push(format!(
            "{metric_name}.recall {value:.6} < {DEGENERATE_RECALL_TARGET:.6}"
        )),
        None => blockers.push(format!("{metric_name}.recall unavailable")),
    }
    if actual_class_confidence_mean < DEGENERATE_CLASS_CONFIDENCE_TARGET {
        blockers.push(format!(
            "{metric_name}.actual_class_confidence_mean {actual_class_confidence_mean:.6} < {DEGENERATE_CLASS_CONFIDENCE_TARGET:.6}"
        ));
    }
    if brier > DEGENERATE_BRIER_MAX {
        blockers.push(format!(
            "{metric_name}.brier {brier:.6} > {DEGENERATE_BRIER_MAX:.6}"
        ));
    }
    if ece > DEGENERATE_ECE_MAX {
        blockers.push(format!(
            "{metric_name}.ece {ece:.6} > {DEGENERATE_ECE_MAX:.6}"
        ));
    }
    (metric_name.to_string(), blockers)
}

fn score_in_abstain_band(score: f32, abstain_low: f32, abstain_high: f32) -> bool {
    score >= abstain_low && score <= abstain_high
}

fn abstain_band_forbidden_reason(
    selection_objective: f32,
    application: &AbstainBandApplicationStats,
    model: &ScoreSharpnessAbstainBandModel,
) -> Option<String> {
    if application.active_coverage + 1.0e-6 < model.min_active_coverage {
        return Some(format!(
            "active_coverage_below_floor: {:.6} < {:.6}",
            application.active_coverage, model.min_active_coverage
        ));
    }
    if application.active_row_count < 2 {
        return Some("active_row_count_below_minimum".to_string());
    }
    if !selection_objective.is_finite() {
        return Some("selection_objective_non_finite".to_string());
    }
    None
}

fn abstain_band_complexity_penalty(
    application: &AbstainBandApplicationStats,
    model: &ScoreSharpnessAbstainBandModel,
) -> f32 {
    let abstain_fraction = 1.0 - application.active_coverage;
    let floor_gap = (model.min_active_coverage - application.active_coverage).max(0.0);
    0.05 * abstain_fraction + 0.50 * floor_gap
}

fn select_cause_band_calibration_repair(
    table: &FeatureTable,
    cause_bin: &CauseBinCalibrationReport,
) -> Result<CauseBandCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best: Option<CauseBandCalibrationReport> = None;
    for scope in [CalibrationScope::TrueMixedPerCategory] {
        for min_support in [3_usize, 5, 8] {
            for smoothing in [1.0_f32, 3.0, 8.0, 16.0] {
                for prior_weight in [0.20_f32, 0.35, 0.50, 0.65] {
                    let model = fit_cause_band_calibration_model(
                        table,
                        &calibration_indices,
                        cause_bin.model.clone(),
                        scope,
                        min_support,
                        smoothing,
                        prior_weight,
                    )?;
                    let calibration = cause_band_cross_window_report(
                        table,
                        &calibration_indices,
                        &cause_bin.model,
                        scope,
                        min_support,
                        smoothing,
                        prior_weight,
                    )?;
                    let holdout = cause_band_split_report(table, &holdout_indices, &model)?;
                    let calibration_application =
                        cause_band_application_stats(table, &calibration_indices, &model)?;
                    let holdout_application =
                        cause_band_application_stats(table, &holdout_indices, &model)?;
                    let selection_objective = selection_objective(&calibration.metrics)
                        + cause_band_complexity_penalty(&model);
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = CauseBandCalibrationReport {
                        selected_on: "calibration_leave_one_window_out".to_string(),
                        selection_forbidden: false,
                        forbidden_reason: None,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        calibration_application,
                        holdout_application,
                        blockers,
                    };
                    if best
                        .as_ref()
                        .map(|previous| report.selection_objective < previous.selection_objective)
                        .unwrap_or(true)
                    {
                        best = Some(report);
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("cause-band calibration sweep produced no candidates"))
}

#[allow(clippy::too_many_arguments)]
fn fit_cause_band_calibration_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: CauseBinCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<CauseBandCalibrationModel> {
    ensure!(min_support > 0, "cause-band min_support must be positive");
    ensure!(
        smoothing >= 0.0,
        "cause-band smoothing must be non-negative"
    );
    ensure!(
        (0.0..=1.0).contains(&prior_weight),
        "cause-band prior_weight must be in [0, 1]"
    );
    let base_rows = cause_bin_calibrated_rows(table, indices, &base_model)?;
    let global_prior = pass_rate(&base_rows);
    let mut stats = BTreeMap::<String, (usize, usize, f32)>::new();
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        let feature_row = &table.rows[*idx];
        if !calibration_scope_applies(scope, &feature_row.category) {
            continue;
        }
        let scope_key = calibration_scope_key(scope, &feature_row.category);
        for key in cause_band_keys(feature_row, &scope_key)? {
            let entry = stats.entry(key).or_insert((0, 0, 0.0));
            entry.0 += 1;
            entry.1 += usize::from(base_row.actual_oracle_pass);
            entry.2 += base_row.predicted_oracle_pass;
        }
    }
    ensure!(
        !stats.is_empty(),
        "cause-band calibration produced no scoped cause-band stats"
    );
    let key_stats = stats
        .into_iter()
        .map(|(key, (support, positives, score_sum))| {
            let prior =
                (positives as f32 + smoothing * global_prior) / (support as f32 + smoothing);
            (
                key,
                CauseBandKeyStats {
                    support,
                    positives,
                    prior: prior.clamp(0.001, 0.999),
                    score_mean: score_sum / support as f32,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(CauseBandCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        scope,
        min_support,
        smoothing,
        prior_weight,
        key_stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn cause_band_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    cause_bin_model: &CauseBinCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SplitReport> {
    let rows = cause_band_cross_window_rows(
        table,
        indices,
        cause_bin_model,
        scope,
        min_support,
        smoothing,
        prior_weight,
    )?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

#[allow(clippy::too_many_arguments)]
fn cause_band_cross_window_rows(
    table: &FeatureTable,
    indices: &[usize],
    cause_bin_model: &CauseBinCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<Vec<PredictionRow>> {
    ensure!(
        indices.len() >= cause_bin_model.base_model.window_count,
        "cause-band cross-window split needs at least one row per window"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_window in 0..cause_bin_model.base_model.window_count {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_bin_model.base_model.window_count)
                    != held_out_window
            })
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_bin_model.base_model.window_count)
                    == held_out_window
            })
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "cause-band cross-window split produced an empty window"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            cause_bin_model
                .base_model
                .base_model
                .affine_model
                .base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            cause_bin_model.base_model.base_model.affine_model.clone(),
            cause_bin_model.base_model.base_model.scope,
            cause_bin_model.base_model.base_model.shrinkage,
        )?;
        let fold_cause = fit_cause_residual_model(
            table,
            &fit_indices,
            fold_mean_bias,
            cause_bin_model.base_model.scope,
            cause_bin_model.base_model.window_count,
            cause_bin_model.base_model.regularization,
            cause_bin_model.base_model.correction_scale,
            cause_bin_model.base_model.correction_cap,
        )?;
        let fold_cause_bin = fit_cause_bin_calibration_model(
            table,
            &fit_indices,
            fold_cause,
            cause_bin_model.scope,
            cause_bin_model.bin_count,
            cause_bin_model.min_support,
            cause_bin_model.smoothing,
            cause_bin_model.prior_weight,
        )?;
        let model = fit_cause_band_calibration_model(
            table,
            &fit_indices,
            fold_cause_bin,
            scope,
            min_support,
            smoothing,
            prior_weight,
        )?;
        for (idx, row) in eval_indices.iter().copied().zip(cause_band_calibrated_rows(
            table,
            &eval_indices,
            &model,
        )?) {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "cause-band cross-window produced duplicate row index {idx}"
            );
        }
    }
    indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("cause-band cross-window missing row index {idx}"))
        })
        .collect()
}

fn cause_band_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBandCalibrationModel,
) -> Result<SplitReport> {
    let rows = cause_band_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn cause_band_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBandCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &model.base_model)?;
    indices
        .iter()
        .zip(base_rows.iter())
        .map(|(idx, base_row)| {
            let score = apply_cause_band_calibration(base_row, &table.rows[*idx], model)?;
            Ok(PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            })
        })
        .collect()
}

fn cause_band_application_stats(
    table: &FeatureTable,
    indices: &[usize],
    model: &CauseBandCalibrationModel,
) -> Result<CauseBandApplicationStats> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &model.base_model)?;
    let mut stats = CauseBandApplicationStats {
        scoped_row_count: 0,
        applied_row_count: 0,
        missing_key_row_count: 0,
        under_min_support_row_count: 0,
        non_scoped_row_count: 0,
    };
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        if !calibration_scope_applies(model.scope, &base_row.category) {
            stats.non_scoped_row_count += 1;
            continue;
        }
        stats.scoped_row_count += 1;
        match cause_band_matching_stats(base_row, &table.rows[*idx], model)? {
            Some(_matching) => stats.applied_row_count += 1,
            None => {
                let scope_key = calibration_scope_key(model.scope, &base_row.category);
                let keys = cause_band_keys(&table.rows[*idx], &scope_key)?;
                if keys.iter().any(|key| model.key_stats.contains_key(key)) {
                    stats.under_min_support_row_count += 1;
                } else {
                    stats.missing_key_row_count += 1;
                }
            }
        }
    }
    Ok(stats)
}

fn apply_cause_band_calibration(
    base_row: &PredictionRow,
    feature_row: &FeatureRow,
    model: &CauseBandCalibrationModel,
) -> Result<f32> {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return Ok(base_score);
    }
    let Some(stats) = cause_band_matching_stats(base_row, feature_row, model)? else {
        return Ok(base_score);
    };
    let support_weight = if model.smoothing <= f32::EPSILON {
        1.0
    } else {
        stats.support as f32 / (stats.support as f32 + model.smoothing)
    };
    let effective_weight = (model.prior_weight * support_weight).clamp(0.0, 1.0);
    Ok(
        ((1.0 - effective_weight) * base_score + effective_weight * stats.prior)
            .clamp(0.001, 0.999),
    )
}

fn cause_band_matching_stats<'a>(
    base_row: &PredictionRow,
    feature_row: &FeatureRow,
    model: &'a CauseBandCalibrationModel,
) -> Result<Option<&'a CauseBandKeyStats>> {
    let scope_key = calibration_scope_key(model.scope, &base_row.category);
    let keys = cause_band_keys(feature_row, &scope_key)?;
    Ok(keys
        .iter()
        .filter_map(|key| model.key_stats.get(key))
        .filter(|stats| stats.support >= model.min_support)
        .max_by(|left, right| {
            left.support.cmp(&right.support).then_with(|| {
                (left.prior - base_row.predicted_oracle_pass)
                    .abs()
                    .partial_cmp(&(right.prior - base_row.predicted_oracle_pass).abs())
                    .unwrap_or(Ordering::Equal)
            })
        }))
}

fn cause_band_keys(feature_row: &FeatureRow, scope_key: &str) -> Result<Vec<String>> {
    ensure!(
        feature_row.cause_features.len() >= 50,
        "cause-band feature count mismatch for {}:{} got {}",
        feature_row.task_id,
        feature_row.category,
        feature_row.cause_features.len()
    );
    let f = &feature_row.cause_features;
    let temporal_shift = bucket_feature(
        [f[0].abs(), f[4].abs(), f[8].abs()]
            .into_iter()
            .fold(0.0_f32, f32::max),
        &[0.01, 0.05, 0.10, 0.25, 0.50],
    );
    let temporal_ratio =
        bucket_feature((f[3] + f[7] + f[11]) / 3.0, &[0.50, 0.90, 1.10, 1.50, 3.0]);
    let live_label_delta = bucket_feature(f[18] + f[19] + f[20], &[0.5, 1.5, 3.5, 7.5, 15.0]);
    let live_label_overlap = bucket_feature(
        (f[21] + f[22] + f[23]) / 3.0,
        &[0.20, 0.50, 0.75, 0.90, 0.98],
    );
    let file_movement = bucket_feature(
        f[27] + f[28] + f[29] + f[31].abs(),
        &[0.5, 1.5, 3.5, 7.5, 15.0],
    );
    let file_overlap = bucket_feature(
        (f[24] + f[25] + f[26]) / 3.0,
        &[0.20, 0.50, 0.75, 0.90, 0.98],
    );
    let failure_skill = signed_band(f[32] - f[33] + f[34] + f[35] + f[36] + f[37]);
    let pass_skill = signed_band(f[38] - f[39] + f[40] + f[41] + f[42] + f[43]);
    let context_skill = signed_band(f[44] - f[45] + f[46] + f[47] + f[48] + f[49]);

    let prefix = format!("scope:{}", key_component(scope_key));
    let mut keys = vec![
        format!("{prefix}|temporal_shift:{temporal_shift}"),
        format!("{prefix}|temporal_ratio:{temporal_ratio}"),
        format!("{prefix}|live_label_delta:{live_label_delta}"),
        format!("{prefix}|live_label_overlap:{live_label_overlap}"),
        format!("{prefix}|file_movement:{file_movement}"),
        format!("{prefix}|file_overlap:{file_overlap}"),
        format!("{prefix}|failure_skill:{failure_skill}"),
        format!("{prefix}|pass_skill:{pass_skill}"),
        format!("{prefix}|context_skill:{context_skill}"),
        format!("{prefix}|temporal_shift:{temporal_shift}|failure_skill:{failure_skill}"),
        format!("{prefix}|file_movement:{file_movement}|pass_skill:{pass_skill}"),
        format!("{prefix}|live_label_delta:{live_label_delta}|context_skill:{context_skill}"),
    ];
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn bucket_feature(value: f32, thresholds: &[f32]) -> usize {
    let finite_value = if value.is_finite() { value } else { 0.0 };
    thresholds
        .iter()
        .position(|threshold| finite_value <= *threshold)
        .unwrap_or(thresholds.len())
}

fn signed_band(value: f32) -> &'static str {
    if value <= -1.0 {
        "neg_large"
    } else if value < -0.05 {
        "neg"
    } else if value < 0.05 {
        "zero"
    } else if value < 1.0 {
        "pos"
    } else {
        "pos_large"
    }
}

fn cause_band_complexity_penalty(model: &CauseBandCalibrationModel) -> f32 {
    let sparse_key_count = model
        .key_stats
        .values()
        .filter(|stats| stats.support < model.min_support)
        .count();
    let sparse_penalty = if model.key_stats.is_empty() {
        0.020
    } else {
        0.008 * sparse_key_count as f32 / model.key_stats.len() as f32
    };
    let support_penalty = if model.min_support < 5 { 0.006 } else { 0.0 };
    let weight_penalty = if model.prior_weight > 0.50 {
        0.006
    } else {
        0.0
    };
    sparse_penalty + support_penalty + weight_penalty
}

fn select_final_bin_calibration_repair(
    table: &FeatureTable,
    cause_bin: &CauseBinCalibrationReport,
) -> Result<FinalBinCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best_allowed: Option<FinalBinCalibrationReport> = None;
    let mut best_forbidden: Option<FinalBinCalibrationReport> = None;
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for bin_count in [5_usize, 10] {
            for smoothing in [3.0_f32, 10.0, 30.0] {
                for raw_weight in [0.50_f32, 0.75, 0.90, 0.95] {
                    let model = fit_final_bin_calibration_model(
                        table,
                        &calibration_indices,
                        cause_bin.model.clone(),
                        scope,
                        bin_count,
                        smoothing,
                        raw_weight,
                    )?;
                    let calibration = final_bin_cross_window_report(
                        table,
                        &calibration_indices,
                        &cause_bin.model,
                        scope,
                        bin_count,
                        smoothing,
                        raw_weight,
                    )?;
                    let holdout = final_bin_split_report(table, &holdout_indices, &model)?;
                    let selection_objective = selection_objective(&calibration.metrics)
                        + final_bin_complexity_penalty(&model);
                    let forbidden_reason = final_bin_forbidden_reason(
                        selection_objective,
                        &calibration.metrics,
                        &holdout.metrics,
                        cause_bin,
                    );
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = FinalBinCalibrationReport {
                        selected_on: "calibration_leave_one_window_out".to_string(),
                        selection_forbidden: forbidden_reason.is_some(),
                        forbidden_reason,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        blockers,
                    };
                    let best_slot = if report.selection_forbidden {
                        &mut best_forbidden
                    } else {
                        &mut best_allowed
                    };
                    if best_slot
                        .as_ref()
                        .map(|previous| {
                            report.selection_objective + FINAL_BIN_GRID_OBJECTIVE_EPSILON
                                < previous.selection_objective
                        })
                        .unwrap_or(true)
                    {
                        *best_slot = Some(report);
                    }
                }
            }
        }
    }
    best_allowed
        .or(best_forbidden)
        .ok_or_else(|| anyhow!("final-bin calibration sweep produced no candidates"))
}

fn final_bin_forbidden_reason(
    selection_objective: f32,
    calibration: &MetricBlock,
    holdout: &MetricBlock,
    cause_bin: &CauseBinCalibrationReport,
) -> Option<String> {
    if selection_objective + FINAL_BIN_LEGAL_BEST_OBJECTIVE_MARGIN >= cause_bin.selection_objective
    {
        return Some(format!(
            "objective_delta_below_material_margin: final_bin={selection_objective:.9} cause_bin={:.9} required_margin={FINAL_BIN_LEGAL_BEST_OBJECTIVE_MARGIN:.9}",
            cause_bin.selection_objective
        ));
    }
    if let Some(reason) =
        metric_nonregression_failure(calibration, &cause_bin.calibration.metrics, "calibration")
    {
        return Some(reason);
    }
    metric_nonregression_failure(holdout, &cause_bin.holdout.metrics, "holdout")
}

fn metric_nonregression_failure(
    candidate: &MetricBlock,
    incumbent: &MetricBlock,
    split: &str,
) -> Option<String> {
    if !optional_metric_at_least(
        candidate.prediction_oracle_correlation,
        incumbent.prediction_oracle_correlation,
    ) {
        return Some(format!(
            "{split}_correlation_regressed: final_bin={:?} cause_bin={:?}",
            candidate.prediction_oracle_correlation, incumbent.prediction_oracle_correlation
        ));
    }
    if candidate.brier > incumbent.brier + FINAL_BIN_NONREGRESSION_EPSILON {
        return Some(format!(
            "{split}_brier_regressed: final_bin={:.9} cause_bin={:.9}",
            candidate.brier, incumbent.brier
        ));
    }
    if candidate.ece > incumbent.ece + FINAL_BIN_NONREGRESSION_EPSILON {
        return Some(format!(
            "{split}_ece_regressed: final_bin={:.9} cause_bin={:.9}",
            candidate.ece, incumbent.ece
        ));
    }
    if candidate.accuracy + FINAL_BIN_NONREGRESSION_EPSILON < incumbent.accuracy {
        return Some(format!(
            "{split}_accuracy_regressed: final_bin={:.9} cause_bin={:.9}",
            candidate.accuracy, incumbent.accuracy
        ));
    }
    if !optional_metric_at_least(candidate.pass_recall, incumbent.pass_recall) {
        return Some(format!(
            "{split}_pass_recall_regressed: final_bin={:?} cause_bin={:?}",
            candidate.pass_recall, incumbent.pass_recall
        ));
    }
    if !optional_metric_at_least(candidate.fail_recall, incumbent.fail_recall) {
        return Some(format!(
            "{split}_fail_recall_regressed: final_bin={:?} cause_bin={:?}",
            candidate.fail_recall, incumbent.fail_recall
        ));
    }
    None
}

fn optional_metric_at_least(candidate: Option<f32>, incumbent: Option<f32>) -> bool {
    match (candidate, incumbent) {
        (Some(candidate), Some(incumbent)) => {
            candidate + FINAL_BIN_NONREGRESSION_EPSILON >= incumbent
        }
        (Some(_), None) | (None, None) => true,
        (None, Some(_)) => false,
    }
}

fn fit_final_bin_calibration_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: CauseBinCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    smoothing: f32,
    raw_weight: f32,
) -> Result<FinalBinCalibrationModel> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &base_model)?;
    let probability_model = fit_probability_calibration_model(
        &base_rows,
        CURRENT_RBF,
        scope,
        bin_count,
        smoothing,
        raw_weight,
    )?;
    Ok(FinalBinCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        probability_model,
    })
}

fn final_bin_cross_window_report(
    table: &FeatureTable,
    indices: &[usize],
    cause_model: &CauseBinCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    smoothing: f32,
    raw_weight: f32,
) -> Result<SplitReport> {
    ensure!(
        indices.len() >= cause_model.base_model.window_count,
        "final-bin cross-window split needs at least one row per window"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_window in 0..cause_model.base_model.window_count {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_model.base_model.window_count)
                    != held_out_window
            })
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| {
                cause_window_index(&table.rows[*idx], cause_model.base_model.window_count)
                    == held_out_window
            })
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "final-bin cross-window split produced an empty window"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            cause_model
                .base_model
                .base_model
                .affine_model
                .base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            cause_model.base_model.base_model.affine_model.clone(),
            cause_model.base_model.base_model.scope,
            cause_model.base_model.base_model.shrinkage,
        )?;
        let fold_cause = fit_cause_residual_model(
            table,
            &fit_indices,
            fold_mean_bias,
            cause_model.base_model.scope,
            cause_model.base_model.window_count,
            cause_model.base_model.regularization,
            cause_model.base_model.correction_scale,
            cause_model.base_model.correction_cap,
        )?;
        let fold_cause_bin = fit_cause_bin_calibration_model(
            table,
            &fit_indices,
            fold_cause,
            cause_model.scope,
            cause_model.bin_count,
            cause_model.min_support,
            cause_model.smoothing,
            cause_model.prior_weight,
        )?;
        let model = fit_final_bin_calibration_model(
            table,
            &fit_indices,
            fold_cause_bin,
            scope,
            bin_count,
            smoothing,
            raw_weight,
        )?;
        for (idx, row) in eval_indices.iter().copied().zip(final_bin_calibrated_rows(
            table,
            &eval_indices,
            &model,
        )?) {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "final-bin cross-window produced duplicate row index {idx}"
            );
        }
    }
    let calibrated_rows = indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("final-bin cross-window missing row index {idx}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn final_bin_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &FinalBinCalibrationModel,
) -> Result<SplitReport> {
    let rows = final_bin_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn final_bin_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &FinalBinCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = cause_bin_calibrated_rows(table, indices, &model.base_model)?;
    probability_calibrated_rows(&base_rows, &model.probability_model)
}

fn final_bin_complexity_penalty(model: &FinalBinCalibrationModel) -> f32 {
    let layer_penalty = 0.006;
    let scope_penalty = match model.probability_model.scope {
        CalibrationScope::Global => 0.0,
        CalibrationScope::TrueMixedPerCategory => 0.006,
        CalibrationScope::PerCategory => 0.012,
    };
    let bin_penalty = if model.probability_model.bin_count > 5 {
        0.004
    } else {
        0.0
    };
    let raw_weight_penalty = if model.probability_model.raw_weight < 0.75 {
        0.006
    } else {
        0.0
    };
    layer_penalty + scope_penalty + bin_penalty + raw_weight_penalty
}

fn select_support_shrink_calibration_repair(
    table: &FeatureTable,
    mean_bias: &MeanBiasCalibrationReport,
) -> Result<SupportShrinkCalibrationReport> {
    let base_candidate = mean_bias.model.affine_model.base_candidate;
    let calibration_base_rows =
        prediction_rows_for_candidate(table, "calibration", base_candidate)?;
    let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
    let mut best: Option<SupportShrinkCalibrationReport> = None;
    for scope in [
        CalibrationScope::Global,
        CalibrationScope::PerCategory,
        CalibrationScope::TrueMixedPerCategory,
    ] {
        for smoothing in [0.0_f32, 3.0, 10.0, 30.0] {
            for prior_weight in [0.05_f32, 0.10, 0.15, 0.20, 0.30, 0.40] {
                let model = fit_support_shrink_calibration_model(
                    &calibration_base_rows,
                    mean_bias.model.clone(),
                    scope,
                    smoothing,
                    prior_weight,
                )?;
                let calibration = support_shrink_calibration_cross_split_report(
                    &calibration_base_rows,
                    &mean_bias.model,
                    scope,
                    smoothing,
                    prior_weight,
                )?;
                let holdout = support_shrink_calibration_split_report(&holdout_base_rows, &model)?;
                let selection_objective = selection_objective(&calibration.metrics);
                let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                let report = SupportShrinkCalibrationReport {
                    selected_on: "calibration".to_string(),
                    selection_forbidden: false,
                    forbidden_reason: None,
                    selection_objective,
                    model,
                    calibration,
                    holdout,
                    blockers,
                };
                if best
                    .as_ref()
                    .map(|previous| report.selection_objective < previous.selection_objective)
                    .unwrap_or(true)
                {
                    best = Some(report);
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("support-shrink calibration sweep produced no candidates"))
}

fn fit_support_shrink_calibration_model(
    rows: &[PredictionRow],
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SupportShrinkCalibrationModel> {
    ensure!(
        smoothing >= 0.0,
        "support-shrink smoothing must be non-negative"
    );
    ensure!(
        (0.0..=1.0).contains(&prior_weight),
        "support-shrink prior_weight must be in [0, 1]"
    );
    let global_prior = pass_rate(rows);
    let mut stats = BTreeMap::<String, (usize, usize)>::new();
    for row in rows {
        if !calibration_scope_applies(scope, &row.category) {
            continue;
        }
        let key = calibration_scope_key(scope, &row.category);
        let entry = stats.entry(key).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += usize::from(row.actual_oracle_pass);
    }
    ensure!(
        !stats.is_empty(),
        "support-shrink calibration produced no scoped rows"
    );
    let mut prior_by_key = BTreeMap::new();
    let mut support_by_key = BTreeMap::new();
    for (key, (count, positives)) in stats {
        let prior = (positives as f32 + smoothing * global_prior) / (count as f32 + smoothing);
        prior_by_key.insert(key.clone(), prior.clamp(0.001, 0.999));
        support_by_key.insert(key, count);
    }
    Ok(SupportShrinkCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        scope,
        smoothing,
        prior_weight,
        global_prior,
        prior_by_key,
        support_by_key,
    })
}

fn support_shrink_calibration_cross_split_report(
    rows: &[PredictionRow],
    base_model: &MeanBiasCalibrationModel,
    scope: CalibrationScope,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SplitReport> {
    ensure!(
        rows.len() >= 2,
        "support-shrink calibration cross split needs at least two rows"
    );
    let mut calibrated_rows = Vec::with_capacity(rows.len());
    for held_out_fold in 0..2_u8 {
        let fit_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) != held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        let eval_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) == held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !fit_rows.is_empty() && !eval_rows.is_empty(),
            "support-shrink calibration cross split produced an empty fold"
        );
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_rows,
            base_model.affine_model.clone(),
            base_model.scope,
            base_model.shrinkage,
        )?;
        let model = fit_support_shrink_calibration_model(
            &fit_rows,
            fold_mean_bias,
            scope,
            smoothing,
            prior_weight,
        )?;
        calibrated_rows.extend(support_shrink_calibrated_rows(&eval_rows, &model)?);
    }
    ensure!(
        calibrated_rows.len() == rows.len(),
        "support-shrink calibration cross split lost rows"
    );
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn support_shrink_calibration_split_report(
    rows: &[PredictionRow],
    model: &SupportShrinkCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = support_shrink_calibrated_rows(rows, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn support_shrink_calibrated_rows(
    rows: &[PredictionRow],
    model: &SupportShrinkCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = apply_support_shrink_calibration(row, model)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_support_shrink_calibration(
    row: &PredictionRow,
    model: &SupportShrinkCalibrationModel,
) -> Result<f32> {
    let base_score = apply_mean_bias_calibration(row, &model.base_model)?;
    if !calibration_scope_applies(model.scope, &row.category) {
        return Ok(base_score);
    }
    let key = calibration_scope_key(model.scope, &row.category);
    let prior = model
        .prior_by_key
        .get(&key)
        .ok_or_else(|| anyhow!("support-shrink calibration missing prior for key {key}"))?;
    let effective_prior_weight = support_shrink_effective_prior_weight(model, &key)?;
    Ok(
        ((1.0 - effective_prior_weight) * base_score + effective_prior_weight * prior)
            .clamp(0.001, 0.999),
    )
}

fn support_shrink_effective_prior_weight(
    model: &SupportShrinkCalibrationModel,
    key: &str,
) -> Result<f32> {
    let support = *model
        .support_by_key
        .get(key)
        .ok_or_else(|| anyhow!("support-shrink calibration missing support for key {key}"))?
        as f32;
    ensure!(
        support.is_finite() && support > 0.0,
        "support-shrink calibration support must be positive for key {key}"
    );
    let weight = if model.smoothing <= 0.0 {
        model.prior_weight
    } else {
        model.prior_weight * (model.smoothing / (support + model.smoothing))
    };
    Ok(weight.clamp(0.0, 1.0))
}

fn select_evidence_consequence_calibration_repair(
    table: &FeatureTable,
    mean_bias: &MeanBiasCalibrationReport,
) -> Result<EvidenceConsequenceCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best: Option<EvidenceConsequenceCalibrationReport> = None;
    for scope in [
        CalibrationScope::TrueMixedPerCategory,
        CalibrationScope::PerCategory,
        CalibrationScope::Global,
    ] {
        for min_support in [3_usize, 5, 8, 12] {
            for smoothing in [0.0_f32, 1.0, 3.0, 10.0] {
                for prior_weight in [0.10_f32, 0.20, 0.35, 0.50, 0.65] {
                    let model = fit_evidence_consequence_calibration_model(
                        table,
                        &calibration_indices,
                        mean_bias.model.clone(),
                        scope,
                        min_support,
                        smoothing,
                        prior_weight,
                    )?;
                    let calibration = evidence_consequence_cross_split_report(
                        table,
                        &calibration_indices,
                        &mean_bias.model,
                        scope,
                        min_support,
                        smoothing,
                        prior_weight,
                    )?;
                    let holdout =
                        evidence_consequence_split_report(table, &holdout_indices, &model)?;
                    let selection_objective = selection_objective(&calibration.metrics)
                        + evidence_consequence_complexity_penalty(&model);
                    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                    let report = EvidenceConsequenceCalibrationReport {
                        selected_on: "calibration".to_string(),
                        selection_forbidden: false,
                        forbidden_reason: None,
                        selection_objective,
                        model,
                        calibration,
                        holdout,
                        blockers,
                    };
                    if best
                        .as_ref()
                        .map(|previous| report.selection_objective < previous.selection_objective)
                        .unwrap_or(true)
                    {
                        best = Some(report);
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("evidence-consequence calibration sweep produced no candidates"))
}

fn evidence_consequence_complexity_penalty(model: &EvidenceConsequenceCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.010,
        CalibrationScope::Global => 0.020,
    };
    let support_penalty = if model.min_support < 5 { 0.010 } else { 0.0 };
    let weight_penalty = if model.prior_weight > 0.50 {
        0.010
    } else {
        0.0
    };
    scope_penalty + support_penalty + weight_penalty
}

fn evidence_consequence_cross_split_report(
    table: &FeatureTable,
    indices: &[usize],
    base_model: &MeanBiasCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SplitReport> {
    ensure!(
        indices.len() >= 2,
        "evidence-consequence calibration cross split needs at least two rows"
    );
    let mut calibrated_rows = Vec::with_capacity(indices.len());
    for held_out_fold in 0..2_u8 {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) != held_out_fold)
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) == held_out_fold)
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "evidence-consequence calibration cross split produced an empty fold"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            base_model.affine_model.base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            base_model.affine_model.clone(),
            base_model.scope,
            base_model.shrinkage,
        )?;
        let model = fit_evidence_consequence_calibration_model(
            table,
            &fit_indices,
            fold_mean_bias,
            scope,
            min_support,
            smoothing,
            prior_weight,
        )?;
        calibrated_rows.extend(evidence_consequence_calibrated_rows(
            table,
            &eval_indices,
            &model,
        )?);
    }
    ensure!(
        calibrated_rows.len() == indices.len(),
        "evidence-consequence calibration cross split lost rows"
    );
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn fit_evidence_consequence_calibration_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: MeanBiasCalibrationModel,
    scope: CalibrationScope,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<EvidenceConsequenceCalibrationModel> {
    ensure!(
        min_support > 0,
        "evidence-consequence min_support must be positive"
    );
    ensure!(
        smoothing >= 0.0,
        "evidence-consequence smoothing must be non-negative"
    );
    ensure!(
        (0.0..=1.0).contains(&prior_weight),
        "evidence-consequence prior_weight must be in [0, 1]"
    );
    let base_rows = mean_bias_rows_for_indices(table, indices, &base_model)?;
    let global_prior = pass_rate(&base_rows);
    let mut stats = BTreeMap::<String, (usize, usize, f32)>::new();
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        let feature_row = &table.rows[*idx];
        if !calibration_scope_applies(scope, &feature_row.category) {
            continue;
        }
        ensure!(
            !feature_row.evidence_keys.is_empty(),
            "evidence key ladder missing for {}:{}",
            feature_row.task_id,
            feature_row.category
        );
        for key in &feature_row.evidence_keys {
            let entry = stats.entry(key.clone()).or_insert((0, 0, 0.0));
            entry.0 += 1;
            entry.1 += usize::from(base_row.actual_oracle_pass);
            entry.2 += base_row.predicted_oracle_pass;
        }
    }
    ensure!(
        !stats.is_empty(),
        "evidence-consequence calibration produced no scoped evidence stats"
    );
    let key_stats = stats
        .into_iter()
        .map(|(key, (support, positives, score_sum))| {
            let prior =
                (positives as f32 + smoothing * global_prior) / (support as f32 + smoothing);
            let score_mean = score_sum / support as f32;
            (
                key,
                EvidenceConsequenceKeyStats {
                    support,
                    positives,
                    prior: prior.clamp(0.001, 0.999),
                    score_mean,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(EvidenceConsequenceCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        scope,
        min_support,
        smoothing,
        prior_weight,
        global_prior,
        key_stats,
    })
}

fn evidence_consequence_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &EvidenceConsequenceCalibrationModel,
) -> Result<SplitReport> {
    let rows = evidence_consequence_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn evidence_consequence_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &EvidenceConsequenceCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = mean_bias_rows_for_indices(table, indices, &model.base_model)?;
    indices
        .iter()
        .zip(base_rows.iter())
        .map(|(idx, base_row)| {
            let score = apply_evidence_consequence_calibration(base_row, &table.rows[*idx], model)?;
            Ok(PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_evidence_consequence_calibration(
    base_row: &PredictionRow,
    feature_row: &FeatureRow,
    model: &EvidenceConsequenceCalibrationModel,
) -> Result<f32> {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return Ok(base_score);
    }
    let Some((_key, stats)) = feature_row
        .evidence_keys
        .iter()
        .filter_map(|key| model.key_stats.get(key).map(|stats| (key, stats)))
        .find(|(_key, stats)| stats.support >= model.min_support)
    else {
        return Ok(base_score);
    };
    let support_weight = if model.smoothing <= f32::EPSILON {
        1.0
    } else {
        stats.support as f32 / (stats.support as f32 + model.smoothing)
    };
    let effective_weight = (model.prior_weight * support_weight).clamp(0.0, 1.0);
    Ok(
        ((1.0 - effective_weight) * base_score + effective_weight * stats.prior)
            .clamp(0.001, 0.999),
    )
}

fn select_bin_consequence_calibration_repair(
    table: &FeatureTable,
    evidence: &EvidenceConsequenceCalibrationReport,
) -> Result<BinConsequenceCalibrationReport> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let mut best: Option<BinConsequenceCalibrationReport> = None;
    for scope in [CalibrationScope::TrueMixedPerCategory] {
        for bin_count in [10_usize] {
            for min_support in [5_usize, 8, 12] {
                for smoothing in [3.0_f32, 10.0] {
                    for prior_weight in [0.25_f32, 0.35, 0.50] {
                        let model = fit_bin_consequence_calibration_model(
                            table,
                            &calibration_indices,
                            evidence.model.clone(),
                            scope,
                            bin_count,
                            min_support,
                            smoothing,
                            prior_weight,
                        )?;
                        let calibration = bin_consequence_cross_split_report(
                            table,
                            &calibration_indices,
                            &evidence.model,
                            scope,
                            bin_count,
                            min_support,
                            smoothing,
                            prior_weight,
                        )?;
                        let holdout =
                            bin_consequence_split_report(table, &holdout_indices, &model)?;
                        let calibration_application =
                            bin_consequence_cross_split_application_stats(
                                table,
                                &calibration_indices,
                                &evidence.model,
                                scope,
                                bin_count,
                                min_support,
                                smoothing,
                                prior_weight,
                            )?;
                        if !calibration_application.all_passed {
                            continue;
                        }
                        let selection_objective = selection_objective(&calibration.metrics)
                            + bin_consequence_complexity_penalty(&model);
                        let blockers =
                            repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                        let report = BinConsequenceCalibrationReport {
                            selected_on: "calibration".to_string(),
                            selection_forbidden: false,
                            forbidden_reason: None,
                            selection_objective,
                            model,
                            calibration,
                            holdout,
                            blockers,
                        };
                        if best
                            .as_ref()
                            .map(|previous| {
                                report.selection_objective < previous.selection_objective
                            })
                            .unwrap_or(true)
                        {
                            best = Some(report);
                        }
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("bin-consequence calibration sweep produced no candidates"))
}

fn bin_consequence_complexity_penalty(model: &BinConsequenceCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::TrueMixedPerCategory => 0.0,
        CalibrationScope::PerCategory => 0.012,
        CalibrationScope::Global => 0.020,
    };
    let bin_penalty = if model.bin_count > 5 { 0.006 } else { 0.0 };
    let support_penalty = if model.min_support < 5 { 0.012 } else { 0.0 };
    let weight_penalty = if model.prior_weight > 0.50 {
        0.012
    } else {
        0.0
    };
    let sparse_key_count = model
        .key_stats
        .values()
        .filter(|stats| stats.support < 5)
        .count();
    let sparse_penalty = if model.key_stats.is_empty() {
        0.020
    } else {
        0.012 * sparse_key_count as f32 / model.key_stats.len() as f32
    };
    scope_penalty + bin_penalty + support_penalty + weight_penalty + sparse_penalty
}

fn bin_consequence_cross_split_report(
    table: &FeatureTable,
    indices: &[usize],
    base_model: &EvidenceConsequenceCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<SplitReport> {
    let calibrated_rows = bin_consequence_cross_split_rows(
        table,
        indices,
        base_model,
        scope,
        bin_count,
        min_support,
        smoothing,
        prior_weight,
    )?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn bin_consequence_cross_split_rows(
    table: &FeatureTable,
    indices: &[usize],
    base_model: &EvidenceConsequenceCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<Vec<PredictionRow>> {
    ensure!(
        indices.len() >= 2,
        "bin-consequence calibration cross split needs at least two rows"
    );
    let mut calibrated_rows_by_index = BTreeMap::<usize, PredictionRow>::new();
    for held_out_fold in 0..2_u8 {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) != held_out_fold)
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) == held_out_fold)
            .collect::<Vec<_>>();
        ensure!(
            !fit_indices.is_empty() && !eval_indices.is_empty(),
            "bin-consequence calibration cross split produced an empty fold"
        );
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            base_model.base_model.affine_model.base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            base_model.base_model.affine_model.clone(),
            base_model.base_model.scope,
            base_model.base_model.shrinkage,
        )?;
        let fold_evidence = fit_evidence_consequence_calibration_model(
            table,
            &fit_indices,
            fold_mean_bias,
            base_model.scope,
            base_model.min_support,
            base_model.smoothing,
            base_model.prior_weight,
        )?;
        let model = fit_bin_consequence_calibration_model(
            table,
            &fit_indices,
            fold_evidence,
            scope,
            bin_count,
            min_support,
            smoothing,
            prior_weight,
        )?;
        for (idx, row) in eval_indices
            .iter()
            .copied()
            .zip(bin_consequence_calibrated_rows(
                table,
                &eval_indices,
                &model,
            )?)
        {
            ensure!(
                calibrated_rows_by_index.insert(idx, row).is_none(),
                "bin-consequence cross split produced duplicate row index {idx}"
            );
        }
    }
    let calibrated_rows = indices
        .iter()
        .map(|idx| {
            calibrated_rows_by_index
                .remove(idx)
                .ok_or_else(|| anyhow!("bin-consequence cross split missing row index {idx}"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        calibrated_rows.len() == indices.len(),
        "bin-consequence calibration cross split lost rows"
    );
    Ok(calibrated_rows)
}

fn fit_bin_consequence_calibration_model(
    table: &FeatureTable,
    indices: &[usize],
    base_model: EvidenceConsequenceCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<BinConsequenceCalibrationModel> {
    ensure!(bin_count > 1, "bin-consequence bin_count must be > 1");
    ensure!(
        min_support > 0,
        "bin-consequence min_support must be positive"
    );
    ensure!(
        smoothing >= 0.0,
        "bin-consequence smoothing must be non-negative"
    );
    ensure!(
        (0.0..=1.0).contains(&prior_weight),
        "bin-consequence prior_weight must be in [0, 1]"
    );
    let base_rows = evidence_consequence_calibrated_rows(table, indices, &base_model)?;
    let global_prior = pass_rate(&base_rows);
    let mut stats = BTreeMap::<String, (usize, usize, f32)>::new();
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        let feature_row = &table.rows[*idx];
        if !calibration_scope_applies(scope, &feature_row.category) {
            continue;
        }
        ensure!(
            !feature_row.evidence_keys.is_empty(),
            "bin-consequence evidence key ladder missing for {}:{}",
            feature_row.task_id,
            feature_row.category
        );
        let bin_idx = probability_bin_index(base_row.predicted_oracle_pass, bin_count);
        for key in &feature_row.evidence_keys {
            let entry = stats
                .entry(bin_consequence_key(bin_idx, key))
                .or_insert((0, 0, 0.0));
            entry.0 += 1;
            entry.1 += usize::from(base_row.actual_oracle_pass);
            entry.2 += base_row.predicted_oracle_pass;
        }
    }
    ensure!(
        !stats.is_empty(),
        "bin-consequence calibration produced no scoped evidence stats"
    );
    let key_stats = stats
        .into_iter()
        .map(|(key, (support, positives, score_sum))| {
            let prior =
                (positives as f32 + smoothing * global_prior) / (support as f32 + smoothing);
            (
                key,
                BinConsequenceKeyStats {
                    support,
                    positives,
                    prior: prior.clamp(0.001, 0.999),
                    score_mean: score_sum / support as f32,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(BinConsequenceCalibrationModel {
        selected_on: "calibration".to_string(),
        base_model,
        scope,
        bin_count,
        min_support,
        smoothing,
        prior_weight,
        key_stats,
    })
}

fn bin_consequence_split_report(
    table: &FeatureTable,
    indices: &[usize],
    model: &BinConsequenceCalibrationModel,
) -> Result<SplitReport> {
    let rows = bin_consequence_calibrated_rows(table, indices, model)?;
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn bin_consequence_calibrated_rows(
    table: &FeatureTable,
    indices: &[usize],
    model: &BinConsequenceCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let base_rows = evidence_consequence_calibrated_rows(table, indices, &model.base_model)?;
    indices
        .iter()
        .zip(base_rows.iter())
        .map(|(idx, base_row)| {
            let score = apply_bin_consequence_calibration(base_row, &table.rows[*idx], model)?;
            Ok(PredictionRow {
                task_id: base_row.task_id.clone(),
                category: base_row.category.clone(),
                actual_oracle_pass: base_row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base_row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_bin_consequence_calibration(
    base_row: &PredictionRow,
    feature_row: &FeatureRow,
    model: &BinConsequenceCalibrationModel,
) -> Result<f32> {
    let base_score = base_row.predicted_oracle_pass;
    if !calibration_scope_applies(model.scope, &base_row.category) {
        return Ok(base_score);
    }
    let Some(stats) = bin_consequence_matching_stats(base_score, feature_row, model) else {
        return Ok(base_score);
    };
    let support_weight = if model.smoothing <= f32::EPSILON {
        1.0
    } else {
        stats.support as f32 / (stats.support as f32 + model.smoothing)
    };
    let effective_weight = (model.prior_weight * support_weight).clamp(0.0, 1.0);
    Ok(
        ((1.0 - effective_weight) * base_score + effective_weight * stats.prior)
            .clamp(0.001, 0.999),
    )
}

fn bin_consequence_matching_stats<'a>(
    base_score: f32,
    feature_row: &FeatureRow,
    model: &'a BinConsequenceCalibrationModel,
) -> Option<&'a BinConsequenceKeyStats> {
    let bin_idx = probability_bin_index(base_score, model.bin_count);
    feature_row
        .evidence_keys
        .iter()
        .filter_map(|key| model.key_stats.get(&bin_consequence_key(bin_idx, key)))
        .find(|stats| stats.support >= model.min_support)
}

fn bin_consequence_key(bin_idx: usize, key: &str) -> String {
    format!("bin:{bin_idx}|{key}")
}

fn bin_consequence_candidate_audit(
    table: &FeatureTable,
    report: &BinConsequenceCalibrationReport,
) -> Result<BinConsequenceCandidateAudit> {
    let calibration_indices = bucket_indices(table, "calibration");
    let holdout_indices = bucket_indices(table, "holdout");
    let model = &report.model;
    let calibration_rows = bin_consequence_cross_split_rows(
        table,
        &calibration_indices,
        &model.base_model,
        model.scope,
        model.bin_count,
        model.min_support,
        model.smoothing,
        model.prior_weight,
    )?;
    let holdout_rows = bin_consequence_calibrated_rows(table, &holdout_indices, model)?;
    let calibration_row_reproduction =
        reproduction_report(&report.calibration.metrics, &metrics_for(&calibration_rows));
    let holdout_row_reproduction =
        reproduction_report(&report.holdout.metrics, &metrics_for(&holdout_rows));
    let calibration_row_audit = split_row_audit(
        table,
        "calibration",
        &report.calibration,
        &calibration_rows,
        &calibration_row_reproduction,
    );
    let holdout_row_audit = split_row_audit(
        table,
        "holdout",
        &report.holdout,
        &holdout_rows,
        &holdout_row_reproduction,
    );
    let calibration_ece_breakdown = ece_breakdown_report(&calibration_rows);
    let holdout_ece_breakdown = ece_breakdown_report(&holdout_rows);
    let calibration_ece_breakdown_audit =
        ece_breakdown_audit(&report.calibration.metrics, &calibration_ece_breakdown);
    let holdout_ece_breakdown_audit =
        ece_breakdown_audit(&report.holdout.metrics, &holdout_ece_breakdown);
    let calibration_application = bin_consequence_cross_split_application_stats(
        table,
        &calibration_indices,
        &model.base_model,
        model.scope,
        model.bin_count,
        model.min_support,
        model.smoothing,
        model.prior_weight,
    )?;
    let holdout_application = bin_consequence_application_stats(table, &holdout_indices, model)?;
    Ok(BinConsequenceCandidateAudit {
        all_passed: calibration_row_reproduction.passed
            && holdout_row_reproduction.passed
            && calibration_row_audit.all_passed
            && holdout_row_audit.all_passed
            && calibration_ece_breakdown_audit.all_passed
            && holdout_ece_breakdown_audit.all_passed
            && calibration_application.all_passed
            && holdout_application.all_passed,
        calibration_row_reproduction,
        holdout_row_reproduction,
        calibration_row_audit,
        holdout_row_audit,
        calibration_ece_breakdown_audit,
        holdout_ece_breakdown_audit,
        calibration_application,
        holdout_application,
    })
}

fn bin_consequence_cross_split_application_stats(
    table: &FeatureTable,
    indices: &[usize],
    base_model: &EvidenceConsequenceCalibrationModel,
    scope: CalibrationScope,
    bin_count: usize,
    min_support: usize,
    smoothing: f32,
    prior_weight: f32,
) -> Result<BinConsequenceApplicationStats> {
    let mut accumulator = BinConsequenceApplicationAccumulator::default();
    for held_out_fold in 0..2_u8 {
        let fit_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) != held_out_fold)
            .collect::<Vec<_>>();
        let eval_indices = indices
            .iter()
            .copied()
            .filter(|idx| calibration_fold_feature(&table.rows[*idx]) == held_out_fold)
            .collect::<Vec<_>>();
        let fit_base_rows = prediction_rows_for_indices(
            table,
            &fit_indices,
            base_model.base_model.affine_model.base_candidate,
        )?;
        let fold_mean_bias = fit_mean_bias_calibration_model(
            &fit_base_rows,
            base_model.base_model.affine_model.clone(),
            base_model.base_model.scope,
            base_model.base_model.shrinkage,
        )?;
        let fold_evidence = fit_evidence_consequence_calibration_model(
            table,
            &fit_indices,
            fold_mean_bias,
            base_model.scope,
            base_model.min_support,
            base_model.smoothing,
            base_model.prior_weight,
        )?;
        let model = fit_bin_consequence_calibration_model(
            table,
            &fit_indices,
            fold_evidence,
            scope,
            bin_count,
            min_support,
            smoothing,
            prior_weight,
        )?;
        accumulator.add(&bin_consequence_application_stats(
            table,
            &eval_indices,
            &model,
        )?);
    }
    Ok(accumulator.into_stats())
}

fn bin_consequence_application_stats(
    table: &FeatureTable,
    indices: &[usize],
    model: &BinConsequenceCalibrationModel,
) -> Result<BinConsequenceApplicationStats> {
    let base_rows = evidence_consequence_calibrated_rows(table, indices, &model.base_model)?;
    let mut scoped_row_count = 0usize;
    let mut applied_row_count = 0usize;
    for (idx, base_row) in indices.iter().zip(&base_rows) {
        if !calibration_scope_applies(model.scope, &base_row.category) {
            continue;
        }
        scoped_row_count += 1;
        if bin_consequence_matching_stats(base_row.predicted_oracle_pass, &table.rows[*idx], model)
            .is_some()
        {
            applied_row_count += 1;
        }
    }
    let sample_count = indices.len();
    let noop_scoped_row_count = scoped_row_count.saturating_sub(applied_row_count);
    let applied_fraction = if sample_count == 0 {
        0.0
    } else {
        applied_row_count as f32 / sample_count as f32
    };
    let scoped_applied_fraction = if scoped_row_count == 0 {
        0.0
    } else {
        applied_row_count as f32 / scoped_row_count as f32
    };
    Ok(BinConsequenceApplicationStats {
        all_passed: sample_count > 0
            && scoped_row_count > 0
            && applied_row_count > 0
            && applied_row_count <= scoped_row_count,
        sample_count,
        scoped_row_count,
        applied_row_count,
        noop_scoped_row_count,
        applied_fraction,
        scoped_applied_fraction,
    })
}

#[derive(Default)]
struct BinConsequenceApplicationAccumulator {
    sample_count: usize,
    scoped_row_count: usize,
    applied_row_count: usize,
}

impl BinConsequenceApplicationAccumulator {
    fn add(&mut self, stats: &BinConsequenceApplicationStats) {
        self.sample_count += stats.sample_count;
        self.scoped_row_count += stats.scoped_row_count;
        self.applied_row_count += stats.applied_row_count;
    }

    fn into_stats(self) -> BinConsequenceApplicationStats {
        let noop_scoped_row_count = self.scoped_row_count.saturating_sub(self.applied_row_count);
        let applied_fraction = if self.sample_count == 0 {
            0.0
        } else {
            self.applied_row_count as f32 / self.sample_count as f32
        };
        let scoped_applied_fraction = if self.scoped_row_count == 0 {
            0.0
        } else {
            self.applied_row_count as f32 / self.scoped_row_count as f32
        };
        BinConsequenceApplicationStats {
            all_passed: self.sample_count > 0
                && self.scoped_row_count > 0
                && self.applied_row_count > 0
                && self.applied_row_count <= self.scoped_row_count,
            sample_count: self.sample_count,
            scoped_row_count: self.scoped_row_count,
            applied_row_count: self.applied_row_count,
            noop_scoped_row_count,
            applied_fraction,
            scoped_applied_fraction,
        }
    }
}

fn bucket_indices(table: &FeatureTable, bucket: &str) -> Vec<usize> {
    table
        .rows
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| (row.bucket == bucket).then_some(idx))
        .collect()
}

fn prediction_rows_for_indices(
    table: &FeatureTable,
    indices: &[usize],
    candidate: Candidate,
) -> Result<Vec<PredictionRow>> {
    indices
        .iter()
        .map(|idx| {
            let row = table
                .rows
                .get(*idx)
                .ok_or_else(|| anyhow!("row index out of range"))?;
            let score = score_candidate(table, *idx, candidate)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn mean_bias_rows_for_indices(
    table: &FeatureTable,
    indices: &[usize],
    model: &MeanBiasCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    let raw_rows = prediction_rows_for_indices(table, indices, model.affine_model.base_candidate)?;
    mean_bias_calibrated_rows(&raw_rows, model)
}

fn calibration_fold_feature(row: &FeatureRow) -> u8 {
    let digest = Sha256::digest(format!("{}|{}", row.task_id, row.category).as_bytes());
    digest[0] % 2
}

fn calibration_scope_applies(scope: CalibrationScope, category: &str) -> bool {
    match scope {
        CalibrationScope::Global | CalibrationScope::PerCategory => true,
        CalibrationScope::TrueMixedPerCategory => TRUE_MIXED_CATEGORIES.contains(&category),
    }
}

fn select_probability_calibration_repair(
    table: &FeatureTable,
) -> Result<ProbabilityCalibrationReport> {
    let mut best: Option<ProbabilityCalibrationReport> = None;
    for base_candidate in probability_calibration_base_candidates() {
        let calibration_base_rows =
            prediction_rows_for_candidate(table, "calibration", base_candidate)?;
        let holdout_base_rows = prediction_rows_for_candidate(table, "holdout", base_candidate)?;
        for scope in [
            CalibrationScope::Global,
            CalibrationScope::PerCategory,
            CalibrationScope::TrueMixedPerCategory,
        ] {
            for bin_count in [5_usize, 10, 15] {
                for smoothing in [0.0_f32, 1.0, 3.0, 10.0] {
                    for raw_weight in [0.0_f32, 0.25, 0.50, 0.75] {
                        let model = fit_probability_calibration_model(
                            &calibration_base_rows,
                            base_candidate,
                            scope,
                            bin_count,
                            smoothing,
                            raw_weight,
                        )?;
                        let calibration = probability_calibration_cross_split_report(
                            &calibration_base_rows,
                            base_candidate,
                            scope,
                            bin_count,
                            smoothing,
                            raw_weight,
                        )?;
                        let holdout =
                            probability_calibration_split_report(&holdout_base_rows, &model)?;
                        let unpenalized_selection_objective =
                            selection_objective(&calibration.metrics);
                        let complexity_penalty = probability_calibration_complexity_penalty(&model);
                        let selection_objective =
                            unpenalized_selection_objective + complexity_penalty;
                        let blockers =
                            repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
                        let report = ProbabilityCalibrationReport {
                            selected_on: "calibration".to_string(),
                            selection_forbidden: false,
                            forbidden_reason: None,
                            unpenalized_selection_objective,
                            complexity_penalty,
                            selection_objective,
                            model,
                            calibration,
                            holdout,
                            blockers,
                        };
                        if best
                            .as_ref()
                            .map(|previous| {
                                report.selection_objective < previous.selection_objective
                            })
                            .unwrap_or(true)
                        {
                            best = Some(report);
                        }
                    }
                }
            }
        }
    }
    best.ok_or_else(|| anyhow!("probability calibration sweep produced no candidates"))
}

fn probability_calibration_base_candidates() -> [Candidate; 6] {
    [
        Candidate::CategoryLocalKnn {
            k: 31,
            fail_boost: 2.0,
        },
        Candidate::CategoryLocalKnn {
            k: 31,
            fail_boost: 1.5,
        },
        Candidate::CategoryLocalKnn {
            k: 31,
            fail_boost: 1.25,
        },
        Candidate::CategoryLocalKnn {
            k: 121,
            fail_boost: 1.5,
        },
        Candidate::CategoryLocalRbf {
            tau: 10.0,
            fail_boost: 2.0,
        },
        CURRENT_RBF,
    ]
}

fn probability_calibration_complexity_penalty(model: &ProbabilityCalibrationModel) -> f32 {
    let scope_penalty = match model.scope {
        CalibrationScope::Global => 0.0,
        CalibrationScope::TrueMixedPerCategory => 0.015,
        CalibrationScope::PerCategory => 0.025,
    };
    let bin_penalty = (model.bin_count.saturating_sub(5) as f32) * 0.001;
    let smoothing_penalty = if model.smoothing <= f32::EPSILON {
        0.010
    } else {
        0.0
    };
    let raw_weight_penalty = if model.raw_weight < 0.25 { 0.010 } else { 0.0 };
    let mut sparse_nonempty_bins = 0usize;
    let mut nonempty_bins = 0usize;
    for (key, bins) in &model.bins_by_key {
        if key != "__global__" && !calibration_scope_applies(model.scope, key) {
            continue;
        }
        for bin in bins {
            if bin.count > 0 {
                nonempty_bins += 1;
                if bin.count < 3 {
                    sparse_nonempty_bins += 1;
                }
            }
        }
    }
    let sparse_penalty = if nonempty_bins == 0 {
        0.010
    } else {
        0.020 * sparse_nonempty_bins as f32 / nonempty_bins as f32
    };
    scope_penalty + bin_penalty + smoothing_penalty + raw_weight_penalty + sparse_penalty
}

fn fit_probability_calibration_model(
    rows: &[PredictionRow],
    base_candidate: Candidate,
    scope: CalibrationScope,
    bin_count: usize,
    smoothing: f32,
    raw_weight: f32,
) -> Result<ProbabilityCalibrationModel> {
    ensure!(
        bin_count > 1,
        "probability calibration bin_count must be > 1"
    );
    ensure!(
        (0.0..=1.0).contains(&raw_weight),
        "probability calibration raw_weight must be in [0, 1]"
    );
    ensure!(
        smoothing >= 0.0,
        "probability calibration smoothing must be non-negative"
    );
    ensure!(!rows.is_empty(), "probability calibration rows are empty");
    let global_prior = pass_rate(rows);
    let category_priors = REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            let category_rows = rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            let prior = if category_rows.is_empty() {
                global_prior
            } else {
                pass_rate(&category_rows)
            };
            ((*category).to_string(), prior)
        })
        .collect::<BTreeMap<_, _>>();
    let mut counts_by_key = BTreeMap::<String, Vec<(usize, usize)>>::from([(
        "__global__".to_string(),
        vec![(0, 0); bin_count],
    )]);
    for category in REQUIRED_CATEGORIES {
        counts_by_key
            .entry(category.to_string())
            .or_insert_with(|| vec![(0, 0); bin_count]);
    }
    for row in rows {
        let key = calibration_scope_key(scope, &row.category);
        let bin_idx = probability_bin_index(row.predicted_oracle_pass, bin_count);
        let bins = counts_by_key
            .entry(key)
            .or_insert_with(|| vec![(0, 0); bin_count]);
        bins[bin_idx].0 += 1;
        if row.actual_oracle_pass {
            bins[bin_idx].1 += 1;
        }
    }

    let mut bins_by_key = BTreeMap::new();
    for (key, counts) in counts_by_key {
        let prior = if key == "__global__" {
            global_prior
        } else {
            category_priors.get(&key).copied().unwrap_or(global_prior)
        };
        let bins = counts
            .into_iter()
            .map(|(count, positives)| {
                let calibrated_probability = if count == 0 && smoothing <= f32::EPSILON {
                    prior
                } else {
                    (positives as f32 + smoothing * prior) / (count as f32 + smoothing)
                }
                .clamp(0.001, 0.999);
                ProbabilityCalibrationBin {
                    count,
                    positives,
                    calibrated_probability,
                }
            })
            .collect::<Vec<_>>();
        bins_by_key.insert(key, bins);
    }

    Ok(ProbabilityCalibrationModel {
        selected_on: "calibration".to_string(),
        scope,
        base_candidate,
        bin_count,
        smoothing,
        raw_weight,
        global_prior,
        category_priors,
        bins_by_key,
    })
}

fn probability_calibration_split_report(
    rows: &[PredictionRow],
    model: &ProbabilityCalibrationModel,
) -> Result<SplitReport> {
    let calibrated_rows = rows
        .iter()
        .map(|row| {
            let score = apply_probability_calibration(row, model)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn probability_calibration_cross_split_report(
    rows: &[PredictionRow],
    base_candidate: Candidate,
    scope: CalibrationScope,
    bin_count: usize,
    smoothing: f32,
    raw_weight: f32,
) -> Result<SplitReport> {
    ensure!(
        rows.len() >= 2,
        "probability calibration cross split needs at least two rows"
    );
    let mut calibrated_rows = Vec::with_capacity(rows.len());
    for held_out_fold in 0..2_u8 {
        let fit_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) != held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        let eval_rows = rows
            .iter()
            .filter(|row| calibration_fold(row) == held_out_fold)
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            !fit_rows.is_empty() && !eval_rows.is_empty(),
            "probability calibration cross split produced an empty fold"
        );
        let model = fit_probability_calibration_model(
            &fit_rows,
            base_candidate,
            scope,
            bin_count,
            smoothing,
            raw_weight,
        )?;
        let split = eval_rows
            .iter()
            .map(|row| {
                let score = apply_probability_calibration(row, &model)?;
                Ok(PredictionRow {
                    task_id: row.task_id.clone(),
                    category: row.category.clone(),
                    actual_oracle_pass: row.actual_oracle_pass,
                    predicted_oracle_pass: score,
                    predicted_pass: score >= 0.5,
                    correct: (score >= 0.5) == row.actual_oracle_pass,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        calibrated_rows.extend(split);
    }
    ensure!(
        calibrated_rows.len() == rows.len(),
        "probability calibration cross split lost rows"
    );
    Ok(SplitReport {
        metrics: metrics_for(&calibrated_rows),
        per_category_metrics: per_category_metrics(&calibrated_rows),
    })
}

fn calibration_fold(row: &PredictionRow) -> u8 {
    let digest = Sha256::digest(format!("{}|{}", row.task_id, row.category).as_bytes());
    digest[0] % 2
}

fn apply_probability_calibration(
    row: &PredictionRow,
    model: &ProbabilityCalibrationModel,
) -> Result<f32> {
    let key = calibration_scope_key(model.scope, &row.category);
    let bins = model
        .bins_by_key
        .get(&key)
        .or_else(|| model.bins_by_key.get("__global__"))
        .ok_or_else(|| anyhow!("probability calibration model has no global bins"))?;
    let bin_idx = probability_bin_index(row.predicted_oracle_pass, model.bin_count);
    let calibrated = bins
        .get(bin_idx)
        .ok_or_else(|| anyhow!("probability calibration bin index out of range"))?
        .calibrated_probability;
    Ok(
        (model.raw_weight * row.predicted_oracle_pass + (1.0 - model.raw_weight) * calibrated)
            .clamp(0.001, 0.999),
    )
}

fn calibration_scope_key(scope: CalibrationScope, category: &str) -> String {
    match scope {
        CalibrationScope::Global => "__global__".to_string(),
        CalibrationScope::PerCategory => category.to_string(),
        CalibrationScope::TrueMixedPerCategory => {
            if TRUE_MIXED_CATEGORIES.contains(&category) {
                category.to_string()
            } else {
                "__global__".to_string()
            }
        }
    }
}

fn probability_bin_index(score: f32, bin_count: usize) -> usize {
    ((score.clamp(0.0, 0.999_999) * bin_count as f32).floor() as usize).min(bin_count - 1)
}

fn pass_rate(rows: &[PredictionRow]) -> f32 {
    if rows.is_empty() {
        0.5
    } else {
        rows.iter().filter(|row| row.actual_oracle_pass).count() as f32 / rows.len() as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedF32(f32);

impl Eq for OrderedF32 {}

impl PartialOrd for OrderedF32 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF32 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

fn selection_objective(metrics: &MetricBlock) -> f32 {
    let correlation_reward = metrics
        .prediction_oracle_correlation
        .map(|value| -0.10 * value)
        .unwrap_or(10.0);
    metrics.brier + SELECTION_ECE_WEIGHT * metrics.ece + correlation_reward
}

fn ece_repair_objective(metrics: &MetricBlock) -> f32 {
    let correlation_reward = metrics
        .prediction_oracle_correlation
        .map(|value| -0.02 * value)
        .unwrap_or(10.0);
    metrics.ece + 0.05 * metrics.brier + correlation_reward
}

fn repair_blockers(
    metrics: &MetricBlock,
    per_category: &BTreeMap<String, MetricBlock>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if metrics
        .prediction_oracle_correlation
        .map(|value| value < BASELINE_536_CORRELATION)
        .unwrap_or(true)
    {
        blockers.push(format!(
            "aggregate_correlation_not_above_536: {:?}",
            metrics.prediction_oracle_correlation
        ));
    }
    if metrics.brier > BASELINE_536_BRIER {
        blockers.push(format!("aggregate_brier_regressed: {:.9}", metrics.brier));
    }
    if metrics.ece > BASELINE_536_ECE {
        blockers.push(format!("aggregate_ece_regressed: {:.9}", metrics.ece));
    }
    for category in TRUE_MIXED_CATEGORIES {
        let Some(block) = per_category.get(category) else {
            blockers.push(format!("{category}:missing_metrics"));
            continue;
        };
        if block.predicted_pass_count == 0 || block.predicted_fail_count == 0 {
            blockers.push(format!(
                "{category}:threshold_degenerate_predicted_pass={} predicted_fail={}",
                block.predicted_pass_count, block.predicted_fail_count
            ));
        }
        if block
            .prediction_oracle_correlation
            .map(|value| value <= 0.0)
            .unwrap_or(true)
        {
            blockers.push(format!(
                "{category}:non_positive_correlation={:?}",
                block.prediction_oracle_correlation
            ));
        }
    }
    blockers
}

fn legal_best_candidate(
    best_global: &RankedCandidate,
    reports: &LegalCandidateReports<'_>,
) -> LegalBestCandidate {
    let mut best_kind = "calibration_selected_global_candidate";
    let mut best_objective = best_global.selection_objective;
    if reports.threshold.selection_objective < best_objective {
        best_kind = "calibration_selected_threshold_repair";
        best_objective = reports.threshold.selection_objective;
    }
    if reports.affine_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_affine_calibration";
        best_objective = reports.affine_calibration.selection_objective;
    }
    if reports.prediction_blend.selection_objective < best_objective {
        best_kind = "calibration_selected_prediction_blend";
        best_objective = reports.prediction_blend.selection_objective;
    }
    if reports.logit_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_logit_calibration";
        best_objective = reports.logit_calibration.selection_objective;
    }
    if reports.mean_bias_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_mean_bias_calibration";
        best_objective = reports.mean_bias_calibration.selection_objective;
    }
    if reports
        .cross_window_stability_calibration
        .selection_objective
        < best_objective
    {
        best_kind = "calibration_selected_cross_window_stability_calibration";
        best_objective = reports
            .cross_window_stability_calibration
            .selection_objective;
    }
    if reports.cause_residual_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_cause_residual_calibration";
        best_objective = reports.cause_residual_calibration.selection_objective;
    }
    if reports.cause_bin_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_cause_bin_calibration";
        best_objective = reports.cause_bin_calibration.selection_objective;
    }
    if !reports.score_sharpness_calibration.selection_forbidden
        && reports.score_sharpness_calibration.selection_objective < best_objective
    {
        best_kind = "calibration_selected_score_sharpness_calibration";
        best_objective = reports.score_sharpness_calibration.selection_objective;
    }
    if !reports
        .score_sharpness_logit_calibration
        .selection_forbidden
        && reports
            .score_sharpness_logit_calibration
            .selection_objective
            < best_objective
    {
        best_kind = "calibration_selected_score_sharpness_logit_calibration";
        best_objective = reports
            .score_sharpness_logit_calibration
            .selection_objective;
    }
    if reports.cause_band_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_cause_band_calibration";
        best_objective = reports.cause_band_calibration.selection_objective;
    }
    if !reports.final_bin_calibration.selection_forbidden
        && reports.final_bin_calibration.selection_objective + FINAL_BIN_LEGAL_BEST_OBJECTIVE_MARGIN
            < best_objective
    {
        best_kind = "calibration_selected_final_bin_calibration";
        best_objective = reports.final_bin_calibration.selection_objective;
    }
    if reports.support_shrink_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_support_shrink_calibration";
        best_objective = reports.support_shrink_calibration.selection_objective;
    }
    if reports.evidence_consequence_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_evidence_consequence_calibration";
        best_objective = reports.evidence_consequence_calibration.selection_objective;
    }
    if reports.bin_consequence_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_bin_consequence_calibration";
        best_objective = reports.bin_consequence_calibration.selection_objective;
    }
    if reports.probability_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_probability_calibration";
        best_objective = reports.probability_calibration.selection_objective;
    }
    let _ = best_objective;
    match best_kind {
        "calibration_selected_threshold_repair" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.threshold).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected off_by_one threshold transform by calibration objective"
                    .to_string(),
            calibration: reports.threshold.calibration.clone(),
            holdout: reports.threshold.holdout.clone(),
            blockers: reports.threshold.blockers.clone(),
        },
        "calibration_selected_affine_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.affine_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected affine score calibration by calibration objective"
                    .to_string(),
            calibration: reports.affine_calibration.calibration.clone(),
            holdout: reports.affine_calibration.holdout.clone(),
            blockers: reports.affine_calibration.blockers.clone(),
        },
        "calibration_selected_prediction_blend" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.prediction_blend).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected affine/rank-lift prediction blend by calibration objective"
                    .to_string(),
            calibration: reports.prediction_blend.calibration.clone(),
            holdout: reports.prediction_blend.holdout.clone(),
            blockers: reports.prediction_blend.blockers.clone(),
        },
        "calibration_selected_logit_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.logit_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected logit score calibration by calibration objective"
                    .to_string(),
            calibration: reports.logit_calibration.calibration.clone(),
            holdout: reports.logit_calibration.holdout.clone(),
            blockers: reports.logit_calibration.blockers.clone(),
        },
        "calibration_selected_mean_bias_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.mean_bias_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected mean-bias score calibration by out-of-fold calibration objective"
                    .to_string(),
            calibration: reports.mean_bias_calibration.calibration.clone(),
            holdout: reports.mean_bias_calibration.holdout.clone(),
            blockers: reports.mean_bias_calibration.blockers.clone(),
        },
        "calibration_selected_cross_window_stability_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.cross_window_stability_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected cross-window stable bias correction by calibration objective"
                    .to_string(),
            calibration: reports
                .cross_window_stability_calibration
                .calibration
                .clone(),
            holdout: reports.cross_window_stability_calibration.holdout.clone(),
            blockers: reports
                .cross_window_stability_calibration
                .blockers
                .clone(),
        },
        "calibration_selected_cause_residual_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.cause_residual_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected cause-residual correction by leave-one-window-out calibration objective"
                    .to_string(),
            calibration: reports.cause_residual_calibration.calibration.clone(),
            holdout: reports.cause_residual_calibration.holdout.clone(),
            blockers: reports.cause_residual_calibration.blockers.clone(),
        },
        "calibration_selected_cause_bin_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.cause_bin_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected cause-residual score-bin calibration by leave-one-window-out ECE objective"
                    .to_string(),
            calibration: reports.cause_bin_calibration.calibration.clone(),
            holdout: reports.cause_bin_calibration.holdout.clone(),
            blockers: reports.cause_bin_calibration.blockers.clone(),
        },
        "calibration_selected_score_sharpness_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.score_sharpness_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected monotone tail-shaping repair on top of cause-bin scores"
                    .to_string(),
            calibration: reports.score_sharpness_calibration.calibration.clone(),
            holdout: reports.score_sharpness_calibration.holdout.clone(),
            blockers: reports.score_sharpness_calibration.blockers.clone(),
        },
        "calibration_selected_score_sharpness_logit_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.score_sharpness_logit_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected Platt/logit repair on top of score-sharpness scores"
                    .to_string(),
            calibration: reports.score_sharpness_logit_calibration.calibration.clone(),
            holdout: reports.score_sharpness_logit_calibration.holdout.clone(),
            blockers: reports.score_sharpness_logit_calibration.blockers.clone(),
        },
        "calibration_selected_cause_band_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.cause_band_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected compact cause-band calibration over temporal, file, label, and skill deltas"
                    .to_string(),
            calibration: reports.cause_band_calibration.calibration.clone(),
            holdout: reports.cause_band_calibration.holdout.clone(),
            blockers: reports.cause_band_calibration.blockers.clone(),
        },
        "calibration_selected_final_bin_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.final_bin_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected final probability-bin calibration on top of cause-bin scores"
                    .to_string(),
            calibration: reports.final_bin_calibration.calibration.clone(),
            holdout: reports.final_bin_calibration.holdout.clone(),
            blockers: reports.final_bin_calibration.blockers.clone(),
        },
        "calibration_selected_support_shrink_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.support_shrink_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected support-aware prior shrinkage by out-of-fold calibration objective"
                    .to_string(),
            calibration: reports.support_shrink_calibration.calibration.clone(),
            holdout: reports.support_shrink_calibration.holdout.clone(),
            blockers: reports.support_shrink_calibration.blockers.clone(),
        },
        "calibration_selected_evidence_consequence_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.evidence_consequence_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected consequence baseline over explicit constellation evidence keys"
                    .to_string(),
            calibration: reports.evidence_consequence_calibration.calibration.clone(),
            holdout: reports.evidence_consequence_calibration.holdout.clone(),
            blockers: reports.evidence_consequence_calibration.blockers.clone(),
        },
        "calibration_selected_bin_consequence_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.bin_consequence_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected bin-aware consequence baseline over explicit evidence keys"
                    .to_string(),
            calibration: reports.bin_consequence_calibration.calibration.clone(),
            holdout: reports.bin_consequence_calibration.holdout.clone(),
            blockers: reports.bin_consequence_calibration.blockers.clone(),
        },
        "calibration_selected_probability_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(reports.probability_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected probability calibration map by calibration objective"
                    .to_string(),
            calibration: reports.probability_calibration.calibration.clone(),
            holdout: reports.probability_calibration.holdout.clone(),
            blockers: reports.probability_calibration.blockers.clone(),
        },
        _ => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(best_global).unwrap_or(Value::Null),
            selection_reason:
                "best global RBF/KNN candidate by calibration objective, with no holdout-selected threshold"
                    .to_string(),
            calibration: best_global.calibration.clone(),
            holdout: best_global.holdout.clone(),
            blockers: best_global.blockers.clone(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn legal_best_holdout_rows(
    table: &FeatureTable,
    legal_best: &LegalBestCandidate,
    best_global: &RankedCandidate,
    legal_threshold: &ThresholdRepairReport,
    legal_affine_calibration: &AffineCalibrationReport,
    legal_prediction_blend: &PredictionBlendReport,
    score_traces: &ScoreTraceRows,
    legal_logit_calibration: &LogitCalibrationReport,
    legal_mean_bias_calibration: &MeanBiasCalibrationReport,
    legal_cross_window_stability_calibration: &CrossWindowStabilityCalibrationReport,
    legal_cause_residual_calibration: &CauseResidualCalibrationReport,
    legal_cause_bin_calibration: &CauseBinCalibrationReport,
    legal_score_sharpness_calibration: &ScoreSharpnessCalibrationReport,
    legal_score_sharpness_logit_calibration: &ScoreSharpnessLogitCalibrationReport,
    legal_cause_band_calibration: &CauseBandCalibrationReport,
    legal_final_bin_calibration: &FinalBinCalibrationReport,
    legal_support_shrink_calibration: &SupportShrinkCalibrationReport,
    legal_evidence_consequence_calibration: &EvidenceConsequenceCalibrationReport,
    legal_bin_consequence_calibration: &BinConsequenceCalibrationReport,
    legal_probability_calibration: &ProbabilityCalibrationReport,
) -> Result<Vec<PredictionRow>> {
    match legal_best.kind.as_str() {
        "calibration_selected_threshold_repair" => threshold_prediction_rows(
            table,
            "holdout",
            legal_threshold.category.as_str(),
            legal_threshold.base_candidate,
            legal_threshold.probe_candidate,
            legal_threshold.threshold,
            legal_threshold.low,
            legal_threshold.high,
        ),
        "calibration_selected_affine_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_affine_calibration.model.base_candidate,
            )?;
            Ok(apply_affine_rows(&rows, &legal_affine_calibration.model))
        }
        "calibration_selected_prediction_blend" => {
            let base_rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_prediction_blend.model.affine_model.base_candidate,
            )?;
            prediction_blend_rows(
                &base_rows,
                &score_traces.holdout_rank_lift,
                &legal_prediction_blend.model,
            )
        }
        "calibration_selected_logit_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_logit_calibration.model.base_candidate,
            )?;
            logit_calibrated_rows(&rows, &legal_logit_calibration.model)
        }
        "calibration_selected_mean_bias_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_mean_bias_calibration
                    .model
                    .affine_model
                    .base_candidate,
            )?;
            mean_bias_calibrated_rows(&rows, &legal_mean_bias_calibration.model)
        }
        "calibration_selected_cross_window_stability_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_cross_window_stability_calibration
                    .model
                    .affine_model
                    .base_candidate,
            )?;
            cross_window_stability_calibrated_rows(
                &rows,
                &legal_cross_window_stability_calibration.model,
            )
        }
        "calibration_selected_cause_residual_calibration" => cause_residual_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_cause_residual_calibration.model,
        ),
        "calibration_selected_cause_bin_calibration" => cause_bin_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_cause_bin_calibration.model,
        ),
        "calibration_selected_score_sharpness_calibration" => score_sharpness_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_score_sharpness_calibration.model,
        ),
        "calibration_selected_score_sharpness_logit_calibration" => {
            score_sharpness_logit_calibrated_rows(
                table,
                &bucket_indices(table, "holdout"),
                &legal_score_sharpness_logit_calibration.model,
            )
        }
        "calibration_selected_cause_band_calibration" => cause_band_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_cause_band_calibration.model,
        ),
        "calibration_selected_final_bin_calibration" => final_bin_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_final_bin_calibration.model,
        ),
        "calibration_selected_support_shrink_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_support_shrink_calibration
                    .model
                    .base_model
                    .affine_model
                    .base_candidate,
            )?;
            support_shrink_calibrated_rows(&rows, &legal_support_shrink_calibration.model)
        }
        "calibration_selected_evidence_consequence_calibration" => {
            evidence_consequence_calibrated_rows(
                table,
                &bucket_indices(table, "holdout"),
                &legal_evidence_consequence_calibration.model,
            )
        }
        "calibration_selected_bin_consequence_calibration" => bin_consequence_calibrated_rows(
            table,
            &bucket_indices(table, "holdout"),
            &legal_bin_consequence_calibration.model,
        ),
        "calibration_selected_probability_calibration" => {
            let rows = prediction_rows_for_candidate(
                table,
                "holdout",
                legal_probability_calibration.model.base_candidate,
            )?;
            probability_calibrated_rows(&rows, &legal_probability_calibration.model)
        }
        "calibration_selected_global_candidate" => {
            prediction_rows_for_candidate(table, "holdout", best_global.candidate)
        }
        other => Err(anyhow!("unknown legal best candidate kind {other}")),
    }
}

fn threshold_prediction_rows(
    table: &FeatureTable,
    bucket: &str,
    category: &str,
    base_candidate: Candidate,
    probe_candidate: Candidate,
    threshold: f32,
    low: f32,
    high: f32,
) -> Result<Vec<PredictionRow>> {
    table
        .rows
        .iter()
        .enumerate()
        .filter(|(_idx, row)| row.bucket == bucket)
        .map(|(idx, row)| {
            let score = if row.category == category {
                let probe = score_candidate(table, idx, probe_candidate)?;
                if probe >= threshold {
                    high
                } else {
                    low
                }
            } else {
                score_candidate(table, idx, base_candidate)?
            };
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn apply_affine_rows(rows: &[PredictionRow], model: &AffineCalibrationModel) -> Vec<PredictionRow> {
    rows.iter()
        .map(|row| {
            let score = apply_affine_calibration(row, model);
            PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            }
        })
        .collect()
}

fn prediction_blend_rows(
    base_rows: &[PredictionRow],
    rank_lift_rows: &[PredictionRow],
    model: &PredictionBlendModel,
) -> Result<Vec<PredictionRow>> {
    ensure_prediction_rows_aligned(base_rows, rank_lift_rows, "selected legal prediction blend")?;
    Ok(base_rows
        .iter()
        .zip(rank_lift_rows)
        .map(|(base, rank_lift)| {
            let affine_score = apply_affine_calibration(base, &model.affine_model);
            let score = (model.affine_weight * affine_score
                + model.rank_lift_weight * rank_lift.predicted_oracle_pass)
                .clamp(0.001, 0.999);
            PredictionRow {
                task_id: base.task_id.clone(),
                category: base.category.clone(),
                actual_oracle_pass: base.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == base.actual_oracle_pass,
            }
        })
        .collect())
}

fn logit_calibrated_rows(
    rows: &[PredictionRow],
    model: &LogitCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = apply_logit_calibration(row, model);
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn probability_calibrated_rows(
    rows: &[PredictionRow],
    model: &ProbabilityCalibrationModel,
) -> Result<Vec<PredictionRow>> {
    rows.iter()
        .map(|row| {
            let score = apply_probability_calibration(row, model)?;
            Ok(PredictionRow {
                task_id: row.task_id.clone(),
                category: row.category.clone(),
                actual_oracle_pass: row.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == row.actual_oracle_pass,
            })
        })
        .collect()
}

fn legal_best_row_audit(
    table: &FeatureTable,
    legal_best: &LegalBestCandidate,
    rows: &[PredictionRow],
    reproduction: &ReproductionReport,
) -> LegalBestRowAudit {
    split_row_audit(table, "holdout", &legal_best.holdout, rows, reproduction)
}

fn legal_best_holdout_row_diagnostics(
    table: &FeatureTable,
    legal_best_rows: &[PredictionRow],
    score_sharpness_model: &ScoreSharpnessCalibrationModel,
) -> Result<Vec<LegalBestRowDiagnostic>> {
    let holdout_indices = bucket_indices(table, "holdout");
    ensure!(
        holdout_indices.len() == legal_best_rows.len(),
        "legal best diagnostics holdout index count mismatch"
    );
    let cause_bin_rows =
        cause_bin_calibrated_rows(table, &holdout_indices, &score_sharpness_model.base_model)?;
    let score_sharpness_rows =
        score_sharpness_calibrated_rows(table, &holdout_indices, score_sharpness_model)?;
    ensure_prediction_rows_aligned(legal_best_rows, &score_sharpness_rows, "row diagnostics")?;
    ensure_prediction_rows_aligned(
        &cause_bin_rows,
        &score_sharpness_rows,
        "row diagnostics cause",
    )?;

    holdout_indices
        .iter()
        .copied()
        .zip(legal_best_rows.iter())
        .zip(cause_bin_rows.iter())
        .zip(score_sharpness_rows.iter())
        .map(
            |(((row_index, legal_best_row), cause_bin_row), score_sharpness_row)| {
                let feature_row = table
                    .rows
                    .get(row_index)
                    .ok_or_else(|| anyhow!("row diagnostic index out of range"))?;
                ensure!(
                    feature_row.task_id == legal_best_row.task_id
                        && feature_row.category == legal_best_row.category
                        && feature_row.actual_oracle_pass == legal_best_row.actual_oracle_pass,
                    "row diagnostic identity mismatch for {}:{}",
                    legal_best_row.task_id,
                    legal_best_row.category
                );
                let target = if legal_best_row.actual_oracle_pass {
                    1.0
                } else {
                    0.0
                };
                Ok(LegalBestRowDiagnostic {
                    row_index,
                    task_id: legal_best_row.task_id.clone(),
                    category: legal_best_row.category.clone(),
                    repo: feature_row.repo.clone(),
                    patch_path: feature_row.patch_path.clone(),
                    actual_oracle_pass: legal_best_row.actual_oracle_pass,
                    predicted_oracle_pass: legal_best_row.predicted_oracle_pass,
                    predicted_pass: legal_best_row.predicted_pass,
                    correct: legal_best_row.correct,
                    absolute_error: (legal_best_row.predicted_oracle_pass - target).abs(),
                    ece_bin_index: ece_bin_index(legal_best_row.predicted_oracle_pass),
                    current_rbf_score: score_candidate(table, row_index, CURRENT_RBF)?,
                    selected_knn_score: score_candidate(
                        table,
                        row_index,
                        Candidate::CategoryLocalKnn {
                            k: 31,
                            fail_boost: 1.5,
                        },
                    )?,
                    cause_bin_score: cause_bin_row.predicted_oracle_pass,
                    score_sharpness_score: score_sharpness_row.predicted_oracle_pass,
                    evidence_key_count: feature_row.evidence_keys.len(),
                    evidence_key_sample: feature_row
                        .evidence_keys
                        .iter()
                        .take(16)
                        .cloned()
                        .collect(),
                    cause_feature_nonzero_count: feature_row
                        .cause_features
                        .iter()
                        .filter(|value| value.abs() > f32::EPSILON)
                        .count(),
                    cause_feature_large_magnitude_count: feature_row
                        .cause_features
                        .iter()
                        .filter(|value| value.abs() >= 1.0)
                        .count(),
                })
            },
        )
        .collect()
}

fn split_row_audit(
    table: &FeatureTable,
    bucket: &str,
    split: &SplitReport,
    rows: &[PredictionRow],
    reproduction: &ReproductionReport,
) -> LegalBestRowAudit {
    let expected_sample_count = split.metrics.sample_count;
    let actual_sample_count = rows.len();
    let expected_row_identity_sha256 = feature_row_identity_hash(table, bucket);
    let actual_row_identity_sha256 = prediction_row_identity_hash(rows);
    let expected_per_category_counts = split
        .per_category_metrics
        .iter()
        .map(|(category, metrics)| (category.clone(), metrics.sample_count))
        .collect::<BTreeMap<_, _>>();
    let actual_per_category_counts = prediction_row_counts_by_category(rows);
    let tight_tolerance = 1.0e-6_f32;
    let exact_metric_reproduction_passed = reproduction.brier_delta_abs <= tight_tolerance
        && reproduction.ece_delta_abs <= tight_tolerance
        && reproduction
            .correlation_delta_abs
            .map(|delta| delta <= tight_tolerance)
            .unwrap_or(true);
    let sample_count_matches = expected_sample_count == actual_sample_count;
    let row_identity_matches = expected_row_identity_sha256 == actual_row_identity_sha256;
    let per_category_counts_match = expected_per_category_counts == actual_per_category_counts;
    LegalBestRowAudit {
        all_passed: reproduction.passed
            && sample_count_matches
            && row_identity_matches
            && per_category_counts_match
            && exact_metric_reproduction_passed,
        expected_sample_count,
        actual_sample_count,
        sample_count_matches,
        expected_row_identity_sha256,
        actual_row_identity_sha256,
        row_identity_matches,
        expected_per_category_counts,
        actual_per_category_counts,
        per_category_counts_match,
        exact_metric_reproduction_passed,
        tight_tolerance,
    }
}

fn feature_row_identity_hash(table: &FeatureTable, bucket: &str) -> String {
    let parts = table
        .rows
        .iter()
        .filter(|row| row.bucket == bucket)
        .map(|row| format!("{}:{}", row.task_id, row.category))
        .collect::<Vec<_>>();
    identity_hash(&parts)
}

fn prediction_row_identity_hash(rows: &[PredictionRow]) -> String {
    let parts = rows
        .iter()
        .map(|row| format!("{}:{}", row.task_id, row.category))
        .collect::<Vec<_>>();
    identity_hash(&parts)
}

fn identity_hash(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn prediction_row_counts_by_category(rows: &[PredictionRow]) -> BTreeMap<String, usize> {
    REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            (
                (*category).to_string(),
                rows.iter().filter(|row| row.category == *category).count(),
            )
        })
        .collect()
}

struct ScoreTraceRows {
    calibration_raw: Vec<PredictionRow>,
    calibration_rank_safe: Vec<PredictionRow>,
    calibration_rank_lift: Vec<PredictionRow>,
    holdout_raw: Vec<PredictionRow>,
    holdout_rank_safe: Vec<PredictionRow>,
    holdout_rank_lift: Vec<PredictionRow>,
}

fn score_fusion_report(report: &Value) -> Result<ScoreFusionReport> {
    let traces = score_trace_rows(report)?;
    let mut candidates = score_fusion_candidates(&traces)?;
    ensure!(
        !candidates.is_empty(),
        "score-fusion candidate grid returned no candidates"
    );
    let candidate_count = candidates.len();

    candidates.sort_by(|left, right| {
        left.selection_objective
            .partial_cmp(&right.selection_objective)
            .unwrap_or(Ordering::Equal)
    });
    for (idx, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = idx + 1;
    }
    let best_legal_candidate = candidates[0].clone();
    let mut top_calibration_selected_candidates = candidates.clone();
    top_calibration_selected_candidates.truncate(12);

    let mut holdout_ranked = candidates.clone();
    holdout_ranked.sort_by(|left, right| {
        left.holdout_objective
            .partial_cmp(&right.holdout_objective)
            .unwrap_or(Ordering::Equal)
    });
    for (idx, candidate) in holdout_ranked.iter_mut().enumerate() {
        candidate.rank = idx + 1;
    }
    let forbidden_holdout_oracle = ForbiddenScoreFusionOracle {
        selected_on: "holdout".to_string(),
        selection_forbidden: true,
        forbidden_reason: "score_fusion_weights_selected_from_holdout_scores".to_string(),
        candidate: holdout_ranked[0].clone(),
    };
    let edge_cases = score_fusion_edge_case_audit(report, &traces, &forbidden_holdout_oracle)?;

    Ok(ScoreFusionReport {
        row_trace_source_path: BASELINE_REPORT_PATH.to_string(),
        required_row_trace_fields: SCORE_TRACE_FIELDS.to_vec(),
        row_trace_counts: BTreeMap::from([
            (
                "calibration_predictions".to_string(),
                traces.calibration_raw.len(),
            ),
            (
                "calibration_rank_safe_predictions".to_string(),
                traces.calibration_rank_safe.len(),
            ),
            (
                "calibration_rank_lift_predictions".to_string(),
                traces.calibration_rank_lift.len(),
            ),
            (
                "holdout_raw_predictions".to_string(),
                traces.holdout_raw.len(),
            ),
            (
                "holdout_rank_safe_predictions".to_string(),
                traces.holdout_rank_safe.len(),
            ),
            (
                "holdout_predictions".to_string(),
                traces.holdout_rank_lift.len(),
            ),
        ]),
        candidate_count,
        best_legal_candidate,
        top_calibration_selected_candidates,
        forbidden_holdout_oracle,
        edge_cases,
    })
}

fn score_trace_rows(report: &Value) -> Result<ScoreTraceRows> {
    let traces = ScoreTraceRows {
        calibration_raw: prediction_rows_from_report(report, "calibration_predictions")?,
        calibration_rank_safe: prediction_rows_from_report(
            report,
            "calibration_rank_safe_predictions",
        )?,
        calibration_rank_lift: prediction_rows_from_report(
            report,
            "calibration_rank_lift_predictions",
        )?,
        holdout_raw: prediction_rows_from_report(report, "holdout_raw_predictions")?,
        holdout_rank_safe: prediction_rows_from_report(report, "holdout_rank_safe_predictions")?,
        holdout_rank_lift: prediction_rows_from_report(report, "holdout_predictions")?,
    };
    ensure!(
        traces.calibration_raw.len() == REQUIRED_EVAL_SPLIT_COUNT
            && traces.calibration_rank_safe.len() == REQUIRED_EVAL_SPLIT_COUNT
            && traces.calibration_rank_lift.len() == REQUIRED_EVAL_SPLIT_COUNT
            && traces.holdout_raw.len() == REQUIRED_EVAL_SPLIT_COUNT
            && traces.holdout_rank_safe.len() == REQUIRED_EVAL_SPLIT_COUNT
            && traces.holdout_rank_lift.len() == REQUIRED_EVAL_SPLIT_COUNT,
        "score trace row counts must all be {REQUIRED_EVAL_SPLIT_COUNT}"
    );
    ensure_prediction_rows_aligned(
        &traces.calibration_raw,
        &traces.calibration_rank_safe,
        "calibration raw/rank-safe",
    )?;
    ensure_prediction_rows_aligned(
        &traces.calibration_raw,
        &traces.calibration_rank_lift,
        "calibration raw/rank-lift",
    )?;
    ensure_prediction_rows_aligned(
        &traces.holdout_raw,
        &traces.holdout_rank_safe,
        "holdout raw/rank-safe",
    )?;
    ensure_prediction_rows_aligned(
        &traces.holdout_raw,
        &traces.holdout_rank_lift,
        "holdout raw/rank-lift",
    )?;
    Ok(traces)
}

fn prediction_rows_from_report(report: &Value, key: &str) -> Result<Vec<PredictionRow>> {
    let value = report
        .get("execute_and_inspect")
        .and_then(|section| section.get(key))
        .ok_or_else(|| anyhow!("missing score trace field execute_and_inspect.{key}"))?;
    let rows = serde_json::from_value::<Vec<PredictionRow>>(value.clone())
        .with_context(|| format!("parse execute_and_inspect.{key}"))?;
    ensure!(!rows.is_empty(), "score trace field {key} is empty");
    Ok(rows)
}

fn ensure_prediction_rows_aligned(
    left: &[PredictionRow],
    right: &[PredictionRow],
    label: &str,
) -> Result<()> {
    ensure!(
        left.len() == right.len(),
        "{label} row counts differ: {} vs {}",
        left.len(),
        right.len()
    );
    for (idx, (left, right)) in left.iter().zip(right).enumerate() {
        ensure!(
            left.task_id == right.task_id
                && left.category == right.category
                && left.actual_oracle_pass == right.actual_oracle_pass,
            "{label} row {idx} identity mismatch: {}:{} vs {}:{}",
            left.task_id,
            left.category,
            right.task_id,
            right.category
        );
    }
    Ok(())
}

fn score_fusion_candidates(traces: &ScoreTraceRows) -> Result<Vec<ScoreFusionCandidate>> {
    let mut candidates = Vec::new();
    let default = ScoreWeights {
        raw: 0.0,
        rank_safe: 0.0,
        rank_lift: 1.0,
    };
    let grid = score_weight_grid();

    for weights in &grid {
        candidates.push(score_fusion_candidate(
            "global_weight_blend".to_string(),
            *weights,
            BTreeMap::new(),
            traces,
        )?);
    }

    for category in TRUE_MIXED_CATEGORIES {
        for weights in &grid {
            if is_rank_lift_only(*weights) {
                continue;
            }
            candidates.push(score_fusion_candidate(
                format!("one_true_mixed_category_override:{category}"),
                default,
                BTreeMap::from([(category.to_string(), *weights)]),
                traces,
            )?);
        }
    }

    for off_by_one in &grid {
        for subtle_flip in &grid {
            for swap_variable in &grid {
                candidates.push(score_fusion_candidate(
                    "all_true_mixed_category_overrides".to_string(),
                    default,
                    BTreeMap::from([
                        ("off_by_one".to_string(), *off_by_one),
                        ("subtle_flip".to_string(), *subtle_flip),
                        ("swap_variable".to_string(), *swap_variable),
                    ]),
                    traces,
                )?);
            }
        }
    }
    Ok(candidates)
}

fn score_fusion_candidate(
    policy: String,
    default_weights: ScoreWeights,
    category_overrides: BTreeMap<String, ScoreWeights>,
    traces: &ScoreTraceRows,
) -> Result<ScoreFusionCandidate> {
    let calibration = score_fusion_split_report(
        &traces.calibration_raw,
        &traces.calibration_rank_safe,
        &traces.calibration_rank_lift,
        default_weights,
        &category_overrides,
    )?;
    let holdout = score_fusion_split_report(
        &traces.holdout_raw,
        &traces.holdout_rank_safe,
        &traces.holdout_rank_lift,
        default_weights,
        &category_overrides,
    )?;
    let calibration_objective = selection_objective(&calibration.metrics);
    let holdout_objective = selection_objective(&holdout.metrics);
    let blockers = repair_blockers(&holdout.metrics, &holdout.per_category_metrics);
    Ok(ScoreFusionCandidate {
        rank: 0,
        policy,
        default_weights,
        category_overrides,
        selection_objective: calibration_objective,
        holdout_objective,
        calibration,
        holdout,
        blockers,
    })
}

fn score_fusion_split_report(
    raw_rows: &[PredictionRow],
    rank_safe_rows: &[PredictionRow],
    rank_lift_rows: &[PredictionRow],
    default_weights: ScoreWeights,
    category_overrides: &BTreeMap<String, ScoreWeights>,
) -> Result<SplitReport> {
    ensure_prediction_rows_aligned(raw_rows, rank_safe_rows, "score fusion raw/rank-safe")?;
    ensure_prediction_rows_aligned(raw_rows, rank_lift_rows, "score fusion raw/rank-lift")?;
    let rows = raw_rows
        .iter()
        .zip(rank_safe_rows)
        .zip(rank_lift_rows)
        .map(|((raw, rank_safe), rank_lift)| {
            let weights = category_overrides
                .get(&raw.category)
                .copied()
                .unwrap_or(default_weights);
            let score = (weights.raw * raw.predicted_oracle_pass
                + weights.rank_safe * rank_safe.predicted_oracle_pass
                + weights.rank_lift * rank_lift.predicted_oracle_pass)
                .clamp(0.001, 0.999);
            PredictionRow {
                task_id: raw.task_id.clone(),
                category: raw.category.clone(),
                actual_oracle_pass: raw.actual_oracle_pass,
                predicted_oracle_pass: score,
                predicted_pass: score >= 0.5,
                correct: (score >= 0.5) == raw.actual_oracle_pass,
            }
        })
        .collect::<Vec<_>>();
    Ok(SplitReport {
        metrics: metrics_for(&rows),
        per_category_metrics: per_category_metrics(&rows),
    })
}

fn score_weight_grid() -> Vec<ScoreWeights> {
    vec![
        ScoreWeights {
            raw: 0.0,
            rank_safe: 0.0,
            rank_lift: 1.0,
        },
        ScoreWeights {
            raw: 1.0,
            rank_safe: 0.0,
            rank_lift: 0.0,
        },
        ScoreWeights {
            raw: 0.0,
            rank_safe: 1.0,
            rank_lift: 0.0,
        },
        ScoreWeights {
            raw: 0.25,
            rank_safe: 0.0,
            rank_lift: 0.75,
        },
        ScoreWeights {
            raw: 0.0,
            rank_safe: 0.25,
            rank_lift: 0.75,
        },
        ScoreWeights {
            raw: 0.5,
            rank_safe: 0.0,
            rank_lift: 0.5,
        },
        ScoreWeights {
            raw: 0.0,
            rank_safe: 0.5,
            rank_lift: 0.5,
        },
        ScoreWeights {
            raw: 0.25,
            rank_safe: 0.25,
            rank_lift: 0.5,
        },
        ScoreWeights {
            raw: 0.5,
            rank_safe: 0.25,
            rank_lift: 0.25,
        },
        ScoreWeights {
            raw: 0.25,
            rank_safe: 0.5,
            rank_lift: 0.25,
        },
    ]
}

fn is_rank_lift_only(weights: ScoreWeights) -> bool {
    weights.raw.abs() < f32::EPSILON
        && weights.rank_safe.abs() < f32::EPSILON
        && (weights.rank_lift - 1.0).abs() < f32::EPSILON
}

fn score_fusion_edge_case_audit(
    report: &Value,
    traces: &ScoreTraceRows,
    forbidden: &ForbiddenScoreFusionOracle,
) -> Result<ScoreFusionEdgeCases> {
    let missing_row_trace_fails_closed = prediction_rows_from_report(
        &json!({"execute_and_inspect": {}}),
        "holdout_raw_predictions",
    )
    .is_err();
    let mismatched_row_identity_fails_closed = {
        let mut mismatched = traces.holdout_rank_safe.clone();
        if let Some(row) = mismatched.first_mut() {
            row.task_id.push_str("__mismatch");
        }
        ensure_prediction_rows_aligned(&traces.holdout_raw, &mismatched, "mismatch edge case")
            .is_err()
    };
    let holdout_weight_selection_flagged_forbidden = forbidden.selection_forbidden
        && forbidden.forbidden_reason == "score_fusion_weights_selected_from_holdout_scores";
    let source_still_parses = score_trace_rows(report).is_ok();
    let all_passed = missing_row_trace_fails_closed
        && mismatched_row_identity_fails_closed
        && holdout_weight_selection_flagged_forbidden
        && source_still_parses;
    Ok(ScoreFusionEdgeCases {
        all_passed,
        missing_row_trace_fails_closed,
        mismatched_row_identity_fails_closed,
        holdout_weight_selection_flagged_forbidden,
    })
}

fn typed_blocker(
    legal_best: &LegalBestCandidate,
    forbidden_threshold: &ThresholdRepairReport,
) -> TypedBlocker {
    let accepted = !legal_best.blockers.is_empty() && forbidden_threshold.selection_forbidden;
    let blocker_summary = if legal_best.blockers.is_empty() {
        "no blockers".to_string()
    } else {
        legal_best.blockers.join("; ")
    };
    let non_ece_blocker_count = legal_best
        .blockers
        .iter()
        .filter(|blocker| !blocker.starts_with("aggregate_ece_regressed"))
        .count();
    let code = if legal_best.blockers.is_empty() {
        "TEMPORAL_FILE_SKILL_TRAJECTORY_CLEAR"
    } else if non_ece_blocker_count == 0 {
        "TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED"
    } else {
        "TEMPORAL_FILE_SKILL_TRAJECTORY_INSUFFICIENT"
    };
    TypedBlocker {
        code: code.to_string(),
        accepted,
        why: format!(
            "The ordered file-path plus skill/ability trajectory surface is replayable and split-legal, and it produces real correlation/Brier lift, but the calibration-selected candidate still cannot pass the promotion gate because its blocker list is non-empty: {blocker_summary}. This is a valid #392 diagnostic slice, not a promotion."
        ),
        evidence: vec![
            format!("trajectory_legal_best_kind={}", legal_best.kind),
            format!("trajectory_legal_best_blockers={}", legal_best.blockers.join(";")),
            format!(
                "forbidden_holdout_corr={:?}",
                forbidden_threshold.holdout.metrics.prediction_oracle_correlation
            ),
            format!("forbidden_holdout_brier={:.9}", forbidden_threshold.holdout.metrics.brier),
        ],
        missing_signal: vec![
            "split-legal calibration that preserves the strongest file/skill trajectory lift without ECE regression".to_string(),
            "swap_variable-specific skill/ability evidence that creates both predicted pass and predicted fail support without increasing aggregate ECE".to_string(),
            "larger calibration support or cross-window validation for sparse mixed failure modes".to_string(),
            "per-cell or per-constellation baseline families beyond a single aggregate threshold".to_string(),
        ],
        next_work: vec![
            "Do not promote this trajectory surface as a ship-gate repair unless its blocker list is empty.".to_string(),
            "Do not tune more thresholds on #536 holdout.".to_string(),
            "Advance split-legal ECE repair, swap_variable skill specialization, or cross-window validation before rerunning the ship gate.".to_string(),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn edge_case_audit(
    table: &FeatureTable,
    forbidden_threshold: &ThresholdRepairReport,
    legal_affine_calibration: &AffineCalibrationReport,
    legal_support_shrink_calibration: &SupportShrinkCalibrationReport,
    legal_evidence_consequence_calibration: &EvidenceConsequenceCalibrationReport,
    legal_bin_consequence_calibration: &BinConsequenceCalibrationReport,
    legal_cause_residual_calibration: &CauseResidualCalibrationReport,
    legal_cause_bin_calibration: &CauseBinCalibrationReport,
    legal_score_sharpness_calibration: &ScoreSharpnessCalibrationReport,
    legal_cause_band_calibration: &CauseBandCalibrationReport,
) -> Result<EdgeCaseAudit> {
    let ordered_replay_passed = {
        let base = synthetic_code_state("edge-order", "base", "known_good", true);
        let target = synthetic_code_state("edge-order", "mutation_off_by_one", "off_by_one", false);
        let test_patch = synthetic_code_state("edge-order", "test_patch", "known_good", true);
        ensure_ordered_states(&base, &target, &test_patch).is_ok()
            && trajectory_feature_vector(&base, &target, &test_patch).is_ok()
    };
    let non_monotonic_order_fails_closed = {
        let base = synthetic_code_state("edge-order", "base", "known_good", true);
        let target = synthetic_code_state("edge-order", "mutation_off_by_one", "off_by_one", false);
        let test_patch = synthetic_code_state("edge-order", "test_patch", "known_good", true);
        ensure_ordered_states(&target, &base, &test_patch).is_err()
    };
    let missing_code_state_row_fails_closed = {
        let mut entries = table
            .rows
            .iter()
            .map(|row| CorpusEntry {
                bucket: row.bucket.clone(),
                category: row.category.clone(),
                oracle_all_passed: row.actual_oracle_pass,
                patch_path: row.patch_path.clone(),
                repo: row.repo.clone(),
                task_id: row.task_id.clone(),
            })
            .collect::<Vec<_>>();
        entries.push(CorpusEntry {
            bucket: "holdout".to_string(),
            category: "off_by_one".to_string(),
            oracle_all_passed: true,
            patch_path: "mutations/missing-code-state-row/known_good.patch".to_string(),
            repo: "missing/repo".to_string(),
            task_id: "missing_code_state_row_edge".to_string(),
        });
        load_feature_table(
            &entries,
            FeatureMode::FileSkillTrajectory(SkillFeaturePolicy::UsefulnessCombined),
            true,
        )
        .is_err()
    };
    let missing_file_path_row_fails_closed = {
        let empty = BTreeMap::new();
        file_path_trajectory_feature_vector(&empty, "missing-file-task", "mutation_off_by_one")
            .is_err()
    };
    let target_label_prefixes_excluded = {
        let mut hashed = vec![0.0_f32; HASH_FEATURES];
        let counts = BTreeMap::from([
            ("oracle:pass".to_string(), 99),
            ("docker:passed".to_string(), 99),
            ("live:allowed".to_string(), 3),
        ]);
        add_count_features(&mut hashed, "group", &counts, 3.0);
        let target_idx = hash_index("group:oracle:pass", HASH_FEATURES);
        let live_idx = hash_index("group:live:allowed", HASH_FEATURES);
        hashed[target_idx] == 0.0 && hashed[live_idx] > 0.0
    };
    let holdout_threshold_flagged_forbidden = forbidden_threshold.selection_forbidden
        && forbidden_threshold.forbidden_reason.as_deref()
            == Some("threshold_selected_from_holdout_scores");
    let hierarchy_edge_entries = vec![CorpusEntry {
        bucket: "train".to_string(),
        category: "known_good".to_string(),
        oracle_all_passed: true,
        patch_path: "synthetic/hierarchy-edge.patch".to_string(),
        repo: "synthetic/repo".to_string(),
        task_id: "hierarchy_edge".to_string(),
    }];
    let hierarchy_db_readback_passed =
        load_skill_feature_index(&BTreeMap::new(), &hierarchy_edge_entries)
            .map(|index| {
                index.level2_skill_count > 0
                    && index.membership_count > 0
                    && !index.by_code_state.is_empty()
                    && !index.hierarchy_report_sha256.is_empty()
            })
            .unwrap_or(false);
    let affine_selected_on_calibration = legal_affine_calibration.selected_on == "calibration"
        && !legal_affine_calibration.selection_forbidden
        && legal_affine_calibration.forbidden_reason.is_none();
    let affine_true_mixed_scope_leaves_non_mixed_rows_unchanged = {
        let row = synthetic_prediction_row("affine-known-good", "known_good", true, 0.42);
        let model = AffineCalibrationModel {
            selected_on: "calibration".to_string(),
            scope: CalibrationScope::TrueMixedPerCategory,
            base_candidate: CURRENT_RBF,
            scale: 1.30,
            offset: 0.08,
        };
        (apply_affine_calibration(&row, &model) - row.predicted_oracle_pass).abs() < 1.0e-6
    };
    let affine_clamp_passed = {
        let high_row = synthetic_prediction_row("affine-high", "off_by_one", true, 0.99);
        let low_row = synthetic_prediction_row("affine-low", "off_by_one", false, 0.01);
        let high_model = AffineCalibrationModel {
            selected_on: "calibration".to_string(),
            scope: CalibrationScope::TrueMixedPerCategory,
            base_candidate: CURRENT_RBF,
            scale: 2.0,
            offset: 0.50,
        };
        let low_model = AffineCalibrationModel {
            selected_on: "calibration".to_string(),
            scope: CalibrationScope::TrueMixedPerCategory,
            base_candidate: CURRENT_RBF,
            scale: 2.0,
            offset: -0.50,
        };
        (apply_affine_calibration(&high_row, &high_model) - 0.999).abs() < 1.0e-6
            && (apply_affine_calibration(&low_row, &low_model) - 0.001).abs() < 1.0e-6
    };
    let missing_slot_scalar_fails_closed = {
        let mut missing_slot = synthetic_code_state("missing-slot", "base", "known_good", true);
        missing_slot.slot_feature_means.remove(SLOT_IDS[0]);
        let mut missing_scalar = synthetic_code_state("missing-scalar", "base", "known_good", true);
        missing_scalar
            .slot_feature_means
            .get_mut(SLOT_IDS[0])
            .expect("synthetic state includes active slots")
            .remove(SLOT_SCALAR_NAMES[0]);
        slot_matrix(&missing_slot).is_err() && slot_matrix(&missing_scalar).is_err()
    };
    let missing_skill_summary_fails_closed = {
        let skill_index = SkillFeatureIndex {
            by_code_state: BTreeMap::new(),
            level2_skill_count: 1,
            membership_count: 1,
            hierarchy_report_sha256: "synthetic-skill-hierarchy-readback".to_string(),
            coverage: SkillCoverageReport {
                expected_code_state_count: 0,
                covered_code_state_count: 0,
                uncovered_code_state_count: 0,
                uncovered_code_state_sample: Vec::new(),
                usefulness_policy: empty_skill_usefulness_policy_report(),
            },
        };
        let base = synthetic_code_state("missing-skill", "base", "known_good", true);
        let target =
            synthetic_code_state("missing-skill", "mutation_off_by_one", "off_by_one", false);
        let test_patch = synthetic_code_state("missing-skill", "test_patch", "known_good", true);
        skill_trajectory_feature_vector(
            &skill_index,
            &base,
            &target,
            &test_patch,
            SkillFeaturePolicy::UsefulnessCombined,
        )
        .is_err()
    };
    let support_shrink_missing_key_fails_closed = {
        let mut model = legal_support_shrink_calibration.model.clone();
        model.prior_by_key.clear();
        model.support_by_key.clear();
        let row = synthetic_prediction_row("support-missing", "off_by_one", true, 0.75);
        apply_support_shrink_calibration(&row, &model).is_err()
    };
    let support_shrink_non_scoped_row_unchanged = {
        let row = synthetic_prediction_row("support-nonscoped", "known_good", true, 0.95);
        let mut model = legal_support_shrink_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        let expected = apply_mean_bias_calibration(&row, &model.base_model)?;
        (apply_support_shrink_calibration(&row, &model)? - expected).abs() < 1.0e-6
    };
    let support_shrink_clamp_passed = {
        let mut model = legal_support_shrink_calibration.model.clone();
        model.scope = CalibrationScope::Global;
        model.prior_weight = 1.0;
        model.smoothing = 0.0;
        model.prior_by_key = BTreeMap::from([("__global__".to_string(), 2.0)]);
        model.support_by_key = BTreeMap::from([("__global__".to_string(), 1)]);
        let row = synthetic_prediction_row("support-clamp", "off_by_one", true, 0.75);
        (apply_support_shrink_calibration(&row, &model)? - 0.999).abs() < 1.0e-6
    };
    let evidence_consequence_non_scoped_row_unchanged = {
        let mut model = legal_evidence_consequence_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.base_model.scope = CalibrationScope::Global;
        model.base_model.bias_by_key = BTreeMap::from([("__global__".to_string(), -0.25)]);
        let row = synthetic_prediction_row("evidence-nonscoped", "known_good", true, 0.95);
        let feature = synthetic_feature_row("evidence-nonscoped", "known_good", vec!["edge-key"]);
        (apply_evidence_consequence_calibration(&row, &feature, &model)?
            - row.predicted_oracle_pass)
            .abs()
            < 1.0e-6
    };
    let bin_consequence_missing_key_noops = {
        let mut model = legal_bin_consequence_calibration.model.clone();
        model.key_stats.clear();
        let row = synthetic_prediction_row("bin-missing", "off_by_one", true, 0.25);
        let feature = synthetic_feature_row("bin-missing", "off_by_one", vec!["missing-key"]);
        (apply_bin_consequence_calibration(&row, &feature, &model)? - row.predicted_oracle_pass)
            .abs()
            < 1.0e-6
    };
    let bin_consequence_non_scoped_row_unchanged = {
        let mut model = legal_bin_consequence_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        let row = synthetic_prediction_row("bin-nonscoped", "known_good", true, 0.95);
        let feature = synthetic_feature_row("bin-nonscoped", "known_good", vec!["edge-key"]);
        (apply_bin_consequence_calibration(&row, &feature, &model)? - row.predicted_oracle_pass)
            .abs()
            < 1.0e-6
    };
    let bin_consequence_clamp_passed = {
        let mut model = legal_bin_consequence_calibration.model.clone();
        model.scope = CalibrationScope::Global;
        model.bin_count = ECE_BIN_COUNT;
        model.min_support = 1;
        model.smoothing = 0.0;
        model.prior_weight = 1.0;
        model.key_stats = BTreeMap::from([(
            bin_consequence_key(ece_bin_index(0.75), "edge-key"),
            BinConsequenceKeyStats {
                support: 1,
                positives: 1,
                prior: 2.0,
                score_mean: 0.75,
            },
        )]);
        let row = synthetic_prediction_row("bin-clamp", "off_by_one", true, 0.75);
        let feature = synthetic_feature_row("bin-clamp", "off_by_one", vec!["edge-key"]);
        (apply_bin_consequence_calibration(&row, &feature, &model)? - 0.999).abs() < 1.0e-6
    };
    let cause_residual_non_scoped_row_unchanged = {
        let mut model = legal_cause_residual_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.feature_count = 2;
        model.feature_mean = vec![0.0, 0.0];
        model.feature_std = vec![1.0, 1.0];
        model.weights_by_key = BTreeMap::from([(
            calibration_scope_key(model.scope, "off_by_one"),
            vec![0.50, 0.0, 0.0],
        )]);
        let row = synthetic_prediction_row("cause-residual-nonscoped", "known_good", true, 0.95);
        let mut feature = synthetic_feature_row("cause-residual-nonscoped", "known_good", vec![]);
        feature.cause_features = vec![1.0, 1.0];
        (apply_cause_residual_calibration(&row, &feature, &model)? - row.predicted_oracle_pass)
            .abs()
            < 1.0e-6
    };
    let cause_residual_missing_weight_noops = {
        let mut model = legal_cause_residual_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.feature_count = 2;
        model.feature_mean = vec![0.0, 0.0];
        model.feature_std = vec![1.0, 1.0];
        model.weights_by_key.clear();
        let row = synthetic_prediction_row("cause-residual-missing", "off_by_one", true, 0.45);
        let mut feature = synthetic_feature_row("cause-residual-missing", "off_by_one", vec![]);
        feature.cause_features = vec![1.0, 1.0];
        (apply_cause_residual_calibration(&row, &feature, &model)? - row.predicted_oracle_pass)
            .abs()
            < 1.0e-6
    };
    let cause_residual_feature_dimension_mismatch_fails_closed = {
        let mut model = legal_cause_residual_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.feature_count = 2;
        model.feature_mean = vec![0.0, 0.0];
        model.feature_std = vec![1.0, 1.0];
        model.weights_by_key = BTreeMap::from([(
            calibration_scope_key(model.scope, "off_by_one"),
            vec![0.0, 0.0, 0.0],
        )]);
        let row = synthetic_prediction_row("cause-residual-dim", "off_by_one", true, 0.45);
        let mut feature = synthetic_feature_row("cause-residual-dim", "off_by_one", vec![]);
        feature.cause_features = vec![1.0];
        apply_cause_residual_calibration(&row, &feature, &model).is_err()
    };
    let cause_residual_clamp_passed = {
        let mut model = legal_cause_residual_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.feature_count = 2;
        model.feature_mean = vec![0.0, 0.0];
        model.feature_std = vec![1.0, 1.0];
        model.correction_scale = 1.0;
        model.correction_cap = 0.50;
        model.weights_by_key = BTreeMap::from([(
            calibration_scope_key(model.scope, "off_by_one"),
            vec![2.0, 0.0, 0.0],
        )]);
        let row = synthetic_prediction_row("cause-residual-clamp", "off_by_one", true, 0.75);
        let mut feature = synthetic_feature_row("cause-residual-clamp", "off_by_one", vec![]);
        feature.cause_features = vec![0.0, 0.0];
        (apply_cause_residual_calibration(&row, &feature, &model)? - 0.999).abs() < 1.0e-6
    };
    let cause_bin_missing_key_noops = {
        let mut model = legal_cause_bin_calibration.model.clone();
        model.key_stats.clear();
        let row = synthetic_prediction_row("cause-bin-missing", "off_by_one", true, 0.75);
        (apply_cause_bin_calibration(&row, &model) - row.predicted_oracle_pass).abs() < 1.0e-6
    };
    let cause_bin_under_min_support_noops = {
        let mut model = legal_cause_bin_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.bin_count = 10;
        model.min_support = 4;
        model.smoothing = 0.0;
        model.prior_weight = 1.0;
        let row = synthetic_prediction_row("cause-bin-under-support", "off_by_one", true, 0.75);
        let scope_key = calibration_scope_key(model.scope, &row.category);
        let bin_idx = probability_bin_index(row.predicted_oracle_pass, model.bin_count);
        model.key_stats = BTreeMap::from([(
            cause_bin_key(&scope_key, bin_idx),
            CauseBinKeyStats {
                support: 1,
                positives: 0,
                prior: 0.001,
                score_mean: row.predicted_oracle_pass,
            },
        )]);
        (apply_cause_bin_calibration(&row, &model) - row.predicted_oracle_pass).abs() < 1.0e-6
    };
    let cause_bin_non_scoped_row_unchanged = {
        let mut model = legal_cause_bin_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        let row = synthetic_prediction_row("cause-bin-nonscoped", "known_good", true, 0.95);
        (apply_cause_bin_calibration(&row, &model) - row.predicted_oracle_pass).abs() < 1.0e-6
    };
    let cause_bin_clamp_passed = {
        let mut model = legal_cause_bin_calibration.model.clone();
        model.scope = CalibrationScope::Global;
        model.bin_count = 10;
        model.min_support = 1;
        model.smoothing = 0.0;
        model.prior_weight = 1.0;
        let row = synthetic_prediction_row("cause-bin-clamp", "off_by_one", true, 0.75);
        let bin_idx = probability_bin_index(row.predicted_oracle_pass, model.bin_count);
        model.key_stats = BTreeMap::from([(
            cause_bin_key("__global__", bin_idx),
            CauseBinKeyStats {
                support: 1,
                positives: 1,
                prior: 2.0,
                score_mean: row.predicted_oracle_pass,
            },
        )]);
        (apply_cause_bin_calibration(&row, &model) - 0.999).abs() < 1.0e-6
    };
    let score_sharpness_non_scoped_row_unchanged = {
        let mut model = legal_score_sharpness_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        let row = synthetic_prediction_row("sharpness-nonscoped", "known_good", true, 0.25);
        (apply_score_sharpness_calibration(&row, &model) - row.predicted_oracle_pass).abs() < 1.0e-6
    };
    let score_sharpness_low_tail_continuous_sharpens = {
        let mut model = legal_score_sharpness_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.low_cutoff = 0.30;
        model.low_tail_scale = 2.0;
        model.high_cutoff = 0.90;
        model.high_tail_scale = 1.0;
        let row = synthetic_prediction_row("sharpness-low", "off_by_one", false, 0.25);
        (apply_score_sharpness_calibration(&row, &model) - 0.20).abs() < 1.0e-6
    };
    let score_sharpness_high_tail_continuous_compresses = {
        let mut model = legal_score_sharpness_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.low_cutoff = 0.30;
        model.low_tail_scale = 1.0;
        model.high_cutoff = 0.90;
        model.high_tail_scale = 0.50;
        let row = synthetic_prediction_row("sharpness-high", "off_by_one", true, 0.95);
        (apply_score_sharpness_calibration(&row, &model) - 0.925).abs() < 1.0e-6
    };
    let score_sharpness_clamp_passed = {
        let mut model = legal_score_sharpness_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.low_cutoff = 0.40;
        model.low_tail_scale = 3.0;
        model.high_cutoff = 0.90;
        model.high_tail_scale = 1.0;
        let row = synthetic_prediction_row("sharpness-clamp", "off_by_one", false, 0.01);
        (apply_score_sharpness_calibration(&row, &model) - 0.001).abs() < 1.0e-6
    };
    let cause_band_missing_key_noops = {
        let mut model = legal_cause_band_calibration.model.clone();
        model.key_stats.clear();
        let row = synthetic_prediction_row("cause-band-missing", "off_by_one", true, 0.75);
        let mut feature = synthetic_feature_row("cause-band-missing", "off_by_one", vec![]);
        feature.cause_features = vec![0.0; 50];
        (apply_cause_band_calibration(&row, &feature, &model)? - row.predicted_oracle_pass).abs()
            < 1.0e-6
    };
    let cause_band_under_min_support_noops = {
        let mut model = legal_cause_band_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.min_support = 4;
        model.smoothing = 0.0;
        model.prior_weight = 1.0;
        let row = synthetic_prediction_row("cause-band-under-support", "off_by_one", true, 0.75);
        let mut feature = synthetic_feature_row("cause-band-under-support", "off_by_one", vec![]);
        feature.cause_features = vec![0.0; 50];
        let scope_key = calibration_scope_key(model.scope, &row.category);
        let key = cause_band_keys(&feature, &scope_key)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("synthetic cause-band key missing"))?;
        model.key_stats = BTreeMap::from([(
            key,
            CauseBandKeyStats {
                support: 1,
                positives: 0,
                prior: 0.001,
                score_mean: row.predicted_oracle_pass,
            },
        )]);
        (apply_cause_band_calibration(&row, &feature, &model)? - row.predicted_oracle_pass).abs()
            < 1.0e-6
    };
    let cause_band_non_scoped_row_unchanged = {
        let mut model = legal_cause_band_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        let row = synthetic_prediction_row("cause-band-nonscoped", "known_good", true, 0.95);
        let mut feature = synthetic_feature_row("cause-band-nonscoped", "known_good", vec![]);
        feature.cause_features = vec![0.0; 50];
        (apply_cause_band_calibration(&row, &feature, &model)? - row.predicted_oracle_pass).abs()
            < 1.0e-6
    };
    let cause_band_dimension_mismatch_fails_closed = {
        let model = legal_cause_band_calibration.model.clone();
        let row = synthetic_prediction_row("cause-band-dim", "off_by_one", true, 0.75);
        let mut feature = synthetic_feature_row("cause-band-dim", "off_by_one", vec![]);
        feature.cause_features = vec![0.0; 49];
        apply_cause_band_calibration(&row, &feature, &model).is_err()
    };
    let cause_band_clamp_passed = {
        let mut model = legal_cause_band_calibration.model.clone();
        model.scope = CalibrationScope::TrueMixedPerCategory;
        model.min_support = 1;
        model.smoothing = 0.0;
        model.prior_weight = 1.0;
        let row = synthetic_prediction_row("cause-band-clamp", "off_by_one", true, 0.75);
        let mut feature = synthetic_feature_row("cause-band-clamp", "off_by_one", vec![]);
        feature.cause_features = vec![0.0; 50];
        let scope_key = calibration_scope_key(model.scope, &row.category);
        let key = cause_band_keys(&feature, &scope_key)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("synthetic cause-band key missing"))?;
        model.key_stats = BTreeMap::from([(
            key,
            CauseBandKeyStats {
                support: 1,
                positives: 1,
                prior: 2.0,
                score_mean: row.predicted_oracle_pass,
            },
        )]);
        (apply_cause_band_calibration(&row, &feature, &model)? - 0.999).abs() < 1.0e-6
    };
    let ece_empty_rows_zero = {
        let breakdown = ece_breakdown_report(&[]);
        breakdown.aggregate_ece == 0.0
            && breakdown.ece_from_bins == 0.0
            && breakdown.non_empty_bin_count == 0
            && breakdown.worst_bins.is_empty()
    };
    let ece_boundary_bins_passed = {
        let rows = vec![
            synthetic_prediction_row("ece-low", "off_by_one", false, 0.001),
            synthetic_prediction_row("ece-high", "off_by_one", true, 0.999),
        ];
        let breakdown = ece_breakdown_report(&rows);
        breakdown
            .worst_bins
            .iter()
            .any(|bin| bin.bin_index == 0 && bin.sample_count == 1)
            && breakdown
                .worst_bins
                .iter()
                .any(|bin| bin.bin_index == ECE_BIN_COUNT - 1 && bin.sample_count == 1)
    };
    let ece_per_category_empty_subset_passed = {
        let rows = vec![synthetic_prediction_row(
            "ece-known-good",
            "known_good",
            true,
            0.95,
        )];
        let breakdown = ece_breakdown_report(&rows);
        breakdown
            .per_category
            .get("off_by_one")
            .map(|category| {
                category.metrics.sample_count == 0
                    && category.non_empty_bin_count == 0
                    && category.bins.is_empty()
            })
            .unwrap_or(false)
    };
    let all_passed = missing_code_state_row_fails_closed
        && missing_file_path_row_fails_closed
        && ordered_replay_passed
        && non_monotonic_order_fails_closed
        && target_label_prefixes_excluded
        && holdout_threshold_flagged_forbidden
        && hierarchy_db_readback_passed
        && affine_selected_on_calibration
        && affine_true_mixed_scope_leaves_non_mixed_rows_unchanged
        && affine_clamp_passed
        && missing_slot_scalar_fails_closed
        && missing_skill_summary_fails_closed
        && support_shrink_missing_key_fails_closed
        && support_shrink_non_scoped_row_unchanged
        && support_shrink_clamp_passed
        && evidence_consequence_non_scoped_row_unchanged
        && bin_consequence_missing_key_noops
        && bin_consequence_non_scoped_row_unchanged
        && bin_consequence_clamp_passed
        && cause_residual_non_scoped_row_unchanged
        && cause_residual_missing_weight_noops
        && cause_residual_feature_dimension_mismatch_fails_closed
        && cause_residual_clamp_passed
        && cause_bin_missing_key_noops
        && cause_bin_under_min_support_noops
        && cause_bin_non_scoped_row_unchanged
        && cause_bin_clamp_passed
        && score_sharpness_non_scoped_row_unchanged
        && score_sharpness_low_tail_continuous_sharpens
        && score_sharpness_high_tail_continuous_compresses
        && score_sharpness_clamp_passed
        && cause_band_missing_key_noops
        && cause_band_under_min_support_noops
        && cause_band_non_scoped_row_unchanged
        && cause_band_dimension_mismatch_fails_closed
        && cause_band_clamp_passed
        && ece_empty_rows_zero
        && ece_boundary_bins_passed
        && ece_per_category_empty_subset_passed;
    Ok(EdgeCaseAudit {
        all_passed,
        ordered_replay_passed,
        non_monotonic_order_fails_closed,
        missing_code_state_row_fails_closed,
        missing_file_path_row_fails_closed,
        target_label_prefixes_excluded,
        holdout_threshold_flagged_forbidden,
        hierarchy_db_readback_passed,
        affine_selected_on_calibration,
        affine_true_mixed_scope_leaves_non_mixed_rows_unchanged,
        affine_clamp_passed,
        missing_slot_scalar_fails_closed,
        missing_skill_summary_fails_closed,
        support_shrink_missing_key_fails_closed,
        support_shrink_non_scoped_row_unchanged,
        support_shrink_clamp_passed,
        evidence_consequence_non_scoped_row_unchanged,
        bin_consequence_missing_key_noops,
        bin_consequence_non_scoped_row_unchanged,
        bin_consequence_clamp_passed,
        cause_residual_non_scoped_row_unchanged,
        cause_residual_missing_weight_noops,
        cause_residual_feature_dimension_mismatch_fails_closed,
        cause_residual_clamp_passed,
        cause_bin_missing_key_noops,
        cause_bin_under_min_support_noops,
        cause_bin_non_scoped_row_unchanged,
        cause_bin_clamp_passed,
        score_sharpness_non_scoped_row_unchanged,
        score_sharpness_low_tail_continuous_sharpens,
        score_sharpness_high_tail_continuous_compresses,
        score_sharpness_clamp_passed,
        cause_band_missing_key_noops,
        cause_band_under_min_support_noops,
        cause_band_non_scoped_row_unchanged,
        cause_band_dimension_mismatch_fails_closed,
        cause_band_clamp_passed,
        ece_empty_rows_zero,
        ece_boundary_bins_passed,
        ece_per_category_empty_subset_passed,
    })
}

fn synthetic_prediction_row(
    task_id: &str,
    category: &str,
    actual_oracle_pass: bool,
    predicted_oracle_pass: f32,
) -> PredictionRow {
    PredictionRow {
        task_id: task_id.to_string(),
        category: category.to_string(),
        actual_oracle_pass,
        predicted_oracle_pass,
        predicted_pass: predicted_oracle_pass >= 0.5,
        correct: (predicted_oracle_pass >= 0.5) == actual_oracle_pass,
    }
}

fn synthetic_feature_row(task_id: &str, category: &str, evidence_keys: Vec<&str>) -> FeatureRow {
    FeatureRow {
        task_id: task_id.to_string(),
        category: category.to_string(),
        bucket: "synthetic-edge".to_string(),
        actual_oracle_pass: true,
        patch_path: "synthetic/edge.patch".to_string(),
        repo: "synthetic/repo".to_string(),
        evidence_keys: evidence_keys.into_iter().map(str::to_string).collect(),
        cause_features: vec![0.0; 50],
        standardized_features: Vec::new(),
    }
}

fn synthetic_code_state(
    task_id: &str,
    workspace_state: &str,
    mutation_category: &str,
    oracle_all_passed: bool,
) -> CodeStateConstellationRow {
    let slot_feature_means = SLOT_IDS
        .iter()
        .enumerate()
        .map(|(idx, slot)| {
            (
                (*slot).to_string(),
                BTreeMap::from([
                    ("abs_mean".to_string(), 0.01 + idx as f64 * 0.001),
                    ("entropy_proxy".to_string(), 0.90),
                    ("norm_l2".to_string(), 1.0),
                    ("std".to_string(), 0.03),
                    ("zero_fraction".to_string(), 0.0),
                ]),
            )
        })
        .collect();
    CodeStateConstellationRow {
        task_instance_id: task_id.to_string(),
        mutation_category: mutation_category.to_string(),
        workspace_state: workspace_state.to_string(),
        oracle_all_passed,
        relative_path: None,
        chunk_count: 1,
        file_count: 1,
        slot_identity_preserved: true,
        flat_vector_concat_used: false,
        labels: vec![
            format!("code_state_identity:workspace_state:{workspace_state}"),
            "dominant_surface:edge".to_string(),
        ],
        slot_feature_means,
        group_label_counts_top: BTreeMap::new(),
        pair_label_counts_top: BTreeMap::new(),
        slot_label_counts_top: BTreeMap::new(),
        surface_counts_top: BTreeMap::new(),
    }
}

fn metrics_for(rows: &[PredictionRow]) -> MetricBlock {
    if rows.is_empty() {
        return MetricBlock {
            sample_count: 0,
            accuracy: 0.0,
            prediction_oracle_correlation: None,
            oracle_variance_class: OracleVarianceClass::Empty,
            brier: 0.0,
            ece: 0.0,
            predicted_pass_count: 0,
            predicted_fail_count: 0,
            actual_pass_count: 0,
            actual_fail_count: 0,
            pass_recall: None,
            fail_recall: None,
            actual_class_confidence_mean: 0.0,
            score_min: 0.0,
            score_max: 0.0,
            score_mean: 0.0,
        };
    }
    let mut predicted = Vec::with_capacity(rows.len());
    let mut actual = Vec::with_capacity(rows.len());
    let mut correct = 0usize;
    let mut brier = 0.0f32;
    let mut predicted_pass_count = 0usize;
    let mut actual_pass_count = 0usize;
    let mut true_pass = 0usize;
    let mut true_fail = 0usize;
    let mut actual_class_confidence = 0.0f32;
    for row in rows {
        let target = if row.actual_oracle_pass { 1.0 } else { 0.0 };
        predicted.push(row.predicted_oracle_pass);
        actual.push(target);
        if row.correct {
            correct += 1;
        }
        if row.predicted_pass {
            predicted_pass_count += 1;
        }
        if row.actual_oracle_pass {
            actual_pass_count += 1;
            if row.predicted_pass {
                true_pass += 1;
            }
            actual_class_confidence += row.predicted_oracle_pass;
        } else if !row.predicted_pass {
            true_fail += 1;
            actual_class_confidence += 1.0 - row.predicted_oracle_pass;
        } else {
            actual_class_confidence += 1.0 - row.predicted_oracle_pass;
        }
        brier += (row.predicted_oracle_pass - target).powi(2);
    }
    let actual_fail_count = rows.len() - actual_pass_count;
    let predicted_fail_count = rows.len() - predicted_pass_count;
    let score_min = predicted.iter().copied().fold(f32::INFINITY, f32::min);
    let score_max = predicted.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let score_mean = predicted.iter().sum::<f32>() / predicted.len() as f32;
    MetricBlock {
        sample_count: rows.len(),
        accuracy: correct as f32 / rows.len() as f32,
        prediction_oracle_correlation: pearson(&predicted, &actual),
        oracle_variance_class: oracle_variance_class(actual_pass_count, actual_fail_count),
        brier: brier / rows.len() as f32,
        ece: expected_calibration_error(rows),
        predicted_pass_count,
        predicted_fail_count,
        actual_pass_count,
        actual_fail_count,
        pass_recall: ratio_opt(true_pass, actual_pass_count),
        fail_recall: ratio_opt(true_fail, actual_fail_count),
        actual_class_confidence_mean: actual_class_confidence / rows.len() as f32,
        score_min,
        score_max,
        score_mean,
    }
}

fn per_category_metrics(rows: &[PredictionRow]) -> BTreeMap<String, MetricBlock> {
    REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            let subset = rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            ((*category).to_string(), metrics_for(&subset))
        })
        .collect()
}

fn expected_calibration_error(rows: &[PredictionRow]) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    let mut bins = vec![(0usize, 0.0f32, 0.0f32); ECE_BIN_COUNT];
    for row in rows {
        let index = ((row.predicted_oracle_pass * ECE_BIN_COUNT as f32).floor() as usize)
            .min(ECE_BIN_COUNT - 1);
        let target = if row.actual_oracle_pass { 1.0 } else { 0.0 };
        bins[index].0 += 1;
        bins[index].1 += row.predicted_oracle_pass;
        bins[index].2 += target;
    }
    bins.into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, confidence_sum, target_sum)| {
            let weight = count as f32 / rows.len() as f32;
            let avg_confidence = confidence_sum / count as f32;
            let avg_target = target_sum / count as f32;
            weight * (avg_confidence - avg_target).abs()
        })
        .sum()
}

fn ece_breakdown_report(rows: &[PredictionRow]) -> EceBreakdownReport {
    let bins = ece_bin_breakdown(rows);
    let ece_from_bins = bins.iter().map(|bin| bin.weighted_ece).sum::<f32>();
    let mut worst_bins = bins.clone();
    worst_bins.sort_by(|left, right| {
        right
            .weighted_ece
            .partial_cmp(&left.weighted_ece)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.sample_count.cmp(&left.sample_count))
    });
    worst_bins.truncate(12);

    let per_category = REQUIRED_CATEGORIES
        .iter()
        .map(|category| {
            let subset = rows
                .iter()
                .filter(|row| row.category == *category)
                .cloned()
                .collect::<Vec<_>>();
            let bins = ece_bin_breakdown(&subset);
            let worst_gap = bins.iter().map(|bin| bin.abs_gap).fold(0.0_f32, f32::max);
            (
                (*category).to_string(),
                CategoryEceBreakdown {
                    metrics: metrics_for(&subset),
                    non_empty_bin_count: bins.len(),
                    worst_gap,
                    bins,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    EceBreakdownReport {
        aggregate_ece: expected_calibration_error(rows),
        ece_from_bins,
        sample_count: rows.len(),
        non_empty_bin_count: bins.len(),
        worst_bins,
        per_category,
    }
}

fn ece_breakdown_audit(metrics: &MetricBlock, breakdown: &EceBreakdownReport) -> EceBreakdownAudit {
    let tolerance = 1.0e-6_f32;
    let sample_count_matches = metrics.sample_count == breakdown.sample_count;
    let aggregate_matches_metric = (metrics.ece - breakdown.aggregate_ece).abs() <= tolerance;
    let bins_sum_matches_aggregate =
        (breakdown.aggregate_ece - breakdown.ece_from_bins).abs() <= tolerance;
    let non_empty_bins_when_sampled =
        breakdown.sample_count == 0 || breakdown.non_empty_bin_count > 0;
    EceBreakdownAudit {
        all_passed: sample_count_matches
            && aggregate_matches_metric
            && bins_sum_matches_aggregate
            && non_empty_bins_when_sampled,
        metric_ece: metrics.ece,
        aggregate_ece: breakdown.aggregate_ece,
        ece_from_bins: breakdown.ece_from_bins,
        sample_count_matches,
        aggregate_matches_metric,
        bins_sum_matches_aggregate,
        non_empty_bins_when_sampled,
        tolerance,
    }
}

fn ece_bin_breakdown(rows: &[PredictionRow]) -> Vec<EceBinBreakdown> {
    if rows.is_empty() {
        return Vec::new();
    }
    let mut bins = (0..ECE_BIN_COUNT)
        .map(|idx| EceBinAccumulator {
            bin_index: idx,
            ..EceBinAccumulator::default()
        })
        .collect::<Vec<_>>();
    for row in rows {
        let index = ece_bin_index(row.predicted_oracle_pass);
        let bin = &mut bins[index];
        bin.sample_count += 1;
        bin.confidence_sum += row.predicted_oracle_pass;
        if row.actual_oracle_pass {
            bin.actual_pass_count += 1;
        }
        if row.predicted_pass {
            bin.predicted_pass_count += 1;
        }
        if bin.sample_task_ids.len() < 5 {
            bin.sample_task_ids
                .push(format!("{}:{}", row.task_id, row.category));
        }
    }
    bins.into_iter()
        .filter(|bin| bin.sample_count > 0)
        .map(|bin| bin.into_breakdown(rows.len()))
        .collect()
}

fn ece_bin_index(score: f32) -> usize {
    (score.clamp(0.0, 0.999_999) * ECE_BIN_COUNT as f32).floor() as usize
}

#[derive(Debug, Clone, Default)]
struct EceBinAccumulator {
    bin_index: usize,
    sample_count: usize,
    actual_pass_count: usize,
    predicted_pass_count: usize,
    confidence_sum: f32,
    sample_task_ids: Vec<String>,
}

impl EceBinAccumulator {
    fn into_breakdown(self, total_count: usize) -> EceBinBreakdown {
        let confidence_mean = self.confidence_sum / self.sample_count as f32;
        let empirical_pass_rate = self.actual_pass_count as f32 / self.sample_count as f32;
        let abs_gap = (confidence_mean - empirical_pass_rate).abs();
        let weighted_ece = if total_count == 0 {
            0.0
        } else {
            (self.sample_count as f32 / total_count as f32) * abs_gap
        };
        EceBinBreakdown {
            bin_index: self.bin_index,
            range_start: self.bin_index as f32 / ECE_BIN_COUNT as f32,
            range_end: (self.bin_index + 1) as f32 / ECE_BIN_COUNT as f32,
            sample_count: self.sample_count,
            actual_pass_count: self.actual_pass_count,
            actual_fail_count: self.sample_count - self.actual_pass_count,
            predicted_pass_count: self.predicted_pass_count,
            predicted_fail_count: self.sample_count - self.predicted_pass_count,
            confidence_mean,
            empirical_pass_rate,
            abs_gap,
            weighted_ece,
            sample_task_ids: self.sample_task_ids,
        }
    }
}

fn pearson(xs: &[f32], ys: &[f32]) -> Option<f32> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let mean_x = xs.iter().sum::<f32>() / xs.len() as f32;
    let mean_y = ys.iter().sum::<f32>() / ys.len() as f32;
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for (x, y) in xs.iter().zip(ys) {
        let dx = *x - mean_x;
        let dy = *y - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    if var_x <= f32::EPSILON || var_y <= f32::EPSILON {
        None
    } else {
        Some((cov / (var_x.sqrt() * var_y.sqrt())).clamp(-1.0, 1.0))
    }
}

fn oracle_variance_class(
    actual_pass_count: usize,
    actual_fail_count: usize,
) -> OracleVarianceClass {
    match (actual_pass_count, actual_fail_count) {
        (0, 0) => OracleVarianceClass::Empty,
        (0, _) => OracleVarianceClass::AllFail,
        (_, 0) => OracleVarianceClass::AllPass,
        _ => OracleVarianceClass::Mixed,
    }
}

fn ratio_opt(numerator: usize, denominator: usize) -> Option<f32> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f32 / denominator as f32)
    }
}

fn squared_distance(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = left - right;
            if delta.is_finite() {
                delta * delta
            } else {
                0.0
            }
        })
        .sum()
}

fn reproduction_report(baseline: &MetricBlock, reproduced: &MetricBlock) -> ReproductionReport {
    let tolerance = 0.002;
    let correlation_delta_abs = match (
        baseline.prediction_oracle_correlation,
        reproduced.prediction_oracle_correlation,
    ) {
        (Some(left), Some(right)) => Some((left - right).abs()),
        _ => None,
    };
    let brier_delta_abs = (baseline.brier - reproduced.brier).abs();
    let ece_delta_abs = (baseline.ece - reproduced.ece).abs();
    let passed = correlation_delta_abs
        .map(|delta| delta <= tolerance)
        .unwrap_or(false)
        && brier_delta_abs <= tolerance
        && ece_delta_abs <= tolerance;
    ReproductionReport {
        passed,
        correlation_delta_abs,
        brier_delta_abs,
        ece_delta_abs,
        tolerance,
    }
}

fn directional_signal_report(
    baseline: &MetricBlock,
    legal_best: &LegalBestCandidate,
) -> DirectionalSignalReport {
    let candidate = &legal_best.holdout.metrics;
    let correlation_delta = baseline
        .prediction_oracle_correlation
        .zip(candidate.prediction_oracle_correlation)
        .map(|(base, current)| current - base);
    let brier_delta = candidate.brier - baseline.brier;
    let ece_delta = candidate.ece - baseline.ece;
    let passed = correlation_delta.map(|delta| delta > 0.0).unwrap_or(false) && brier_delta < 0.0;
    DirectionalSignalReport {
        passed,
        baseline_correlation: baseline.prediction_oracle_correlation,
        candidate_correlation: candidate.prediction_oracle_correlation,
        correlation_delta,
        baseline_brier: baseline.brier,
        candidate_brier: candidate.brier,
        brier_delta,
        baseline_ece: baseline.ece,
        candidate_ece: candidate.ece,
        ece_delta,
    }
}

fn metrics_from_report(report: &Value, path: &[&str]) -> Result<MetricBlock> {
    let mut value = report;
    for segment in path {
        value = value
            .get(*segment)
            .ok_or_else(|| anyhow!("report path missing segment {segment}"))?;
    }
    metric_block_from_json(value)
}

fn per_category_metrics_from_report(report: &Value) -> Result<BTreeMap<String, MetricBlock>> {
    let value = &report["execute_and_inspect"]["per_category_metrics"];
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("baseline final_per_category_metrics must be object"))?;
    object
        .iter()
        .map(|(category, metric)| Ok((category.clone(), metric_block_from_json(metric)?)))
        .collect()
}

fn metric_block_from_json(value: &Value) -> Result<MetricBlock> {
    let oracle_variance_class = match value["oracle_variance_class"].as_str().unwrap_or("empty") {
        "all_fail" => OracleVarianceClass::AllFail,
        "all_pass" => OracleVarianceClass::AllPass,
        "mixed" => OracleVarianceClass::Mixed,
        _ => OracleVarianceClass::Empty,
    };
    let score_mean = json_f32(&value["score_mean"])?;
    let actual_class_confidence_mean = value
        .get("actual_class_confidence_mean")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .unwrap_or(match oracle_variance_class {
            OracleVarianceClass::AllPass => score_mean,
            OracleVarianceClass::AllFail => 1.0 - score_mean,
            _ => 0.0,
        });
    Ok(MetricBlock {
        sample_count: value["sample_count"].as_u64().unwrap_or_default() as usize,
        accuracy: json_f32(&value["accuracy"])?,
        prediction_oracle_correlation: option_json_f32(&value["prediction_oracle_correlation"])?,
        oracle_variance_class,
        brier: json_f32(&value["brier"])?,
        ece: json_f32(&value["ece"])?,
        predicted_pass_count: value["predicted_pass_count"].as_u64().unwrap_or_default() as usize,
        predicted_fail_count: value["predicted_fail_count"].as_u64().unwrap_or_default() as usize,
        actual_pass_count: value["actual_pass_count"].as_u64().unwrap_or_default() as usize,
        actual_fail_count: value["actual_fail_count"].as_u64().unwrap_or_default() as usize,
        pass_recall: option_json_f32(&value["pass_recall"])?,
        fail_recall: option_json_f32(&value["fail_recall"])?,
        actual_class_confidence_mean,
        score_min: json_f32(&value["score_min"])?,
        score_max: json_f32(&value["score_max"])?,
        score_mean,
    })
}

fn json_f32(value: &Value) -> Result<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .ok_or_else(|| anyhow!("expected JSON number, got {value}"))
}

fn option_json_f32(value: &Value) -> Result<Option<f32>> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(json_f32(value)?))
    }
}

fn split_counts(entries: &[CorpusEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.bucket.clone()).or_default() += 1;
    }
    counts
}

fn ensure_category_grid(entries: &[CorpusEntry], bucket: &str) -> Result<()> {
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in entries.iter().filter(|entry| entry.bucket == bucket) {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }
    for category in REQUIRED_CATEGORIES {
        ensure!(
            counts.get(category).copied() == Some(30),
            "{bucket} must contain 30 rows for category {category}; got {:?}",
            counts.get(category)
        );
    }
    Ok(())
}

fn train_counts_by_category(rows: &[FeatureRow]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows.iter().filter(|row| row.bucket == "train") {
        *counts.entry(row.category.clone()).or_default() += 1;
    }
    counts
}

fn class_support_by_category(rows: &[FeatureRow]) -> BTreeMap<String, Value> {
    let mut support = BTreeMap::<String, (usize, usize, usize, usize, usize, usize)>::new();
    for row in rows {
        let entry = support.entry(row.category.clone()).or_default();
        let pair = match row.bucket.as_str() {
            "train" => (&mut entry.0, &mut entry.1),
            "calibration" => (&mut entry.2, &mut entry.3),
            "holdout" => (&mut entry.4, &mut entry.5),
            _ => continue,
        };
        if row.actual_oracle_pass {
            *pair.0 += 1;
        } else {
            *pair.1 += 1;
        }
    }
    support
        .into_iter()
        .map(|(category, counts)| {
            (
                category,
                json!({
                    "train_pass": counts.0,
                    "train_fail": counts.1,
                    "calibration_pass": counts.2,
                    "calibration_fail": counts.3,
                    "holdout_pass": counts.4,
                    "holdout_fail": counts.5,
                }),
            )
        })
        .collect()
}

fn active_input_paths() -> Vec<String> {
    vec![
        CORPUS_DIR.to_string(),
        INDEX_PATH.to_string(),
        LABEL_ROOT.to_string(),
        CODE_STATE_CONSTELLATIONS_PATH.to_string(),
        FILE_PATH_CONSTELLATIONS_PATH.to_string(),
        hierarchy_db_path(),
        hierarchy_report_path(),
        BASELINE_REPORT_PATH.to_string(),
        fsv_root(),
    ]
}

fn active_paths_include_mntd() -> bool {
    active_input_paths()
        .iter()
        .any(|path| active_path_violates_aiwonder_policy(path))
}

fn active_path_violates_aiwonder_policy(path: &str) -> bool {
    if path == "/mnt/d" || path.starts_with("/mnt/d/") || path.starts_with("/mnt/c/") {
        return true;
    }
    let allowed_logical_root =
        path.starts_with("/zfs/archive/contextgraph") || path.starts_with("/zfs/hot/contextgraph");
    if !allowed_logical_root {
        return true;
    }
    if let Ok(canonical) = fs::canonicalize(path) {
        let display = canonical.to_string_lossy();
        display == "/mnt/d" || display.starts_with("/mnt/d/") || display.starts_with("/mnt/c/")
    } else {
        false
    }
}

fn hierarchy_db_path() -> String {
    env::var(HIERARCHY_DB_PATH_ENV).unwrap_or_else(|_| HIERARCHY_DB_PATH.to_string())
}

fn hierarchy_report_path() -> String {
    env::var(HIERARCHY_REPORT_PATH_ENV).unwrap_or_else(|_| HIERARCHY_REPORT_PATH.to_string())
}

fn fsv_root() -> String {
    env::var(TASK_SKILL_002_FSV_ROOT_ENV).unwrap_or_else(|_| FSV_ROOT.to_string())
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path.as_ref())?)?)
}

fn write_json_0600(path: impl AsRef<Path>, value: &Value) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    ensure!(
        fs::read(path)? == bytes,
        "JSON readback bytes differed for {}",
        path.display()
    );
    Ok(())
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(sha256_hex(&fs::read(path.as_ref())?))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
