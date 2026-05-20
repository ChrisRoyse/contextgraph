//! Real SWE-bench Lite Python 300x8 ship-gate rebaseline FSV.
//!
//! This FSV intentionally does not train a model and does not use a synthetic
//! holdout predictor. It loads the manifest-backed trained checkpoint, builds
//! real frozen-instrument panels from the verified SWE-bench patch files, runs
//! the checkpoint-backed CUDA predictor/oracle head, and records the resulting
//! post-fix ship-gate measurement.

use anyhow::{anyhow, ensure, Context, Result};
use candle_core::{DType, Device, Tensor};
use chrono::Utc;
use context_graph_mejepa::{
    load_verified_trained_predictor_checkpoint, materialize_inference_panels, panel_sha,
    predictor_weight_content_sha256, valid_witness_segment, AstDiff, DiffHunk, FrozenTargetAdapter,
    Language, MeJepaPredictor, PatchBundle, PredictorConfig, TaskContext, TaskEnvironment, TaskId,
    TestId, PANEL_DIM,
};
use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CORPUS_DIR: &str = "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const STATS_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/stats.json";
const VERIFY_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json";
const CHECKPOINT_MANIFEST_PATH: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json";
const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv";
const REPORT_NAME: &str = "ship-gate-fsv.json";
const READBACK_NAME: &str = "ship-gate-readback.json";
const SHIP_GATE_TARGET: f32 = 0.95;
const REQUIRED_HOLDOUT_COUNT: usize = 240;
const REQUIRED_SPLIT_COUNTS: (usize, usize, usize) = (1920, 240, 240);
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

    let verify_evidence = read_json(VERIFY_PATH)?;
    let verify_evidence_sha256 = sha256_file(VERIFY_PATH)?;
    let legacy_mntd_in_verify_evidence = json_contains_string(&verify_evidence, "/mnt/d/");
    ensure!(
        verify_evidence["passes"].as_bool() == Some(true),
        "corpus verify evidence must pass"
    );
    ensure!(
        verify_evidence["entry_count"].as_u64() == Some(2400),
        "corpus verify evidence must contain 2400 entries"
    );
    let stats = read_json(STATS_PATH)?;
    ensure!(
        stats["full_ship_gate_300x8"]["passes"].as_bool() == Some(true),
        "corpus stats full_ship_gate_300x8 must pass"
    );
    ensure!(
        stats["leakage_check"]["passes"].as_bool() == Some(true),
        "corpus stats leakage_check must pass"
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
    let holdout = index
        .entries
        .iter()
        .filter(|entry| entry.bucket == "holdout")
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        holdout.len() == REQUIRED_HOLDOUT_COUNT,
        "holdout split must contain {REQUIRED_HOLDOUT_COUNT} entries"
    );
    ensure_category_grid(&holdout)?;

    let predictor_config = PredictorConfig::default();
    let device = Device::new_cuda(0).context("ship-gate rebaseline requires aiwonder CUDA 0")?;
    let mut predictor = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded_checkpoint = load_verified_trained_predictor_checkpoint(
        &mut predictor,
        Path::new(CHECKPOINT_MANIFEST_PATH),
        &predictor_config,
    )?;
    ensure!(
        loaded_checkpoint.trained_weight_sha256 != loaded_checkpoint.initial_weight_sha256,
        "trained checkpoint manifest must prove weights changed"
    );
    let loaded_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    ensure!(
        loaded_weight_sha256 == loaded_checkpoint.trained_weight_sha256,
        "loaded predictor weight hash must match manifest trained_weight_sha256"
    );

    let predictions = evaluate_holdout(&predictor, &device, &holdout)?;
    let final_metrics = metrics_for(&predictions);
    let per_category_metrics = per_category_metrics(&predictions);
    let rolling_windows = rolling_window_reports(&predictions);
    let final_blockers = ship_gate_blockers(&final_metrics, &per_category_metrics);
    let rolling_blockers = rolling_windows
        .iter()
        .flat_map(|window| {
            window
                .blockers
                .iter()
                .map(move |blocker| format!("window_{}:{blocker}", window.window_index))
        })
        .collect::<Vec<_>>();
    let ship_gate_passed = final_blockers.is_empty() && rolling_blockers.is_empty();
    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    let source_audit = source_audit()?;
    let panel_audit = panel_audit(&predictions);
    let elapsed = (Utc::now() - started_at)
        .to_std()
        .unwrap_or_default()
        .as_secs_f32();
    let all_passed = predictions.len() == REQUIRED_HOLDOUT_COUNT
        && loaded_checkpoint.trained_weight_sha256 != loaded_checkpoint.initial_weight_sha256
        && loaded_weight_sha256 == loaded_checkpoint.trained_weight_sha256
        && source_audit["echo_predictor_present"] == false
        && source_audit["category_majority_oracle_present"] == false
        && panel_audit["unique_t1_panel_sha_count"]
            .as_u64()
            .unwrap_or_default()
            > 1
        && panel_audit["real_patch_paths_read"].as_u64() == Some(REQUIRED_HOLDOUT_COUNT as u64)
        && !active_paths_include_mntd();

    let report = json!({
        "schema_version": 2,
        "task_id": "TASK-PY-G-REBASELINE",
        "issue": 529,
        "generated_at": Utc::now().to_rfc3339(),
        "run_id": run_id,
        "source_of_truth": {
            "corpus_dir": CORPUS_DIR,
            "index": INDEX_PATH,
            "stats": STATS_PATH,
            "verify_evidence": VERIFY_PATH,
            "checkpoint_manifest": CHECKPOINT_MANIFEST_PATH,
            "fsv_run_dir": run_dir,
            "report_path": report_path,
            "readback_path": readback_path
        },
        "pre_state": {
            "active_path_policy": {
                "mnt_d_active_reads_or_writes": false,
                "active_inputs": active_input_paths(),
                "legacy_mntd_strings_in_verify_evidence": legacy_mntd_in_verify_evidence,
                "verify_evidence_sha256": verify_evidence_sha256
            },
            "corpus_verify": {
                "passes": verify_evidence["passes"],
                "entry_count": verify_evidence["entry_count"],
                "corpus_sha256": verify_evidence["corpus_sha256"],
                "verdict_cf_count": verify_evidence["verdict_cf_count"],
                "task_manifest_count": verify_evidence["task_manifest_count"]
            },
            "corpus_stats": {
                "total_entries": stats["total_entries"],
                "by_bucket": stats["by_bucket"],
                "by_category": stats["by_category"],
                "by_oracle": stats["by_oracle"],
                "full_ship_gate_300x8": stats["full_ship_gate_300x8"],
                "leakage_check": stats["leakage_check"]
            },
            "loaded_split_counts": split_counts,
            "holdout_category_counts": category_counts(&holdout)
        },
        "checkpoint": {
            "manifest_path": loaded_checkpoint.manifest_path,
            "checkpoint_path": loaded_checkpoint.checkpoint_path,
            "manifest_sha256": loaded_checkpoint.manifest_sha256,
            "checkpoint_sha256": loaded_checkpoint.checkpoint_sha256,
            "checkpoint_bytes": loaded_checkpoint.checkpoint_bytes,
            "payload_step": loaded_checkpoint.payload_step,
            "optimizer_steps": loaded_checkpoint.optimizer_steps,
            "training_mode": loaded_checkpoint.training_mode,
            "initial_weight_sha256": loaded_checkpoint.initial_weight_sha256,
            "trained_weight_sha256": loaded_checkpoint.trained_weight_sha256,
            "loaded_weight_sha256": loaded_weight_sha256,
            "weight_delta_observed": true,
            "training_certificate_sha256": loaded_checkpoint.training_certificate_sha256,
            "native_active_constellation_adapter": loaded_checkpoint.native_active_constellation_adapter,
            "corpus_sha256": loaded_checkpoint.corpus_sha256,
            "config_sha256": loaded_checkpoint.config_sha256,
            "code_version": loaded_checkpoint.code_version
        },
        "execute_and_inspect": {
            "prediction_path": "real patch bytes -> PatchBundle -> materialize_inference_panels -> MeJepaPredictor::forward -> checkpoint-backed OracleHead",
            "patch_source_encoding": "unified diff removed/added text is embedded as JSON-escaped string constants in parseable Python carrier modules before code instruments run; patch_sha256 remains verified against original patch bytes",
            "synthetic_seed_features_used": false,
            "echo_predictor_used": false,
            "category_majority_oracle_used": false,
            "ship_gate_target": SHIP_GATE_TARGET,
            "holdout_prediction_count": predictions.len(),
            "final_metrics": final_metrics,
            "per_category_metrics": per_category_metrics,
            "rolling_windows": rolling_windows,
            "ship_gate_passed": ship_gate_passed,
            "ship_gate_blockers": final_blockers,
            "rolling_window_blockers": rolling_blockers,
            "sample_predictions": predictions.iter().take(16).collect::<Vec<_>>()
        },
        "edge_case_audit": {
            "source_audit": source_audit,
            "panel_audit": panel_audit
        },
        "wall_clock_seconds": elapsed,
        "all_passed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    let report_readback = read_json(&report_path)?;
    ensure!(
        report_readback["all_passed"].as_bool() == Some(true),
        "FSV measurement failed: {}",
        serde_json::to_string_pretty(&report_readback)?
    );
    ensure!(
        report_readback["execute_and_inspect"]["holdout_prediction_count"].as_u64()
            == Some(REQUIRED_HOLDOUT_COUNT as u64),
        "report readback must contain all holdout predictions"
    );
    let readback = json!({
        "schema_version": 1,
        "report_path": report_path,
        "report_sha256": report_sha256,
        "readback_all_passed": report_readback["all_passed"],
        "readback_ship_gate_passed": report_readback["execute_and_inspect"]["ship_gate_passed"],
        "readback_prediction_oracle_correlation": report_readback["execute_and_inspect"]["final_metrics"]["prediction_oracle_correlation"],
        "readback_holdout_prediction_count": report_readback["execute_and_inspect"]["holdout_prediction_count"],
    });
    write_json_0600(&readback_path, &readback)?;
    let readback_sha256 = sha256_file(&readback_path)?;
    let final_output = json!({
        "report": report_readback,
        "readback": readback,
        "hashes": {
            "report_sha256": report_sha256,
            "readback_sha256": readback_sha256
        }
    });
    println!("{}", serde_json::to_string_pretty(&final_output)?);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    #[allow(dead_code)]
    corpus_sha256: Option<String>,
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusEntry {
    bucket: String,
    category: String,
    mutation_note: Option<String>,
    oracle_all_passed: bool,
    oracle_per_test_count: Option<usize>,
    oracle_verdict_sha256: Option<String>,
    patch_path: String,
    patch_sha256: String,
    repo: String,
    task_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct PredictionRow {
    task_id: String,
    repo: String,
    category: String,
    actual_oracle_pass: bool,
    predicted_oracle_pass: f32,
    predicted_pass: bool,
    correct: bool,
    patch_path: String,
    patch_sha256: String,
    panel_t0_sha256: String,
    panel_t1_sha256: String,
    panel_t2_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct MetricBlock {
    sample_count: usize,
    accuracy: f32,
    prediction_oracle_correlation: Option<f32>,
    brier: f32,
    predicted_pass_count: usize,
    actual_pass_count: usize,
    score_min: f32,
    score_max: f32,
    score_mean: f32,
}

#[derive(Debug, Clone, Serialize)]
struct RollingWindowReport {
    window_index: usize,
    sample_count: usize,
    metrics: MetricBlock,
    per_category_metrics: BTreeMap<String, MetricBlock>,
    ship_gate_passed: bool,
    blockers: Vec<String>,
}

fn read_index(path: impl AsRef<Path>) -> Result<CorpusIndex> {
    let path = path.as_ref();
    let index: CorpusIndex = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(!index.entries.is_empty(), "corpus index has no entries");
    Ok(index)
}

fn evaluate_holdout(
    predictor: &MeJepaPredictor,
    device: &Device,
    entries: &[CorpusEntry],
) -> Result<Vec<PredictionRow>> {
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let patch_path = Path::new(CORPUS_DIR).join(&entry.patch_path);
        let patch = patch_bundle_from_entry(entry, &patch_path)?;
        let context = task_context_from_entry(entry)?;
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
        let score = predict_oracle_pass(predictor, device, &panel_t0, &panel_t1)?;
        let predicted_pass = score >= 0.5;
        out.push(PredictionRow {
            task_id: entry.task_id.clone(),
            repo: entry.repo.clone(),
            category: entry.category.clone(),
            actual_oracle_pass: entry.oracle_all_passed,
            predicted_oracle_pass: score,
            predicted_pass,
            correct: predicted_pass == entry.oracle_all_passed,
            patch_path: patch_path.display().to_string(),
            patch_sha256: entry.patch_sha256.clone(),
            panel_t0_sha256: hex::encode(panel_sha(&panel_t0)),
            panel_t1_sha256: hex::encode(panel_sha(&panel_t1)),
            panel_t2_sha256: hex::encode(panel_sha(&panel_t2)),
        });
    }
    Ok(out)
}

fn predict_oracle_pass(
    predictor: &MeJepaPredictor,
    device: &Device,
    panel_t0: &Panel,
    panel_t1: &Panel,
) -> Result<f32> {
    let t0 = panel_to_tensor(panel_t0, device, predictor.dtype())?;
    let t1 = panel_to_tensor(panel_t1, device, predictor.dtype())?;
    let predicted = predictor.forward(&t0, &t1)?;
    device.synchronize()?;
    let logits = predictor.oracle_head().predict_logits(&predicted)?;
    let probabilities = predictor.oracle_head().predict_probabilities(&logits)?;
    let values = probabilities
        .tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let score = *values
        .first()
        .ok_or_else(|| anyhow!("oracle head emitted no oracle-pass probability"))?;
    ensure!(
        score.is_finite() && (0.0..=1.0).contains(&score),
        "oracle-pass probability must be finite in [0,1], got {score}"
    );
    Ok(score)
}

fn panel_to_tensor(panel: &Panel, device: &Device, dtype: DType) -> Result<Tensor> {
    ensure!(
        panel.data().len() == PANEL_DIM,
        "panel dim mismatch: {} != {PANEL_DIM}",
        panel.data().len()
    );
    Ok(Tensor::from_slice(panel.data(), (1, PANEL_DIM), device)?.to_dtype(dtype)?)
}

fn patch_bundle_from_entry(entry: &CorpusEntry, patch_path: &Path) -> Result<PatchBundle> {
    let patch_bytes = fs::read(patch_path)
        .with_context(|| format!("read patch file {}", patch_path.display()))?;
    let patch_text = String::from_utf8(patch_bytes.clone())
        .with_context(|| format!("patch file is not UTF-8: {}", patch_path.display()))?;
    let observed_sha = format!("sha256:{}", sha256_hex(&patch_bytes));
    ensure!(
        observed_sha == entry.patch_sha256,
        "patch SHA mismatch for {} expected {} got {}",
        patch_path.display(),
        entry.patch_sha256,
        observed_sha
    );
    let patch_sha = parse_sha256_prefixed(&entry.patch_sha256)?;
    let ast_diff = parse_unified_diff(&patch_text)?;
    let commit_message = format!(
        "task_id={} repo={} category={} note={}",
        entry.task_id,
        entry.repo,
        entry.category,
        entry.mutation_note.as_deref().unwrap_or("none")
    );
    Ok(PatchBundle::try_new(
        ast_diff,
        valid_witness_segment(),
        commit_message,
        patch_sha,
    )?)
}

fn task_context_from_entry(entry: &CorpusEntry) -> Result<TaskContext> {
    let mut session_id = [0u8; 16];
    let digest = Sha256::digest(format!("{}:{}", entry.task_id, entry.category).as_bytes());
    session_id.copy_from_slice(&digest[..16]);
    let test_count = entry.oracle_per_test_count.unwrap_or(1).clamp(1, 1024);
    let tests = (0..test_count)
        .map(|idx| TestId(format!("oracle_test_{idx}")))
        .collect::<Vec<_>>();
    let context = TaskContext {
        task_id: TaskId(format!("{}:{}", entry.task_id, entry.category)),
        session_id,
        language: Language::Python,
        problem_statement: format!(
            "SWE-bench Lite task {} in repo {} with mutation category {}; oracle verdict {}",
            entry.task_id,
            entry.repo,
            entry.category,
            entry
                .oracle_verdict_sha256
                .as_deref()
                .unwrap_or("sha256:unknown")
        ),
        tests,
        environment: TaskEnvironment {
            repo_root: PathBuf::from(CORPUS_DIR),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    context.validate()?;
    Ok(context)
}

#[derive(Debug, Default)]
struct ParsedFilePatch {
    path: Option<PathBuf>,
    before: Vec<String>,
    after: Vec<String>,
}

fn parse_unified_diff(text: &str) -> Result<AstDiff> {
    let mut hunks = Vec::new();
    let mut current = ParsedFilePatch::default();
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            push_parsed_file(&mut hunks, &mut current)?;
            current.path = parse_diff_git_path(line);
            continue;
        }
        if let Some(path) = line.strip_prefix("+++ b/") {
            current.path = Some(PathBuf::from(path));
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("+++") || line.starts_with("@@") {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            current.after.push(added.to_string());
        } else if let Some(removed) = line.strip_prefix('-') {
            current.before.push(removed.to_string());
        } else if let Some(context) = line.strip_prefix(' ') {
            current.before.push(context.to_string());
            current.after.push(context.to_string());
        }
    }
    push_parsed_file(&mut hunks, &mut current)?;
    if hunks.is_empty() {
        let before = python_patch_carrier("patch.diff", "before", "")?;
        let after = python_patch_carrier("patch.diff", "after", text)?;
        hunks.push(DiffHunk {
            path: PathBuf::from("patch.diff"),
            pre_sha: sha256_bytes(before.as_bytes()),
            post_sha: sha256_bytes(after.as_bytes()),
            before,
            after,
        });
    }
    Ok(AstDiff { hunks })
}

fn push_parsed_file(hunks: &mut Vec<DiffHunk>, current: &mut ParsedFilePatch) -> Result<()> {
    if current.path.is_none() && current.before.is_empty() && current.after.is_empty() {
        return Ok(());
    }
    let path = current
        .path
        .take()
        .unwrap_or_else(|| PathBuf::from("unknown.py"));
    let raw_before = current.before.join("\n");
    let raw_after = current.after.join("\n");
    let display_path = path.display().to_string();
    let before = python_patch_carrier(&display_path, "before", &raw_before)?;
    let after = python_patch_carrier(&display_path, "after", &raw_after)?;
    hunks.push(DiffHunk {
        path,
        pre_sha: sha256_bytes(before.as_bytes()),
        post_sha: sha256_bytes(after.as_bytes()),
        before,
        after,
    });
    current.before.clear();
    current.after.clear();
    Ok(())
}

fn python_patch_carrier(path: &str, side: &str, body: &str) -> Result<String> {
    let path_lit = serde_json::to_string(path)?;
    let side_lit = serde_json::to_string(side)?;
    let body_lit = serde_json::to_string(body)?;
    Ok(format!(
        "MEJEPA_PATCH_PATH = {path_lit}\nMEJEPA_PATCH_SIDE = {side_lit}\nMEJEPA_PATCH_BODY = {body_lit}\nMEJEPA_PATCH_LINE_COUNT = {}\n",
        body.lines().count()
    ))
}

fn parse_diff_git_path(line: &str) -> Option<PathBuf> {
    line.split_whitespace()
        .nth(3)
        .and_then(|path| path.strip_prefix("b/"))
        .map(PathBuf::from)
}

fn metrics_for(rows: &[PredictionRow]) -> MetricBlock {
    if rows.is_empty() {
        return MetricBlock {
            sample_count: 0,
            accuracy: 0.0,
            prediction_oracle_correlation: None,
            brier: 0.0,
            predicted_pass_count: 0,
            actual_pass_count: 0,
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
        }
        brier += (row.predicted_oracle_pass - target).powi(2);
    }
    let score_min = predicted.iter().copied().fold(f32::INFINITY, f32::min);
    let score_max = predicted.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let score_mean = predicted.iter().sum::<f32>() / predicted.len() as f32;
    MetricBlock {
        sample_count: rows.len(),
        accuracy: correct as f32 / rows.len() as f32,
        prediction_oracle_correlation: pearson(&predicted, &actual),
        brier: brier / rows.len() as f32,
        predicted_pass_count,
        actual_pass_count,
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

fn rolling_window_reports(rows: &[PredictionRow]) -> Vec<RollingWindowReport> {
    let mut by_category: BTreeMap<String, Vec<PredictionRow>> = BTreeMap::new();
    for row in rows {
        by_category
            .entry(row.category.clone())
            .or_default()
            .push(row.clone());
    }
    let mut reports = Vec::new();
    for window_index in 0..4 {
        let mut window_rows = Vec::new();
        for category in REQUIRED_CATEGORIES {
            let category_rows = by_category.get(category).cloned().unwrap_or_default();
            let start = category_rows.len() * window_index / 4;
            let end = category_rows.len() * (window_index + 1) / 4;
            window_rows.extend_from_slice(&category_rows[start..end]);
        }
        let metrics = metrics_for(&window_rows);
        let per_category = per_category_metrics(&window_rows);
        let blockers = ship_gate_blockers(&metrics, &per_category);
        reports.push(RollingWindowReport {
            window_index,
            sample_count: window_rows.len(),
            metrics,
            per_category_metrics: per_category,
            ship_gate_passed: blockers.is_empty(),
            blockers,
        });
    }
    reports
}

fn ship_gate_blockers(
    metrics: &MetricBlock,
    per_category: &BTreeMap<String, MetricBlock>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    match metrics.prediction_oracle_correlation {
        Some(value) if value >= SHIP_GATE_TARGET => {}
        Some(value) => blockers.push(format!(
            "overall prediction_oracle_correlation {value:.6} < {SHIP_GATE_TARGET:.6}"
        )),
        None => blockers.push("overall prediction_oracle_correlation unavailable".to_string()),
    }
    for (category, block) in per_category {
        match block.prediction_oracle_correlation {
            Some(value) if value >= SHIP_GATE_TARGET => {}
            Some(value) => blockers.push(format!(
                "{category}::python prediction_oracle_correlation {value:.6} < {SHIP_GATE_TARGET:.6}"
            )),
            None => blockers.push(format!(
                "{category}::python prediction_oracle_correlation unavailable"
            )),
        }
    }
    blockers
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

fn split_counts(entries: &[CorpusEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.bucket.clone()).or_default() += 1;
    }
    counts
}

fn category_counts(entries: &[CorpusEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }
    counts
}

fn ensure_category_grid(entries: &[CorpusEntry]) -> Result<()> {
    let counts = category_counts(entries);
    for category in REQUIRED_CATEGORIES {
        ensure!(
            counts.get(category).copied() == Some(30),
            "holdout must contain 30 rows for category {category}; got {:?}",
            counts.get(category)
        );
    }
    Ok(())
}

fn panel_audit(predictions: &[PredictionRow]) -> Value {
    let unique_t0 = predictions
        .iter()
        .map(|row| row.panel_t0_sha256.clone())
        .collect::<BTreeSet<_>>();
    let unique_t1 = predictions
        .iter()
        .map(|row| row.panel_t1_sha256.clone())
        .collect::<BTreeSet<_>>();
    let unique_t2 = predictions
        .iter()
        .map(|row| row.panel_t2_sha256.clone())
        .collect::<BTreeSet<_>>();
    json!({
        "real_patch_paths_read": predictions.len(),
        "unique_t0_panel_sha_count": unique_t0.len(),
        "unique_t1_panel_sha_count": unique_t1.len(),
        "unique_t2_panel_sha_count": unique_t2.len(),
        "panel_dim": PANEL_DIM
    })
}

fn source_audit() -> Result<Value> {
    let source = fs::read_to_string(file!()).unwrap_or_default();
    let legacy_echo = ["struct", "EchoPredictor"].join(" ");
    let legacy_oracle = ["Category", "Majority", "Oracle"].join("");
    let synthetic_vector = ["deterministic", "_vector("].join("");
    let predictor_forward = ["predictor", ".forward("].join("");
    Ok(json!({
        "source_file": file!(),
        "echo_predictor_present": source.contains(&legacy_echo),
        "category_majority_oracle_present": source.contains(&legacy_oracle),
        "deterministic_vector_symbol_present": source.contains(&synthetic_vector),
        "mejepa_predictor_forward_present": source.contains(&predictor_forward),
        "materialize_inference_panels_present": source.contains("materialize_inference_panels"),
    }))
}

fn active_input_paths() -> Vec<&'static str> {
    vec![
        CORPUS_DIR,
        INDEX_PATH,
        STATS_PATH,
        VERIFY_PATH,
        CHECKPOINT_MANIFEST_PATH,
        FSV_ROOT,
    ]
}

fn active_paths_include_mntd() -> bool {
    active_input_paths()
        .iter()
        .any(|path| path.starts_with("/mnt/d/"))
}

fn json_contains_string(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(items) => items.iter().any(|item| json_contains_string(item, needle)),
        Value::Object(map) => map.values().any(|item| json_contains_string(item, needle)),
        _ => false,
    }
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

fn parse_sha256_prefixed(value: &str) -> Result<[u8; 32]> {
    let raw = value
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("expected sha256:<hex>, got {value}"))?;
    let bytes = hex::decode(raw)?;
    ensure!(bytes.len() == 32, "sha256 must decode to 32 bytes");
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256_bytes(bytes))
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    Ok(sha256_hex(&fs::read(path.as_ref())?))
}
