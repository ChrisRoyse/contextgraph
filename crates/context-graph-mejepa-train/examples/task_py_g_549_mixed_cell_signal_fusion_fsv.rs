//! TASK-PY-G-549 mixed-cell signal-fusion diagnostic FSV.
//!
//! This deliberately does not rerun the CUDA predictor. It replays the
//! calibration-only code-state constellation rank-lift surface from the
//! aiwonder corpus and adds live-safe repo/path metadata features to test
//! whether #549 can legally repair true mixed cells without holdout-selected
//! thresholds.

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const CODE_STATE_CONSTELLATIONS_PATH: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/code_state_constellations.jsonl";
const BASELINE_REPORT_PATH: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/ship-gate-fsv.json";
const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-549-mixed-cell-signal-fusion-fsv";
const REPORT_NAME: &str = "task_py_g_549_mixed_cell_signal_fusion_fsv.json";
const READBACK_NAME: &str = "task_py_g_549_mixed_cell_signal_fusion_readback.json";
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
    let run_dir = Path::new(FSV_ROOT).join(&run_id);
    fs::create_dir_all(&run_dir)?;

    let baseline_report = read_json(BASELINE_REPORT_PATH)?;
    ensure!(
        baseline_report["all_passed"].as_bool() == Some(true),
        "baseline #536 FSV must have all_passed=true"
    );
    let baseline_report_sha256 = sha256_file(BASELINE_REPORT_PATH)?;
    let baseline_final_metrics =
        metrics_from_report(&baseline_report, &["execute_and_inspect", "final_metrics"])?;
    let baseline_per_category = per_category_metrics_from_report(&baseline_report)?;

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

    let base_feature_table = load_feature_table(&index.entries, false)?;
    let feature_table = load_feature_table(&index.entries, true)?;
    let train_counts = train_counts_by_category(&feature_table.rows);
    let class_support = class_support_by_category(&feature_table.rows);

    let reproduced = candidate_split_report(&base_feature_table, CURRENT_RBF)?;
    let reproduction = reproduction_report(&baseline_final_metrics, &reproduced.holdout.metrics);

    let candidate_results = rank_candidate_sweep(&feature_table)?;
    let best_legal_global = candidate_results
        .first()
        .ok_or_else(|| anyhow!("candidate sweep returned no candidates"))?
        .clone();
    let legal_threshold = select_threshold_repair(
        &feature_table,
        ThresholdSelectionSplit::Calibration,
        ThresholdRepairSpec::default(),
    )?;
    let forbidden_threshold = select_threshold_repair(
        &feature_table,
        ThresholdSelectionSplit::Holdout,
        ThresholdRepairSpec::default(),
    )?;
    ensure!(
        forbidden_threshold.selection_forbidden,
        "holdout-selected threshold repair must be marked forbidden"
    );

    let legal_best_candidate = legal_best_candidate(&best_legal_global, &legal_threshold);
    let score_fusion = score_fusion_report(&baseline_report)?;
    let typed_blocker = typed_blocker(
        &legal_best_candidate,
        &forbidden_threshold,
        &score_fusion.best_legal_candidate,
    );
    let edge_cases = edge_case_audit(&feature_table, &forbidden_threshold)?;

    let promotable_candidate_found = legal_best_candidate.blockers.is_empty()
        || score_fusion.best_legal_candidate.blockers.is_empty();
    let all_passed = reproduction.passed
        && (promotable_candidate_found || typed_blocker.accepted)
        && score_fusion.edge_cases.all_passed
        && edge_cases.all_passed;
    let report = json!({
        "schema_version": 1,
        "issue": 549,
        "run_id": run_id,
        "started_at": started_at.to_rfc3339(),
        "all_passed": all_passed,
        "promotable_candidate_found": promotable_candidate_found,
        "active_input_paths": active_input_paths(),
        "baseline_report_path": BASELINE_REPORT_PATH,
        "baseline_report_sha256": baseline_report_sha256,
        "feature_source": {
            "index_path": INDEX_PATH,
            "code_state_constellations_path": CODE_STATE_CONSTELLATIONS_PATH,
            "feature_policy": "slot-preserving code-state constellation means plus hashed live group/pair/slot/surface label counts plus live-safe repo and touched-path metadata; target-side oracle/docker/test/leakage labels excluded; train split standardization only",
            "feature_dim": feature_table.feature_dim,
            "baseline_feature_dim_without_metadata": base_feature_table.feature_dim,
            "metadata_feature_policy": "repo hash features use CorpusEntry.repo; path hash features use diff ---/+++ paths, top-level dir, dir2, and extension; no oracle/test result labels are used",
            "metadata_feature_dims": {
                "repo_hash_features": REPO_HASH_FEATURES,
                "path_hash_features": PATH_HASH_FEATURES,
            },
            "loaded_rows": feature_table.rows.len(),
            "split_counts": split_counts,
            "train_counts_by_category": train_counts,
            "class_support_by_category": class_support,
            "target_label_prefixes_excluded": TARGET_LABEL_PREFIXES,
        },
        "baseline_536": {
            "final_metrics": baseline_final_metrics,
            "per_category_metrics": baseline_per_category,
        },
        "reproduced_current_policy": {
            "candidate": CURRENT_RBF,
            "calibration": reproduced.calibration,
            "holdout": reproduced.holdout,
            "reproduction": reproduction,
        },
        "candidate_results": candidate_results,
        "legal_best_candidate": legal_best_candidate,
        "score_fusion": score_fusion,
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
        "readback_reproduced_correlation": report_readback["reproduced_current_policy"]["holdout"]["metrics"]["prediction_oracle_correlation"],
        "readback_legal_best_blockers": report_readback["legal_best_candidate"]["blockers"],
        "readback_score_fusion_best_blockers": report_readback["score_fusion"]["best_legal_candidate"]["blockers"],
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
    oracle_all_passed: bool,
    #[serde(default)]
    chunk_count: usize,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
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

#[derive(Debug, Default)]
struct Aggregate {
    count: usize,
    feature_sum: Vec<f32>,
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
    missing_code_state_row_fails_closed: bool,
    target_label_prefixes_excluded: bool,
    holdout_threshold_flagged_forbidden: bool,
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
    include_metadata_features: bool,
) -> Result<FeatureTable> {
    let split = entries
        .iter()
        .map(|entry| {
            (
                (entry.task_id.clone(), entry.category.clone()),
                (entry.bucket.clone(), entry.oracle_all_passed),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut aggregates = BTreeMap::<(String, String), Aggregate>::new();
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
        let key = (row.task_instance_id.clone(), row.mutation_category.clone());
        let Some((_bucket, expected_label)) = split.get(&key) else {
            continue;
        };
        ensure!(
            row.oracle_all_passed == *expected_label,
            "code-state oracle label mismatch for {}:{}",
            row.task_instance_id,
            row.mutation_category
        );
        let features = feature_vector(&row);
        let aggregate = aggregates.entry(key).or_insert_with(|| Aggregate {
            count: 0,
            feature_sum: vec![0.0; features.len()],
        });
        ensure!(
            aggregate.feature_sum.len() == features.len(),
            "feature dimension changed within aggregate"
        );
        aggregate.count += 1;
        for (sum, value) in aggregate.feature_sum.iter_mut().zip(features) {
            *sum += value;
        }
    }

    let mut rows = Vec::with_capacity(split.len());
    let mut raw_features = Vec::with_capacity(split.len());
    for entry in entries {
        let key = (entry.task_id.clone(), entry.category.clone());
        let aggregate = aggregates
            .get(&key)
            .ok_or_else(|| anyhow!("missing code-state constellation row for {:?}", key))?;
        ensure!(aggregate.count > 0, "empty aggregate for {:?}", key);
        let mut features = aggregate
            .feature_sum
            .iter()
            .map(|value| *value / aggregate.count as f32)
            .collect::<Vec<_>>();
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
    })
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

fn feature_vector(row: &CodeStateConstellationRow) -> Vec<f32> {
    let mut features = Vec::with_capacity(61 + HASH_FEATURES);
    for embedder_id in [
        "e1", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e6", "e7", "e8", "e9",
    ] {
        let slot = row.slot_feature_means.get(embedder_id);
        for feature_name in [
            "abs_mean",
            "entropy_proxy",
            "norm_l2",
            "std",
            "zero_fraction",
        ] {
            features.push(
                slot.and_then(|values| values.get(feature_name))
                    .copied()
                    .unwrap_or_default() as f32,
            );
        }
    }
    features.push((row.chunk_count as f32).ln_1p());
    let mut hashed = vec![0.0_f32; HASH_FEATURES];
    let denom = (row.chunk_count as f32).max(1.0);
    add_count_features(&mut hashed, "group", &row.group_label_counts_top, denom);
    add_count_features(&mut hashed, "pair", &row.pair_label_counts_top, denom);
    add_count_features(&mut hashed, "slot", &row.slot_label_counts_top, denom);
    add_count_features(&mut hashed, "surface", &row.surface_counts_top, denom);
    features.extend(hashed);
    features
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
    let mut means = vec![0.0_f64; feature_dim];
    for idx in &train_indices {
        ensure!(
            raw_features[*idx].len() == feature_dim,
            "feature dimension mismatch"
        );
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
        row.standardized_features = features
            .iter()
            .zip(&means)
            .zip(&stds)
            .map(|((value, mean), std)| ((*value as f64 - *mean) / *std) as f32)
            .collect();
    }
    Ok(())
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
    let mut weight_sum = 0.0_f64;
    let mut weighted_label_sum = 0.0_f64;
    for train_idx in train_indices {
        let train = table
            .rows
            .get(*train_idx)
            .ok_or_else(|| anyhow!("train index out of range"))?;
        let distance = squared_distance(&row.standardized_features, &train.standardized_features);
        let class_weight = if train.actual_oracle_pass {
            1.0
        } else {
            fail_boost as f64
        };
        let weight = (-(distance as f64) / (2.0 * (tau as f64).powi(2))).exp() * class_weight;
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
    metrics.brier + 0.25 * metrics.ece + correlation_reward
}

fn repair_blockers(
    metrics: &MetricBlock,
    per_category: &BTreeMap<String, MetricBlock>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if metrics
        .prediction_oracle_correlation
        .map(|value| value < 0.794_219)
        .unwrap_or(true)
    {
        blockers.push(format!(
            "aggregate_correlation_not_above_536: {:?}",
            metrics.prediction_oracle_correlation
        ));
    }
    if metrics.brier > 0.092_373 {
        blockers.push(format!("aggregate_brier_regressed: {:.9}", metrics.brier));
    }
    if metrics.ece > 0.024_032 {
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
) -> LegalBestCandidate {
    let threshold_better = legal_threshold
        .holdout
        .metrics
        .prediction_oracle_correlation
        .zip(best_global.holdout.metrics.prediction_oracle_correlation)
        .map(|(threshold, global)| threshold > global)
        .unwrap_or(false);
    if threshold_better {
        LegalBestCandidate {
            kind: "calibration_selected_threshold_repair".to_string(),
            candidate: serde_json::to_value(legal_threshold).unwrap_or(Value::Null),
            selection_reason:
                "best calibration-selected off_by_one threshold transform by calibration objective"
                    .to_string(),
            calibration: legal_threshold.calibration.clone(),
            holdout: legal_threshold.holdout.clone(),
            blockers: legal_threshold.blockers.clone(),
        }
    } else {
        LegalBestCandidate {
            kind: "calibration_selected_global_candidate".to_string(),
            candidate: serde_json::to_value(best_global).unwrap_or(Value::Null),
            selection_reason:
                "best global RBF/KNN candidate by calibration objective, with no holdout-selected threshold"
                    .to_string(),
            calibration: best_global.calibration.clone(),
            holdout: best_global.holdout.clone(),
            blockers: best_global.blockers.clone(),
        }
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
    score_fusion_best: &ScoreFusionCandidate,
) -> TypedBlocker {
    let forbidden_shows_signal = forbidden_threshold
        .holdout
        .metrics
        .prediction_oracle_correlation
        .map(|value| value > 0.80)
        .unwrap_or(false)
        && forbidden_threshold.holdout.metrics.brier < 0.092_373;
    let accepted = !legal_best.blockers.is_empty()
        && !score_fusion_best.blockers.is_empty()
        && forbidden_threshold.selection_forbidden
        && forbidden_shows_signal;
    TypedBlocker {
        code: "MIXED_CELL_SIGNAL_FUSION_INSUFFICIENT".to_string(),
        accepted,
        why: "Adding live-safe repo/touched-path metadata and calibration-selected raw/rank-safe/rank-lift score fusion improves the legal search but still cannot produce a promotable mixed-cell repair. The remaining true mixed cells need temporal, multi-chunk, or cross-window evidence rather than more holdout threshold tuning.".to_string(),
        evidence: vec![
            format!("repo_path_legal_best_kind={}", legal_best.kind),
            format!("repo_path_legal_best_blockers={}", legal_best.blockers.join(";")),
            format!("score_fusion_policy={}", score_fusion_best.policy),
            format!("score_fusion_best_blockers={}", score_fusion_best.blockers.join(";")),
            format!(
                "forbidden_holdout_corr={:?}",
                forbidden_threshold.holdout.metrics.prediction_oracle_correlation
            ),
            format!("forbidden_holdout_brier={:.9}", forbidden_threshold.holdout.metrics.brier),
        ],
        missing_signal: vec![
            "temporal patch trajectory and multi-chunk failure-chain evidence for true mixed cells".to_string(),
            "larger calibration support or cross-window validation for sparse mixed failure modes".to_string(),
            "finer live-safe structural labels than repo/touched-path metadata for subtle_flip and swap_variable".to_string(),
            "per-cell or per-constellation baseline families beyond a single aggregate threshold".to_string(),
        ],
        next_work: vec![
            "Do not promote repo/path metadata fusion as a ship-gate repair from this FSV.".to_string(),
            "Do not promote raw/rank-safe/rank-lift score fusion as a ship-gate repair from this FSV unless its blocker list is empty.".to_string(),
            "Do not tune more thresholds on #536 holdout.".to_string(),
            "Advance temporal trajectory evidence, multi-chunk failure chains, or cross-window validation before rerunning the ship gate.".to_string(),
        ],
    }
}

fn edge_case_audit(
    table: &FeatureTable,
    forbidden_threshold: &ThresholdRepairReport,
) -> Result<EdgeCaseAudit> {
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
        load_feature_table(&entries, true).is_err()
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
    let all_passed = missing_code_state_row_fails_closed
        && target_label_prefixes_excluded
        && holdout_threshold_flagged_forbidden;
    Ok(EdgeCaseAudit {
        all_passed,
        missing_code_state_row_fails_closed,
        target_label_prefixes_excluded,
        holdout_threshold_flagged_forbidden,
    })
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
            delta * delta
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

fn active_input_paths() -> Vec<&'static str> {
    vec![
        CORPUS_DIR,
        INDEX_PATH,
        CODE_STATE_CONSTELLATIONS_PATH,
        BASELINE_REPORT_PATH,
        FSV_ROOT,
    ]
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
