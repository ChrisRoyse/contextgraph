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
    "/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/failure-mode-hierarchy.rocksdb";
const HIERARCHY_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/task_py_g_110_failure_mode_hierarchy_fsv.json";
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
const SELECTION_ECE_WEIGHT: f32 = 0.25;
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
    let legal_support_shrink_calibration =
        select_support_shrink_calibration_repair(feature_table, &legal_mean_bias_calibration)?;
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

    let legal_best_candidate = legal_best_candidate(
        &best_legal_global,
        &legal_threshold,
        &legal_affine_calibration,
        &legal_prediction_blend,
        &legal_logit_calibration,
        &legal_mean_bias_calibration,
        &legal_support_shrink_calibration,
        &legal_probability_calibration,
    );
    let directional_signal =
        directional_signal_report(&baseline_final_metrics, &legal_best_candidate);
    let score_fusion = score_fusion_report(&baseline_report)?;
    let typed_blocker = typed_blocker(&legal_best_candidate, &forbidden_threshold);
    let edge_cases = edge_case_audit(
        feature_table,
        &forbidden_threshold,
        &legal_affine_calibration,
        &legal_support_shrink_calibration,
    )?;

    let promotable_candidate_found = legal_best_candidate.blockers.is_empty();
    let all_passed = edge_cases.ordered_replay_passed
        && baseline_row_trace_comparison.passed
        && edge_cases.non_monotonic_order_fails_closed
        && edge_cases.missing_code_state_row_fails_closed
        && edge_cases.missing_file_path_row_fails_closed
        && edge_cases.hierarchy_db_readback_passed
        && directional_signal.passed
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
        "legal_affine_calibration": legal_affine_calibration,
        "legal_prediction_blend": legal_prediction_blend,
        "legal_logit_calibration": legal_logit_calibration,
        "legal_mean_bias_calibration": legal_mean_bias_calibration,
        "legal_ece_focused_mean_bias_calibration": legal_ece_focused_mean_bias_calibration,
        "legal_support_shrink_calibration": legal_support_shrink_calibration,
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
            "report": report_readback,
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
struct LegalBestCandidate {
    kind: String,
    candidate: Value,
    selection_reason: String,
    calibration: SplitReport,
    holdout: SplitReport,
    blockers: Vec<String>,
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
    score_min: f32,
    score_max: f32,
    score_mean: f32,
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
        raw_features.push(features);
        rows.push(FeatureRow {
            task_id: entry.task_id.clone(),
            category: entry.category.clone(),
            bucket: entry.bucket.clone(),
            actual_oracle_pass: entry.oracle_all_passed,
            patch_path: entry.patch_path.clone(),
            repo: entry.repo.clone(),
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
        true,
    )?;
    let raw_metrics = raw_eval.mean_bias.holdout.metrics.clone();
    let mut evaluations = vec![
        evaluate_skill_policy(
            "code_state_only",
            base_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            false,
        )?,
        evaluate_skill_policy(
            "no_skill_file_trajectory",
            no_skill_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            false,
        )?,
        raw_eval,
        evaluate_skill_policy(
            SkillFeaturePolicy::FailureOnly.label(),
            failure_skill_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::PassStabilityOnly.label(),
            pass_stability_skill_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::ContextOnly.label(),
            context_skill_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            true,
        )?,
        evaluate_skill_policy(
            SkillFeaturePolicy::UsefulnessCombined.label(),
            usefulness_feature_table,
            baseline_metrics,
            Some(&raw_metrics),
            true,
        )?,
    ];
    for eval in &mut evaluations {
        if eval.policy == SkillFeaturePolicy::RawCurrent.label() {
            eval.delta_vs_raw_current = Some(metric_delta(
                "current_raw_skill_surface",
                "current_raw_skill_surface",
                &raw_metrics,
                &eval.mean_bias.holdout.metrics,
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
            && useful.mean_bias.holdout.metrics.ece < raw_metrics.ece
            && useful.mean_bias.selection_objective
                <= selected.mean_bias.selection_objective + 0.002
        {
            selected = useful.clone();
        }
    }
    let selected_reason = if selected.policy == SkillFeaturePolicy::UsefulnessCombined.label() {
        "usefulness-weighted ordered skills selected by calibration objective/non-regression"
            .to_string()
    } else {
        "usefulness-weighted ordered skills evaluated but not selected; current raw skill surface remains the non-regressing active policy".to_string()
    };
    let typed_blocker = if selected.policy == SkillFeaturePolicy::UsefulnessCombined.label()
        && selected.mean_bias.holdout.metrics.ece < BASELINE_536_ECE
    {
        "TASK_SKILL_002_USEFULNESS_WEIGHTED_CLEAR".to_string()
    } else if selected.selection_eligible {
        "TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED".to_string()
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
            &mean_bias.holdout.metrics,
        )
    });
    let selection_eligible = if skill_policy_selectable {
        delta_vs_raw_current
            .as_ref()
            .map(|delta| delta.non_regressing)
            .unwrap_or(true)
    } else {
        false
    };
    let rejection_reason = if selection_eligible {
        None
    } else if skill_policy_selectable {
        Some("policy regressed versus current raw skill surface".to_string())
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
    legal_threshold: &ThresholdRepairReport,
    legal_affine_calibration: &AffineCalibrationReport,
    legal_prediction_blend: &PredictionBlendReport,
    legal_logit_calibration: &LogitCalibrationReport,
    legal_mean_bias_calibration: &MeanBiasCalibrationReport,
    legal_support_shrink_calibration: &SupportShrinkCalibrationReport,
    legal_probability_calibration: &ProbabilityCalibrationReport,
) -> LegalBestCandidate {
    let mut best_kind = "calibration_selected_global_candidate";
    let mut best_objective = best_global.selection_objective;
    if legal_threshold.selection_objective < best_objective {
        best_kind = "calibration_selected_threshold_repair";
        best_objective = legal_threshold.selection_objective;
    }
    if legal_affine_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_affine_calibration";
        best_objective = legal_affine_calibration.selection_objective;
    }
    if legal_prediction_blend.selection_objective < best_objective {
        best_kind = "calibration_selected_prediction_blend";
        best_objective = legal_prediction_blend.selection_objective;
    }
    if legal_logit_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_logit_calibration";
        best_objective = legal_logit_calibration.selection_objective;
    }
    if legal_mean_bias_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_mean_bias_calibration";
        best_objective = legal_mean_bias_calibration.selection_objective;
    }
    if legal_support_shrink_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_support_shrink_calibration";
        best_objective = legal_support_shrink_calibration.selection_objective;
    }
    if legal_probability_calibration.selection_objective < best_objective {
        best_kind = "calibration_selected_probability_calibration";
    }
    match best_kind {
        "calibration_selected_threshold_repair" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_threshold).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected off_by_one threshold transform by calibration objective"
                    .to_string(),
            calibration: legal_threshold.calibration.clone(),
            holdout: legal_threshold.holdout.clone(),
            blockers: legal_threshold.blockers.clone(),
        },
        "calibration_selected_affine_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_affine_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected affine score calibration by calibration objective"
                    .to_string(),
            calibration: legal_affine_calibration.calibration.clone(),
            holdout: legal_affine_calibration.holdout.clone(),
            blockers: legal_affine_calibration.blockers.clone(),
        },
        "calibration_selected_prediction_blend" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_prediction_blend).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected affine/rank-lift prediction blend by calibration objective"
                    .to_string(),
            calibration: legal_prediction_blend.calibration.clone(),
            holdout: legal_prediction_blend.holdout.clone(),
            blockers: legal_prediction_blend.blockers.clone(),
        },
        "calibration_selected_logit_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_logit_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected logit score calibration by calibration objective"
                    .to_string(),
            calibration: legal_logit_calibration.calibration.clone(),
            holdout: legal_logit_calibration.holdout.clone(),
            blockers: legal_logit_calibration.blockers.clone(),
        },
        "calibration_selected_mean_bias_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_mean_bias_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected mean-bias score calibration by out-of-fold calibration objective"
                    .to_string(),
            calibration: legal_mean_bias_calibration.calibration.clone(),
            holdout: legal_mean_bias_calibration.holdout.clone(),
            blockers: legal_mean_bias_calibration.blockers.clone(),
        },
        "calibration_selected_support_shrink_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_support_shrink_calibration)
                .unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected support-aware prior shrinkage by out-of-fold calibration objective"
                    .to_string(),
            calibration: legal_support_shrink_calibration.calibration.clone(),
            holdout: legal_support_shrink_calibration.holdout.clone(),
            blockers: legal_support_shrink_calibration.blockers.clone(),
        },
        "calibration_selected_probability_calibration" => LegalBestCandidate {
            kind: best_kind.to_string(),
            candidate: serde_json::to_value(legal_probability_calibration).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected probability calibration map by calibration objective"
                    .to_string(),
            calibration: legal_probability_calibration.calibration.clone(),
            holdout: legal_probability_calibration.holdout.clone(),
            blockers: legal_probability_calibration.blockers.clone(),
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

fn edge_case_audit(
    table: &FeatureTable,
    forbidden_threshold: &ThresholdRepairReport,
    legal_affine_calibration: &AffineCalibrationReport,
    legal_support_shrink_calibration: &SupportShrinkCalibrationReport,
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
        && support_shrink_clamp_passed;
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
        } else if !row.predicted_pass {
            true_fail += 1;
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
    Ok(MetricBlock {
        sample_count: value["sample_count"].as_u64().unwrap_or_default() as usize,
        accuracy: json_f32(&value["accuracy"])?,
        prediction_oracle_correlation: option_json_f32(&value["prediction_oracle_correlation"])?,
        oracle_variance_class: match value["oracle_variance_class"].as_str().unwrap_or("empty") {
            "all_fail" => OracleVarianceClass::AllFail,
            "all_pass" => OracleVarianceClass::AllPass,
            "mixed" => OracleVarianceClass::Mixed,
            _ => OracleVarianceClass::Empty,
        },
        brier: json_f32(&value["brier"])?,
        ece: json_f32(&value["ece"])?,
        predicted_pass_count: value["predicted_pass_count"].as_u64().unwrap_or_default() as usize,
        predicted_fail_count: value["predicted_fail_count"].as_u64().unwrap_or_default() as usize,
        actual_pass_count: value["actual_pass_count"].as_u64().unwrap_or_default() as usize,
        actual_fail_count: value["actual_fail_count"].as_u64().unwrap_or_default() as usize,
        pass_recall: option_json_f32(&value["pass_recall"])?,
        fail_recall: option_json_f32(&value["fail_recall"])?,
        score_min: json_f32(&value["score_min"])?,
        score_max: json_f32(&value["score_max"])?,
        score_mean: json_f32(&value["score_mean"])?,
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
        .any(|path| path.starts_with("/mnt/d/"))
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
